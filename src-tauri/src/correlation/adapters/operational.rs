//! Normalization for the provider-neutral Sprint 11 operational producers.
//!
//! These adapters consume already captured `NormalizedAlert`, `AnomalySignal`
//! and `HealthCheckResult` values (or equivalent deterministic replay JSON).
//! They never create a client, query a provider, resolve a credential or call
//! back through a Tauri command.

use chrono::DateTime;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thalassa_domain::{
    AnomalyCondition, AnomalySignal, ConsoleSeverity, EvidenceSourceKind, HealthCheckOutcome,
    HealthCheckResult, Signal, SignalId, SignalKind, SignalPayload, SignalState, SignalTarget,
    SignalTargetKind, SourceRecordRef, SuppressionKind, SuppressionState,
};
use uuid::Uuid;

use crate::observability::alertmanager::{NormalizedAlert, ResourceReference};

use super::super::source_records::{SourceRecordError, SourceRecordInput, SourceRecordStore};
use super::super::{ReplayableSignalFixture, FIXTURE_CLOCK};
use super::{payload_value, SignalAdapter, SignalAdapterError};

const DEFAULT_EVALUATION_TIME: &str = FIXTURE_CLOCK;

/// Adapter for one of the existing operational source kinds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalAdapter {
    source: EvidenceSourceKind,
    policy_version: u64,
    evaluation_time: Option<String>,
}

/// Descriptive alias used by callers that distinguish this from future
/// security adapters while sharing the same trait seam.
pub type OperationalSignalAdapter = OperationalAdapter;

impl Default for OperationalAdapter {
    fn default() -> Self {
        Self::new(EvidenceSourceKind::Alertmanager)
    }
}

impl OperationalAdapter {
    pub fn new(source: EvidenceSourceKind) -> Self {
        Self {
            source,
            policy_version: 0,
            evaluation_time: None,
        }
    }

    pub fn with_policy_version(mut self, policy_version: u64) -> Self {
        self.policy_version = policy_version;
        self
    }

    pub fn with_evaluation_time(mut self, evaluation_time: impl Into<String>) -> Self {
        self.evaluation_time = Some(evaluation_time.into());
        self
    }

    pub fn source(&self) -> EvidenceSourceKind {
        self.source
    }
}

impl SignalAdapter for OperationalAdapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        self.source
    }

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError> {
        validate_fixture_for_source(self.source, fixture)?;
        match self.source {
            EvidenceSourceKind::Alertmanager => normalize_alert_fixture(
                fixture,
                records,
                self.policy_version,
                self.evaluation_time.as_deref(),
            ),
            EvidenceSourceKind::Prometheus => normalize_anomaly_fixture(
                fixture,
                records,
                self.policy_version,
                self.evaluation_time.as_deref(),
            ),
            EvidenceSourceKind::HealthCheck => normalize_health_fixture(
                fixture,
                records,
                self.policy_version,
                self.evaluation_time.as_deref(),
            ),
            _ => Err(SignalAdapterError::UnsupportedSource),
        }
    }
}

/// Normalize one operational replay fixture using the source kind recorded in
/// the fixture.
pub fn normalize_operational(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    OperationalAdapter::new(fixture.source_kind).normalize(fixture, records)
}

/// Normalize an existing Sprint 11 `NormalizedAlert` while retaining the
/// complete local replay record as its source evidence.
pub fn normalize_alert(
    alert: &NormalizedAlert,
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    validate_fixture_for_source(EvidenceSourceKind::Alertmanager, fixture)?;
    let source_record = retain_source(
        fixture,
        records,
        Some(alert.fingerprint.clone()),
        revision_from_payload(fixture),
    )?;
    let targets = targets_from_resource_reference(&alert.resource_reference)?;
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::Alert,
        alert_state(&alert.state),
        optional_timestamp(Some(&alert.starts_at))?,
        severity_from_labels(&alert.labels),
        SignalPayload::Alert,
        targets,
        alert.fingerprint.as_str(),
        Some(alert.fingerprint.as_str()),
        revision_from_payload(fixture),
        policy_version_from_fixture(fixture),
        evaluation_time_from_fixture(fixture),
    )
}

