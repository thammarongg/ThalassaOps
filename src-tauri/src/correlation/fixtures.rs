//! Deterministic, local-only replay inputs for Sprint 13 signal correlation.
//!
//! These values are deliberately provider-neutral at the contract boundary.
//! The source payloads remain complete JSON values so adapters can retain
//! unknown fields while indexing only the facts they understand.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thalassa_domain::{
    CorrelationError, EvidenceRedaction, EvidenceRef, EvidenceSourceKind, MaintenanceWindow,
    MaintenanceWindowReason, ResourceScope, SignalKind, SignalTarget, SignalTargetKind,
    SuppressionRule, TimeWindow,
};
use uuid::Uuid;

const FIXTURE_TIME_SECONDS: i64 = 1_787_907_600;
/// Shared deterministic clock for all Sprint 13 replay values.
pub const FIXTURE_CLOCK: &str = "2026-08-28T09:00:00Z";

const TRIVY_RECORD: &str =
    include_str!("../../../docs/superpowers/fixtures/2026-08-28-capture/security/trivy.json");
const FALCO_RECORD: &str =
    include_str!("../../../docs/superpowers/fixtures/2026-08-28-capture/security/falco.json");
const KYVERNO_RECORD: &str =
    include_str!("../../../docs/superpowers/fixtures/2026-08-28-capture/security/kyverno.json");
const GATEKEEPER_RECORD: &str =
    include_str!("../../../docs/superpowers/fixtures/2026-08-28-capture/security/gatekeeper.json");

/// A source record admitted for deterministic replay.
///
/// `recorded_json` is the complete post-policy record, not a normalized copy.
/// Adapters may use the optional times and evidence as indexes, but they must
/// retain every field in this value in the source-record ledger.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReplayableSignalFixture {
    pub key: String,
    pub source_kind: EvidenceSourceKind,
    pub scope: ResourceScope,
    pub recorded_json: Value,
    pub observed_at: Option<String>,
    pub ingested_at: Option<String>,
    pub evidence: Vec<EvidenceRef>,
}

impl ReplayableSignalFixture {
    /// Validate fixture shape, timestamps, scope and admitted evidence.
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_fixture_text(&self.key)?;
        if !self.recorded_json.is_object() && !self.recorded_json.is_array() {
            return Err(CorrelationError::InvalidPayload);
        }
        if let Some(observed_at) = self.observed_at.as_deref() {
            DateTime::parse_from_rfc3339(observed_at)
                .map_err(|_| CorrelationError::InvalidTimestamp)?;
        }
        if let Some(ingested_at) = self.ingested_at.as_deref() {
            DateTime::parse_from_rfc3339(ingested_at)
                .map_err(|_| CorrelationError::InvalidTimestamp)?;
        }
        if self.evidence.is_empty() {
            return Err(CorrelationError::EvidenceMissing);
        }
        let mut evidence_ids = BTreeSet::new();
        for evidence in &self.evidence {
            validate_fixture_text(&evidence.id)?;
            if !evidence_ids.insert(evidence.id.as_str()) {
                return Err(CorrelationError::DuplicateId);
            }
            if evidence.source_kind != self.source_kind {
                return Err(CorrelationError::SourceMismatch);
            }
            if !self.scope.contains(&evidence.scope) {
                return Err(CorrelationError::ScopeMismatch);
            }
            if !evidence.redaction.classification_verified
                || !evidence.redaction.redaction_verified
                || (evidence.redaction.unparsed && evidence.redaction.masked)
            {
                return Err(CorrelationError::InvalidEvidence);
            }
        }
        if contains_forbidden_fixture_data(self)? {
            return Err(CorrelationError::InvalidPayload);
        }
        Ok(())
    }

    /// Alias used by adapter admission code.
    pub fn validate_for_replay(&self) -> Result<(), CorrelationError> {
        self.validate()
    }

    /// Return the fixture key under the descriptive name used by adapters.
    pub fn fixture_key(&self) -> &str {
        &self.key
    }
}

/// Complete internal catalog used by the correlation adapter and projection.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CorrelationFixtureCatalog {
    pub fixtures: Vec<ReplayableSignalFixture>,
    pub suppression_rules: Vec<SuppressionRule>,
    pub maintenance_windows: Vec<MaintenanceWindow>,
}

impl CorrelationFixtureCatalog {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        let mut keys = BTreeSet::new();
        for fixture in &self.fixtures {
            fixture.validate()?;
            if !keys.insert(fixture.key.as_str()) {
                return Err(CorrelationError::DuplicateId);
            }
        }
        for rule in &self.suppression_rules {
            rule.validate()?;
        }
        for window in &self.maintenance_windows {
            window.validate()?;
        }
        Ok(())
    }

    pub fn security_fixtures(&self) -> impl Iterator<Item = &ReplayableSignalFixture> {
        self.fixtures.iter().filter(|fixture| {
            fixture.source_kind.is_security_source() && fixture.key.starts_with("security-")
        })
    }
}

