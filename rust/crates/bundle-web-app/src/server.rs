//! Web server — axum-based HTTP API with SSE streaming.
//!
//! Endpoints:
//! - `GET /health` — Health check
//! - `POST /api/*path` — JSON-RPC Typert Gateway protocol (the main web frontend API)
//! - `POST /api/v1/chat` — Legacy non-streaming agent turn
//! - `POST /api/v1/chat/stream` — Legacy streaming agent turn
//! - `POST /api/v1/llm/complete` — Raw LLM completion
//! - `POST /api/v1/llm/stream` — Raw LLM streaming

pub mod rpc;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, Query, State, ws::{Message as WsMessage, WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_session::event::SessionEvent;
use dsh_llm_adapter::{LlmAdapter, LlmRequest, Message, Role};
use dsh_seam_tools::{
    BashTool, CompactionTool, EditFileTool, FileReadTool, FileWriteTool, ListFilesTool,
    LspTool, PlanTool, SearchTool, TodoTool, WebFetchTool, WebSearchTool,
};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub llm: Arc<dyn LlmAdapter>,
    pub model: String,
    pub sessions: Arc<Mutex<HashMap<String, Vec<SessionEvent>>>>,
    pub sessions_sdk: rpc::SessionStore,
    pub plugin_dir: Option<String>,
}

/// System prompt that tells the agent about available tools.
pub const SYSTEM_PROMPT: &str = r#"You are an AI coding agent powered by DeepSeek, running on the DeepSeek Harness (Rust).

You have access to tools. Use them to accomplish tasks: read files before editing, execute commands with bash, search the web for current information, and create plans for complex work.

Available tools:
- bash — Execute shell commands
- file_read — Read file contents
- file_write — Create or overwrite a file
- edit_file — Make targeted edits to a file
- list_files — List files in a directory
- search — Search for patterns in files using regex
- web_search — Search the web
- web_fetch — Fetch a web page
- plan — Create and manage execution plans
- todo_write — Track tasks with a todo list
- compact — Analyze token usage
- lsp — Code intelligence from language servers

Always use tools when they would help. Execute commands and file operations directly — do not describe how to do them."#;

/// Create a fully configured AgentRunner with all tools and system prompt.
pub fn create_runner(llm: Arc<dyn LlmAdapter>, model: &str) -> AgentRunner {
    let mut runner = AgentRunner::new(llm, model.to_string())
        .with_system_prompt(SYSTEM_PROMPT.to_string())
        .with_max_context_tokens(128000)
        .with_max_turns(50)
        .with_max_iterations(30);

    // Core tools
    runner.register_tool(Arc::new(BashTool::new()));
    runner.register_tool(Arc::new(FileReadTool));
    runner.register_tool(Arc::new(FileWriteTool));
    runner.register_tool(Arc::new(EditFileTool));
    runner.register_tool(Arc::new(SearchTool));
    runner.register_tool(Arc::new(ListFilesTool));

    // Web tools
    runner.register_tool(Arc::new(WebSearchTool::new()));
    runner.register_tool(Arc::new(WebFetchTool::new()));

    // Meta tools
    runner.register_tool(Arc::new(PlanTool::new()));
    runner.register_tool(Arc::new(TodoTool::new()));
    runner.register_tool(Arc::new(CompactionTool::new()));
    runner.register_tool(Arc::new(LspTool::new()));

    runner
}

/// Generate a unique session ID.
pub fn generate_session_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sess-{:x}", ts)
}

/// Extract tool call entries from the session log.
pub fn extract_tool_calls(log: &[SessionEvent]) -> Vec<ToolCallEntry> {
    let mut entries = Vec::new();
    let mut pending: HashMap<String, (String, String)> = HashMap::new();

    for event in log {
        match event {
            SessionEvent::ToolCall {
                call_id, name, arguments, ..
            } => {
                pending.insert(
                    call_id.clone(),
                    (name.clone(), arguments.clone()),
                );
            }
            SessionEvent::ToolResult {
                call_id,
                content,
                is_error,
                ..
            } => {
                if let Some((name, arguments)) = pending.remove(call_id) {
                    let preview = if content.len() > 200 {
                        format!("{}...", &content[..197])
                    } else {
                        content.clone()
                    };
                    entries.push(ToolCallEntry {
                        name,
                        arguments,
                        result_preview: preview,
                        is_error: *is_error,
                    });
                }
            }
            _ => {}
        }
    }

    entries
}

