//! Deterministic, source-qualified logical identity for normalized Signals.
//!
//! A deduplication key is an association index, not a retention decision.  A
//! `DedupIndex` therefore keeps every Signal ID that produced a key and only
//! coalesces the edge used by later grouping code.  The identity tuple is
//! intentionally private to this module: only its opaque SHA-256 digest is
//! suitable for the console boundary.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    AnomalyCondition, CorrelationError, EvidenceSourceKind, Signal, SignalId, SignalKind,
    SignalPayload, SignalTarget, SignalTargetKind, SourceRecordRef,
};
use thiserror::Error;

use super::source_records::{SourceRecord, SourceRecordStore};

/// Typed failures from source-aware identity construction.
///
/// Error text is fixed and never contains a source payload, native identity or
/// other provider value.  This makes the error safe to expose through a typed
/// IPC error envelope later in the correlation pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum DedupError {
    #[error("signal failed correlation validation")]
    Signal(#[source] CorrelationError),
    #[error("source record is missing for signal")]
    SourceRecordMissing,
    #[error("source record source does not match signal source")]
    SourceMismatch,
    #[error("source record payload cannot provide a logical identity")]
    InvalidPayload,
    #[error("source identity is unsafe for deduplication")]
    UnsafeIdentity,
    #[error("source identity is unavailable for deduplication")]
    MissingIdentity,
    #[error("native source identity conflicts with a retained revision")]
    ConflictingNativeIdentity,
    #[error("signal identifier is duplicated in the deduplication input")]
    DuplicateSignal,
}

/// The field-labelled identity tuple used to derive an opaque key.
///
/// This type intentionally does not implement `Serialize`; callers can use it
/// for backend diagnostics/tests without accidentally placing source identity
/// values on the React wire contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalIdentity {
    source: EvidenceSourceKind,
    kind: SignalKind,
    fields: BTreeMap<String, String>,
}

impl CanonicalIdentity {
    pub fn source(&self) -> EvidenceSourceKind {
        self.source
    }

    pub fn kind(&self) -> SignalKind {
        self.kind
    }

    /// Backend-only access to the canonical fields for deterministic tests.
    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }

    fn digest(&self) -> String {
        let mut hash = Sha256::new();
        // Length-prefix labels and values so delimiters in a safe source value
        // cannot create an ambiguous tuple. BTreeMap iteration is canonical.
        for (label, value) in &self.fields {
            hash.update((label.len() as u64).to_be_bytes());
            hash.update(label.as_bytes());
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        let digest = hash.finalize();
        format!("{digest:x}")
    }

    fn key(&self) -> String {
        format!(
            "dedup:v1:{}:{}:{}",
            source_wire(self.source),
            signal_kind_wire(self.kind),
            self.digest()
        )
    }
}

/// One opaque association edge and all retained source Signals behind it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DedupAssociation {
    pub key: String,
    pub signal_ids: Vec<SignalId>,
    pub source_records: Vec<SourceRecordRef>,
}

/// Deterministic index of source-aware keys.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DedupIndex {
    associations: BTreeMap<String, DedupAssociation>,
    signal_keys: BTreeMap<SignalId, Option<String>>,
}

impl DedupIndex {
    pub fn from_signals(
        signals: &[Signal],
        records: Option<&SourceRecordStore>,
    ) -> Result<Self, DedupError> {
        let mut signals = signals.to_vec();
        deduplicate_signals(&mut signals, records)
    }

    pub fn len(&self) -> usize {
        self.associations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.associations.is_empty()
    }

    pub fn total_signal_count(&self) -> usize {
        self.signal_keys.len()
    }

    pub fn associations(&self) -> impl Iterator<Item = &DedupAssociation> {
        self.associations.values()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.associations.keys().map(String::as_str)
    }

    pub fn signal_ids_for(&self, key: &str) -> Option<&[SignalId]> {
        self.associations
            .get(key)
            .map(|association| association.signal_ids.as_slice())
    }

