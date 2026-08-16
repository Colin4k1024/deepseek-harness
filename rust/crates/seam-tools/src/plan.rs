//! Plan tool — create and manage execution plans.
//!
//! Plans are markdown documents with a title. The plan state machine is:
//! `draft → finalized → approved → executing → completed`.
//! The agent creates a plan, presents it for approval, and executes after approval.

use async_trait::async_trait;
use std::sync::Mutex;

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// Plan status.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    Draft,
    Finalized,
    Approved,
    Executing,
    Completed,
}

/// A structured execution plan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub title: String,
    pub content: String,
    pub status: PlanStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Tool that manages execution plans.
///
/// The agent uses this tool to create a plan before executing complex tasks.
/// The plan is presented to the user for approval before execution begins.
pub struct PlanTool {
    plan: Mutex<Option<Plan>>,
}

impl PlanTool {
    /// Create a new plan tool with no active plan.
    pub fn new() -> Self {
        Self {
            plan: Mutex::new(None),
        }
    }

    /// Return the current plan, if any.
    pub fn current(&self) -> Option<Plan> {
        self.plan.lock().unwrap().clone()
    }

    /// Whether a plan is currently active.
    pub fn has_plan(&self) -> bool {
        self.plan.lock().unwrap().is_some()
    }