/// Chat request body.
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

fn default_model() -> String {
    "deepseek-chat".into()
}

/// Non-streaming chat response.
#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub session_events: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ToolCallEntry>,
}

/// A single tool call entry for the response.
#[derive(Serialize)]
pub struct ToolCallEntry {
    pub name: String,
    pub arguments: String,
    pub result_preview: String,
    pub is_error: bool,
}

/// SSE event data for streaming.
#[derive(Serialize)]
pub struct StreamEvent {
    pub event: String, // "chunk", "done", "error"
    pub data: String,
}

/// Error response.
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Raw LLM completion request.
#[derive(Deserialize)]
pub struct LlmCompleteRequest {
    pub messages: Vec<LlmMessage>,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub system: Option<String>,
}

/// A single message in LLM format.
#[derive(Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Raw LLM completion response.
#[derive(Serialize)]
pub struct LlmCompleteResponse {
    pub content: String,
    pub finish_reason: String,
    pub usage: Option<LlmUsageInfo>,
}

/// Token usage info.
#[derive(Serialize)]
pub struct LlmUsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).unwrap();
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            body,
        )
            .into_response()
    }
}

/// Build the axum Router with all routes.
pub fn build_router(state: AppState, static_dir: Option<&str>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/api/events.mux", get(ws_events_handler))
        .route("/api/events.host", get(ws_events_handler))
        .route("/plugins/events", get(ws_events_handler))
        .route("/api/v1/chat", post(chat))
        .route("/api/v1/chat", get(chat_get))
        .route("/api/v1/chat/stream", post(chat_stream))
        .route("/api/v1/llm/complete", post(llm_complete))
        .route("/api/v1/llm/stream", post(llm_stream))
        .route("/api/{*path}", post(rpc::api_handler))
        .route("/plugins/{*path}", get(serve_plugin));

    if let Some(dir) = static_dir {
        let serve_dir = tower_http::services::ServeDir::new(dir)
            .fallback(tower_http::services::ServeFile::new(format!("{}/index.html", dir)));
        router = router.fallback_service(serve_dir);
    }

    router.layer(CorsLayer::permissive()).with_state(state)
}

/// Serve client plugin files from the DSH installation's node_modules.
async fn serve_plugin(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    // path is like "@deepseek-ai/dsh-client-modules/client.js"
    // Split at the last '/' — everything before is the package name, after is the file name
    let last_slash = match path.rfind('/') {
        Some(pos) => pos,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let package = &path[..last_slash];
    let file = &path[last_slash + 1..];

    // Try the main plugin_dir first
    if let Some(dir) = &state.plugin_dir {
        let file_path = format!("{}/node_modules/{}/lib/{}", dir, package, file);
        if let Ok(content) = tokio::fs::read(&file_path).await {
            return serve_js_file(file, content);
        }
    }

    // Fallback: try the stub directory
    let stub_path = format!("/tmp/dsh-stubs/node_modules/{}/lib/{}", package, file);
    if let Ok(content) = tokio::fs::read(&stub_path).await {
        return serve_js_file(file, content);
    }

    StatusCode::NOT_FOUND.into_response()
}

fn serve_js_file(file: &str, content: Vec<u8>) -> Response {
    let content_type = match file.rsplit('.').next() {
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("map") => "application/json",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("html") => "text/html",
        _ => "application/octet-stream",
    };
    Response::builder()
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(axum::body::Body::from(content))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// WebSocket handler for event streaming endpoints.
async fn ws_events_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_events(socket))
}

