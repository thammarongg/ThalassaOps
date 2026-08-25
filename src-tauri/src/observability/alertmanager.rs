use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::observability::client::{ObservabilityClient, ObservabilityClientError};

#[derive(Debug, Deserialize)]
pub struct AlertmanagerAlert {
    pub fingerprint: String,
    pub status: AlertmanagerStatus,
    #[serde(rename = "startsAt")]
    pub starts_at: DateTime<Utc>,
    #[serde(rename = "endsAt")]
    pub ends_at: DateTime<Utc>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    #[serde(rename = "generatorURL")]
    pub generator_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlertmanagerStatus {
    pub state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceReference {
    Resolved {
        namespace: String,
        kind: String,
        name: String,
    },
    Unresolved {
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AlertSourceReference {
    pub connector_id: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NormalizedAlert {
    pub fingerprint: String,
    pub state: String,
    pub starts_at: String,
    pub ends_at: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub generator_url: Option<String>,
    pub source: AlertSourceReference,
    pub resource_reference: ResourceReference,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AlertmanagerAlertsRequest {
    pub connector_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AlertmanagerError {
    #[error("client error: {0}")]
    Client(#[from] ObservabilityClientError),
}

pub fn resolve_resource_reference(labels: &BTreeMap<String, String>) -> ResourceReference {
    let namespace = labels.get("namespace");
    if namespace.is_none() {
        return ResourceReference::Unresolved { reason: "missing namespace label".into() };
    }
    let namespace = namespace.unwrap().clone();

    let mut targets = Vec::new();
    if let Some(pod) = labels.get("pod") {
        targets.push(("Pod".to_string(), pod.clone()));
    }
    if let Some(service) = labels.get("service") {
        targets.push(("Service".to_string(), service.clone()));
    }
    if let Some(deployment) = labels.get("deployment") {
        targets.push(("Deployment".to_string(), deployment.clone()));
    }

    if targets.is_empty() {
        return ResourceReference::Unresolved { reason: "missing target label (pod, service, or deployment)".into() };
    }
    if targets.len() > 1 {
        return ResourceReference::Unresolved { reason: "ambiguous resource reference (multiple target labels found)".into() };
    }

    let (kind, name) = targets.into_iter().next().unwrap();
    ResourceReference::Resolved {
        namespace,
        kind,
        name,
    }
}

pub async fn alerts(
    client: &ObservabilityClient,
    request: AlertmanagerAlertsRequest,
) -> Result<Vec<NormalizedAlert>, AlertmanagerError> {
    let url = client.build_url("/api/v2/alerts").map_err(AlertmanagerError::Client)?;
    let req = client.prepare_get(url).map_err(AlertmanagerError::Client)?;

    let response: Vec<AlertmanagerAlert> = client.execute_json(req).await?;

    let normalized = response.into_iter().map(|alert| {
        let resource_reference = resolve_resource_reference(&alert.labels);
        NormalizedAlert {
            fingerprint: alert.fingerprint,
            state: alert.status.state,
            starts_at: alert.starts_at.to_rfc3339(),
            ends_at: alert.ends_at.to_rfc3339(),
            labels: alert.labels,
            annotations: alert.annotations,
            generator_url: alert.generator_url,
            source: AlertSourceReference {
                connector_id: request.connector_id.clone(),
                endpoint: "/api/v2/alerts".into(),
            },
            resource_reference,
        }
    }).collect();

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_resource_reference() {
        let mut labels = BTreeMap::new();
        labels.insert("namespace".into(), "default".into());
        labels.insert("pod".into(), "my-pod".into());
        
        match resolve_resource_reference(&labels) {
            ResourceReference::Resolved { namespace, kind, name } => {
                assert_eq!(namespace, "default");
                assert_eq!(kind, "Pod");
                assert_eq!(name, "my-pod");
            }
            _ => panic!("Expected resolved"),
        }

        labels.insert("service".into(), "my-service".into());
        match resolve_resource_reference(&labels) {
            ResourceReference::Unresolved { reason } => {
                assert!(reason.contains("ambiguous"));
            }
            _ => panic!("Expected unresolved due to ambiguous labels"),
        }

        let mut no_ns = BTreeMap::new();
        no_ns.insert("pod".into(), "my-pod".into());
        match resolve_resource_reference(&no_ns) {
            ResourceReference::Unresolved { reason } => {
                assert!(reason.contains("missing namespace"));
            }
            _ => panic!("Expected unresolved due to missing namespace"),
        }
    }
}