    pub fn source_records_for(&self, key: &str) -> Option<&[SourceRecordRef]> {
        self.associations
            .get(key)
            .map(|association| association.source_records.as_slice())
    }

    pub fn key_for_signal(&self, signal_id: SignalId) -> Option<Option<&str>> {
        self.signal_keys.get(&signal_id).map(|key| key.as_deref())
    }

    pub fn dedup_key_for_signal(&self, signal_id: SignalId) -> Option<&str> {
        self.signal_keys
            .get(&signal_id)
            .and_then(|key| key.as_deref())
    }

    /// Pick the smallest opaque key (or Signal ID when no key exists) behind
    /// the index. This is the stable anchor used by candidate construction.
    pub fn stable_candidate_anchor(&self) -> Option<String> {
        let mut anchors = self.associations.keys().cloned().collect::<Vec<_>>();
        anchors.extend(
            self.signal_keys
                .iter()
                .filter(|(_, key)| key.is_none())
                .map(|(id, _)| id.to_string()),
        );
        anchors.into_iter().min()
    }
}

/// Compute a source-aware opaque key for one Signal.
///
/// `records` should be the local source-record ledger when available. Passing
/// `None` is useful for already-normalized Signals whose adapter has supplied a
/// key. When the ledger is available, the retained post-policy payload is the
/// source of truth for the complete tuple.
pub fn compute_dedup_key(
    signal: &Signal,
    records: Option<&SourceRecordStore>,
) -> Result<Option<String>, DedupError> {
    validate_signal_source(signal)?;
    if records.is_none() {
        if let Some(key) = signal.dedup_key.as_deref() {
            if is_opaque_key(key) {
                return Ok(Some(key.to_owned()));
            }
        }
    }
    let record = records
        .map(|records| {
            records
                .get(&signal.source_record)
                .ok_or(DedupError::SourceRecordMissing)
        })
        .transpose()?;
    let identity = canonical_identity(signal, record)?;
    Ok(identity
        .map(|identity| identity.key())
        // Adapter-normalized Signals already carry an opaque key. Preserve it
        // when the source ledger is intentionally unavailable, while keeping
        // the strict canonical path for callers that provide retained data.
        .or_else(|| {
            records
                .is_none()
                .then(|| signal.dedup_key.clone())
                .flatten()
        }))
}

