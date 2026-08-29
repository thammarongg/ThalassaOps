//! Exact target and delegated topology grouping for normalized Signals.
//!
//! This module deliberately owns no topology graph logic.  Exact association
//! edges are built from the canonical Signal targets; topology relationships
//! are requested through the Sprint 12 resolver seam and its returned paths
//! are retained unchanged.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use thalassa_domain::{
    CorrelationError, CorrelationQualification, CorrelationReason, CorrelationReasonKind,
    CorrelationWindow, Signal, SignalId, SignalTarget, SignalTargetKind, SourceState, SourceStatus,
    StatusReason, TopologyPath, TopologyPathTermination,
};

pub use crate::topology::TopologyCorrelationResolver;

/// A connected component of Signals and the structural reasons that connect
/// it.  Evidence is populated while the component is built, so this result is
/// useful independently of the snapshot aggregator as well.
#[derive(Clone, Debug, PartialEq)]
pub struct CorrelationComponent {
    pub signal_ids: Vec<SignalId>,
    pub grouping_targets: Vec<SignalTarget>,
    pub reasons: Vec<CorrelationReason>,
}

/// Output of exact-target and topology grouping.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupingResult {
    pub components: Vec<CorrelationComponent>,
    pub topology_paths: Vec<TopologyPath>,
    pub source_status: Vec<SourceStatus>,
}

/// Descriptive alias for callers that distinguish the topology adapter from
/// the broader correlation pipeline.
pub use crate::topology::TopologyCorrelationResolver as CorrelationTopologyResolver;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ReasonKey {
    Target(
        Option<thalassa_domain::WorkspaceId>,
        SignalTargetKind,
        String,
    ),
    Topology(String),
}

#[derive(Clone, Debug)]
struct ReasonRecord {
    kind: CorrelationReasonKind,
    target: Option<SignalTarget>,
    topology_path_ids: BTreeSet<String>,
    signal_ids: BTreeSet<SignalId>,
}

