// SPDX-License-Identifier: Apache-2.0

use serde_json::{json, Value};
use thalassa_domain::*;

fn scope() -> ResourceScope {
    ResourceScope::workspace(
        uuid::Uuid::from_u128(1),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(3),
    )
}

fn evidence_for(id: &str, source_kind: EvidenceSourceKind) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        source_kind,
        connector_id: None,
        scope: scope(),
        endpoint: "fixture://signal-correlation".into(),
        query: None,
        observed_at: "2026-08-28T09:00:00Z".into(),
        excerpt: "synthetic evidence".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    }
}

fn drill_down(ids: &[&str]) -> DrillDownTarget {
    DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: ids.iter().map(|id| (*id).into()).collect(),
        filter_key: None,
    }
}

fn reference(ids: &[&str]) -> DrillDownReference {
    DrillDownReference {
        source_query: "fixture://signal-correlation".into(),
        scope: scope(),
        time_window: Some(TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:05:00Z".into(),
        }),
        evidence_ids: ids.iter().map(|id| (*id).into()).collect(),
    }
}

fn suppression(evaluated_at: &str) -> SuppressionState {
    SuppressionState {
        kind: SuppressionKind::NotSuppressed,
        rule_ids: vec![],
        maintenance_window_ids: vec![],
        evaluated_at: evaluated_at.into(),
        policy_version: 13,
    }
}

fn source_record(source_kind: EvidenceSourceKind, evidence_ids: &[&str]) -> SourceRecordRef {
    SourceRecordRef {
        source_kind,
        native_id: Some("finding-1".into()),
        revision: Some("revision-1".into()),
        content_digest: "sha256:fixture-record-1".into(),
        evidence_ids: evidence_ids.iter().map(|id| (*id).into()).collect(),
    }
}

fn security_signal(id: u128, evidence_ids: &[&str], target_id: &str) -> Signal {
    let ids = evidence_ids.to_vec();
    Signal {
        id: uuid::Uuid::from_u128(id),
        kind: SignalKind::SecurityFinding,
        source: EvidenceSourceKind::Trivy,
        state: SignalState::Observed,
        observed_at: Some("2026-08-28T08:59:00Z".into()),
        ingested_at: Some("2026-08-28T09:00:00Z".into()),
        scope: scope(),
        targets: vec![SignalTarget {
            kind: SignalTargetKind::Resource,
            id: target_id.into(),
        }],
        business_severity: Some(ConsoleSeverity::S2),
        payload: SignalPayload::SecurityFinding {
            finding: VulnerabilityFinding {
                source: EvidenceSourceKind::Trivy,
                asset: FindingAsset {
                    kind: FindingAssetKind::ContainerImage,
                    target: SignalTarget {
                        kind: SignalTargetKind::Resource,
                        id: target_id.into(),
                    },
                    display_name: Some("checkout:2026.08.28".into()),
                    artifact_digest: Some("sha256:artifact-1".into()),
                },
                severity: Some(FindingSeverity::High),
                exploitability: Some(Exploitability::KnownExploit),
                cvss_score: Some(8.1),
                evidence_ids: ids.iter().map(|id| (*id).into()).collect(),
            },
        },
        source_record: source_record(EvidenceSourceKind::Trivy, evidence_ids),
        dedup_key: Some("dedup:v1:trivy:security_finding:fixture".into()),
        suppression: suppression("2026-08-28T09:00:00Z"),
        evidence_ids: ids.iter().map(|id| (*id).into()).collect(),
        drill_down: drill_down(evidence_ids),
        drill_down_reference: reference(evidence_ids),
    }
}

fn topology_path() -> TopologyPath {
    TopologyPath {
        id: "path-checkout".into(),
        root_node_id: "node-checkout".into(),
        terminal_node_id: "node-checkout".into(),
        node_ids: vec!["node-checkout".into()],
        edge_ids: vec![],
        direction: TopologyDirection::Both,
        depth: 0,
        confidence: 1.0,
        kind: TopologyPathKind::ProbableStructural,
        termination: TopologyPathTermination::Leaf,
        cycle_edge_id: None,
        evidence_ids: vec!["evidence-1".into()],
        drill_down: drill_down(&["evidence-1"]),
    }
}

fn correlation_window() -> CorrelationWindow {
    CorrelationWindow {
        range: TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:05:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        watermark: "2026-08-28T08:55:00Z".into(),
        allowed_lateness_seconds: 300,
        state: CorrelationWindowState::Open,
    }
}

