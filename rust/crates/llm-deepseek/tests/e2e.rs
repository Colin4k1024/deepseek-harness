//! E2E tests for DeepSeek provider SSE streaming.

use dsh_llm_adapter::{FunctionDefinition, LlmAdapter, LlmRequest, Message, Role, ToolDefinition};
use dsh_llm_deepseek::DeepSeekAdapter;
use futures::StreamExt;

/// Create a mock SSE response for testing.
fn mock_sse_response(chunks: Vec<&str>) -> String {
    chunks
        .iter()
        .map(|c| format!("data: {c}\n\n"))
        .collect::<String>()
}

#[tokio::test]
async fn stream_text_response() {
    let response_body = mock_sse_response(vec![
        r#"{"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{"content":" World"},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
    ]);

    // Start a mock server
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(&response_body)
        .create_async()
        .await;

    let adapter = DeepSeekAdapter::new("test-key".into()).with_base_url(server.url());

    let request = LlmRequest {
        model: "deepseek-chat".into(),
        messages: vec![Message {
            role: Role::User,
            content: "Hello".into(),
            tool_calls: None,
            tool_call_id: None,
        }],
        max_tokens: Some(100),
        temperature: None,
        stream: true,
        tools: None,
    };

    let mut stream = adapter.stream(request).await.unwrap();
    let mut content = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        content.push_str(&chunk.delta);
    }

    assert_eq!(content, "Hello World");
    mock.assert_async().await;
}

#[tokio::test]
async fn stream_tool_calls() {
    let response_body = mock_sse_response(vec![
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"bash","arguments":""}}]},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"command\":\"ls\"}"}}]},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
    ]);

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(&response_body)
        .create_async()
        .await;

    let adapter = DeepSeekAdapter::new("test-key".into()).with_base_url(server.url());

    let request = LlmRequest {
        model: "deepseek-chat".into(),
        messages: vec![Message {
            role: Role::User,
            content: "list files".into(),
            tool_calls: None,
            tool_call_id: None,
        }],
        max_tokens: Some(100),
        temperature: None,
        stream: true,
        tools: Some(vec![ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "bash".into(),
                description: "Run a bash command".into(),
                parameters: serde_json::json!({"type": "object"}),
            },
        }]),
    };

    let mut stream = adapter.stream(request).await.unwrap();
    let mut tool_calls = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        if let Some(calls) = chunk.tool_calls {
            tool_calls.extend(calls);
        }
    }

    assert!(tool_calls.len() >= 1);
    assert_eq!(tool_calls[0].function.name, "bash");

    mock.assert_async().await;
}

#[tokio::test]
async fn stream_partial_lines() {
    // Simulate SSE chunks split across HTTP chunks
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel";
    let body2 = "lo\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n";

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(format!("{body}{body2}"))
        .create_async()
        .await;

    let adapter = DeepSeekAdapter::new("test-key".into()).with_base_url(server.url());

    let request = LlmRequest {
        model: "deepseek-chat".into(),
        messages: vec![Message {
            role: Role::User,
            content: "Hi".into(),
            tool_calls: None,
            tool_call_id: None,
        }],
        max_tokens: Some(100),
        temperature: None,
        stream: true,
        tools: None,
    };

    let mut stream = adapter.stream(request).await.unwrap();
    let mut content = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        content.push_str(&chunk.delta);
    }

    assert_eq!(content, "Hello");
    mock.assert_async().await;
}

#[tokio::test]
async fn complete_non_streaming() {
    let response_body = serde_json::json!({
        "choices": [{
            "message": {"role": "assistant", "content": "Hi there!"},
            "finish_reason": "stop"
        }]
    });

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body.to_string())
        .create_async()
        .await;

    let adapter = DeepSeekAdapter::new("test-key".into()).with_base_url(server.url());

    let request = LlmRequest {
        model: "deepseek-chat".into(),
        messages: vec![Message {
            role: Role::User,
            content: "Hello".into(),
            tool_calls: None,
            tool_call_id: None,
        }],
        max_tokens: Some(100),
        temperature: None,
        stream: false,
        tools: None,
    };

    let response = adapter.complete(request).await.unwrap();
    assert_eq!(response.content, "Hi there!");
    assert_eq!(response.finish_reason, "stop");

    mock.assert_async().await;
}
