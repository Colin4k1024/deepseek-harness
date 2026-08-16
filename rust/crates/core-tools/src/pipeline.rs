use crate::trait_def::ToolResult;
use thiserror::Error;

/// Rejection from a pre-execution middleware.
#[derive(Debug, Error)]
#[error("tool call rejected: {reason}")]
pub struct Rejection {
    pub reason: String,
}

/// Control flow for middleware chain.
pub enum ControlFlow {
    /// Continue to the next middleware / tool execution.
    Continue,
    /// Reject the tool call.
    Reject(Rejection),
}

/// Pre-execution hook: can inspect and modify a tool call, or reject it.
pub type PreHook = Box<dyn Fn(&mut ToolCallContext) -> ControlFlow + Send + Sync>;

/// Post-execution hook: can inspect and modify a tool result.
pub type PostHook = Box<dyn Fn(&mut ToolResult) + Send + Sync>;

/// Context passed to pre-hooks.
pub struct ToolCallContext {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

/// Tool pipeline with pre/post middleware chain.
pub struct ToolPipeline {
    pre_hooks: Vec<PreHook>,
    post_hooks: Vec<PostHook>,
}

impl ToolPipeline {
    pub fn new() -> Self {
        Self {
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
        }
    }

    pub fn add_pre_hook(&mut self, hook: PreHook) {
        self.pre_hooks.push(hook);
    }

    pub fn add_post_hook(&mut self, hook: PostHook) {
        self.post_hooks.push(hook);
    }

    /// Run pre-hooks. Returns `Err` if any hook rejects.
    pub fn run_pre(&self, ctx: &mut ToolCallContext) -> Result<(), Rejection> {
        for hook in &self.pre_hooks {
            match hook(ctx) {
                ControlFlow::Continue => {}
                ControlFlow::Reject(r) => return Err(r),
            }
        }
        Ok(())
    }

    /// Run post-hooks on a tool result.
    pub fn run_post(&self, result: &mut ToolResult) {
        for hook in &self.post_hooks {
            hook(result);
        }
    }
}

impl Default for ToolPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pipeline_passes() {
        let pipeline = ToolPipeline::new();
        let mut ctx = ToolCallContext {
            tool_name: "test".into(),
            arguments: serde_json::json!({}),
        };
        assert!(pipeline.run_pre(&mut ctx).is_ok());
    }

    #[test]
    fn pre_hook_can_reject() {
        let mut pipeline = ToolPipeline::new();
        pipeline.add_pre_hook(Box::new(|_| {
            ControlFlow::Reject(Rejection {
                reason: "blocked".into(),
            })
        }));
        let mut ctx = ToolCallContext {
            tool_name: "test".into(),
            arguments: serde_json::json!({}),
        };
        assert!(pipeline.run_pre(&mut ctx).is_err());
    }
}
