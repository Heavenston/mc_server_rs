#[cfg(test)]
mod tests;

pub use mc_events_macros::*;

use dashmap::DashMap;
use downcast_rs::{impl_downcast, Downcast};
use std::any::TypeId;
use smallvec::SmallVec;

type ADashMap<K, V> = DashMap<K, V, ahash::RandomState>;

/// Traits for Events
pub trait Event: Downcast + Send + Sync {
    fn is_cancellable(&self) -> bool;
}
impl_downcast!(Event);

/// Trait for event handlers
pub trait EventHandler {
    fn event_type(&self) -> TypeId;
    fn on_event(&mut self, event: &mut dyn Event);
}

/// Define at which position the event handler should be called
pub enum EventHandlerPosition {
    /// The EventHandler will be added at the beginning of the handler list
    First,
    /// The EventHandler will be added at the end of the handler list
    Last,
    /// The EventHandler will always be called after all other handlers
    Final,
}

pub struct EventManager {
    pub handlers: ADashMap<
        TypeId,
        (Vec<Box<dyn EventHandler>>, SmallVec<[Box<dyn EventHandler>; 2]>)
    >,
}
impl EventManager {
    pub fn new() -> Self {
        Self {
            handlers: ADashMap::default(),
        }
    }

    /// Add an event handler with specified [EventHandlerPosition]
    /// if [Option::None] is given than it will use [EventHandlerPosition:Last]
    pub fn on<T: EventHandler + 'static>(&mut self, handler: T, position: Option<EventHandlerPosition>) {
        let position = position.unwrap_or(EventHandlerPosition::Last);
        let handler = Box::new(handler);
        let event_type = handler.event_type();
        let mut handlers = if let Some(handlers) = self.handlers.get_mut(&event_type) {
            handlers
        }
        else {
            self.handlers.insert(event_type, (vec![], SmallVec::new()));
            self.handlers.get_mut(&event_type).unwrap()
        };
        match position {
            EventHandlerPosition::First => handlers.0.insert(0, handler),
            EventHandlerPosition::Last => handlers.0.push(handler),
            EventHandlerPosition::Final => handlers.1.push(handler),
        }
    }

    /// Dispatch an event calling every event handler
    pub fn dispatch<E: Event>(&self, event: &mut E) {
        let event_type = TypeId::of::<E>();
        if !self.handlers.contains_key(&event_type) {
            return;
        }

        for handler in self.handlers.get_mut(&event_type).unwrap().0.iter_mut() {
            handler.on_event(event);
        }
        for handler in self.handlers.get_mut(&event_type).unwrap().1.iter_mut() {
            handler.on_event(event);
        }
    }
}
