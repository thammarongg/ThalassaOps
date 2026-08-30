//! Normalize an admitted provider payload into the canonical change event.
//!
//! This module only indexes source fields.  It never fetches or rewrites a
//! provider record, and every safe downgrade is returned as a typed
//! `SourceStatus` alongside the event.

use chrono::DateTime;
use serde_json::Value;
use thalassa_domain::{
    ChangeActor, ChangeActorKind, ChangeDiffStat, ChangeError, ChangeEvent, ChangeKind,
    ChangeLinkKind, ChangeOutcome, ChangeRepositoryRef, ChangeRevision, ChangeSourceLink,
    DrillDownDestination, DrillDownReference, DrillDownTarget, EvidenceSourceKind, NumberUnit,
    SignalTarget, SignalTargetKind, SourceRecordRef, SourceState, SourceStatus, StatusReason,
};
use uuid::Uuid;

use super::records::{occurred_at_for, revision_for, AdmittedRecord};

/// A normalized event and the typed source statuses produced while safely
/// downgrading source fields.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationOutput {
    pub event: ChangeEvent,
    pub statuses: Vec<SourceStatus>,
}

/// Normalize one retained source record without substituting ingestion time
/// for a missing source timestamp.
pub fn to_change_event(record: &AdmittedRecord) -> Result<NormalizationOutput, ChangeError> {
    let scope = record
        .evidence
        .first()
        .map(|evidence| evidence.scope.clone())
        .ok_or(ChangeError::EvidenceMissing)?;
    if record
        .evidence
        .iter()
        .any(|evidence| evidence.scope != scope)
    {
        return Err(ChangeError::ScopeMismatch);
    }
    let occurred_at = occurred_at_for(record.record_ref.source_kind, &record.body)
        .ok_or(ChangeError::MissingTimestamp)?;
    DateTime::parse_from_rfc3339(&occurred_at).map_err(|_| ChangeError::InvalidTimestamp)?;

    let mut statuses = Vec::new();
    let source = record.record_ref.source_kind;
    let actor = actor_for(record, &mut statuses);
    let repository = repository_for(record, &mut statuses);
    let targets = targets_for(record, &repository);
    let revision = revision_for(source, &record.body).map(|id| ChangeRevision {
        short_id: Some(id.chars().take(7).collect()),
        id,
        parent_ids: parent_ids_for(source, &record.body),
    });
    let changed_paths = changed_paths_for(record, &mut statuses);
    let source_link = source_link_for(record, &mut statuses);
    let event = ChangeEvent {
        id: event_id(&record.record_ref)?,
        source,
        kind: kind_for(source, &record.body)?,
        outcome: outcome_for(source, &record.body)?,
        occurred_at,
        ingested_at: None,
        scope: scope.clone(),
        targets,
        revision,
        actor,
        repository,
        environment: environment_for(source, &record.body),
        diff_stat: diff_stat_for(source, &record.body)?,
        changed_paths,
        source_link,
        source_record: record.record_ref.clone(),
        evidence_ids: record.record_ref.evidence_ids.clone(),
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::Evidence,
            evidence_ids: record.record_ref.evidence_ids.clone(),
            filter_key: Some(record.record_ref.content_digest.clone()),
        },
        drill_down_reference: DrillDownReference {
            source_query: "change_source_record".into(),
            scope,
            time_window: None,
            evidence_ids: record.record_ref.evidence_ids.clone(),
        },
    };
    event.validate()?;
    Ok(NormalizationOutput { event, statuses })
}

fn event_id(reference: &SourceRecordRef) -> Result<Uuid, ChangeError> {
    let encoded = reference
        .content_digest
        .strip_prefix("sha256:")
        .ok_or(ChangeError::InvalidSourceRecord)?;
    if encoded.len() < 32
        || !encoded
            .chars()
            .take(32)
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ChangeError::InvalidSourceRecord);
    }
    let mut bytes = [0u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
            .map_err(|_| ChangeError::InvalidSourceRecord)?;
    }
    let id = Uuid::from_bytes(bytes);
    if id.is_nil() {
        return Err(ChangeError::InvalidId);
    }
    Ok(id)
}

