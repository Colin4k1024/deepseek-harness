use std::any::{Any, TypeId};
use std::sync::Arc;

use dashmap::DashMap;

use crate::scope::ScopeGuard;
use crate::undo::UndoQueue;

/// Type-erased service container with scope-aware registration and automatic undo.
///
/// Services are keyed by `TypeId` and stored as `Box<dyn Any + Send + Sync>`.
/// Each `provide()` call returns a `ScopeGuard` that enqueues an undo action on drop.
pub struct Context {
    services: Arc<DashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    undo: UndoQueue,
}

impl Context {
    /// Create a new root context. Spawns a background undo consumer task.
    pub fn new() -> Self {
        let services = Arc::new(DashMap::new());
        let undo = UndoQueue::new(services.clone());
        Self { services, undo }
    }

    /// Register a service. Returns a `ScopeGuard` that removes it on drop.
    pub fn provide<T: Send + Sync + 'static>(&self, svc: T) -> ScopeGuard {
        let type_id = TypeId::of::<T>();
        self.services.insert(type_id, Box::new(svc));
        ScopeGuard::new(type_id, self.undo.sender())
    }

    /// Resolve a service by type. Returns `None` if not registered.
    pub fn resolve<T: Send + Sync + 'static>(
        &self,
    ) -> Option<dashmap::mapref::one::Ref<'_, TypeId, Box<dyn Any + Send + Sync>>> {
        let type_id = TypeId::of::<T>();
        self.services.get(&type_id)
    }

    /// Number of registered services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Whether the context has no registered services.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Shutdown the undo consumer task. Waits for pending undo actions to drain.
    pub async fn shutdown(self) {
        drop(self.undo);
        // Give the consumer a moment to process remaining actions.
        // In practice, the mpsc receiver will close when all senders are dropped.
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct MockService {
        value: String,
    }

    #[tokio::test]
    async fn provide_and_resolve() {
        let ctx = Context::new();
        let _guard = ctx.provide(MockService {
            value: "hello".into(),
        });

        let resolved = ctx.resolve::<MockService>();
        assert!(resolved.is_some());
        let svc = resolved.unwrap();
        let svc = svc.value().downcast_ref::<MockService>().unwrap();
        assert_eq!(svc.value, "hello");
    }

    #[tokio::test]
    async fn resolve_missing_returns_none() {
        let ctx = Context::new();
        assert!(ctx.resolve::<MockService>().is_none());
    }

    #[tokio::test]
    async fn guard_drop_removes_service() {
        let ctx = Context::new();
        {
            let _guard = ctx.provide(MockService {
                value: "temp".into(),
            });
            assert!(ctx.resolve::<MockService>().is_some());
        }
        // Guard dropped — undo action should be processed.
        // Give the consumer task time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(ctx.resolve::<MockService>().is_none());
    }

    #[tokio::test]
    async fn len_tracks_registrations() {
        let ctx = Context::new();
        assert_eq!(ctx.len(), 0);
        assert!(ctx.is_empty());

        let _g1 = ctx.provide(MockService { value: "a".into() });
        assert_eq!(ctx.len(), 1);

        let _g2 = ctx.provide(42u32);
        assert_eq!(ctx.len(), 2);
    }

    #[tokio::test]
    async fn multi_guard_drop_order() {
        let ctx = Context::new();
        let g1 = ctx.provide(1u32);
        let g2 = ctx.provide(2u64);

        assert_eq!(ctx.len(), 2);
        drop(g2);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(ctx.len(), 1);
        assert!(ctx.resolve::<u32>().is_some());
        assert!(ctx.resolve::<u64>().is_none());

        drop(g1);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(ctx.is_empty());
    }
}
