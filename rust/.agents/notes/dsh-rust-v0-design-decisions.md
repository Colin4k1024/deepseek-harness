# dsh-rust v0: Design Decisions and v1 Roadmap

> Status: active | Date: 2026-08-15

## What shipped in v0

A Rust-native agent harness framework with 5 layers (L0-L4), 15 crates, 2 apps, and 42 tests. The core loop works: user message → LLM stream → tool execution → final answer, with session log invariant enforcement.

## Key design decisions

### Scoped reversible effects (replaces Cordis hot-swap)

Cordis allows arbitrary plugin load/unload at runtime. Rust has no safe dynamic code loading — `dlopen`/`libloading` require unsafe FFI with ABI stability risks. Instead, v0 uses RAII `ScopeGuard`s that send `TypeId` through an mpsc channel on drop, processed by a background task. This covers agent/session-level lifecycle (the common case) but not global hot-swap (rare).

**Tradeoff**: ~80% of dsh hot-swap scenarios covered. Global provider swap uses `tokio::sync::watch` (next-call生效). Sandbox backend swap requires task restart.

### Closed SessionEvent enum

`SessionEvent` is a closed enum, not an extensible interface. New event types require updating all match sites — the compiler enforces this. This catches "forgot to implement projection" at compile time, not runtime.

**Tradeoff**: Adding events is more work (must update all matches). If event types exceed 20+, consider `#[non_exhaustive]` + visitor pattern.

### Sandbox as type system

`SandboxResolution::Unavailable` forces callers to handle rejection at compile time. "Forgot to handle denial" is a compiler error, not a runtime behavior.

## v1 Roadmap

1. **L3 Composition** — Cargo feature flags + Profile/Bundle config system
2. **MCP client** — Assess Rust MCP crate maturity; build if needed
3. **More LLM providers** — OpenAI, Anthropic adapters via `LlmAdapter` trait
4. **Web surface** — axum HTTP API with SSE streaming
5. **Session persistence** — redb/sled for performance-critical paths
6. **Oris integration** — Extract Gene/Capsule state management patterns after Oris `unwrap()` cleanup
7. **Windows sandbox** — `windows-rs` restricted token API
8. **Python/Node SDK** — UniFFI or gRPC bridge