async fn handle_ws_events(mut socket: WebSocket) {
    // Keep the connection alive with periodic pings
    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(WsMessage::Ping(data))) => {
                        let _ = socket.send(WsMessage::Pong(data)).await;
                    }
                    Some(Ok(_)) => {} // Ignore text/binary messages
                    Some(Err(_)) => break,
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                if socket.send(WsMessage::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Health check endpoint.
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Non-streaming chat endpoint.
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let llm = state.llm.clone();
    let model = if req.model.is_empty() {
        state.model.clone()
    } else {
        req.model
    };

    let session_id = req.session_id.clone().unwrap_or_else(generate_session_id);
    let mut runner = create_runner(llm, &model);

    // Import existing session log if available
    {
        let sessions = state.sessions.lock().unwrap();
        if let Some(log) = sessions.get(&session_id) {
            if !log.is_empty() {
                let json = serde_json::to_string(log).map_err(|e| ErrorResponse {
                    error: format!("serialize session: {e}"),
                })?;
                runner.import_session(&json).map_err(|e| ErrorResponse {
                    error: e.to_string(),
                })?;
            }
        }
    }

    let response = runner
        .run_turn(req.message)
        .await
        .map_err(|e| ErrorResponse {
            error: e.to_string(),
        })?;

    let tool_calls = extract_tool_calls(runner.session_log());
    let event_count = runner.session_log().len();

    // Save session log
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), runner.session_log().to_vec());
    }

    Ok(Json(ChatResponse {
        response,
        session_events: event_count,
        session_id: Some(session_id),
        tool_calls,
    }))
}

/// Non-streaming chat endpoint (GET with query parameter).
async fn chat_get(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let message = params.get("message").cloned().unwrap_or_default();
    let model = params
        .get("model")
        .cloned()
        .unwrap_or_else(|| state.model.clone());
    let session_id = params
        .get("session")
        .cloned()
        .unwrap_or_else(generate_session_id);

    let llm = state.llm.clone();
    let mut runner = create_runner(llm, &model);

    // Import existing session log
    {
        let sessions = state.sessions.lock().unwrap();
        if let Some(log) = sessions.get(&session_id) {
            if !log.is_empty() {
                let json = serde_json::to_string(log).map_err(|e| ErrorResponse {
                    error: format!("serialize session: {e}"),
                })?;
                runner.import_session(&json).map_err(|e| ErrorResponse {
                    error: e.to_string(),
                })?;
            }
        }
    }

    let response = runner
        .run_turn(message)
        .await
        .map_err(|e| ErrorResponse {
            error: e.to_string(),
        })?;

    let tool_calls = extract_tool_calls(runner.session_log());
    let event_count = runner.session_log().len();

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), runner.session_log().to_vec());
    }

    Ok(Json(ChatResponse {
        response,
        session_events: event_count,
        session_id: Some(session_id),
        tool_calls,
    }))
}

/// Streaming chat endpoint (SSE).
async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let llm = state.llm.clone();
    let model = if req.model.is_empty() {
        state.model.clone()
    } else {
        req.model
    };
    let session_id = req.session_id.clone().unwrap_or_else(generate_session_id);

    let stream = async_stream::stream! {
        let mut runner = create_runner(llm, &model);

        // Import existing session log
        {
            let sessions = state.sessions.lock().unwrap();
            if let Some(log) = sessions.get(&session_id) {
                if !log.is_empty() {
                    if let Ok(json) = serde_json::to_string(log) {
                        let _ = runner.import_session(&json);
                    }
                }
            }
        }

        match runner.run_turn(req.message).await {
            Ok(response) => {
                let tool_calls = extract_tool_calls(runner.session_log());
                let event_count = runner.session_log().len();

                // Save session log
                {
                    let mut sessions = state.sessions.lock().unwrap();
                    sessions.insert(session_id.clone(), runner.session_log().to_vec());
                }

                // Send meta event with session_id and tool_calls
                let meta = serde_json::json!({
                    "event": "meta",
                    "data": {
                        "session_id": session_id,
                        "session_events": event_count,
                        "tool_calls": tool_calls,
                    }
                });
                if let Ok(data) = serde_json::to_string(&meta) {
                    yield Ok(Event::default().data(data));
                }

                // Stream the final response
                let event = StreamEvent {
                    event: "chunk".into(),
                    data: response,
                };
                if let Ok(data) = serde_json::to_string(&event) {
                    yield Ok(Event::default().data(data));
                }

                let done = StreamEvent {
                    event: "done".into(),
                    data: format!("{} events", event_count),
                };
                if let Ok(data) = serde_json::to_string(&done) {
                    yield Ok(Event::default().data(data));
                }
            }
            Err(e) => {
                let error = StreamEvent {
                    event: "error".into(),
                    data: e.to_string(),
                };
                if let Ok(data) = serde_json::to_string(&error) {
                    yield Ok(Event::default().data(data));
                }
            }
        }
    };

    Sse::new(stream)
}