fn complete_snapshot() -> CorrelationSnapshot {
    let signal_a = security_signal(10, &["evidence-1"], "resource-checkout");
    let mut signal_b = security_signal(11, &["evidence-2"], "resource-checkout");
    signal_b.source_record.native_id = Some("finding-2".into());
    signal_b.source_record.content_digest = "sha256:fixture-record-2".into();
    signal_b.payload = SignalPayload::Anomaly {
        observed_value: 0.9,
        comparison_value: 0.5,
        condition: AnomalyCondition::Threshold {
            operator: ThresholdOperator::GreaterThan,
            threshold: "0.5".into(),
        },
    };
    signal_b.kind = SignalKind::Anomaly;
    signal_b.source = EvidenceSourceKind::Prometheus;
    signal_b.business_severity = Some(ConsoleSeverity::S3);
    signal_b.source_record.source_kind = EvidenceSourceKind::Prometheus;

    let window = correlation_window();
    let reason = CorrelationReason {
        kind: CorrelationReasonKind::SharedResource,
        qualification: CorrelationQualification::ExactAssociation,
        signal_ids: vec![signal_a.id, signal_b.id],
        target: Some(SignalTarget {
            kind: SignalTargetKind::Resource,
            id: "resource-checkout".into(),
        }),
        topology_path_ids: vec![],
        evidence_ids: vec!["evidence-1".into(), "evidence-2".into()],
    };
    let candidate = CorrelationCandidate {
        id: "candidate-checkout".into(),
        scope: scope(),
        window: window.clone(),
        signal_ids: vec![signal_a.id, signal_b.id],
        grouping_targets: vec![SignalTarget {
            kind: SignalTargetKind::Resource,
            id: "resource-checkout".into(),
        }],
        reasons: vec![reason],
        status: CandidateStatus::Provisional,
        late_signal_ids: vec![signal_b.id],
        evidence_ids: vec!["evidence-1".into(), "evidence-2".into()],
        drill_down: drill_down(&["evidence-1", "evidence-2"]),
        drill_down_reference: reference(&["evidence-1", "evidence-2"]),
    };
    CorrelationSnapshot {
        generated_at: "2026-08-28T09:00:00Z".into(),
        scope: scope(),
        request: CorrelationRequest {
            window: window.range.clone(),
            evaluated_at: window.evaluated_at.clone(),
            allowed_lateness_seconds: window.allowed_lateness_seconds,
        },
        window,
        summary: CorrelationSummary {
            metrics: vec![CorrelationMetric {
                key: CorrelationMetricKey::NormalizedSignals,
                value: 2.0,
                unit: NumberUnit::Count,
                evidence_ids: vec!["evidence-1".into(), "evidence-2".into()],
                drill_down: drill_down(&["evidence-1", "evidence-2"]),
                drill_down_reference: reference(&["evidence-1", "evidence-2"]),
            }],
        },
        signals: vec![signal_a, signal_b],
        candidates: vec![candidate],
        topology_paths: vec![topology_path()],
        source_status: vec![],
        evidence: vec![
            evidence_for("evidence-1", EvidenceSourceKind::Trivy),
            evidence_for("evidence-2", EvidenceSourceKind::Prometheus),
        ],
    }
}

