# dsh-core-session

L1: Session log — the source of truth for all model-visible state.

## Purpose

Defines the session event log and enforces the core invariant: **model-visible ⟺ logged**. Everything that reaches an LLM request must be reconstructable from the session log.

## Key Types

- **`SessionEvent`** — Closed enum of all event types (`UserMessage`, `AssistantMessage`, `AssistantChunk`, `ToolCall`, `ToolResult`). New model-visible types MUST be added here — the compiler forces exhaustive matching.
- **`Surface`** — Trait for projecting events into `ModelMessage`. Implemented by `SessionEvent`.
- **`derive_messages()`** — Derives the model-visible message list from a session log.
- **`assert_invariant()`** — Runtime assertion that request messages match derived messages. Panics in both debug and release.

## Usage

```rust
use dsh_core_session::{SessionEvent, derive::derive_messages};

let log = vec![
    SessionEvent::UserMessage { content: "hello".into() },
    SessionEvent::AssistantMessage { content: "hi".into() },
];
let messages = derive_messages(&log);
assert_eq!(messages.len(), 2);
```

## Persistence

`persist::append_event()` and `persist::load_events()` provide JSONL-based append-only persistence.
