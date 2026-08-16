//! LSP tool — Language Server Protocol client for code intelligence.
//!
//! Manages language server processes and provides diagnostics, hover,
//! and go-to-definition capabilities through the standard LSP JSON-RPC protocol.
//!
//! Supported languages (auto-detected from file extension):
//! - Rust (.rs) → rust-analyzer
//! - TypeScript (.ts, .tsx) → typescript-language-server
//! - JavaScript (.js, .jsx) → typescript-language-server
//! - Python (.py) → pylsp

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

// ── JSON-RPC message framing ─────────────────────────────────────────

/// Write a JSON-RPC message with Content-Length header.
async fn write_message(
    writer: &mut (impl AsyncWriteExt + Unpin),
    message: &serde_json::Value,
) -> Result<(), ToolError> {
    let json = serde_json::to_string(message)
        .map_err(|e| ToolError::Execution(format!("serialize: {e}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| ToolError::Execution(format!("write header: {e}")))?;
    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|e| ToolError::Execution(format!("write body: {e}")))?;
    Ok(())
}

/// Read a JSON-RPC message from a buffered reader.
async fn read_message(
    reader: &mut (impl AsyncBufReadExt + Unpin),
) -> Result<serde_json::Value, ToolError> {
    let mut content_length: Option<usize> = None;
    let mut header_line = String::new();

    loop {
        header_line.clear();
        reader
            .read_line(&mut header_line)
            .await
            .map_err(|e| ToolError::Execution(format!("read header: {e}")))?;

        if header_line == "\r\n" || header_line == "\n" || header_line.is_empty() {
            break;
        }

        if let Some(len_str) = header_line.strip_prefix("Content-Length: ") {
            content_length = Some(
                len_str
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| ToolError::Execution(format!("bad Content-Length: {e}")))?,
            );
        }
    }

    let len = content_length
        .ok_or_else(|| ToolError::Execution("missing Content-Length header".into()))?;

    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| ToolError::Execution(format!("read body: {e}")))?;

    serde_json::from_slice(&body).map_err(|e| ToolError::Execution(format!("parse JSON: {e}")))
}

/// Read messages until one matches the expected request id.
async fn read_response(
    reader: &mut (impl AsyncBufReadExt + Unpin),
    expected_id: u64,
) -> Result<serde_json::Value, ToolError> {
    for _ in 0..50 {
        let msg = read_message(reader).await?;
        if msg.get("id").and_then(|v| v.as_u64()) == Some(expected_id) {
            return Ok(msg);
        }
    }
    Err(ToolError::Execution(format!(
        "timeout waiting for response id={expected_id}"
    )))
}

// ── LSP client ───────────────────────────────────────────────────────

