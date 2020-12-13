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

#[derive(Default)]
struct EventHandlers {
    normal: Vec<Box<dyn EventHandler>>,
    final_: SmallVec<[Box<dyn EventHandler>; 2]>,
    once: SmallVec<[Box<dyn EventHandler>; 1]>,
}

pub struct EventManager {
    handlers: ADashMap<TypeId, EventHandlers>,
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
            self.handlers.insert(event_type, EventHandlers::default());
            self.handlers.get_mut(&event_type).unwrap()
        };
        match position {
            EventHandlerPosition::First => handlers.normal.insert(0, handler),
            EventHandlerPosition::Last => handlers.normal.push(handler),
            EventHandlerPosition::Final => handlers.final_.push(handler),
        }
    }

    /// Schedule an event handler that will only be called once
    pub fn once<T: EventHandler + 'static>(&mut self, handler: T) {
        let event_type = handler.event_type();
        if let Some(mut handlers) = self.handlers.get_mut(&event_type) {
            handlers.once.push(Box::new(handler));
        }
        else {
            let mut handlers = EventHandlers::default();
            handlers.once.push(Box::new(handler));
            self.handlers.insert(event_type, handlers);
        };
    }

    /// Dispatch an event calling every event handler
    pub fn dispatch<E: Event>(&self, event: &mut E) {
        let event_type = TypeId::of::<E>();
        if !self.handlers.contains_key(&event_type) {
            return;
        }

        let mut handlers = self.handlers.get_mut(&event_type).unwrap();
        let mut once = SmallVec::new();
        std::mem::swap(&mut once, &mut handlers.once);
        for handler in once.iter_mut() {
            handler.on_event(event);
        }
        for handler in handlers.normal.iter_mut() {
            handler.on_event(event);
        }
        for handler in handlers.final_.iter_mut() {
            handler.on_event(event);
        }
    }
}
