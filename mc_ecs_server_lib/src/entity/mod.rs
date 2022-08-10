pub mod chunk;

use mc_networking::client::Client;
use mc_utils::Location;

use std::sync::atomic::{AtomicI32, Ordering};

use uuid::Uuid;
use bevy_ecs::component::Component;

const NETWORK_ID_COUNTER: AtomicI32 = AtomicI32::new(0);

#[derive(Component, Clone, Copy, Debug)]
#[readonly::make]
pub struct NetworkIdComponent(pub i32);
impl NetworkIdComponent {
    pub fn new() -> Self {
        Self(NETWORK_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Component)]
pub struct LocationComponent(pub Location);

#[derive(Component)]
pub struct MobKindComponent(pub i32);

#[derive(Component)]
pub struct ObjectUuidComponent(pub Uuid);

#[derive(Component)]
pub struct LivingEntityComponent;

#[derive(Component)]
pub struct ExperienceOrbComponent {
    pub count: i16,
}

#[derive(Component)]
pub struct ClientComponent(pub Client);

#[derive(Component)]
pub struct UsernameComponent(pub String);

#[derive(Component)]
pub struct CustomNameComponent(pub serde_json::Value);
