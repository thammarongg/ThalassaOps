//! Workspace-scoped evidence lookup for the topology IPC surface.

use std::collections::BTreeMap;

use thalassa_domain::{
    EvidenceRef, ResourceScope, TopologyError, TopologyEvidenceRequest, TopologySnapshot,
};

/// Evidence admitted by one topology snapshot.
#[derive(Clone, Debug)]
pub(crate) struct TopologyEvidenceStore {
    evidence: BTreeMap<String, EvidenceRef>,
}

impl TopologyEvidenceStore {
    pub(crate) fn from_snapshot(snapshot: &TopologySnapshot) -> Self {
        Self {
            evidence: snapshot
                .evidence
                .iter()
                .cloned()
                .map(|evidence| (evidence.id.clone(), evidence))
                .collect(),
        }
    }

    /// Resolve all requested IDs before returning any evidence.
    pub(crate) fn get_for_scope(
        &self,
        request: &TopologyEvidenceRequest,
        scope: &ResourceScope,
    ) -> Result<Vec<EvidenceRef>, TopologyError> {
        request.validate()?;

        let mut resolved = Vec::with_capacity(request.evidence_ids.len());
        for evidence_id in &request.evidence_ids {
            let Some(evidence) = self.evidence.get(evidence_id) else {
                return Err(TopologyError::EvidenceMissing);
            };
            if !scope.contains(&evidence.scope) {
                return Err(TopologyError::ScopeDenied);
            }
            if !evidence.redaction.classification_verified
                || !evidence.redaction.redaction_verified
                || (evidence.redaction.unparsed && evidence.redaction.masked)
            {
                return Err(TopologyError::EvidenceUnverified);
            }
            resolved.push(evidence.clone());
        }
        Ok(resolved)
    }
}