fn kind_for(source: EvidenceSourceKind, body: &Value) -> Result<ChangeKind, ChangeError> {
    let kind = match source {
        EvidenceSourceKind::GitHub => match string_at_path(body, &["event_type"]).as_deref() {
            Some("push") => ChangeKind::CodeCommit,
            Some("pull_request") => ChangeKind::CodeMerge,
            Some("deployment_status") => ChangeKind::Deployment,
            _ => return Err(ChangeError::MalformedPayload),
        },
        EvidenceSourceKind::GitLab => match string_at_path(body, &["object_kind"]).as_deref() {
            Some("push") => ChangeKind::CodeCommit,
            Some("merge_request") => ChangeKind::CodeMerge,
            Some("pipeline") => ChangeKind::Deployment,
            _ => return Err(ChangeError::MalformedPayload),
        },
        EvidenceSourceKind::ArgoCd => match string_at_path(body, &["type"]).as_deref() {
            Some("sync") => ChangeKind::Sync,
            Some("rollback") => ChangeKind::Rollback,
            _ => return Err(ChangeError::MalformedPayload),
        },
        _ => return Err(ChangeError::MalformedPayload),
    };
    Ok(kind)
}

fn outcome_for(source: EvidenceSourceKind, body: &Value) -> Result<ChangeOutcome, ChangeError> {
    let value = match source {
        EvidenceSourceKind::GitHub => string_at_path(body, &["deployment_status", "state"]),
        EvidenceSourceKind::GitLab => string_at_path(body, &["object_attributes", "status"]),
        EvidenceSourceKind::ArgoCd => string_at_path(body, &["operationState", "phase"]),
        _ => None,
    };
    let Some(value) = value else {
        return Ok(ChangeOutcome::Unknown);
    };
    match value.to_ascii_lowercase().as_str() {
        "success" | "succeeded" | "successful" => Ok(ChangeOutcome::Succeeded),
        "failed" | "failure" => Ok(ChangeOutcome::Failed),
        "running" | "in_progress" | "pending" => Ok(ChangeOutcome::InProgress),
        "reverted" | "rollback" => Ok(ChangeOutcome::Reverted),
        _ => Err(ChangeError::MalformedPayload),
    }
}

fn actor_for(record: &AdmittedRecord, statuses: &mut Vec<SourceStatus>) -> ChangeActor {
    let source = record.record_ref.source_kind;
    let candidate = match source {
        EvidenceSourceKind::GitHub => first_string(
            &record.body,
            &[
                &["pull_request", "user", "login"],
                &["deployment_status", "creator", "login"],
                &["sender", "login"],
                &["pusher", "name"],
            ],
        ),
        EvidenceSourceKind::GitLab => {
            first_string(&record.body, &[&["user", "username"], &["user_username"]])
        }
        EvidenceSourceKind::ArgoCd => None,
        _ => None,
    };
    let email_present = has_email_identity(&record.body);
    let unsafe_handle = candidate
        .as_deref()
        .is_some_and(|handle| looks_like_email(handle) || !safe_identity(handle));
    if email_present || unsafe_handle {
        push_status(record, statuses, "actor");
        return ChangeActor {
            kind: ChangeActorKind::Unknown,
            handle: None,
        };
    }
    let Some(handle) = candidate else {
        return ChangeActor {
            kind: ChangeActorKind::Unknown,
            handle: None,
        };
    };
    let kind = if handle.to_ascii_lowercase().contains("bot")
        || handle.to_ascii_lowercase().contains("release")
        || handle.to_ascii_lowercase().contains("argo")
    {
        ChangeActorKind::Automation
    } else {
        ChangeActorKind::Human
    };
    ChangeActor {
        kind,
        handle: Some(handle),
    }
}

fn repository_for(
    record: &AdmittedRecord,
    statuses: &mut Vec<SourceStatus>,
) -> Option<ChangeRepositoryRef> {
    let source = record.record_ref.source_kind;
    let (host, repository, reference) = match source {
        EvidenceSourceKind::GitHub => (
            "github.com".to_owned(),
            first_string(
                &record.body,
                &[
                    &["repository", "full_name"],
                    &["pull_request", "base", "repo", "full_name"],
                ],
            ),
            first_string(&record.body, &[&["ref"], &["pull_request", "base", "ref"]]),
        ),
        EvidenceSourceKind::GitLab => (
            "gitlab.com".to_owned(),
            first_string(&record.body, &[&["project", "path_with_namespace"]]),
            first_string(
                &record.body,
                &[&["ref"], &["object_attributes", "target_branch"]],
            ),
        ),
        EvidenceSourceKind::ArgoCd => {
            let url = string_at_path(&record.body, &["application", "spec", "source", "repoURL"]);
            let (host, repository) = url
                .as_deref()
                .and_then(split_repository_url)
                .unwrap_or_else(|| (String::new(), String::new()));
            (
                host,
                Some(repository),
                first_string(
                    &record.body,
                    &[&["application", "spec", "source", "targetRevision"]],
                ),
            )
        }
        _ => return None,
    };
    let repository = repository?;
    let mut parts = repository.split('/');
    let namespace = parts.next().unwrap_or_default().to_owned();
    let name = parts.next().unwrap_or_default().to_owned();
    if namespace.is_empty()
        || name.is_empty()
        || !safe_identity(&host)
        || !safe_identity(&namespace)
        || !safe_identity(&name)
        || reference
            .as_deref()
            .is_some_and(|value| !safe_identity(value))
    {
        push_status(record, statuses, "repository");
        return None;
    }
    Some(ChangeRepositoryRef {
        host,
        namespace,
        name,
        reference: reference.map(|value| {
            value
                .strip_prefix("refs/heads/")
                .unwrap_or(&value)
                .to_owned()
        }),
    })
}

