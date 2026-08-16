# dsh-core-context

L0: Effect & Context core for dsh-rust.

## Purpose

Provides the foundational service container, scoped registration, undo semantics, event bus, and provider hot-replace notification that all higher layers build on.

## Key Types

- **`Context`** — Type-erased service container (`DashMap<TypeId, Box<dyn Any>>`). Services are registered with `provide::<T>()` and resolved with `resolve::<T>()`.
- **`ScopeGuard`** — RAII guard that enqueues service deregistration on drop via an mpsc channel.
- **`ScopedContext`** — Holds a batch of `ScopeGuard`s for a logical scope (e.g., an agent's lifetime).
- **`EventBus`** — Typed event dispatch with subscription guards. `on::<T>()` subscribes, `emit()` dispatches.
- **`ProviderRegistry`** — `tokio::sync::watch` channels for provider hot-replace notification (next-call生效).

## Usage

```rust
use dsh_core_context::Context;

let ctx = Context::new();
let _guard = ctx.provide(42u32);
assert!(ctx.resolve::<u32>().is_some());
// When _guard drops, the service is automatically removed.
```

## Undo Queue

When a `ScopeGuard` drops, it sends its `TypeId` through an mpsc channel. A background `tokio::spawn`ed task receives and removes the entry from the shared `DashMap`. This avoids async operations in `Drop`.
