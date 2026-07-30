//! Minimal type-erased event bus.
//!
//! Each event type is keyed by its `TypeId`. Subscribers receive an `Arc<Event>`
//! they can downcast from.

use parking_lot::Mutex;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

pub type SubscriberId = u64;

/// Trait-object-friendly event wrapper.
pub trait Event: Any + Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync + std::fmt::Debug> Event for T {
    fn as_any(&self) -> &dyn Any { self }
}

type Handler = Box<dyn Fn(&dyn Event) + Send + Sync>;

/// A simple publish/subscribe bus with type-erased events.
pub struct EventBus {
    inner: Mutex<BusState>,
}

struct BusState {
    next_id: SubscriberId,
    handlers: HashMap<TypeId, Vec<(SubscriberId, Handler)>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            inner: Mutex::new(BusState {
                next_id: 1,
                handlers: HashMap::new(),
            }),
        }
    }
}

impl EventBus {
    pub fn new() -> Self { Self::default() }

    /// Subscribe to events of type `T`. Returns an id you can pass to `unsubscribe`.
    pub fn subscribe<T, F>(&self, f: F) -> SubscriberId
    where
        T: Event,
        F: Fn(&T) + Send + Sync + 'static,
    {
        let mut state = self.inner.lock();
        let id = state.next_id;
        state.next_id += 1;
        let handler: Handler = Box::new(move |evt: &dyn Event| {
            if let Some(typed) = evt.as_any().downcast_ref::<T>() {
                f(typed);
            }
        });
        state.handlers.entry(TypeId::of::<T>()).or_default().push((id, handler));
        id
    }

    /// Remove a subscriber by id.
    pub fn unsubscribe(&self, id: SubscriberId) {
        let mut state = self.inner.lock();
        for handlers in state.handlers.values_mut() {
            handlers.retain(|(sid, _)| *sid != id);
        }
    }

    /// Publish an event. All matching handlers are called synchronously.
    pub fn publish<T: Event>(&self, event: T) {
        let arc: Arc<dyn Event> = Arc::new(event);
        let tid = TypeId::of::<T>();
        let state = self.inner.lock();
        if let Some(handlers) = state.handlers.get(&tid) {
            for (_, handler) in handlers {
                handler(arc.as_ref());
            }
        }
    }
}
