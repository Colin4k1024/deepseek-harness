//! L3: Base bundle — assembles L0-L2 into a usable agent runtime.
//!
//! # Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `llm-deepseek` | ✅ | Include DeepSeek LLM provider |
//! | `llm-openai` | ❌ | Include OpenAI LLM provider |
//! | `llm-anthropic` | ❌ | Include Anthropic LLM provider |
//! | `sandbox` | ✅ | Include sandbox capability seam |
//! | `persistence-jsonl` | ✅ | Include JSONL persistence |
//! | `persistence-sqlite` | ❌ | Include SQLite persistence |
//! | `mcp` | ❌ | Include MCP client |

pub mod assembly;
pub mod profile;

pub use assembly::{AssemblyError, BaseBundle};
pub use profile::{LlmProvider, PersistenceBackend, Profile, SandboxBackend};
