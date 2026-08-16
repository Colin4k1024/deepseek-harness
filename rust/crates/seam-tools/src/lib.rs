//! Built-in tools for the agent: bash, file read/write/edit, search, list files.

pub mod bash;
pub mod compact;
pub mod edit_file;
pub mod file_read;
pub mod file_write;
pub mod list_files;
pub mod lsp;
pub mod plan;
pub mod search;
pub mod skill;
pub mod subagent;
pub mod terminal;
pub mod todo;
pub mod web_fetch;
pub mod web_search;

pub use bash::BashTool;
pub use compact::CompactionTool;
pub use edit_file::EditFileTool;
pub use file_read::FileReadTool;
pub use file_write::FileWriteTool;
pub use list_files::ListFilesTool;
pub use lsp::LspTool;
pub use plan::PlanTool;
pub use search::SearchTool;
pub use skill::SkillTool;
pub use subagent::SubagentTool;
pub use terminal::TerminalTool;
pub use todo::TodoTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
