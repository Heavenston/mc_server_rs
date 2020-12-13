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
    let event_manager = EventManager::new();

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

#[event_callback(CounterHandler)]
fn on_event(counter_event: &mut CounterEvent, count: &u32) {
    counter_event.0 += 1;
    assert_eq!(counter_event.0, *count);
}

#[test]
fn positions() {
    let event_manager = EventManager::new();

    event_manager.on(CounterHandler::new(5), Some(EventHandlerPosition::Final));
    event_manager.on(CounterHandler::new(3), Some(EventHandlerPosition::Last));
    event_manager.on(CounterHandler::new(2), Some(EventHandlerPosition::First));
    event_manager.on(CounterHandler::new(6), Some(EventHandlerPosition::Final));
    event_manager.on(CounterHandler::new(4), Some(EventHandlerPosition::Last));
    event_manager.on(CounterHandler::new(1), Some(EventHandlerPosition::First));

    let mut event = CounterEvent(0);
    event_manager.dispatch(&mut event);
}

#[test]
fn once_events() {
    let event_manager = EventManager::new();

    event_manager.once(CounterHandler::new(1));
    
    let mut event = CounterEvent(0);
    event_manager.dispatch(&mut event);
    event.0 = 3456;
    event_manager.dispatch(&mut event);
    assert_eq!(event.0, 3456);
}