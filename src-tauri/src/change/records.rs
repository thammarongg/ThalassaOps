//! Post-policy retention for replayed change source records.
//!
//! Change admission deliberately works with the existing Sprint 13
//! [`SourceRecordStore`].  The caller owns the store, its SQLite connection,
//! scope and policy; this module never creates process-global state or an
//! implicit workspace.

use chrono::{DateTime, Utc};
use serde_json::Value;
use thalassa_domain::{
    ChangeError, ConsoleEvidenceId, EvidenceRedaction, EvidenceRef, EvidenceSourceKind,
    ResourceScope, SourceRecordRef,
};

use crate::correlation::{SourceRecordError, SourceRecordInput, SourceRecordStore};

/// The complete post-policy JSON value and the evidence minted for it.
#[derive(Clone, Debug, PartialEq)]
pub struct AdmittedRecord {
    pub record_ref: SourceRecordRef,
    pub body: Value,
    pub evidence: Vec<EvidenceRef>,
}

/// Admit one provider payload into the caller-owned, scoped source ledger.
///
/// Diff bodies are removed before any policy or digest work.  The existing
/// source-record store then applies its masking and local-storage policy,
/// writes the shared `source_record_evidence` row and returns the canonical
/// `SourceRecordRef`.  The Sprint 14 payload row is written only after that
/// shared admission succeeds.
pub fn admit(
    store: &mut SourceRecordStore,
    payload: &str,
    source: EvidenceSourceKind,
    scope: &ResourceScope,
    clock: DateTime<Utc>,
) -> Result<AdmittedRecord, ChangeError> {
    if !scope.is_bounded() {
        return Err(ChangeError::ScopeMismatch);
    }
    let mut body: Value =
        serde_json::from_str(payload).map_err(|_| ChangeError::MalformedPayload)?;
    if !body.is_object() && !body.is_array() {
        return Err(ChangeError::MalformedPayload);
    }

    // This is intentionally the first transformation after parsing.  Diff
    // hunk content is never handed to policy, digesting, retention or the
    // normalized contract.
    strip_diff_body_fields(&mut body);
    // A URL carrying a query, fragment or userinfo is not safe source data.
    // Keep the source field as explicit null so normalization can report the
    // downgrade, while ensuring query credentials never enter the ledger.
    null_unsafe_urls(&mut body);

    let occurred_at = occurred_at_for(source, &body).ok_or(ChangeError::MissingTimestamp)?;
    DateTime::parse_from_rfc3339(&occurred_at).map_err(|_| ChangeError::InvalidTimestamp)?;
    let admitted_at = clock.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let native_id = native_id_for(source, &body);
    let revision = revision_for(source, &body);

    // The existing store owns masking and digest computation.  Its digest is
    // mirrored here solely to mint a deterministic evidence ID before the
    // store's `retain` call; no change-specific evidence format is introduced.
    let evidence_id = SourceRecordStore::content_digest_for(&body);
    let evidence = EvidenceRef {
        id: evidence_id,
        source_kind: source,
        connector_id: None,
        scope: scope.clone(),
        endpoint: format!("fixture://change/{}", source_wire(source)),
        query: None,
        observed_at: occurred_at.clone(),
        excerpt: "retained".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    };

    let input = SourceRecordInput::new(
        source,
        native_id,
        revision,
        scope.clone(),
        body,
        vec![evidence],
    )
    .with_times(Some(occurred_at.clone()), Some(admitted_at.clone()));
    let record_ref = store.retain(input).map_err(map_source_error)?;
    let retained_body = store
        .payload_for(&record_ref)
        .cloned()
        .ok_or(ChangeError::InvalidSourceRecord)?;
    store
        .persist_change_record(
            &record_ref.content_digest,
            source,
            record_ref.native_id.as_deref(),
            record_ref.revision.as_deref(),
            &occurred_at,
            &admitted_at,
            &retained_body,
        )
        .map_err(map_source_error)?;
    let evidence = store
        .evidence_for_record(&record_ref)
        .map_err(map_source_error)?;

    Ok(AdmittedRecord {
        record_ref,
        body: retained_body,
        evidence,
    })
}

/// Resolve evidence through the shared Sprint 13 source-record store.
pub fn resolve_evidence(store: &SourceRecordStore, id: &ConsoleEvidenceId) -> Option<EvidenceRef> {
    store.evidence(id).cloned()
}

