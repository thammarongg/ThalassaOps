---
status: accepted
---

# Treat operational policies as versioned product data

ThalassaOps will ship safe default presets for Severity, Incident Status, Data Redaction and Action behavior, while allowing Organization, Team, Workspace and Environment policies to customize behavior through Policy Center. Policies will be versioned, auditable, testable and reversible; immutable secret-protection rules and fail-closed external data transmission cannot be weakened by normal configuration. The policy runtime and immutable safety guard are foundational services delivered before the full Policy Center administration UI, so later features such as context redaction never hard-code mutable policy. This balances immediate usability with enterprise governance and avoids hard-coding business rules into the application.

Action risk class (`READ-ONLY`, `MUTATING`, `BLOCKED`, `REQUIRES APPROVAL`) is separate from execution mode (`OBSERVE`, `RECOMMEND`, `APPROVAL`, `POLICY_AUTO`). `POLICY_AUTO` is disabled by default and can only be enabled for narrowly scoped, reversible mutations by explicit policy; the AI model is never the authorization layer.

## Addendum

The policy document now carries per-destination data-class lists for external integration and audit log.
