//! Replay adapter for the committed Trivy JSON result.

use serde_json::Value;
use thalassa_domain::{
    EvidenceSourceKind, FindingAssetKind, Signal, SignalTarget, SignalTargetKind,
};

use super::super::{ReplayableSignalFixture, SourceRecordStore};
use super::{
    build_security_signal, object, optional_string, parse_cvss, parse_exploitability,
    parse_finding_severity, payload_value, required_string, retain_source, revision_from_payload,
    security_state, validate_fixture_for_source, validate_source_identity, validate_source_text,
    validate_timestamp, SignalAdapter, SignalAdapterError,
};

/// Adapter for one deterministic Trivy scan result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrivyAdapter;

/// Compatibility alias for callers that name adapters by their Signal output.
pub type TrivySignalAdapter = TrivyAdapter;

impl TrivyAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl SignalAdapter for TrivyAdapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        EvidenceSourceKind::Trivy
    }

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError> {
        validate_fixture_for_source(EvidenceSourceKind::Trivy, fixture)?;
        let payload = object(payload_value(fixture))?;

        // Retain the complete masked record before parsing typed finding facts.
        // Identity extraction is intentionally tolerant here so a malformed
        // payload still has a safe source record available for diagnostics.
        let native_id =
            trivy_result(payload).and_then(|result| trivy_native_identity(payload, result));
        let source_record =
            retain_source(fixture, records, native_id, revision_from_payload(fixture))?;

        let result = trivy_result(payload).ok_or(SignalAdapterError::MalformedPayload)?;

        let vulnerability_id =
            required_string_alias(result, &["VulnerabilityID", "vulnerability_id"])?;
        let package = optional_string_alias(result, &["PkgName", "package"])?.unwrap_or_default();
        validate_source_identity(&vulnerability_id)?;
        if !package.is_empty() {
            validate_source_text(&package)?;
        }
        let path = optional_string_alias(result, &["PkgPath", "VulnerablePath", "path"])?;
        if let Some(path) = path.as_deref() {
            validate_source_text(path)?;
        }

        let image_identity = image_identity(payload, result)?;
        validate_source_identity(&image_identity)?;
        let target = image_target(payload, result, &image_identity)?;
        let severity = parse_finding_severity(result, "Severity")?;
        let exploitability = parse_exploitability(result)?;
        let cvss_score = parse_cvss(result)?;
        let observed_at = validate_timestamp(fixture.observed_at.as_deref())?;
        let state = security_state(payload)?;
        let stable_identity = format!(
            "vulnerability_id={vulnerability_id};package={package};path={};image={image_identity}",
            path.as_deref().unwrap_or("")
        );
        build_security_signal(
            fixture,
            records,
            source_record,
            target,
            FindingAssetKind::ContainerImage,
            None,
            artifact_digest(payload, result)?,
            severity,
            exploitability,
            cvss_score,
            observed_at,
            state,
            &stable_identity,
            (!package.is_empty()).then_some(stable_identity.as_str()),
        )
    }
}

/// Normalize the committed Trivy replay fixture.
pub fn normalize_trivy(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    TrivyAdapter.normalize(fixture, records)
}

fn required_string_alias(
    object: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Result<String, SignalAdapterError> {
    for key in keys {
        if object.contains_key(*key) {
            return required_string(object, key);
        }
    }
    Err(SignalAdapterError::MalformedPayload)
}

fn optional_string_alias(
    object: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Result<Option<String>, SignalAdapterError> {
    for key in keys {
        if object.contains_key(*key) {
            return optional_string(object, key);
        }
    }
    Ok(None)
}

fn image_identity(
    payload: &serde_json::Map<String, Value>,
    result: &serde_json::Map<String, Value>,
) -> Result<String, SignalAdapterError> {
    optional_string_alias(result, &["Target"])?
        .or(optional_string_alias(
            payload,
            &["ArtifactName", "artifact_name"],
        )?)
        .or_else(|| {
            result
                .get("target")
                .and_then(|value| value.as_object())
                .and_then(|target| target.get("id"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
        })
        .ok_or(SignalAdapterError::MalformedPayload)
}

fn image_target(
    payload: &serde_json::Map<String, Value>,
    result: &serde_json::Map<String, Value>,
    image_identity: &str,
) -> Result<SignalTarget, SignalAdapterError> {
    let explicit = result
        .get("resolved_target")
        .or_else(|| payload.get("resolved_target"))
        .or_else(|| {
            result
                .get("target")
                .filter(|value| value.as_object().is_some())
        })
        .or_else(|| {
            payload
                .get("target")
                .filter(|value| value.as_object().is_some())
        });
    if let Some(explicit) = explicit {
        let target: SignalTarget = serde_json::from_value(explicit.clone())
            .map_err(|_| SignalAdapterError::MalformedPayload)?;
        target.validate().map_err(SignalAdapterError::Signal)?;
        return Ok(target);
    }
    Ok(SignalTarget {
        kind: SignalTargetKind::Resource,
        id: image_identity.to_owned(),
    })
}

fn trivy_result(
    payload: &serde_json::Map<String, Value>,
) -> Option<&serde_json::Map<String, Value>> {
    if let Some(results) = payload.get("Results").and_then(Value::as_array) {
        return (results.len() == 1)
            .then(|| results[0].as_object())
            .flatten();
    }
    // A mixed correlation fixture may carry an already normalized Trivy
    // result directly.  It is still retained as the complete source object.
    payload
        .get("vulnerability_id")
        .or_else(|| payload.get("VulnerabilityID"))
        .map(|_| payload)
}

/// Return the complete stable identity when all source identity fields are
/// present.  A vulnerability ID alone is not unique in a scan: the same CVE
/// can occur in multiple packages, paths or images.  Missing or malformed
/// fields intentionally produce no native identity; strict typed parsing
/// below still retains the source row before rejecting the finding.
fn trivy_native_identity(
    payload: &serde_json::Map<String, Value>,
    result: &serde_json::Map<String, Value>,
) -> Option<String> {
    let vulnerability_id = result
        .get("VulnerabilityID")
        .or_else(|| result.get("vulnerability_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?;
    let package = result
        .get("PkgName")
        .or_else(|| result.get("package"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?;
    let path = result
        .get("PkgPath")
        .or_else(|| result.get("VulnerablePath"))
        .or_else(|| result.get("path"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let image = result
        .get("Target")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            payload
                .get("ArtifactName")
                .or_else(|| payload.get("artifact_name"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            result
                .get("target")
                .and_then(Value::as_object)
                .and_then(|target| target.get("id"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            payload
                .get("target")
                .and_then(Value::as_object)
                .and_then(|target| target.get("id"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })?;
    Some(format!(
        "vulnerability_id={vulnerability_id};package={package};path={path};image={image}"
    ))
}

fn artifact_digest(
    payload: &serde_json::Map<String, Value>,
    result: &serde_json::Map<String, Value>,
) -> Result<Option<String>, SignalAdapterError> {
    let digest = optional_string_alias(result, &["ArtifactDigest", "artifact_digest"])?.or(
        optional_string_alias(payload, &["ArtifactDigest", "artifact_digest"])?,
    );
    if let Some(digest) = digest.as_deref() {
        validate_source_identity(digest)?;
    }
    Ok(digest)
}
