---
status: accepted
---

# Delegate credentials, own the wire

ThalassaOps integrates with roughly twenty-five external systems across its
roadmap. Each integration will be built by a different agent in a different
sprint, so the question of how to talk to a provider must be answered once
rather than re-litigated per connector; twenty-five locally reasonable answers
would produce twenty-five shapes to maintain.

Credential acquisition is delegated to each provider's own auth library.
Signature construction, SSO session handling and token refresh are the part of
any integration where a hand-rolled implementation is both dangerous to get
subtly wrong and permanently in maintenance as providers evolve their
authentication. Everything after the credential is owned by ThalassaOps: URL
construction, the request, pagination and error handling run through a shared
adapter that guarantees GET-only reads, disabled redirects, a bounded timeout,
and failures sanitized to a status code carrying no response body. Those
guarantees are the product's auditability claim, and a provider SDK's own HTTP
stack, retry policy and error types would dilute them one connector at a time.

A full provider SDK is reserved for protocols whose types are themselves the
domain model. `kube` and `k8s-openapi` qualify: the Kubernetes API is genuinely
complex, and its types are what Sprint 6 and 7 reason about directly. Cloud
inventory reads, and the HTTP APIs of GitHub, GitLab, Jira, Slack, Argo CD and
the observability backends, are versioned JSON list calls that do not meet that
bar and belong on the shared adapter.

Provider APIs are version-pinned and backward compatible within a version, so a
provider shipping a new feature does not break an existing call; surfacing that
feature is product work under any transport choice. This decision therefore
trades no maintenance burden for a uniform, auditable egress path and a
predictable cost per new integration.
