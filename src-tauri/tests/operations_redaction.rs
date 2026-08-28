// SPDX-License-Identifier: Apache-2.0

use thalassaops::operations::{fixture_catalog, fixture_time, OperationsAggregator};

#[test]
fn sensitive_cloud_identifiers_are_not_serialized_in_console_evidence() {
    let mut catalog = fixture_catalog();
    let evidence = catalog
        .evidence
        .first_mut()
        .expect("fixture catalog should contain evidence");
    evidence.endpoint = "https://cloud.example/subscriptions/subscription-identifier-123".into();
    evidence.query = Some("account_id=account-identifier-456&cursor=cursor-identifier-789".into());
    evidence.excerpt = "cursor-identifier-789 was returned by account-identifier-456".into();
    evidence.native_url =
        Some("https://cloud.example/subscriptions/subscription-identifier-123".into());

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("sensitive fields should be replaced by safe fallback evidence");
    let serialized = serde_json::to_string(&snapshot).expect("snapshot should serialize");

    for secret in [
        "subscription-identifier-123",
        "account-identifier-456",
        "cursor-identifier-789",
    ] {
        assert!(!serialized.contains(secret), "{secret} leaked");
    }
}
