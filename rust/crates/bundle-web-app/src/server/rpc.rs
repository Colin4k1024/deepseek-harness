//! JSON-RPC API handler — implements the Typert Gateway protocol that the TS
//! web frontend expects. Wire format: POST /api/<method> with body
//! { "type": "client-request", "rpcId": "...", "method": "...", "payload": ... }
//! and response { "type": "server-response", "rpcId": "...", "result": { "ok": true, "value": ... } }.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use dsh_core_session::event::SessionEvent;

use super::{
    AppState, create_runner, generate_session_id,
};

// ── Wire types ──────────────────────────────────────────────────────────

/// Client-to-server request envelope.
#[derive(Deserialize)]
pub struct ClientRequest {
    #[serde(rename = "type")]
    pub msg_type: String, // "client-request"
    #[serde(rename = "rpcId")]
    pub rpc_id: String,
    pub method: String,
    pub payload: Value,
}

/// Server-to-client response envelope.
#[derive(Serialize)]
pub struct ServerResponse {
    #[serde(rename = "type")]
    pub msg_type: &'static str, // "server-response"
    #[serde(rename = "rpcId")]
    pub rpc_id: String,
    pub result: RpcResultValue,
}

/// Business result: { ok: true, value: T } or { ok: false, error: ... }
#[derive(Serialize)]
#[serde(untagged)]
pub enum RpcResultValue {
    Ok { ok: bool, value: Value },
    Err { ok: bool, error: RpcErrorBody },
}

/// Error body with code + message + details.
#[derive(Serialize)]
pub struct RpcErrorBody {
    pub code: String,
    pub message: String,
    pub details: Value,
}

/// Per-session metadata stored in memory.
#[derive(Clone)]
pub struct SessionMeta {
    pub session_id: String,
    pub created_at: u64,
    pub provider: String,
    pub model: String,
}

/// Session events are stored as raw JSON values (serialized SessionEvent).
pub type SessionStore = Arc<Mutex<HashMap<String, (SessionMeta, Vec<Value>)>>>;

// ── Helpers ─────────────────────────────────────────────────────────────

fn ok_response(rpc_id: String, value: Value) -> ServerResponse {
    ServerResponse {
        msg_type: "server-response",
        rpc_id,
        result: RpcResultValue::Ok { ok: true, value },
    }
}

fn err_response(rpc_id: String, code: &str, message: String, details: Value) -> ServerResponse {
    ServerResponse {
        msg_type: "server-response",
        rpc_id,
        result: RpcResultValue::Err {
            ok: false,
            error: RpcErrorBody {
                code: code.to_string(),
                message,
                details,
            },
        },
    }
}

