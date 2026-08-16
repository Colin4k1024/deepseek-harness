# PRD: dsh-rust — Rust Reimplementation of DeepSeek Harness Core

> State: plan | Role: tech-lead | Date: 2026-08-15

## 1. Background

DeepSeek Harness (dsh) is currently a TypeScript/Cordis-based plugin harness with 219 packages. The Cordis model provides "time-reversible + spatially-responsive" guarantees via dynamic plugin hot-swap, but Rust has no language-native support for these patterns. This design reimagines dsh's core guarantees using Rust's ownership model.

## 2. Goals

1. Build a Rust-native agent harness framework preserving dsh's core invariants
2. Session log invariant: `model-visible ⟺ logged`, enforced at compile-time + runtime
3. Scoped reversible effects via RAII `ScopeGuard`, covering agent/session-level lifecycle
4. Snapshot-based replay testing using `insta` crate
5. At least one LLM provider (DeepSeek) and one sandbox backend functional

## 3. Non-Goals (v0)

- Runtime arbitrary plugin hot-swap (replaced by scoped effects + config-time composition)
- Web UI / frontend (L4 is API-only for v0)
- Distributed session / Postgres backend
- Python/Node SDK
- MCP client support (deferred to v1 pending Rust ecosystem maturity assessment)

## 4. Architecture (5-Layer)

| Layer | Purpose | Key Components |
|-------|---------|---------------|
| L0 | Effect & Context core | `Context`, `ScopeGuard`, `EventBus`, `DashMap<TypeId, Service>` |
| L1 | Substrate | Session log (`SessionEvent` enum), tool registry, agent-loop state machine, LLM adapter trait |
| L2 | Capability Seam | Sandbox (Landlock/sandbox-exec/windows-rs), FS, subprocess, terminal |
| L3 | Composition | Cargo feature flags, Profile/Bundle config-driven activation |
| L4 | Surface | Headless CLI runner, axum web server, Rust crate SDK |

## 5. Key Design Decisions

### 5.1 Scoped Reversible Effects (replaces Cordis hot-swap)

- Each `provide()` returns a `ScopeGuard`; `Drop` enqueues undo to a dedicated queue
- `ScopedContext` = parent `Context` ref + layer's guards; agent end = bulk undo
- Tradeoff: covers ~80% of dsh hot-swap use cases; global provider swap uses `tokio::sync::watch` notification (next-call生效, not in-place replacement)

### 5.2 Session Log as Closed Enum

- `SessionEvent` is a closed enum (not extensible interface); compiler forces exhaustive match
- New event types require updating all match sites — compile-time enforcement vs runtime miss
- Consider `#[non_exhaustive]` + visitor pattern if event types exceed 20+

### 5.3 Sandbox Resolution as Type System

- `SandboxResolution::Unavailable { requested }` forces caller to handle rejection at compile-time
- Linux: `landlock` crate (first-class, not fallback)
- macOS: spawn `sandbox-exec` (no stable Rust binding)
- Windows: `windows-rs` restricted token API

### 5.4 Cargo Workspace

```
crates/
  core-context/       # L0
  core-session/       # L1
  core-tools/
  core-agent-loop/
  llm-adapter/
  llm-deepseek/
  seam-fs/
  seam-sandbox/
  seam-subprocess/
  bundle-base/
  bundle-web-app/
  bundle-headless/
  test-support/
apps/
  dsh-web/
  dsh-headless/
```

## 6. User Stories

| ID | Story | Acceptance Criteria |
|----|-------|-------------------|
| US1 | As a framework user, I can run an agent loop with a mock LLM and verify session log output | `basic_tool_call.jsonl` snapshot passes via `insta` |
| US2 | As a framework developer, I can register a service in a scope and have it auto-deregistered when the scope ends | `ScopeGuard::drop` triggers undo; service no longer resolvable after scope exit |
| US3 | As a tool author, I can implement the `Tool` trait and have it participate in the tool pipeline | Tool schema exposed, execute called with correct args, pre/post middleware chain works |
| US4 | As a framework operator, I can switch LLM provider via config without restarting the process | `watch` channel notifies subscribers; next LLM call uses new provider |
| US5 | As a sandbox user, I get a compile error if I don't handle the `Unavailable` sandbox case | `SandboxResolution::match` must include `Unavailable` branch |

## 7. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Oris 289 `unwrap()` leaks into L0 if reusing without fix | High | High | Decision needed: fix-first vs write-new-first (C1) |
| MCP client work underestimated | Medium | High | Defer MCP to v1; assess Rust crate maturity first |
| `landlock` crate API instability | Low | Medium | Pin version; abstract behind `SandboxBackend` trait |
| Closed `SessionEvent` enum maintenance burden at scale | Medium | Medium | Monitor event type count; plan `#[non_exhaustive]` migration path |
| ScopeGuard undo queue error handling complexity | Medium | High | Architect to specify queue consumer design (C6) |

## 8. Open Questions

| # | Question | Owner | Status |
|---|----------|-------|--------|
| C1 | Oris reuse strategy: fix-first or write-new-first? | tech-lead + architect | Open |
| C2 | Does v0 include L3 Composition (feature flags + Profile)? | tech-lead | Open |
| C3 | MCP client: build vs wait for ecosystem? | architect | Open |
| C4 | Session log persistence: redb vs sled vs JSONL? | architect | Open |
| C5 | Relationship with existing dsh TS: independent or backend? | tech-lead | Open |
| C6 | ScopeGuard undo queue: consumer design + error handling? | architect | Open |
| C7 | Windows sandbox via windows-rs: feasibility validation? | backend-engineer | Open |

## 9. Honest Tradeoff Summary (from design doc)

1. No true "runtime arbitrary hot-swap" — replaced by "scoped reversible effects + config-time composition"
2. LLM provider switch via watch notification, not in-place replacement; sandbox backend switch requires task restart
3. No Node ecosystem's mature MCP client / model SDKs — must self-build or accept gaps
4. This is "redesign a new framework", not a port — workload comparable to prior superagent-base Rust rewrite estimates
