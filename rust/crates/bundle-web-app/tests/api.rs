//! Integration tests for the web API endpoints.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;

use dsh_bundle_web_app::{AppState, build_router};
use dsh_llm_adapter::{LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse};

/// Mock LLM for testing.
struct MockLlm;

#[async_trait]
impl LlmAdapter for MockLlm {
    async fn stream(
        &self,
        _request: LlmRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError>
    {
        let chunks = vec![Ok(LlmChunk {
            delta: "Hello from mock!".into(),
            finish_reason: Some("stop".into()),
            tool_calls: None,
        })];
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: "Hello from mock!".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        })
    }
}

async fn spawn_server() -> SocketAddr {
    let state = AppState {
        llm: Arc::new(MockLlm),
        model: "test-model".into(),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    addr
}

#[tokio::test]
async fn health_endpoint() {
    let addr = spawn_server().await;
    let resp = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn chat_endpoint() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/v1/chat"))
        .json(&serde_json::json!({
            "message": "Hello",
            "model": "test-model"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["response"], "Hello from mock!");
    assert!(body["session_events"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn chat_stream_endpoint() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/v1/chat/stream"))
        .json(&serde_json::json!({
            "message": "Hello",
            "model": "test-model"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    // SSE format: data: {...}\n\n
    assert!(body.contains("\"event\":\"chunk\""));
    assert!(body.contains("Hello from mock!"));
    assert!(body.contains("\"event\":\"done\""));
}
