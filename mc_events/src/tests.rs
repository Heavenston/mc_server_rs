use crate::*;
struct EmptyEvent(String);
impl Event for EmptyEvent {
    fn is_cancellable(&self) -> bool {
        false
    }
}

#[event_callback(OnEvent)]
fn on_event(event: &mut EmptyEvent) {
    event.0 = "worked".to_string();
}

#[event_callback(OnEventTwo)]
fn on_event(event: &mut EmptyEvent) {
    event.0 = "worked too!".to_string();
}

#[test]
fn dispatching() {
    let mut event_manager = EventManager::new();

    event_manager.on(OnEvent, None);
    let mut event = EmptyEvent("didn't work".to_string());
    event_manager.dispatch(&mut event);
    assert_eq!(event.0, "worked");

    event_manager.on(OnEventTwo, None);
    event_manager.dispatch(&mut event);

    assert_eq!(event.0, "worked too!");
}

struct CounterEvent(u32);
impl Event for CounterEvent {
    fn is_cancellable(&self) -> bool {
        false
    }
}

struct CounterHandler(u32);
impl EventHandler for CounterHandler {
    fn event_type(&self) -> TypeId {
        TypeId::of::<CounterEvent>()
    }
    fn on_event(&mut self, event: &mut dyn Event) {
        let counter_event = event.downcast_mut::<CounterEvent>().unwrap();
        counter_event.0 += 1;
        assert_eq!(counter_event.0, self.0);
    }
}

#[test]
fn positions() {
    let mut event_manager = EventManager::new();

    event_manager.on(CounterHandler(5), Some(EventHandlerPosition::Final));
    event_manager.on(CounterHandler(3), Some(EventHandlerPosition::Last));
    event_manager.on(CounterHandler(2), Some(EventHandlerPosition::First));
    event_manager.on(CounterHandler(6), Some(EventHandlerPosition::Final));
    event_manager.on(CounterHandler(4), Some(EventHandlerPosition::Last));
    event_manager.on(CounterHandler(1), Some(EventHandlerPosition::First));

    let mut event = CounterEvent(0);
    event_manager.dispatch(&mut event);
}

#[test]
fn once_events() {
    let mut event_manager = EventManager::new();

    event_manager.once(CounterHandler(1));
    
    let mut event = CounterEvent(0);
    event_manager.dispatch(&mut event);
    event.0 = 3456;
    event_manager.dispatch(&mut event);
    assert_eq!(event.0, 3456);
}