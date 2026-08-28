//! Source adapters for the common, source-preserving Signal envelope.

use serde_json::Value;
use thalassa_domain::{CorrelationError, EvidenceSourceKind, Signal};
use thiserror::Error;

use super::source_records::{SourceRecordError, SourceRecordStore};
use super::ReplayableSignalFixture;

pub mod operational;

pub use operational::{
    normalize_alert, normalize_anomaly, normalize_health_check, normalize_operational,
    OperationalAdapter, OperationalSignalAdapter,
};

/// Typed failures returned by source adapters.  Payload details remain in the
/// local rejection/evidence path and are never copied into error strings.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SignalAdapterError {
    #[error("replay fixture failed validation")]
    Fixture(#[source] CorrelationError),
    #[error("source record failed admission")]
    Source(#[source] SourceRecordError),
    #[error("adapter source does not match the fixture source")]
    SourceMismatch,
    #[error("operational source is not supported by this adapter")]
    UnsupportedSource,
    #[error("operational source payload is malformed")]
    MalformedPayload,
    #[error("operational source payload contains an invalid number")]
    InvalidNumber,
    #[error("operational source payload contains an invalid timestamp")]
    InvalidTimestamp,
    #[error("normalized signal failed contract validation")]
    Signal(#[source] CorrelationError),
}

impl From<SourceRecordError> for SignalAdapterError {
    fn from(error: SourceRecordError) -> Self {
        Self::Source(error)
    }
}

/// Common seam implemented by every source adapter.
pub trait SignalAdapter {
    fn source_kind(&self) -> EvidenceSourceKind;

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError>;
}

/// Parse a JSON object/array while preserving all source fields in the ledger.
pub(crate) fn payload_value(fixture: &ReplayableSignalFixture) -> &Value {
    &fixture.recorded_json
}
