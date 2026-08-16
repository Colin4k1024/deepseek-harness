# Contributing to dsh-rust

## Prerequisites

- Rust 1.77+ (stable toolchain)
- macOS, Linux, or Windows (WSL2)

## Quick Start

```bash
cd rust
cargo build          # compile all crates
cargo test           # run all tests
cargo clippy -- -D warnings   # lint (zero warnings policy)
cargo fmt --all      # format
```

## Crate Layout

```
crates/
  core-context/       # L0: Effect & Context (ScopeGuard, EventBus)
  core-session/       # L1: Session log (SessionEvent, invariant assertion)
  core-tools/         # L1: Tool trait + pipeline
  core-agent-loop/    # L1: Agent loop state machine
  llm-adapter/        # L1: LLM adapter trait
  llm-deepseek/       # L1: DeepSeek provider
  seam-fs/            # L2: Filesystem capability seam
  seam-sandbox/       # L2: Sandbox capability seam
  seam-subprocess/    # L2: Subprocess capability seam
  bundle-base/        # L3: Base assembly
  bundle-web-app/     # L3: Web server bundle
  bundle-headless/    # L3: Headless runner bundle
  test-support/       # Mock LLM + fixture loader
apps/
  dsh-headless/       # CLI binary
  dsh-web/            # Web server binary
```

## Coding Standards

### No `unwrap()` / `expect()` in library code

All library crates (`crates/`) must use `Result` propagation (`?`). `unwrap()` is only allowed in tests and `apps/` binaries. Clippy enforces this via `-D warnings`.

### Dead code

Stub code for future phases uses `#[allow(dead_code)]` with a comment referencing the phase/task that will use it. Example:

```rust
#[allow(dead_code)] // Phase 1 T1.4 will wire undo queue to this method
pub(crate) fn remove(&self, type_id: TypeId) { ... }
```

### Session log invariant

Any new `SessionEvent` variant MUST implement `Surface::project()`. The compiler will enforce this via exhaustive matching. See `crates/core-session/src/event.rs`.

### Sandbox resolution

`SandboxResolution::Unavailable` MUST be handled by callers — the compiler enforces this. Never silently ignore the unavailable case.

## Testing

### Unit tests

Each crate has inline `#[cfg(test)]` modules. Run per-crate:

```bash
cargo test -p dsh-core-session
```

### Snapshot tests

Snapshot tests use the `insta` crate. Review snapshots with:

```bash
cargo insta review
```

### E2E tests

E2E tests requiring a DeepSeek API key self-skip when `DEEPSEEK_API_KEY` is not set.

## CI

GitHub Actions runs on every push/PR touching `rust/`:
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
