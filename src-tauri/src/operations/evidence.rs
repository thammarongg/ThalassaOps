//! Workspace-scoped lookup for evidence already admitted to an Operations
//! Console snapshot.
//!
//! This module deliberately accepts evidence IDs only.  It never accepts a
//! provider URL, query or connector selector from the UI and never performs an
//! external request.

use super::model::{ConsoleEvidenceId, EvidenceRef, OperationsSnapshot, ResourceScope};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EvidenceError {
    #[error("evidence request must include at least one ID")]
    EmptyRequest,
    #[error("evidence request contains a duplicate ID")]
    DuplicateId,
    #[error("evidence ID was not emitted by the snapshot")]
    UnknownId,
    #[error("evidence is outside the current workspace")]
    CrossScope,
    #[error("evidence redaction is not verified")]
    Unverified,
}

#[derive(Clone, Debug)]
pub struct EvidenceStore {
    evidence: BTreeMap<ConsoleEvidenceId, EvidenceRef>,
}

impl EvidenceStore {
    /// Admit the evidence set from one already validated snapshot.
    pub fn from_snapshot(snapshot: &OperationsSnapshot) -> Self {
        let evidence = snapshot
            .evidence
            .iter()
            .cloned()
            .map(|item| (item.id.clone(), item))
            .collect();
        Self { evidence }
    }

    /// Resolve backend-issued IDs inside a workspace scope.
    pub fn get_for_scope(
        &self,
        ids: &[ConsoleEvidenceId],
        workspace_scope: &ResourceScope,
    ) -> Result<Vec<EvidenceRef>, EvidenceError> {
        if ids.is_empty() {
            return Err(EvidenceError::EmptyRequest);
        }

        let mut seen = BTreeSet::new();
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            if id.trim().is_empty() || !seen.insert(id) {
                return Err(EvidenceError::DuplicateId);
            }
            let evidence = self.evidence.get(id).ok_or(EvidenceError::UnknownId)?;
            if !workspace_scope.contains(&evidence.scope) {
                return Err(EvidenceError::CrossScope);
            }
            if !evidence.redaction.classification_verified || !evidence.redaction.redaction_verified
            {
                return Err(EvidenceError::Unverified);
            }
            result.push(evidence.clone());
        }
        Ok(result)
    }
}
