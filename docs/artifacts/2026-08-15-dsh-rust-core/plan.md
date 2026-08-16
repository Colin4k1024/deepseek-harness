# Plan: dsh-rust — Detailed Task Breakdown

> State: plan | Role: tech-lead | Date: 2026-08-15

## Tech Lead Decisions (Blocking Items Resolved)

| # | Decision | Rationale |
|---|----------|-----------|
| C1 | Write L0 from scratch; Oris reuse deferred to v1 parallel track | Oris 289 `unwrap()` fix is independent work; clean L0 unblocks everything |
| C2 | v0 excludes L3 Composition (Profile/Bundle) | Feature flags are compile-time; Profile config needs L0-L2 stable first |
| C3 | v0 excludes MCP client | Spike needed to assess Rust MCP crate maturity; not a blocker |
| C4 | v0 uses JSONL persistence | Simplest; naturally compatible with dsh snapshot test format |
| C5 | Independent project, no dsh TS backend | Rust framework is standalone; future interop via ACP protocol |
| C6 | Undo queue design deferred to Phase 1 architect deliverable | Blocks L0 implementation; assigned to Phase 1 T1.1 |
| C7 | Windows sandbox deferred to v1 | v0 focuses on Linux (Landlock) + macOS (sandbox-exec) |

## Phase 0: Workspace Bootstrap (~2 days)

**Goal**: Cargo workspace skeleton compiles, CI runs, all crate stubs in place.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T0.1 | Init Cargo workspace (root `Cargo.toml` + `crates/` + `apps/` dirs) | `Cargo.toml` workspace def | `cargo check` passes |
| T0.2 | Create all crate stubs (`lib.rs` + `Cargo.toml` + inter-crate deps) | 13 crates + 2 apps | `cargo build` full compile passes |
| T0.3 | CI config (GitHub Actions: check + test + clippy) | `.github/workflows/rust-ci.yml` | CI green |
| T0.4 | `test-support` crate infra (`insta` + fixture loader) | `test-support/src/lib.rs` | `cargo test -p test-support` passes |
| T0.5 | `CONTRIBUTING.rust.md` (clippy zero warning, no-unwrap policy, snapshot workflow) | Doc | — |

**Dependencies**: None (all tasks parallelizable)

## Phase 1: L0 — Effect & Context Core (~5 days)

**Goal**: `Context` + `ScopeGuard` + `EventBus` functional. US2 verifiable.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T1.1 | **Design deliverable**: ScopeGuard undo queue mechanism (resolves C6) | Design doc / Agent Note | Architect review passes |
| T1.2 | `Context` core: `DashMap<TypeId, Box<dyn Any>>` service registration + typed resolve | `core-context/src/context.rs` | Unit: provide → resolve → type match |
| T1.3 | `ScopeGuard` + `ScopedContext`: RAII undo semantics | `core-context/src/scope.rs` | US2: scope exit → service unresolvable |
| T1.4 | Undo queue consumer (tokio mpsc) | `core-context/src/undo.rs` | Unit: guard drop → undo executes → no panic |
| T1.5 | `EventBus`: typed event dispatch + waterfall semantics | `core-context/src/event.rs` | Unit: subscribe → dispatch → receive; waterfall `next()` propagation |
| T1.6 | `tokio::sync::watch` channel for provider hot-replace notification | `core-context/src/provider.rs` | Unit: update → subscriber notified → next resolve gets new value |
| T1.7 | Integration: multi-layer scope nesting + bulk undo | `core-context/tests/` | 3-layer nesting, verify undo order inside-out |

**Dependencies**: T0.1, T0.2
**Parallelism**: T1.1 (architect) || T1.2-T1.6 (backend-engineer)

## Phase 2: L1 — Session Log & Agent Loop (~7 days)

**Goal**: Session log invariant verifiable, agent loop runs a complete turn. US1 verifiable.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T2.1 | `SessionEvent` closed enum + `Surface` trait (`project()` method) | `core-session/src/event.rs` | Compiles: all variants have `project()` |
| T2.2 | `derive_messages()` + invariant assertion | `core-session/src/derive.rs` | Unit: log → derived == expected; assertion rejects mismatch |
| T2.3 | JSONL persistence (append-only write + load) | `core-session/src/persist.rs` | Unit: write events → read back → equal |
| T2.4 | `Tool` trait + `ToolSchema` + `ToolResult` types | `core-tools/src/trait_def.rs` | Compiles |
| T2.5 | `ToolPipeline`: pre/post middleware chain | `core-tools/src/pipeline.rs` | Unit: pre rejection → tool skipped; post hook receives result |
| T2.6 | `LlmAdapter` trait (`stream()` → `Stream<Item = Chunk>`) | `llm-adapter/src/lib.rs` | Compiles |
| T2.7 | `MockLlm` (replay from fixture JSONL) | `test-support/src/mock_llm.rs` | Constructable from file, returns pre-recorded chunks |
| T2.8 | Agent-loop state machine (`TurnState` enum + tokio async) | `core-agent-loop/src/loop.rs` | Unit: ClaimingInput → AssemblingPrompt → Streaming → Closed |
| T2.9 | **Integration**: `basic_tool_call` snapshot test | `apps/dsh-headless/tests/` | US1: `insta` snapshot passes |
| T2.10 | Agent Note: closed enum vs extensible events tradeoff | `.agents/notes/` | — |

