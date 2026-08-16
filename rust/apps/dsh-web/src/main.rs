//! dsh-web — HTTP API server for DeepSeek Harness.
//!
//! Serves the agent API via axum with SSE streaming support,
//! plus the Typert Gateway JSON-RPC /api endpoint for the web frontend.

use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;

use clap::Parser;

use dsh_bundle_web_app::{AppState, build_router};
use dsh_llm_deepseek::DeepSeekAdapter;

#[derive(Parser, Debug)]
#[command(name = "dsh-web", about = "DeepSeek Harness — HTTP API server")]
struct Args {
    /// Listen address
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Listen port
    #[arg(long, default_value = "3000")]
    port: u16,

    /// LLM model name
    #[arg(short, long, default_value = "deepseek-chat")]
    model: String,

    /// DeepSeek API key (or set DEEPSEEK_API_KEY env var)
    #[arg(long, env = "DEEPSEEK_API_KEY")]
    api_key: String,

    /// Static file directory for the web frontend
    #[arg(long, default_value = "")]
    static_dir: String,

    /// DSH installation directory for serving plugin files
    #[arg(long, default_value = "")]
    plugin_dir: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let llm = Arc::new(DeepSeekAdapter::new(args.api_key));
    let plugin_dir: Option<String> = if args.plugin_dir.is_empty() {
        None
    } else {
        Some(args.plugin_dir)
    };

    let state = AppState {
        llm,
        model: args.model,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        sessions_sdk: Arc::new(Mutex::new(HashMap::new())),
        plugin_dir,
    };

    let static_dir: Option<&str> = if args.static_dir.is_empty() {
        None
    } else {
        Some(&args.static_dir)
    };

    let app = build_router(state, static_dir);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!(addr = %addr, "dsh-web starting");
    println!("dsh-web listening on {addr}");
    println!("  POST /api/{{*path}}       — Typert Gateway JSON-RPC (web frontend API)");
    println!("  POST /api/v1/chat        — non-streaming agent turn");
    println!("  POST /api/v1/chat/stream  — SSE streaming agent turn");
    println!("  GET  /health              — health check");

    axum::serve(listener, app).await.unwrap();
}
