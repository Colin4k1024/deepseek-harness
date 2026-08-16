//! Web server — axum-based HTTP API with SSE streaming.
//!
//! Endpoints:
//! - `GET /health` — Health check
//! - `POST /api/v1/chat` — Non-streaming agent turn (JSON response)
//! - `POST /api/v1/chat/stream` — Streaming agent turn (SSE events)

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use dsh_core_agent_loop::AgentRunner;
use dsh_llm_adapter::LlmAdapter;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub llm: Arc<dyn LlmAdapter>,
    pub model: String,
}

/// Chat request body.
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    "deepseek-chat".into()
}

/// Non-streaming chat response.
#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub session_events: usize,
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
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/chat", post(chat))
        .route("/api/v1/chat/stream", post(chat_stream))
        .layer(CorsLayer::permissive())
        .with_state(state)
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

    let mut runner = AgentRunner::new(llm, model);
    let response = runner
        .run_turn(req.message)
        .await
        .map_err(|e| ErrorResponse {
            error: e.to_string(),
        })?;

    Ok(Json(ChatResponse {
        response,
        session_events: runner.session_log().len(),
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

    let stream = async_stream::stream! {
        let mut runner = AgentRunner::new(llm, model);

        // We need to drive the agent loop manually for streaming.
        // For now, run the full turn and stream events as they complete.
        match runner.run_turn(req.message).await {
            Ok(response) => {
                // Stream the final response as a single chunk event
                let event = StreamEvent {
                    event: "chunk".into(),
                    data: response,
                };
                if let Ok(data) = serde_json::to_string(&event) {
                    yield Ok(Event::default().data(data));
                }

                // Send done event
                let done = StreamEvent {
                    event: "done".into(),
                    data: format!("{} events", runner.session_log().len()),
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