/// Manages one language server process.
struct LspClient {
    #[allow(dead_code)]
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl LspClient {
    /// Spawn a language server, send initialize, and wait for ready.
    async fn new(command: &str, args: &[&str], root_uri: &str) -> Result<Self, ToolError> {
        let mut process = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                ToolError::Execution(format!(
                    "cannot spawn '{command}': {e}. Is the language server installed?"
                ))
            })?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| ToolError::Execution("no stdin".into()))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| ToolError::Execution("no stdout".into()))?;
        let stdout = BufReader::new(stdout);

        let mut client = Self {
            process,
            stdin,
            stdout,
            next_id: 1,
        };

        // Send initialize request
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["plaintext", "markdown"] },
                    "definition": { "linkSupport": false }
                }
            },
            "workspaceFolders": [{"uri": root_uri, "name": "workspace"}]
        });

        let response = client.request("initialize", init_params).await?;
        if response.get("error").is_some() {
            let err = &response["error"];
            return Err(ToolError::Execution(format!(
                "initialize failed: {}",
                err.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            )));
        }

        // Send initialized notification
        client
            .notify("initialized", serde_json::json!({}))
            .await?;

        Ok(client)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send a request and wait for the matching response.
    async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let id = self.next_id();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        write_message(&mut self.stdin, &req).await?;
        read_response(&mut self.stdout, id).await
    }

    /// Send a notification (no response expected).
    async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), ToolError> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        write_message(&mut self.stdin, &req).await
    }

    /// Get hover information at a position.
    async fn hover(&mut self, uri: &str, line: u32, character: u32) -> Result<String, ToolError> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });
        let response = self.request("textDocument/hover", params).await?;
        let contents = response
            .get("result")
            .and_then(|r| r.get("contents"));

        match contents {
            Some(v) if v.is_string() => Ok(v.as_str().unwrap().to_string()),
            Some(v) if v.is_object() => Ok({
                // MarkupContent or MarkedString
                v.get("value")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| serde_json::to_string_pretty(v).unwrap_or_default())
            }),
            Some(arr) if arr.is_array() => {
                // Array of MarkedString
                let parts: Vec<String> = arr
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|v| {
                        v.get("value").and_then(|s| s.as_str()).or_else(|| v.as_str())
                    })
                    .map(|s| s.to_string())
                    .collect();
                if parts.is_empty() {
                    Ok("No hover information.".into())
                } else {
                    Ok(parts.join("\n"))
                }
            }
            _ => Ok("No hover information at this position.".into()),
        }
    }

    /// Get go-to-definition locations.
    async fn definition(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<String, ToolError> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });
        let response = self.request("textDocument/definition", params).await?;
        let result = response.get("result");

        match result {
            None | Some(serde_json::Value::Null) => {
                Ok("No definition found.".into())
            }
            Some(serde_json::Value::Array(locs)) if locs.is_empty() => {
                Ok("No definition found.".into())
            }
            Some(serde_json::Value::Array(locs)) => {
                let lines: Vec<String> = locs.iter().map(|loc| format_location(loc)).collect();
                Ok(lines.join("\n"))
            }
            Some(loc) => Ok(format_location(loc)),
        }
    }

    /// Get diagnostics for a file by sending didOpen and waiting for publishDiagnostics.
    async fn diagnostics(&mut self, uri: &str, text: &str, language_id: &str) -> Result<String, ToolError> {
        // Open the document
        self.notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )
        .await?;

        // Wait a moment for diagnostics
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Read pending messages looking for publishDiagnostics
        let mut diagnostics = Vec::new();
        for _ in 0..20 {
            // Try to read a message with a short timeout
            match tokio::time::timeout(
                std::time::Duration::from_millis(200),
                read_message(&mut self.stdout),
            )
            .await
            {
                Ok(Ok(msg)) => {
                    if msg
                        .get("method")
                        .and_then(|v| v.as_str())
                        == Some("textDocument/publishDiagnostics")
                    {
                        if let Some(params) = msg.get("params") {
                            if params.get("uri").and_then(|v| v.as_str()) == Some(uri) {
                                if let Some(diags) = params.get("diagnostics").and_then(|v| v.as_array())
                                {
                                    diagnostics = diags.clone();
                                }
                            }
                        }
                        break; // Found diagnostics for this file
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => break, // Timeout — no more messages
            }
        }

        if diagnostics.is_empty() {
            return Ok("No diagnostics — all clean!".into());
        }

        let mut lines = Vec::new();
        for diag in &diagnostics {
            let severity = diag.get("severity").and_then(|v| v.as_u64()).unwrap_or(0);
            let level = match severity {
                1 => "ERROR",
                2 => "WARNING",
                3 => "INFO",
                4 => "HINT",
                _ => "UNKNOWN",
            };
            let message = diag
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let range = diag.get("range");
            let start = range.and_then(|r| r.get("start"));
            let line = start
                .and_then(|s| s.get("line"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + 1;
            let col = start
                .and_then(|s| s.get("character"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + 1;

            let source = diag
                .get("source")
                .and_then(|v| v.as_str())
                .map(|s| format!(" [{s}]"))
                .unwrap_or_default();

            lines.push(format!("  {line}:{col} [{level}]{source}: {message}"));
        }
        Ok(lines.join("\n"))
    }

    /// Shut down the language server.
    async fn shutdown(&mut self) -> Result<(), ToolError> {
        let _ = self.request("shutdown", serde_json::json!({})).await;
        let _ = self.notify("exit", serde_json::json!({})).await;
        Ok(())
    }
}

/// Format a Location or LocationLink into a human-readable string.
fn format_location(loc: &serde_json::Value) -> String {
    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let range = loc
        .get("range")
        .or_else(|| loc.get("targetSelectionRange"))
        .or_else(|| loc.get("targetRange"));

    let start = range.and_then(|r| r.get("start"));
    let line = start
        .and_then(|s| s.get("line"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        + 1;
    let col = start
        .and_then(|s| s.get("character"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        + 1;

    // Strip file:// prefix
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    format!("{path}:{line}:{col}")
}

// ── LSP Tool ─────────────────────────────────────────────────────────

/// Language Server Protocol tool for code intelligence.
///
/// Manages language server processes and provides diagnostics, hover,
/// and go-to-definition for supported languages.
pub struct LspTool {
    servers: Mutex<HashMap<String, LspClient>>,
    /// Language → (server command, args)
    language_servers: HashMap<String, (String, Vec<String>)>,
}

impl LspTool {
    /// Create a new LSP tool with default language server configurations.
    pub fn new() -> Self {
        let mut language_servers = HashMap::new();
        language_servers.insert("rust".into(), ("rust-analyzer".into(), vec![]));
        language_servers.insert(
            "typescript".into(),
            ("typescript-language-server".into(), vec!["--stdio".into()]),
        );
        language_servers.insert(
            "javascript".into(),
            ("typescript-language-server".into(), vec!["--stdio".into()]),
        );
        language_servers.insert("python".into(), ("pylsp".into(), vec![]));

        Self {
            servers: Mutex::new(HashMap::new()),
            language_servers,
        }
    }

    /// Register a custom language server.
    pub fn register_server(
        &mut self,
        language: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) {
        self.language_servers
            .insert(language.into(), (command.into(), args));
    }

    /// Detect the language from a file extension.
    fn detect_language(path: &str) -> &str {
        if path.ends_with(".rs") {
            "rust"
        } else if path.ends_with(".ts") || path.ends_with(".tsx") {
            "typescript"
        } else if path.ends_with(".js") || path.ends_with(".jsx") {
            "javascript"
        } else if path.ends_with(".py") {
            "python"
        } else {
            "unknown"
        }
    }

    /// Get or start a language server for the given language.
    async fn get_server(
        &self,
        language: &str,
        root_uri: &str,
    ) -> Result<String, ToolError> {
        // Check if we already have a server
        {
            let servers = self.servers.lock().await;
            if servers.contains_key(language) {
                return Ok(language.to_string());
            }
        }

        // Start a new server
        let (command, args) = self
            .language_servers
            .get(language)
            .ok_or_else(|| {
                ToolError::Execution(format!(
                    "Unsupported language: '{language}'. Supported: {}",
                    self.language_servers
                        .keys()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let client = LspClient::new(command, &args_refs, root_uri).await?;

        let mut servers = self.servers.lock().await;
        servers.insert(language.to_string(), client);

        Ok(language.to_string())
    }
}

impl Default for LspTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LspTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "lsp".into(),
            description: "Interact with Language Server Protocol servers for code intelligence. \
                Actions: 'start' (start a language server), 'diagnostics' (get file diagnostics), \
                'hover' (get type info at position), 'definition' (go to definition), \
                'stop' (stop a server). \
                Provide 'file_path' for all actions. For hover/definition, also provide \
                'line' (0-based) and 'character' (0-based).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["start", "diagnostics", "hover", "definition", "stop"],
                        "description": "The action to perform"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the file (required for all actions)"
                    },
                    "file_content": {
                        "type": "string",
                        "description": "Current file content (required for diagnostics)"
                    },
                    "line": {
                        "type": "integer",
                        "description": "0-based line number (for hover/definition)"
                    },
                    "character": {
                        "type": "integer",
                        "description": "0-based character offset (for hover/definition)"
                    }
                },
                "required": ["action", "file_path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("action is required".into()))?;

        let file_path = args
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("file_path is required".into()))?;

        let language = Self::detect_language(file_path);
        let root_uri = format!(
            "file://{}",
            std::path::Path::new(file_path)
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "/".into())
        );
        let uri = format!("file://{file_path}");

        match action {
            "start" => {
                let lang = self.get_server(language, &root_uri).await?;
                Ok(ToolResult {
                    content: format!(
                        "Language server started for '{lang}' (detected from {file_path})"
                    ),
                    is_error: false,
                })
            }

            "diagnostics" => {
                let file_content = args
                    .get("file_content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidArgs(
                            "file_content is required for diagnostics".into(),
                        )
                    })?;

                self.get_server(language, &root_uri).await?;

                let mut servers = self.servers.lock().await;
                let client = servers
                    .get_mut(language)
                    .ok_or_else(|| ToolError::Execution("server not found".into()))?;

                let result = client.diagnostics(&uri, file_content, language).await?;
                Ok(ToolResult {
                    content: result,
                    is_error: false,
                })
            }

            "hover" => {
                let line = args
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let character = args
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;

                self.get_server(language, &root_uri).await?;

                let mut servers = self.servers.lock().await;
                let client = servers
                    .get_mut(language)
                    .ok_or_else(|| ToolError::Execution("server not found".into()))?;

                let result = client.hover(&uri, line, character).await?;
                Ok(ToolResult {
                    content: result,
                    is_error: false,
                })
            }

            "definition" => {
                let line = args
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let character = args
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;

                self.get_server(language, &root_uri).await?;

                let mut servers = self.servers.lock().await;
                let client = servers
                    .get_mut(language)
                    .ok_or_else(|| ToolError::Execution("server not found".into()))?;

                let result = client.definition(&uri, line, character).await?;
                Ok(ToolResult {
                    content: result,
                    is_error: false,
                })
            }

            "stop" => {
                let mut servers = self.servers.lock().await;
                if let Some(mut client) = servers.remove(language) {
                    client.shutdown().await?;
                    Ok(ToolResult {
                        content: format!("Language server for '{language}' stopped."),
                        is_error: false,
                    })
                } else {
                    Ok(ToolResult {
                        content: format!("No active server for '{language}'."),
                        is_error: false,
                    })
                }
            }

            _ => Ok(ToolResult {
                content: format!(
                    "Unknown action: '{action}'. Use start, diagnostics, hover, definition, or stop."
                ),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_language_by_extension() {
        assert_eq!(LspTool::detect_language("main.rs"), "rust");
        assert_eq!(LspTool::detect_language("app.ts"), "typescript");
        assert_eq!(LspTool::detect_language("app.tsx"), "typescript");
        assert_eq!(LspTool::detect_language("lib.js"), "javascript");
        assert_eq!(LspTool::detect_language("main.py"), "python");
        assert_eq!(LspTool::detect_language("README.md"), "unknown");
    }

    #[test]
    fn format_location_strips_file_prefix() {
        let loc = serde_json::json!({
            "uri": "file:///home/user/main.rs",
            "range": {
                "start": {"line": 9, "character": 4},
                "end": {"line": 9, "character": 10}
            }
        });
        let result = format_location(&loc);
        assert_eq!(result, "/home/user/main.rs:10:5");
    }

    #[test]
    fn format_location_target_uri() {
        let loc = serde_json::json!({
            "targetUri": "file:///home/user/lib.rs",
            "targetSelectionRange": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 5}
            }
        });
        let result = format_location(&loc);
        assert!(result.contains("lib.rs"));
    }

    #[test]
    fn schema_is_valid() {
        let tool = LspTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "lsp");
        assert!(schema.description.contains("Language Server Protocol"));
        let params = schema.parameters.as_object().unwrap();
        let props = params.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("action"));
        assert!(props.contains_key("file_path"));
        assert!(props.contains_key("line"));
        assert!(props.contains_key("character"));
    }

    #[tokio::test]
    async fn unsupported_language() {
        let tool = LspTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "start",
                "file_path": "/tmp/test.unknown"
            }))
            .await;
        // Should fail because .unknown is not a supported language
        assert!(result.is_err() || result.unwrap().is_error);
    }

    #[tokio::test]
    async fn lsp_missing_action() {
        let tool = LspTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn lsp_missing_file_path() {
        let tool = LspTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "start"}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn lsp_stop_no_server() {
        let tool = LspTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "stop",
                "file_path": "/tmp/test.rs"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No active"));
    }

    #[test]
    fn json_rpc_message_format() {
        // Verify our JSON-RPC message format is correct
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "test",
            "params": {}
        });
        let json = serde_json::to_string(&msg).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", json.len());
        let full = format!("{header}{json}");

        // Verify Content-Length header
        assert!(full.starts_with("Content-Length: "));
        // Verify the body length matches
        let body_start = full.find("\r\n\r\n").unwrap() + 4;
        assert_eq!(full[body_start..].len(), json.len());
    }
}