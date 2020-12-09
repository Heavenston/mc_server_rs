pub mod chunk;

use std::{
    ops::Deref,
    sync::atomic::{AtomicI32, Ordering},
};
use uuid::Uuid;

use mc_networking::client::Client;
use mc_utils::Location;

const NETWORK_ID_COUNTER: AtomicI32 = AtomicI32::new(0);

#[readonly::make]
pub struct NetworkIdComponent {
    pub id: i32,
}
impl NetworkIdComponent {
    pub fn new() -> Self {
        Self {
            id: NETWORK_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
}

pub struct LocationComponent {
    pub loc: Location,
}

pub struct MobKindComponent(i32);

pub struct ObjectUuidComponent(Uuid);

pub struct LivingEntityComponent;

pub struct ExperienceOrbComponent {
    pub count: i16,
}

pub struct ClientComponent {
    pub client: Client,
}

pub struct UsernameComponent(String);

pub struct CustomNameComponent(serde_json::Value);
