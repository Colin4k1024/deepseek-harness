//! Waterfall extension points for the agent loop.
//!
//! Each hook runs in sequence; returning `Err` short-circuits the chain.
//! `pre_execute` hooks can set `approved = false` to block tool execution.

use crate::state::LoopError;
use dsh_llm_adapter::{LlmRequest, Message, ToolDefinition};

/// Context passed to pre-step hooks (before prompt assembly).
pub struct PreStepCtx<'a> {
    pub turn: u64,
    pub step: u64,
    pub messages: &'a mut Vec<Message>,
    pub tools: &'a mut Option<Vec<ToolDefinition>>,
    pub iteration: usize,
}

/// Context passed to pre-request hooks (before LLM dispatch).
pub struct PreRequestCtx<'a> {
    pub request: &'a mut LlmRequest,
}

/// Context passed to pre-execute hooks (before tool execution).
pub struct PreExecuteCtx<'a> {
    pub tool_name: &'a str,
    pub arguments: &'a serde_json::Value,
    /// Set to `false` to deny execution.
    pub approved: &'a mut bool,
}

/// Context passed to post-execute hooks (after tool execution).
pub struct PostExecuteCtx<'a> {
    pub tool_name: &'a str,
    pub arguments: &'a serde_json::Value,
    pub result: &'a dsh_core_tools::ToolResult,
}

/// Registry of waterfall extension hooks for the agent loop.
///
/// Each hook type runs in registration order. A hook that returns `Err`
/// stops the chain immediately.
pub struct AgentHooks {
    pre_step: Vec<Box<dyn Fn(&mut PreStepCtx<'_>) -> Result<(), LoopError> + Send + Sync>>,
    pre_request: Vec<Box<dyn Fn(&mut PreRequestCtx<'_>) -> Result<(), LoopError> + Send + Sync>>,
    pre_execute: Vec<Box<dyn Fn(&mut PreExecuteCtx<'_>) -> Result<(), LoopError> + Send + Sync>>,
    post_execute: Vec<Box<dyn Fn(&PostExecuteCtx<'_>) + Send + Sync>>,
}

impl AgentHooks {
    /// Create an empty hook registry.
    pub fn new() -> Self {
        Self {
            pre_step: Vec::new(),
            pre_request: Vec::new(),
            pre_execute: Vec::new(),
            post_execute: Vec::new(),
        }
    }

    /// Register a pre-step hook. Called before prompt assembly for each step.
    pub fn on_pre_step(
        &mut self,
        hook: impl Fn(&mut PreStepCtx<'_>) -> Result<(), LoopError> + Send + Sync + 'static,
    ) {
        self.pre_step.push(Box::new(hook));
    }

    /// Register a pre-request hook. Called before each LLM request is dispatched.
    pub fn on_pre_request(
        &mut self,
        hook: impl Fn(&mut PreRequestCtx<'_>) -> Result<(), LoopError> + Send + Sync + 'static,
    ) {
        self.pre_request.push(Box::new(hook));
    }

    /// Register a pre-execute hook. Called before each tool is executed.
    /// Hooks can set `ctx.approved = false` to block execution.
    pub fn on_pre_execute(
        &mut self,
        hook: impl Fn(&mut PreExecuteCtx<'_>) -> Result<(), LoopError> + Send + Sync + 'static,
    ) {
        self.pre_execute.push(Box::new(hook));
    }

    /// Register a post-execute hook. Called after each tool completes.
    pub fn on_post_execute(
        &mut self,
        hook: impl Fn(&PostExecuteCtx<'_>) + Send + Sync + 'static,
    ) {
        self.post_execute.push(Box::new(hook));
    }

    /// Run all pre-step hooks in sequence.
    pub fn run_pre_step(&self, ctx: &mut PreStepCtx<'_>) -> Result<(), LoopError> {
        for hook in &self.pre_step {
            hook(ctx)?;
        }
        Ok(())
    }

    /// Run all pre-request hooks in sequence.
    pub fn run_pre_request(&self, ctx: &mut PreRequestCtx<'_>) -> Result<(), LoopError> {
        for hook in &self.pre_request {
            hook(ctx)?;
        }
        Ok(())
    }

    /// Run all pre-execute hooks in sequence. If any hook sets `approved = false`,
    /// execution is blocked (but the chain continues for other hooks to observe).
    pub fn run_pre_execute(&self, ctx: &mut PreExecuteCtx<'_>) -> Result<(), LoopError> {
        for hook in &self.pre_execute {
            hook(ctx)?;
        }
        Ok(())
    }

    /// Run all post-execute hooks in sequence. These are fire-and-forget observers.
    pub fn run_post_execute(&self, ctx: &PostExecuteCtx<'_>) {
        for hook in &self.post_execute {
            hook(ctx);
        }
    }
}

impl Default for AgentHooks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hooks_do_nothing() {
        let hooks = AgentHooks::new();
        let mut messages = Vec::new();
        let mut tools = None;
        let mut ctx = PreStepCtx {
            turn: 1,
            step: 1,
            messages: &mut messages,
            tools: &mut tools,
            iteration: 0,
        };
        assert!(hooks.run_pre_step(&mut ctx).is_ok());
    }

    #[test]
    fn pre_step_hook_modifies_context() {
        let mut hooks = AgentHooks::new();
        hooks.on_pre_step(|ctx| {
            ctx.messages.push(Message {
                role: dsh_llm_adapter::Role::System,
                content: "intercepted".into(),
                tool_calls: None,
                tool_call_id: None,
            });
            Ok(())
        });

        let mut messages = Vec::new();
        let mut tools = None;
        let mut ctx = PreStepCtx {
            turn: 1,
            step: 1,
            messages: &mut messages,
            tools: &mut tools,
            iteration: 0,
        };
        hooks.run_pre_step(&mut ctx).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "intercepted");
    }

    #[test]
    fn pre_step_hook_short_circuits() {
        let mut hooks = AgentHooks::new();
        hooks.on_pre_step(|_ctx| Err(LoopError::Llm(dsh_llm_adapter::LlmError::Api("blocked".into()))));
        hooks.on_pre_step(|_ctx| {
            // This should never run
            unreachable!();
        });

        let mut messages = Vec::new();
        let mut tools = None;
        let mut ctx = PreStepCtx {
            turn: 1,
            step: 1,
            messages: &mut messages,
            tools: &mut tools,
            iteration: 0,
        };
        let result = hooks.run_pre_step(&mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn pre_execute_can_deny() {
        let mut hooks = AgentHooks::new();
        hooks.on_pre_execute(|ctx| {
            if ctx.tool_name == "dangerous" {
                *ctx.approved = false;
            }
            Ok(())
        });

        let mut approved = true;
        let args = serde_json::json!({});
        let mut ctx = PreExecuteCtx {
            tool_name: "dangerous",
            arguments: &args,
            approved: &mut approved,
        };
        hooks.run_pre_execute(&mut ctx).unwrap();
        assert!(!approved);
    }

    #[test]
    fn post_execute_observes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let observed = Arc::new(AtomicBool::new(false));
        let obs = Arc::clone(&observed);

        let mut hooks = AgentHooks::new();
        hooks.on_post_execute(move |_ctx| {
            obs.store(true, Ordering::SeqCst);
        });

        let result = dsh_core_tools::ToolResult {
            content: "ok".into(),
            is_error: false,
        };
        let args = serde_json::json!({});
        let ctx = PostExecuteCtx {
            tool_name: "test",
            arguments: &args,
            result: &result,
        };
        hooks.run_post_execute(&ctx);
        assert!(observed.load(Ordering::SeqCst));
    }
}