/// Normalize an existing Sprint 11 `AnomalySignal` while retaining its source
/// fixture and source evidence.
pub fn normalize_anomaly(
    anomaly: &AnomalySignal,
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    validate_fixture_for_source(EvidenceSourceKind::Prometheus, fixture)?;
    ensure_evidence_id(fixture, &anomaly.evidence_id)?;
    if !fixture.scope.contains(&anomaly.scope) {
        return Err(SignalAdapterError::Source(SourceRecordError::ScopeMismatch));
    }
    let source_record = retain_source(
        fixture,
        records,
        Some(anomaly.id.clone()),
        revision_from_payload(fixture),
    )?;
    let targets = target_from_payload(fixture)?;
    let dedup_identity = (!targets.is_empty()).then_some(anomaly.id.as_str());
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::Anomaly,
        SignalState::Active,
        optional_timestamp(Some(&anomaly.observed_at))?,
        Some(anomaly.severity),
        SignalPayload::Anomaly {
            observed_value: finite_number(anomaly.observed_value)?,
            comparison_value: finite_number(anomaly.comparison_value)?,
            condition: anomaly.condition.clone(),
        },
        targets,
        anomaly.id.as_str(),
        dedup_identity,
        revision_from_payload(fixture),
        policy_version_from_fixture(fixture),
        evaluation_time_from_fixture(fixture),
    )
}

/// Normalize an existing Sprint 11 `HealthCheckResult` while retaining its
/// complete audit/source record.
pub fn normalize_health_check(
    result: &HealthCheckResult,
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    validate_fixture_for_source(EvidenceSourceKind::HealthCheck, fixture)?;
    if !fixture.scope.contains(&result.audit.scope) {
        return Err(SignalAdapterError::Source(SourceRecordError::ScopeMismatch));
    }
    if let Some(evidence_id) = result.evidence_id.as_deref() {
        ensure_evidence_id(fixture, evidence_id)?;
    }
    let source_record = retain_source(
        fixture,
        records,
        Some(result.audit.run_id.clone()),
        revision_from_payload(fixture),
    )?;
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::HealthCheck,
        health_state(result.outcome),
        optional_timestamp(Some(&result.observed_at))?,
        None,
        SignalPayload::HealthCheck {
            outcome: result.outcome,
        },
        target_from_payload(fixture)?,
        result.audit.run_id.as_str(),
        Some(result.schedule_id.as_str()),
        revision_from_payload(fixture),
        result.audit.policy_version,
        Some(result.observed_at.clone()),
    )
}

fn normalize_alert_fixture(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
    policy_version: u64,
    evaluation_time: Option<&str>,
) -> Result<Vec<Signal>, SignalAdapterError> {
    let payload = object(payload_value(fixture))?;
    let native_id = optional_string(payload, "fingerprint")?;
    let revision = revision_from_payload(fixture);
    let source_record = retain_source(fixture, records, native_id.clone(), revision.clone())?;
    let state = optional_string(payload, "state")?
        .or_else(|| nested_string(payload, "status", "state"))
        .as_deref()
        .map(alert_state)
        .unwrap_or(SignalState::Unknown);
    let observed_at = fixture.observed_at.as_deref().or_else(|| {
        payload
            .get("starts_at")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                payload
                    .get("startsAt")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
            })
    });
    let observed_at = optional_timestamp(observed_at)?;
    let severity = labels_from_payload(payload)
        .as_ref()
        .and_then(severity_from_labels);
    let targets = target_from_payload_or_alert_labels(payload)?;
    let dedup_identity = native_id.clone().or_else(|| {
        targets
            .first()
            .map(|target| format!("{:?}:{}", target.kind, target.id))
    });
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::Alert,
        state,
        observed_at,
        severity,
        SignalPayload::Alert,
        targets,
        native_id.as_deref().unwrap_or(fixture.key.as_str()),
        dedup_identity.as_deref(),
        revision,
        policy_version,
        evaluation_time
            .map(str::to_owned)
            .or_else(|| evaluation_time_from_fixture(fixture)),
    )
}