/// Return the private canonical identity tuple for backend tests and the
/// grouping pipeline. The tuple itself is never serialized.
pub fn canonical_identity(
    signal: &Signal,
    record: Option<&SourceRecord>,
) -> Result<Option<CanonicalIdentity>, DedupError> {
    validate_signal_source(signal)?;
    if let Some(record) = record {
        if record.source_kind != signal.source_record.source_kind
            || record.source_kind != signal.source
        {
            return Err(DedupError::SourceMismatch);
        }
    }

    let payload = record.and_then(|record| record.payload().as_object());
    let mut fields = BTreeMap::new();
    let source = signal.source;
    let kind = signal.kind;

    match (source, kind) {
        (EvidenceSourceKind::Alertmanager, SignalKind::Alert) => {
            if let Some(fingerprint) =
                payload_string(payload, &["fingerprint", "alert_fingerprint"])?
                    .or_else(|| signal.source_record.native_id.clone())
            {
                insert_safe(&mut fields, "fingerprint", fingerprint)?;
            } else if let Some(targets) = exact_targets(signal) {
                insert_safe(
                    &mut fields,
                    "source_digest",
                    signal.source_record.content_digest.clone(),
                )?;
                insert_targets(&mut fields, targets)?;
            }
        }
        (EvidenceSourceKind::Prometheus, SignalKind::Anomaly) => {
            let rule_id = payload_string(payload, &["rule_id", "rule"])?
                .or_else(|| signal.source_record.native_id.clone());
            let metric_key = payload_string(payload, &["metric_key", "metric"])?;
            let condition = payload
                .and_then(|object| object.get("condition"))
                .map(canonical_value)
                .or_else(|| match &signal.payload {
                    SignalPayload::Anomaly { condition, .. } => {
                        Some(canonical_condition(condition))
                    }
                    _ => None,
                });
            let Some(targets) = exact_targets(signal) else {
                return Ok(None);
            };
            let (Some(rule_id), Some(metric_key), Some(condition)) =
                (rule_id, metric_key, condition)
            else {
                return Ok(None);
            };
            insert_safe(&mut fields, "rule_id", rule_id)?;
            insert_safe(&mut fields, "metric_key", metric_key)?;
            insert_safe(&mut fields, "condition", condition)?;
            insert_targets(&mut fields, targets)?;
        }
        (EvidenceSourceKind::Trivy, SignalKind::SecurityFinding) => {
            let result = trivy_result(payload);
            let vulnerability_id =
                payload_string(result, &["VulnerabilityID", "vulnerability_id"])?
                    .or_else(|| signal.source_record.native_id.clone());
            let package = payload_string(result, &["PkgName", "package"])?;
            let path =
                payload_string(result, &["PkgPath", "VulnerablePath", "path"])?.unwrap_or_default();
            let image = payload_string(result, &["Target"])?
                .or(payload_string(payload, &["ArtifactName", "artifact_name"])?);
            let image = image
                .or_else(|| nested_target_id(result))
                .or_else(|| nested_target_id(payload))
                .or_else(|| first_target_id(signal));
            let (Some(vulnerability_id), Some(package), Some(image)) =
                (vulnerability_id, package, image)
            else {
                return Ok(None);
            };
            insert_safe(&mut fields, "vulnerability_id", vulnerability_id)?;
            insert_safe(&mut fields, "package", package)?;
            insert_optional(&mut fields, "path", path)?;
            insert_safe(&mut fields, "image", image)?;
        }
        (EvidenceSourceKind::Falco, SignalKind::SecurityFinding) => {
            let rule = payload_string(payload, &["rule", "rule_id"])?;
            let event_id = payload_string(payload, &["event_id", "event_fingerprint"])?
                .or_else(|| signal.source_record.native_id.clone());
            let Some(targets) = exact_targets(signal) else {
                return Ok(None);
            };
            let (Some(rule), Some(event_id)) = (rule, event_id) else {
                return Ok(None);
            };
            insert_safe(&mut fields, "rule", rule)?;
            insert_safe(&mut fields, "event_fingerprint", event_id)?;
            insert_targets(&mut fields, targets)?;
            if let Some(target) = payload
                .and_then(|object| object.get("target"))
                .and_then(Value::as_object)
            {
                if let Some(namespace) = target.get("namespace").and_then(Value::as_str) {
                    insert_safe(&mut fields, "target_namespace", namespace.to_owned())?;
                }
                if let Some(container) = target.get("container").and_then(Value::as_str) {
                    insert_safe(&mut fields, "target_container", container.to_owned())?;
                }
            }
        }
        (EvidenceSourceKind::Kyverno, SignalKind::SecurityFinding) => {
            let policy = payload_string(payload, &["policy", "policy_id"])?;
            let rule = payload_string(payload, &["rule", "rule_id"])?;
            let path = payload_string(payload, &["violation_path", "path"])?;
            let Some((namespace, resource_kind, name)) = kubernetes_identity(payload) else {
                return Ok(None);
            };
            let (Some(policy), Some(rule), Some(path)) = (policy, rule, path) else {
                return Ok(None);
            };
            insert_safe(&mut fields, "policy", policy)?;
            insert_safe(&mut fields, "rule", rule)?;
            insert_safe(&mut fields, "namespace", namespace)?;
            insert_safe(&mut fields, "resource_kind", resource_kind)?;
            insert_safe(&mut fields, "resource_name", name)?;
            insert_safe(&mut fields, "violation_path", path)?;
        }
        (EvidenceSourceKind::OpaGatekeeper, SignalKind::SecurityFinding) => {
            let template = payload_string(payload, &["constraint_template", "template"])?;
            let constraint = payload_string(payload, &["constraint", "constraint_id"])?;
            let path = payload_string(payload, &["violation_path", "path"])?;
            let Some((namespace, resource_kind, name)) = kubernetes_identity(payload) else {
                return Ok(None);
            };
            let (Some(template), Some(constraint), Some(path)) = (template, constraint, path)
            else {
                return Ok(None);
            };
            insert_safe(&mut fields, "template", template)?;
            insert_safe(&mut fields, "constraint", constraint)?;
            insert_safe(&mut fields, "namespace", namespace)?;
            insert_safe(&mut fields, "resource_kind", resource_kind)?;
            insert_safe(&mut fields, "resource_name", name)?;
            insert_safe(&mut fields, "violation_path", path)?;
        }
        (EvidenceSourceKind::HealthCheck, SignalKind::HealthCheck) => {
            let schedule = payload_string(payload, &["schedule_id", "schedule"])?
                .or_else(|| signal.source_record.native_id.clone());
            let probe = payload_string(payload, &["probe_key", "probe", "resource_key"])?
                .or_else(|| first_target_id(signal))
                .or_else(|| schedule.clone());
            let (Some(schedule), Some(probe)) = (schedule, probe) else {
                return Ok(None);
            };
            insert_safe(&mut fields, "schedule_id", schedule)?;
            insert_safe(&mut fields, "probe_key", probe)?;
        }
        _ => {
            // The initial source adapters above own the complete tuples. For
            // other provider-neutral Signals, use a native identity only when
            // it is explicit and pair it with an exact target where present.
            let Some(native_id) = signal.source_record.native_id.clone() else {
                return Ok(None);
            };
            insert_safe(&mut fields, "native_id", native_id)?;
            if let Some(targets) = exact_targets(signal) {
                insert_targets(&mut fields, targets)?;
            }
        }
    }

    if fields.is_empty() {
        return Ok(None);
    }
    Ok(Some(CanonicalIdentity {
        source,
        kind,
        fields,
    }))
}