/// Build exact Resource/Service/Deployment components and ask `resolver` for
/// topology relationships between Signals that are not already exactly
/// associated.  The input is expected to contain Signals admitted to one
/// event-time window; `group_signals_in_scope` is the convenience boundary
/// when a caller still has a wider workspace set.
pub fn group_signals(
    signals: &[Signal],
    window: &CorrelationWindow,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<GroupingResult, CorrelationError> {
    window.validate()?;

    let mut ordered = signals.to_vec();
    ordered.sort_by_key(|signal| signal.id);
    let mut seen_ids = BTreeSet::new();
    for signal in &ordered {
        signal.validate()?;
        if !seen_ids.insert(signal.id) {
            return Err(CorrelationError::DuplicateId);
        }
    }

    let start = DateTime::parse_from_rfc3339(&window.range.start)
        .map_err(|_| CorrelationError::InvalidTimestamp)?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(&window.range.end)
        .map_err(|_| CorrelationError::InvalidTimestamp)?
        .with_timezone(&Utc);
    ordered.retain(|signal| {
        signal
            .observed_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|observed_at| {
                let observed_at = observed_at.with_timezone(&Utc);
                observed_at >= start && observed_at < end
            })
            .unwrap_or(false)
    });
    let mut signal_map = BTreeMap::new();
    for signal in &ordered {
        signal_map.insert(signal.id, signal);
    }

    let mut union_find = UnionFind::new(signal_map.keys().copied());
    let mut reasons = BTreeMap::<ReasonKey, ReasonRecord>::new();
    let mut topology_paths = BTreeMap::<String, TopologyPath>::new();
    let mut ambiguous_topology_paths = BTreeSet::new();
    let mut topology_edges = Vec::<(SignalId, SignalId, String)>::new();
    let mut source_status = BTreeMap::<String, SourceStatus>::new();

    // Exact target membership is indexed by workspace plus the complete
    // `(kind, id)` pair; names, labels, timestamps and source kinds never
    // enter this key.
    let mut target_members = BTreeMap::<
        (
            Option<thalassa_domain::WorkspaceId>,
            SignalTargetKind,
            String,
        ),
        BTreeSet<SignalId>,
    >::new();
    for signal in &ordered {
        for target in &signal.targets {
            if matches!(
                target.kind,
                SignalTargetKind::Resource
                    | SignalTargetKind::Service
                    | SignalTargetKind::Deployment
            ) {
                target_members
                    .entry((signal.scope.workspace_id, target.kind, target.id.clone()))
                    .or_default()
                    .insert(signal.id);
            }
        }
    }

    for ((workspace_id, kind, id), members) in target_members {
        if members.len() < 2 {
            continue;
        }
        let member_ids = members.iter().copied().collect::<Vec<_>>();
        union_all(&mut union_find, &member_ids);
        let reason_kind = match kind {
            SignalTargetKind::Resource => CorrelationReasonKind::SharedResource,
            SignalTargetKind::Service => CorrelationReasonKind::SharedService,
            SignalTargetKind::Deployment => CorrelationReasonKind::SharedDeployment,
            SignalTargetKind::Topology => continue,
        };
        reasons.insert(
            ReasonKey::Target(workspace_id, kind, id.clone()),
            ReasonRecord {
                kind: reason_kind,
                target: Some(SignalTarget { kind, id }),
                topology_path_ids: BTreeSet::new(),
                signal_ids: members,
            },
        );
    }

    // Topology calls are pair-bounded and only occur for pairs without an
    // exact edge.  The resolver owns traversal, ownership and path limits.
    for left_index in 0..ordered.len() {
        for right_index in (left_index + 1)..ordered.len() {
            let left = &ordered[left_index];
            let right = &ordered[right_index];
            if left.scope.workspace_id != right.scope.workspace_id
                || share_exact_target(left, right)
            {
                continue;
            }
            let mut left_targets = left.targets.clone();
            let mut right_targets = right.targets.clone();
            left_targets.sort_by(target_ordering);
            right_targets.sort_by(target_ordering);
            for left_target in &left_targets {
                for right_target in &right_targets {
                    if left_target == right_target {
                        continue;
                    }
                    match resolver.relation(left_target, right_target, window) {
                        Ok(Some(path)) => {
                            let path_is_valid = path.validate().is_ok();
                            let path_is_safe = topology_path_identifiers_are_safe(&path);
                            let endpoints_are_present = path.node_ids.contains(&left_target.id)
                                && path.node_ids.contains(&right_target.id);
                            let depth_limited =
                                path.termination == TopologyPathTermination::DepthLimit;
                            if ambiguous_topology_paths.contains(&path.id) {
                                record_topology_limitation(
                                    &mut source_status,
                                    TopologyLimitation::Unverified,
                                );
                                continue;
                            }
                            let conflicting_path = topology_paths
                                .get(&path.id)
                                .is_some_and(|existing| existing != &path);
                            if conflicting_path {
                                topology_paths.remove(&path.id);
                                reasons.remove(&ReasonKey::Topology(path.id.clone()));
                                ambiguous_topology_paths.insert(path.id.clone());
                                record_topology_limitation(
                                    &mut source_status,
                                    TopologyLimitation::Unverified,
                                );
                                continue;
                            }
                            if !path_is_valid
                                || !path_is_safe
                                || !endpoints_are_present
                                || depth_limited
                            {
                                let limitation = if path_is_valid && path_is_safe && depth_limited {
                                    TopologyLimitation::Unavailable
                                } else {
                                    TopologyLimitation::Unverified
                                };
                                record_topology_limitation(&mut source_status, limitation);
                                continue;
                            }
                            if !topology_paths.contains_key(&path.id) {
                                topology_paths.insert(path.id.clone(), path.clone());
                            }
                            topology_edges.push((left.id, right.id, path.id.clone()));
                            let record = reasons
                                .entry(ReasonKey::Topology(path.id.clone()))
                                .or_insert_with(|| ReasonRecord {
                                    kind: CorrelationReasonKind::TopologyRelation,
                                    target: None,
                                    topology_path_ids: BTreeSet::new(),
                                    signal_ids: BTreeSet::new(),
                                });
                            record.signal_ids.insert(left.id);
                            record.signal_ids.insert(right.id);
                            record.topology_path_ids.insert(path.id);
                        }
                        Ok(None) => record_topology_limitation(
                            &mut source_status,
                            TopologyLimitation::Unavailable,
                        ),
                        Err(_) => record_topology_limitation(
                            &mut source_status,
                            TopologyLimitation::Unverified,
                        ),
                    }
                }
            }
        }
    }

    // A path ID is the provenance identity of a topology association.  Delay
    // unioning until all resolver observations have been compared so a later
    // conflicting duplicate cannot leave an orphaned Signal in a component.
    for (left, right, path_id) in topology_edges {
        if !ambiguous_topology_paths.contains(&path_id) && topology_paths.contains_key(&path_id) {
            union_find.union(left, right);
        }
    }

    let mut component_members = BTreeMap::<SignalId, BTreeSet<SignalId>>::new();
    for signal_id in signal_map.keys().copied() {
        let root = union_find.find(signal_id);
        component_members.entry(root).or_default().insert(signal_id);
    }

    let mut components = Vec::new();
    for members in component_members.into_values() {
        if members.len() < 2 {
            continue;
        }
        let signal_ids = members.iter().copied().collect::<Vec<_>>();
        let mut component_reasons = reasons
            .iter()
            .filter(|(_, reason)| reason.signal_ids.iter().all(|id| members.contains(id)))
            .map(|(_, reason)| build_reason(reason, &signal_map, &topology_paths))
            .collect::<Result<Vec<_>, _>>()?;
        if component_reasons.is_empty() {
            continue;
        }
        component_reasons.sort_by(reason_ordering);
        let mut grouping_targets = component_reasons
            .iter()
            .filter_map(|reason| reason.target.clone())
            .collect::<Vec<_>>();
        grouping_targets.sort_by(target_ordering);
        grouping_targets.dedup();
        components.push(CorrelationComponent {
            signal_ids,
            grouping_targets,
            reasons: component_reasons,
        });
    }

    components.sort_by(component_ordering);
    Ok(GroupingResult {
        components,
        topology_paths: topology_paths.into_values().collect(),
        source_status: source_status.into_values().collect(),
    })
}

