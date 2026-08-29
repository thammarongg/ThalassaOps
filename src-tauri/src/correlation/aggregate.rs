//! Deterministic candidate and snapshot aggregation for grouped Signals.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thalassa_domain::{
    CandidateStatus, CorrelationCandidate, CorrelationError, CorrelationMetric,
    CorrelationMetricKey, CorrelationRequest, CorrelationSnapshot, CorrelationSummary,
    CorrelationWindow, CorrelationWindowState, DrillDownDestination, DrillDownReference,
    DrillDownTarget, EvidenceRef, MaintenanceWindow, NumberUnit, ResourceScope, Signal, SignalId,
    SignalTargetKind, SourceStatus, SuppressionKind, SuppressionRule, TimeWindow, TopologyPath,
};

use super::grouping::{CorrelationComponent, GroupingResult};
use super::suppression::apply_suppression;

/// Pure input to one correlation projection.  Signals and evidence are
/// already admitted by source adapters; this type carries the explicit
/// evaluation request, current workspace scope and local suppression policy
/// inputs into aggregation.
#[derive(Clone, Debug, PartialEq)]
pub struct CorrelationInput {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub request: CorrelationRequest,
    pub signals: Vec<Signal>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
    pub prior_window: Option<CorrelationWindow>,
    pub suppression_rules: Vec<SuppressionRule>,
    pub maintenance_windows: Vec<MaintenanceWindow>,
    pub policy_version: u64,
}

/// Assemble a fully validated snapshot from an already evaluated window and
/// grouped components.  No partial candidate is returned if closure or
/// evidence validation fails.
pub fn aggregate_snapshot(
    input: &CorrelationInput,
    window: &CorrelationWindow,
    grouping: &GroupingResult,
    late_signal_ids: &[SignalId],
) -> Result<CorrelationSnapshot, CorrelationError> {
    input.request.validate()?;
    // Keep this public assembly seam safe for callers that do not use the
    // higher-level orchestration function: suppression must be evaluated over
    // the admitted Signal set before candidate status is projected.
    let mut evaluated_input = input.clone();
    apply_suppression(
        &mut evaluated_input.signals,
        &evaluated_input.suppression_rules,
        &evaluated_input.maintenance_windows,
        &evaluated_input.request.evaluated_at,
        evaluated_input.policy_version,
    )?;
    aggregate_evaluated_snapshot(&evaluated_input, window, grouping, late_signal_ids)
}