/// Compute keys, sort Signals deterministically and build the association
/// index. No Signal or source record is removed by this operation.
pub fn deduplicate_signals(
    signals: &mut [Signal],
    records: Option<&SourceRecordStore>,
) -> Result<DedupIndex, DedupError> {
    let mut seen_ids = BTreeSet::new();
    let mut native_identity =
        BTreeMap::<(EvidenceSourceKind, String, Option<String>), String>::new();

    for signal in signals.iter_mut() {
        if !seen_ids.insert(signal.id) {
            return Err(DedupError::DuplicateSignal);
        }
        signal.validate().map_err(DedupError::Signal)?;
        let key = compute_dedup_key(signal, records)?;
        if let Some(native_id) = signal.source_record.native_id.clone() {
            let native_key = (
                signal.source,
                native_id,
                signal.source_record.revision.clone(),
            );
            if let Some(existing_digest) = native_identity.get(&native_key) {
                if existing_digest != &signal.source_record.content_digest {
                    return Err(DedupError::ConflictingNativeIdentity);
                }
            } else {
                native_identity.insert(native_key, signal.source_record.content_digest.clone());
            }
        }
        signal.dedup_key = key;
    }

    signals.sort_by(signal_ordering);
    let mut index = DedupIndex::default();
    for signal in signals.iter() {
        index
            .signal_keys
            .insert(signal.id, signal.dedup_key.clone());
        let Some(key) = signal.dedup_key.as_ref() else {
            continue;
        };
        let association =
            index
                .associations
                .entry(key.clone())
                .or_insert_with(|| DedupAssociation {
                    key: key.clone(),
                    signal_ids: Vec::new(),
                    source_records: Vec::new(),
                });
        // A stable logical key may intentionally span multiple source
        // revisions or execution IDs (for example, repeated health-check
        // runs).  The native identity/revision index above rejects an actual
        // conflicting identity; an association key must still retain every
        // source reference rather than treating distinct revisions as a
        // duplicate or conflict.
        association.signal_ids.push(signal.id);
        if !association.source_records.contains(&signal.source_record) {
            association
                .source_records
                .push(signal.source_record.clone());
        }
    }
    for association in index.associations.values_mut() {
        association.signal_ids.sort();
        association.source_records.sort_by(|left, right| {
            (
                left.source_kind,
                left.native_id.as_deref().unwrap_or_default(),
                left.revision.as_deref().unwrap_or_default(),
                left.content_digest.as_str(),
            )
                .cmp(&(
                    right.source_kind,
                    right.native_id.as_deref().unwrap_or_default(),
                    right.revision.as_deref().unwrap_or_default(),
                    right.content_digest.as_str(),
                ))
        });
    }
    Ok(index)
}

