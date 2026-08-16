use std::any::TypeId;
use std::sync::Arc;

use tokio::sync::mpsc;

/// RAII guard that enqueues a service deregistration on drop.
///
/// When dropped, sends the `TypeId` through the undo channel rather than
/// executing inline. This avoids async-in-Drop issues and keeps the hot path fast.
pub struct ScopeGuard {
    type_id: TypeId,
    undo_tx: Arc<mpsc::UnboundedSender<TypeId>>,
}

impl ScopeGuard {
    pub(crate) fn new(type_id: TypeId, undo_tx: Arc<mpsc::UnboundedSender<TypeId>>) -> Self {
        Self { type_id, undo_tx }
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        // Send is non-blocking and infallible for unbounded channels.
        // If the receiver is closed (Context dropped), the undo is a no-op.
        let _ = self.undo_tx.send(self.type_id);
    }
}

/// A scoped view of a `Context`, holding guards for this scope's registrations.
///
/// When the `ScopedContext` is dropped, all its guards are dropped in reverse
/// registration order, triggering undo for each service registered in this scope.
pub struct ScopedContext {
    guards: Vec<ScopeGuard>,
}

impl ScopedContext {
    /// Create a new empty scoped context.
    pub fn new() -> Self {
        Self { guards: Vec::new() }
    }

    /// Add a guard to this scope. The guard will be dropped when the scope is dropped.
    pub fn add_guard(&mut self, guard: ScopeGuard) {
        self.guards.push(guard);
    }

    /// Number of guards held by this scope.
    pub fn len(&self) -> usize {
        self.guards.len()
    }

    /// Whether this scope has no guards.
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

impl Default for ScopedContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_guard_holds_type_id() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let tx = Arc::new(tx);
        let guard = ScopeGuard::new(TypeId::of::<String>(), tx);
        assert_eq!(guard.type_id, TypeId::of::<String>());
    }

    #[test]
    fn scope_guard_drop_sends_type_id() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Arc::new(tx);
        {
            let _guard = ScopeGuard::new(TypeId::of::<u32>(), tx);
        }
        let received = rx.try_recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap(), TypeId::of::<u32>());
    }

    #[test]
    fn scoped_context_holds_guards() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let tx = Arc::new(tx);
        let mut scope = ScopedContext::new();
        scope.add_guard(ScopeGuard::new(TypeId::of::<u32>(), tx.clone()));
        scope.add_guard(ScopeGuard::new(TypeId::of::<u64>(), tx));
        assert_eq!(scope.len(), 2);
    }

    #[test]
    fn scoped_context_drop_sends_all_guard_ids() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Arc::new(tx);
        {
            let mut scope = ScopedContext::new();
            scope.add_guard(ScopeGuard::new(TypeId::of::<u32>(), tx.clone()));
            scope.add_guard(ScopeGuard::new(TypeId::of::<u64>(), tx));
        }
        let mut received = Vec::new();
        while let Ok(id) = rx.try_recv() {
            received.push(id);
        }
        assert_eq!(received.len(), 2);
        // Vec drops in order, so u32 is sent first, u64 second.
        assert_eq!(received[0], TypeId::of::<u32>());
        assert_eq!(received[1], TypeId::of::<u64>());
    }
}
