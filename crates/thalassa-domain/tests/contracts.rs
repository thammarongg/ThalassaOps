// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use thalassa_domain::*;

#[test]
fn hierarchy_and_local_principal_form_a_single_user_secure_workspace() {
    let organization = Organization::new("Acme");
    let team = Team::new(organization.id, "Platform");
    let workspace = Workspace::new(team.id, "Production");
    let environment = Environment::new(workspace.id, "prod", EnvironmentKind::Kubernetes);
    let principal = Principal::local("operator", "Operator");
    let membership = Membership::workspace_owner(principal.id, workspace.id);

    assert!(membership.scope.contains(&ResourceScope::environment(
        environment.id,
        workspace.id,
        team.id,
        organization.id
    )));
    assert_eq!(principal.identity.issuer, None);
    assert_eq!(environment.workspace_id, workspace.id);
}

#[test]
fn resource_scope_membership_can_be_narrowed_to_specific_resources() {
    let organization_id = uuid::Uuid::new_v4();
    let team_id = uuid::Uuid::new_v4();
    let workspace_id = uuid::Uuid::new_v4();
    let environment_id = uuid::Uuid::new_v4();
    let allowed_resource = uuid::Uuid::new_v4();
    let other_resource = uuid::Uuid::new_v4();
    let membership_scope =
        ResourceScope::environment(environment_id, workspace_id, team_id, organization_id)
            .resource(allowed_resource);
    let allowed =
        ResourceScope::environment(environment_id, workspace_id, team_id, organization_id)
            .resource(allowed_resource);
    let denied = ResourceScope::environment(environment_id, workspace_id, team_id, organization_id)
        .resource(other_resource);

    assert!(membership_scope.contains(&allowed));
    assert!(!membership_scope.contains(&denied));
    assert!(!membership_scope.contains(&ResourceScope::environment(
        environment_id,
        workspace_id,
        team_id,
        organization_id,
    )));
}

#[test]
fn domain_entities_preserve_glossary_relationships_and_action_dimensions() {
    let organization = Organization::new("Acme");
    let team = Team::new(organization.id, "Platform");
    let workspace = Workspace::new(team.id, "Production");
    let environment = Environment::new(workspace.id, "prod", EnvironmentKind::Kubernetes);
    let scope = ResourceScope::environment(environment.id, workspace.id, team.id, organization.id);
    let resource = Resource::new(environment.id, scope.clone(), "deployment", "api");
    let signal = Signal::new("prometheus", "alert", vec![resource.id]);
    let evidence = Evidence::new("prometheus", scope.clone(), "query", "api is unavailable");
    let hypothesis = Hypothesis::new("api pods are unhealthy", 0.8, vec![evidence.id]);
    let action = Action::new(
        "restart api",
        ActionRiskClass::Mutating,
        ExecutionMode::Approval,
        scope.clone(),
    );
    let incident = Incident::new("API outage", IncidentSeverity::S2, scope.clone());
    let policy = Policy::new("baseline", 1, scope.clone());
    let audit = Audit::new("incident.created", scope);

    assert_eq!(resource.name, "api");
    assert_eq!(signal.resource_ids, vec![resource.id]);
    assert_eq!(hypothesis.evidence_ids, vec![evidence.id]);
    assert_eq!(action.risk_class, ActionRiskClass::Mutating);
    assert_eq!(action.execution_mode, ExecutionMode::Approval);
    assert_eq!(incident.severity, IncidentSeverity::S2);
    assert_eq!(policy.version, 1);
    assert_eq!(audit.event_type, "incident.created");
}

#[test]
fn entities_serialize_as_stable_json_contracts() {
    let mut labels = BTreeMap::new();
    labels.insert("service".to_string(), "api".to_string());
    let resource = Resource::new(
        uuid::Uuid::nil(),
        ResourceScope::workspace(uuid::Uuid::nil(), uuid::Uuid::nil(), uuid::Uuid::nil()),
        "service",
        "api",
    )
    .with_labels(labels);
    let value = serde_json::to_value(resource).unwrap();
    assert_eq!(value["kind"], "service");
    assert_eq!(value["labels"]["service"], "api");
}