#[test]
fn signal_and_correlation_wire_values_are_stable() {
    macro_rules! assert_wire_values {
        ($type:ty, $( $variant:expr => $wire:expr ),+ $(,)?) => {
            $(
                assert_eq!(serde_json::to_value($variant).unwrap(), json!($wire));
                assert_eq!(
                    serde_json::from_value::<$type>(json!($wire)).unwrap(),
                    $variant
                );
            )+
        };
    }

    assert_wire_values!(
        SignalKind,
        SignalKind::Alert => "alert",
        SignalKind::Anomaly => "anomaly",
        SignalKind::SecurityFinding => "security_finding",
        SignalKind::HealthCheck => "health_check",
    );
    assert_wire_values!(
        SignalState,
        SignalState::Active => "active",
        SignalState::Cleared => "cleared",
        SignalState::Observed => "observed",
        SignalState::Unknown => "unknown",
    );
    assert_wire_values!(
        SignalTargetKind,
        SignalTargetKind::Resource => "resource",
        SignalTargetKind::Service => "service",
        SignalTargetKind::Deployment => "deployment",
        SignalTargetKind::Topology => "topology",
    );
    assert_wire_values!(
        EvidenceSourceKind,
        EvidenceSourceKind::Trivy => "trivy",
        EvidenceSourceKind::Falco => "falco",
        EvidenceSourceKind::Kyverno => "kyverno",
        EvidenceSourceKind::OpaGatekeeper => "opa_gatekeeper",
    );
    assert_wire_values!(
        FindingAssetKind,
        FindingAssetKind::ContainerImage => "container_image",
        FindingAssetKind::RuntimeResource => "runtime_resource",
        FindingAssetKind::KubernetesResource => "kubernetes_resource",
        FindingAssetKind::Host => "host",
        FindingAssetKind::PolicySubject => "policy_subject",
    );
    assert_wire_values!(
        FindingSeverity,
        FindingSeverity::Critical => "critical",
        FindingSeverity::High => "high",
        FindingSeverity::Medium => "medium",
        FindingSeverity::Low => "low",
        FindingSeverity::Negligible => "negligible",
        FindingSeverity::Unknown => "unknown",
    );
    assert_wire_values!(
        Exploitability,
        Exploitability::Exploited => "exploited",
        Exploitability::KnownExploit => "known_exploit",
        Exploitability::Probable => "probable",
        Exploitability::Possible => "possible",
        Exploitability::Unlikely => "unlikely",
        Exploitability::None => "none",
        Exploitability::Unknown => "unknown",
    );
    assert_wire_values!(
        CorrelationWindowState,
        CorrelationWindowState::Open => "open",
        CorrelationWindowState::ReadyToFinalize => "ready_to_finalize",
        CorrelationWindowState::Finalized => "finalized",
        CorrelationWindowState::Reopened => "reopened",
    );
    assert_wire_values!(
        CorrelationReasonKind,
        CorrelationReasonKind::SharedResource => "shared_resource",
        CorrelationReasonKind::SharedService => "shared_service",
        CorrelationReasonKind::SharedDeployment => "shared_deployment",
        CorrelationReasonKind::TopologyRelation => "topology_relation",
    );
    assert_wire_values!(
        CorrelationQualification,
        CorrelationQualification::ExactAssociation => "exact_association",
        CorrelationQualification::ProbableStructural => "probable_structural",
    );
    assert_wire_values!(
        CandidateStatus,
        CandidateStatus::Active => "active",
        CandidateStatus::Provisional => "provisional",
        CandidateStatus::Suppressed => "suppressed",
    );
    assert_wire_values!(
        CorrelationMetricKey,
        CorrelationMetricKey::NormalizedSignals => "normalized_signals",
        CorrelationMetricKey::ActiveCandidates => "active_candidates",
        CorrelationMetricKey::SuppressedCandidates => "suppressed_candidates",
        CorrelationMetricKey::UncorrelatedSignals => "uncorrelated_signals",
    );
    assert_wire_values!(
        SuppressionKind,
        SuppressionKind::NotSuppressed => "not_suppressed",
        SuppressionKind::Rule => "rule",
        SuppressionKind::MaintenanceWindow => "maintenance_window",
        SuppressionKind::RuleAndMaintenanceWindow => "rule_and_maintenance_window",
    );
    assert_wire_values!(
        MaintenanceWindowReason,
        MaintenanceWindowReason::PlannedChange => "planned_change",
        MaintenanceWindowReason::RoutineMaintenance => "routine_maintenance",
        MaintenanceWindowReason::SecurityTesting => "security_testing",
        MaintenanceWindowReason::Unknown => "unknown",
    );

    assert_eq!(
        serde_json::to_value(SignalPayload::Alert).unwrap(),
        json!("alert")
    );
    assert_eq!(
        serde_json::to_value(SignalPayload::Anomaly {
            observed_value: 1.0,
            comparison_value: 0.5,
            condition: AnomalyCondition::Threshold {
                operator: ThresholdOperator::GreaterThan,
                threshold: "0.5".into(),
            },
        })
        .unwrap()["anomaly"]["observed_value"],
        json!(1.0)
    );
    assert!(serde_json::to_value(SignalPayload::SecurityFinding {
        finding: match &security_signal(15, &["evidence-1"], "resource-checkout").payload {
            SignalPayload::SecurityFinding { finding } => finding.clone(),
            _ => unreachable!(),
        },
    })
    .unwrap()
    .get("security_finding")
    .is_some());
    assert_eq!(
        serde_json::to_value(SignalPayload::HealthCheck {
            outcome: HealthCheckOutcome::Healthy,
        })
        .unwrap(),
        json!({"health_check": {"outcome": "healthy"}})
    );
}

