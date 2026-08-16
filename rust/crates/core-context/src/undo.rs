use std::any::{Any, TypeId};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::mpsc;

/// Undo queue: manages the background consumer task that processes scope guard drops.
///
/// When a `ScopeGuard` is dropped, it sends its `TypeId` through the channel.
/// The consumer task receives these and removes the corresponding service entry
/// from the shared services map.
pub(crate) struct UndoQueue {
    tx: Arc<mpsc::UnboundedSender<TypeId>>,
    // Dropping this handle aborts the consumer task.
    _handle: tokio::task::JoinHandle<()>,
}

impl UndoQueue {
    /// Create a new undo queue with a background consumer task.
    pub(crate) fn new(services: Arc<DashMap<TypeId, Box<dyn Any + Send + Sync>>>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx = Arc::new(tx);
        let handle = tokio::spawn(undo_consumer(rx, services));
        Self {
            tx,
            _handle: handle,
        }
    }

    /// Get a clone of the sender for creating `ScopeGuard`s.
    pub(crate) fn sender(&self) -> Arc<mpsc::UnboundedSender<TypeId>> {
        self.tx.clone()
    }
}

/// Background task that processes undo actions.
async fn undo_consumer(
    mut rx: mpsc::UnboundedReceiver<TypeId>,
    services: Arc<DashMap<TypeId, Box<dyn Any + Send + Sync>>>,
) {
    while let Some(type_id) = rx.recv().await {
        services.remove(&type_id);
        tracing::trace!(?type_id, "undo: removed service");
    }
    tracing::debug!("undo consumer shut down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    #[tokio::test]
    async fn undo_queue_removes_service() {
        let services: Arc<DashMap<TypeId, Box<dyn Any + Send + Sync>>> = Arc::new(DashMap::new());
        let type_id = TypeId::of::<String>();
        services.insert(type_id, Box::new("hello".to_string()));
        assert!(services.contains_key(&type_id));

        let queue = UndoQueue::new(services.clone());
        let tx = queue.sender();
        tx.send(type_id).unwrap();

        // Give the consumer time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!services.contains_key(&type_id));
    }

    #[tokio::test]
    async fn undo_queue_multiple_actions() {
        let services: Arc<DashMap<TypeId, Box<dyn Any + Send + Sync>>> = Arc::new(DashMap::new());
        let queue = UndoQueue::new(services.clone());
        let tx = queue.sender();

        let id1 = TypeId::of::<u32>();
        let id2 = TypeId::of::<u64>();
        services.insert(id1, Box::new(42u32));
        services.insert(id2, Box::new(99u64));

        tx.send(id1).unwrap();
        tx.send(id2).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!services.contains_key(&id1));
        assert!(!services.contains_key(&id2));
    }
}
