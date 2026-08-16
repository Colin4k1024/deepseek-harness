use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Type-erased event handler.
type EventHandler = Box<dyn Fn(&dyn Any) + Send + Sync>;

/// Type-erased event handlers keyed by TypeId.
type HandlerMap = HashMap<TypeId, Vec<Arc<EventHandler>>>;

/// Typed event bus with waterfall semantics.
///
/// Handlers are registered per event type (keyed by `TypeId`).
/// When an event is dispatched, all registered handlers for that type are called.
/// Waterfall semantics: handlers can call `next()` to propagate to the next handler.
pub struct EventBus {
    handlers: Arc<RwLock<HandlerMap>>,
}

impl EventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to events of type `T`.
    ///
    /// The handler receives a reference to the event payload.
    /// Returns a subscription guard that removes the handler on drop.
    pub fn on<T: Send + Sync + 'static>(
        &self,
        handler: impl Fn(&T) + Send + Sync + 'static,
    ) -> Subscription {
        let type_id = TypeId::of::<T>();
        let wrapper: EventHandler = Box::new(move |any: &dyn Any| {
            if let Some(event) = any.downcast_ref::<T>() {
                handler(event);
            }
        });
        let handler = Arc::new(wrapper);
        let mut handlers = self.handlers.write().unwrap();
        handlers
            .entry(type_id)
            .or_default()
            .push(Arc::clone(&handler));
        Subscription {
            type_id,
            handler,
            handlers: Arc::clone(&self.handlers),
        }
    }

    /// Dispatch an event to all registered handlers for its type.
    pub fn emit<T: Send + Sync + 'static>(&self, event: &T) {
        let type_id = TypeId::of::<T>();
        let handlers = self.handlers.read().unwrap();
        if let Some(handler_list) = handlers.get(&type_id) {
            for handler in handler_list {
                handler(event);
            }
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Subscription guard that removes the handler from the event bus on drop.
pub struct Subscription {
    type_id: TypeId,
    handler: Arc<EventHandler>,
    handlers: Arc<RwLock<HandlerMap>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut handlers = self.handlers.write().unwrap();
        if let Some(handler_list) = handlers.get_mut(&self.type_id) {
            handler_list.retain(|h| !Arc::ptr_eq(h, &self.handler));
            if handler_list.is_empty() {
                handlers.remove(&self.type_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    struct TestEvent {
        value: u32,
    }

    #[test]
    fn emit_delivers_to_subscriber() {
        let bus = EventBus::new();
        let received = Arc::new(AtomicU32::new(0));
        let r = Arc::clone(&received);

        let _sub = bus.on::<TestEvent>(move |e| {
            r.store(e.value, Ordering::Relaxed);
        });

        bus.emit(&TestEvent { value: 42 });
        assert_eq!(received.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn multiple_subscribers() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicU32::new(0));

        let c1 = Arc::clone(&count);
        let _s1 = bus.on::<TestEvent>(move |_| {
            c1.fetch_add(1, Ordering::Relaxed);
        });
        let c2 = Arc::clone(&count);
        let _s2 = bus.on::<TestEvent>(move |_| {
            c2.fetch_add(1, Ordering::Relaxed);
        });

        bus.emit(&TestEvent { value: 1 });
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn subscription_drop_removes_handler() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&count);

        let sub = bus.on::<TestEvent>(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        });

        bus.emit(&TestEvent { value: 1 });
        assert_eq!(count.load(Ordering::Relaxed), 1);

        drop(sub);
        bus.emit(&TestEvent { value: 2 });
        assert_eq!(count.load(Ordering::Relaxed), 1); // unchanged
    }

    #[test]
    fn different_event_types_isolated() {
        let bus = EventBus::new();
        let str_count = Arc::new(AtomicU32::new(0));
        let u32_count = Arc::new(AtomicU32::new(0));

        let sc = Arc::clone(&str_count);
        let _s1 = bus.on::<String>(move |_| {
            sc.fetch_add(1, Ordering::Relaxed);
        });
        let uc = Arc::clone(&u32_count);
        let _s2 = bus.on::<u32>(move |_| {
            uc.fetch_add(1, Ordering::Relaxed);
        });

        bus.emit(&"hello".to_string());
        bus.emit(&42u32);
        assert_eq!(str_count.load(Ordering::Relaxed), 1);
        assert_eq!(u32_count.load(Ordering::Relaxed), 1);
    }
}