    /// Get the current plan status.
    pub fn status(&self) -> Option<PlanStatus> {
        self.plan.lock().unwrap().as_ref().map(|p| p.status.clone())
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for PlanTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PlanTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "plan".into(),
            description: "Create and manage an execution plan. \
                Use this before complex multi-step tasks to organize your approach. \
                Actions: 'create' (start a new plan), 'update' (modify the plan), \
                'finalize' (mark as ready for review), 'approve' (mark as approved), \
                'execute' (begin execution), 'complete' (mark as done), \
                'status' (show current plan). \
                Plans are markdown documents with a title.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "update", "finalize", "approve", "execute", "complete", "status"],
                        "description": "The action to perform on the plan"
                    },
                    "title": {
                        "type": "string",
                        "description": "Plan title (required for create)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Plan content as markdown (required for create and update)"
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
            "create" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("title is required for create".into()))?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("content is required for create".into()))?;

                let now = Self::now_ms();
                let plan = Plan {
                    title: title.to_string(),
                    content: content.to_string(),
                    status: PlanStatus::Draft,
                    created_at: now,
                    updated_at: now,
                };

                let mut guard = self.plan.lock().unwrap();
                *guard = Some(plan.clone());

                Ok(ToolResult {
                    content: format!(
                        "Plan created: **{}**\n\n{}\n\nStatus: draft — use 'finalize' when ready, then 'approve' to begin.",
                        plan.title, plan.content
                    ),
                    is_error: false,
                })
            }

            "update" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("content is required for update".into()))?;

                let mut guard = self.plan.lock().unwrap();
                match guard.as_mut() {
                    Some(plan) => {
                        plan.content = content.to_string();
                        plan.updated_at = Self::now_ms();
                        Ok(ToolResult {
                            content: format!("Plan updated: **{}**\n\n{}", plan.title, plan.content),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: true,
                    }),
                }
            }

            "finalize" => {
                let mut guard = self.plan.lock().unwrap();
                match guard.as_mut() {
                    Some(plan) => {
                        if plan.status != PlanStatus::Draft {
                            return Ok(ToolResult {
                                content: format!(
                                    "Cannot finalize plan in status '{}'. Only draft plans can be finalized.",
                                    serde_json::to_string(&plan.status).unwrap_or_default()
                                ),
                                is_error: true,
                            });
                        }
                        plan.status = PlanStatus::Finalized;
                        plan.updated_at = Self::now_ms();
                        Ok(ToolResult {
                            content: format!(
                                "Plan finalized: **{}**\n\n{}\n\nReady for approval. Use action='approve' to begin execution.",
                                plan.title, plan.content
                            ),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: true,
                    }),
                }
            }

            "approve" => {
                let mut guard = self.plan.lock().unwrap();
                match guard.as_mut() {
                    Some(plan) => {
                        if plan.status != PlanStatus::Finalized {
                            return Ok(ToolResult {
                                content: format!(
                                    "Cannot approve plan in status '{}'. Finalize the plan first.",
                                    serde_json::to_string(&plan.status).unwrap_or_default()
                                ),
                                is_error: true,
                            });
                        }
                        plan.status = PlanStatus::Approved;
                        plan.updated_at = Self::now_ms();
                        Ok(ToolResult {
                            content: format!(
                                "Plan approved: **{}**\n\n{}\n\nUse action='execute' to begin.",
                                plan.title, plan.content
                            ),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: true,
                    }),
                }
            }

            "execute" => {
                let mut guard = self.plan.lock().unwrap();
                match guard.as_mut() {
                    Some(plan) => {
                        if plan.status != PlanStatus::Approved {
                            return Ok(ToolResult {
                                content: format!(
                                    "Cannot execute plan in status '{}'. Approve the plan first.",
                                    serde_json::to_string(&plan.status).unwrap_or_default()
                                ),
                                is_error: true,
                            });
                        }
                        plan.status = PlanStatus::Executing;
                        plan.updated_at = Self::now_ms();
                        Ok(ToolResult {
                            content: format!(
                                "Executing plan: **{}**\n\n{}",
                                plan.title, plan.content
                            ),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: true,
                    }),
                }
            }

            "complete" => {
                let mut guard = self.plan.lock().unwrap();
                match guard.as_mut() {
                    Some(plan) => {
                        plan.status = PlanStatus::Completed;
                        plan.updated_at = Self::now_ms();
                        let result = format!(
                            "Plan completed: **{}**\n\n{}\n\n✅ All tasks done.",
                            plan.title, plan.content
                        );
                        *guard = None; // Clear the plan
                        Ok(ToolResult {
                            content: result,
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: true,
                    }),
                }
            }

            "status" => {
                let guard = self.plan.lock().unwrap();
                match guard.as_ref() {
                    Some(plan) => {
                        let status_str = serde_json::to_string(&plan.status).unwrap_or_default();
                        Ok(ToolResult {
                            content: format!(
                                "Current plan: **{}**\nStatus: {}\n\n{}",
                                plan.title, status_str, plan.content
                            ),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        content: "No active plan. Use action='create' to start one.".into(),
                        is_error: false,
                    }),
                }
            }

            _ => Ok(ToolResult {
                content: format!(
                    "Unknown action: '{action}'. Use create, update, finalize, approve, execute, complete, or status."
                ),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plan_create_and_status() {
        let tool = PlanTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "create",
                "title": "My Plan",
                "content": "# Plan\n1. Step one\n2. Step two"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("My Plan"));
        assert!(result.content.contains("Step one"));

        let result = tool
            .execute(serde_json::json!({"action": "status"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("My Plan"));
    }

    #[tokio::test]
    async fn plan_lifecycle() {
        let tool = PlanTool::new();

        // Create
        tool.execute(serde_json::json!({
            "action": "create",
            "title": "Test",
            "content": "Test content"
        }))
        .await
        .unwrap();

        // Finalize
        let result = tool
            .execute(serde_json::json!({"action": "finalize"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("finalized"));

        // Approve
        let result = tool
            .execute(serde_json::json!({"action": "approve"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("approved"));

        // Execute
        let result = tool
            .execute(serde_json::json!({"action": "execute"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Executing"));

        // Complete
        let result = tool
            .execute(serde_json::json!({"action": "complete"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("completed"));

        // After complete, plan should be cleared
        let result = tool
            .execute(serde_json::json!({"action": "status"}))
            .await
            .unwrap();
        assert!(result.content.contains("No active plan"));
    }

    #[tokio::test]
    async fn plan_invalid_transitions() {
        let tool = PlanTool::new();

        // Create and try to approve directly (should fail)
        tool.execute(serde_json::json!({
            "action": "create",
            "title": "Test",
            "content": "Test"
        }))
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({"action": "approve"}))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Finalize"));
    }

    #[tokio::test]
    async fn plan_update() {
        let tool = PlanTool::new();
        tool.execute(serde_json::json!({
            "action": "create",
            "title": "Original",
            "content": "Original content"
        }))
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({
                "action": "update",
                "content": "Updated content"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Updated content"));
    }

    #[tokio::test]
    async fn plan_no_active_plan() {
        let tool = PlanTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "status"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No active plan"));
    }

    #[tokio::test]
    async fn plan_missing_fields() {
        let tool = PlanTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "create"}))
            .await;
        assert!(result.is_err());
    }
}