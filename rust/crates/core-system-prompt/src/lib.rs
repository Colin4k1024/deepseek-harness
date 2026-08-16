//! L1: System prompt assembly — section registry, ordering, and variable interpolation.
//!
//! `SystemPrompt` collects ordered sections and assembles them into a final
//! prompt string with `{{variable}}` interpolation.

use std::collections::HashMap;

/// A section of the system prompt.
#[derive(Debug, Clone)]
pub struct PromptSection {
    /// Unique identifier for this section.
    pub id: String,
    /// Ordering priority (lower = earlier in the assembled prompt).
    pub order: i32,
    /// The text content of this section (may contain `{{variable}}` placeholders).
    pub text: String,
}

/// System prompt assembler.
///
/// Registers ordered sections and assembles them into a final prompt string
/// with variable interpolation.
pub struct SystemPrompt {
    sections: Vec<PromptSection>,
    variables: HashMap<String, String>,
}

impl SystemPrompt {
    /// Create a new empty system prompt assembler.
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            variables: HashMap::new(),
        }
    }

    /// Register a section. Sections are sorted by `order` during assembly.
    pub fn register(&mut self, id: impl Into<String>, order: i32, text: impl Into<String>) {
        self.sections.push(PromptSection {
            id: id.into(),
            order,
            text: text.into(),
        });
    }

    /// Set a variable for interpolation. Variables are replaced as `{{key}}` in section text.
    pub fn set_variable(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    /// Set multiple variables at once.
    pub fn set_variables(&mut self, vars: HashMap<String, String>) {
        self.variables.extend(vars);
    }

    /// Remove a section by id.
    pub fn remove(&mut self, id: &str) {
        self.sections.retain(|s| s.id != id);
    }

    /// Clear all sections.
    pub fn clear(&mut self) {
        self.sections.clear();
    }

    /// Return the number of registered sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Assemble the system prompt by sorting sections and interpolating variables.
    ///
    /// Sections are sorted by `order` (ascending), then concatenated with
    /// newlines between sections. `{{variable}}` placeholders are replaced
    /// with their registered values; missing variables are left as-is.
    pub fn assemble(&self) -> String {
        let mut sorted: Vec<&PromptSection> = self.sections.iter().collect();
        sorted.sort_by_key(|s| s.order);

        let mut result = String::new();
        for section in &sorted {
            let text = self.interpolate(&section.text);
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&text);
        }
        result
    }

    /// Replace `{{variable}}` placeholders with their values.
    fn interpolate(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (key, value) in &self.variables {
            let placeholder = format!("{{{{{key}}}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }
}

impl Default for SystemPrompt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_system_prompt() {
        let sp = SystemPrompt::new();
        assert_eq!(sp.assemble(), "");
    }

    #[test]
    fn single_section() {
        let mut sp = SystemPrompt::new();
        sp.register("intro", 0, "You are a helpful assistant.");
        assert_eq!(sp.assemble(), "You are a helpful assistant.");
    }

    #[test]
    fn ordering() {
        let mut sp = SystemPrompt::new();
        sp.register("middle", 50, "MIDDLE");
        sp.register("first", 10, "FIRST");
        sp.register("last", 100, "LAST");
        let result = sp.assemble();
        assert_eq!(result, "FIRST\nMIDDLE\nLAST");
    }

    #[test]
    fn variable_interpolation() {
        let mut sp = SystemPrompt::new();
        sp.register("intro", 0, "You are {{role}}.");
        sp.set_variable("role", "a helpful assistant");
        assert_eq!(sp.assemble(), "You are a helpful assistant.");
    }

    #[test]
    fn multiple_variables() {
        let mut sp = SystemPrompt::new();
        sp.register("info", 0, "Model: {{model}}, Version: {{version}}");
        sp.set_variable("model", "deepseek-v4");
        sp.set_variable("version", "1.0");
        assert_eq!(sp.assemble(), "Model: deepseek-v4, Version: 1.0");
    }

    #[test]
    fn remove_section() {
        let mut sp = SystemPrompt::new();
        sp.register("a", 0, "A");
        sp.register("b", 1, "B");
        sp.remove("a");
        assert_eq!(sp.assemble(), "B");
    }

    #[test]
    fn set_variables_bulk() {
        let mut sp = SystemPrompt::new();
        sp.register("t", 0, "{{a}} {{b}}");
        let mut vars = HashMap::new();
        vars.insert("a".into(), "hello".into());
        vars.insert("b".into(), "world".into());
        sp.set_variables(vars);
        assert_eq!(sp.assemble(), "hello world");
    }

    #[test]
    fn no_interpolation_for_missing_variables() {
        let mut sp = SystemPrompt::new();
        sp.register("t", 0, "Hello {{name}}!");
        let result = sp.assemble();
        assert_eq!(result, "Hello {{name}}!");
    }
}