fn internal_err(rpc_id: String, msg: String) -> ServerResponse {
    err_response(rpc_id, "internal", msg, serde_json::json!({}))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Convert a Rust SessionEvent to a JSON value compatible with the TS frontend event format.
fn event_to_json(event: &SessionEvent) -> Value {
    // Serialize the event, then normalize the type field to TS convention.
    let mut v = serde_json::to_value(event).unwrap_or(Value::Null);
    // Normalize Rust enum variant names to TS event type convention (e.g. TurnStart -> turn/start)
    let normalized = v.get("type")
        .and_then(|t| t.as_str())
        .map(|typ| normalize_event_type(typ).to_string());
    if let (Some(norm), Some(obj)) = (normalized, v.as_object_mut()) {
        obj.insert("type".to_string(), Value::String(norm));
    }
    v
}

/// Normalize Rust SessionEvent variant names to TS frontend event type convention.
/// The TS frontend expects types like "turn/start", "user/message", "assistant/message", etc.
fn normalize_event_type(rust_type: &str) -> &str {
    match rust_type {
        // Turn lifecycle
        "TurnStart" => "turn/start",
        "TurnEnd" => "turn/end",
        // Messages
        "UserMessage" => "user/message",
        "AssistantMessage" => "assistant/message",
        "SystemMessage" => "system/message",
        // Tool calls
        "ToolCall" => "tool/call",
        "ToolResult" => "tool/result",
        // Reasoning
        "ReasoningStart" => "reasoning/start",
        "ReasoningDelta" => "reasoning/delta",
        "ReasoningEnd" => "reasoning/end",
        // Chunks
        "TextChunk" => "assistant/chunk",
        "ChunkEnd" => "assistant/chunk-end",
        // Compaction
        "CompactionSummary" => "compaction/summary",
        // Plan
        "PlanDraft" => "plan/draft",
        "PlanFinalized" => "plan/finalized",
        "PlanApproved" => "plan/approved",
        "PlanExecuting" => "plan/executing",
        "PlanCompleted" => "plan/completed",
        // Todo
        "TodoWrite" => "todo/write",
        // Session
        "SessionTitle" => "session/title",
        // Goal
        "GoalCreated" => "goal/created",
        "GoalCompleted" => "goal/completed",
        "GoalBlocked" => "goal/blocked",
        "GoalResumed" => "goal/resumed",
        "GoalPaused" => "goal/paused",
        "GoalEdited" => "goal/edited",
        // Fallback: keep the original type
        _ => rust_type,
    }
}

// ── Main API handler ────────────────────────────────────────────────────

/// Handle POST /api/{*path} — the main JSON-RPC entry point.
pub async fn api_handler(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(req): Json<ClientRequest>,
) -> impl IntoResponse {
    let method = if req.method.is_empty() { &path } else { &req.method };
    let rpc_id = req.rpc_id.clone();

    let result = dispatch(&state, method, req.payload).await;
    let response = match result {
        Ok(value) => ok_response(rpc_id, value),
        Err(e) => internal_err(rpc_id, e),
    };

    (StatusCode::OK, Json(response))
}

// ── Method dispatcher ───────────────────────────────────────────────────

async fn dispatch(state: &AppState, method: &str, payload: Value) -> Result<Value, String> {
    match method {
        "session.create" => handle_session_create(state, payload).await,
        "session.prompt" => handle_session_prompt(state, payload).await,
        "session.history" => handle_session_history(state, payload).await,
        "session.list" => handle_session_list(state, payload).await,
        "session.models" => handle_session_models(state, payload).await,
        "session.selectModel" => handle_session_select_model(state, payload).await,
        "session.cancel" => handle_session_cancel(state, payload).await,
        "session.rename" => handle_session_rename(state, payload).await,
        "session.fork" => handle_session_fork(state, payload).await,
        "session.updateQueue" => handle_session_update_queue(state, payload).await,
        "session.search" => handle_session_search(state, payload).await,
        "session.attachment" => handle_session_attachment(state, payload).await,
        "host.status" => handle_host_status(state, payload).await,
        "host.listSettings" => handle_host_list_settings(state, payload).await,
        "host.readSettings" => handle_host_read_settings(state, payload).await,
        "host.writeSettings" => handle_host_write_settings(state, payload).await,
        "workspace.list" => handle_workspace_list(state, payload).await,
        "workspace.create" => handle_workspace_create(state, payload).await,
        "workspace.read" => handle_workspace_read(state, payload).await,
        "workspace.write" => handle_workspace_write(state, payload).await,
        "workspace.move" => handle_workspace_move(state, payload).await,
        "workspace.delete" => handle_workspace_delete(state, payload).await,
        "goals.create" => handle_goals_create(state, payload).await,
        "goals.edit" => handle_goals_edit(state, payload).await,
        "goals.pause" => handle_goals_pause(state, payload).await,
        "goals.resume" => handle_goals_resume(state, payload).await,
        "goals.complete" => handle_goals_complete(state, payload).await,
        "goals.clear" => handle_goals_clear(state, payload).await,
        "commands.list" => handle_commands_list(state, payload).await,
        "commands.execute" => handle_commands_execute(state, payload).await,
        "subagent.list" => handle_subagent_list(state, payload).await,
        "subagent.prompt" => handle_subagent_prompt(state, payload).await,
        "subagent.cancel" => handle_subagent_cancel(state, payload).await,
        "subagent.interrupt" => handle_subagent_interrupt(state, payload).await,
        "subagent.resume" => handle_subagent_resume(state, payload).await,
        "questions.answer" => handle_questions_answer(state, payload).await,
        "approvals.review" => handle_approvals_review(state, payload).await,
        "downloads.list" => handle_downloads_list(state, payload).await,
        "downloads.open" => handle_downloads_open(state, payload).await,
        "events.subscribe" => handle_events_subscribe(state, payload).await,
        "agent-presets.list" => handle_agent_presets_list(state, payload).await,
        "agent-presets.read" => handle_agent_presets_read(state, payload).await,
        "skills.list" => handle_skills_list(state, payload).await,
        "credentials.list" => handle_credentials_list(state, payload).await,
        "dynamicCordisRunner.syncInspectManifest" => handle_dynamic_cordis_sync(state, payload).await,
        "dynamicCordisRunner.inventory" => handle_dynamic_cordis_inventory(state, payload).await,
        _ => Err(format!("unknown method: {}", method)),
    }
}

// ── Session handlers ────────────────────────────────────────────────────

async fn handle_session_create(state: &AppState, payload: Value) -> Result<Value, String> {
    let session_id = payload
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_session_id);
    let provider = payload
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("deepseek")
        .to_string();
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(&state.model)
        .to_string();

    let meta = SessionMeta {
        session_id: session_id.clone(),
        created_at: now_ms(),
        provider,
        model,
    };

    {
        let mut sessions = state.sessions_sdk.lock().unwrap();
        sessions.insert(session_id.clone(), (meta, Vec::new()));
    }

    Ok(serde_json::json!({
        "sessionId": session_id,
        "agentPreset": null
    }))
}