#[test]
fn complete_snapshot_round_trips_and_preserves_source_record() {
    let snapshot = complete_snapshot();
    snapshot.validate().expect("complete snapshot is valid");
    let encoded = serde_json::to_value(&snapshot).expect("snapshot serializes");
    assert_eq!(
        encoded["signals"][0]["observed_at"],
        json!("2026-08-28T08:59:00Z")
    );
    assert_eq!(
        encoded["signals"][0]["source_record"]["native_id"],
        json!("finding-1")
    );
    assert_eq!(
        encoded["signals"][0]["payload"]["security_finding"]["finding"]["source"],
        json!("trivy")
    );
    assert_eq!(
        encoded["signals"][1]["payload"]["anomaly"]["observed_value"],
        json!(0.9)
    );
    assert_eq!(
        encoded["candidates"][0]["late_signal_ids"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let decoded: CorrelationSnapshot =
        serde_json::from_value(encoded.clone()).expect("snapshot deserializes");
    assert_eq!(decoded, snapshot);
    assert_eq!(
        encoded["signals"][0]["ingested_at"],
        json!("2026-08-28T09:00:00Z")
    );
}

#[test]
fn absent_source_values_are_explicit_nulls() {
    let mut signal = security_signal(12, &["evidence-1"], "resource-checkout");
    signal.observed_at = None;
    signal.ingested_at = None;
    signal.source_record.native_id = None;
    signal.source_record.revision = None;
    if let SignalPayload::SecurityFinding { finding } = &mut signal.payload {
        finding.asset.display_name = None;
        finding.asset.artifact_digest = None;
        finding.severity = None;
        finding.exploitability = None;
        finding.cvss_score = None;
    }
    let value = serde_json::to_value(signal).unwrap();
    for path in [
        ["observed_at", ""],
        ["ingested_at", ""],
        ["source_record", "native_id"],
        ["source_record", "revision"],
    ] {
        let actual = if path[1].is_empty() {
            &value[path[0]]
        } else {
            &value[path[0]][path[1]]
        };
        assert!(actual.is_null(), "{path:?} should serialize as null");
    }
    let finding = &value["payload"]["security_finding"]["finding"];
    assert!(finding["asset"]["display_name"].is_null());
    assert!(finding["asset"]["artifact_digest"].is_null());
    assert!(finding["severity"].is_null());
    assert!(finding["exploitability"].is_null());
    assert!(finding["cvss_score"].is_null());
}

#[test]
fn suppression_state_keeps_sorted_policy_ids_and_rejects_kind_mismatch() {
    let mut signal = security_signal(16, &["evidence-1"], "resource-checkout");
    signal.suppression = SuppressionState {
        kind: SuppressionKind::RuleAndMaintenanceWindow,
        rule_ids: vec!["rule-a".into(), "rule-z".into()],
        maintenance_window_ids: vec!["maintenance-a".into(), "maintenance-z".into()],
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        policy_version: 13,
    };
    assert!(signal.validate().is_ok());
    let encoded = serde_json::to_value(&signal).unwrap();
    assert_eq!(
        encoded["suppression"]["rule_ids"],
        json!(["rule-a", "rule-z"])
    );
    assert_eq!(encoded["suppression"]["policy_version"], json!(13));

    signal.suppression.kind = SuppressionKind::Rule;
    assert_eq!(
        signal.validate(),
        Err(CorrelationError::SuppressionMismatch)
    );
}

#[test]
fn validation_rejects_non_finite_values_invalid_cvss_and_missing_evidence() {
    let mut snapshot = complete_snapshot();
    if let SignalPayload::Anomaly { observed_value, .. } = &mut snapshot.signals[1].payload {
        *observed_value = f64::NAN;
    }
    assert!(snapshot.validate().is_err());

    let mut finding = match &snapshot.signals[0].payload {
        SignalPayload::SecurityFinding { finding } => finding.clone(),
        _ => unreachable!(),
    };
    finding.cvss_score = Some(10.1);
    assert!(finding.validate().is_err());

    snapshot = complete_snapshot();
    snapshot.signals[0].evidence_ids.clear();
    assert!(snapshot.validate().is_err());

    let mut snapshot = complete_snapshot();
    snapshot.signals[0].dedup_key = Some("token-derived-key".into());
    assert!(snapshot.validate().is_err());
}

#[test]
fn correlation_requires_verified_in_scope_evidence() {
    let mut snapshot = complete_snapshot();
    snapshot.evidence[0].redaction.classification_verified = false;
    assert!(snapshot.validate().is_err());

    let mut snapshot = complete_snapshot();
    snapshot.evidence[0].scope = ResourceScope::workspace(
        uuid::Uuid::from_u128(99),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(1),
    );
    assert!(snapshot.validate().is_err());
}

#[test]
fn correlation_rejects_evidence_from_a_different_signal_source() {
    let mut snapshot = complete_snapshot();
    snapshot.evidence[0].source_kind = EvidenceSourceKind::Prometheus;
    assert_eq!(snapshot.validate(), Err(CorrelationError::SourceMismatch));
}

#[test]
fn candidate_signal_ids_must_resolve_inside_snapshot() {
    let mut snapshot = complete_snapshot();
    snapshot.candidates[0].signal_ids[0] = uuid::Uuid::from_u128(999);
    assert!(snapshot.validate().is_err());
}

#[test]
fn candidate_reasons_must_explain_every_member_signal() {
    let mut snapshot = complete_snapshot();
    let mut third_signal = snapshot.signals[1].clone();
    third_signal.id = uuid::Uuid::from_u128(3);
    snapshot.signals.push(third_signal);
    snapshot.candidates[0]
        .signal_ids
        .push(uuid::Uuid::from_u128(3));
    snapshot.candidates[0].signal_ids.sort();
    assert_eq!(snapshot.validate(), Err(CorrelationError::InvalidReason));
}

#[test]
fn candidate_reason_target_must_match_the_member_signals() {
    let mut snapshot = complete_snapshot();
    let unrelated_target = SignalTarget {
        kind: SignalTargetKind::Resource,
        id: "resource-other".into(),
    };
    snapshot.candidates[0].grouping_targets = vec![unrelated_target.clone()];
    snapshot.candidates[0].reasons[0].target = Some(unrelated_target);
    assert_eq!(snapshot.validate(), Err(CorrelationError::InvalidReason));
}

#[test]
fn candidate_status_must_follow_suppression_and_late_signal_precedence() {
    let mut snapshot = complete_snapshot();
    snapshot.candidates[0].status = CandidateStatus::Active;
    assert_eq!(
        snapshot.validate(),
        Err(CorrelationError::CandidateStatusMismatch)
    );

    snapshot.candidates[0].status = CandidateStatus::Suppressed;
    for signal in &mut snapshot.signals {
        signal.suppression.kind = SuppressionKind::Rule;
        signal.suppression.rule_ids = vec!["rule-checkout".into()];
    }
    assert!(snapshot.validate().is_ok());
}

#[test]
fn payload_kind_and_security_source_must_match() {
    let mut signal = security_signal(13, &["evidence-1"], "resource-checkout");
    signal.kind = SignalKind::Anomaly;
    assert!(signal.validate().is_err());

    let mut signal = security_signal(14, &["evidence-1"], "resource-checkout");
    signal.source = EvidenceSourceKind::Falco;
    assert!(signal.validate().is_err());
}

#[test]
fn correlation_request_enforces_half_open_bounded_window() {
    let request = CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T09:00:00Z".into(),
            end: "2026-08-28T08:00:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        allowed_lateness_seconds: 0,
    };
    assert!(request.validate().is_err());

    let mut request = CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T08:00:00Z".into(),
            end: "2026-08-28T08:00:01Z".into(),
        },
        evaluated_at: "2026-08-28T08:00:01Z".into(),
        allowed_lateness_seconds: 21_601,
    };
    assert!(request.validate().is_err());
    request.allowed_lateness_seconds = 0;
    assert!(request.validate().is_ok());
}

#[test]
fn security_finding_payload_is_not_a_second_signal_envelope() {
    let value =
        serde_json::to_value(security_signal(14, &["evidence-1"], "resource-checkout")).unwrap();
    let finding = &value["payload"]["security_finding"]["finding"];
    assert!(finding.get("id").is_none());
    assert!(finding.get("kind").is_none());
    assert!(finding.get("source_record").is_none());
}

#[allow(dead_code)]
fn _assert_json_object(value: &Value) {
    assert!(value.is_object());
}
