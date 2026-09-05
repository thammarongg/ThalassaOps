use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Transaction};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thalassa_domain::{
    ContentDeclaration, ModelAttempt, ModelBudget, ModelFinishReason, ModelRequest, ModelUsage,
    PrincipalId, ProviderErrorReason,
};
use thiserror::Error;
use uuid::Uuid;

const AI_REQUESTS_MIGRATION: &str = include_str!("../../migrations/0007_ai_requests.sql");

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum AiStoreError {
    #[error("AI request database operation failed: {0}")]
    Database(String),
    #[error("AI request serialization failed: {0}")]
    Serialization(String),
    #[error("stored AI request data is not valid {0}")]
    Corruption(String),
    #[error("AI request record is invalid: {0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiAttemptRecord {
    pub ordinal: u32,
    pub attempt: ModelAttempt,
    pub usage: ModelUsage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiRequestOutcome {
    pub finish: Option<ModelFinishReason>,
    pub error: Option<ProviderErrorReason>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiRequestRecord {
    pub request_id: Uuid,
    pub principal_id: PrincipalId,
    pub data_class: String,
    pub declaration: ContentDeclaration,
    pub budget: ModelBudget,
    pub finish: Option<ModelFinishReason>,
    pub policy_version: u64,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<ProviderErrorReason>,
}

pub type StoredAiRequest = AiRequestRecord;

/// SQLite-backed AI audit store. Only request metadata and attempt accounting
/// are retained; content is accepted by `record_request` only to derive that
/// metadata and is never serialized.
#[derive(Clone, Debug)]
pub struct AiRequestStore {
    database_path: PathBuf,
}

impl AiRequestStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AiStoreError> {
        let database_path = path.as_ref().to_path_buf();
        let connection = Connection::open(&database_path).map_err(database_error)?;
        connection
            .execute_batch(AI_REQUESTS_MIGRATION)
            .map_err(database_error)?;
        Ok(Self { database_path })
    }

    pub fn record_request(
        &self,
        principal_id: PrincipalId,
        request: &ModelRequest,
        policy_version: u64,
        outcome: AiRequestOutcome,
        attempts: &[AiAttemptRecord],
    ) -> Result<(), AiStoreError> {
        if policy_version == 0 {
            return Err(AiStoreError::Invalid(
                "policy version must be greater than zero".into(),
            ));
        }
        if attempts.is_empty() {
            return Err(AiStoreError::Invalid(
                "an AI request records at least one attempt".into(),
            ));
        }
        for (expected, attempt) in attempts.iter().enumerate() {
            if usize::try_from(attempt.ordinal).ok() != Some(expected) {
                return Err(AiStoreError::Invalid(
                    "attempt ordinals must start at zero and be contiguous".into(),
                ));
            }
            if attempt.attempt.provider_id.trim().is_empty()
                || attempt.attempt.model_id.trim().is_empty()
            {
                return Err(AiStoreError::Invalid(
                    "an AI attempt names a provider and model".into(),
                ));
            }
        }

        let mut connection = Connection::open(&self.database_path).map_err(database_error)?;
        connection
            .execute_batch(AI_REQUESTS_MIGRATION)
            .map_err(database_error)?;
        let transaction = connection.transaction().map_err(database_error)?;
        insert_request(
            &transaction,
            &RequestInsert {
                principal_id,
                request,
                policy_version,
                outcome: &outcome,
            },
        )?;
        for attempt in attempts {
            insert_attempt(&transaction, request.request_id, attempt)?;
        }
        transaction.commit().map_err(database_error)
    }

    pub fn list_for_principal(
        &self,
        principal_id: PrincipalId,
        limit: usize,
    ) -> Result<Vec<AiRequestRecord>, AiStoreError> {
        let limit = i64::try_from(limit)
            .map_err(|_| AiStoreError::Invalid("request window is too large".into()))?;
        let connection = Connection::open(&self.database_path).map_err(database_error)?;
        let mut statement = connection
            .prepare(
                "SELECT id, principal_id, data_class, declaration, budget_json,
                        finish_reason, policy_version, started_at, finished_at, error_reason
                 FROM ai_requests
                 WHERE principal_id = ?1
                 ORDER BY started_at DESC, id DESC
                 LIMIT ?2",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(params![principal_id.to_string(), limit], decode_request)
            .map_err(database_error)?;
        rows.map(|row| row.map_err(database_error)).collect()
    }
}

struct RequestInsert<'a> {
    principal_id: PrincipalId,
    request: &'a ModelRequest,
    policy_version: u64,
    outcome: &'a AiRequestOutcome,
}

fn insert_request(
    transaction: &Transaction<'_>,
    record: &RequestInsert<'_>,
) -> Result<(), AiStoreError> {
    transaction
        .execute(
            "INSERT INTO ai_requests
                (id, principal_id, data_class, declaration, budget_json,
                 finish_reason, policy_version, started_at, finished_at, error_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.request.request_id.to_string(),
                record.principal_id.to_string(),
                &record.request.data_class,
                to_json(&record.request.declaration)?,
                to_json(&record.request.budget)?,
                optional_json(record.outcome.finish.as_ref())?,
                to_i64(record.policy_version)?,
                record.outcome.started_at.to_rfc3339(),
                record
                    .outcome
                    .finished_at
                    .map(|timestamp| timestamp.to_rfc3339()),
                optional_json(record.outcome.error.as_ref())?,
            ],
        )
        .map(|_| ())
        .map_err(database_error)
}

fn insert_attempt(
    transaction: &Transaction<'_>,
    request_id: Uuid,
    attempt: &AiAttemptRecord,
) -> Result<(), AiStoreError> {
    transaction
        .execute(
            "INSERT INTO ai_request_attempts
                (request_id, ordinal, provider_id, model_id, outcome_json,
                 input_tokens, output_tokens, cost_micros)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                request_id.to_string(),
                to_i64(u64::from(attempt.ordinal))?,
                attempt.attempt.provider_id,
                attempt.attempt.model_id,
                to_json(&attempt.attempt.outcome)?,
                to_i64(attempt.usage.input_tokens)?,
                to_i64(attempt.usage.output_tokens)?,
                attempt.usage.cost_micros.map(to_i64).transpose()?,
            ],
        )
        .map(|_| ())
        .map_err(database_error)
}

