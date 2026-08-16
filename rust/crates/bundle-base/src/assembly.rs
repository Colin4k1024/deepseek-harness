//! L3: Base bundle — assembles L0-L2 into a usable agent runtime.
//!
//! Uses Cargo feature flags for compile-time selection of which providers
//! and capabilities are included. The `Profile` struct provides runtime
//! configuration of which components are active.

use std::sync::Arc;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_context::Context;
use dsh_llm_adapter::LlmAdapter;

use crate::profile::{LlmProvider, Profile};

/// Errors from bundle assembly.
#[derive(Debug, thiserror::Error)]
pub enum AssemblyError {
    #[error("LLM provider not available: {0}")]
    LlmNotAvailable(String),
    #[error("configuration error: {0}")]
    Config(String),
}

/// Base bundle — assembles Context + LLM + Tools into an AgentRunner.
pub struct BaseBundle {
    profile: Profile,
    ctx: Context,
}

impl BaseBundle {
    /// Create a new base bundle with the given profile.
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            ctx: Context::new(),
        }
    }

    /// Get a reference to the profile.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Get a reference to the context.
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// Create an LLM adapter based on the profile configuration.
    ///
    /// Returns `None` if the requested provider is not available at compile time.
    pub fn create_llm(&self, api_key: &str) -> Result<Arc<dyn LlmAdapter>, AssemblyError> {
        match &self.profile.llm {
            #[cfg(feature = "llm-deepseek")]
            LlmProvider::DeepSeek => Ok(Arc::new(dsh_llm_deepseek::DeepSeekAdapter::new(
                api_key.to_string(),
            ))),

            #[cfg(feature = "llm-openai")]
            LlmProvider::OpenAi => Ok(Arc::new(dsh_llm_openai::OpenAiAdapter::new(
                api_key.to_string(),
            ))),

            #[cfg(feature = "llm-anthropic")]
            LlmProvider::Anthropic => Ok(Arc::new(dsh_llm_anthropic::AnthropicAdapter::new(
                api_key.to_string(),
            ))),

            #[cfg(not(feature = "llm-deepseek"))]
            LlmProvider::DeepSeek => Err(AssemblyError::LlmNotAvailable(
                "DeepSeek provider not compiled (enable 'llm-deepseek' feature)".into(),
            )),

            #[cfg(not(feature = "llm-openai"))]
            LlmProvider::OpenAi => Err(AssemblyError::LlmNotAvailable(
                "OpenAI provider not compiled (enable 'llm-openai' feature)".into(),
            )),

            #[cfg(not(feature = "llm-anthropic"))]
            LlmProvider::Anthropic => Err(AssemblyError::LlmNotAvailable(
                "Anthropic provider not compiled (enable 'llm-anthropic' feature)".into(),
            )),

            LlmProvider::Custom(name) => Err(AssemblyError::LlmNotAvailable(format!(
                "Custom provider '{name}' not supported"
            ))),
        }
    }

    /// Create an AgentRunner with the configured LLM provider.
    pub fn create_runner(&self, api_key: &str) -> Result<AgentRunner, AssemblyError> {
        let llm = self.create_llm(api_key)?;
        Ok(AgentRunner::new(llm, self.profile.model.clone()))
    }

    /// Resolve the sandbox backend based on profile and compile-time features.
    #[cfg(feature = "sandbox")]
    pub fn resolve_sandbox(&self) -> dsh_seam_sandbox::SandboxResolution {
        let policy = dsh_seam_sandbox::SandboxPolicy::Confined;
        dsh_seam_sandbox::resolve(policy)
    }
}

impl Default for BaseBundle {
    fn default() -> Self {
        Self::new(Profile::default())
    }
}
