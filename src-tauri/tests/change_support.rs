use rusqlite::Connection;
use thalassa_domain::ResourceScope;
use thalassaops::correlation::SourceRecordStore;
use uuid::Uuid;

pub fn fixture_scope() -> ResourceScope {
    ResourceScope::workspace(Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3))
}

pub fn memory_store(scope: ResourceScope) -> SourceRecordStore {
    let connection = Connection::open_in_memory().expect("in-memory database");
    connection
        .execute_batch(include_str!("../migrations/0005_change_records.sql"))
        .expect("change-record migration applies");
    SourceRecordStore::with_connection_and_scope(connection, scope)
        .expect("source-record store opens")
}