fn targets_for(
    record: &AdmittedRecord,
    repository: &Option<ChangeRepositoryRef>,
) -> Vec<SignalTarget> {
    let name = repository
        .as_ref()
        .map(|repository| repository.name.clone())
        .or_else(|| string_at_path(&record.body, &["application", "metadata", "name"]));
    name.filter(|value| safe_identity(value))
        .map(|name| {
            // Sprint 13 names a deployment target `deployment/<name>`. Emitting
            // the same value is what makes association an exact comparison
            // instead of a heuristic match on a bare name.
            vec![SignalTarget {
                kind: SignalTargetKind::Deployment,
                id: format!("deployment/{name}"),
            }]
        })
        .unwrap_or_default()
}

fn parent_ids_for(source: EvidenceSourceKind, body: &Value) -> Vec<String> {
    let mut parents = match source {
        EvidenceSourceKind::GitHub => first_string(body, &[&["before"]])
            .into_iter()
            .filter(|value| !value.chars().all(|character| character == '0'))
            .collect(),
        _ => Vec::new(),
    };
    parents.sort();
    parents.dedup();
    parents
}

fn changed_paths_for(record: &AdmittedRecord, statuses: &mut Vec<SourceStatus>) -> Vec<String> {
    let mut candidates = Vec::new();
    match record.record_ref.source_kind {
        EvidenceSourceKind::GitHub => {
            if let Some(Value::Array(files)) = value_at_path(&record.body, &["files"]) {
                for file in files {
                    if let Some(filename) = string_at_path(file, &["filename"]) {
                        candidates.push(filename);
                    }
                }
            }
        }
        EvidenceSourceKind::GitLab => {
            if let Some(Value::Array(commits)) = value_at_path(&record.body, &["commits"]) {
                for commit in commits {
                    for key in ["added", "modified", "removed"] {
                        if let Some(Value::Array(paths)) = value_at_path(commit, &[key]) {
                            candidates
                                .extend(paths.iter().filter_map(Value::as_str).map(str::to_owned));
                        }
                    }
                }
            }
        }
        EvidenceSourceKind::ArgoCd => {}
        _ => {}
    }
    candidates.sort();
    candidates.dedup();
    candidates
        .into_iter()
        .filter_map(|path| {
            if safe_path(&path) {
                Some(path)
            } else {
                push_status(record, statuses, "changed_path");
                None
            }
        })
        .collect()
}

fn diff_stat_for(
    source: EvidenceSourceKind,
    body: &Value,
) -> Result<Option<ChangeDiffStat>, ChangeError> {
    if source != EvidenceSourceKind::GitHub {
        return Ok(None);
    }
    let Some(Value::Array(files)) = value_at_path(body, &["files"]) else {
        return Ok(None);
    };
    let mut insertions = 0.0;
    let mut deletions = 0.0;
    for file in files {
        insertions += number_at_path(file, &["additions"])?;
        deletions += number_at_path(file, &["deletions"])?;
    }
    Ok(Some(ChangeDiffStat {
        files_changed: files.len() as f64,
        insertions,
        deletions,
        unit: NumberUnit::Count,
    }))
}

fn environment_for(source: EvidenceSourceKind, body: &Value) -> Option<String> {
    let paths: &[&[&str]] = match source {
        EvidenceSourceKind::GitHub => &[&["deployment", "environment"]],
        EvidenceSourceKind::GitLab => &[&["deployment", "environment"]],
        EvidenceSourceKind::ArgoCd => &[&["application", "spec", "destination", "namespace"]],
        _ => &[],
    };
    paths.iter().find_map(|path| string_at_path(body, path))
}

