# dsh-rust Architecture

> Five-layer agent harness framework, reimplemented in Rust from the TypeScript/Cordis dsh.

## Layer Overview

```
L4  Surface         dsh-headless (CLI) / dsh-web (axum) / SDK
L3  Composition     bundle-base / bundle-headless / bundle-web-app
L2  Capability      seam-sandbox / seam-fs / seam-subprocess
L1  Substrate       core-session / core-tools / core-agent-loop / llm-adapter / llm-deepseek
L0  Effect          core-context (Context, ScopeGuard, EventBus, ProviderRegistry)
```

## L0: Effect & Context Core

Replaces Cordis's dynamic plugin hot-swap with **scoped reversible effects** via RAII.

### Context

`DashMap<TypeId, Box<dyn Any + Send + Sync>>` — type-erased service container. Services are keyed by `TypeId` and registered/resolved by concrete type.

### ScopeGuard + UndoQueue

Each `provide()` returns a `ScopeGuard`. On `Drop`, the guard sends its `TypeId` through an `mpsc::UnboundedSender`. A background `tokio::spawn` task receives and removes the entry from the shared `DashMap`. This is the "reversible" semantics — scoped to agent/session lifetime, not global hot-swap.

### EventBus

Typed event dispatch keyed by `TypeId`. `on::<T>(handler)` subscribes, `emit(event)` dispatches. `Subscription` guard auto-removes on drop.

### ProviderRegistry

`tokio::sync::watch` channels for provider hot-replace. When a provider is updated, subscribers are notified. The replacement takes effect on the **next call**, not in-place.

## L1: Substrate

### Session Log (`core-session`)

**Core invariant: model-visible ⟺ logged.** `SessionEvent` is a closed enum — the compiler forces exhaustive matching when new event types are added. `derive_messages()` projects events into model-visible messages. `assert_invariant()` panics (debug AND release) if the derived messages don't match what was sent to the LLM.

### Tools (`core-tools`)

`Tool` trait with `schema()` + `execute(args)`. `ToolPipeline` provides pre/post middleware chain with rejection support.

### Agent Loop (`core-agent-loop`)

State machine: `ClaimingInput → AssemblingPrompt → Streaming → ExecutingTools → Closed`. `AgentRunner::run_turn()` drives a complete turn including tool-call loops. Tool calls are detected by parsing the assistant's content for `{"tool_call": {...}}` JSON.

### LLM Adapter (`llm-adapter` + `llm-deepseek`)

Trait-based abstraction. `DeepSeekAdapter` implements SSE streaming via `reqwest::bytes_stream()` + `flat_map` line parsing.

## L2: Capability Seam

### Sandbox (`seam-sandbox`)

Platform-specific backends behind `SandboxBackend` trait:
- **macOS**: `SeatbeltBackend` — spawns `sandbox-exec` with compiled profiles (Minimal/Confined/Strict)
- **Linux**: `LandlockBackend` — uses Landlock kernel feature (5.13+), conditional compilation

`SandboxResolution::Unavailable` forces callers to handle the no-sandbox case at compile time.

### Filesystem (`seam-fs`)

`FileSystem` trait + `LocalFileSystem` provider. Async read/write/list/exists.

### Subprocess (`seam-subprocess`)

`Subprocess` trait + `LocalSubprocess` provider. Async exec (wait) and spawn (background).

## L3: Composition

v0 uses direct assembly (no runtime plugin system). Cargo feature flags control which crates are linked. `bundle-base` provides the base wiring; `bundle-headless` and `bundle-web-app` specialize it.

## L4: Surface

### Headless Runner (`apps/dsh-headless`)

CLI binary using `clap`. Assembles Context → DeepSeekAdapter → AgentRunner → session JSONL output. Sandbox auto-detected via `resolve()`.

### Web Server (`apps/dsh-web`)

Placeholder for axum-based HTTP API (v1).

## Cargo Workspace

```
crates/
  core-context/          # L0: 5 modules, 17 tests
  core-session/          # L1: 3 modules, 6 tests
  core-tools/            # L1: 2 modules, 2 tests
  core-agent-loop/       # L1: 2 modules + 2 snapshots, 4 tests
  llm-adapter/           # L1: trait definitions
  llm-deepseek/          # L1: DeepSeek provider
  seam-fs/               # L2: filesystem seam
  seam-sandbox/          # L2: sandbox seam, 6 tests
  seam-subprocess/       # L2: subprocess seam
  bundle-base/           # L3: base assembly
  bundle-headless/       # L3: headless bundle
  bundle-web-app/        # L3: web bundle
  test-support/          # Mock LLM + fixtures
apps/
  dsh-headless/          # CLI binary, 3 e2e tests
  dsh-web/               # Web binary (stub)
```

## Design Tradeoffs vs TypeScript dsh

| Aspect | TypeScript dsh | Rust dsh |
|--------|---------------|----------|
| Hot-swap | Runtime plugin load/unload | Scoped RAII guards + watch notification |
| Event system | Extensible `SessionEventMap` | Closed `SessionEvent` enum |
| Sandbox | Spawn bubblewrap/landlock | Native Landlock + seatbelt profiles |
| Persistence | SQLite + session log | JSONL (v0) |
| Composition | cordis.yml runtime config | Cargo feature flags + config |
| Testing | vitest + snapshot | insta + tokio::test |