fn map_source_error(error: SourceRecordError) -> ChangeError {
    match error {
        SourceRecordError::ScopeMismatch => ChangeError::ScopeMismatch,
        SourceRecordError::EvidenceMissing => ChangeError::EvidenceMissing,
        SourceRecordError::PolicyDenied => ChangeError::PolicyDenied,
        SourceRecordError::InvalidTimestamp => ChangeError::InvalidTimestamp,
        SourceRecordError::UnsafeIdentity => ChangeError::UnsafeIdentity,
        SourceRecordError::InvalidPayload => ChangeError::InvalidSourceRecord,
        SourceRecordError::InvalidScope => ChangeError::ScopeMismatch,
        SourceRecordError::SourceMismatch
        | SourceRecordError::DuplicateEvidence
        | SourceRecordError::AmbiguousSourceIdentity
        | SourceRecordError::InvalidEvidence
        | SourceRecordError::Contract(_)
        | SourceRecordError::Database(_) => ChangeError::InvalidSourceRecord,
    }
}

fn strip_diff_body_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| {
                !matches!(
                    key.to_ascii_lowercase().as_str(),
                    "patch" | "diff" | "content"
                )
            });
            for child in object.values_mut() {
                strip_diff_body_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_diff_body_fields(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn null_unsafe_urls(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                if let Value::String(candidate) = child {
                    if is_unsafe_url(candidate) {
                        *child = Value::Null;
                    }
                } else {
                    null_unsafe_urls(child);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                null_unsafe_urls(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_unsafe_url(value: &str) -> bool {
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    remainder.contains('?') || remainder.contains('#') || authority.contains('@')
}

pub(crate) fn occurred_at_for(source: EvidenceSourceKind, body: &Value) -> Option<String> {
    let paths: &[&[&str]] = match source {
        EvidenceSourceKind::GitHub => &[
            &["head_commit", "timestamp"],
            &["pull_request", "merged_at"],
            &["deployment_status", "created_at"],
            &["deployment", "created_at"],
            &["occurred_at"],
        ],
        EvidenceSourceKind::GitLab => &[
            &["commits", "0", "timestamp"],
            &["object_attributes", "merged_at"],
            &["object_attributes", "finished_at"],
            &["object_attributes", "created_at"],
            &["occurred_at"],
        ],
        EvidenceSourceKind::ArgoCd => &[
            &["operationState", "finishedAt"],
            &["operationState", "startedAt"],
            &["occurred_at"],
        ],
        _ => &[&["occurred_at"]],
    };
    paths.iter().find_map(|path| string_at_path(body, path))
}

fn native_id_for(source: EvidenceSourceKind, body: &Value) -> Option<String> {
    let paths: &[&[&str]] = match source {
        EvidenceSourceKind::GitHub => &[
            &["head_commit", "id"],
            &["pull_request", "number"],
            &["deployment_status", "id"],
            &["deployment", "id"],
            &["after"],
        ],
        EvidenceSourceKind::GitLab => {
            &[&["after"], &["object_attributes", "id"], &["project", "id"]]
        }
        EvidenceSourceKind::ArgoCd => &[
            &["operationState", "syncResult", "revision"],
            &["rollback", "toRevision"],
            &["application", "metadata", "name"],
        ],
        _ => &[&["id"]],
    };
    paths
        .iter()
        .find_map(|path| scalar_string_at_path(body, path))
}

pub(crate) fn revision_for(source: EvidenceSourceKind, body: &Value) -> Option<String> {
    let paths: &[&[&str]] = match source {
        EvidenceSourceKind::GitHub => &[
            &["head_commit", "id"],
            &["pull_request", "merge_commit_sha"],
            &["deployment", "sha"],
            &["after"],
        ],
        EvidenceSourceKind::GitLab => &[
            &["object_attributes", "sha"],
            &["object_attributes", "merge_commit_sha"],
            &["after"],
        ],
        EvidenceSourceKind::ArgoCd => &[
            &["operationState", "syncResult", "revision"],
            &["rollback", "toRevision"],
            &["application", "spec", "source", "targetRevision"],
        ],
        _ => &[&["revision"]],
    };
    paths
        .iter()
        .find_map(|path| scalar_string_at_path(body, path))
}

fn string_at_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for component in path {
        current = match current {
            Value::Object(object) => object.get(*component)?,
            Value::Array(values) => values.get(component.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    match current {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn scalar_string_at_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for component in path {
        current = match current {
            Value::Object(object) => object.get(*component)?,
            Value::Array(values) => values.get(component.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    match current {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn source_wire(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::Alertmanager => "alertmanager",
        EvidenceSourceKind::Prometheus => "prometheus",
        EvidenceSourceKind::Kubernetes => "kubernetes",
        EvidenceSourceKind::Cloud => "cloud",
        EvidenceSourceKind::HealthCheck => "health_check",
        EvidenceSourceKind::Fixture => "fixture",
        EvidenceSourceKind::Trivy => "trivy",
        EvidenceSourceKind::Falco => "falco",
        EvidenceSourceKind::Kyverno => "kyverno",
        EvidenceSourceKind::OpaGatekeeper => "opa_gatekeeper",
        EvidenceSourceKind::GitHub => "github",
        EvidenceSourceKind::GitLab => "gitlab",
        EvidenceSourceKind::ArgoCd => "argo_cd",
    }
}
