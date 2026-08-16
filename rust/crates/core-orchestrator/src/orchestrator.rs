//! Orchestrator — coordinates multi-agent workflows.
//!
//! Supports three orchestration patterns:
//! - **Sequential**: agents run one after another, each getting the previous result
//! - **Fan-out**: dispatch a task to multiple agents in parallel
//! - **Fan-in**: collect results from multiple agents

use std::sync::Arc;

use crate::agent::AgentHandle;
use crate::message::AgentMessage;
use crate::registry::AgentRegistry;

/// Errors from orchestration.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("no agents with capability: {0}")]
    NoAgentsForCapability(String),
    #[error("send error: {0}")]
    SendError(String),
    #[error("timeout waiting for response")]
    Timeout,
}

/// Result from an orchestration operation.
#[derive(Debug, Clone)]
pub struct OrchestrationResult {
    pub agent_id: String,
    pub result: String,
}

/// Orchestrator coordinates multi-agent workflows.
pub struct Orchestrator {
    registry: Arc<AgentRegistry>,
}

impl Orchestrator {
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self { registry }
    }

    /// Sequential: run tasks one after another, passing results forward.
    ///
    /// Agent 1 processes initial input → Agent 2 processes Agent 1's output → ...
    pub async fn sequential(
        &self,
        agent_ids: &[&str],
        initial_input: &str,
        timeout: std::time::Duration,
    ) -> Result<Vec<OrchestrationResult>, OrchestratorError> {
        let mut results = Vec::new();
        let mut current_input = initial_input.to_string();

        for &agent_id in agent_ids {
            let agent = self
                .registry
                .get(agent_id)
                .await
                .ok_or_else(|| OrchestratorError::AgentNotFound(agent_id.into()))?;

            let response = self
                .send_and_wait(&agent, "orchestrator", &current_input, timeout)
                .await?;

            current_input = response.clone();
            results.push(OrchestrationResult {
                agent_id: agent_id.into(),
                result: response,
            });
        }

        Ok(results)
    }

    /// Fan-out: dispatch a task to multiple agents in parallel.
    ///
    /// Returns results from all agents (order not guaranteed).
    pub async fn fan_out(
        &self,
        agent_ids: &[&str],
        task: &str,
        timeout: std::time::Duration,
    ) -> Result<Vec<OrchestrationResult>, OrchestratorError> {
        let mut handles = Vec::new();

        for &agent_id in agent_ids {
            let agent = self
                .registry
                .get(agent_id)
                .await
                .ok_or_else(|| OrchestratorError::AgentNotFound(agent_id.into()))?;

            let task = task.to_string();
            let orchestrator_id = "orchestrator".to_string();

            handles.push(tokio::spawn(async move {
                // In a real implementation, this would use the agent's actual
                // processing capability. For now, we send the message.
                let msg = AgentMessage::task_request(&orchestrator_id, &agent.id, &task);
                agent
                    .send(msg)
                    .map_err(|e| OrchestratorError::SendError(e.to_string()))?;

                // Wait for response (simplified — real impl would use response channel)
                tokio::time::sleep(timeout / 10).await;

                Ok(OrchestrationResult {
                    agent_id: agent.id.clone(),
                    result: format!("processed by {}", agent.name),
                })
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(OrchestratorError::SendError(e.to_string())),
            }
        }

        Ok(results)
    }

    /// Fan-in: collect results from multiple agents and merge.
    ///
    /// Dispatches to all agents, collects results, then passes them
    /// to a collector agent for synthesis.
    pub async fn fan_in(
        &self,
        worker_ids: &[&str],
        collector_id: &str,
        task: &str,
        timeout: std::time::Duration,
    ) -> Result<OrchestrationResult, OrchestratorError> {
        // Fan-out to workers
        let worker_results = self.fan_out(worker_ids, task, timeout).await?;

        // Collect and format results
        let collected = worker_results
            .iter()
            .map(|r| format!("[{}]: {}", r.agent_id, r.result))
            .collect::<Vec<_>>()
            .join("\n");

        // Send to collector
        let collector = self
            .registry
            .get(collector_id)
            .await
            .ok_or_else(|| OrchestratorError::AgentNotFound(collector_id.into()))?;

        let final_result = self
            .send_and_wait(&collector, "orchestrator", &collected, timeout)
            .await?;

        Ok(OrchestrationResult {
            agent_id: collector_id.into(),
            result: final_result,
        })
    }

    /// Send a message to an agent and wait for a response.
    async fn send_and_wait(
        &self,
        agent: &AgentHandle,
        from: &str,
        task: &str,
        _timeout: std::time::Duration,
    ) -> Result<String, OrchestratorError> {
        let msg = AgentMessage::task_request(from, &agent.id, task);
        agent
            .send(msg)
            .map_err(|e| OrchestratorError::SendError(e.to_string()))?;

        // Simplified: return a placeholder. Real implementation would wait
        // on a response channel with timeout.
        Ok(format!("result from {}: {}", agent.name, task))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn sequential_orchestration() {
        let registry = Arc::new(AgentRegistry::new());

        let (a1, _rx1) = AgentHandle::new("a1", "step1", vec!["process".into()]);
        let (a2, _rx2) = AgentHandle::new("a2", "step2", vec!["refine".into()]);

        registry.register(a1).await;
        registry.register(a2).await;

        let orch = Orchestrator::new(registry);
        let results = orch
            .sequential(
                &["a1", "a2"],
                "initial input",
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].agent_id, "a1");
        assert_eq!(results[1].agent_id, "a2");
    }

    #[tokio::test]
    async fn agent_not_found() {
        let registry = Arc::new(AgentRegistry::new());
        let orch = Orchestrator::new(registry);

        let result = orch
            .sequential(&["nonexistent"], "input", std::time::Duration::from_secs(1))
            .await;

        assert!(matches!(result, Err(OrchestratorError::AgentNotFound(_))));
    }
}
