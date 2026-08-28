//! Deterministic Operations Console aggregation.
//!
//! This module is deliberately a projection boundary.  It consumes the
//! provider-neutral records produced by the existing source adapters and the
//! two deterministic Sprint 11 producers, then emits one evidence-backed
//! [`OperationsSnapshot`].  It does not perform IPC, network I/O, incident
//! mutation, correlation, or background scheduling.

use super::anomaly::{evaluate_rule, AnomalyError};
use super::fixtures::FixtureCatalog;
use super::health_check::{HealthCheckError, HealthCheckScheduler};
use super::model::*;
use crate::observability::alertmanager::NormalizedAlert;
use chrono::{DateTime, SecondsFormat, Utc};
use std::collections::{BTreeMap, BTreeSet};

const SUMMARY_ENDPOINT: &str = "fixture://operations/snapshot";
const DEFAULT_STATUS_DETAIL: &str = "source data is unavailable";

/// Provider-neutral inputs accepted by the aggregation projection.
///
/// Existing cloud, Kubernetes and observability adapters map their results to
/// these fields before calling the aggregator.  Sprint 11's fixture catalog
/// supplies the same shape without opening a network connection.
#[derive(Clone, Debug)]
pub struct AggregationInput {
    pub generated_at: DateTime<Utc>,
    pub source_status: Vec<SourceStatus>,
    pub alerts: Vec<NormalizedAlert>,
    pub metrics: Vec<MetricFixture>,
    pub anomaly_rules: Vec<AnomalyRule>,
    pub health_checks: Vec<HealthCheckSchedule>,
    pub health_check_results: BTreeMap<String, FixtureHealthCheck>,
    pub changes: Vec<ChangeStreamItem>,
    pub environments: Vec<EnvironmentStatus>,
    pub evidence: Vec<EvidenceRef>,
}

impl AggregationInput {
    fn from_fixture_catalog(catalog: FixtureCatalog) -> Self {
        Self {
            generated_at: super::fixtures::fixture_time(),
            source_status: catalog.source_status,
            alerts: catalog.alerts,
            metrics: catalog.metrics,
            anomaly_rules: catalog.anomaly_rules,
            health_checks: catalog.health_checks,
            health_check_results: catalog.health_check_results,
            changes: catalog.changes,
            environments: catalog.environments,
            evidence: catalog.evidence,
        }
    }
}

/// Errors that can prevent construction of a valid projection.
///
/// Source-level malformed data is not returned as a whole-snapshot error.  It
/// is represented in `source_status` while healthy source records continue to
/// render.  This error type is reserved for an invariant failure in the
/// projection itself.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AggregationError {
    #[error("operations snapshot validation failed")]
    SnapshotInvalid,
}

/// Aggregates fixture or adapter input into an evidence-backed console view.
#[derive(Clone, Debug)]
pub struct OperationsAggregator {
    input: AggregationInput,
}

impl OperationsAggregator {
    /// Construct an aggregator from Sprint 11's deterministic fixture catalog.
    pub fn from_fixture_catalog(catalog: FixtureCatalog) -> Self {
        Self {
            input: AggregationInput::from_fixture_catalog(catalog),
        }
    }

    /// Construct an aggregator from provider-neutral adapter output.
    pub fn from_input(input: AggregationInput) -> Self {
        Self { input }
    }

    /// Build a snapshot at an explicit evaluation time.
    ///
    /// The supplied timestamp controls anomaly eligibility and health-check
    /// scheduling.  No wall-clock call or asynchronous task is started here.
    pub fn snapshot_at(&self, now: DateTime<Utc>) -> Result<OperationsSnapshot, AggregationError> {
        let scope = infer_scope(&self.input);
        let mut evidence = EvidenceStore::new(&self.input.evidence, &scope);
        let mut statuses = StatusBook::new(&self.input.source_status, &evidence);

        let alerts = project_alerts(&self.input.alerts, &scope, &mut evidence, &mut statuses);
        let anomalies = project_anomalies(
            &self.input.anomaly_rules,
            &self.input.metrics,
            now,
            &scope,
            &mut evidence,
            &mut statuses,
        );
        let health = project_health_checks(
            &self.input.health_checks,
            &self.input.health_check_results,
            now,
            &scope,
            &mut evidence,
            &mut statuses,
        );
        let changes = project_changes(&self.input.changes, &scope, &mut evidence, &mut statuses);
        let environments = project_environments(
            &self.input.environments,
            &scope,
            &mut evidence,
            &mut statuses,
        );

        let mut queue = Vec::new();
        queue.extend(alerts.items.iter().cloned());
        queue.extend(anomalies.items.iter().cloned());
        queue.extend(health.items.iter().cloned());
        queue.sort_by(queue_order);

        let signal_summary = signal_summary(alerts, anomalies, health, &scope, now, &mut evidence);
        let health_summary =
            health_summary(&queue, &environments, &statuses, &scope, now, &mut evidence);

        let change_stream_status = change_stream_status(&changes, &statuses);
        let mut source_status = statuses.finish(&evidence);
        source_status.sort_by(|left, right| left.source_key.cmp(&right.source_key));

        let snapshot = OperationsSnapshot {
            generated_at: format_timestamp(now),
            scope,
            source_status,
            health_summary,
            incident_queue: queue,
            signal_summary,
            changes,
            change_stream_status,
            environments,
            evidence: evidence.into_sorted_vec(),
            widget_registry: widget_registry(),
        };

        snapshot
            .validate()
            .map_err(|_| AggregationError::SnapshotInvalid)?;
        Ok(snapshot)
    }
}

#[derive(Clone, Debug, Default)]
struct ProjectedItems {
    items: Vec<IncidentQueueItem>,
    evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Debug, Default)]
struct ProjectedHealth {
    items: Vec<IncidentQueueItem>,
    due_evidence_ids: Vec<ConsoleEvidenceId>,
    timed_out_evidence_ids: Vec<ConsoleEvidenceId>,
    has_degraded: bool,
    has_unavailable: bool,
    has_timed_out: bool,
}

#[derive(Clone, Debug, Default)]
struct ProjectedAnomalies {
    items: Vec<IncidentQueueItem>,
    evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Debug)]
struct EvidenceStore {
    accepted: BTreeMap<ConsoleEvidenceId, EvidenceRef>,
    rejected: BTreeSet<ConsoleEvidenceId>,
    scope: ResourceScope,
}