fn decode_request(row: &rusqlite::Row<'_>) -> Result<AiRequestRecord, rusqlite::Error> {
    let request_id: String = row.get(0)?;
    let principal_id: String = row.get(1)?;
    let data_class: String = row.get(2)?;
    let declaration: String = row.get(3)?;
    let budget: String = row.get(4)?;
    let finish: Option<String> = row.get(5)?;
    let policy_version: i64 = row.get(6)?;
    let started_at: String = row.get(7)?;
    let finished_at: Option<String> = row.get(8)?;
    let error: Option<String> = row.get(9)?;

    decode_request_values(
        &request_id,
        &principal_id,
        &data_class,
        &declaration,
        &budget,
        finish.as_deref(),
        policy_version,
        &started_at,
        finished_at.as_deref(),
        error.as_deref(),
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}

#[allow(clippy::too_many_arguments)]
fn decode_request_values(
    request_id: &str,
    principal_id: &str,
    data_class: &str,
    declaration: &str,
    budget: &str,
    finish: Option<&str>,
    policy_version: i64,
    started_at: &str,
    finished_at: Option<&str>,
    error: Option<&str>,
) -> Result<AiRequestRecord, AiStoreError> {
    Ok(AiRequestRecord {
        request_id: parse_uuid(request_id, "request id")?,
        principal_id: parse_uuid(principal_id, "principal id")?,
        data_class: data_class.into(),
        declaration: from_json(declaration, "content declaration")?,
        budget: from_json(budget, "model budget")?,
        finish: finish
            .map(|value| from_json(value, "finish reason"))
            .transpose()?,
        policy_version: u64::try_from(policy_version)
            .map_err(|_| AiStoreError::Corruption("policy version".into()))?,
        started_at: parse_timestamp(started_at)?,
        finished_at: finished_at.map(parse_timestamp).transpose()?,
        error: error
            .map(|value| from_json(value, "provider error reason"))
            .transpose()?,
    })
}

fn database_error(error: rusqlite::Error) -> AiStoreError {
    AiStoreError::Database(error.to_string())
}

fn serialization_error(error: serde_json::Error) -> AiStoreError {
    AiStoreError::Serialization(error.to_string())
}

fn to_json<T: Serialize>(value: &T) -> Result<String, AiStoreError> {
    serde_json::to_string(value).map_err(serialization_error)
}

fn optional_json<T: Serialize>(value: Option<&T>) -> Result<Option<String>, AiStoreError> {
    value.map(to_json).transpose()
}

fn from_json<T: DeserializeOwned>(value: &str, kind: &str) -> Result<T, AiStoreError> {
    serde_json::from_str(value).map_err(|_| AiStoreError::Corruption(kind.into()))
}

fn to_i64(value: u64) -> Result<i64, AiStoreError> {
    i64::try_from(value)
        .map_err(|_| AiStoreError::Invalid("value exceeds the stored integer range".into()))
}

fn parse_uuid(value: &str, kind: &str) -> Result<Uuid, AiStoreError> {
    Uuid::parse_str(value).map_err(|_| AiStoreError::Corruption(kind.into()))
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, AiStoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| AiStoreError::Corruption("timestamp".into()))
}
