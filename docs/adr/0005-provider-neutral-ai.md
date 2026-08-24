---
status: accepted
---

# Keep AI behind a provider-neutral contract

ThalassaOps will represent model requests, structured findings, evidence references and proposed actions through provider-neutral Rust contracts, with hosted and local providers implemented behind adapters in later sprints. Provider selection cannot grant tool or mutation authority: connector capabilities, resource scopes and policy remain the authorization boundary, and immutable Restricted data remains protected regardless of model location. This allows OpenAI, Anthropic, Gemini, Ollama, vLLM and custom endpoints to evolve without a React or domain-model rewrite.
