//! Command — write-side operations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Trait for commands.
pub trait Command: Send + Sync + 'static {
    /// Command type name.
    fn command_type(&self) -> &str;
}

/// Result of a command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Whether the command succeeded.
    pub success: bool,

    /// Result data (events produced, IDs created, etc.).
    pub data: serde_json::Value,

    /// Error message if failed.
    pub error: Option<String>,
}

/// Trait for command handlers.
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    /// Handle a command and return a result.
    async fn handle(&self, command: C) -> CommandResult;
}

/// Simple command handler wrapper.
pub struct SimpleCommandHandler<C: Command> {
    handler: Box<dyn Fn(C) -> CommandResult + Send + Sync>,
}

impl<C: Command> SimpleCommandHandler<C> {
    pub fn new(handler: impl Fn(C) -> CommandResult + Send + Sync + 'static) -> Self {
        Self {
            handler: Box::new(handler),
        }
    }
}

#[async_trait]
impl<C: Command> CommandHandler<C> for SimpleCommandHandler<C> {
    async fn handle(&self, command: C) -> CommandResult {
        (self.handler)(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestCommand {
        value: String,
    }

    impl Command for TestCommand {
        fn command_type(&self) -> &str {
            "test"
        }
    }

    #[tokio::test]
    async fn simple_handler() {
        let handler = SimpleCommandHandler::new(|cmd: TestCommand| CommandResult {
            success: true,
            data: serde_json::json!({"echo": cmd.value}),
            error: None,
        });

        let result = handler
            .handle(TestCommand {
                value: "hello".into(),
            })
            .await;
        assert!(result.success);
        assert_eq!(result.data["echo"], "hello");
    }
}
