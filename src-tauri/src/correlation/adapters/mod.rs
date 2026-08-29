//! Source adapters for the common, source-preserving Signal envelope.

use chrono::DateTime;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thalassa_domain::{
    CorrelationError, EvidenceSourceKind, Exploitability, FindingAsset, FindingAssetKind,
    FindingSeverity, Signal, SignalId, SignalKind, SignalPayload, SignalState, SignalTarget,
    SourceRecordRef, SuppressionKind, SuppressionState,
};
use thiserror::Error;
use uuid::Uuid;

use super::source_records::{SourceRecordError, SourceRecordInput, SourceRecordStore};
use super::ReplayableSignalFixture;

pub mod falco;
pub mod gatekeeper;
pub mod kyverno;
pub mod operational;
pub mod trivy;

pub use falco::{normalize_falco, FalcoAdapter, FalcoSignalAdapter};
pub use gatekeeper::{normalize_gatekeeper, GatekeeperAdapter, GatekeeperSignalAdapter};
pub use kyverno::{normalize_kyverno, KyvernoAdapter, KyvernoSignalAdapter};
pub use operational::{
    normalize_alert, normalize_anomaly, normalize_health_check, normalize_operational,
    OperationalAdapter, OperationalSignalAdapter,
};
pub use trivy::{normalize_trivy, TrivyAdapter, TrivySignalAdapter};

/// Normalize one replay fixture through the registered source adapter.
pub fn normalize_security(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    match fixture.source_kind {
        EvidenceSourceKind::Trivy => TrivyAdapter.normalize(fixture, records),
        EvidenceSourceKind::Falco => FalcoAdapter.normalize(fixture, records),
        EvidenceSourceKind::Kyverno => KyvernoAdapter.normalize(fixture, records),
        EvidenceSourceKind::OpaGatekeeper => GatekeeperAdapter.normalize(fixture, records),
        _ => Err(SignalAdapterError::UnsupportedSecuritySource),
    }
}

