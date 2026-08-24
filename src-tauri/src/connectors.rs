use crate::kubernetes::KUBERNETES_CONNECTOR_KIND;
use chrono::Utc;
use keyring::Entry;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thalassa_connectors::{ConnectorCapability, ConnectorManifest};
use thalassa_domain::ActionRiskClass;
use uuid::Uuid;

pub const FIXTURE_CONNECTOR_KIND: &str = "fixture";
const LOG_HISTORY_LIMIT: i64 = 25;
const CONNECTION_TIMEOUT_MS: u64 = 1_000;
const CONNECTION_MAX_ATTEMPTS: usize = 3;
const CONNECTION_RETRY_BACKOFF_MS: u64 = 100;
const KEYRING_SERVICE: &str = "io.thalassaops.connector";

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("credential store error: {0}")]
    Credential(String),
    #[error("connector not found")]
    NotFound,
    #[error("connector is disabled")]
    Disabled,
}

pub trait CredentialStore: Send + Sync {
    fn set(&self, reference: &str, secret: &str) -> Result<(), ConnectorError>;
    fn has(&self, reference: &str) -> Result<bool, ConnectorError>;
    fn delete(&self, reference: &str) -> Result<(), ConnectorError>;
}

#[derive(Default)]
pub struct OsKeychainCredentialStore;
impl CredentialStore for OsKeychainCredentialStore {
    fn set(&self, reference: &str, secret: &str) -> Result<(), ConnectorError> {
        Entry::new(KEYRING_SERVICE, reference)
            .map_err(|error| ConnectorError::Credential(error.to_string()))?
            .set_password(secret)
            .map_err(|error| ConnectorError::Credential(error.to_string()))
    }
    fn has(&self, reference: &str) -> Result<bool, ConnectorError> {
        match Entry::new(KEYRING_SERVICE, reference)
            .map_err(|error| ConnectorError::Credential(error.to_string()))?
            .get_password()
        {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(ConnectorError::Credential(error.to_string())),
        }
    }
    fn delete(&self, reference: &str) -> Result<(), ConnectorError> {
        match Entry::new(KEYRING_SERVICE, reference)
            .map_err(|error| ConnectorError::Credential(error.to_string()))?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(ConnectorError::Credential(error.to_string())),
        }
    }
}

