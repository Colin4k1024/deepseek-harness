# dsh-llm-deepseek

DeepSeek LLM provider with SSE streaming support.

## Purpose

Implements `LlmAdapter` for the DeepSeek API (`/chat/completions`), supporting both streaming (SSE) and non-streaming modes.

## Usage

```rust
use dsh_llm_deepseek::DeepSeekAdapter;
use dsh_llm_adapter::LlmAdapter;

let adapter = DeepSeekAdapter::new("sk-...".into());
// Optional: adapter.with_base_url("https://custom.api.com".into());
let mut stream = adapter.stream(request).await?;
```

## SSE Parsing

The streaming implementation uses `reqwest::Response::bytes_stream()` + `flat_map` to parse SSE lines in real-time. Each `data: {json}` line is deserialized into an `LlmChunk`. The `data: [DONE]` sentinel terminates the stream.
