use serde::{Deserialize, Serialize};

pub mod alertmanager;
pub mod client;
pub mod grafana;
pub mod masking;
pub mod prometheus;

pub const PROMETHEUS_CONNECTOR_KIND: &str = "prometheus";
pub const ALERTMANAGER_CONNECTOR_KIND: &str = "alertmanager";
pub const GRAFANA_CONNECTOR_KIND: &str = "grafana";
pub const LOKI_CONNECTOR_KIND: &str = "loki";
pub const TEMPO_CONNECTOR_KIND: &str = "tempo";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ObservabilityAuthMode {
    None,
    Bearer,
    Basic,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConnectorConfig {
    pub base_url: String,
    pub auth_mode: ObservabilityAuthMode,
    pub username: Option<String>,
    // Grafana specific fields
    pub datasource_uid: Option<String>,
    pub default_dashboard_uid: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

impl ObservabilityConnectorConfig {
    pub fn validate(&self, credential_value: Option<&str>) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err("base_url is required".into());
        }

        let url = reqwest::Url::parse(&self.base_url)
            .map_err(|e| format!("base_url is invalid: {}", e))?;

        if url.scheme() != "http" && url.scheme() != "https" {
            return Err("base_url must use http or https scheme".into());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("base_url cannot contain embedded credentials or userinfo".into());
        }
        if url.query().is_some() {
            return Err("base_url cannot contain a query string".into());
        }
        if url.fragment().is_some() {
            return Err("base_url cannot contain a fragment".into());
        }
        if url.host_str().is_none() || url.host_str().unwrap().is_empty() {
            return Err("base_url must have a host".into());
        }
        if self
            .tenant_id
            .as_deref()
            .is_some_and(|tenant_id| tenant_id.trim().is_empty())
        {
            return Err("tenant_id cannot be blank".into());
        }

        match self.auth_mode {
            ObservabilityAuthMode::None => {
                if credential_value.is_some() {
                    return Err("credentials cannot be provided for 'none' auth mode".into());
                }
                if self.username.is_some() {
                    return Err("username cannot be provided for 'none' auth mode".into());
                }
            }
            ObservabilityAuthMode::Bearer => {
                if credential_value.unwrap_or_default().trim().is_empty() {
                    return Err("credential_value is required for 'bearer' auth mode".into());
                }
                if self.username.is_some() {
                    return Err("username cannot be provided for 'bearer' auth mode".into());
                }
            }
            ObservabilityAuthMode::Basic => {
                if credential_value.unwrap_or_default().trim().is_empty() {
                    return Err("credential_value is required for 'basic' auth mode".into());
                }
                if self
                    .username
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    return Err("username is required for 'basic' auth mode".into());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_validation() {
        let mut config = ObservabilityConnectorConfig {
            base_url: "https://example.com".into(),
            auth_mode: ObservabilityAuthMode::None,
            username: None,
            datasource_uid: None,
            default_dashboard_uid: None,
            tenant_id: None,
        };

        // None mode
        assert!(config.validate(None).is_ok());
        assert!(config.validate(Some("")).is_err());
        assert!(config.validate(Some("secret")).is_err());

        // Bearer mode
        config.auth_mode = ObservabilityAuthMode::Bearer;
        assert!(config.validate(None).is_err());
        assert!(config.validate(Some("   ")).is_err()); // blank is rejected
        assert!(config.validate(Some("secret")).is_ok());

        // Basic mode
        config.auth_mode = ObservabilityAuthMode::Basic;
        assert!(config.validate(Some("secret")).is_err()); // missing username
        config.username = Some("  ".into()); // blank username
        assert!(config.validate(Some("secret")).is_err());
        config.username = Some("admin".into());
        assert!(config.validate(Some("secret")).is_ok());
    }

    #[test]
    fn allows_http_and_https_endpoints_for_any_host() {
        for base_url in [
            "http://observability.example.test:9090",
            "http://localhost:9090",
            "http://[::1]:9090",
            "https://observability.example.test:9090",
        ] {
            let config = ObservabilityConnectorConfig {
                base_url: base_url.into(),
                auth_mode: ObservabilityAuthMode::None,
                username: None,
                datasource_uid: None,
                default_dashboard_uid: None,
                tenant_id: None,
            };

            assert!(config.validate(None).is_ok(), "{base_url}");
        }
    }

    #[test]
    fn rejects_unsupported_schemes_and_url_components() {
        for base_url in [
            "ftp://observability.example.test",
            "https://user:password@observability.example.test",
            "https://observability.example.test?query=value",
            "https://observability.example.test#fragment",
        ] {
            let config = ObservabilityConnectorConfig {
                base_url: base_url.into(),
                auth_mode: ObservabilityAuthMode::None,
                username: None,
                datasource_uid: None,
                default_dashboard_uid: None,
                tenant_id: None,
            };

            assert!(config.validate(None).is_err(), "{base_url}");
        }
    }

    #[test]
    fn tenant_id_is_optional_but_blank_values_are_rejected() {
        let mut config = ObservabilityConnectorConfig {
            base_url: "https://example.com".into(),
            auth_mode: ObservabilityAuthMode::None,
            username: None,
            datasource_uid: None,
            default_dashboard_uid: None,
            tenant_id: Some("  ".into()),
        };
        assert!(config.validate(None).is_err());

        config.tenant_id = None;
        assert!(config.validate(None).is_ok());
    }
}
