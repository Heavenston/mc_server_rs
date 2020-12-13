pub mod chunk;

use std::sync::atomic::{AtomicI32, Ordering};
use uuid::Uuid;

use mc_networking::client::Client;
use mc_utils::Location;

const NETWORK_ID_COUNTER: AtomicI32 = AtomicI32::new(0);

#[readonly::make]
pub struct NetworkIdComponent(pub i32);
impl NetworkIdComponent {
    pub fn new() -> Self {
        Self(NETWORK_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

pub struct LocationComponent(pub Location);

pub struct MobKindComponent(pub i32);

pub struct ObjectUuidComponent(pub Uuid);

pub struct LivingEntityComponent;

pub struct ExperienceOrbComponent {
    pub count: i16,
}

pub struct ClientComponent(pub Client);

pub struct UsernameComponent(pub String);

pub struct CustomNameComponent(pub serde_json::Value);