/// Compatibility name for callers that describe this phase as index
/// construction rather than mutation of the Signal key field.
pub fn build_dedup_index(
    signals: &mut [Signal],
    records: Option<&SourceRecordStore>,
) -> Result<DedupIndex, DedupError> {
    deduplicate_signals(signals, records)
}

/// Build an index from an immutable Signal slice while retaining the caller's
/// ordering and key fields unchanged.
pub fn index_signals(
    signals: &[Signal],
    records: Option<&SourceRecordStore>,
) -> Result<DedupIndex, DedupError> {
    DedupIndex::from_signals(signals, records)
}

/// Explicitly named alias used by aggregation code that wants to emphasize
/// the source-qualified nature of the key.
pub fn source_aware_dedup_key(
    signal: &Signal,
    records: Option<&SourceRecordStore>,
) -> Result<Option<String>, DedupError> {
    compute_dedup_key(signal, records)
}

/// Return a stable candidate anchor from the smallest key, falling back to a
/// Signal UUID when no safe identity exists.
pub fn stable_candidate_anchor(signals: &[Signal]) -> Option<String> {
    signals
        .iter()
        .map(|signal| {
            signal
                .dedup_key
                .clone()
                .unwrap_or_else(|| signal.id.to_string())
        })
        .min()
}

fn validate_signal_source(signal: &Signal) -> Result<(), DedupError> {
    if signal.source_record.source_kind != signal.source {
        return Err(DedupError::SourceMismatch);
    }
    Ok(())
}

fn signal_ordering(left: &Signal, right: &Signal) -> Ordering {
    observed_sort_key(left)
        .cmp(&observed_sort_key(right))
        .then_with(|| source_wire(left.source).cmp(source_wire(right.source)))
        .then_with(|| {
            left.source_record
                .content_digest
                .cmp(&right.source_record.content_digest)
        })
        .then_with(|| left.id.cmp(&right.id))
}

fn observed_sort_key(signal: &Signal) -> (bool, i64, u32, String) {
    let Some(value) = signal.observed_at.as_deref() else {
        return (true, 0, 0, String::new());
    };
    match chrono::DateTime::parse_from_rfc3339(value) {
        Ok(timestamp) => (
            false,
            timestamp.timestamp(),
            timestamp.timestamp_subsec_nanos(),
            value.to_owned(),
        ),
        Err(_) => (false, i64::MAX, u32::MAX, value.to_owned()),
    }
}

