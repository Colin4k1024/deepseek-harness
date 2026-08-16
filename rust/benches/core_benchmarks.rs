//! Core benchmarks — Context, Session, Tools, Agent Loop.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_context::Context;
use dsh_core_session::derive::derive_messages;
use dsh_core_session::event::{SessionEvent, Surface};
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_llm_adapter::{LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse};

// ──────────────────────────────────────────────
// Mock LLM
// ──────────────────────────────────────────────

struct FastMockLlm;

#[async_trait]
impl LlmAdapter for FastMockLlm {
    async fn stream(
        &self,
        _request: LlmRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError>
    {
        let chunks = vec![Ok(LlmChunk {
            delta: "ok".into(),
            finish_reason: Some("stop".into()),
        })];
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        unimplemented!()
    }
}

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: serde_json::json!({}),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            content: args.to_string(),
            is_error: false,
        })
    }
}

// ──────────────────────────────────────────────
// Benchmarks
// ──────────────────────────────────────────────

fn bench_context_provide_resolve(c: &mut Criterion) {
    let ctx = Context::new();
    c.bench_function("context_provide_resolve", |b| {
        b.iter(|| {
            let guard = ctx.provide(black_box(42u32));
            let _ = ctx.resolve::<u32>();
            drop(guard);
        })
    });
}

fn bench_session_derive_messages(c: &mut Criterion) {
    let log: Vec<SessionEvent> = (0..100)
        .flat_map(|i| {
            vec![
                SessionEvent::UserMessage {
                    content: format!("msg {i}"),
                },
                SessionEvent::AssistantMessage {
                    content: format!("reply {i}"),
                },
            ]
        })
        .collect();

    c.bench_function("derive_messages_200", |b| {
        b.iter(|| {
            let _ = derive_messages(black_box(&log));
        })
    });
}

fn bench_session_surface_project(c: &mut Criterion) {
    let event = SessionEvent::UserMessage {
        content: "test message".into(),
    };

    c.bench_function("surface_project", |b| {
        b.iter(|| {
            let _ = black_box(&event).project();
        })
    });
}

fn bench_tool_execute(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tool = EchoTool;

    c.bench_function("tool_execute", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = tool
                    .execute(black_box(serde_json::json!({"text": "hello"})))
                    .await;
            })
        })
    });
}

fn bench_agent_loop_turn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let llm = Arc::new(FastMockLlm);

    c.bench_function("agent_loop_turn", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut runner = AgentRunner::new(llm.clone(), "bench".into());
                let _ = runner.run_turn(black_box("test".into())).await;
            })
        })
    });
}

fn bench_agent_loop_with_tool(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _tool_call = r#"{"tool_call": {"name": "echo", "arguments": {"text": "hi"}}}"#;
    let llm = Arc::new(FastMockLlm);
    // Note: FastMockLlm always returns "ok", not the tool_call JSON.
    // This benchmarks the non-tool path. For tool path, use ScriptedLlm.

    c.bench_function("agent_loop_no_tool", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut runner = AgentRunner::new(llm.clone(), "bench".into());
                runner.register_tool(Arc::new(EchoTool));
                let _ = runner.run_turn(black_box("test".into())).await;
            })
        })
    });
}

criterion_group!(
    core_benches,
    bench_context_provide_resolve,
    bench_session_derive_messages,
    bench_session_surface_project,
    bench_tool_execute,
    bench_agent_loop_turn,
    bench_agent_loop_with_tool,
);
criterion_main!(core_benches);
