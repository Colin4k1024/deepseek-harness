//! Compaction tool — estimates token usage and summarizes session logs.
//!
//! Helps manage long conversations by estimating token counts and
//! identifying when context compaction is needed.

use async_trait::async_trait;
use std::sync::Mutex;

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// Estimate tokens from text (4 chars ≈ 1 token).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Compaction tool that analyzes session logs and estimates token usage.
///
/// The tool can:
/// - Estimate token usage of a session log
/// - Summarize conversation turns
/// - Identify when context compaction is needed
pub struct CompactionTool {
    max_tokens: Mutex<usize>,
}

impl CompactionTool {
    /// Create a new compaction tool with a default limit of 128K tokens.
    pub fn new() -> Self {
        Self {
            max_tokens: Mutex::new(128_000),
        }
    }

    /// Set the maximum token limit.
    pub fn with_max_tokens(mut self, max: usize) -> Self {
        *self.max_tokens.lock().unwrap() = max;
        self
    }

    /// Get the current max token limit.
    pub fn max_tokens(&self) -> usize {
        *self.max_tokens.lock().unwrap()
    }

    /// Analyze a session log and return token usage statistics.
    fn analyze(&self, log: &[serde_json::Value]) -> TokenStats {
        let mut stats = TokenStats::default();
        let mut turn_count = 0u64;
        let mut current_turn = 0u64;

        for event in log {
            let event_type = event
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // Track turns
            if event_type == "turn/start" {
                if let Some(t) = event.get("turn").and_then(|v| v.as_u64()) {
                    if t != current_turn {
                        current_turn = t;
                        turn_count += 1;
                    }
                }
            }

            // Count tokens in message content
            let content = match event_type {
                "user/message" => event.get("content").and_then(|v| v.as_str()),
                "assistant/message" => event.get("message").and_then(|v| v.as_str()),
                "assistant/chunk" => event.get("content").and_then(|v| v.as_str()),
                "tool/result" => event.get("content").and_then(|v| v.as_str()),
                _ => None,
            };

            if let Some(text) = content {
                let tokens = estimate_tokens(text);
                stats.total_tokens += tokens as u64;
                if event_type == "user/message" {
                    stats.user_tokens += tokens as u64;
                } else if event_type == "assistant/message" {
                    stats.assistant_tokens += tokens as u64;
                } else if event_type == "tool/result" {
                    stats.tool_tokens += tokens as u64;
                }
            }
        }

        stats.turn_count = turn_count;
        stats.event_count = log.len() as u64;
        stats
    }
}

impl Default for CompactionTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Token usage statistics for a session log.
#[derive(Debug, Default)]
struct TokenStats {
    total_tokens: u64,
    user_tokens: u64,
    assistant_tokens: u64,
    tool_tokens: u64,
    turn_count: u64,
    event_count: u64,
}

#[async_trait]
impl Tool for CompactionTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "compact".into(),
            description: "Analyze the session log and estimate token usage. \
                Use this to check if context compaction is needed. \
                When token usage exceeds the limit, earlier turns should be summarized. \
                Provide the session log as a JSON array of events with the 'log' parameter. \
                Use 'action=analyze' to get token statistics, or 'action=estimate' to \
                estimate tokens for a single text.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["analyze", "estimate"],
                        "description": "analyze: analyze a session log; estimate: estimate tokens for text"
                    },
                    "log": {
                        "type": "array",
                        "description": "The session log as a JSON array of events (required for analyze)"
                    },
                    "text": {
                        "type": "string",
                        "description": "Text to estimate tokens for (required for estimate)"
                    }
                },
                "required": ["action"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("action is required".into()))?;

        match action {
            "analyze" => {
                let log = args
                    .get("log")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| ToolError::InvalidArgs("log array is required for analyze".into()))?;

                let stats = self.analyze(log);
                let max = self.max_tokens();
                let usage_pct = if max > 0 {
                    (stats.total_tokens as f64 / max as f64) * 100.0
                } else {
                    0.0
                };

                let mut result = format!(
                    "Token usage analysis:\n\
                     - Total tokens: {} / {} ({:.1}%)\n\
                     - User messages: {} tokens\n\
                     - Assistant messages: {} tokens\n\
                     - Tool results: {} tokens\n\
                     - Turns: {}\n\
                     - Events: {}",
                    stats.total_tokens,
                    max,
                    usage_pct,
                    stats.user_tokens,
                    stats.assistant_tokens,
                    stats.tool_tokens,
                    stats.turn_count,
                    stats.event_count,
                );

                if stats.total_tokens > max as u64 {
                    result.push_str(&format!(
                        "\n\n⚠️ Context exceeded! Exceeds limit by {} tokens. \
                         Consider summarizing earlier turns.",
                        stats.total_tokens - max as u64
                    ));
                } else if usage_pct > 80.0 {
                    result.push_str(&format!(
                        "\n\n⚠️ Approaching context limit ({:.0}% used). \
                         Consider compaction soon.",
                        usage_pct
                    ));
                } else {
                    result.push_str("\n\n✅ Context usage is within limits.");
                }

                Ok(ToolResult {
                    content: result,
                    is_error: false,
                })
            }

            "estimate" => {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("text is required for estimate".into()))?;

                let tokens = estimate_tokens(text);
                Ok(ToolResult {
                    content: format!(
                        "Estimated tokens: {} ({} characters, ~4 chars/token)",
                        tokens,
                        text.len()
                    ),
                    is_error: false,
                })
            }

            _ => Ok(ToolResult {
                content: format!("Unknown action: '{action}'. Use analyze or estimate."),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_estimation() {
        assert_eq!(estimate_tokens("hello"), 2); // 5 chars / 4 = 2
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("1234"), 1); // exactly 4 chars
        assert_eq!(estimate_tokens("12345"), 2); // 5 chars, ceil
    }

    #[tokio::test]
    async fn compact_analyze_empty_log() {
        let tool = CompactionTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "analyze",
                "log": []
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("0 /"));
    }

    #[tokio::test]
    async fn compact_analyze_with_events() {
        let tool = CompactionTool::new();
        let log = serde_json::json!([
            {"type": "turn/start", "turn": 1, "seq": 1, "time": 1000},
            {"type": "user/message", "content": "Hello, how are you?", "seq": 2, "time": 1001, "turn": 1},
            {"type": "assistant/message", "message": "I am doing well, thank you!", "seq": 3, "time": 1002, "turn": 1, "step": 1},
            {"type": "turn/end", "turn": 1, "seq": 4, "time": 1003, "reason": {"kind": "completed"}}
        ]);

        let result = tool
            .execute(serde_json::json!({
                "action": "analyze",
                "log": log
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("User messages:"));
        assert!(result.content.contains("Assistant messages:"));
        assert!(result.content.contains("Turns: 1"));
    }

    #[tokio::test]
    async fn compact_estimate() {
        let tool = CompactionTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "estimate",
                "text": "Hello world"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Estimated tokens:"));
    }

    #[tokio::test]
    async fn compact_exceeded_limit() {
        let tool = CompactionTool::new().with_max_tokens(10);
        // Create a log that exceeds 10 tokens
        let log = serde_json::json!([
            {"type": "user/message", "content": "This is a very long message that should exceed the token limit easily", "seq": 1, "time": 1000, "turn": 1},
        ]);

        let result = tool
            .execute(serde_json::json!({
                "action": "analyze",
                "log": log
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Exceeds"));
    }

    #[tokio::test]
    async fn compact_missing_action() {
        let tool = CompactionTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }
}