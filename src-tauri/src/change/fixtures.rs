//! Committed synthetic provider payloads used for deterministic change replay.

use chrono::{DateTime, Utc};
use thalassa_domain::EvidenceSourceKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChangeFixture {
    pub source: EvidenceSourceKind,
    pub path: &'static str,
    pub payload: &'static str,
}

/// Return every committed change fixture in path order.
pub fn catalog() -> Vec<ChangeFixture> {
    let mut fixtures = vec![
        ChangeFixture {
            source: EvidenceSourceKind::GitHub,
            path: "github/push.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/github/push.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::GitHub,
            path: "github/pull-request-merged.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/github/pull-request-merged.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::GitHub,
            path: "github/deployment-status.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/github/deployment-status.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::GitLab,
            path: "gitlab/push.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/gitlab/push.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::GitLab,
            path: "gitlab/merge-request-merged.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/gitlab/merge-request-merged.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::GitLab,
            path: "gitlab/pipeline-deployment.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/gitlab/pipeline-deployment.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::ArgoCd,
            path: "argocd/sync-succeeded.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/argocd/sync-succeeded.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::ArgoCd,
            path: "argocd/sync-failed.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/argocd/sync-failed.json"
            ),
        },
        ChangeFixture {
            source: EvidenceSourceKind::ArgoCd,
            path: "argocd/rollback.json",
            payload: include_str!(
                "../../../docs/superpowers/fixtures/2026-08-29-change/argocd/rollback.json"
            ),
        },
    ];
    fixtures.sort_by_key(|fixture| fixture.path);
    fixtures
}

/// Fixed clock used by all fixture replay tests and producers.
pub fn fixture_clock() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-08-28T09:00:00Z")
        .expect("fixture clock is a valid RFC3339 timestamp")
        .with_timezone(&Utc)
}
