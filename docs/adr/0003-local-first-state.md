---
status: accepted
---

# Keep the operational state local-first

ThalassaOps will keep workspace metadata, policy versions, incident history and audit state in a local SQLite-backed Rust core, while connectors remain responsible for reaching external systems. The UI and future sync or team services consume domain contracts rather than owning state. This preserves useful operation during degraded connectivity, keeps sensitive data under local policy control, and leaves a path to shared enterprise state without coupling the first desktop shell to a cloud service.
