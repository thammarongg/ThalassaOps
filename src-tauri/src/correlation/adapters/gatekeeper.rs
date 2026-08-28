//! Replay adapter for the committed OPA Gatekeeper violation fixture.

use serde_json::Value;
use thalassa_domain::{EvidenceSourceKind, FindingAssetKind, Signal};

use super::super::{ReplayableSignalFixture, SourceRecordStore};
use super::{
    build_security_signal, object, optional_string, parse_exploitability, parse_finding_severity,
    payload_value, required_string, retain_source, revision_from_payload, security_state,
    validate_fixture_for_source, validate_source_identity, validate_source_text,
    validate_timestamp, SignalAdapter, SignalAdapterError,
};

/// Adapter for one deterministic OPA Gatekeeper violation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GatekeeperAdapter;

/// Compatibility alias for callers that name adapters by their Signal output.
pub type GatekeeperSignalAdapter = GatekeeperAdapter;

impl GatekeeperAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl SignalAdapter for GatekeeperAdapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        EvidenceSourceKind::OpaGatekeeper
    }

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError> {
        validate_fixture_for_source(EvidenceSourceKind::OpaGatekeeper, fixture)?;
        let payload = object(payload_value(fixture))?;
        let native_id = gatekeeper_native_identity(payload);
        let source_record =
            retain_source(fixture, records, native_id, revision_from_payload(fixture))?;

        let template = required_string(payload, "constraint_template")?;
        let constraint = required_string(payload, "constraint")?;
        validate_source_identity(&template)?;
        validate_source_identity(&constraint)?;
        let result = required_string(payload, "result")?;
        if !matches!(
            result.trim().to_ascii_lowercase().as_str(),
            "violation" | "fail" | "failed"
        ) {
            return Err(SignalAdapterError::UnsupportedSchema);
        }
        let resource = payload
            .get("resource")
            .ok_or(SignalAdapterError::AmbiguousTarget)?;
        let (namespace, kind, name) = super::kyverno::kubernetes_identity(resource)?;
        let violation_path = optional_string(payload, "violation_path")?
            .ok_or(SignalAdapterError::MalformedPayload)?;
        validate_source_text(&violation_path)?;
        let target = super::kyverno::kubernetes_target(&namespace, &kind, &name)?;
        let severity = parse_finding_severity(payload, "severity")?;
        let exploitability = parse_exploitability(payload)?;
        let observed_at = validate_timestamp(fixture.observed_at.as_deref())?;
        let state = security_state(payload)?;
        let stable_identity = format!(
            "template={template};constraint={constraint};namespace={namespace};kind={kind};name={name};path={violation_path}"
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
            Some(&stable_identity),
        )
    }
}

/// Normalize the committed OPA Gatekeeper replay fixture.
pub fn normalize_gatekeeper(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    GatekeeperAdapter.normalize(fixture, records)
}

/// A Gatekeeper constraint can produce many violations. Include the complete
/// resource and violation path so each retained source record has a distinct
/// native identity at one revision.
fn gatekeeper_native_identity(payload: &serde_json::Map<String, Value>) -> Option<String> {
    let template = payload
        .get("constraint_template")
        .and_then(Value::as_str)?
        .trim();
    let constraint = payload.get("constraint").and_then(Value::as_str)?.trim();
    let resource = payload.get("resource")?.as_object()?;
    let namespace = resource.get("namespace").and_then(Value::as_str)?.trim();
    let kind = resource.get("kind").and_then(Value::as_str)?.trim();
    let name = resource.get("name").and_then(Value::as_str)?.trim();
    let path = payload
        .get("violation_path")
        .and_then(Value::as_str)?
        .trim();
    if [template, constraint, namespace, kind, name, path]
        .iter()
        .any(|value| value.is_empty())
    {
        return None;
    }
    Some(format!(
        "template={template};constraint={constraint};namespace={namespace};kind={kind};name={name};path={path}"
    ))
}
