use serde::{Deserialize, Serialize};

pub const AWS_CONNECTOR_KIND: &str = "aws";
pub const AZURE_CONNECTOR_KIND: &str = "azure";
pub const GCP_CONNECTOR_KIND: &str = "gcp";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudProvider {
    #[serde(rename = "aws")]
    Aws,
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "gcp")]
    Gcp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudResourceType {
    #[serde(rename = "kubernetes_cluster")]
    KubernetesCluster,
    #[serde(rename = "compute_instance")]
    ComputeInstance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudHealthState {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudAccessState {
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "no_credential")]
    NoCredential,
    #[serde(rename = "session_expired")]
    SessionExpired,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudEnvironment {
    pub connector_id: String,
    pub provider: CloudProvider,
    /// The configured selector, shown verbatim: AWS profile, Azure
    /// subscription, or GCP project.
    pub account_label: String,
    pub location: String,
    pub access: CloudAccessState,
    /// Empty when access is Confirmed. Otherwise the operator's remedy: a
    /// copyable login command, or the name of the missing permission.
    pub remedy: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudResource {
    pub provider: CloudProvider,
    pub environment_id: String,
    pub resource_type: CloudResourceType,
    pub id: String,
    pub name: String,
    pub location: String,
    pub health: CloudHealthState,
    /// The provider's own status string, unmodified.
    pub status_detail: String,
    pub console_url: String,
    pub cli_command: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cloud_enums_serialize_to_their_documented_wire_values() {
        assert_eq!(
            serde_json::to_value(CloudProvider::Aws).unwrap(),
            json!("aws")
        );
        assert_eq!(
            serde_json::to_value(CloudProvider::Azure).unwrap(),
            json!("azure")
        );
        assert_eq!(
            serde_json::to_value(CloudProvider::Gcp).unwrap(),
            json!("gcp")
        );

        assert_eq!(
            serde_json::to_value(CloudResourceType::KubernetesCluster).unwrap(),
            json!("kubernetes_cluster")
        );
        assert_eq!(
            serde_json::to_value(CloudResourceType::ComputeInstance).unwrap(),
            json!("compute_instance")
        );

        assert_eq!(
            serde_json::to_value(CloudHealthState::Healthy).unwrap(),
            json!("healthy")
        );
        assert_eq!(
            serde_json::to_value(CloudHealthState::Degraded).unwrap(),
            json!("degraded")
        );
        assert_eq!(
            serde_json::to_value(CloudHealthState::Unavailable).unwrap(),
            json!("unavailable")
        );
        assert_eq!(
            serde_json::to_value(CloudHealthState::Unknown).unwrap(),
            json!("unknown")
        );

        assert_eq!(
            serde_json::to_value(CloudAccessState::Confirmed).unwrap(),
            json!("confirmed")
        );
        assert_eq!(
            serde_json::to_value(CloudAccessState::NoCredential).unwrap(),
            json!("no_credential")
        );
        assert_eq!(
            serde_json::to_value(CloudAccessState::SessionExpired).unwrap(),
            json!("session_expired")
        );
        assert_eq!(
            serde_json::to_value(CloudAccessState::PermissionDenied).unwrap(),
            json!("permission_denied")
        );
        assert_eq!(
            serde_json::to_value(CloudAccessState::Unavailable).unwrap(),
            json!("unavailable")
        );
    }
}