#[derive(Default)]
pub struct InMemoryCredentialStore(Mutex<HashMap<String, String>>);
impl CredentialStore for InMemoryCredentialStore {
    fn set(&self, reference: &str, secret: &str) -> Result<(), ConnectorError> {
        self.0
            .lock()
            .expect("credential test store lock")
            .insert(reference.into(), secret.into());
        Ok(())
    }
    fn has(&self, reference: &str) -> Result<bool, ConnectorError> {
        Ok(self
            .0
            .lock()
            .expect("credential test store lock")
            .contains_key(reference))
    }
    fn delete(&self, reference: &str) -> Result<(), ConnectorError> {
        self.0
            .lock()
            .expect("credential test store lock")
            .remove(reference);
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AddConnectorRequest {
    pub kind: String,
    pub display_name: String,
    #[serde(default)]
    pub config_metadata: Value,
    #[serde(default)]
    pub credential_value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectorIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectorSummary {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub config_metadata: Value,
    pub credential_configured: bool,
    pub health_state: String,
    pub last_checked_at: Option<String>,
    pub last_successful_sync_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectorLogEntry {
    pub id: String,
    pub checked_at: String,
    pub outcome: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectorDiagnostics {
    pub connector: ConnectorSummary,
    pub manifest: ConnectorManifest,
    pub logs: Vec<ConnectorLogEntry>,
}

pub fn fixture_manifest() -> ConnectorManifest {
    ConnectorManifest::new(FIXTURE_CONNECTOR_KIND, "Fixture connector", "0.1.0")
        .with_capability(ConnectorCapability::read(
            "fixture.status.read",
            ["fixture"],
        ))
        .with_capability(ConnectorCapability::act(
            "fixture.test",
            ["fixture"],
            ActionRiskClass::ReadOnly,
        ))
}

/// The installed connector is read-only. Runtime RBAC discovery narrows the availability
/// reported for each of these declared resource kinds.
pub fn kubernetes_manifest() -> ConnectorManifest {
    ConnectorManifest::new(KUBERNETES_CONNECTOR_KIND, "Kubernetes", "0.1.0")
        .with_capability(ConnectorCapability::read(
            "kubernetes.resources.read",
            [
                "Node",
                "Namespace",
                "Deployment",
                "StatefulSet",
                "DaemonSet",
                "Service",
                "Pod",
            ],
        ))
        .with_capability(ConnectorCapability::read(
            "kubernetes.events.read",
            ["Event"],
        ))
        .with_capability(ConnectorCapability::read(
            "kubernetes.pod_logs.read",
            ["Pod"],
        ))
}

pub fn add(
    connection: &Connection,
    store: &dyn CredentialStore,
    request: AddConnectorRequest,
) -> Result<ConnectorSummary, ConnectorError> {
    let id = Uuid::new_v4().to_string();
    let credential_reference = request
        .credential_value
        .as_ref()
        .map(|_| format!("connector/{id}"));
    if let (Some(reference), Some(secret)) = (&credential_reference, &request.credential_value) {
        store.set(reference, secret)?;
    }
    let result = connection.execute(
        "INSERT INTO connector_instances (id, kind, display_name, enabled, config_metadata_json, credential_reference, health_state, created_at) VALUES (?1, ?2, ?3, 1, ?4, ?5, 'unavailable', ?6)",
        params![id, request.kind, request.display_name, serde_json::to_string(&request.config_metadata)?, credential_reference, Utc::now().to_rfc3339()],
    );
    if let Err(error) = result {
        if let Some(reference) = credential_reference {
            let _ = store.delete(&reference);
        }
        return Err(error.into());
    }
    get(connection, store, &id)?.ok_or(ConnectorError::NotFound)
}

pub fn list(
    connection: &Connection,
    store: &dyn CredentialStore,
) -> Result<Vec<ConnectorSummary>, ConnectorError> {
    let mut statement = connection.prepare("SELECT id, kind, display_name, enabled, config_metadata_json, credential_reference, health_state, last_checked_at, last_successful_sync_at FROM connector_instances ORDER BY display_name")?;
    let results = statement
        .query_map([], |row| row_to_summary(row, store))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(ConnectorError::from)?;
    Ok(results)
}

pub fn get(
    connection: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<Option<ConnectorSummary>, ConnectorError> {
    connection.query_row("SELECT id, kind, display_name, enabled, config_metadata_json, credential_reference, health_state, last_checked_at, last_successful_sync_at FROM connector_instances WHERE id = ?1", [id], |row| row_to_summary(row, store)).optional().map_err(Into::into)
}

pub fn set_enabled(
    connection: &Connection,
    store: &dyn CredentialStore,
    id: &str,
    enabled: bool,
) -> Result<ConnectorSummary, ConnectorError> {
    if connection.execute(
        "UPDATE connector_instances SET enabled = ?2 WHERE id = ?1",
        params![id, enabled],
    )? == 0
    {
        return Err(ConnectorError::NotFound);
    }
    get(connection, store, id)?.ok_or(ConnectorError::NotFound)
}

pub fn remove(
    connection: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<(), ConnectorError> {
    let reference: Option<String> = connection
        .query_row(
            "SELECT credential_reference FROM connector_instances WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ConnectorError::NotFound)?;
    connection.execute("DELETE FROM connector_instances WHERE id = ?1", [id])?;
    if let Some(reference) = reference {
        store.delete(&reference)?;
    }
    Ok(())
}

pub fn diagnose(
    connection: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<ConnectorDiagnostics, ConnectorError> {
    let connector = get(connection, store, id)?.ok_or(ConnectorError::NotFound)?;
    let manifest = manifest_for(&connector.kind);
    let mut statement = connection.prepare("SELECT id, checked_at, outcome, message FROM connector_test_logs WHERE connector_id = ?1 ORDER BY checked_at DESC LIMIT ?2")?;
    let logs = statement
        .query_map(params![id, LOG_HISTORY_LIMIT], |row| {
            Ok(ConnectorLogEntry {
                id: row.get(0)?,
                checked_at: row.get(1)?,
                outcome: row.get(2)?,
                message: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ConnectorDiagnostics {
        connector,
        manifest,
        logs,
    })
}

pub fn test_connection(
    connection: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<ConnectorSummary, ConnectorError> {
    let connector = get(connection, store, id)?.ok_or(ConnectorError::NotFound)?;
    if !connector.enabled {
        return Err(ConnectorError::Disabled);
    }
    let result = run_connection_test(&connector);
    let log_message = format!("{} Attempts: {}.", result.message, result.attempts);
    let checked_at = Utc::now().to_rfc3339();
    connection.execute("UPDATE connector_instances SET health_state = ?2, last_checked_at = ?3, last_successful_sync_at = CASE WHEN ?2 = 'healthy' THEN ?3 ELSE last_successful_sync_at END WHERE id = ?1", params![id, result.outcome, checked_at])?;
    connection.execute("INSERT INTO connector_test_logs (id, connector_id, checked_at, outcome, message) VALUES (?1, ?2, ?3, ?4, ?5)", params![Uuid::new_v4().to_string(), id, checked_at, result.outcome, log_message])?;
    connection.execute("DELETE FROM connector_test_logs WHERE id IN (SELECT id FROM connector_test_logs WHERE connector_id = ?1 ORDER BY checked_at DESC LIMIT -1 OFFSET ?2)", params![id, LOG_HISTORY_LIMIT])?;
    get(connection, store, id)?.ok_or(ConnectorError::NotFound)
}

#[derive(Debug)]
struct ConnectionTestResult {
    outcome: &'static str,
    message: String,
    attempts: usize,
}

#[derive(Debug)]
struct ConnectionTestPolicy {
    timeout: Duration,
    retry_backoff: Duration,
}

impl ConnectionTestPolicy {
    fn for_connector(connector: &ConnectorSummary) -> Self {
        Self {
            timeout: fixture_duration(connector, "fixture_timeout_ms", CONNECTION_TIMEOUT_MS),
            retry_backoff: fixture_duration(
                connector,
                "fixture_retry_backoff_ms",
                CONNECTION_RETRY_BACKOFF_MS,
            ),
        }
    }
}

fn fixture_duration(connector: &ConnectorSummary, key: &str, default_ms: u64) -> Duration {
    let configured_ms = connector
        .config_metadata
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(default_ms);
    Duration::from_millis(configured_ms.clamp(1, default_ms))
}

fn run_connection_test(connector: &ConnectorSummary) -> ConnectionTestResult {
    let policy = ConnectionTestPolicy::for_connector(connector);
    let mut last_failure = String::new();

    for attempt in 1..=CONNECTION_MAX_ATTEMPTS {
        match fixture_test(connector, attempt, policy.timeout) {
            Ok((outcome, message)) => {
                return ConnectionTestResult {
                    outcome,
                    message: format!(
                        "{message} Connection test completed after {attempt} attempts."
                    ),
                    attempts: attempt,
                };
            }
            Err(message) => {
                last_failure = message;
                if attempt < CONNECTION_MAX_ATTEMPTS {
                    std::thread::sleep(policy.retry_backoff);
                }
            }
        }
    }

    ConnectionTestResult {
        outcome: "unavailable",
        message: format!(
            "Fixture connection failed after {CONNECTION_MAX_ATTEMPTS} attempts: {last_failure}"
        ),
        attempts: CONNECTION_MAX_ATTEMPTS,
    }
}

fn fixture_test(
    connector: &ConnectorSummary,
    attempt: usize,
    timeout: Duration,
) -> Result<(&'static str, String), String> {
    let failures_before_recovery = connector
        .config_metadata
        .get("fails_then_recovers")
        .and_then(Value::as_u64)
        .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
        .unwrap_or(0);
    if attempt <= failures_before_recovery {
        return Err("Fixture connection failed.".into());
    }

    let behavior = connector
        .config_metadata
        .get("fixture_health")
        .and_then(Value::as_str)
        .unwrap_or("healthy");
    match behavior {
        "healthy" => Ok(("healthy", "Fixture connection succeeded.".into())),
        "warning" => Ok((
            "warning",
            "Fixture connection completed with a warning.".into(),
        )),
        "degraded" => Ok(("degraded", "Fixture connection is degraded.".into())),
        "timeout" => {
            std::thread::sleep(timeout);
            Err(format!(
                "Fixture connection timed out after {}ms.",
                timeout.as_millis()
            ))
        }
        _ => Err("Fixture connection failed.".into()),
    }
}

fn manifest_for(kind: &str) -> ConnectorManifest {
    if kind == FIXTURE_CONNECTOR_KIND {
        fixture_manifest()
    } else if kind == KUBERNETES_CONNECTOR_KIND {
        kubernetes_manifest()
    } else {
        ConnectorManifest::new(kind, kind, "unavailable")
    }
}
fn row_to_summary(
    row: &rusqlite::Row<'_>,
    store: &dyn CredentialStore,
) -> rusqlite::Result<ConnectorSummary> {
    let reference: Option<String> = row.get(5)?;
    let credential_configured = reference
        .as_deref()
        .map(|value| {
            store
                .has(value)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
        })
        .transpose()?
        .unwrap_or(false);
    Ok(ConnectorSummary {
        id: row.get(0)?,
        kind: row.get(1)?,
        display_name: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        config_metadata: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        credential_configured,
        health_state: row.get(6)?,
        last_checked_at: row.get(7)?,
        last_successful_sync_at: row.get(8)?,
    })
}

pub type SharedCredentialStore = Arc<dyn CredentialStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn fixture(config_metadata: Value) -> ConnectorSummary {
        ConnectorSummary {
            id: "fixture-id".into(),
            kind: FIXTURE_CONNECTOR_KIND.into(),
            display_name: "Fixture".into(),
            enabled: true,
            config_metadata,
            credential_configured: false,
            health_state: "unavailable".into(),
            last_checked_at: None,
            last_successful_sync_at: None,
        }
    }

    #[test]
    fn fixture_connection_retries_until_a_transient_failure_recovers() {
        let started = Instant::now();
        let result = run_connection_test(&fixture(json!({
            "fails_then_recovers": 2,
            "fixture_retry_backoff_ms": 5,
        })));

        assert_eq!(result.outcome, "healthy");
        assert_eq!(result.attempts, 3);
        assert!(started.elapsed() >= Duration::from_millis(10));
    }

    #[test]
    fn fixture_connection_stops_after_the_configured_attempt_limit() {
        let started = Instant::now();
        let result = run_connection_test(&fixture(json!({
            "fixture_health": "timeout",
            "fixture_timeout_ms": 1,
            "fixture_retry_backoff_ms": 1,
        })));

        assert_eq!(result.outcome, "unavailable");
        assert_eq!(result.attempts, CONNECTION_MAX_ATTEMPTS);
        assert!(started.elapsed() >= Duration::from_millis(3));
    }
}