impl EvidenceStore {
    fn new(input: &[EvidenceRef], scope: &ResourceScope) -> Self {
        let mut ordered = input.to_vec();
        ordered.sort_by(|left, right| left.id.cmp(&right.id));
        let mut accepted = BTreeMap::new();
        let mut rejected = BTreeSet::new();
        for evidence in ordered {
            if evidence.id.trim().is_empty()
                || contains_sensitive_word(&evidence.id.to_ascii_lowercase())
                || !scope.contains(&evidence.scope)
            {
                continue;
            }
            if !evidence.redaction.classification_verified || !evidence.redaction.redaction_verified
            {
                rejected.insert(evidence.id.clone());
                accepted.remove(&evidence.id);
                continue;
            }
            if rejected.contains(&evidence.id) {
                continue;
            }
            if accepted.contains_key(&evidence.id) {
                continue;
            }
            accepted.insert(evidence.id.clone(), sanitize_evidence(evidence));
        }
        Self {
            accepted,
            rejected,
            scope: scope.clone(),
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.accepted.contains_key(id)
    }

    fn was_rejected(&self, id: &str) -> bool {
        self.rejected.contains(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn admit_generated(
        &mut self,
        id: impl Into<String>,
        source_kind: EvidenceSourceKind,
        connector_id: Option<String>,
        endpoint: impl Into<String>,
        query: Option<String>,
        observed_at: impl Into<String>,
        excerpt: impl Into<String>,
    ) -> ConsoleEvidenceId {
        let raw_id = id.into();
        let id = if contains_sensitive_word(&raw_id.to_ascii_lowercase()) {
            stable_id(&[&raw_id])
        } else {
            raw_id
        };
        let id = if self.was_rejected(&id) {
            stable_id(&["generated-evidence", &id])
        } else {
            id
        };
        let endpoint = endpoint.into();
        let observed_at = observed_at.into();
        let excerpt = excerpt.into();
        if !self.contains(&id) && !self.was_rejected(&id) {
            self.accepted.insert(
                id.clone(),
                EvidenceRef {
                    id: id.clone(),
                    source_kind,
                    connector_id: connector_id.and_then(safe_identifier),
                    scope: self.scope.clone(),
                    endpoint: safe_text(&endpoint, SUMMARY_ENDPOINT),
                    query: query.map(|value| safe_text(&value, "operations.source")),
                    observed_at: safe_text(&observed_at, "unknown"),
                    excerpt: safe_text(&excerpt, DEFAULT_STATUS_DETAIL),
                    native_url: None,
                    redaction: EvidenceRedaction {
                        classification_verified: true,
                        redaction_verified: true,
                        masked: false,
                        unparsed: false,
                    },
                },
            );
        }
        id
    }

    fn usable_ids(&self, ids: &[ConsoleEvidenceId]) -> Vec<ConsoleEvidenceId> {
        unique_ids(ids.iter().filter(|id| self.contains(id.as_str())))
    }

    fn fallback(
        &mut self,
        key: &str,
        destination: EvidenceSourceKind,
        query: &str,
        observed_at: DateTime<Utc>,
    ) -> ConsoleEvidenceId {
        self.admit_generated(
            format!("evidence-summary-{key}"),
            destination,
            None,
            SUMMARY_ENDPOINT,
            Some(query.to_owned()),
            format_timestamp(observed_at),
            format!("No source records were available for {key}"),
        )
    }

    fn into_sorted_vec(self) -> Vec<EvidenceRef> {
        self.accepted.into_values().collect()
    }
}

fn sanitize_evidence(mut evidence: EvidenceRef) -> EvidenceRef {
    evidence.endpoint = safe_text(&evidence.endpoint, SUMMARY_ENDPOINT);
    evidence.query = evidence
        .query
        .map(|value| safe_text(&value, "operations.source"));
    evidence.observed_at = safe_text(&evidence.observed_at, "unknown");
    evidence.excerpt = safe_text(&evidence.excerpt, DEFAULT_STATUS_DETAIL);
    evidence.native_url = evidence.native_url.and_then(|url| {
        let lower = url.to_ascii_lowercase();
        (lower.starts_with("https://") && !contains_sensitive_word(&lower) && !url.contains('?'))
            .then_some(url)
    });
    evidence.connector_id = evidence.connector_id.and_then(safe_identifier);
    evidence
}

#[derive(Clone, Debug)]
struct StatusBook {
    records: BTreeMap<String, SourceStatus>,
}

impl StatusBook {
    fn new(input: &[SourceStatus], evidence: &EvidenceStore) -> Self {
        let mut book = Self {
            records: BTreeMap::new(),
        };
        let mut ordered = input.to_vec();
        ordered.sort_by(|left, right| left.source_key.cmp(&right.source_key));
        for status in ordered {
            if status.source_key.trim().is_empty() {
                continue;
            }
            let source_key = safe_status_key(&status.source_key);
            let had_evidence = !status.evidence_ids.is_empty();
            let evidence_ids = evidence.usable_ids(&status.evidence_ids);
            let evidence_unverified = had_evidence && evidence_ids.is_empty();
            book.merge(SourceStatus {
                source_key,
                state: if evidence_unverified {
                    SourceState::Unverified
                } else {
                    status.state
                },
                reason: if evidence_unverified {
                    Some(StatusReason::Unknown)
                } else {
                    status.reason
                },
                detail: status.detail.and_then(|value| safe_detail(&value)),
                observed_at: status.observed_at.and_then(safe_optional_timestamp),
                evidence_ids,
            });
        }
        book
    }

    fn merge(&mut self, status: SourceStatus) {
        let key = status.source_key.clone();
        match self.records.get_mut(&key) {
            Some(existing) => {
                if source_state_rank(status.state) > source_state_rank(existing.state) {
                    existing.state = status.state;
                    existing.reason = status.reason;
                    existing.detail = status.detail;
                    existing.observed_at = status.observed_at;
                } else if existing.reason.is_none() {
                    existing.reason = status.reason;
                    existing.detail = status.detail;
                }
                existing.evidence_ids = unique_ids(
                    existing
                        .evidence_ids
                        .iter()
                        .chain(status.evidence_ids.iter()),
                );
            }
            None => {
                self.records.insert(key, status);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mark(
        &mut self,
        source_key: &str,
        state: SourceState,
        reason: Option<StatusReason>,
        detail: Option<&str>,
        evidence_ids: &[ConsoleEvidenceId],
        evidence: &EvidenceStore,
        observed_at: Option<String>,
    ) {
        self.merge(SourceStatus {
            source_key: safe_status_key(source_key),
            state,
            reason,
            detail: detail.and_then(safe_detail),
            observed_at: observed_at.and_then(safe_optional_timestamp),
            evidence_ids: evidence.usable_ids(evidence_ids),
        });
    }

    fn get(&self, source_key: &str) -> Option<&SourceStatus> {
        self.records.get(source_key)
    }

    fn finish(&self, evidence: &EvidenceStore) -> Vec<SourceStatus> {
        self.records
            .values()
            .cloned()
            .map(|mut status| {
                status.evidence_ids = evidence.usable_ids(&status.evidence_ids);
                if status.state == SourceState::Fresh && status.evidence_ids.is_empty() {
                    status.state = SourceState::Unverified;
                    status.reason.get_or_insert(StatusReason::NoDataInWindow);
                }
                if matches!(
                    status.state,
                    SourceState::Unavailable | SourceState::Unverified
                ) && status.reason.is_none()
                {
                    status.reason = Some(StatusReason::Unknown);
                }
                status
            })
            .collect()
    }
}

fn project_alerts(
    alerts: &[NormalizedAlert],
    scope: &ResourceScope,
    evidence: &mut EvidenceStore,
    statuses: &mut StatusBook,
) -> ProjectedItems {
    let mut ordered: Vec<&NormalizedAlert> = alerts.iter().collect();
    ordered.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
    let mut projected = ProjectedItems::default();
    let mut malformed = false;
    let mut seen_ids = BTreeSet::new();
    for alert in ordered {
        if !is_active_alert(alert) || alert.fingerprint.trim().is_empty() {
            if !is_resolved_alert(alert) {
                malformed = true;
            }
            continue;
        }
        if !seen_ids.insert(alert.fingerprint.clone()) {
            malformed = true;
            continue;
        }
        let Some(detected_at) = parse_timestamp(&alert.starts_at) else {
            malformed = true;
            continue;
        };
        let evidence_id = format!("evidence-{}", safe_id_component(&alert.fingerprint));
        let evidence_id = if evidence.contains(&evidence_id) {
            evidence_id
        } else if evidence.was_rejected(&evidence_id) {
            malformed = true;
            continue;
        } else {
            evidence.admit_generated(
                evidence_id,
                EvidenceSourceKind::Alertmanager,
                Some(alert.source.connector_id.clone()),
                alert.source.endpoint.clone(),
                Some("active alerts".into()),
                alert.starts_at.clone(),
                alert_summary(alert),
            )
        };
        let severity = severity_from_labels(&alert.labels);
        let business_impact = business_impact_from_labels(
            &alert.labels,
            alert_summary(alert),
            impact_from_severity(severity),
        );
        let priority = priority_from_labels(&alert.labels);
        let last_update = parse_timestamp(&alert.ends_at)
            .filter(|timestamp| *timestamp >= detected_at)
            .map(format_timestamp)
            .unwrap_or_else(|| format_timestamp(detected_at));
        projected.evidence_ids.push(evidence_id.clone());
        projected.items.push(IncidentQueueItem {
            id: format!("alert-{}", safe_id_component(&alert.fingerprint)),
            title: alert_summary(alert),
            source_kind: QueueItemSourceKind::Alert,
            source_id: safe_id_component(&alert.fingerprint),
            severity,
            priority,
            status: queue_status_from_labels(&alert.labels),
            business_impact,
            scope: scope.clone(),
            detected_at: format_timestamp(detected_at),
            opened_at: format_timestamp(detected_at),
            last_update,
            affected_scope: scope.clone(),
            evidence_ids: vec![evidence_id.clone()],
            drill_down: drill_down(
                DrillDownDestination::IncidentQueue,
                vec![evidence_id.clone()],
                Some(&alert.fingerprint),
            ),
            drill_down_reference: drill_down_reference(
                "alertmanager:active_alerts",
                scope,
                Some(window(detected_at, detected_at)),
                vec![evidence_id],
            ),
        });
    }
    let existing_state = statuses.get("alertmanager").map(|status| status.state);
    let no_records = alerts.is_empty();
    let state = if malformed {
        SourceState::Unverified
    } else if no_records {
        existing_state.unwrap_or(SourceState::Unavailable)
    } else {
        SourceState::Fresh
    };
    let reason = if no_records {
        Some(if existing_state == Some(SourceState::Fresh) {
            StatusReason::NoDataInWindow
        } else {
            StatusReason::NotConfigured
        })
    } else if malformed {
        Some(StatusReason::Unknown)
    } else {
        None
    };
    statuses.mark(
        "alertmanager",
        state,
        reason,
        malformed.then_some("one or more alert records were malformed"),
        &projected.evidence_ids,
        evidence,
        alerts
            .iter()
            .filter_map(|alert| parse_timestamp(&alert.starts_at))
            .max()
            .map(format_timestamp),
    );
    projected
}

fn project_anomalies(
    rules: &[AnomalyRule],
    metrics: &[MetricFixture],
    now: DateTime<Utc>,
    scope: &ResourceScope,
    evidence: &mut EvidenceStore,
    statuses: &mut StatusBook,
) -> ProjectedAnomalies {
    let mut ordered: Vec<&AnomalyRule> = rules.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut projected = ProjectedAnomalies::default();
    let mut malformed = false;
    let mut seen_ids = BTreeSet::new();
    let mut source_evidence_ids = Vec::new();
    for metric in metrics
        .iter()
        .filter(|metric| scope.contains(&metric.scope))
    {
        let derived = format!("evidence-metric-{}", safe_id_component(&metric.key));
        let evidence_id = if evidence.contains(&derived) {
            derived
        } else if evidence.was_rejected(&derived) {
            malformed = true;
            continue;
        } else {
            evidence.admit_generated(
                derived,
                EvidenceSourceKind::Prometheus,
                Some(metric.source.connector_id.clone()),
                metric.source.endpoint.clone(),
                Some(metric.source.query.clone()),
                format_timestamp(now),
                format!("Metric fixture {} is available", metric.key),
            )
        };
        source_evidence_ids.push(evidence_id);
    }
    for rule in ordered {
        if !rule.enabled {
            continue;
        }
        if !seen_ids.insert(rule.id.clone()) {
            malformed = true;
            continue;
        }
        let matching: Vec<&MetricFixture> = metrics
            .iter()
            .filter(|metric| metric.key == rule.metric_key && scope.contains(&metric.scope))
            .collect();
        let Some(metric) = (match matching.as_slice() {
            [metric] => Some(*metric),
            _ => None,
        }) else {
            malformed = true;
            continue;
        };
        let evaluation = match evaluate_rule(rule, metric, now) {
            Ok(evaluation) => evaluation,
            Err(
                AnomalyError::InvalidRule(_)
                | AnomalyError::DuplicateRuleId(_)
                | AnomalyError::MetricNotFound(_)
                | AnomalyError::AmbiguousMetric(_)
                | AnomalyError::ScopeMismatch(_)
                | AnomalyError::InvalidSample(_)
                | AnomalyError::MalformedFixture(_),
            ) => {
                malformed = true;
                continue;
            }
        };
        let Some(signal) = evaluation.signal else {
            continue;
        };
        let evidence_id = signal.evidence_id.clone();
        let evidence_id = if evidence.contains(&evidence_id) {
            evidence_id
        } else if evidence.was_rejected(&evidence_id) {
            malformed = true;
            continue;
        } else {
            evidence.admit_generated(
                evidence_id,
                EvidenceSourceKind::Prometheus,
                Some(metric.source.connector_id.clone()),
                metric.source.endpoint.clone(),
                Some(metric.source.query.clone()),
                signal.observed_at.clone(),
                format!("Anomaly rule {} produced a signal", rule.name),
            )
        };
        let impact = impact_from_severity(signal.severity);
        projected.evidence_ids.push(evidence_id.clone());
        projected.items.push(IncidentQueueItem {
            id: format!("anomaly-{}", safe_id_component(&signal.id)),
            title: safe_text(&rule.name, "Anomaly rule"),
            source_kind: QueueItemSourceKind::Anomaly,
            source_id: safe_id_component(&signal.id),
            severity: signal.severity,
            priority: None,
            status: QueueStatus::Detected,
            business_impact: BusinessImpact {
                level: impact,
                summary: safe_text(
                    &format!("{} requires attention", rule.name),
                    "Anomaly requires attention",
                ),
                customer_scope: "scope represented by source evidence".into(),
                service_criticality: service_criticality_from_impact(impact).into(),
                trajectory: ImpactTrajectory::Unknown,
            },
            scope: signal.scope.clone(),
            detected_at: signal.observed_at.clone(),
            opened_at: signal.observed_at.clone(),
            last_update: signal.observed_at.clone(),
            affected_scope: signal.scope.clone(),
            evidence_ids: vec![evidence_id.clone()],
            drill_down: drill_down(
                DrillDownDestination::IncidentQueue,
                vec![evidence_id.clone()],
                Some(&signal.id),
            ),
            drill_down_reference: drill_down_reference(
                "prometheus:anomaly",
                scope,
                Some(window(
                    now,
                    parse_timestamp(&signal.observed_at).unwrap_or(now),
                )),
                vec![evidence_id],
            ),
        });
    }
    projected.items.sort_by(queue_order);
    let existing_state = statuses.get("prometheus").map(|status| status.state);
    let status_evidence_ids = source_evidence_ids
        .iter()
        .chain(projected.evidence_ids.iter())
        .cloned()
        .collect::<Vec<_>>();
    let no_rules = rules.is_empty();
    let missing_metrics = metrics.is_empty() && !no_rules;
    let no_records = no_rules || missing_metrics;
    statuses.mark(
        "prometheus",
        if malformed {
            SourceState::Unverified
        } else if missing_metrics {
            SourceState::Unavailable
        } else if no_records {
            existing_state.unwrap_or(SourceState::Unavailable)
        } else {
            SourceState::Fresh
        },
        if malformed {
            Some(StatusReason::Unknown)
        } else if missing_metrics {
            Some(StatusReason::NoDataInWindow)
        } else if no_records {
            Some(if existing_state == Some(SourceState::Fresh) {
                StatusReason::NoDataInWindow
            } else {
                StatusReason::NotConfigured
            })
        } else {
            None
        },
        malformed.then_some("one or more metric or rule records were malformed"),
        &status_evidence_ids,
        evidence,
        Some(format_timestamp(now)),
    );
    projected
}

fn project_health_checks(
    schedules: &[HealthCheckSchedule],
    fixtures: &BTreeMap<String, FixtureHealthCheck>,
    now: DateTime<Utc>,
    scope: &ResourceScope,
    evidence: &mut EvidenceStore,
    statuses: &mut StatusBook,
) -> ProjectedHealth {
    let scheduler = HealthCheckScheduler::new(now);
    let mut ordered: Vec<&HealthCheckSchedule> = schedules.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut projected = ProjectedHealth::default();
    let mut malformed = false;
    let mut seen_ids = BTreeSet::new();
    for schedule in ordered {
        if !seen_ids.insert(schedule.id.clone()) {
            malformed = true;
            continue;
        }
        if !scope.contains(&schedule.scope) {
            malformed = true;
            continue;
        }
        let results = scheduler.run_due_checks(std::slice::from_ref(schedule), fixtures, 0);
        let result = match results {
            Ok(mut results) => results.pop(),
            Err(
                HealthCheckError::InvalidSchedule
                | HealthCheckError::InvalidTimestamp
                | HealthCheckError::InvalidDuration
                | HealthCheckError::FixtureNotFound
                | HealthCheckError::PolicyDenied
                | HealthCheckError::AlreadyRunning
                | HealthCheckError::SchedulerUnavailable
                | HealthCheckError::DuplicateSchedule,
            ) => {
                malformed = true;
                None
            }
        };
        let Some(result) = result else {
            continue;
        };
        let evidence_id = result.evidence_id.clone();
        let usable_evidence = evidence_id.filter(|id| evidence.contains(id));
        let evidence_id = match usable_evidence {
            Some(id) => Some(id),
            None if result.evidence_id.is_some() => {
                let id = result.evidence_id.clone().unwrap_or_default();
                if evidence.was_rejected(&id) {
                    malformed = true;
                    None
                } else {
                    Some(evidence.admit_generated(
                        id,
                        EvidenceSourceKind::HealthCheck,
                        Some("fixture-health".into()),
                        "fixture://health-check",
                        Some(schedule.id.clone()),
                        result.observed_at.clone(),
                        format!(
                            "Health check {} returned {:?}",
                            schedule.name, result.outcome
                        ),
                    ))
                }
            }
            None => None,
        };
        let due = !matches!(
            result.outcome,
            HealthCheckOutcome::SkippedNotDue
                | HealthCheckOutcome::SkippedCooldown
                | HealthCheckOutcome::SkippedDisabled
        );
        if due {
            if let Some(id) = evidence_id.clone() {
                projected.due_evidence_ids.push(id);
            }
        }
        if result.outcome == HealthCheckOutcome::TimedOut {
            projected.has_timed_out = true;
            if let Some(id) = evidence_id.clone() {
                projected.timed_out_evidence_ids.push(id);
            }
        }
        if matches!(
            result.outcome,
            HealthCheckOutcome::Degraded
                | HealthCheckOutcome::Unavailable
                | HealthCheckOutcome::TimedOut
        ) {
            projected.has_degraded |= result.outcome == HealthCheckOutcome::Degraded;
            projected.has_unavailable |= result.outcome == HealthCheckOutcome::Unavailable;
            let Some(evidence_id) = evidence_id else {
                malformed = true;
                continue;
            };
            let severity = match result.outcome {
                HealthCheckOutcome::TimedOut | HealthCheckOutcome::Unavailable => {
                    ConsoleSeverity::S2
                }
                HealthCheckOutcome::Degraded => ConsoleSeverity::S3,
                _ => ConsoleSeverity::S3,
            };
            let impact = impact_from_severity(severity);
            projected.items.push(IncidentQueueItem {
                id: format!("health-check-{}", safe_id_component(&schedule.id)),
                title: safe_text(&schedule.name, "Health check"),
                source_kind: QueueItemSourceKind::ScheduledHealthCheck,
                source_id: safe_id_component(&schedule.id),
                severity,
                priority: None,
                status: QueueStatus::Detected,
                business_impact: BusinessImpact {
                    level: impact,
                    summary: safe_text(
                        &format!("{} is {:?}", schedule.name, result.outcome),
                        "Health check requires attention",
                    ),
                    customer_scope: "scope represented by health-check evidence".into(),
                    service_criticality: service_criticality_from_impact(impact).into(),
                    trajectory: ImpactTrajectory::Unknown,
                },
                scope: schedule.scope.clone(),
                detected_at: result.observed_at.clone(),
                opened_at: result.observed_at.clone(),
                last_update: result.observed_at.clone(),
                affected_scope: schedule.scope.clone(),
                evidence_ids: vec![evidence_id.clone()],
                drill_down: drill_down(
                    DrillDownDestination::IncidentQueue,
                    vec![evidence_id.clone()],
                    Some(&schedule.id),
                ),
                drill_down_reference: drill_down_reference(
                    "health_check:results",
                    scope,
                    Some(window(now, now)),
                    vec![evidence_id],
                ),
            });
        }
    }
    projected.items.sort_by(queue_order);
    let existing_state = statuses.get("health_checks").map(|status| status.state);
    let no_records = schedules.is_empty();
    let result_state = if projected.has_unavailable {
        SourceState::Unavailable
    } else if projected.has_degraded || projected.has_timed_out {
        SourceState::Stale
    } else {
        SourceState::Fresh
    };
    statuses.mark(
        "health_checks",
        if malformed {
            SourceState::Unverified
        } else if no_records {
            existing_state.unwrap_or(SourceState::Unavailable)
        } else {
            result_state
        },
        if malformed {
            Some(StatusReason::Unknown)
        } else if no_records {
            Some(if existing_state == Some(SourceState::Fresh) {
                StatusReason::NoDataInWindow
            } else {
                StatusReason::NotConfigured
            })
        } else if projected.has_unavailable {
            Some(StatusReason::Unreachable)
        } else if projected.has_timed_out {
            Some(StatusReason::TimedOut)
        } else if projected.has_degraded {
            Some(StatusReason::Unknown)
        } else {
            None
        },
        malformed.then_some("one or more health-check schedules were unavailable"),
        &projected.due_evidence_ids,
        evidence,
        Some(format_timestamp(now)),
    );
    projected
}

fn project_changes(
    changes: &[ChangeStreamItem],
    scope: &ResourceScope,
    evidence: &mut EvidenceStore,
    statuses: &mut StatusBook,
) -> Vec<ChangeStreamItem> {
    let mut ordered = changes.to_vec();
    ordered.sort_by(|left, right| {
        parse_timestamp(&right.occurred_at)
            .cmp(&parse_timestamp(&left.occurred_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut projected = Vec::new();
    let mut evidence_ids = Vec::new();
    let mut malformed = false;
    let mut seen_ids = BTreeSet::new();
    for mut change in ordered {
        if change.id.trim().is_empty() || parse_timestamp(&change.occurred_at).is_none() {
            malformed = true;
            continue;
        }
        if !seen_ids.insert(change.id.clone()) {
            malformed = true;
            continue;
        }
        if !scope.contains(&change.scope) {
            malformed = true;
            continue;
        }
        let mut ids = evidence.usable_ids(&change.evidence_ids);
        if ids.is_empty() {
            let derived = format!("evidence-change-{}", safe_id_component(&change.id));
            if evidence.was_rejected(&derived) {
                malformed = true;
                continue;
            }
            ids.push(evidence.admit_generated(
                derived,
                EvidenceSourceKind::Fixture,
                Some("fixture-changes".into()),
                "fixture://changes",
                Some(change.id.clone()),
                change.occurred_at.clone(),
                change.summary.clone(),
            ));
        }
        change.evidence_ids = unique_ids(ids.iter());
        change.id = safe_id_component(&change.id);
        change.source = change.source.and_then(|value| safe_detail(&value));
        change.summary = safe_text(&change.summary, "Operational change");
        change.actor = change.actor.and_then(|value| safe_detail(&value));
        change.target_resource = change.target_resource.and_then(|value| safe_detail(&value));
        change.native_link = change.native_link.and_then(safe_native_link);
        change.occurred_at = parse_timestamp(&change.occurred_at)
            .map(format_timestamp)
            .unwrap_or_else(|| "unknown".into());
        change.drill_down.filter_key = Some(change.id.clone());
        change.drill_down.evidence_ids = change.evidence_ids.clone();
        evidence_ids.extend(change.evidence_ids.iter().cloned());
        projected.push(change);
    }
    let existing_state = statuses.get("changes").map(|status| status.state);
    let no_records = changes.is_empty();
    statuses.mark(
        "changes",
        if malformed {
            SourceState::Unverified
        } else if no_records {
            existing_state.unwrap_or(SourceState::Unavailable)
        } else {
            SourceState::Fresh
        },
        if malformed {
            Some(StatusReason::Unknown)
        } else if no_records {
            Some(if existing_state == Some(SourceState::Fresh) {
                StatusReason::NoDataInWindow
            } else {
                StatusReason::NotConfigured
            })
        } else {
            None
        },
        malformed.then_some("one or more change records were malformed"),
        &evidence_ids,
        evidence,
        projected
            .iter()
            .filter_map(|change| parse_timestamp(&change.occurred_at))
            .max()
            .map(format_timestamp),
    );
    projected.sort_by(|left, right| {
        parse_timestamp(&right.occurred_at)
            .cmp(&parse_timestamp(&left.occurred_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    projected
}

fn project_environments(
    environments: &[EnvironmentStatus],
    scope: &ResourceScope,
    evidence: &mut EvidenceStore,
    statuses: &mut StatusBook,
) -> Vec<EnvironmentStatus> {
    let mut ordered = environments.to_vec();
    ordered.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.environment_id.cmp(&right.environment_id))
    });
    let mut seen = BTreeSet::new();
    let mut projected = Vec::new();
    for mut environment in ordered {
        if environment.environment_id.trim().is_empty()
            || !seen.insert(environment.environment_id.clone())
        {
            statuses.mark(
                "environment_status",
                SourceState::Unverified,
                Some(StatusReason::Unknown),
                Some("an environment record was malformed"),
                &[],
                evidence,
                None,
            );
            continue;
        }
        let inferred_scope = environment
            .resource_count
            .drill_down_reference
            .scope
            .clone();
        if !scope.contains(&inferred_scope) {
            statuses.mark(
                &format!("environment:{}", environment.environment_id),
                SourceState::Unavailable,
                Some(StatusReason::Unreachable),
                Some("environment scope is outside the requested workspace"),
                &[],
                evidence,
                None,
            );
            continue;
        }
        let mut ids = evidence.usable_ids(&environment.evidence_ids);
        if ids.is_empty() {
            let derived = format!(
                "evidence-environment-{}",
                safe_id_component(&environment.environment_id)
            );
            if !evidence.was_rejected(&derived) {
                ids.push(evidence.admit_generated(
                    derived,
                    EvidenceSourceKind::Cloud,
                    environment.provider.clone(),
                    "fixture://environment",
                    Some("environment status".into()),
                    environment.last_observed_at.clone(),
                    environment.status_detail.clone(),
                ));
            }
        }
        if ids.is_empty() {
            statuses.mark(
                &format!("environment:{}", environment.environment_id),
                SourceState::Unverified,
                Some(StatusReason::Unknown),
                Some("environment evidence is unavailable"),
                &[],
                evidence,
                None,
            );
            continue;
        }
        let Some(last_observed_at) = parse_timestamp(&environment.last_observed_at) else {
            statuses.mark(
                "environment_status",
                SourceState::Unverified,
                Some(StatusReason::Unknown),
                Some("an environment record was malformed"),
                &ids,
                evidence,
                None,
            );
            continue;
        };
        let resource_value = environment.resource_count.value.trim().to_owned();
        let Ok(parsed_resource_count) = resource_value.parse::<f64>() else {
            statuses.mark(
                "environment_status",
                SourceState::Unverified,
                Some(StatusReason::Unknown),
                Some("an environment record was malformed"),
                &ids,
                evidence,
                None,
            );
            continue;
        };
        if !parsed_resource_count.is_finite() {
            statuses.mark(
                "environment_status",
                SourceState::Unverified,
                Some(StatusReason::Unknown),
                Some("an environment record was malformed"),
                &ids,
                evidence,
                None,
            );
            continue;
        }
        environment.evidence_ids = unique_ids(ids.iter());
        environment.environment_id = safe_id_component(&environment.environment_id);
        environment.name = safe_text(&environment.name, "Environment");
        environment.provider = environment.provider.and_then(safe_identifier);
        environment.status_detail = safe_text(&environment.status_detail, DEFAULT_STATUS_DETAIL);
        environment.last_observed_at = format_timestamp(last_observed_at);
        environment.resource_count = critical_number(
            format!("environment.{}.resource_count", environment.environment_id),
            resource_value,
            NumberUnit::Count,
            environment.evidence_ids.clone(),
            DrillDownDestination::EnvironmentStatus,
            Some(&environment.environment_id),
            scope,
            "environment resources",
            None,
        );
        environment.drill_down.filter_key = Some(environment.environment_id.clone());
        environment.drill_down.evidence_ids = environment.evidence_ids.clone();
        let source_key = format!(
            "{}:{}",
            environment.provider.as_deref().unwrap_or("environment"),
            environment.environment_id
        );
        let (state, reason) = match environment.health {
            ConsoleHealthState::Healthy => (SourceState::Fresh, None),
            ConsoleHealthState::Degraded => (SourceState::Stale, Some(StatusReason::Unknown)),
            ConsoleHealthState::Critical => (SourceState::Unavailable, Some(StatusReason::Unknown)),
            ConsoleHealthState::Unknown => {
                (SourceState::Unavailable, Some(StatusReason::Unreachable))
            }
        };
        statuses.mark(
            &source_key,
            state,
            reason,
            (state != SourceState::Fresh).then_some(environment.status_detail.as_str()),
            &environment.evidence_ids,
            evidence,
            Some(environment.last_observed_at.clone()),
        );
        projected.push(environment);
    }
    projected
}

fn signal_summary(
    alerts: ProjectedItems,
    anomalies: ProjectedAnomalies,
    health: ProjectedHealth,
    scope: &ResourceScope,
    now: DateTime<Utc>,
    evidence: &mut EvidenceStore,
) -> SignalSummary {
    let active_alert_count = alerts.evidence_ids.len();
    let active_anomaly_count = anomalies.evidence_ids.len();
    let checks_due_count = health.due_evidence_ids.len();
    let checks_timed_out_count = health.timed_out_evidence_ids.len();
    let alert_ids = fallback_ids(
        alerts.evidence_ids,
        "active-alerts",
        EvidenceSourceKind::Alertmanager,
        "alertmanager:active_alerts",
        now,
        evidence,
    );
    let anomaly_ids = fallback_ids(
        anomalies.evidence_ids,
        "active-anomalies",
        EvidenceSourceKind::Prometheus,
        "prometheus:anomalies",
        now,
        evidence,
    );
    let due_ids = fallback_ids(
        health.due_evidence_ids,
        "checks-due",
        EvidenceSourceKind::HealthCheck,
        "health_check:due",
        now,
        evidence,
    );
    let timed_out_ids = fallback_ids(
        health.timed_out_evidence_ids,
        "checks-timed-out",
        EvidenceSourceKind::HealthCheck,
        "health_check:timed_out",
        now,
        evidence,
    );
    SignalSummary {
        active_alerts: critical_number(
            "active_alerts".into(),
            active_alert_count.to_string(),
            NumberUnit::Count,
            alert_ids.clone(),
            DrillDownDestination::SignalSummary,
            Some("active_alerts"),
            scope,
            "alertmanager:active_alerts",
            Some(window(now, now)),
        ),
        active_anomalies: critical_number(
            "active_anomalies".into(),
            active_anomaly_count.to_string(),
            NumberUnit::Count,
            anomaly_ids.clone(),
            DrillDownDestination::SignalSummary,
            Some("active_anomalies"),
            scope,
            "prometheus:anomalies",
            Some(window(now, now)),
        ),
        checks_due: critical_number(
            "checks_due".into(),
            checks_due_count.to_string(),
            NumberUnit::Count,
            due_ids.clone(),
            DrillDownDestination::SignalSummary,
            Some("checks_due"),
            scope,
            "health_check:due",
            Some(window(now, now)),
        ),
        checks_timed_out: critical_number(
            "checks_timed_out".into(),
            checks_timed_out_count.to_string(),
            NumberUnit::Count,
            timed_out_ids.clone(),
            DrillDownDestination::SignalSummary,
            Some("checks_timed_out"),
            scope,
            "health_check:timed_out",
            Some(window(now, now)),
        ),
        by_source: vec![
            SignalCount {
                source_kind: QueueItemSourceKind::Alert,
                count: critical_number(
                    "signals.alert".into(),
                    active_alert_count.to_string(),
                    NumberUnit::Count,
                    alert_ids,
                    DrillDownDestination::SignalSummary,
                    Some("source:alert"),
                    scope,
                    "alertmanager:active_alerts",
                    Some(window(now, now)),
                ),
            },
            SignalCount {
                source_kind: QueueItemSourceKind::Anomaly,
                count: critical_number(
                    "signals.anomaly".into(),
                    active_anomaly_count.to_string(),
                    NumberUnit::Count,
                    anomaly_ids,
                    DrillDownDestination::SignalSummary,
                    Some("source:anomaly"),
                    scope,
                    "prometheus:anomalies",
                    Some(window(now, now)),
                ),
            },
            SignalCount {
                source_kind: QueueItemSourceKind::ScheduledHealthCheck,
                count: critical_number(
                    "signals.scheduled_health_check".into(),
                    checks_due_count.to_string(),
                    NumberUnit::Count,
                    due_ids,
                    DrillDownDestination::SignalSummary,
                    Some("source:scheduled_health_check"),
                    scope,
                    "health_check:due",
                    Some(window(now, now)),
                ),
            },
        ],
    }
}

fn health_summary(
    queue: &[IncidentQueueItem],
    environments: &[EnvironmentStatus],
    statuses: &StatusBook,
    scope: &ResourceScope,
    now: DateTime<Utc>,
    evidence: &mut EvidenceStore,
) -> HealthSummary {
    let queue_ids = queue
        .iter()
        .flat_map(|item| item.evidence_ids.iter().cloned())
        .collect::<Vec<_>>();
    let attention_ids = fallback_ids(
        queue_ids,
        "attention",
        EvidenceSourceKind::Fixture,
        "operations:attention",
        now,
        evidence,
    );
    let impacted_ids = fallback_ids(
        queue
            .iter()
            .filter(|item| !matches!(item.business_impact.level, ImpactLevel::None))
            .flat_map(|item| item.evidence_ids.iter().cloned())
            .chain(
                environments
                    .iter()
                    .filter(|environment| environment.health != ConsoleHealthState::Healthy)
                    .flat_map(|environment| environment.evidence_ids.iter().cloned()),
            )
            .collect(),
        "impacted-services",
        EvidenceSourceKind::Fixture,
        "operations:impacted_services",
        now,
        evidence,
    );

    let headline_item = queue.iter().min_by(|left, right| {
        left.business_impact
            .level
            .cmp(&right.business_impact.level)
            .then_with(|| left.severity.cmp(&right.severity))
            .then_with(|| left.id.cmp(&right.id))
    });
    let headline = headline_item
        .map(|item| item.business_impact.clone())
        .unwrap_or_else(|| BusinessImpact {
            level: ImpactLevel::None,
            summary: "No active business impact".into(),
            customer_scope: "no customers currently identified".into(),
            service_criticality: "none".into(),
            trajectory: ImpactTrajectory::Improving,
        });

    let mut active_by_severity = Vec::new();
    for severity in [
        ConsoleSeverity::S1,
        ConsoleSeverity::S2,
        ConsoleSeverity::S3,
        ConsoleSeverity::S4,
        ConsoleSeverity::S5,
    ] {
        let ids = queue
            .iter()
            .filter(|item| item.severity == severity)
            .flat_map(|item| item.evidence_ids.iter().cloned())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            continue;
        }
        active_by_severity.push(critical_number(
            format!("active_by_severity.{severity:?}"),
            queue
                .iter()
                .filter(|item| item.severity == severity)
                .count()
                .to_string(),
            NumberUnit::Count,
            ids,
            DrillDownDestination::IncidentQueue,
            Some(&format!("severity:{severity:?}")),
            scope,
            "operations:incident_queue",
            Some(window(now, now)),
        ));
    }

    let mut environments_by_state = Vec::new();
    for state in [
        ConsoleHealthState::Critical,
        ConsoleHealthState::Degraded,
        ConsoleHealthState::Healthy,
        ConsoleHealthState::Unknown,
    ] {
        let ids = environments
            .iter()
            .filter(|environment| environment.health == state)
            .flat_map(|environment| environment.evidence_ids.iter().cloned())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            continue;
        }
        environments_by_state.push(critical_number(
            format!("environments_by_state.{state:?}"),
            environments
                .iter()
                .filter(|environment| environment.health == state)
                .count()
                .to_string(),
            NumberUnit::Count,
            ids,
            DrillDownDestination::EnvironmentStatus,
            Some(&format!("health:{state:?}")),
            scope,
            "operations:environment_status",
            Some(window(now, now)),
        ));
    }

    let attention = queue.len();
    let impacted_services = queue
        .iter()
        .filter(|item| !matches!(item.business_impact.level, ImpactLevel::None))
        .map(|item| item.business_impact.summary.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        + environments
            .iter()
            .filter(|environment| environment.health != ConsoleHealthState::Healthy)
            .count();

    let state = overall_state(queue, environments, statuses);
    HealthSummary {
        state,
        headline,
        attention: critical_number(
            "attention".into(),
            attention.to_string(),
            NumberUnit::Count,
            attention_ids,
            DrillDownDestination::IncidentQueue,
            Some("active"),
            scope,
            "operations:incident_queue",
            Some(window(now, now)),
        ),
        impacted_services: critical_number(
            "impacted_services".into(),
            impacted_services.to_string(),
            NumberUnit::Count,
            impacted_ids,
            DrillDownDestination::IncidentQueue,
            Some("business_impact"),
            scope,
            "operations:business_impact",
            Some(window(now, now)),
        ),
        active_by_severity,
        environments_by_state,
        contributing_scopes: contributing_scopes(queue),
    }
}

fn overall_state(
    queue: &[IncidentQueueItem],
    environments: &[EnvironmentStatus],
    statuses: &StatusBook,
) -> ConsoleHealthState {
    if queue.iter().any(|item| {
        item.severity == ConsoleSeverity::S1 || item.business_impact.level == ImpactLevel::Critical
    }) || statuses.records.values().any(|status| {
        status.state == SourceState::Unavailable
            && !status.source_key.starts_with("changes")
            && !status.evidence_ids.is_empty()
    }) {
        return ConsoleHealthState::Critical;
    }
    if statuses.records.values().any(|status| {
        !status.source_key.starts_with("changes")
            && (status.state == SourceState::Unverified || status.evidence_ids.is_empty())
    }) {
        return ConsoleHealthState::Unknown;
    }
    if queue.iter().any(|item| {
        matches!(
            item.business_impact.level,
            ImpactLevel::Critical
                | ImpactLevel::High
                | ImpactLevel::Medium
                | ImpactLevel::Low
                | ImpactLevel::Unknown
        )
    }) || environments
        .iter()
        .any(|environment| environment.health != ConsoleHealthState::Healthy)
        || statuses
            .records
            .values()
            .any(|status| status.state == SourceState::Stale)
    {
        return ConsoleHealthState::Degraded;
    }
    ConsoleHealthState::Healthy
}

fn contributing_scopes(queue: &[IncidentQueueItem]) -> Vec<ContributingScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    for item in queue {
        let key = format!(
            "{:?}:{}",
            item.business_impact.level,
            item.scope.scope_json_key()
        );
        if seen.insert(key) {
            scopes.push(ContributingScope {
                scope: item.scope.clone(),
                impact: item.business_impact.level,
                summary: item.business_impact.summary.clone(),
                evidence_ids: item.evidence_ids.clone(),
            });
        }
    }
    scopes.sort_by(|left, right| {
        left.impact
            .cmp(&right.impact)
            .then_with(|| left.summary.cmp(&right.summary))
    });
    scopes
}

trait ScopeKey {
    fn scope_json_key(&self) -> String;
}

impl ScopeKey for ResourceScope {
    fn scope_json_key(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "scope".into())
    }
}

fn change_stream_status(changes: &[ChangeStreamItem], statuses: &StatusBook) -> ChangeStreamStatus {
    if !changes.is_empty() {
        return ChangeStreamStatus {
            state: ChangeStreamState::Available,
            reason: None,
            detail: None,
        };
    }
    let status = statuses.get("changes");
    if matches!(
        status.map(|item| item.state),
        Some(SourceState::Stale | SourceState::Unavailable | SourceState::Unverified)
    ) && status.and_then(|item| item.reason) != Some(StatusReason::NotConfigured)
    {
        return ChangeStreamStatus {
            state: ChangeStreamState::Unavailable,
            reason: Some(
                status
                    .and_then(|item| item.reason)
                    .unwrap_or(StatusReason::Unknown),
            ),
            detail: status.and_then(|item| item.detail.clone()),
        };
    }
    let reason = status
        .and_then(|item| item.reason)
        .unwrap_or(StatusReason::NotConfigured);
    ChangeStreamStatus {
        state: ChangeStreamState::Empty,
        reason: Some(if reason == StatusReason::Unknown {
            StatusReason::NoDataInWindow
        } else {
            reason
        }),
        detail: status.and_then(|item| item.detail.clone()),
    }
}

fn widget_registry() -> Vec<WidgetDefinition> {
    [
        (WidgetId::HealthSummary, "operations.health_summary", true),
        (WidgetId::IncidentQueue, "operations.incident_queue", true),
        (WidgetId::SignalSummary, "operations.signal_summary", false),
        (WidgetId::ChangeStream, "operations.change_stream", false),
        (
            WidgetId::EnvironmentStatus,
            "operations.environment_status",
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(default_order, (id, title_key, required))| WidgetDefinition {
            id,
            title_key: title_key.into(),
            default_order: default_order as u16,
            default_size: if required {
                WidgetSize::Wide
            } else {
                WidgetSize::Standard
            },
            required,
        },
    )
    .collect()
}

#[allow(clippy::too_many_arguments)]
fn critical_number(
    key: String,
    value: String,
    unit: NumberUnit,
    evidence_ids: Vec<ConsoleEvidenceId>,
    destination: DrillDownDestination,
    filter_key: Option<&str>,
    scope: &ResourceScope,
    source_query: &str,
    time_window: Option<TimeWindow>,
) -> CriticalNumber {
    let evidence_ids = unique_ids(evidence_ids.iter());
    let first_evidence = evidence_ids.first().cloned().unwrap_or_default();
    CriticalNumber {
        key,
        value,
        unit,
        evidence_ids: evidence_ids.clone(),
        drill_down: drill_down(destination, evidence_ids.clone(), filter_key),
        drill_down_reference: DrillDownReference {
            source_query: source_query.into(),
            scope: scope.clone(),
            time_window,
            evidence_ids: if first_evidence.is_empty() {
                evidence_ids
            } else {
                vec![first_evidence]
            },
        },
    }
}

fn drill_down(
    destination: DrillDownDestination,
    evidence_ids: Vec<ConsoleEvidenceId>,
    filter_key: Option<&str>,
) -> DrillDownTarget {
    DrillDownTarget {
        destination,
        evidence_ids,
        filter_key: filter_key.and_then(safe_detail),
    }
}

fn drill_down_reference(
    source_query: &str,
    scope: &ResourceScope,
    time_window: Option<TimeWindow>,
    evidence_ids: Vec<ConsoleEvidenceId>,
) -> DrillDownReference {
    DrillDownReference {
        source_query: source_query.into(),
        scope: scope.clone(),
        time_window,
        evidence_ids: unique_ids(evidence_ids.iter()),
    }
}

fn fallback_ids(
    ids: Vec<ConsoleEvidenceId>,
    key: &str,
    source_kind: EvidenceSourceKind,
    query: &str,
    now: DateTime<Utc>,
    evidence: &mut EvidenceStore,
) -> Vec<ConsoleEvidenceId> {
    let ids = evidence.usable_ids(&ids);
    if ids.is_empty() {
        vec![evidence.fallback(key, source_kind, query, now)]
    } else {
        ids
    }
}

fn infer_scope(input: &AggregationInput) -> ResourceScope {
    let mut scopes: Vec<ResourceScope> = input
        .metrics
        .iter()
        .map(|metric| metric.scope.clone())
        .chain(input.anomaly_rules.iter().map(|rule| rule.scope.clone()))
        .chain(input.health_checks.iter().map(|check| check.scope.clone()))
        .chain(input.evidence.iter().map(|item| item.scope.clone()))
        .chain(input.changes.iter().map(|change| change.scope.clone()))
        .chain(input.environments.iter().map(|environment| {
            environment
                .resource_count
                .drill_down_reference
                .scope
                .clone()
        }))
        .filter(|scope| scope.is_bounded())
        .collect();
    scopes.sort_by(|left, right| {
        serde_json::to_string(left)
            .unwrap_or_default()
            .cmp(&serde_json::to_string(right).unwrap_or_default())
    });
    scopes.into_iter().next().unwrap_or_default()
}

fn queue_order(left: &IncidentQueueItem, right: &IncidentQueueItem) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.status.cmp(&right.status))
        .then_with(|| parse_timestamp(&right.detected_at).cmp(&parse_timestamp(&left.detected_at)))
        .then_with(|| left.id.cmp(&right.id))
}

fn severity_from_labels(labels: &BTreeMap<String, String>) -> ConsoleSeverity {
    labels
        .get("severity")
        .or_else(|| labels.get("severity_class"))
        .and_then(|value| match value.trim().to_ascii_uppercase().as_str() {
            "S1" | "CRITICAL" => Some(ConsoleSeverity::S1),
            "S2" | "HIGH" => Some(ConsoleSeverity::S2),
            "S3" | "WARNING" | "WARN" => Some(ConsoleSeverity::S3),
            "S4" | "INFO" => Some(ConsoleSeverity::S4),
            "S5" => Some(ConsoleSeverity::S5),
            _ => None,
        })
        .unwrap_or(ConsoleSeverity::S3)
}

fn priority_from_labels(labels: &BTreeMap<String, String>) -> Option<ConsolePriority> {
    labels.get("priority").and_then(|value| match value.trim() {
        "P1" => Some(ConsolePriority::P1),
        "P2" => Some(ConsolePriority::P2),
        "P3" => Some(ConsolePriority::P3),
        "P4" => Some(ConsolePriority::P4),
        "P5" => Some(ConsolePriority::P5),
        _ => None,
    })
}

fn queue_status_from_labels(labels: &BTreeMap<String, String>) -> QueueStatus {
    labels
        .get("queue_status")
        .or_else(|| labels.get("status"))
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "detected" => Some(QueueStatus::Detected),
            "triage" => Some(QueueStatus::Triage),
            "investigating" => Some(QueueStatus::Investigating),
            "mitigating" => Some(QueueStatus::Mitigating),
            "monitoring" => Some(QueueStatus::Monitoring),
            _ => None,
        })
        .unwrap_or(QueueStatus::Detected)
}

fn impact_from_labels(labels: &BTreeMap<String, String>, fallback: ImpactLevel) -> ImpactLevel {
    labels
        .get("impact")
        .or_else(|| labels.get("business_impact"))
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "critical" => Some(ImpactLevel::Critical),
            "high" => Some(ImpactLevel::High),
            "medium" => Some(ImpactLevel::Medium),
            "low" => Some(ImpactLevel::Low),
            "none" => Some(ImpactLevel::None),
            "unknown" => Some(ImpactLevel::Unknown),
            _ => None,
        })
        .unwrap_or(fallback)
}

fn business_impact_from_labels(
    labels: &BTreeMap<String, String>,
    summary: String,
    fallback: ImpactLevel,
) -> BusinessImpact {
    let level = impact_from_labels(labels, fallback);
    BusinessImpact {
        level,
        summary: safe_text(&summary, "Business impact requires attention"),
        customer_scope: safe_text(
            labels
                .get("customer_scope")
                .map(String::as_str)
                .unwrap_or("customer scope is not specified"),
            "customer scope is not specified",
        ),
        service_criticality: safe_text(
            labels
                .get("service_criticality")
                .map(String::as_str)
                .unwrap_or_else(|| service_criticality_from_impact(level)),
            service_criticality_from_impact(level),
        ),
        trajectory: labels
            .get("trajectory")
            .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
                "expanding" => Some(ImpactTrajectory::Expanding),
                "stable" => Some(ImpactTrajectory::Stable),
                "improving" => Some(ImpactTrajectory::Improving),
                _ => None,
            })
            .unwrap_or(ImpactTrajectory::Unknown),
    }
}

fn impact_from_severity(severity: ConsoleSeverity) -> ImpactLevel {
    match severity {
        ConsoleSeverity::S1 => ImpactLevel::Critical,
        ConsoleSeverity::S2 => ImpactLevel::High,
        ConsoleSeverity::S3 => ImpactLevel::Medium,
        ConsoleSeverity::S4 => ImpactLevel::Low,
        ConsoleSeverity::S5 => ImpactLevel::None,
    }
}

fn service_criticality_from_impact(impact: ImpactLevel) -> &'static str {
    match impact {
        ImpactLevel::Critical => "tier-0",
        ImpactLevel::High => "tier-1",
        ImpactLevel::Medium => "tier-2",
        ImpactLevel::Low => "tier-3",
        ImpactLevel::None => "none",
        ImpactLevel::Unknown => "unknown",
    }
}

