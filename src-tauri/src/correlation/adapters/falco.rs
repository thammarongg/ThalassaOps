//! Replay adapter for the committed Falco runtime-event fixture.

use serde_json::Value;
use thalassa_domain::{
    EvidenceSourceKind, FindingAssetKind, FindingSeverity, Signal, SignalTarget, SignalTargetKind,
};

use super::super::{ReplayableSignalFixture, SourceRecordStore};
use super::{
    build_security_signal, object, optional_string, parse_exploitability, parse_severity_text,
    payload_value, required_string, retain_source, revision_from_payload, security_state,
    validate_fixture_for_source, validate_source_identity, validate_source_text,
    validate_timestamp, SignalAdapter, SignalAdapterError,
};

/// Adapter for one deterministic Falco runtime event.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FalcoAdapter;

/// Compatibility alias for callers that name adapters by their Signal output.
pub type FalcoSignalAdapter = FalcoAdapter;

impl FalcoAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl SignalAdapter for FalcoAdapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        EvidenceSourceKind::Falco
    }

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError> {
        validate_fixture_for_source(EvidenceSourceKind::Falco, fixture)?;
        let payload = object(payload_value(fixture))?;

        // Retain before parsing source-specific fields.  Missing event IDs are
        // represented by an absent source identity until the typed parse below
        // reports the malformed payload.
        let native_id = payload
            .get("event_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned);
        let source_record =
            retain_source(fixture, records, native_id, revision_from_payload(fixture))?;

        let event_id = required_string(payload, "event_id")?;
        let rule = required_string(payload, "rule")?;
        validate_source_identity(&event_id)?;
        validate_source_text(&rule)?;

        let target = runtime_target(payload.get("target"))?;
        let namespace = target_namespace(payload.get("target"))?;
        let container = target_container(payload.get("target"))?;
        let severity = optional_string(payload, "priority")?
            .map(|priority| parse_falco_priority(&priority))
            .transpose()?
            .flatten();
        let exploitability = parse_exploitability(payload)?;
        let event_time = optional_string(payload, "time")?;
        let observed_at =
            validate_timestamp(event_time.as_deref().or(fixture.observed_at.as_deref()))?;
        let state = security_state(payload)?;
        let stable_identity = format!(
            "rule={rule};namespace={namespace};target={};container={container};event_id={event_id}",
            target.id
        );

        build_security_signal(
            fixture,
            records,
            source_record,
            target,
            FindingAssetKind::RuntimeResource,
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

/// Normalize the committed Falco replay fixture.
pub fn normalize_falco(
    fixture: &ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, SignalAdapterError> {
    FalcoAdapter.normalize(fixture, records)
}

fn parse_falco_priority(value: &str) -> Result<Option<FindingSeverity>, SignalAdapterError> {
    // Falco's explicit priority vocabulary is intentionally kept as a fixed
    // table.  The shared parser maps only these source meanings and rejects a
    // new/unknown value rather than guessing a severity.
    parse_severity_text(value)
}

fn runtime_target(value: Option<&Value>) -> Result<SignalTarget, SignalAdapterError> {
    let Some(value) = value else {
        return Err(SignalAdapterError::AmbiguousTarget);
    };
    if let Some(object) = value.as_object() {
        if let (Some(kind), Some(id)) = (object.get("kind"), object.get("id")) {
            let target: SignalTarget = serde_json::from_value(Value::Object(
                [
                    ("kind".to_owned(), kind.clone()),
                    ("id".to_owned(), id.clone()),
                ]
                .into_iter()
                .collect(),
            ))
            .map_err(|_| SignalAdapterError::AmbiguousTarget)?;
            target.validate().map_err(SignalAdapterError::Signal)?;
            return Ok(target);
        }
        let namespace = object
            .get("namespace")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let pod = object
            .get("pod")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let workload = object
            .get("workload")
            .or_else(|| object.get("deployment"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let host = object
            .get("host")
            .or_else(|| object.get("node"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let (target_prefix, target_name) = if let Some(pod) = pod {
            ("pod", pod)
        } else if let Some(workload) = workload {
            ("workload", workload)
        } else if let Some(host) = host {
            ("host", host)
        } else {
            return Err(SignalAdapterError::AmbiguousTarget);
        };
        if let Some(namespace) = namespace {
            validate_source_identity(namespace)?;
        } else if target_prefix != "host" {
            return Err(SignalAdapterError::AmbiguousTarget);
        }
        validate_source_identity(target_name)?;
        let target = SignalTarget {
            kind: SignalTargetKind::Resource,
            // Namespace and container remain in the opaque identity tuple and
            // source record; the canonical target follows the existing
            // resource `kind/name` convention used by the console fixtures.
            id: format!("{target_prefix}/{target_name}"),
        };
        target.validate().map_err(SignalAdapterError::Signal)?;
        return Ok(target);
    }
    Err(SignalAdapterError::AmbiguousTarget)
}

fn target_namespace(value: Option<&Value>) -> Result<String, SignalAdapterError> {
    if value
        .and_then(Value::as_object)
        .is_some_and(|object| object.contains_key("kind") && object.contains_key("id"))
    {
        return Ok(String::new());
    }
    if value
        .and_then(Value::as_object)
        .is_some_and(|object| object.contains_key("host") || object.contains_key("node"))
    {
        return Ok(String::new());
    }
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("namespace"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            validate_source_identity(value)?;
            Ok(value.to_owned())
        })
        .unwrap_or_else(|| Err(SignalAdapterError::AmbiguousTarget))
}

fn target_container(value: Option<&Value>) -> Result<String, SignalAdapterError> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Err(SignalAdapterError::AmbiguousTarget);
    };
    let Some(value) = object.get("container") else {
        return Ok(String::new());
    };
    let container = value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or(SignalAdapterError::AmbiguousTarget)?;
    validate_source_identity(container)?;
    Ok(container.to_owned())
}