async fn handle_session_prompt(state: &AppState, payload: Value) -> Result<Value, String> {
    let session_id = payload
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or("missing sessionId")?
        .to_string();
    let content = payload
        .get("content")
        .ok_or("missing content")?;
    let mode = payload
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("queue");

    if mode == "steer" {
        return Ok(serde_json::json!({ "accepted": true }));
    }

    // Extract text from content parts
    let text = extract_text_from_content(content);

    // Get session metadata
    let (meta, events) = {
        let sessions = state.sessions_sdk.lock().unwrap();
        sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| format!("session not found: {}", session_id))?
    };

    // Create runner and import session log
    let llm = state.llm.clone();
    let model = meta.model.clone();
    let mut runner = create_runner(llm, &model);

    if !events.is_empty() {
        let json = serde_json::to_string(&events).map_err(|e| e.to_string())?;
        // Reconstruct SessionEvent vec from the stored JSON
        let log: Vec<SessionEvent> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        if !log.is_empty() {
            let log_json = serde_json::to_string(&log).map_err(|e| e.to_string())?;
            runner.import_session(&log_json).map_err(|e| e.to_string())?;
        }
    }

    // Run the agent turn
    let _response = runner
        .run_turn(text)
        .await
        .map_err(|e| e.to_string())?;

    // Save updated session log as raw JSON values (NOT normalized — normalization happens at read time)
    let new_events: Vec<Value> = runner
        .session_log()
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
        .collect();

    {
        let mut sessions = state.sessions_sdk.lock().unwrap();
        if let Some((_, stored_events)) = sessions.get_mut(&session_id) {
            *stored_events = new_events;
        }
    }

    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_session_history(state: &AppState, payload: Value) -> Result<Value, String> {
    let session_id = payload
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or("missing sessionId")?
        .to_string();

    let events = {
        let sessions = state.sessions_sdk.lock().unwrap();
        sessions
            .get(&session_id)
            .map(|(_, events)| events.clone())
            .unwrap_or_default()
    };

    let history_entries: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut v = e.clone();
            // Normalize the event type to TS frontend convention
            if let Some(typ) = v.get("type").and_then(|t| t.as_str()) {
                let normalized = normalize_event_type(typ).to_string();
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("type".to_string(), Value::String(normalized));
                }
            }
            serde_json::json!({ "event": v })
        })
        .collect();

    Ok(serde_json::json!({
        "events": history_entries,
        "hasMore": false
    }))
}

async fn handle_session_list(state: &AppState, _payload: Value) -> Result<Value, String> {
    let sessions = {
        let sessions = state.sessions_sdk.lock().unwrap();
        sessions
            .iter()
            .map(|(id, (meta, events))| {
                let blank = events.is_empty();
                let event_count = events.len();
                serde_json::json!({
                    "sessionId": id,
                    "updatedAt": meta.created_at,
                    "running": false,
                    "blank": blank,
                    "agentPreset": null,
                    "projections": {
                        "asOfSeq": if event_count > 0 { event_count as i64 - 1 } else { -1_i64 },
                        "values": {
                            "sessionListMetadata": {
                                "blank": blank,
                                "lastPromptAt": null
                            }
                        }
                    }
                })
            })
            .collect::<Vec<_>>()
    };

    Ok(serde_json::json!({ "items": sessions }))
}

async fn handle_session_models(state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({
        "current": {
            "provider": "deepseek",
            "model": &state.model
        },
        "routable": true,
        "groups": [{
            "id": "deepseek",
            "name": "DeepSeek",
            "models": [{
                "id": "deepseek-chat",
                "name": "DeepSeek Chat",
                "description": "DeepSeek Chat model"
            }, {
                "id": "deepseek-reasoner",
                "name": "DeepSeek Reasoner",
                "description": "DeepSeek Reasoner model"
            }]
        }],
        "failures": []
    }))
}