/// Filter an admitted Signal set to the current scope before exact grouping.
pub fn group_signals_in_scope(
    signals: &[Signal],
    scope: &thalassa_domain::ResourceScope,
    window: &CorrelationWindow,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<GroupingResult, CorrelationError> {
    let scoped = signals
        .iter()
        .filter(|signal| scope.contains(&signal.scope))
        .cloned()
        .collect::<Vec<_>>();
    group_signals(&scoped, window, resolver)
}

/// Alias used by callers that name the output as signal groups.
pub fn build_signal_groups(
    signals: &[Signal],
    window: &CorrelationWindow,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<GroupingResult, CorrelationError> {
    group_signals(signals, window, resolver)
}

fn build_reason(
    record: &ReasonRecord,
    signal_map: &BTreeMap<SignalId, &Signal>,
    topology_paths: &BTreeMap<String, TopologyPath>,
) -> Result<CorrelationReason, CorrelationError> {
    let signal_ids = record.signal_ids.iter().copied().collect::<Vec<_>>();
    let mut evidence_ids = BTreeSet::new();
    for signal_id in &signal_ids {
        let signal = signal_map
            .get(signal_id)
            .ok_or(CorrelationError::CandidateReferenceMissing)?;
        evidence_ids.extend(signal.evidence_ids.iter().cloned());
    }
    for path_id in &record.topology_path_ids {
        let path = topology_paths
            .get(path_id)
            .ok_or(CorrelationError::InvalidTopologyPath)?;
        evidence_ids.extend(path.evidence_ids.iter().cloned());
    }
    let reason = CorrelationReason {
        kind: record.kind,
        qualification: if record.kind == CorrelationReasonKind::TopologyRelation {
            CorrelationQualification::ProbableStructural
        } else {
            CorrelationQualification::ExactAssociation
        },
        signal_ids,
        target: record.target.clone(),
        topology_path_ids: record.topology_path_ids.iter().cloned().collect(),
        evidence_ids: evidence_ids.into_iter().collect(),
    };
    reason.validate()?;
    Ok(reason)
}

fn share_exact_target(left: &Signal, right: &Signal) -> bool {
    left.targets.iter().any(|left_target| {
        matches!(
            left_target.kind,
            SignalTargetKind::Resource | SignalTargetKind::Service | SignalTargetKind::Deployment
        ) && right
            .targets
            .iter()
            .any(|right_target| left_target == right_target)
    })
}

fn target_ordering(left: &SignalTarget, right: &SignalTarget) -> std::cmp::Ordering {
    left.kind
        .cmp(&right.kind)
        .then_with(|| left.id.cmp(&right.id))
}

fn reason_ordering(left: &CorrelationReason, right: &CorrelationReason) -> std::cmp::Ordering {
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

fn reason_kind_rank(kind: CorrelationReasonKind) -> u8 {
    match kind {
        CorrelationReasonKind::SharedResource => 0,
        CorrelationReasonKind::SharedService => 1,
        CorrelationReasonKind::SharedDeployment => 2,
        CorrelationReasonKind::TopologyRelation => 3,
        CorrelationReasonKind::PrecedingChange => 4,
    }
}

fn component_ordering(
    left: &CorrelationComponent,
    right: &CorrelationComponent,
) -> std::cmp::Ordering {
    component_key(left)
        .cmp(&component_key(right))
        .then_with(|| left.signal_ids.cmp(&right.signal_ids))
}

fn component_key(component: &CorrelationComponent) -> String {
    let target_key = component
        .grouping_targets
        .iter()
        .map(|target| format!("target:{}:{}", target_kind_rank(target.kind), target.id))
        .min();
    let topology_key = component
        .reasons
        .iter()
        .flat_map(|reason| reason.topology_path_ids.iter())
        .min()
        .map(|path_id| format!("path:{path_id}"));
    target_key
        .or(topology_key)
        .unwrap_or_else(|| format!("signal:{}", component.signal_ids[0]))
}

fn target_kind_rank(kind: SignalTargetKind) -> u8 {
    match kind {
        SignalTargetKind::Resource => 0,
        SignalTargetKind::Service => 1,
        SignalTargetKind::Deployment => 2,
        SignalTargetKind::Topology => 3,
    }
}

#[derive(Clone, Copy)]
enum TopologyLimitation {
    Unavailable,
    Unverified,
}

fn record_topology_limitation(
    statuses: &mut BTreeMap<String, SourceStatus>,
    limitation: TopologyLimitation,
) {
    let (state, reason) = match limitation {
        TopologyLimitation::Unavailable => {
            (SourceState::Unavailable, Some(StatusReason::NoDataInWindow))
        }
        TopologyLimitation::Unverified => (SourceState::Unverified, Some(StatusReason::Unknown)),
    };
    let candidate = SourceStatus {
        source_key: "topology".into(),
        state,
        reason,
        detail: None,
        observed_at: None,
        evidence_ids: Vec::new(),
    };
    statuses
        .entry("topology".into())
        .and_modify(|existing| {
            if source_state_rank(candidate.state) > source_state_rank(existing.state) {
                *existing = candidate.clone();
            }
        })
        .or_insert(candidate);
}

fn source_state_rank(state: SourceState) -> u8 {
    match state {
        SourceState::Fresh => 0,
        SourceState::Stale => 1,
        SourceState::Unavailable => 2,
        SourceState::Unverified => 3,
    }
}

fn topology_path_identifiers_are_safe(path: &TopologyPath) -> bool {
    std::iter::once(path.id.as_str())
        .chain(std::iter::once(path.root_node_id.as_str()))
        .chain(std::iter::once(path.terminal_node_id.as_str()))
        .chain(path.node_ids.iter().map(String::as_str))
        .chain(path.edge_ids.iter().map(String::as_str))
        .chain(path.cycle_edge_id.iter().map(String::as_str))
        .chain(path.evidence_ids.iter().map(String::as_str))
        .chain(path.drill_down.evidence_ids.iter().map(String::as_str))
        .chain(path.drill_down.filter_key.iter().map(String::as_str))
        .all(safe_identifier)
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

fn union_all(union_find: &mut UnionFind, ids: &[SignalId]) {
    if let Some(first) = ids.first().copied() {
        for signal_id in ids.iter().copied().skip(1) {
            union_find.union(first, signal_id);
        }
    }
}

#[derive(Clone, Debug)]
struct UnionFind {
    parents: BTreeMap<SignalId, SignalId>,
}

impl UnionFind {
    fn new(ids: impl IntoIterator<Item = SignalId>) -> Self {
        let parents = ids.into_iter().map(|id| (id, id)).collect();
        Self { parents }
    }

    fn find(&mut self, id: SignalId) -> SignalId {
        let mut root = id;
        while let Some(parent) = self.parents.get(&root).copied() {
            if parent == root {
                break;
            }
            root = parent;
        }
        let mut current = id;
        while let Some(parent) = self.parents.get(&current).copied() {
            if parent == current {
                break;
            }
            self.parents.insert(current, root);
            current = parent;
        }
        root
    }

    fn union(&mut self, left: SignalId, right: SignalId) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        let (parent, child) = if left_root < right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        self.parents.insert(child, parent);
    }
}