/// Typed failures returned by source adapters.  Payload details remain in the
/// local rejection/evidence path and are never copied into error strings.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SignalAdapterError {
    #[error("replay fixture failed validation")]
    Fixture(#[source] CorrelationError),
    #[error("source record failed admission")]
    Source(#[source] SourceRecordError),
    #[error("adapter source does not match the fixture source")]
    SourceMismatch,
    #[error("operational source is not supported by this adapter")]
    UnsupportedSource,
    #[error("security source is not supported by this adapter")]
    UnsupportedSecuritySource,
    #[error("security source schema is unsupported")]
    UnsupportedSchema,
    #[error("security source target is ambiguous or missing")]
    AmbiguousTarget,
    #[error("security source identity is unsafe")]
    UnsafeIdentity,
    #[error("security source severity is malformed")]
    InvalidSeverity,
    #[error("security source exploitability is malformed")]
    InvalidExploitability,
    #[error("security source CVSS score is outside its allowed range")]
    CvssOutOfRange,
    #[error("operational source payload is malformed")]
    MalformedPayload,
    #[error("operational source payload contains an invalid number")]
    InvalidNumber,
    #[error("operational source payload contains an invalid timestamp")]
    InvalidTimestamp,
    #[error("normalized signal failed contract validation")]
    Signal(#[source] CorrelationError),
}

impl From<SourceRecordError> for SignalAdapterError {
    fn from(error: SourceRecordError) -> Self {
        Self::Source(error)
    }
}

/// Common seam implemented by every source adapter.
pub trait SignalAdapter {
    fn source_kind(&self) -> EvidenceSourceKind;

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError>;
}

/// Parse a JSON object/array while preserving all source fields in the ledger.
pub(crate) fn payload_value(fixture: &ReplayableSignalFixture) -> &Value {
    &fixture.recorded_json
}

/// Validate a replay fixture before source-specific parsing.
pub(super) fn validate_fixture_for_source(
    source: EvidenceSourceKind,
    fixture: &ReplayableSignalFixture,
) -> Result<(), SignalAdapterError> {
    fixture
        .validate_for_replay()
        .map_err(SignalAdapterError::Fixture)?;
    if fixture.source_kind != source {
        return Err(SignalAdapterError::SourceMismatch);
    }
    Ok(())
}

/// Retain a complete fixture record before constructing typed finding facts.
pub(super) fn retain_source(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
    native_id: Option<String>,
    revision: Option<String>,
) -> Result<SourceRecordRef, SignalAdapterError> {
    records
        .retain(SourceRecordInput::from_fixture(
            fixture, native_id, revision,
        ))
        .map_err(SignalAdapterError::Source)
}

/// Return a non-empty string field, preserving typed malformed-payload errors.
pub(super) fn required_string(
    object: &Map<String, Value>,
    key: &str,
) -> Result<String, SignalAdapterError> {
    optional_string(object, key)?.ok_or(SignalAdapterError::MalformedPayload)
}

/// Return an optional source string.  Null and empty values mean honest absence;
/// a non-string value is a malformed source payload.
pub(super) fn optional_string(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, SignalAdapterError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::String(value) if !value.trim().is_empty() => Ok(Some(value.clone())),
        Value::String(_) => Ok(None),
        _ => Err(SignalAdapterError::MalformedPayload),
    }
}

pub(super) fn object(value: &Value) -> Result<&Map<String, Value>, SignalAdapterError> {
    value
        .as_object()
        .ok_or(SignalAdapterError::MalformedPayload)
}

pub(super) fn validate_source_identity(value: &str) -> Result<(), SignalAdapterError> {
    let lower = value.to_ascii_lowercase();
    if value.trim().is_empty()
        || value.chars().any(|character| character.is_control())
        || [
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
        .any(|marker| lower.contains(marker))
        || contains_sensitive_account_id(&lower)
    {
        return Err(SignalAdapterError::UnsafeIdentity);
    }
    Ok(())
}

pub(super) fn validate_source_text(value: &str) -> Result<(), SignalAdapterError> {
    if value.trim().is_empty()
        || value.chars().any(|character| character.is_control())
        || [
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
        || contains_sensitive_account_id(value)
    {
        return Err(SignalAdapterError::UnsafeIdentity);
    }
    Ok(())
}

fn contains_sensitive_account_id(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("sha256:") || lower.contains("dedup:v1:") || lower.contains("candidate:v1:") {
        return false;
    }
    if looks_like_uuid(value) {
        return false;
    }
    let mut run_length = 0usize;
    for character in value.chars() {
        if character.is_ascii_digit() {
            run_length = run_length.saturating_add(1);
        } else {
            if run_length >= 12 {
                return true;
            }
            run_length = 0;
        }
    }
    run_length >= 12
}

fn looks_like_uuid(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(parts.iter())
            .all(|(length, part)| {
                part.len() == *length && part.chars().all(|c| c.is_ascii_hexdigit())
            })
}

pub(super) fn validate_timestamp(
    value: Option<&str>,
) -> Result<Option<String>, SignalAdapterError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|_| value.to_owned())
        .map(Some)
        .map_err(|_| SignalAdapterError::InvalidTimestamp)
}

/// A revision is source metadata, not part of the normalized finding facts.
pub(super) fn revision_from_payload(fixture: &ReplayableSignalFixture) -> Option<String> {
    let payload = fixture.recorded_json.as_object()?;
    let top_level = payload
        .get("revision")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            payload
                .get("vendor_extension")
                .and_then(Value::as_object)
                .and_then(|extension| extension.get("revision_hint"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
        });
    top_level.or_else(|| {
        payload
            .get("Results")
            .and_then(Value::as_array)
            .and_then(|results| results.first())
            .and_then(Value::as_object)
            .and_then(|result| {
                result
                    .get("revision")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_owned)
                    .or_else(|| {
                        result
                            .get("vendor_extension")
                            .and_then(Value::as_object)
                            .and_then(|extension| extension.get("revision_hint"))
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(str::to_owned)
                    })
            })
    })
}

pub(super) fn source_query(
    fixture: &ReplayableSignalFixture,
) -> Result<String, SignalAdapterError> {
    let evidence = fixture
        .evidence
        .first()
        .ok_or(SignalAdapterError::MalformedPayload)?;
    if let Some(query) = evidence
        .query
        .as_deref()
        .filter(|query| !query.trim().is_empty())
    {
        return Ok(query.to_owned());
    }
    if evidence.endpoint.trim().is_empty() {
        return Err(SignalAdapterError::MalformedPayload);
    }
    Ok(evidence.endpoint.clone())
}