async fn handle_session_select_model(state: &AppState, payload: Value) -> Result<Value, String> {
    let session_id = payload
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or("missing sessionId")?
        .to_string();
    let provider = payload
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("deepseek");
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("deepseek-chat");

    {
        let mut sessions = state.sessions_sdk.lock().unwrap();
        if let Some((meta, _)) = sessions.get_mut(&session_id) {
            meta.provider = provider.to_string();
            meta.model = model.to_string();
        }
    }

    Ok(serde_json::json!({
        "selected": {
            "provider": provider,
            "model": model
        }
    }))
}

async fn handle_session_cancel(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_session_rename(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "title": "Session", "seq": 0 }))
}

async fn handle_session_fork(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("session.fork is not supported".to_string())
}

async fn handle_session_update_queue(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_session_search(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [], "hasMore": false }))
}

async fn handle_session_attachment(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("session.attachment is not supported".to_string())
}

// ── Host handlers ───────────────────────────────────────────────────────

async fn handle_host_status(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({
        "version": "0.1.0-rust",
        "status": "ok",
        "platform": std::env::consts::OS,
        "nodeVersion": "rust",
        "uptime": now_secs()
    }))
}

async fn handle_host_list_settings(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "namespaces": [] }))
}

async fn handle_host_read_settings(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "data": {}, "revision": 0 }))
}

async fn handle_host_write_settings(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true, "revision": 1 }))
}

// ── Workspace handlers ──────────────────────────────────────────────────

async fn handle_workspace_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

async fn handle_workspace_create(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("workspace.create is not supported".to_string())
}

async fn handle_workspace_read(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("workspace.read is not supported".to_string())
}

async fn handle_workspace_write(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("workspace.write is not supported".to_string())
}

async fn handle_workspace_move(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("workspace.move is not supported".to_string())
}

async fn handle_workspace_delete(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("workspace.delete is not supported".to_string())
}

// ── Goal handlers ───────────────────────────────────────────────────────

async fn handle_goals_create(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "goalId": "goal-rust-1", "revision": 1 }))
}

async fn handle_goals_edit(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_goals_pause(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_goals_resume(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_goals_complete(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_goals_clear(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

// ── Command handlers ────────────────────────────────────────────────────

async fn handle_commands_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "commands": [] }))
}

async fn handle_commands_execute(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("commands.execute is not supported".to_string())
}

// ── Subagent handlers ───────────────────────────────────────────────────

async fn handle_subagent_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

async fn handle_subagent_prompt(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("subagent.prompt is not supported".to_string())
}

async fn handle_subagent_cancel(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_subagent_interrupt(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_subagent_resume(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("subagent.resume is not supported".to_string())
}

// ── Question/Approval handlers ──────────────────────────────────────────

async fn handle_questions_answer(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

async fn handle_approvals_review(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "accepted": true }))
}

// ── Download handlers ───────────────────────────────────────────────────

async fn handle_downloads_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

async fn handle_downloads_open(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("downloads.open is not supported".to_string())
}

// ── Event handlers ──────────────────────────────────────────────────────

async fn handle_events_subscribe(_state: &AppState, _payload: Value) -> Result<Value, String> {
    // SSE subscription is not supported in this minimum implementation.
    Ok(serde_json::json!({ "subscribed": true }))
}

// ── Agent preset handlers ───────────────────────────────────────────────

async fn handle_agent_presets_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

async fn handle_agent_presets_read(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Err("agent-presets.read is not supported".to_string())
}

// ── Skill handlers ──────────────────────────────────────────────────────

async fn handle_skills_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

// ── Credential handlers ─────────────────────────────────────────────────

async fn handle_credentials_list(_state: &AppState, _payload: Value) -> Result<Value, String> {
    Ok(serde_json::json!({ "items": [] }))
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Extract text from content blocks (TS format: [{ type: "text", text: "..." }, ...]).
fn extract_text_from_content(content: &Value) -> String {
    if let Some(arr) = content.as_array() {
        let mut text = String::new();
        for part in arr {
            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
        if !text.is_empty() {
            return text;
        }
    }
    // Fallback: try as raw string
    content.as_str().unwrap_or("").to_string()
}

// ── Dynamic Cordis Runner stubs ─────────────────────────────────────────

async fn handle_dynamic_cordis_sync(_state: &AppState, _payload: Value) -> Result<Value, String> {
    // Stub: dynamic cordis plugins are not supported in the Rust backend
    Ok(serde_json::json!({ "manifests": {} }))
}

async fn handle_dynamic_cordis_inventory(_state: &AppState, _payload: Value) -> Result<Value, String> {
    // Stub: dynamic cordis plugins are not supported in the Rust backend
    Ok(serde_json::json!({ "plugins": [], "packages": {} }))
}