fn source_link_for(
    record: &AdmittedRecord,
    statuses: &mut Vec<SourceStatus>,
) -> Option<ChangeSourceLink> {
    let (kind, paths): (ChangeLinkKind, &[&[&str]]) = match record.record_ref.source_kind {
        EvidenceSourceKind::GitHub => (
            if string_at_path(&record.body, &["pull_request", "html_url"]).is_some() {
                ChangeLinkKind::PullRequest
            } else if string_at_path(&record.body, &["deployment_status", "target_url"]).is_some() {
                ChangeLinkKind::Deployment
            } else {
                ChangeLinkKind::Compare
            },
            &[
                &["pull_request", "html_url"],
                &["deployment_status", "target_url"],
                &["repository", "html_url"],
            ],
        ),
        EvidenceSourceKind::GitLab => (
            if string_at_path(&record.body, &["object_attributes", "url"]).is_some() {
                ChangeLinkKind::PullRequest
            } else {
                ChangeLinkKind::Deployment
            },
            &[
                &["object_attributes", "url"],
                &["deployment", "web_url"],
                &["project", "web_url"],
            ],
        ),
        EvidenceSourceKind::ArgoCd => (ChangeLinkKind::Application, &[&["url"]]),
        _ => return None,
    };
    let path = paths.iter().find(|path| has_path(&record.body, path))?;
    let Some(url) = string_at_path(&record.body, path) else {
        push_status(record, statuses, "source_link");
        return None;
    };
    let link = ChangeSourceLink { kind, url };
    if link.validate(record.record_ref.source_kind).is_err() {
        push_status(record, statuses, "source_link");
        None
    } else {
        Some(link)
    }
}

fn push_status(record: &AdmittedRecord, statuses: &mut Vec<SourceStatus>, field: &str) {
    let source_key = format!(
        "change-status-{}-{}",
        record.record_ref.content_digest.replace(':', "-"),
        field
    );
    if statuses
        .iter()
        .any(|status| status.source_key == source_key)
    {
        return;
    }
    statuses.push(SourceStatus {
        source_key,
        state: SourceState::Unverified,
        reason: Some(StatusReason::Unknown),
        detail: None,
        observed_at: None,
        evidence_ids: record.record_ref.evidence_ids.clone(),
    });
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| string_at_path(value, path))
}

fn string_at_path(value: &Value, path: &[&str]) -> Option<String> {
    match value_at_path(value, path) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn number_at_path(value: &Value, path: &[&str]) -> Result<f64, ChangeError> {
    let Some(value) = value_at_path(value, path) else {
        return Ok(0.0);
    };
    let number = value.as_f64().ok_or(ChangeError::MalformedPayload)?;
    if !number.is_finite() {
        return Err(ChangeError::NonFiniteNumber);
    }
    if number < 0.0 {
        return Err(ChangeError::NegativeNumber);
    }
    Ok(number)
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for component in path {
        current = match current {
            Value::Object(object) => object.get(*component)?,
            Value::Array(values) => values.get(component.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn has_path(value: &Value, path: &[&str]) -> bool {
    value_at_path(value, path).is_some()
}

fn split_repository_url(value: &str) -> Option<(String, String)> {
    let (_, remainder) = value.split_once("://")?;
    let end = remainder.find('/').unwrap_or(remainder.len());
    let host = remainder[..end].to_owned();
    let repository = remainder[end..]
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or_else(|| remainder[end..].trim_matches('/'))
        .to_owned();
    Some((host, repository))
}

fn safe_identity(value: &str) -> bool {
    !value.trim().is_empty()
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && ![
            "password",
            "passwd",
            "secret",
            "token",
            "credential",
            "authorization",
            "cookie",
            "bearer",
            "api_key",
            "access_key",
            "private_key",
            "arn:",
            "/subscriptions/",
            "subscription_id",
            "account_id",
            "pagination",
            "cursor",
            "next_link",
        ]
        .iter()
        .any(|marker| value.to_ascii_lowercase().contains(marker))
}

fn safe_path(value: &str) -> bool {
    safe_identity(value)
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

fn looks_like_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

fn has_email_identity(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            (key.to_ascii_lowercase().contains("email")
                && value.as_str().is_some_and(looks_like_email))
                || has_email_identity(value)
        }),
        Value::Array(values) => values.iter().any(has_email_identity),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}
