use std::any::{Any, TypeId};
use std::collections::HashMap;

use tokio::sync::watch;

/// Type-erased watch sender.
type BoxedSender = Box<dyn Any + Send + Sync>;

/// Type-erased watch receiver.
type BoxedReceiver = Box<dyn Any + Send + Sync>;

/// Provider hot-replace notification registry.
///
/// Each service type can have a `watch` channel. When the provider is replaced,
/// the sender is updated, and all subscribers receive the notification on their
/// next call. This implements "next-call生效" semantics: the replacement takes
/// effect on the next resolve, not on the currently-running call.
pub struct ProviderRegistry {
    channels: HashMap<TypeId, (BoxedSender, BoxedReceiver)>,
}

impl ProviderRegistry {
    /// Create a new empty provider registry.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Register a watch channel for a service type `T`.
    ///
    /// Returns a `watch::Receiver<T>` that will be notified when the provider changes.
    pub fn register<T: Send + Sync + Clone + 'static>(&mut self, initial: T) -> watch::Receiver<T> {
        let (tx, rx) = watch::channel(initial);
        let type_id = TypeId::of::<T>();
        self.channels
            .insert(type_id, (Box::new(tx), Box::new(rx.clone())));
        rx
    }

    /// Update the provider for type `T`. All subscribers are notified.
    pub fn update<T: Send + Sync + Clone + 'static>(&self, value: T) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some((sender, _)) = self.channels.get(&type_id)
            && let Some(tx) = sender.downcast_ref::<watch::Sender<T>>()
        {
            let _ = tx.send(value);
            return true;
        }
        false
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct MockProvider {
        name: String,
    }

    #[tokio::test]
    async fn register_and_update() {
        let mut registry = ProviderRegistry::new();
        let mut rx = registry.register(MockProvider { name: "v1".into() });

        assert_eq!(*rx.borrow_and_update(), MockProvider { name: "v1".into() });

        registry.update(MockProvider { name: "v2".into() });
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow_and_update(), MockProvider { name: "v2".into() });
    }

    #[tokio::test]
    async fn update_nonexistent_returns_false() {
        let registry = ProviderRegistry::new();
        assert!(!registry.update(42u32));
    }
}
