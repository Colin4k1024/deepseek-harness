# Architecture — dsh-rust

## Design Principles

1. **Type Safety First** — Leverage Rust's type system for compile-time guarantees
2. **Layered Architecture** — Clear separation between L0-L4 layers
3. **Capability Seams** — Service Definition / Provider / Consumer pattern
4. **RAII for Lifecycle** — ScopeGuard for automatic cleanup
5. **Fail Loud** — Misconfiguration fails at load time, not runtime

---

## Layer Architecture

### L0: Effect & Context

The foundation layer provides:

- **Context** — Type-erased service container (`DashMap<TypeId, Box<dyn Any>>`)
- **ScopeGuard** — RAII guard that enqueues undo actions on drop
- **EventBus** — Typed event dispatch with subscription guards
- **ProviderRegistry** — Hot-replace notification via `tokio::sync::watch`

Key design: Undo queue uses `mpsc::UnboundedSender` — guards send TypeId on drop, background task removes from DashMap. No async in Drop.

### L1: Substrate

#### Session Log

Core invariant: **model-visible ⟺ logged**

```rust
pub enum SessionEvent {
    UserMessage { content: String },
    AssistantMessage { content: String },
    AssistantChunk { content: String },
    ToolCall { id: String, name: String, arguments: Value },
    ToolResult { call_id: String, content: String },
}
```

`derive_messages()` projects events into model-visible messages. `assert_invariant()` panics if they don't match.

#### Agent Loop

State machine: `ClaimingInput → AssemblingPrompt → Streaming → ExecutingTools → Closed`

```
UserMessage → derive_messages() → LlmRequest
  → LLM stream → AssistantChunk events
  → parse for tool_call JSON
    ├─ tool call → execute → ToolCall + ToolResult → loop
    └─ no tool call → AssistantMessage → Closed
```

#### LLM Adapter

Trait-based abstraction for streaming completions:

```rust
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn stream(&self, request: LlmRequest)
        -> Result<Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError>;
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}
```

### L2: Capability Seams

Each capability follows the Service Definition / Provider / Consumer pattern:

| Capability | Trait | Providers |
|------------|-------|-----------|
| Sandbox | `SandboxBackend` | Landlock (Linux), Seatbelt (macOS), Windows |
| Filesystem | `FileSystem` | Local filesystem |
| Subprocess | `Subprocess` | Local process tree |
| Persistence | `SessionStore` | JSONL, SQLite |
| Auth | `AuthManager` | API key + JWT |
| Observability | `Metrics` + `Telemetry` | tracing + custom metrics |

### L3: Composition

Compile-time feature flags + runtime profile configuration:

```toml
[features]
default = ["llm-deepseek", "sandbox", "persistence-jsonl"]
llm-openai = []
llm-anthropic = []
persistence-sqlite = []
```

### L4: Surface

- **dsh-headless** — CLI runner with clap
- **dsh-web** — axum HTTP + SSE streaming
- **Python SDK** — urllib-based, zero dependencies
- **Node SDK** — native fetch, zero dependencies

---

## Data Flow

```
User Request
    ↓
Surface (CLI / HTTP)
    ↓
AgentRunner.run_turn()
    ↓
Session Log (append UserMessage)
    ↓
derive_messages() → LlmRequest
    ↓
LlmAdapter.stream()
    ↓
Parse response for tool calls
    ↓
┌─────────────────────────────┐
│ Tool call found?            │
│   Yes → execute tool        │
│       → append ToolCall     │
│       → append ToolResult   │
│       → loop back           │
│   No  → append AssistantMsg │
│       → return response     │
└─────────────────────────────┘
```

---

## Concurrency Model

- **Tokio runtime** — Async executor for all I/O
- **DashMap** — Lock-free concurrent service container
- **mpsc channels** — Undo queue, agent messaging
- **watch channels** — Provider hot-replace notification
- **RwLock** — Auth, metrics, memory store

---

## Error Handling

- **No `unwrap()` in library code** — All errors propagated via `Result`
- **`thiserror`** — Structured error types per crate
- **Fail-closed** — Auth, sandbox, quota all deny on error

---

## Testing Strategy

| Tier | Tool | Coverage |
|------|------|----------|
| Unit | `cargo test` | Per-crate, 112 tests |
| Snapshot | `insta` | Agent output regression, 3 tests |
| E2E | Custom | Full-stack integration, 22 tests |
| Benchmark | `criterion` | Performance baselines, 15 benches |
| Coverage | `cargo-llvm-cov` | LCOV report |
