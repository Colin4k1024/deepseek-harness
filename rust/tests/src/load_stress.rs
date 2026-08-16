//! Load and stress tests for dsh-rust.
//!
//! Tests system behavior under high concurrency, many requests,
//! and resource pressure. Not correctness tests — these verify
//! the system doesn't panic, leak, or degrade ungracefully.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;
use tokio::sync::Semaphore;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_context::Context;
use dsh_core_memory::{InMemoryStore, MemoryEntry, MemoryStore};
use dsh_core_orchestrator::{AgentHandle, AgentRegistry};
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_llm_adapter::{LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse};
use dsh_seam_auth::{AuthManager, RateLimitConfig, RateLimiter};
use dsh_seam_observability::Metrics;

// ============================================================================
// Fixtures
// ============================================================================

/// Fast mock LLM with configurable latency.
struct LoadTestLlm {
    latency_ms: u64,
}

impl LoadTestLlm {
    fn new(latency_ms: u64) -> Self {
        Self { latency_ms }
    }
}

#[async_trait]
impl LlmAdapter for LoadTestLlm {
    async fn stream(
        &self,
        _request: LlmRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError>
    {
        if self.latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;
        }
        let chunks = vec![Ok(LlmChunk {
            delta: "ok".into(),
            finish_reason: Some("stop".into()),
            tool_calls: None,
        })];
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        unimplemented!()
    }
}

struct FastTool;

#[async_trait]
impl Tool for FastTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "fast".into(),
            description: "Fast tool".into(),
            parameters: serde_json::json!({}),
        }
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            content: "done".into(),
            is_error: false,
        })
    }
}

// ============================================================================
// Load Tests: Concurrent Agent Turns
// ============================================================================

#[tokio::test]
async fn load_concurrent_agent_turns_100() {
    let llm = Arc::new(LoadTestLlm::new(0));
    let semaphore = Arc::new(Semaphore::new(100));
    let metrics = Metrics::new();

    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..100 {
        let llm = llm.clone();
        let sem = semaphore.clone();
        let m = metrics.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let mut runner = AgentRunner::new(llm, "load-test".into());
            let result = runner.run_turn(format!("request {i}")).await;
            m.inc("requests.total");
            result
        }));
    }

    let mut success = 0;
    let mut fail = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => success += 1,
            Err(_) => fail += 1,
        }
    }

    let elapsed = start.elapsed();
    println!(
        "100 concurrent turns: {} ok, {} fail in {:?}",
        success, fail, elapsed
    );

    assert_eq!(success, 100);
    assert_eq!(fail, 0);
    assert!(
        elapsed < Duration::from_secs(10),
        "took too long: {:?}",
        elapsed
    );
}

// ============================================================================
// Load Tests: Concurrent Tool Execution
// ============================================================================

