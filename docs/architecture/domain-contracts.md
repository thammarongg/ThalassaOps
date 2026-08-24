# Sprint 1 domain contracts

The Rust workspace is split into four provider-neutral crates:

- `thalassa-domain` owns hierarchy, identity, scope, entities, and shared vocabulary.
- `thalassa-policy` owns versioned policy documents and the baseline runtime. The immutable secret guard and fail-closed verification are here so later redaction and Policy Center features cannot bypass them.
- `thalassa-ipc` owns Tauri command naming, capability descriptors, request envelopes, and the serializable error shape.
- `thalassa-connectors` owns connector capability declarations. A declaration describes what an adapter can read or do; it is not authorization.

## IPC conventions

Commands are lowercase `resource.verb` names, for example `incident.list` and `policy.evaluate`. Every command descriptor declares one capability, one permission, and a `ResourceScope`; Tauri handlers must validate all three before invoking domain code. Responses use a success value or the shared `{ code, message, details }` error shape. The React mirror lives in `ui/contracts/ipc.ts` and uses the same JSON names and enum values.

## Scope and identity

The hierarchy is Organization → Team → Workspace → Environment. A local principal has the same enterprise-compatible identity fields (`issuer`, `subject`, `provider`, and `external_id`) as a future OIDC/SSO principal; local bootstrap simply leaves issuer/provider/external ID unset. Memberships carry a role, lifecycle status, and resource scope, allowing Sprint 20 policy work and Sprint 24 multi-user access to extend the model without changing entity ownership.

## Policy runtime

`PolicyDocument` is versioned and loaded before evaluation. The baseline allows only verified Public data to hosted AI, denies when classification/redaction verification is absent, and prevents immutable Restricted data from reaching hosted AI, external integrations, or unredacted audit logs. `POLICY_AUTO` is disabled by default, requires a Mutating action, and requires an explicit resource scope. Mutable redaction rules belong in later policy documents, not in connector or UI code.
