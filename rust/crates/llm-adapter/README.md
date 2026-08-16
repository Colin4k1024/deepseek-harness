# dsh-llm-adapter

L1: LLM adapter trait and request/response types.

## Purpose

Defines the trait interface that all LLM providers implement, plus the request/response types shared across the framework.

## Key Types

- **`LlmAdapter`** — Async trait with `stream()` and `complete()` methods.
- **`LlmRequest`** — Model, messages, max_tokens, temperature, stream flag.
- **`LlmChunk`** — Streaming chunk: delta text + optional finish_reason.
- **`LlmResponse`** — Complete response: content + finish_reason + usage.
- **`Message`** — Role + content.