fn alert_summary(alert: &NormalizedAlert) -> String {
    let summary = alert
        .annotations
        .get("summary")
        .or_else(|| alert.annotations.get("description"))
        .or_else(|| alert.labels.get("alertname"))
        .cloned()
        .unwrap_or_else(|| "Active alert".into());
    safe_text(&summary, "Active alert")
}

fn is_active_alert(alert: &NormalizedAlert) -> bool {
    !is_resolved_alert(alert)
        && matches!(
            alert.state.trim().to_ascii_lowercase().as_str(),
            "active" | "firing" | "pending" | "open"
        )
}

fn is_resolved_alert(alert: &NormalizedAlert) -> bool {
    matches!(
        alert.state.trim().to_ascii_lowercase().as_str(),
        "resolved" | "inactive" | "closed" | ""
    )
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn safe_optional_timestamp(value: String) -> Option<String> {
    parse_timestamp(&value).map(format_timestamp)
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn window(start: DateTime<Utc>, end: DateTime<Utc>) -> TimeWindow {
    TimeWindow {
        start: format_timestamp(start),
        end: format_timestamp(end),
    }
}

fn unique_ids<'a>(ids: impl IntoIterator<Item = &'a ConsoleEvidenceId>) -> Vec<ConsoleEvidenceId> {
    let mut unique = BTreeSet::new();
    for id in ids {
        if !id.trim().is_empty() {
            unique.insert(id.clone());
        }
    }
    unique.into_iter().collect()
}

fn source_state_rank(state: SourceState) -> u8 {
    match state {
        SourceState::Fresh => 0,
        SourceState::Stale => 1,
        SourceState::Unavailable => 2,
        SourceState::Unverified => 3,
    }
}

fn safe_id_component(value: &str) -> String {
    if !value.is_empty()
        && !contains_sensitive_word(&value.to_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return value.to_owned();
    }
    stable_id(&[value])
}

fn safe_identifier(value: String) -> Option<String> {
    (!value.trim().is_empty() && !contains_sensitive_word(&value.to_ascii_lowercase()))
        .then_some(safe_id_component(&value))
}

fn safe_status_key(value: &str) -> String {
    if !value.trim().is_empty()
        && value.len() <= 128
        && !contains_sensitive_word(&value.to_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_' | b'.'))
    {
        value.to_owned()
    } else {
        stable_id(&[value])
    }
}

fn safe_native_link(value: String) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") && !value.contains('?') && !contains_sensitive_word(&lower))
        .then_some(value)
}

fn safe_detail(value: &str) -> Option<String> {
    (!value.trim().is_empty()
        && value.len() <= 160
        && !contains_sensitive_word(&value.to_ascii_lowercase()))
    .then_some(value.to_owned())
}

fn safe_text(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() || contains_sensitive_word(&value.to_ascii_lowercase()) {
        fallback.to_owned()
    } else {
        value.chars().take(512).collect()
    }
}

fn contains_sensitive_word(value: &str) -> bool {
    [
        "authorization",
        "credential",
        "credential_reference",
        "password",
        "secret",
        "token",
        "bearer",
        "api_key",
        "access_key",
        "private_key",
        "sk-live-",
        "subscription",
        "subscription_id",
        "subscriptionid",
        "account_id",
        "accountid",
        "cursor",
        "next_link",
        "nextlink",
        "next_token",
        "nexttoken",
        "page_token",
        "pagetoken",
        "skip_token",
        "skiptoken",
        "arn:aws:",
    ]
    .iter()
    .any(|word| value.contains(word))
}

fn stable_id(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("evidence-{hash:016x}")
}
