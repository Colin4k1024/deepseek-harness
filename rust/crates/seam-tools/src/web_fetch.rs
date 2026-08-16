//! Web fetch tool — fetches and extracts text from a URL.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// Web fetch tool that retrieves page content from a URL.
pub struct WebFetchTool {
    client: reqwest::Client,
    max_content_length: usize,
}

impl WebFetchTool {
    /// Create a new web fetch tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            max_content_length: 50_000,
        }
    }

    /// Set the maximum content length to return.
    pub fn with_max_content_length(mut self, max: usize) -> Self {
        self.max_content_length = max;
        self
    }

    /// Strip HTML tags and extract readable text.
    fn strip_html(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;

        let lower = html.to_lowercase();
        let chars: Vec<char> = html.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            if ch == '<' {
                in_tag = true;
                // Check for script/style tags
                let rest = &lower[i..];
                if rest.starts_with("<script") {
                    in_script = true;
                } else if rest.starts_with("<style") {
                    in_style = true;
                } else if rest.starts_with("</script") {
                    in_script = false;
                } else if rest.starts_with("</style") {
                    in_style = false;
                }
                continue;
            }
            if ch == '>' {
                in_tag = false;
                if !in_script && !in_style {
                    result.push(' ');
                }
                continue;
            }
            if !in_tag && !in_script && !in_style {
                result.push(ch);
            }
        }

        // Collapse whitespace
        let mut cleaned = String::new();
        let mut prev_space = true;
        for ch in result.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    cleaned.push(' ');
                    prev_space = true;
                }
            } else {
                cleaned.push(ch);
                prev_space = false;
            }
        }
        cleaned.trim().to_string()
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "web_fetch".into(),
            description: "Fetch the content of a web page by URL. Returns the extracted text content (HTML tags stripped).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("url is required".into()))?;

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(ToolResult {
                content: format!("Invalid URL: {url}. Must start with http:// or https://"),
                is_error: true,
            });
        }

        let resp = self
            .client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; dsh-web-fetch/0.1)",
            )
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("fetch failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Ok(ToolResult {
                content: format!("HTTP {status}: failed to fetch {url}"),
                is_error: true,
            });
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        // Only process text content
        if !content_type.contains("text/") && !content_type.contains("application/json") {
            return Ok(ToolResult {
                content: format!("Cannot process content type: {content_type}. Only text/ and application/json are supported."),
                is_error: true,
            });
        }

        let body = resp
            .text()
            .await
            .map_err(|e| ToolError::Execution(format!("read body failed: {e}")))?;

        let text = if content_type.contains("text/html") {
            Self::strip_html(&body)
        } else {
            body
        };

        let truncated = if text.len() > self.max_content_length {
            format!(
                "{}...\n\n[Content truncated at {} characters — original: {} characters]",
                &text[..self.max_content_length],
                self.max_content_length,
                text.len()
            )
        } else {
            text
        };

        Ok(ToolResult {
            content: truncated,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_basic() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = WebFetchTool::strip_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn strip_html_removes_script() {
        let html = "<html><script>alert('xss')</script><p>Safe</p></html>";
        let text = WebFetchTool::strip_html(html);
        assert!(text.contains("Safe"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn strip_html_removes_style() {
        let html = "<html><style>body { color: red; }</style><p>Styled</p></html>";
        let text = WebFetchTool::strip_html(html);
        assert!(text.contains("Styled"));
        assert!(!text.contains("color"));
    }

    #[tokio::test]
    async fn web_fetch_invalid_url() {
        let tool = WebFetchTool::new();
        let result = tool
            .execute(serde_json::json!({"url": "not-a-url"}))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Invalid URL"));
    }

    #[tokio::test]
    async fn web_fetch_missing_url() {
        let tool = WebFetchTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }
}