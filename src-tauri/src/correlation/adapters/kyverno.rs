//! Replay adapter for the committed Kyverno policy-report result.

use serde_json::Value;
use thalassa_domain::{
    EvidenceSourceKind, FindingAssetKind, Signal, SignalTarget, SignalTargetKind,
};

use super::super::{ReplayableSignalFixture, SourceRecordStore};
use super::{
    build_security_signal, object, optional_string, parse_exploitability, parse_finding_severity,
    payload_value, required_string, retain_source, revision_from_payload, security_state,
    validate_fixture_for_source, validate_source_identity, validate_source_text,
    validate_timestamp, SignalAdapter, SignalAdapterError,
};

/// Adapter for one deterministic Kyverno policy-report result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KyvernoAdapter;

/// Compatibility alias for callers that name adapters by their Signal output.
pub type KyvernoSignalAdapter = KyvernoAdapter;

impl KyvernoAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl SignalAdapter for KyvernoAdapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        EvidenceSourceKind::Kyverno
    }

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError> {
        validate_fixture_for_source(EvidenceSourceKind::Kyverno, fixture)?;
        let payload = object(payload_value(fixture))?;
        let native_id = payload
            .get("policy")
            .and_then(Value::as_str)
            .zip(payload.get("rule").and_then(Value::as_str))
            .map(|(policy, rule)| format!("{policy}:{rule}"));
        let source_record =
            retain_source(fixture, records, native_id, revision_from_payload(fixture))?;

        let policy = required_string(payload, "policy")?;
        let rule = required_string(payload, "rule")?;
        validate_source_identity(&policy)?;
        validate_source_identity(&rule)?;
        let result = required_string(payload, "result")?;
        if !matches!(
            result.trim().to_ascii_lowercase().as_str(),
            "fail" | "failed" | "violation"
        ) {
            return Err(SignalAdapterError::UnsupportedSchema);
        }
        let resource = payload
            .get("resource")
            .ok_or(SignalAdapterError::AmbiguousTarget)?;
        let (namespace, kind, name) = kubernetes_identity(resource)?;
        let violation_path = optional_string(payload, "violation_path")?
            .ok_or(SignalAdapterError::MalformedPayload)?;
        validate_source_text(&violation_path)?;
        let target = kubernetes_target(&kind, &name)?;
        let severity = parse_finding_severity(payload, "severity")?;
        let exploitability = parse_exploitability(payload)?;
        let observed_at = validate_timestamp(fixture.observed_at.as_deref())?;
        let state = security_state(payload)?;
        let stable_identity = format!(
            "policy={policy};rule={rule};namespace={namespace};kind={kind};name={name};path={violation_path}"
        );
        validate_source_identity(&stable_identity)?;

        build_security_signal(
            fixture,
            records,
            source_record,
            target,
            FindingAssetKind::PolicySubject,
            None,
            None,
            severity,
            exploitability,
            None,
            observed_at,
            state,
            &stable_identity,
        )
    }
}

/// Normalize the committed Kyverno replay fixture.
pub fn normalize_kyverno(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    KyvernoAdapter.normalize(fixture, records)
}

pub(super) fn kubernetes_identity(
    value: &Value,
) -> Result<(String, String, String), SignalAdapterError> {
    let object = value
        .as_object()
        .ok_or(SignalAdapterError::AmbiguousTarget)?;
    let namespace = object
        .get("namespace")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(SignalAdapterError::AmbiguousTarget)?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(SignalAdapterError::AmbiguousTarget)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(SignalAdapterError::AmbiguousTarget)?;
    validate_source_identity(namespace)?;
    validate_source_identity(kind)?;
    validate_source_identity(name)?;
    Ok((namespace.to_owned(), kind.to_owned(), name.to_owned()))
}

pub(super) fn kubernetes_target(
    kind: &str,
    name: &str,
) -> Result<SignalTarget, SignalAdapterError> {
    let target_kind = match kind.to_ascii_lowercase().as_str() {
        "deployment" | "replicaset" | "statefulset" | "daemonset" | "job" | "cronjob"
        | "workload" => SignalTargetKind::Deployment,
        "service" => SignalTargetKind::Service,
        "pod" | "node" | "namespace" | "host" => SignalTargetKind::Resource,
        _ => SignalTargetKind::Resource,
    };
    let id_prefix = match target_kind {
        SignalTargetKind::Deployment => "deployment",
        SignalTargetKind::Service => "service",
        SignalTargetKind::Resource => "resource",
        SignalTargetKind::Topology => "topology",
    };
    let target = SignalTarget {
        kind: target_kind,
        id: format!("{id_prefix}/{name}"),
    };
    target.validate().map_err(SignalAdapterError::Signal)?;
    Ok(target)
}
