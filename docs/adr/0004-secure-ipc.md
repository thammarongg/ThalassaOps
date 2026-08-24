---
status: accepted
---

# Make IPC capability-scoped and fail closed

Every Rust-to-React command will use a stable lowercase `resource.verb` name and declare its capability, permission and resource scope. The Rust handler is the authorization boundary: Tauri registration, membership scope and policy checks must pass before domain work runs, and failures use one serializable error shape. This prevents UI affordances, connector declarations or AI proposals from becoming implicit authorization and gives React a provider-neutral contract it can evolve against.
