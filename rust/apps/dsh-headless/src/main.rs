//! dsh-headless — Interactive CLI agent runner.
//!
//! Supports two modes:
//! - Single-turn: `dsh --task "prompt"` (existing behavior)
//! - Interactive: `dsh --interactive` for multi-turn conversation

use std::io::{self, Write};
use std::sync::Arc;

use clap::Parser;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_session::event::SessionEvent;
use dsh_core_session::persist;
use dsh_llm_adapter::LlmAdapter;
use dsh_llm_deepseek::DeepSeekAdapter;
use dsh_seam_sandbox::{SandboxPolicy, SandboxResolution, resolve as resolve_sandbox};
use dsh_seam_tools::{
    BashTool, EditFileTool, FileReadTool, FileWriteTool, ListFilesTool, SearchTool,
};

#[derive(Parser, Debug)]
#[command(name = "dsh", about = "DeepSeek Harness — interactive agent runner")]
struct Args {
    /// Task prompt to execute (single-turn mode)
    #[arg(short, long)]
    task: Option<String>,

    /// Interactive multi-turn conversation mode
    #[arg(short, long)]
    interactive: bool,

    /// Session log output path
    #[arg(short, long, default_value = "session.jsonl")]
    output: String,

    /// LLM model name
    #[arg(short, long, default_value = "deepseek-chat")]
    model: String,

    /// DeepSeek API key (or set DEEPSEEK_API_KEY env var)
    #[arg(long, env = "DEEPSEEK_API_KEY")]
    api_key: String,

    /// System prompt
    #[arg(long)]
    system_prompt: Option<String>,

    /// Max context tokens
    #[arg(long, default_value = "128000")]
    max_context_tokens: usize,

    /// Max turns per conversation
    #[arg(long, default_value = "50")]
    max_turns: usize,

    /// Sandbox policy level
    #[arg(long, value_enum, default_value = "minimal")]
    sandbox: SandboxArg,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SandboxArg {
    Minimal,
    Confined,
    Strict,
}

impl From<SandboxArg> for SandboxPolicy {
    fn from(arg: SandboxArg) -> Self {
        match arg {
            SandboxArg::Minimal => SandboxPolicy::Minimal,
            SandboxArg::Confined => SandboxPolicy::Confined,
            SandboxArg::Strict => SandboxPolicy::Strict,
        }
    }
}

fn create_runner(args: &Args) -> AgentRunner {
    let llm: Arc<dyn LlmAdapter> = Arc::new(DeepSeekAdapter::new(args.api_key.clone()));

    let mut runner = AgentRunner::new(llm, args.model.clone())
        .with_max_context_tokens(args.max_context_tokens)
        .with_max_turns(args.max_turns);

    if let Some(ref prompt) = args.system_prompt {
        runner = runner.with_system_prompt(prompt.clone());
    }

    // Register real tools
    runner.register_tool(Arc::new(BashTool::new()));
    runner.register_tool(Arc::new(FileReadTool));
    runner.register_tool(Arc::new(FileWriteTool));
    runner.register_tool(Arc::new(EditFileTool));
    runner.register_tool(Arc::new(SearchTool));
    runner.register_tool(Arc::new(ListFilesTool));

    runner
}

fn print_tool_call(name: &str, args: &str) {
    let args_val: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
    println!("\n  \x1b[33m⚡ {name}\x1b[0m");
    if let Some(obj) = args_val.as_object() {
        for (k, v) in obj {
            let val = match v {
                serde_json::Value::String(s) => {
                    if s.len() > 80 {
                        format!("{}...", &s[..77])
                    } else {
                        s.clone()
                    }
                }
                _ => format!("{v}"),
            };
            println!("    \x1b[90m{k}: {val}\x1b[0m");
        }
    }
}

fn print_tool_result(content: &str, is_error: bool) {
    let color = if is_error { "\x1b[31m" } else { "\x1b[32m" };
    let preview = if content.len() > 200 {
        format!("{}...", &content[..197])
    } else {
        content.to_string()
    };
    println!("  {color}→ {preview}\x1b[0m");
}

async fn run_single_turn(runner: &mut AgentRunner, task: &str) -> Result<String, String> {
    println!("\x1b[36m╭─ Agent ─────────────────────────────────\x1b[0m");

    let response = runner
        .run_turn(task.to_string())
        .await
        .map_err(|e| e.to_string())?;

    // Print tool calls from session log
    for event in runner.session_log() {
        match event {
            SessionEvent::ToolCall {
                name, arguments, ..
            } => {
                print_tool_call(name, arguments);
            }
            SessionEvent::ToolResult { content, .. } => {
                print_tool_result(content, false);
            }
            _ => {}
        }
    }

    println!("\x1b[36m│\x1b[0m {response}");
    println!("\x1b[36m╰─────────────────────────────────────────\x1b[0m");

    Ok(response)
}

async fn run_interactive(runner: &mut AgentRunner, output_path: &str) {
    println!("\x1b[36m╔════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[36m║   DeepSeek Harness — Interactive Mode  ║\x1b[0m");
    println!("\x1b[36m║   Type 'exit' or 'quit' to leave       ║\x1b[0m");
    println!("\x1b[36m╚════════════════════════════════════════╝\x1b[0m");
    println!();

    let stdin = io::stdin();
    let mut turn = 0;

    loop {
        turn += 1;
        print!("\x1b[32m[{}]> \x1b[0m", turn);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match stdin.read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("error reading input: {e}");
                break;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            println!("goodbye!");
            break;
        }

        match runner.run_turn(input.to_string()).await {
            Ok(response) => {
                // Print tool calls
                for event in runner.session_log() {
                    match event {
                        SessionEvent::ToolCall {
                            name, arguments, ..
                        } => {
                            print_tool_call(name, arguments);
                        }
                        SessionEvent::ToolResult { content, .. } => {
                            print_tool_result(content, false);
                        }
                        _ => {}
                    }
                }

                println!("\n{response}");
                let usage = runner.token_usage();
                println!(
                    "\x1b[90m[tokens: prompt={}, completion={}, total={}]\x1b[0m\n",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                );
            }
            Err(e) => {
                eprintln!("\x1b[31merror: {e}\x1b[0m\n");
            }
        }
    }

    // Write session log
    let events = runner.session_log();
    let path = std::path::Path::new(output_path);
    for event in events {
        if let Err(e) = persist::append_event(path, event).await {
            eprintln!("failed to write session log: {e}");
        }
    }
    println!("session log written to {output_path}");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // Resolve sandbox
    let policy: SandboxPolicy = args.sandbox.clone().into();
    match resolve_sandbox(policy) {
        SandboxResolution::Confined(backend) => {
            tracing::info!(backend = backend.name(), "sandbox available");
        }
        SandboxResolution::Unavailable { requested } => {
            tracing::warn!(policy = ?requested, "sandbox unavailable — running unsandboxed");
        }
    }

    let mut runner = create_runner(&args);

    if args.interactive {
        run_interactive(&mut runner, &args.output).await;
    } else if let Some(ref task) = args.task {
        match run_single_turn(&mut runner, task).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }

        // Write session log
        let events = runner.session_log();
        let path = std::path::Path::new(&args.output);
        for event in events {
            if let Err(e) = persist::append_event(path, event).await {
                eprintln!("failed to write session log: {e}");
                std::process::exit(1);
            }
        }
        tracing::info!(path = %args.output, events = events.len(), "session log written");
    } else {
        eprintln!("error: either --task or --interactive is required");
        std::process::exit(1);
    }
}