/// Return the fixed Sprint 13 fixture clock.
pub fn fixture_time() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(FIXTURE_TIME_SECONDS, 0)
        .expect("the committed Sprint 13 fixture clock is a valid timestamp")
}

/// Return the deterministic source and policy fixture catalog.
pub fn correlation_fixture_catalog() -> CorrelationFixtureCatalog {
    let scope = fixture_scope();
    let mut fixtures = vec![
        security_fixture(
            "security-trivy",
            EvidenceSourceKind::Trivy,
            TRIVY_RECORD,
            "2026-08-28T08:57:00Z",
            "evidence-security-trivy",
            "fixture://security/trivy",
            &scope,
        ),
        security_fixture(
            "security-falco",
            EvidenceSourceKind::Falco,
            FALCO_RECORD,
            "2026-08-28T08:58:30Z",
            "evidence-security-falco",
            "fixture://security/falco",
            &scope,
        ),
        security_fixture(
            "security-kyverno",
            EvidenceSourceKind::Kyverno,
            KYVERNO_RECORD,
            "2026-08-28T08:59:00Z",
            "evidence-security-kyverno",
            "fixture://security/kyverno",
            &scope,
        ),
        security_fixture(
            "security-gatekeeper",
            EvidenceSourceKind::OpaGatekeeper,
            GATEKEEPER_RECORD,
            "2026-08-28T08:59:15Z",
            "evidence-security-gatekeeper",
            "fixture://security/gatekeeper",
            &scope,
        ),
        operational_fixture(
            "alert-checkout",
            EvidenceSourceKind::Alertmanager,
            json!({
                "fingerprint": "alert-checkout-s1",
                "state": "firing",
                "starts_at": "2026-08-28T08:55:00Z",
                "labels": {"service": "checkout", "severity": "S1"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:55:00Z",
            "evidence-alert-checkout",
            "fixture://operational/alertmanager",
            &scope,
        ),
        operational_fixture(
            "anomaly-checkout-errors",
            EvidenceSourceKind::Prometheus,
            json!({
                "rule_id": "rule-checkout-errors",
                "metric_key": "checkout_error_rate",
                "observed_value": 0.08,
                "comparison_value": 0.05,
                "condition": {"threshold": {"operator": "gte", "threshold": "0.05"}},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:59:00Z",
            "evidence-anomaly-checkout",
            "fixture://operational/prometheus",
            &scope,
        ),
        operational_fixture(
            "health-check-checkout",
            EvidenceSourceKind::HealthCheck,
            json!({
                "schedule_id": "check-checkout",
                "run_id": "run-checkout-1",
                "outcome": "degraded",
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:58:00Z",
            "evidence-health-checkout",
            "fixture://operational/health-check",
            &scope,
        ),
        operational_fixture(
            "health-check-skipped",
            EvidenceSourceKind::HealthCheck,
            json!({
                "schedule_id": "check-checkout",
                "run_id": "run-checkout-2",
                "outcome": "skipped_cooldown",
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:59:30Z",
            "evidence-health-skipped",
            "fixture://operational/health-check",
            &scope,
        ),
        operational_fixture(
            "late-anomaly-checkout",
            EvidenceSourceKind::Prometheus,
            json!({
                "rule_id": "rule-checkout-errors",
                "metric_key": "checkout_error_rate",
                "observed_value": 0.07,
                "comparison_value": 0.05,
                "condition": {"threshold": {"operator": "gte", "threshold": "0.05"}},
                "vendor_extension": {"capture": "synthetic", "late": true}
            }),
            "2026-08-28T08:56:00Z",
            "evidence-anomaly-late",
            "fixture://operational/prometheus",
            &scope,
        ),
        operational_fixture(
            "shared-service-alert",
            EvidenceSourceKind::Alertmanager,
            json!({
                "fingerprint": "alert-checkout-service",
                "state": "firing",
                "target": {"kind": "service", "id": "service/checkout"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:56:00Z",
            "evidence-shared-service-alert",
            "fixture://grouping/service",
            &scope,
        ),
        operational_fixture(
            "shared-service-anomaly",
            EvidenceSourceKind::Prometheus,
            json!({
                "rule_id": "rule-checkout-service",
                "metric_key": "checkout_requests",
                "target": {"kind": "service", "id": "service/checkout"},
                "observed_value": 120.0,
                "comparison_value": 100.0,
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:57:00Z",
            "evidence-shared-service-anomaly",
            "fixture://grouping/service",
            &scope,
        ),
        operational_fixture(
            "shared-deployment-alert",
            EvidenceSourceKind::Alertmanager,
            json!({
                "fingerprint": "alert-checkout-deployment",
                "state": "firing",
                "target": {"kind": "deployment", "id": "deployment/checkout"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:57:30Z",
            "evidence-shared-deployment-alert",
            "fixture://grouping/deployment",
            &scope,
        ),
        operational_fixture(
            "shared-deployment-finding",
            EvidenceSourceKind::Trivy,
            json!({
                "vulnerability_id": "CVE-2024-1234",
                "target": {"kind": "deployment", "id": "deployment/checkout"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:58:00Z",
            "evidence-shared-deployment-finding",
            "fixture://grouping/deployment",
            &scope,
        ),
        operational_fixture(
            "topology-left",
            EvidenceSourceKind::Kubernetes,
            json!({
                "target": {"kind": "topology", "id": "node-checkout"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:58:15Z",
            "evidence-topology-left",
            "fixture://grouping/topology",
            &scope,
        ),
        operational_fixture(
            "topology-right",
            EvidenceSourceKind::Kubernetes,
            json!({
                "target": {"kind": "topology", "id": "node-checkout-service"},
                "vendor_extension": {"capture": "synthetic"}
            }),
            "2026-08-28T08:58:45Z",
            "evidence-topology-right",
            "fixture://grouping/topology",
            &scope,
        ),
    ];
    fixtures.sort_by(|left, right| left.key.cmp(&right.key));

    let catalog = CorrelationFixtureCatalog {
        fixtures,
        suppression_rules: vec![SuppressionRule {
            id: "rule-suppress-checkout-test".into(),
            enabled: true,
            scope: scope.clone(),
            source: Some(EvidenceSourceKind::Prometheus),
            signal_kind: Some(SignalKind::Anomaly),
            target: Some(SignalTarget {
                kind: SignalTargetKind::Service,
                id: "service/checkout".into(),
            }),
        }],
        maintenance_windows: vec![MaintenanceWindow {
            id: "maintenance-checkout-release".into(),
            enabled: true,
            scope,
            target: Some(SignalTarget {
                kind: SignalTargetKind::Deployment,
                id: "deployment/checkout".into(),
            }),
            window: TimeWindow {
                start: "2026-08-28T08:55:00Z".into(),
                end: "2026-08-28T09:05:00Z".into(),
            },
            reason: MaintenanceWindowReason::PlannedChange,
            policy_version: 13,
        }],
    };
    catalog
        .validate()
        .expect("committed Sprint 13 fixtures must remain safe and deterministic");
    catalog
}

/// Stable workspace scope shared by every committed Sprint 13 fixture.
pub fn fixture_scope() -> ResourceScope {
    ResourceScope::environment(
        Uuid::from_u128(0x00000000000000000000000000000011),
        Uuid::from_u128(0x00000000000000000000000000000012),
        Uuid::from_u128(0x00000000000000000000000000000013),
        Uuid::from_u128(0x00000000000000000000000000000014),
    )
}

fn security_fixture(
    key: &str,
    source_kind: EvidenceSourceKind,
    record: &str,
    observed_at: &str,
    evidence_id: &str,
    endpoint: &str,
    scope: &ResourceScope,
) -> ReplayableSignalFixture {
    ReplayableSignalFixture {
        key: key.into(),
        source_kind,
        scope: scope.clone(),
        recorded_json: serde_json::from_str(record).expect("security fixtures are valid JSON"),
        observed_at: Some(observed_at.into()),
        ingested_at: Some(FIXTURE_CLOCK.into()),
        evidence: vec![fixture_evidence(
            evidence_id,
            source_kind,
            endpoint,
            observed_at,
            scope,
        )],
    }
}

fn operational_fixture(
    key: &str,
    source_kind: EvidenceSourceKind,
    recorded_json: Value,
    observed_at: &str,
    evidence_id: &str,
    endpoint: &str,
    scope: &ResourceScope,
) -> ReplayableSignalFixture {
    ReplayableSignalFixture {
        key: key.into(),
        source_kind,
        scope: scope.clone(),
        recorded_json,
        observed_at: Some(observed_at.into()),
        ingested_at: Some(FIXTURE_CLOCK.into()),
        evidence: vec![fixture_evidence(
            evidence_id,
            source_kind,
            endpoint,
            observed_at,
            scope,
        )],
    }
}

fn fixture_evidence(
    id: &str,
    source_kind: EvidenceSourceKind,
    endpoint: &str,
    observed_at: &str,
    scope: &ResourceScope,
) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        source_kind,
        connector_id: Some("fixture-catalog".into()),
        scope: scope.clone(),
        endpoint: endpoint.into(),
        query: Some("recorded fixture".into()),
        observed_at: observed_at.into(),
        excerpt: "synthetic source record".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    }
}

fn validate_fixture_text(value: &str) -> Result<(), CorrelationError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        Err(CorrelationError::InvalidId)
    } else {
        Ok(())
    }
}

fn contains_forbidden_fixture_data(
    fixture: &ReplayableSignalFixture,
) -> Result<bool, CorrelationError> {
    let encoded = serde_json::to_string(fixture).map_err(|_| CorrelationError::InvalidPayload)?;
    let lower = encoded.to_ascii_lowercase();
    Ok([
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "authorization",
        "cookie",
        "arn:",
        "account",
        "subscription",
        "pagination",
        "cursor",
        "next_link",
        "bearer",
    ]
    .iter()
    .any(|marker| lower.contains(marker)))
}