fn exact_targets(signal: &Signal) -> Option<Vec<&SignalTarget>> {
    let mut targets = signal
        .targets
        .iter()
        .filter(|target| {
            matches!(
                target.kind,
                SignalTargetKind::Resource
                    | SignalTargetKind::Service
                    | SignalTargetKind::Deployment
            )
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return None;
    }
    targets
        .sort_by(|left, right| (left.kind, left.id.as_str()).cmp(&(right.kind, right.id.as_str())));
    Some(targets)
}

fn first_target_id(signal: &Signal) -> Option<String> {
    exact_targets(signal).and_then(|targets| targets.first().map(|target| target.id.clone()))
}

fn insert_targets(
    fields: &mut BTreeMap<String, String>,
    targets: Vec<&SignalTarget>,
) -> Result<(), DedupError> {
    for (index, target) in targets.into_iter().enumerate() {
        insert_safe(
            fields,
            &format!("target_{index}_kind"),
            target_kind_wire(target.kind).to_owned(),
        )?;
        insert_safe(fields, &format!("target_{index}_id"), target.id.clone())?;
    }
    Ok(())
}

fn insert_safe(
    fields: &mut BTreeMap<String, String>,
    label: &str,
    value: String,
) -> Result<(), DedupError> {
    if value.trim().is_empty()
        || value.chars().any(char::is_control)
        || contains_forbidden_marker(&value)
    {
        return Err(DedupError::UnsafeIdentity);
    }
    fields.insert(label.to_owned(), value);
    Ok(())
}

fn insert_optional(
    fields: &mut BTreeMap<String, String>,
    label: &str,
    value: String,
) -> Result<(), DedupError> {
    if value.chars().any(char::is_control) || contains_forbidden_marker(&value) {
        return Err(DedupError::UnsafeIdentity);
    }
    fields.insert(label.to_owned(), value);
    Ok(())
}

fn payload_string(
    object: Option<&Map<String, Value>>,
    aliases: &[&str],
) -> Result<Option<String>, DedupError> {
    let Some(object) = object else {
        return Ok(None);
    };
    for alias in aliases {
        let Some(value) = object.get(*alias) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(value) = value.as_str() else {
            return Err(DedupError::InvalidPayload);
        };
        if value.trim().is_empty() {
            return Ok(None);
        }
        return Ok(Some(value.to_owned()));
    }
    Ok(None)
}

fn trivy_result(object: Option<&Map<String, Value>>) -> Option<&Map<String, Value>> {
    let object = object?;
    if let Some(results) = object.get("Results").and_then(Value::as_array) {
        if results.len() == 1 {
            return results.first().and_then(Value::as_object);
        }
        return None;
    }
    if object.contains_key("vulnerability_id") || object.contains_key("VulnerabilityID") {
        return Some(object);
    }
    None
}

fn kubernetes_identity(object: Option<&Map<String, Value>>) -> Option<(String, String, String)> {
    let resource = object?.get("resource")?.as_object()?;
    Some((
        resource.get("namespace")?.as_str()?.to_owned(),
        resource.get("kind")?.as_str()?.to_owned(),
        resource.get("name")?.as_str()?.to_owned(),
    ))
}

fn nested_target_id(object: Option<&Map<String, Value>>) -> Option<String> {
    object?
        .get("target")?
        .as_object()?
        .get("id")?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn canonical_condition(condition: &AnomalyCondition) -> String {
    serde_json::to_string(condition).expect("AnomalyCondition is serializable")
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut output = String::from("{");
            for key in keys {
                output.push_str(key);
                output.push(':');
                output.push_str(&canonical_value(&object[key]));
                output.push(';');
            }
            output.push('}');
            output
        }
        Value::Array(values) => {
            let mut output = String::from("[");
            for value in values {
                output.push_str(&canonical_value(value));
                output.push(';');
            }
            output.push(']');
            output
        }
        _ => value.to_string(),
    }
}

fn contains_forbidden_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
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

fn is_opaque_key(value: &str) -> bool {
    let mut parts = value.split(':');
    matches!(parts.next(), Some("dedup"))
        && matches!(parts.next(), Some("v1"))
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|digest| {
            digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit())
        })
        && parts.next().is_none()
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

fn target_kind_wire(kind: SignalTargetKind) -> &'static str {
    match kind {
        SignalTargetKind::Resource => "resource",
        SignalTargetKind::Service => "service",
        SignalTargetKind::Deployment => "deployment",
        SignalTargetKind::Topology => "topology",
    }
}