fn aggregate_evaluated_snapshot(
    input: &CorrelationInput,
    window: &CorrelationWindow,
    grouping: &GroupingResult,
    late_signal_ids: &[SignalId],
) -> Result<CorrelationSnapshot, CorrelationError> {
    input.request.validate()?;
    window.validate()?;
    if input.request.window != window.range
        || input.request.evaluated_at != window.evaluated_at
        || input.request.allowed_lateness_seconds != window.allowed_lateness_seconds
    {
        return Err(CorrelationError::WindowMismatch);
    }

    let mut signals = input
        .signals
        .iter()
        .filter(|signal| input.scope.contains(&signal.scope))
        .cloned()
        .collect::<Vec<_>>();
    signals.sort_by_key(|signal| signal.id);
    for signal in &mut signals {
        signal.targets.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    let signal_map = signals
        .iter()
        .map(|signal| (signal.id, signal))
        .collect::<BTreeMap<_, _>>();
    if signal_map.len() != signals.len() {
        return Err(CorrelationError::DuplicateId);
    }

    let mut topology_paths = grouping.topology_paths.clone();
    topology_paths.sort_by(|left, right| left.id.cmp(&right.id));
    let mut unique_paths: Vec<TopologyPath> = Vec::with_capacity(topology_paths.len());
    for path in topology_paths {
        if let Some(existing) = unique_paths.last() {
            if existing.id == path.id {
                if existing != &path {
                    return Err(CorrelationError::DuplicateId);
                }
                continue;
            }
        }
        unique_paths.push(path);
    }
    let topology_paths = unique_paths;
    let topology_map = topology_paths
        .iter()
        .map(|path| (path.id.as_str(), path))
        .collect::<BTreeMap<_, _>>();

    let mut candidates = Vec::new();
    for component in &grouping.components {
        let candidate = build_candidate(
            input,
            window,
            component,
            &signal_map,
            &topology_map,
            late_signal_ids,
        )?;
        candidates.push(candidate);
    }
    // Preserve the grouping contract's smallest-key ordering at the snapshot
    // boundary.  Candidate IDs are opaque digests and are intentionally not
    // used as the user-facing component order.
    candidates.sort_by(candidate_ordering);

    let evidence = canonical_evidence(&input.evidence)?;
    let source_status = canonical_source_status(
        input
            .source_status
            .iter()
            .cloned()
            .chain(grouping.source_status.iter().cloned()),
    )?;
    let summary = build_summary(&input.scope, &signals, &candidates);

    let snapshot = CorrelationSnapshot {
        generated_at: input.generated_at.clone(),
        scope: input.scope.clone(),
        request: input.request.clone(),
        window: window.clone(),
        summary,
        signals,
        candidates,
        topology_paths,
        source_status,
        evidence,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Alias used by callers that describe aggregation as projection assembly.
pub fn assemble_snapshot(
    input: &CorrelationInput,
    window: &CorrelationWindow,
    grouping: &GroupingResult,
    late_signal_ids: &[SignalId],
) -> Result<CorrelationSnapshot, CorrelationError> {
    aggregate_snapshot(input, window, grouping, late_signal_ids)
}

fn build_candidate(
    input: &CorrelationInput,
    window: &CorrelationWindow,
    component: &CorrelationComponent,
    signal_map: &BTreeMap<SignalId, &Signal>,
    topology_map: &BTreeMap<&str, &TopologyPath>,
    late_signal_ids: &[SignalId],
) -> Result<CorrelationCandidate, CorrelationError> {
    let mut signal_ids = component.signal_ids.clone();
    signal_ids.sort();
    signal_ids.dedup();
    if signal_ids.len() < 2 || signal_ids.iter().any(|id| !signal_map.contains_key(id)) {
        return Err(if signal_ids.len() < 2 {
            CorrelationError::CandidateTooSmall
        } else {
            CorrelationError::CandidateReferenceMissing
        });
    }

    let mut reasons = component.reasons.clone();
    reasons.sort_by(reason_ordering);
    for reason in &mut reasons {
        reason.signal_ids.sort();
        reason.signal_ids.dedup();
        reason.topology_path_ids.sort();
        reason.topology_path_ids.dedup();
        let mut evidence_ids = BTreeSet::new();
        for signal_id in &reason.signal_ids {
            let signal = signal_map
                .get(signal_id)
                .ok_or(CorrelationError::CandidateReferenceMissing)?;
            evidence_ids.extend(signal.evidence_ids.iter().cloned());
        }
        for path_id in &reason.topology_path_ids {
            let path = topology_map
                .get(path_id.as_str())
                .ok_or(CorrelationError::CandidateReferenceMissing)?;
            evidence_ids.extend(path.evidence_ids.iter().cloned());
        }
        reason.evidence_ids = evidence_ids.into_iter().collect();
        reason.validate()?;
        if !reason.signal_ids.iter().all(|id| signal_ids.contains(id)) {
            return Err(CorrelationError::CandidateReferenceMissing);
        }
    }
    if reasons.is_empty() {
        return Err(CorrelationError::InvalidReason);
    }

    let mut grouping_targets = component.grouping_targets.clone();
    grouping_targets.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.id.cmp(&right.id))
    });
    grouping_targets.dedup();
    for reason in &reasons {
        if let Some(target) = &reason.target {
            if !grouping_targets.contains(target) {
                return Err(CorrelationError::CandidateReferenceMissing);
            }
        }
    }
    let explained_signal_ids = reasons
        .iter()
        .flat_map(|reason| reason.signal_ids.iter().copied())
        .collect::<BTreeSet<_>>();
    if explained_signal_ids != signal_ids.iter().copied().collect::<BTreeSet<_>>() {
        return Err(CorrelationError::CandidateReferenceMissing);
    }

    let mut late_ids = late_signal_ids
        .iter()
        .copied()
        .filter(|id| signal_ids.contains(id))
        .collect::<Vec<_>>();
    late_ids.sort();
    late_ids.dedup();

    let mut evidence_ids = BTreeSet::new();
    for signal_id in &signal_ids {
        evidence_ids.extend(
            signal_map
                .get(signal_id)
                .ok_or(CorrelationError::CandidateReferenceMissing)?
                .evidence_ids
                .iter()
                .cloned(),
        );
    }
    for reason in &reasons {
        evidence_ids.extend(reason.evidence_ids.iter().cloned());
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let id = candidate_id(
        input,
        window,
        &signal_ids,
        &grouping_targets,
        &reasons,
        signal_map,
    );
    let all_suppressed = signal_ids.iter().all(|signal_id| {
        signal_map
            .get(signal_id)
            .is_some_and(|signal| signal.suppression.kind != SuppressionKind::NotSuppressed)
    });
    let status = if all_suppressed {
        // Suppression is the explainability-preserving top-level outcome; a
        // late/reopened state must not hide that every member is suppressed.
        CandidateStatus::Suppressed
    } else if !late_ids.is_empty() || window.state == CorrelationWindowState::Reopened {
        CandidateStatus::Provisional
    } else {
        CandidateStatus::Active
    };
    let (drill_down, drill_down_reference) =
        candidate_drill_down(&id, &input.scope, &input.request.window, &evidence_ids);
    let candidate = CorrelationCandidate {
        id,
        scope: input.scope.clone(),
        window: window.clone(),
        signal_ids,
        grouping_targets,
        reasons,
        status,
        late_signal_ids: late_ids,
        evidence_ids,
        drill_down,
        drill_down_reference,
    };
    candidate.validate()?;
    Ok(candidate)
}

fn candidate_id(
    input: &CorrelationInput,
    window: &CorrelationWindow,
    signal_ids: &[SignalId],
    targets: &[thalassa_domain::SignalTarget],
    reasons: &[thalassa_domain::CorrelationReason],
    signal_map: &BTreeMap<SignalId, &Signal>,
) -> String {
    let mut grouping_keys = targets
        .iter()
        .map(|target| format!("target:{:?}:{}", target.kind, target.id))
        .collect::<Vec<_>>();
    grouping_keys.extend(reasons.iter().flat_map(|reason| {
        reason
            .topology_path_ids
            .iter()
            .map(|id| format!("path:{id}"))
    }));
    grouping_keys.sort();
    grouping_keys.dedup();
    let workspace_keys = signal_ids
        .iter()
        .filter_map(|signal_id| signal_map.get(signal_id))
        .filter_map(|signal| signal.scope.workspace_id)
        .map(|workspace_id| workspace_id.to_string())
        .collect::<BTreeSet<_>>();
    let anchor = signal_ids
        .iter()
        .filter_map(|signal_id| signal_map.get(signal_id))
        .filter_map(|signal| signal.dedup_key.clone())
        .min()
        .unwrap_or_else(|| signal_ids[0].to_string());
    let material = format!(
        "candidate:v1\n{}\n{}\n{}\n{}\n{}\n{}",
        window.range.start,
        window.range.end,
        grouping_keys.join("|"),
        workspace_keys.into_iter().collect::<Vec<_>>().join("|"),
        anchor,
        input
            .scope
            .workspace_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    );
    let digest = Sha256::digest(material.as_bytes());
    format!("candidate:v1:{digest:x}")
}

fn candidate_drill_down(
    candidate_id: &str,
    scope: &ResourceScope,
    window: &TimeWindow,
    evidence_ids: &[String],
) -> (DrillDownTarget, DrillDownReference) {
    (
        DrillDownTarget {
            destination: DrillDownDestination::Evidence,
            evidence_ids: evidence_ids.to_vec(),
            filter_key: Some(candidate_id.into()),
        },
        DrillDownReference {
            source_query: "correlation:snapshot".into(),
            scope: scope.clone(),
            time_window: Some(window.clone()),
            evidence_ids: evidence_ids.to_vec(),
        },
    )
}

fn build_summary(
    scope: &ResourceScope,
    signals: &[Signal],
    candidates: &[CorrelationCandidate],
) -> CorrelationSummary {
    let candidate_signal_ids = candidates
        .iter()
        .flat_map(|candidate| candidate.signal_ids.iter().copied())
        .collect::<BTreeSet<_>>();
    let uncorrelated_signal_ids = signals
        .iter()
        .filter(|signal| !candidate_signal_ids.contains(&signal.id))
        .map(|signal| signal.id)
        .collect::<BTreeSet<_>>();

    let mut metrics = Vec::new();
    push_metric(
        &mut metrics,
        CorrelationMetricKey::NormalizedSignals,
        signals,
        scope,
    );
    push_candidate_metric(
        &mut metrics,
        CorrelationMetricKey::ActiveCandidates,
        candidates,
        CandidateStatus::Active,
        scope,
    );
    push_candidate_metric(
        &mut metrics,
        CorrelationMetricKey::SuppressedCandidates,
        candidates,
        CandidateStatus::Suppressed,
        scope,
    );
    push_metric_by_ids(
        &mut metrics,
        CorrelationMetricKey::UncorrelatedSignals,
        signals,
        &uncorrelated_signal_ids,
        scope,
    );
    CorrelationSummary { metrics }
}

fn candidate_ordering(
    left: &CorrelationCandidate,
    right: &CorrelationCandidate,
) -> std::cmp::Ordering {
    candidate_grouping_key(left)
        .cmp(&candidate_grouping_key(right))
        .then_with(|| left.signal_ids.cmp(&right.signal_ids))
        .then_with(|| left.id.cmp(&right.id))
}

fn candidate_grouping_key(candidate: &CorrelationCandidate) -> String {
    let target_key = candidate
        .grouping_targets
        .iter()
        .map(|target| format!("target:{}:{}", target_kind_rank(target.kind), target.id))
        .min();
    let topology_key = candidate
        .reasons
        .iter()
        .flat_map(|reason| reason.topology_path_ids.iter())
        .min()
        .map(|path_id| format!("path:{path_id}"));
    target_key
        .or(topology_key)
        .unwrap_or_else(|| format!("signal:{}", candidate.signal_ids[0]))
}

fn target_kind_rank(kind: SignalTargetKind) -> u8 {
    match kind {
        SignalTargetKind::Resource => 0,
        SignalTargetKind::Service => 1,
        SignalTargetKind::Deployment => 2,
        SignalTargetKind::Topology => 3,
    }
}

fn push_metric(
    metrics: &mut Vec<CorrelationMetric>,
    key: CorrelationMetricKey,
    signals: &[Signal],
    scope: &ResourceScope,
) {
    let ids = signals
        .iter()
        .map(|signal| signal.id)
        .collect::<BTreeSet<_>>();
    push_metric_by_ids(metrics, key, signals, &ids, scope);
}

fn push_metric_by_ids(
    metrics: &mut Vec<CorrelationMetric>,
    key: CorrelationMetricKey,
    signals: &[Signal],
    ids: &BTreeSet<SignalId>,
    scope: &ResourceScope,
) {
    if ids.is_empty() {
        return;
    }
    let evidence_ids = signals
        .iter()
        .filter(|signal| ids.contains(&signal.id))
        .flat_map(|signal| signal.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if evidence_ids.is_empty() {
        return;
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let key_name = metric_key(key);
    let drill_down = DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: evidence_ids.clone(),
        filter_key: Some(format!("metric:{key_name}")),
    };
    let drill_down_reference = DrillDownReference {
        source_query: format!("correlation:summary:{key_name}"),
        scope: scope.clone(),
        time_window: None,
        evidence_ids: evidence_ids.clone(),
    };
    metrics.push(CorrelationMetric {
        key,
        value: ids.len() as f64,
        unit: NumberUnit::Count,
        evidence_ids,
        drill_down,
        drill_down_reference,
    });
}

fn push_candidate_metric(
    metrics: &mut Vec<CorrelationMetric>,
    key: CorrelationMetricKey,
    candidates: &[CorrelationCandidate],
    status: CandidateStatus,
    scope: &ResourceScope,
) {
    let selected = candidates
        .iter()
        .filter(|candidate| candidate.status == status)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return;
    }
    let evidence_ids = selected
        .iter()
        .flat_map(|candidate| candidate.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if evidence_ids.is_empty() {
        return;
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let key_name = metric_key(key);
    let drill_down = DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: evidence_ids.clone(),
        filter_key: Some(format!("metric:{key_name}")),
    };
    let drill_down_reference = DrillDownReference {
        source_query: format!("correlation:summary:{key_name}"),
        scope: scope.clone(),
        time_window: None,
        evidence_ids: evidence_ids.clone(),
    };
    metrics.push(CorrelationMetric {
        key,
        value: selected.len() as f64,
        unit: NumberUnit::Count,
        evidence_ids,
        drill_down,
        drill_down_reference,
    });
}

fn metric_key(key: CorrelationMetricKey) -> &'static str {
    match key {
        CorrelationMetricKey::NormalizedSignals => "normalized_signals",
        CorrelationMetricKey::ActiveCandidates => "active_candidates",
        CorrelationMetricKey::SuppressedCandidates => "suppressed_candidates",
        CorrelationMetricKey::UncorrelatedSignals => "uncorrelated_signals",
    }
}

fn canonical_evidence(evidence: &[EvidenceRef]) -> Result<Vec<EvidenceRef>, CorrelationError> {
    let mut values = evidence.to_vec();
    values.sort_by(|left, right| left.id.cmp(&right.id));
    let mut map = BTreeMap::new();
    for item in values {
        if let Some(existing) = map.get(&item.id) {
            if existing != &item {
                return Err(CorrelationError::DuplicateId);
            }
        } else {
            map.insert(item.id.clone(), item);
        }
    }
    Ok(map.into_values().collect())
}

fn canonical_source_status(
    statuses: impl IntoIterator<Item = SourceStatus>,
) -> Result<Vec<SourceStatus>, CorrelationError> {
    let mut map = BTreeMap::<String, SourceStatus>::new();
    for mut status in statuses {
        if !safe_identifier(&status.source_key)
            || status
                .detail
                .as_deref()
                .is_some_and(|detail| !safe_text(detail))
        {
            return Err(CorrelationError::InvalidId);
        }
        status.evidence_ids.sort();
        status.evidence_ids.dedup();
        map.entry(status.source_key.clone())
            .and_modify(|existing| {
                if source_status_rank(&status) > source_status_rank(existing) {
                    std::mem::swap(existing, &mut status);
                }
                existing
                    .evidence_ids
                    .extend(status.evidence_ids.iter().cloned());
                existing.evidence_ids.sort();
                existing.evidence_ids.dedup();
            })
            .or_insert(status);
    }
    Ok(map.into_values().collect())
}

fn safe_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.trim().is_empty()
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && [
            "password",
            "passwd",
            "secret",
            "token",
            "credential",
            "authorization",
            "bearer",
            "api_key",
            "access_key",
            "private_key",
            "arn:",
            "/subscriptions/",
            "subscription_id",
            "account_id",
            "pagination_cursor",
            "next_link",
        ]
        .iter()
        .all(|marker| !lower.contains(marker))
        && !contains_sensitive_account_id(&lower)
}

