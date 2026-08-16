//! Web search tool — queries a search engine and returns results.

use async_trait::async_trait;
use serde::Deserialize;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// A single search result.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Web search tool that queries a search API.
///
/// When no API key is configured, returns simulated results for testing.
pub struct WebSearchTool {
    client: reqwest::Client,
    api_key: Option<String>,
    max_results: usize,
}

impl WebSearchTool {
    /// Create a new web search tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: None,
            max_results: 5,
        }
    }

    /// Configure an API key for real searches.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the maximum number of results to return.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Perform a real search using the configured API.
    async fn search_real(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        // Use a simple search API (DuckDuckGo HTML, SerpAPI, etc.)
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
            urlencoding(query)
        );
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "dsh-web-search/0.1")
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("search request failed: {e}")))?;

        #[derive(Deserialize)]
        struct DuckResult {
            #[serde(rename = "FirstURL")]
            first_url: String,
            #[serde(rename = "Text")]
            text: String,
        }
        #[derive(Deserialize)]
        struct DuckResponse {
            #[serde(rename = "Heading")]
            heading: String,
            #[serde(rename = "AbstractText")]
            abstract_text: String,
            #[serde(rename = "AbstractURL")]
            abstract_url: String,
            #[serde(rename = "RelatedTopics")]
            related_topics: Vec<serde_json::Value>,
        }

        let body: DuckResponse = resp
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("parse error: {e}")))?;

        let mut results = Vec::new();

        if !body.heading.is_empty() {
            results.push(SearchResult {
                title: body.heading,
                url: body.abstract_url,
                snippet: body.abstract_text,
            });
        }

        for topic in body.related_topics.iter().take(self.max_results.saturating_sub(1)) {
            if let Some(text) = topic.get("Text").and_then(|v| v.as_str()) {
                if let Some(url) = topic.get("FirstURL").and_then(|v| v.as_str()) {
                    // Extract title from text (first sentence)
                    let title = text.split(". ").next().unwrap_or(text);
                    results.push(SearchResult {
                        title: title.to_string(),
                        url: url.to_string(),
                        snippet: text.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Simulate search results for testing.
    fn search_simulated(&self, query: &str) -> Vec<SearchResult> {
        vec![
            SearchResult {
                title: format!("Result for '{query}' — Example.com"),
                url: format!("https://example.com/search?q={}", urlencoding(query)),
                snippet: format!("This is a simulated search result for '{query}'. In production, configure an API key for real results."),
            },
            SearchResult {
                title: format!("{query} — Wikipedia"),
                url: format!("https://en.wikipedia.org/wiki/{}", urlencoding(query)),
                snippet: format!("Encyclopedia article about {query}."),
            },
        ]
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "web_search".into(),
            description: "Search the web for information. Returns a list of search results with titles, URLs, and snippets.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("query is required".into()))?;

        let results = if self.api_key.is_some() {
            self.search_real(query).await?
        } else {
            self.search_simulated(query)
        };

        let content = serde_json::to_string_pretty(&results)
            .map_err(|e| ToolError::Execution(format!("serialize error: {e}")))?;

        Ok(ToolResult {
            content,
            is_error: false,
        })
    }
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
        .replace('?', "%3F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn web_search_simulated() {
        let tool = WebSearchTool::new();
        let result = tool
            .execute(serde_json::json!({"query": "rust programming"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("rust programming"));
        assert!(result.content.contains("Example.com"));
    }

    #[tokio::test]
    async fn web_search_missing_query() {
        let tool = WebSearchTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }
}