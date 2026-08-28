//! Deterministic ownership and team resolution for topology nodes.

use std::collections::{BTreeMap, BTreeSet};

use thalassa_domain::{
    TeamId, TopologyError, TopologyNode, TopologyOwnership, TopologyOwnershipRule,
    TopologyOwnershipSelector, TopologyOwnershipSource,
};
use uuid::Uuid;

pub(crate) type OwnershipSelectorKey = (u8, String, String);

/// Validate and admit ownership rules before they are considered for any
/// node. Invalid, duplicate and out-of-workspace rules are omitted from
/// resolution and reported to the caller through the boolean result.
pub(crate) fn validate_rules<'a>(
    rules: &'a [TopologyOwnershipRule],
    known_evidence: &BTreeSet<String>,
    workspace_team_id: Option<TeamId>,
) -> (
    Vec<&'a TopologyOwnershipRule>,
    BTreeSet<OwnershipSelectorKey>,
    bool,
) {
    let mut invalid = false;
    let mut candidates = Vec::new();
    for rule in rules {
        let valid = rule.validate().is_ok()
            && rule.team_id != Uuid::nil()
            && workspace_team_id.is_none_or(|team_id| team_id == rule.team_id)
            && !rule.evidence_ids.is_empty()
            && rule
                .evidence_ids
                .iter()
                .all(|evidence_id| known_evidence.contains(evidence_id))
            && source_matches_selector(rule);
        if valid {
            candidates.push(rule);
        } else {
            invalid = true;
        }
    }

    // Duplicate selectors are rejected even when they carry identical output.
    // Choosing one by input order would make ownership depend on adapter
    // response ordering and would hide a broken mapping source.
    let mut selector_counts = BTreeMap::new();
    for rule in &candidates {
        *selector_counts
            .entry(selector_key(&rule.selector))
            .or_insert(0usize) += 1;
    }
    if selector_counts.values().any(|count| *count > 1) {
        invalid = true;
        let rejected_selectors = selector_counts
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(selector, _)| selector.clone())
            .collect();
        candidates.retain(|rule| selector_counts[&selector_key(&rule.selector)] == 1);
        return (candidates, rejected_selectors, invalid);
    }

    (candidates, BTreeSet::new(), invalid)
}

/// Resolve one node's owner using the documented specificity order.
pub(crate) fn resolve_ownership(
    node: &TopologyNode,
    rules: &[&TopologyOwnershipRule],
    rejected_selectors: &BTreeSet<OwnershipSelectorKey>,
) -> Result<TopologyOwnership, TopologyError> {
    if rejected_selectors
        .iter()
        .any(|selector| selector_matches_node(selector, node))
    {
        return Err(TopologyError::MalformedSource);
    }
    if let Some(mapping) = resolve_node_rules(node, rules)? {
        return Ok(mapping);
    }

    if let Some(team_id) = node.scope.team_id.filter(|team_id| *team_id != Uuid::nil()) {
        if let Some((team_name, mapping_evidence_ids)) = canonical_team_name(rules, team_id)? {
            let evidence_ids = evidence_mapping(&node.evidence_ids, &mapping_evidence_ids);
            return Ok(TopologyOwnership {
                team_id: Some(team_id),
                team_name: Some(team_name),
                source: TopologyOwnershipSource::ResourceScope,
                evidence_ids,
            });
        }
        // A ResourceScope team is a higher-precedence claim than an
        // environment default. If its canonical display context is absent,
        // keep the node explicitly unassigned instead of allowing a lower
        // precedence rule to claim it.
        return Ok(unassigned_ownership());
    }

    if let Some(environment_id) = node.environment_id.as_ref() {
        let matches = rules
            .iter()
            .filter(|rule| {
                matches!(
                    &rule.selector,
                    TopologyOwnershipSelector::Environment {
                        environment_id: rule_environment
                    } if rule_environment == environment_id
                )
            })
            .copied()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return resolve_consistent_rule_set(matches);
        }
    }

    Ok(unassigned_ownership())
}

fn resolve_node_rules(
    node: &TopologyNode,
    rules: &[&TopologyOwnershipRule],
) -> Result<Option<TopologyOwnership>, TopologyError> {
    let node_matches = rules
        .iter()
        .filter(|rule| {
            matches!(
                &rule.selector,
                TopologyOwnershipSelector::NodeId { node_id } if node_id == &node.id
            )
        })
        .copied()
        .collect::<Vec<_>>();
    if !node_matches.is_empty() {
        return resolve_consistent_rule_set(node_matches).map(Some);
    }

    // Label selectors are exact selectors in the adapter contract. Therefore
    // every matching label rule has the same specificity; conflicting owners
    // are ambiguous and must be surfaced instead of selected by ordering.
    let label_matches = rules
        .iter()
        .filter(|rule| {
            matches!(
                &rule.selector,
                TopologyOwnershipSelector::Label { key, value }
                    if node.labels.get(key) == Some(value)
            )
        })
        .copied()
        .collect::<Vec<_>>();
    if !label_matches.is_empty() {
        return resolve_consistent_rule_set(label_matches).map(Some);
    }

    Ok(None)
}

