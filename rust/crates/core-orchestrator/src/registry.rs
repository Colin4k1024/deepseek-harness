//! AgentRegistry — manages available agents and their capabilities.

use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::agent::{AgentHandle, AgentStatus};

/// Registry of available agents.
///
/// Agents register themselves with their capabilities. Other agents
/// can discover agents by capability.
pub struct AgentRegistry {
    agents: RwLock<HashMap<String, AgentEntry>>,
}

struct AgentEntry {
    handle: AgentHandle,
    status: AgentStatus,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Register an agent.
    pub async fn register(&self, handle: AgentHandle) {
        let id = handle.id.clone();
        let mut agents = self.agents.write().await;
        agents.insert(
            id,
            AgentEntry {
                handle,
                status: AgentStatus::Idle,
            },
        );
    }

    /// Unregister an agent.
    pub async fn unregister(&self, agent_id: &str) {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id);
    }

    /// Get an agent handle by ID.
    pub async fn get(&self, agent_id: &str) -> Option<AgentHandle> {
        let agents = self.agents.read().await;
        agents.get(agent_id).map(|e| e.handle.clone())
    }

    /// Find agents by capability.
    pub async fn find_by_capability(&self, capability: &str) -> Vec<AgentHandle> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|e| e.handle.has_capability(capability) && e.status == AgentStatus::Idle)
            .map(|e| e.handle.clone())
            .collect()
    }

    /// Update agent status.
    pub async fn set_status(&self, agent_id: &str, status: AgentStatus) {
        let mut agents = self.agents.write().await;
        if let Some(entry) = agents.get_mut(agent_id) {
            entry.status = status;
        }
    }

    /// Get all registered agent IDs.
    pub async fn list_ids(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Number of registered agents.
    pub async fn len(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Whether the registry is empty.
    pub async fn is_empty(&self) -> bool {
        let agents = self.agents.read().await;
        agents.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_find() {
        let registry = AgentRegistry::new();
        let (handle, _rx) = AgentHandle::new("a1", "researcher", vec!["research".into()]);

        registry.register(handle).await;
        assert_eq!(registry.len().await, 1);

        let found = registry.find_by_capability("research").await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "a1");
    }

    #[tokio::test]
    async fn find_by_capability_filters_busy() {
        let registry = AgentRegistry::new();
        let (handle, _rx) = AgentHandle::new("a1", "worker", vec!["code".into()]);

        registry.register(handle).await;
        registry
            .set_status(
                "a1",
                AgentStatus::Busy {
                    task_id: "t1".into(),
                },
            )
            .await;

        let found = registry.find_by_capability("code").await;
        assert!(found.is_empty()); // busy agents are filtered out
    }

    #[tokio::test]
    async fn unregister() {
        let registry = AgentRegistry::new();
        let (handle, _rx) = AgentHandle::new("a1", "test", vec![]);

        registry.register(handle).await;
        assert_eq!(registry.len().await, 1);

        registry.unregister("a1").await;
        assert!(registry.is_empty().await);
    }
}