fn safe_text(value: &str) -> bool {
    !value.chars().any(char::is_control)
        && [
            "password",
            "passwd",
            "secret",
            "token",
            "credential",
            "authorization",
            "bearer",
            "api_key",
            "access_key",
            "private_key",
            "arn:",
            "/subscriptions/",
            "subscription_id",
            "account_id",
            "pagination_cursor",
            "next_link",
        ]
        .iter()
        .all(|marker| !value.to_ascii_lowercase().contains(marker))
        && !contains_sensitive_account_id(value)
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

fn source_status_rank(status: &SourceStatus) -> (u8, u8, &str, &str, Option<&str>) {
    (
        match status.state {
            thalassa_domain::SourceState::Fresh => 0,
            thalassa_domain::SourceState::Stale => 1,
            thalassa_domain::SourceState::Unavailable => 2,
            thalassa_domain::SourceState::Unverified => 3,
        },
        match status.reason {
            Some(thalassa_domain::StatusReason::PolicyDenied) => 6,
            Some(thalassa_domain::StatusReason::Unknown) => 5,
            Some(thalassa_domain::StatusReason::Unreachable) => 4,
            Some(thalassa_domain::StatusReason::TimedOut) => 3,
            Some(thalassa_domain::StatusReason::NoDataInWindow) => 2,
            Some(thalassa_domain::StatusReason::NotConfigured) => 1,
            None => 0,
        },
        status.detail.as_deref().unwrap_or_default(),
        status.observed_at.as_deref().unwrap_or_default(),
        status.evidence_ids.first().map(String::as_str),
    )
}

fn reason_ordering(
    left: &thalassa_domain::CorrelationReason,
    right: &thalassa_domain::CorrelationReason,
) -> std::cmp::Ordering {
    reason_kind_rank(left.kind)
        .cmp(&reason_kind_rank(right.kind))
        .then_with(|| {
            left.target
                .as_ref()
                .map(|target| target.kind)
                .cmp(&right.target.as_ref().map(|target| target.kind))
        })
        .then_with(|| {
            left.target
                .as_ref()
                .map(|target| target.id.as_str())
                .cmp(&right.target.as_ref().map(|target| target.id.as_str()))
        })
        .then_with(|| left.topology_path_ids.cmp(&right.topology_path_ids))
        .then_with(|| left.signal_ids.cmp(&right.signal_ids))
}

fn reason_kind_rank(kind: thalassa_domain::CorrelationReasonKind) -> u8 {
    match kind {
        thalassa_domain::CorrelationReasonKind::SharedResource => 0,
        thalassa_domain::CorrelationReasonKind::SharedService => 1,
        thalassa_domain::CorrelationReasonKind::SharedDeployment => 2,
        thalassa_domain::CorrelationReasonKind::TopologyRelation => 3,
        thalassa_domain::CorrelationReasonKind::PrecedingChange => 4,
    }
}