**Dependencies**: T1.2, T1.3
**Parallelism**: T2.1-T2.3 (session) || T2.4-T2.5 (tools) || T2.6-T2.7 (llm mock)

## Phase 3: L2 — Capability Seam (~5 days)

**Goal**: Sandbox + FS + subprocess usable. US5 verifiable.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T3.1 | `SandboxBackend` trait + `SandboxResolution` enum | `seam-sandbox/src/trait_def.rs` | US5: `Unavailable` branch compile-time enforced |
| T3.2 | Landlock backend (Linux) | `seam-sandbox/src/landlock.rs` | Linux unit: confine → file access restricted |
| T3.3 | sandbox-exec backend (macOS) | `seam-sandbox/src/seatbelt.rs` | macOS unit: confine → subprocess sandboxed |
| T3.4 | `FileSystem` trait (read/write/list/glob) | `seam-fs/src/trait_def.rs` | Compiles |
| T3.5 | Local filesystem provider | `seam-fs/src/local.rs` | Unit: read/write/list |
| T3.6 | `Subprocess` trait + local process-tree provider | `seam-subprocess/src/lib.rs` | Unit: spawn → stdout capture → exit code |

**Dependencies**: T1.2
**Parallelism**: T3.1-T3.3 (sandbox) || T3.4-T3.5 (fs) || T3.6 (subprocess)

## Phase 4: L4 Surface — Headless Runner (~3 days)

**Goal**: `dsh-headless` CLI runs a complete agent task.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T4.1 | `dsh-headless` CLI entry (clap args + Context assembly) | `apps/dsh-headless/src/main.rs` | `cargo run -p dsh-headless -- --help` works |
| T4.2 | Wire L0→L1→L2 end-to-end: Context → Session → Tool → AgentLoop → output | `apps/dsh-headless/src/run.rs` | E2E: prompt in → agent loop runs → session log out |
| T4.3 | DeepSeek LLM provider (HTTP client + SSE stream parse) | `llm-deepseek/src/lib.rs` | E2E test with API key (self-skip without key) |
| T4.4 | Integration snapshot tests: headless runner full replay | `apps/dsh-headless/tests/` | 2+ snapshot scenarios pass |

**Dependencies**: T2.8, T3.1
**Parallelism**: T4.1-T4.2 || T4.3

## Phase 5: Quality Hardening & Docs (~3 days)

**Goal**: Production-grade quality, documentation complete, publishable.

| Task | Description | Output | Acceptance |
|------|-------------|--------|------------|
| T5.1 | `cargo clippy` zero warning cleanup | All crates | `cargo clippy -- -D warnings` passes |
| T5.2 | `cargo deny` dependency audit config | `deny.toml` | `cargo deny check` passes |
| T5.3 | Per-crate README (purpose, API overview, usage examples) | Crate READMEs | — |
| T5.4 | `docs/architecture-rust.md` (5-layer architecture doc) | Doc | — |
| T5.5 | Agent Note: dsh TS vs Rust diff list + v1 roadmap | `.agents/notes/` | — |

**Dependencies**: Phase 4 complete

## Dependency Graph

```
Phase 0 (Workspace Bootstrap)
    │
    ├──────────────────┐
    ▼                  ▼
Phase 1 (L0)      T0.4 (test-support)
    │                  │
    ├──────────────────┘
    ▼
Phase 2 (L1: Session + Tools + LLM mock + Agent Loop)
    │
    ├──────────┐
    ▼          ▼
Phase 3 (L2)  T2.9 (snapshot ready)
    │          │
    └──────────┘
    ▼
Phase 4 (L4: Headless Runner)
    │
    ▼
Phase 5 (Quality + Docs)
```

## Parallelism Analysis

| Phase | Max Parallel | Role Assignment |
|-------|-------------|-----------------|
| Phase 0 | 5 tasks | backend-engineer |
| Phase 1 | 3 tracks | architect (T1.1 design) + backend-engineer (T1.2-T1.6 impl) |
| Phase 2 | 3 tracks | backend-engineer (session + tools + llm) + qa-engineer (T2.7 mock + T2.9 snapshot) |
| Phase 3 | 3 tracks | backend-engineer (sandbox + fs + subprocess) |
| Phase 4 | 2 tracks | backend-engineer (CLI + DeepSeek provider) |
| Phase 5 | 2 tracks | backend-engineer (clippy + deny) + tech-lead (docs + notes) |

## Risk Register

| Risk | Mitigation | Owner |
|------|-----------|-------|
| `landlock` crate API incompatible with kernel version | Spike in T3.2; fallback to spawn sandbox-exec for all | backend-engineer |
| DeepSeek SSE stream parsing edge cases | T4.3 spike (1 day) before full impl | backend-engineer |
| Closed `SessionEvent` enum needs frequent expansion in Phase 2 | Start with minimal set (UserMessage + AssistantChunk + ToolCall + ToolResult); freeze before adding more | architect |
| L0 undo queue complexity underestimated | T1.1 is explicit design gate; architect must deliver before T1.4 starts | architect |

## Estimated Total: ~25 days (5 weeks)

Critical path: Phase 0 → Phase 1 → Phase 2 → Phase 4 → Phase 5
