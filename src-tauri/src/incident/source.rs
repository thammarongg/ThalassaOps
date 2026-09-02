//! Explicit trigger resolution.
//!
//! Every source-backed trigger is resolved from the deterministic Sprint 13
//! replay projection before an incident write begins.  Resolution performs no
//! provider request, reads no credential and copies no source payload into the
//! incident: it returns only identity, scope, observation time, the immutable
//! source-record digest and the evidence references the local ledger already
//! admitted.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use thalassa_domain::{
    ConsoleEvidenceId, EvidenceSourceKind, IncidentReport, IncidentSourceKind, IncidentTrigger,
    IncidentTriggerId, ResourceScope, Signal, SignalId, SignalKind,
};

use crate::correlation::adapters::{normalize_operational, normalize_security};
use crate::correlation::{correlation_fixture_catalog, SourceRecordStore};

use super::service::IncidentServiceError;

/// One trigger whose provenance has been validated against local state.  The
/// caller assigns the trigger identifier, which keeps identity allocation in
/// the service that also owns the request context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedIncidentTrigger {
    pub source_kind: IncidentSourceKind,
    pub source_id: String,
    pub source_record_digest: Option<String>,
    pub scope: ResourceScope,
    pub observed_at: DateTime<Utc>,
    pub signal_id: Option<SignalId>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub report: Option<IncidentReport>,
}

impl ResolvedIncidentTrigger {
    /// Completes the canonical trigger with its assigned identifier.
    pub fn into_trigger(self, id: IncidentTriggerId) -> IncidentTrigger {
        IncidentTrigger {
            id,
            source_kind: self.source_kind,
            source_id: self.source_id,
            source_record_digest: self.source_record_digest,
            scope: self.scope,
            observed_at: self.observed_at,
            signal_id: self.signal_id,
            evidence_ids: self.evidence_ids,
            report: self.report,
        }
    }
}

/// Returns true only for the exact source-kind to signal-kind pairs Sprint 15
/// supports.  User and manual reports are not signal backed and never match.
pub fn source_kind_matches_signal(kind: IncidentSourceKind, signal: &Signal) -> bool {
    matches!(
        (kind, signal.kind),
        (IncidentSourceKind::Alert, SignalKind::Alert)
            | (IncidentSourceKind::Anomaly, SignalKind::Anomaly)
            | (
                IncidentSourceKind::ScheduledHealthCheck,
                SignalKind::HealthCheck
            )
            | (
                IncidentSourceKind::VulnerabilityFinding,
                SignalKind::SecurityFinding
            )
    )
}

/// Normalizes the committed replay catalog into Signals for the given scope.
///
/// This mirrors the read-only correlation workspace: it retains source records
/// in the caller's ledger and never writes an incident row.
pub fn replay_incident_signals(
    scope: &ResourceScope,
    records: &mut SourceRecordStore,
) -> Result<Vec<Signal>, IncidentServiceError> {
    let mut catalog = correlation_fixture_catalog();
    for fixture in &mut catalog.fixtures {
        fixture.scope = scope.clone();
        for evidence in &mut fixture.evidence {
            evidence.scope = scope.clone();
        }
    }
    catalog.validate().map_err(IncidentServiceError::Contract)?;

    let mut signals = Vec::new();
    for fixture in &catalog.fixtures {
        let normalized = match fixture.source_kind {
            EvidenceSourceKind::Alertmanager
            | EvidenceSourceKind::Prometheus
            | EvidenceSourceKind::HealthCheck => normalize_operational(fixture, records),
            EvidenceSourceKind::Trivy
            | EvidenceSourceKind::Falco
            | EvidenceSourceKind::Kyverno
            | EvidenceSourceKind::OpaGatekeeper => normalize_security(fixture, records),
            // A source with no Sprint 13 adapter contributes no trigger; it is
            // not an incident-creation failure.
            _ => continue,
        };
        signals.extend(normalized.map_err(IncidentServiceError::Replay)?);
    }
    Ok(signals)
}

/// Deterministic index of the local signals a responder may cite as triggers.
#[derive(Clone, Debug, Default)]
pub struct IncidentSourceResolver {
    signals: BTreeMap<SignalId, Signal>,
}

impl IncidentSourceResolver {
    /// Indexes already normalized signals by identifier.
    pub fn from_signals(signals: Vec<Signal>) -> Result<Self, IncidentServiceError> {
        let mut indexed = BTreeMap::new();
        for signal in signals {
            signal.validate().map_err(IncidentServiceError::Contract)?;
            if indexed.insert(signal.id, signal).is_some() {
                return Err(IncidentServiceError::UnresolvableSource);
            }
        }
        Ok(Self { signals: indexed })
    }

    /// Builds the resolver from the committed deterministic replay catalog.
    pub fn replay(
        scope: &ResourceScope,
        records: &mut SourceRecordStore,
    ) -> Result<Self, IncidentServiceError> {
        Self::from_signals(replay_incident_signals(scope, records)?)
    }

    /// Lists the resolvable signal identifiers for one source kind, in
    /// identifier order.  Sprint 16 clients choose from exactly this set.
    pub fn signal_ids(&self, kind: IncidentSourceKind) -> Vec<SignalId> {
        self.signals
            .values()
            .filter(|signal| source_kind_matches_signal(kind, signal))
            .map(|signal| signal.id)
            .collect()
    }

    /// Resolves one source-backed trigger inside the caller's workspace.
    pub fn resolve(
        &self,
        kind: IncidentSourceKind,
        source_id: &str,
        workspace_scope: &ResourceScope,
    ) -> Result<ResolvedIncidentTrigger, IncidentServiceError> {
        if matches!(
            kind,
            IncidentSourceKind::UserReport | IncidentSourceKind::ManualReport
        ) {
            return Err(IncidentServiceError::SourceKindMismatch);
        }
        let signal_id =
            uuid::Uuid::parse_str(source_id).map_err(|_| IncidentServiceError::UnknownSource)?;
        let signal = self
            .signals
            .get(&signal_id)
            .ok_or(IncidentServiceError::UnknownSource)?;
        if !source_kind_matches_signal(kind, signal) {
            return Err(IncidentServiceError::SourceKindMismatch);
        }
        if !workspace_scope.contains(&signal.scope) {
            return Err(IncidentServiceError::ScopeMismatch);
        }
        // Observation time is source data.  The ingest time is the only
        // permitted stand-in; the command clock never becomes a source
        // observation time.
        let observed_at = signal
            .observed_at
            .as_deref()
            .or(signal.ingested_at.as_deref())
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .ok_or(IncidentServiceError::UnresolvableSource)?;

        let mut evidence_ids = signal.evidence_ids.clone();
        evidence_ids.extend(signal.source_record.evidence_ids.iter().cloned());
        evidence_ids.sort();
        evidence_ids.dedup();
        if evidence_ids.is_empty() {
            return Err(IncidentServiceError::EvidenceMissing);
        }

        Ok(ResolvedIncidentTrigger {
            source_kind: kind,
            source_id: signal.id.to_string(),
            source_record_digest: Some(signal.source_record.content_digest.clone()),
            scope: signal.scope.clone(),
            observed_at,
            signal_id: Some(signal.id),
            evidence_ids,
            report: None,
        })
    }
}