fn normalize_anomaly_fixture(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
    policy_version: u64,
    evaluation_time: Option<&str>,
) -> Result<Vec<Signal>, SignalAdapterError> {
    let payload = object(payload_value(fixture))?;
    let native_id = optional_string(payload, "native_id")?.or(optional_string(payload, "id")?);
    let revision = revision_from_payload(fixture);
    let source_record = retain_source(fixture, records, native_id.clone(), revision.clone())?;
    let rule_id = required_string(payload, "rule_id")?;
    let metric_key = required_string(payload, "metric_key")?;
    let observed_value = required_number(payload, "observed_value")?;
    let comparison_value = required_number(payload, "comparison_value")?;
    let condition = condition_from_payload(payload, comparison_value)?;
    let target = target_from_payload(fixture)?;
    let dedup_identity = (!target.is_empty()).then(|| {
        format!("rule={rule_id};metric={metric_key};condition={condition:?};target={target:?}")
    });
    let severity = optional_string(payload, "severity")?
        .as_deref()
        .and_then(parse_console_severity);
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::Anomaly,
        SignalState::Active,
        fixture_timestamp(fixture)?,
        severity,
        SignalPayload::Anomaly {
            observed_value,
            comparison_value,
            condition,
        },
        target,
        native_id.as_deref().unwrap_or(&rule_id),
        dedup_identity.as_deref(),
        revision,
        policy_version,
        evaluation_time
            .map(str::to_owned)
            .or_else(|| evaluation_time_from_fixture(fixture)),
    )
}