/// Raw LLM completion (no agent loop).
async fn llm_complete(
    State(state): State<AppState>,
    Json(req): Json<LlmCompleteRequest>,
) -> Result<Json<LlmCompleteResponse>, ErrorResponse> {
    let model = if req.model.is_empty() {
        state.model.clone()
    } else {
        req.model
    };

    // Build messages from the request
    let mut messages: Vec<Message> = req
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "system" => Role::System,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                _ => Role::User,
            };
            Message {
                role,
                content: m.content,
                tool_calls: None,
                tool_call_id: None,
            }
        })
        .collect();

    // Prepend system message if provided
    if let Some(system) = req.system {
        if !system.is_empty() {
            messages.insert(
                0,
                Message {
                    role: Role::System,
                    content: system,
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }
    }

    let llm_request = LlmRequest {
        model,
        messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        stream: false,
        tools: None,
    };

    let response = state.llm.complete(llm_request).await.map_err(|e| ErrorResponse {
        error: e.to_string(),
    })?;

    Ok(Json(LlmCompleteResponse {
        content: response.content,
        finish_reason: response.finish_reason,
        usage: response.usage.map(|u| LlmUsageInfo {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }),
    }))
}

/// Raw LLM streaming (SSE, no agent loop).
async fn llm_stream(
    State(state): State<AppState>,
    Json(req): Json<LlmCompleteRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let model = if req.model.is_empty() {
        state.model.clone()
    } else {
        req.model
    };

    // Build messages
    let mut messages: Vec<Message> = req
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "system" => Role::System,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                _ => Role::User,
            };
            Message {
                role,
                content: m.content,
                tool_calls: None,
                tool_call_id: None,
            }
        })
        .collect();

    if let Some(system) = req.system {
        if !system.is_empty() {
            messages.insert(
                0,
                Message {
                    role: Role::System,
                    content: system,
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }
    }

    let llm_request = LlmRequest {
        model,
        messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        stream: true,
        tools: None,
    };

    let stream = async_stream::stream! {
        match state.llm.stream(llm_request).await {
            Ok(mut chunk_stream) => {
                while let Some(chunk_result) = chunk_stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            let event = serde_json::json!({
                                "event": "chunk",
                                "data": {
                                    "delta": chunk.delta,
                                    "finish_reason": chunk.finish_reason,
                                }
                            });
                            if let Ok(data) = serde_json::to_string(&event) {
                                yield Ok(Event::default().data(data));
                            }
                        }
                        Err(e) => {
                            let event = serde_json::json!({
                                "event": "error",
                                "data": e.to_string()
                            });
                            if let Ok(data) = serde_json::to_string(&event) {
                                yield Ok(Event::default().data(data));
                            }
                            break;
                        }
                    }
                }
                // Send done event
                let done = serde_json::json!({"event": "done", "data": ""});
                if let Ok(data) = serde_json::to_string(&done) {
                    yield Ok(Event::default().data(data));
                }
            }
            Err(e) => {
                let error = serde_json::json!({
                    "event": "error",
                    "data": e.to_string()
                });
                if let Ok(data) = serde_json::to_string(&error) {
                    yield Ok(Event::default().data(data));
                }
            }
        }
    };

    Sse::new(stream)
}