pub(super) fn parse_finding_severity(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<FindingSeverity>, SignalAdapterError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(SignalAdapterError::InvalidSeverity);
    };
    parse_severity_text(value)
}

pub(super) fn parse_severity_text(
    value: &str,
) -> Result<Option<FindingSeverity>, SignalAdapterError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    validate_source_text(value)?;
    let severity = match value.trim().to_ascii_lowercase().as_str() {
        "critical" | "crit" | "emergency" | "alert" => FindingSeverity::Critical,
        "high" | "error" => FindingSeverity::High,
        "medium" | "moderate" | "warning" | "warn" => FindingSeverity::Medium,
        "low" | "notice" | "info" | "informational" => FindingSeverity::Low,
        "negligible" | "debug" => FindingSeverity::Negligible,
        "unknown" | "unspecified" => FindingSeverity::Unknown,
        _ => return Err(SignalAdapterError::InvalidSeverity),
    };
    Ok(Some(severity))
}

pub(super) fn parse_exploitability(
    object: &Map<String, Value>,
) -> Result<Option<Exploitability>, SignalAdapterError> {
    let value = object
        .get("exploitability")
        .or_else(|| object.get("Exploitability"));
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(SignalAdapterError::InvalidExploitability);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    validate_source_text(value).map_err(|_| SignalAdapterError::InvalidExploitability)?;
    let exploitability = match value.trim().to_ascii_lowercase().as_str() {
        "exploited" => Exploitability::Exploited,
        "known_exploit" | "known-exploit" => Exploitability::KnownExploit,
        "probable" => Exploitability::Probable,
        "possible" => Exploitability::Possible,
        "unlikely" => Exploitability::Unlikely,
        "none" => Exploitability::None,
        "unknown" | "unspecified" => Exploitability::Unknown,
        _ => return Err(SignalAdapterError::InvalidExploitability),
    };
    Ok(Some(exploitability))
}

pub(super) fn parse_cvss(object: &Map<String, Value>) -> Result<Option<f64>, SignalAdapterError> {
    let Some(value) = object.get("cvss_score").or_else(|| object.get("CVSS")) else {
        return Ok(None);
    };
    let score = if let Some(score) = value.as_f64() {
        score
    } else if value.is_null() {
        return Ok(None);
    } else if let Some(cvss) = value.as_object() {
        let direct = cvss
            .get("V3Score")
            .or_else(|| cvss.get("v3_score"))
            .or_else(|| cvss.get("score"));
        let score = direct.or_else(|| {
            cvss.values().find_map(|entry| {
                entry.as_object().and_then(|nested| {
                    nested
                        .get("V3Score")
                        .or_else(|| nested.get("v3_score"))
                        .or_else(|| nested.get("score"))
                })
            })
        });
        let Some(score) = score else {
            return Err(SignalAdapterError::UnsupportedSchema);
        };
        if score.is_null() {
            return Ok(None);
        }
        score.as_f64().ok_or(SignalAdapterError::InvalidNumber)?
    } else {
        return Err(SignalAdapterError::InvalidNumber);
    };
    if !score.is_finite() {
        return Err(SignalAdapterError::InvalidNumber);
    }
    if !(0.0..=10.0).contains(&score) {
        return Err(SignalAdapterError::CvssOutOfRange);
    }
    Ok(Some(score))
}