fn normalize_health_fixture(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
    policy_version: u64,
    evaluation_time: Option<&str>,
) -> Result<Vec<Signal>, SignalAdapterError> {
    let payload = object(payload_value(fixture))?;
    let schedule_id = optional_string(payload, "schedule_id")?;
    let run_id = optional_string(payload, "run_id")?;
    let outcome = optional_string(payload, "outcome")?
        .ok_or(SignalAdapterError::MalformedPayload)
        .and_then(|value| {
            serde_json::from_value(Value::String(value))
                .map_err(|_| SignalAdapterError::MalformedPayload)
        })?;
    let native_id = run_id
        .or(schedule_id)
        .ok_or(SignalAdapterError::MalformedPayload)?;
    let revision = revision_from_payload(fixture);
    let source_record = retain_source(fixture, records, Some(native_id.clone()), revision.clone())?;
    let dedup_identity = payload
        .get("schedule_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(native_id.as_str());
    build_signal(
        fixture,
        records,
        source_record,
        SignalKind::HealthCheck,
        health_state(outcome),
        fixture_timestamp(fixture)?,
        None,
        SignalPayload::HealthCheck { outcome },
        target_from_payload(fixture)?,
        &native_id,
        Some(dedup_identity),
        revision,
        policy_version,
        evaluation_time
            .map(str::to_owned)
            .or_else(|| evaluation_time_from_fixture(fixture)),
    )
}

fn validate_fixture_for_source(
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

fn retain_source(
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

#[allow(clippy::too_many_arguments)]
fn build_signal(
    fixture: &ReplayableSignalFixture,
    records: &SourceRecordStore,
    source_record: SourceRecordRef,
    kind: SignalKind,
    state: SignalState,
    observed_at: Option<String>,
    business_severity: Option<ConsoleSeverity>,
    payload: SignalPayload,
    targets: Vec<SignalTarget>,
    stable_identity: &str,
    dedup_identity: Option<&str>,
    revision: Option<String>,
    policy_version: u64,
    evaluation_time: Option<String>,
) -> Result<Vec<Signal>, SignalAdapterError> {
    let evaluation_time = evaluation_time
        .or_else(|| fixture.ingested_at.clone())
        .or_else(|| fixture.observed_at.clone())
        .unwrap_or_else(|| DEFAULT_EVALUATION_TIME.to_owned());
    let source_query = source_query(fixture)?;
    let evidence_ids = source_record.evidence_ids.clone();
    let source_digest = source_record.content_digest.clone();
    let signal = Signal {
        id: stable_signal_id(
            fixture.source_kind,
            kind,
            stable_identity,
            &source_record.content_digest,
            revision.as_deref(),
        ),
        kind,
        source: fixture.source_kind,
        state,
        observed_at,
        ingested_at: fixture.ingested_at.clone(),
        scope: fixture.scope.clone(),
        targets,
        business_severity,
        payload,
        source_record,
        dedup_key: stable_dedup_key(fixture.source_kind, kind, dedup_identity.unwrap_or("")),
        suppression: SuppressionState {
            kind: SuppressionKind::NotSuppressed,
            rule_ids: vec![],
            maintenance_window_ids: vec![],
            evaluated_at: evaluation_time.clone(),
            policy_version,
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
    // `records` is intentionally accepted by this helper to make the source
    // lookup relationship explicit at the call site and to catch accidental
    // construction of a Signal without a retained row.
    if records.get(&signal.source_record).is_none() {
        return Err(SignalAdapterError::Source(
            SourceRecordError::EvidenceMissing,
        ));
    }
    Ok(vec![signal])
}

fn source_query(fixture: &ReplayableSignalFixture) -> Result<String, SignalAdapterError> {
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

fn object(value: &Value) -> Result<&Map<String, Value>, SignalAdapterError> {
    value
        .as_object()
        .ok_or(SignalAdapterError::MalformedPayload)
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, SignalAdapterError> {
    optional_string(object, key)?.ok_or(SignalAdapterError::MalformedPayload)
}

fn optional_string(
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

fn nested_string(object: &Map<String, Value>, parent: &str, child: &str) -> Option<String> {
    object
        .get(parent)
        .and_then(Value::as_object)
        .and_then(|object| object.get(child))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn required_number(object: &Map<String, Value>, key: &str) -> Result<f64, SignalAdapterError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or(SignalAdapterError::InvalidNumber)?;
    finite_number(value)
}

fn finite_number(value: f64) -> Result<f64, SignalAdapterError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(SignalAdapterError::InvalidNumber)
}

fn optional_timestamp(value: Option<&str>) -> Result<Option<String>, SignalAdapterError> {
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

fn fixture_timestamp(
    fixture: &ReplayableSignalFixture,
) -> Result<Option<String>, SignalAdapterError> {
    optional_timestamp(fixture.observed_at.as_deref())
}

fn revision_from_payload(fixture: &ReplayableSignalFixture) -> Option<String> {
    fixture
        .recorded_json
        .as_object()
        .and_then(|payload| payload.get("revision"))
        .and_then(Value::as_str)
        .filter(|revision| !revision.trim().is_empty())
        .map(str::to_owned)
}

fn policy_version_from_fixture(_fixture: &ReplayableSignalFixture) -> u64 {
    0
}

fn evaluation_time_from_fixture(fixture: &ReplayableSignalFixture) -> Option<String> {
    fixture
        .ingested_at
        .clone()
        .or_else(|| fixture.observed_at.clone())
}

fn alert_state(value: &str) -> SignalState {
    match value.trim().to_ascii_lowercase().as_str() {
        "firing" | "active" => SignalState::Active,
        "resolved" | "cleared" => SignalState::Cleared,
        _ => SignalState::Unknown,
    }
}

fn health_state(outcome: HealthCheckOutcome) -> SignalState {
    match outcome {
        HealthCheckOutcome::Healthy
        | HealthCheckOutcome::SkippedNotDue
        | HealthCheckOutcome::SkippedCooldown
        | HealthCheckOutcome::SkippedDisabled => SignalState::Observed,
        HealthCheckOutcome::Degraded => SignalState::Active,
        HealthCheckOutcome::Unavailable | HealthCheckOutcome::TimedOut => SignalState::Unknown,
    }
}

fn severity_from_labels(
    labels: &std::collections::BTreeMap<String, String>,
) -> Option<ConsoleSeverity> {
    labels
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("severity"))
        .and_then(|(_, value)| parse_console_severity(value))
}

fn parse_console_severity(value: &str) -> Option<ConsoleSeverity> {
    match value.trim().to_ascii_uppercase().as_str() {
        "S1" | "CRITICAL" => Some(ConsoleSeverity::S1),
        "S2" | "HIGH" => Some(ConsoleSeverity::S2),
        "S3" | "WARNING" | "WARN" => Some(ConsoleSeverity::S3),
        "S4" | "INFO" => Some(ConsoleSeverity::S4),
        "S5" => Some(ConsoleSeverity::S5),
        _ => None,
    }
}

fn labels_from_payload(
    payload: &Map<String, Value>,
) -> Option<std::collections::BTreeMap<String, String>> {
    payload.get("labels").and_then(|value| {
        serde_json::from_value::<std::collections::BTreeMap<String, String>>(value.clone()).ok()
    })
}

fn target_from_payload_or_alert_labels(
    payload: &Map<String, Value>,
) -> Result<Vec<SignalTarget>, SignalAdapterError> {
    if payload.get("target").is_some() {
        return target_from_value(payload.get("target"));
    }
    let Some(labels) = labels_from_payload(payload) else {
        return Ok(vec![]);
    };
    let namespace = labels.get("namespace");
    let mut candidates = Vec::new();
    for (label, kind) in [
        ("pod", SignalTargetKind::Resource),
        ("service", SignalTargetKind::Service),
        ("deployment", SignalTargetKind::Deployment),
    ] {
        if let Some(name) = labels.get(label).filter(|name| !name.trim().is_empty()) {
            candidates.push((kind, name));
        }
    }
    if candidates.len() != 1
        || namespace.is_none()
        || namespace.is_some_and(|value| value.trim().is_empty())
    {
        return Ok(vec![]);
    }
    let (kind, name) = candidates[0];
    let id = format!("{}/{}", namespace.unwrap(), name);
    Ok(vec![SignalTarget { kind, id }])
}

fn target_from_payload(
    fixture: &ReplayableSignalFixture,
) -> Result<Vec<SignalTarget>, SignalAdapterError> {
    let Some(payload) = fixture.recorded_json.as_object() else {
        return Err(SignalAdapterError::MalformedPayload);
    };
    target_from_value(payload.get("target"))
}

fn target_from_value(value: Option<&Value>) -> Result<Vec<SignalTarget>, SignalAdapterError> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    if value.is_null() {
        return Ok(vec![]);
    }
    let target: SignalTarget =
        serde_json::from_value(value.clone()).map_err(|_| SignalAdapterError::MalformedPayload)?;
    target.validate().map_err(SignalAdapterError::Signal)?;
    Ok(vec![target])
}

fn targets_from_resource_reference(
    reference: &ResourceReference,
) -> Result<Vec<SignalTarget>, SignalAdapterError> {
    let ResourceReference::Resolved {
        namespace,
        kind,
        name,
    } = reference
    else {
        return Ok(vec![]);
    };
    let target_kind = match kind.to_ascii_lowercase().as_str() {
        "pod" | "node" | "host" => SignalTargetKind::Resource,
        "service" => SignalTargetKind::Service,
        "deployment" | "workload" => SignalTargetKind::Deployment,
        _ => return Ok(vec![]),
    };
    let id = format!("{namespace}/{name}");
    let target = SignalTarget {
        kind: target_kind,
        id,
    };
    target.validate().map_err(SignalAdapterError::Signal)?;
    Ok(vec![target])
}

fn condition_from_payload(
    payload: &Map<String, Value>,
    comparison_value: f64,
) -> Result<AnomalyCondition, SignalAdapterError> {
    if let Some(condition) = payload.get("condition") {
        let condition: AnomalyCondition = serde_json::from_value(condition.clone())
            .map_err(|_| SignalAdapterError::MalformedPayload)?;
        condition
            .validate()
            .map_err(|_| SignalAdapterError::MalformedPayload)?;
        return Ok(condition);
    }
    // A few Sprint 11 grouping fixtures carry only the already-evaluated
    // comparison.  Preserve a typed condition without inventing a target or
    // a severity; the comparison itself is the source-provided bound.
    Ok(AnomalyCondition::Threshold {
        operator: thalassa_domain::ThresholdOperator::GreaterThanOrEqual,
        threshold: comparison_value.to_string(),
    })
}

fn ensure_evidence_id(
    fixture: &ReplayableSignalFixture,
    evidence_id: &str,
) -> Result<(), SignalAdapterError> {
    if fixture.evidence.iter().any(|item| item.id == evidence_id) {
        Ok(())
    } else {
        Err(SignalAdapterError::Source(
            SourceRecordError::EvidenceMissing,
        ))
    }
}

fn stable_signal_id(
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
    // RFC 4122 version 5/variant bits make the deterministic bytes a valid
    // UUID without relying on UUID v4 randomness.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn stable_dedup_key(
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
