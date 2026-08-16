//! Skill tool — loads skill instructions from a directory.
//!
//! Skills are stored as markdown files in a directory. Each file name
//! (without the `.md` extension) is the skill name. The file content
//! is the skill's instructions returned to the agent.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// A registry of skills loaded from a directory.
pub struct SkillRegistry {
    skills: HashMap<String, String>,
}

impl SkillRegistry {
    /// Create an empty skill registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load skills from a directory of `.md` files.
    ///
    /// Each file `<name>.md` becomes a skill named `<name>`.
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<(), ToolError> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            ToolError::Execution(format!("cannot read skill dir {}: {e}", dir.display()))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ToolError::Execution(format!("read dir entry: {e}"))
            })?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    let content = std::fs::read_to_string(&path).map_err(|e| {
                        ToolError::Execution(format!(
                            "cannot read skill file {}: {e}",
                            path.display()
                        ))
                    })?;
                    self.skills.insert(name.to_string(), content);
                }
            }
        }
        Ok(())
    }

    /// Register a skill programmatically.
    pub fn register(&mut self, name: impl Into<String>, instructions: impl Into<String>) {
        self.skills.insert(name.into(), instructions.into());
    }

    /// Get a skill's instructions by name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.skills.get(name).map(|s| s.as_str())
    }

    /// List all available skill names.
    pub fn list(&self) -> Vec<&str> {
        self.skills.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool that loads skill instructions by name.
///
/// When the agent needs to perform a task that matches a skill, it can
/// load the skill's instructions to get detailed guidance.
pub struct SkillTool {
    registry: SkillRegistry,
    skill_dir: Option<PathBuf>,
}

impl SkillTool {
    /// Create a new skill tool with an empty registry.
    pub fn new() -> Self {
        Self {
            registry: SkillRegistry::new(),
            skill_dir: None,
        }
    }

    /// Create a skill tool that loads skills from a directory.
    pub fn from_dir(dir: impl Into<PathBuf>) -> Result<Self, ToolError> {
        let dir = dir.into();
        let mut registry = SkillRegistry::new();
        registry.load_from_dir(&dir)?;
        Ok(Self {
            registry,
            skill_dir: Some(dir),
        })
    }

    /// Register a skill programmatically.
    pub fn register(&mut self, name: impl Into<String>, instructions: impl Into<String>) {
        self.registry.register(name, instructions);
    }

    /// Access the underlying skill registry.
    pub fn registry(&self) -> &SkillRegistry {
        &self.registry
    }

    /// Reload skills from the configured directory.
    pub fn reload(&mut self) -> Result<(), ToolError> {
        if let Some(ref dir) = self.skill_dir {
            self.registry = SkillRegistry::new();
            self.registry.load_from_dir(dir)?;
        }
        Ok(())
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "skill".into(),
            description: "Load detailed instructions for a named skill. \
                Skills provide step-by-step guidance for specific tasks. \
                Use this when you need to follow a documented procedure. \
                Available skills: use the 'list' argument to see them.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The skill name to load, or 'list' to see all available skills"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("name is required".into()))?;

        if name == "list" || name == "list_skills" {
            let skills = self.registry.list();
            if skills.is_empty() {
                return Ok(ToolResult {
                    content: "No skills are currently available. Skills can be loaded from a directory of .md files.".into(),
                    is_error: false,
                });
            }
            let mut content = String::from("Available skills:\n");
            for skill in skills {
                content.push_str(&format!("- {skill}\n"));
            }
            return Ok(ToolResult {
                content,
                is_error: false,
            });
        }

        match self.registry.get(name) {
            Some(instructions) => Ok(ToolResult {
                content: instructions.to_string(),
                is_error: false,
            }),
            None => {
                let available = self.registry.list();
                let hint = if available.is_empty() {
                    "No skills are registered. Use 'skill' with name='list' to see available skills."
                        .to_string()
                } else {
                    format!(
                        "Available skills: {}. Use 'skill' with name='list' to see all.",
                        available.join(", ")
                    )
                };
                Ok(ToolResult {
                    content: format!("Skill '{name}' not found. {hint}"),
                    is_error: true,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_registry_register_and_get() {
        let mut registry = SkillRegistry::new();
        registry.register("test-skill", "These are instructions for testing.");
        assert_eq!(registry.get("test-skill"), Some("These are instructions for testing."));
        assert_eq!(registry.get("nonexistent"), None);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn skill_registry_list() {
        let mut registry = SkillRegistry::new();
        registry.register("skill-a", "A");
        registry.register("skill-b", "B");
        let mut list = registry.list();
        list.sort();
        assert_eq!(list, vec!["skill-a", "skill-b"]);
    }

    #[test]
    fn skill_registry_empty() {
        let registry = SkillRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn skill_tool_loads_skill() {
        let mut tool = SkillTool::new();
        tool.register("test-skill", "Step 1: Do this.\nStep 2: Do that.");

        let result = tool
            .execute(serde_json::json!({"name": "test-skill"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Step 1"));
        assert!(result.content.contains("Step 2"));
    }

    #[tokio::test]
    async fn skill_tool_not_found() {
        let tool = SkillTool::new();
        let result = tool
            .execute(serde_json::json!({"name": "nonexistent"}))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn skill_tool_list() {
        let mut tool = SkillTool::new();
        tool.register("skill-a", "A");
        tool.register("skill-b", "B");

        let result = tool
            .execute(serde_json::json!({"name": "list"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("skill-a"));
        assert!(result.content.contains("skill-b"));
    }

    #[tokio::test]
    async fn skill_tool_missing_name() {
        let tool = SkillTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn skill_registry_load_from_dir() {
        // Create a temp directory with skill files
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("skill-a.md"), "Instructions for A").unwrap();
        std::fs::write(dir.path().join("skill-b.md"), "Instructions for B").unwrap();
        // Non-md file should be ignored
        std::fs::write(dir.path().join("notes.txt"), "Not a skill").unwrap();

        let mut registry = SkillRegistry::new();
        registry.load_from_dir(dir.path()).unwrap();
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.get("skill-a"), Some("Instructions for A"));
        assert_eq!(registry.get("skill-b"), Some("Instructions for B"));
        assert_eq!(registry.get("notes"), None);
    }
}