#[tokio::test]
async fn load_concurrent_tool_execution_500() {
    let tool = Arc::new(FastTool);
    let metrics = Metrics::new();

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..500 {
        let tool = tool.clone();
        let m = metrics.clone();

        handles.push(tokio::spawn(async move {
            let result = tool.execute(serde_json::json!({})).await;
            m.inc("tool.calls");
            result
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success += 1;
        }
    }

    let elapsed = start.elapsed();
    println!("500 concurrent tool calls: {} ok in {:?}", success, elapsed);

    assert_eq!(success, 500);
    assert!(
        elapsed < Duration::from_secs(5),
        "took too long: {:?}",
        elapsed
    );
}

// ============================================================================
// Stress Tests: Rate Limiter Under Load
// ============================================================================

#[tokio::test]
async fn stress_rate_limiter_concurrent() {
    let limiter = Arc::new(RateLimiter::new(RateLimitConfig {
        max_tokens: 10,
        refill_rate: 0.0, // No refill — pure burst test
    }));

    let mut handles = Vec::new();

    for i in 0..100 {
        let limiter = limiter.clone();
        handles.push(tokio::spawn(async move {
            limiter.check(&format!("user_{}", i % 5))
        }));
    }

    let mut allowed = 0;
    let mut denied = 0;
    for handle in handles {
        if handle.await.unwrap() {
            allowed += 1;
        } else {
            denied += 1;
        }
    }

    println!(
        "Rate limiter stress: {} allowed, {} denied",
        allowed, denied
    );
    // 100 requests, 5 users, 10 tokens each = 50 max allowed
    assert!(denied > 0, "expected some rate limit denials");
}

// ============================================================================
// Stress Tests: Auth Under Load
// ============================================================================

#[tokio::test]
async fn stress_auth_concurrent_verify() {
    let auth = Arc::new(AuthManager::new());
    for i in 0..100 {
        auth.register_key(
            &format!("sk-{i}"),
            &format!("key {i}"),
            &format!("user{i}"),
            vec!["chat".into()],
        );
    }

    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..1000 {
        let auth = auth.clone();
        handles.push(tokio::spawn(async move {
            auth.verify(&format!("sk-{}", i % 100))
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success += 1;
        }
    }

    let elapsed = start.elapsed();
    println!(
        "1000 concurrent auth verifies: {} ok in {:?}",
        success, elapsed
    );

    assert_eq!(success, 1000);
    assert!(
        elapsed < Duration::from_secs(5),
        "took too long: {:?}",
        elapsed
    );
}

// ============================================================================
// Stress Tests: Memory Store Under Load
// ============================================================================

#[tokio::test]
async fn stress_memory_store_concurrent_writes() {
    let store = Arc::new(InMemoryStore::new());

    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..1000 {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            store
                .store(MemoryEntry::new(format!("entry {i}"), "load-test"))
                .await
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success += 1;
        }
    }

    let elapsed = start.elapsed();
    let count = store.count().await.unwrap();
    println!(
        "1000 concurrent memory writes: {} ok, total {} in {:?}",
        success, count, elapsed
    );

    assert_eq!(success, 1000);
    assert_eq!(count, 1000);
}

#[tokio::test]
async fn stress_memory_concurrent_read_write() {
    let store = Arc::new(InMemoryStore::new());

    // Pre-populate
    for i in 0..100 {
        store
            .store(MemoryEntry::new(format!("initial {i}"), "fact"))
            .await
            .unwrap();
    }

    let start = Instant::now();
    let mut handles = Vec::new();

    // 500 writes + 500 reads concurrently
    for i in 0..500 {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            // Write
            store
                .store(MemoryEntry::new(format!("new {i}"), "fact"))
                .await
                .unwrap();
            // Read
            let _ = store.search_text("initial", 10).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    let count = store.count().await.unwrap();
    println!(
        "500 concurrent read+write: total entries {} in {:?}",
        count, elapsed
    );

    assert!(count >= 600); // 100 initial + 500 new
}

// ============================================================================
// Stress Tests: Context Under Load
// ============================================================================

#[tokio::test]
async fn stress_context_concurrent_provide_resolve() {
    let ctx = Arc::new(Context::new());

    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..100 {
        let ctx = ctx.clone();
        handles.push(tokio::spawn(async move {
            let guard = ctx.provide(format!("service-{i}"));
            // Small delay to simulate real usage
            tokio::time::sleep(Duration::from_millis(1)).await;
            drop(guard);
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    println!("100 concurrent provide/drop cycles in {:?}", elapsed);

    // Wait for undo queue to drain
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(ctx.is_empty());
}

// ============================================================================
// Stress Tests: Orchestrator Under Load
// ============================================================================

#[tokio::test]
async fn stress_orchestrator_many_agents() {
    let registry = Arc::new(AgentRegistry::new());

    let start = Instant::now();

    // Register 100 agents
    for i in 0..100 {
        let (handle, _rx) = AgentHandle::new(
            format!("agent-{i}"),
            format!("Agent {i}"),
            vec![format!("cap-{}", i % 10)],
        );
        registry.register(handle).await;
    }

    let reg_time = start.elapsed();

    // Find by capability
    let find_start = Instant::now();
    let agents = registry.find_by_capability("cap-5").await;
    let find_time = find_start.elapsed();

    println!(
        "100 agents registered in {:?}, find by cap in {:?}, found {}",
        reg_time,
        find_time,
        agents.len()
    );

    assert_eq!(agents.len(), 10); // 100 agents / 10 capabilities
}

// ============================================================================
// Stress Tests: Metrics Under Load
// ============================================================================

#[tokio::test]
async fn stress_metrics_concurrent_updates() {
    let metrics = Metrics::new();

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..1000 {
        let m = metrics.clone();
        handles.push(tokio::spawn(async move {
            m.inc("requests");
            m.inc_by("bytes", 1024);
            m.record_value("latency", 42.0);
            m.set_gauge("active", 42);
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    println!("4000 concurrent metric updates in {:?}", elapsed);

    assert_eq!(metrics.counter("requests"), 1000);
    assert_eq!(metrics.counter("bytes"), 1024000);
    assert_eq!(metrics.gauge("active"), 42);
}
