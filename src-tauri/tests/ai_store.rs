use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use tempfile::{tempdir, TempDir};
use thalassa_domain::{
    ContentDeclaration, FailoverPermission, ModelAttempt, ModelAttemptOutcome, ModelBudget,
    ModelFinishReason, ModelMessage, ModelRequest, ModelRole, ModelSelector, ModelUsage,
    PrincipalId, ProviderErrorReason,
};
use thalassaops::ai::store::{AiAttemptRecord, AiRequestOutcome, AiRequestStore};
use thalassaops::app::AppState;
use uuid::Uuid;

const PRINCIPAL: PrincipalId = Uuid::from_u128(0x1701);
const OTHER_PRINCIPAL: PrincipalId = Uuid::from_u128(0x1702);

fn timestamp(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 9, minute, 0).unwrap()
}

fn request(id: u128, content: &str) -> ModelRequest {
    ModelRequest {
        request_id: Uuid::from_u128(id),
        instruction: Some("answer the operator".into()),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: content.into(),
        }],
        data_class: "public".into(),
        declaration: ContentDeclaration::OperatorDeclared,
        budget: ModelBudget {
            max_input_tokens: Some(100),
            max_output_tokens: 32,
            max_cost_micros: Some(500),
        },
        timeout_ms: 1_000,
        model: ModelSelector::Explicit {
            provider_id: "hosted".into(),
            model_id: "model".into(),
        },
        failover: FailoverPermission::Permitted,
    }
}

fn attempt(
    ordinal: u32,
    provider_id: &str,
    outcome: ModelAttemptOutcome,
    input_tokens: u64,
    output_tokens: u64,
) -> AiAttemptRecord {
    AiAttemptRecord {
        ordinal,
        attempt: ModelAttempt {
            provider_id: provider_id.into(),
            model_id: "model".into(),
            outcome,
        },
        usage: ModelUsage {
            input_tokens,
            output_tokens,
            cost_micros: Some(3),
        },
    }
}

fn completed_outcome(started_at: DateTime<Utc>, finished_at: DateTime<Utc>) -> AiRequestOutcome {
    AiRequestOutcome {
        finish: Some(ModelFinishReason::Complete),
        error: None,
        started_at,
        finished_at: Some(finished_at),
    }
}

struct Fixture {
    _directory: TempDir,
    database_path: std::path::PathBuf,
    store: AiRequestStore,
}

fn fixture() -> Fixture {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("ai.sqlite3");
    AppState::open(&database_path).unwrap();
    let store = AiRequestStore::open(&database_path).unwrap();
    Fixture {
        _directory: directory,
        database_path,
        store,
    }
}

#[test]
fn completed_request_writes_one_request_and_one_attempt() {
    let fixture = fixture();
    let model_request = request(0x1001, "completed prompt marker");
    fixture
        .store
        .record_request(
            PRINCIPAL,
            &model_request,
            19,
            completed_outcome(timestamp(0), timestamp(1)),
            &[attempt(0, "hosted", ModelAttemptOutcome::Answered, 12, 7)],
        )
        .unwrap();

    let connection = Connection::open(fixture.database_path).unwrap();
    let request_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
        .unwrap();
    let attempt_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM ai_request_attempts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(request_count, 1);
    assert_eq!(attempt_count, 1);
}

#[test]
fn failed_over_request_writes_attempts_in_ordinal_order() {
    let fixture = fixture();
    let model_request = request(0x1002, "failover prompt marker");
    fixture
        .store
        .record_request(
            PRINCIPAL,
            &model_request,
            19,
            completed_outcome(timestamp(2), timestamp(3)),
            &[
                attempt(
                    0,
                    "hosted",
                    ModelAttemptOutcome::Failed(ProviderErrorReason::RateLimited),
                    11,
                    0,
                ),
                attempt(1, "local", ModelAttemptOutcome::Answered, 12, 7),
            ],
        )
        .unwrap();

    let connection = Connection::open(fixture.database_path).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT ordinal, provider_id FROM ai_request_attempts
             ORDER BY ordinal",
        )
        .unwrap();
    let rows: Vec<(i64, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, vec![(0, "hosted".into()), (1, "local".into())]);
}

#[test]
fn prompt_and_completion_content_never_enters_either_table() {
    let fixture = fixture();
    let marker = "recognisable-prompt-content-never-persisted";
    let model_request = request(0x1003, marker);
    fixture
        .store
        .record_request(
            PRINCIPAL,
            &model_request,
            19,
            completed_outcome(timestamp(4), timestamp(5)),
            &[attempt(0, "hosted", ModelAttemptOutcome::Answered, 12, 7)],
        )
        .unwrap();

    let connection = Connection::open(fixture.database_path).unwrap();
    let request_values: (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = connection
        .query_row(
            "SELECT id, principal_id, data_class, declaration, budget_json,
                    finish_reason, error_reason
             FROM ai_requests",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .unwrap();
    let attempt_values: (String, i64, String, String, String, i64, i64, i64) = connection
        .query_row(
            "SELECT request_id, ordinal, provider_id, model_id, outcome_json,
                    input_tokens, output_tokens, cost_micros
             FROM ai_request_attempts",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .unwrap();

    let stored_text = format!("{request_values:?}{attempt_values:?}");
    assert!(!stored_text.contains(marker));
}

#[test]
fn policy_version_is_stored_on_the_request_row() {
    let fixture = fixture();
    let model_request = request(0x1004, "policy version marker");
    fixture
        .store
        .record_request(
            PRINCIPAL,
            &model_request,
            23,
            completed_outcome(timestamp(6), timestamp(7)),
            &[attempt(0, "hosted", ModelAttemptOutcome::Answered, 12, 7)],
        )
        .unwrap();

    let connection = Connection::open(fixture.database_path).unwrap();
    let policy_version: i64 = connection
        .query_row(
            "SELECT policy_version FROM ai_requests WHERE id = ?1",
            [model_request.request_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(policy_version, 23);
}

#[test]
fn request_window_for_a_principal_is_newest_first() {
    let fixture = fixture();
    for (id, principal_id, minute) in [
        (0x1005, PRINCIPAL, 8),
        (0x1006, PRINCIPAL, 10),
        (0x1007, OTHER_PRINCIPAL, 11),
        (0x1008, PRINCIPAL, 9),
    ] {
        let model_request = request(id, "window prompt marker");
        fixture
            .store
            .record_request(
                principal_id,
                &model_request,
                19,
                completed_outcome(timestamp(minute), timestamp(minute)),
                &[attempt(0, "hosted", ModelAttemptOutcome::Answered, 12, 7)],
            )
            .unwrap();
    }

    let records = fixture.store.list_for_principal(PRINCIPAL, 10).unwrap();
    let request_ids: Vec<_> = records.iter().map(|record| record.request_id).collect();
    assert_eq!(
        request_ids,
        vec![
            Uuid::from_u128(0x1006),
            Uuid::from_u128(0x1008),
            Uuid::from_u128(0x1005)
        ]
    );
}
