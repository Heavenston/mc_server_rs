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

    event_manager.on(OnEvent);
    let mut event = EmptyEvent("didn't work".to_string());
    event_manager.dispatch(&mut event);
    assert_eq!(event.0, "worked");

    event_manager.on(OnEventTwo);
    event_manager.dispatch(&mut event);

    assert_eq!(event.0, "worked too!");
}