pub(super) fn security_state(
    object: &Map<String, Value>,
) -> Result<SignalState, SignalAdapterError> {
    let value = object
        .get("state")
        .or_else(|| object.get("status"))
        .or_else(|| object.get("result"));
    let Some(value) = value else {
        return Ok(SignalState::Observed);
    };
    let Some(value) = value.as_str() else {
        if value.is_null() {
            return Ok(SignalState::Observed);
        }
        return Err(SignalAdapterError::MalformedPayload);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "active" | "firing" | "open" | "fail" | "failed" | "violation" => Ok(SignalState::Active),
        "cleared" | "resolved" | "pass" | "passed" => Ok(SignalState::Cleared),
        "observed" | "detected" => Ok(SignalState::Observed),
        "unknown" => Ok(SignalState::Unknown),
        _ => Err(SignalAdapterError::UnsupportedSchema),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_security_signal(
    fixture: &ReplayableSignalFixture,
    records: &SourceRecordStore,
    source_record: SourceRecordRef,
    target: SignalTarget,
    asset_kind: FindingAssetKind,
    display_name: Option<String>,
    artifact_digest: Option<String>,
    severity: Option<FindingSeverity>,
    exploitability: Option<Exploitability>,
    cvss_score: Option<f64>,
    observed_at: Option<String>,
    state: SignalState,
    stable_identity: &str,
    dedup_identity: Option<&str>,
) -> Result<Vec<Signal>, SignalAdapterError> {
    validate_source_identity(stable_identity)?;
    let source_query = source_query(fixture)?;
    let evidence_ids = source_record.evidence_ids.clone();
    let source_digest = source_record.content_digest.clone();
    let finding = thalassa_domain::VulnerabilityFinding {
        source: fixture.source_kind,
        asset: FindingAsset {
            kind: asset_kind,
            target: target.clone(),
            display_name,
            artifact_digest,
        },
        severity,
        exploitability,
        cvss_score,
        evidence_ids: evidence_ids.clone(),
    };
    let evaluation_time = fixture
        .ingested_at
        .clone()
        .or_else(|| fixture.observed_at.clone())
        .unwrap_or_else(|| super::FIXTURE_CLOCK.to_owned());
    let signal = Signal {
        id: stable_signal_id(
            fixture.source_kind,
            SignalKind::SecurityFinding,
            stable_identity,
            &source_digest,
            source_record.revision.as_deref(),
        ),
        kind: SignalKind::SecurityFinding,
        source: fixture.source_kind,
        state,
        observed_at,
        ingested_at: fixture.ingested_at.clone(),
        scope: fixture.scope.clone(),
        targets: vec![target],
        business_severity: None,
        payload: SignalPayload::SecurityFinding { finding },
        source_record,
        dedup_key: stable_dedup_key(
            fixture.source_kind,
            SignalKind::SecurityFinding,
            dedup_identity.unwrap_or(""),
        ),
        suppression: SuppressionState {
            kind: SuppressionKind::NotSuppressed,
            rule_ids: vec![],
            maintenance_window_ids: vec![],
            evaluated_at: evaluation_time,
            policy_version: 0,
        },
        evidence_ids: evidence_ids.clone(),
        drill_down: thalassa_domain::DrillDownTarget {
            destination: thalassa_domain::DrillDownDestination::Evidence,
            evidence_ids: evidence_ids.clone(),
            filter_key: Some(source_digest),
        },
        drill_down_reference: thalassa_domain::DrillDownReference {
            source_query,
            scope: fixture.scope.clone(),
            time_window: None,
            evidence_ids,
        },
    };
    signal.validate().map_err(SignalAdapterError::Signal)?;
    if records.get(&signal.source_record).is_none() {
        return Err(SignalAdapterError::Source(
            SourceRecordError::EvidenceMissing,
        ));
    }
    Ok(vec![signal])
}

pub(super) fn stable_signal_id(
    source: EvidenceSourceKind,
    kind: SignalKind,
    native_id: &str,
    content_digest: &str,
    revision: Option<&str>,
) -> SignalId {
    let mut hash = Sha256::new();
    for part in [
        source_wire(source),
        signal_kind_wire(kind),
        native_id,
        content_digest,
        revision.unwrap_or(""),
    ] {
        hash.update(part.as_bytes());
        hash.update([0xff]);
    }
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub(super) fn stable_dedup_key(
    source: EvidenceSourceKind,
    kind: SignalKind,
    identity: &str,
) -> Option<String> {
    if identity.trim().is_empty() {
        return None;
    }
    let mut hash = Sha256::new();
    for part in [source_wire(source), signal_kind_wire(kind), identity] {
        hash.update(part.as_bytes());
        hash.update([0xff]);
    }
    let digest = hash.finalize();
    Some(format!(
        "dedup:v1:{}:{}:{digest:x}",
        source_wire(source),
        signal_kind_wire(kind)
    ))
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
    }
}

fn signal_kind_wire(kind: SignalKind) -> &'static str {
    match kind {
        SignalKind::Alert => "alert",
        SignalKind::Anomaly => "anomaly",
        SignalKind::SecurityFinding => "security_finding",
        SignalKind::HealthCheck => "health_check",
    }
}