fn resolve_consistent_rule_set(
    mut matches: Vec<&TopologyOwnershipRule>,
) -> Result<TopologyOwnership, TopologyError> {
    matches.sort_by(|left, right| rule_order(left, right));
    let Some(first) = matches.first().copied() else {
        return Ok(unassigned_ownership());
    };
    if matches
        .iter()
        .any(|rule| rule.team_id != first.team_id || rule.team_name != first.team_name)
    {
        return Err(TopologyError::MalformedSource);
    }

    if first.source == TopologyOwnershipSource::Unassigned {
        return Ok(unassigned_ownership());
    }

    let evidence_ids = matches
        .iter()
        .flat_map(|rule| rule.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if evidence_ids.is_empty() {
        // This should be unreachable after validate_rules, but keeping the
        // invariant here prevents a future caller from fabricating ownership.
        return Err(TopologyError::EvidenceMissing);
    }

    Ok(TopologyOwnership {
        team_id: Some(first.team_id),
        team_name: Some(first.team_name.clone()),
        source: first.source,
        evidence_ids: evidence_ids.into_iter().collect(),
    })
}

fn canonical_team_name(
    rules: &[&TopologyOwnershipRule],
    team_id: TeamId,
) -> Result<Option<(String, Vec<String>)>, TopologyError> {
    let mut matches = rules
        .iter()
        .filter(|rule| {
            rule.team_id == team_id && rule.source != TopologyOwnershipSource::Unassigned
        })
        .copied()
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(None);
    }
    matches.sort_by(|left, right| rule_order(left, right));
    let first = matches[0];
    if matches.iter().any(|rule| rule.team_name != first.team_name) {
        return Err(TopologyError::MalformedSource);
    }
    let evidence_ids = matches
        .iter()
        .flat_map(|rule| rule.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Some((first.team_name.clone(), evidence_ids)))
}

fn evidence_mapping(node_evidence_ids: &[String], mapping_evidence_ids: &[String]) -> Vec<String> {
    node_evidence_ids
        .iter()
        .chain(mapping_evidence_ids.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unassigned_ownership() -> TopologyOwnership {
    TopologyOwnership {
        team_id: None,
        team_name: None,
        source: TopologyOwnershipSource::Unassigned,
        evidence_ids: Vec::new(),
    }
}

fn source_matches_selector(rule: &TopologyOwnershipRule) -> bool {
    matches!(
        (&rule.selector, rule.source),
        (
            TopologyOwnershipSelector::NodeId { .. },
            TopologyOwnershipSource::Fixture
        ) | (
            TopologyOwnershipSelector::NodeId { .. },
            TopologyOwnershipSource::Unassigned
        ) | (
            TopologyOwnershipSelector::Label { .. },
            TopologyOwnershipSource::ExplicitLabel
        ) | (
            TopologyOwnershipSelector::Environment { .. },
            TopologyOwnershipSource::EnvironmentDefault,
        )
    )
}

fn selector_key(selector: &TopologyOwnershipSelector) -> OwnershipSelectorKey {
    match selector {
        TopologyOwnershipSelector::NodeId { node_id } => (0, node_id.clone(), String::new()),
        TopologyOwnershipSelector::Label { key, value } => (1, key.clone(), value.clone()),
        TopologyOwnershipSelector::Environment { environment_id } => {
            (2, environment_id.clone(), String::new())
        }
    }
}

fn selector_matches_node(selector: &OwnershipSelectorKey, node: &TopologyNode) -> bool {
    match selector.0 {
        0 => node.id == selector.1,
        1 => node.labels.get(&selector.1) == Some(&selector.2),
        2 => node.environment_id.as_deref() == Some(selector.1.as_str()),
        _ => false,
    }
}

fn rule_order(left: &TopologyOwnershipRule, right: &TopologyOwnershipRule) -> std::cmp::Ordering {
    selector_key(&left.selector)
        .cmp(&selector_key(&right.selector))
        .then_with(|| left.team_id.cmp(&right.team_id))
        .then_with(|| left.team_name.cmp(&right.team_name))
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.evidence_ids.cmp(&right.evidence_ids))
}
