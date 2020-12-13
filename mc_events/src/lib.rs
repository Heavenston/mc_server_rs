#[cfg(test)]
mod tests;

pub use mc_events_macros::*;

use dashmap::DashMap;
use downcast_rs::{impl_downcast, Downcast};
use std::any::TypeId;

type ADashMap<K, V> = DashMap<K, V, ahash::RandomState>;

pub trait Event: Downcast + Send + Sync {
    fn is_cancellable(&self) -> bool;
}
impl_downcast!(Event);

pub trait EventHandler {
    fn event_type(&self) -> TypeId;
    fn on_event(&self, event: &mut dyn Event);
}

pub struct EventManager {
    pub handlers: ADashMap<TypeId, Vec<Box<dyn EventHandler>>>,
}
impl EventManager {
    pub fn new() -> Self {
        Self {
            handlers: ADashMap::default(),
        }
    }

    pub fn on<T: EventHandler + 'static>(&mut self, handler: T) {
        let event_type = handler.event_type();
        if let Some(mut handlers) = self.handlers.get_mut(&event_type) {
            handlers.push(Box::new(handler))
        }
        else {
            self.handlers.insert(event_type, vec![Box::new(handler)]);
        }
    }

    pub fn dispatch<E: Event>(&self, event: &mut E) {
        let event_type = TypeId::of::<E>();
        if !self.handlers.contains_key(&event_type) {
            return;
        }

        for handler in self.handlers.get(&event_type).unwrap().iter() {
            handler.on_event(event);
        }
    }
}
