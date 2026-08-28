//! Deterministic ownership and team resolution for topology nodes.

use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    TopologyError, TopologyNode, TopologyOwnership, TopologyOwnershipRule,
    TopologyOwnershipSelector, TopologyOwnershipSource,
};

/// Resolve one node's owner using the documented specificity order.
pub(crate) fn resolve_ownership(
    node: &TopologyNode,
    rules: &[TopologyOwnershipRule],
    known_evidence: &BTreeSet<String>,
) -> Result<TopologyOwnership, TopologyError> {
    let valid_rules = rules
        .iter()
        .filter(|rule| rule.validate().is_ok())
        .filter(|rule| {
            !rule.evidence_ids.is_empty()
                && rule
                    .evidence_ids
                    .iter()
                    .all(|evidence_id| known_evidence.contains(evidence_id))
        })
        .collect::<Vec<_>>();

    if let Some(mapping) = resolve_node_rules(node, &valid_rules)? {
        return Ok(mapping);
    }

    if let Some(team_id) = node.scope.team_id {
        let team_names = canonical_team_names(&valid_rules);
        if let Some(team_name) = team_names.get(&team_id) {
            return Ok(TopologyOwnership {
                team_id: Some(team_id),
                team_name: Some(team_name.clone()),
                source: TopologyOwnershipSource::ResourceScope,
                evidence_ids: node.evidence_ids.clone(),
            });
        }
    }

    if let Some(environment_id) = node.environment_id.as_ref() {
        let matches = valid_rules
            .iter()
            .filter(|rule| {
                matches!(
                    &rule.selector,
                    TopologyOwnershipSelector::Environment { environment_id: rule_environment }
                        if rule_environment == environment_id
                )
            })
            .copied()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return resolve_consistent_rule_set(matches, node);
        }
    }

    Ok(TopologyOwnership {
        team_id: None,
        team_name: None,
        source: TopologyOwnershipSource::Unassigned,
        evidence_ids: Vec::new(),
    })
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
        return resolve_consistent_rule_set(node_matches, node).map(Some);
    }

    let mut label_matches = rules
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
    label_matches.sort_by(|left, right| label_rule_order(left, right));
    if !label_matches.is_empty() {
        return resolve_consistent_rule_set(label_matches, node).map(Some);
    }

    Ok(None)
}

fn resolve_consistent_rule_set(
    mut matches: Vec<&TopologyOwnershipRule>,
    node: &TopologyNode,
) -> Result<TopologyOwnership, TopologyError> {
    matches.sort_by(|left, right| rule_order(left, right));
    let Some(first) = matches.first().copied() else {
        return Ok(TopologyOwnership {
            team_id: None,
            team_name: None,
            source: TopologyOwnershipSource::Unassigned,
            evidence_ids: Vec::new(),
        });
    };
    if matches
        .iter()
        .any(|rule| rule.team_id != first.team_id || rule.team_name != first.team_name)
    {
        return Err(TopologyError::MalformedSource);
    }

    if first.source == TopologyOwnershipSource::Unassigned {
        return Ok(TopologyOwnership {
            team_id: None,
            team_name: None,
            source: TopologyOwnershipSource::Unassigned,
            evidence_ids: Vec::new(),
        });
    }

    let mut evidence_ids = matches
        .iter()
        .flat_map(|rule| rule.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if evidence_ids.is_empty() {
        evidence_ids.extend(node.evidence_ids.iter().cloned());
    }
    if evidence_ids.is_empty() {
        return Err(TopologyError::EvidenceMissing);
    }

    Ok(TopologyOwnership {
        team_id: Some(first.team_id),
        team_name: Some(first.team_name.clone()),
        source: first.source,
        evidence_ids: evidence_ids.into_iter().collect(),
    })
}

fn canonical_team_names(rules: &[&TopologyOwnershipRule]) -> BTreeMap<uuid::Uuid, String> {
    let mut names = BTreeMap::new();
    let mut sorted = rules.to_vec();
    sorted.sort_by(|left, right| rule_order(left, right));
    for rule in sorted {
        names
            .entry(rule.team_id)
            .or_insert_with(|| rule.team_name.clone());
    }
    names
}

fn label_rule_order(
    left: &TopologyOwnershipRule,
    right: &TopologyOwnershipRule,
) -> std::cmp::Ordering {
    let left_key = match &left.selector {
        TopologyOwnershipSelector::Label { key, value } => (key, value),
        _ => (&String::new(), &String::new()),
    };
    let right_key = match &right.selector {
        TopologyOwnershipSelector::Label { key, value } => (key, value),
        _ => (&String::new(), &String::new()),
    };
    left_key
        .cmp(&right_key)
        .then_with(|| left.team_id.cmp(&right.team_id))
        .then_with(|| left.evidence_ids.cmp(&right.evidence_ids))
}

fn rule_order(left: &TopologyOwnershipRule, right: &TopologyOwnershipRule) -> std::cmp::Ordering {
    selector_order(&left.selector, &right.selector)
        .then_with(|| left.team_id.cmp(&right.team_id))
        .then_with(|| left.team_name.cmp(&right.team_name))
        .then_with(|| left.evidence_ids.cmp(&right.evidence_ids))
}

fn selector_order(
    left: &TopologyOwnershipSelector,
    right: &TopologyOwnershipSelector,
) -> std::cmp::Ordering {
    fn selector_key(selector: &TopologyOwnershipSelector) -> (&str, &str, &str) {
        match selector {
            TopologyOwnershipSelector::NodeId { node_id } => ("node_id", node_id, ""),
            TopologyOwnershipSelector::Label { key, value } => ("label", key, value),
            TopologyOwnershipSelector::Environment { environment_id } => {
                ("environment", environment_id, "")
            }
        }
    }
    selector_key(left).cmp(&selector_key(right))
}
