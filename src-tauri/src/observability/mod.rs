use serde::{Deserialize, Serialize};

pub mod client;
pub mod prometheus;
pub mod alertmanager;
pub mod grafana;

pub const PROMETHEUS_CONNECTOR_KIND: &str = "prometheus";
pub const ALERTMANAGER_CONNECTOR_KIND: &str = "alertmanager";
pub const GRAFANA_CONNECTOR_KIND: &str = "grafana";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ObservabilityAuthMode {
    None,
    Bearer,
    Basic,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ObservabilityConnectorConfig {
    pub base_url: String,
    pub auth_mode: ObservabilityAuthMode,
    pub username: Option<String>,
    // Grafana specific fields
    pub datasource_uid: Option<String>,
    pub default_dashboard_uid: Option<String>,
}
