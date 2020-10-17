pub mod player;

use crate::location::Location;
use mc_networking::data_types::MetadataValue;
use crate::server::Server;
use crate::entity::player::Player;
use mc_networking::packets::client_bound::{ClientBoundPacket, C02SpawnLivingEntity};

use downcast_rs::{DowncastSync, impl_downcast};
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Error};
use std::ops::{Deref, DerefMut};

pub trait Entity: Send + Sync + DowncastSync {
    fn entity_id(&self) -> i32;
    fn uuid(&self) -> &Uuid;

    fn location(&self) -> &Location;
    fn location_mut(&mut self) -> &mut Location;
    fn set_location(&mut self, new_location: Location) {
        *self.location_mut() = new_location;
    }

    fn on_ground(&self) -> bool;
    fn set_on_ground(&mut self, on_ground: bool);

    fn metadata(&self) -> HashMap<i32, MetadataValue>;
    fn metadata_value(&self, id: i32) -> Option<MetadataValue>;
    fn set_metadata_value(&mut self, id: i32, value: MetadataValue) -> bool;
}
impl_downcast!(sync Entity);

pub enum BoxedEntity {
    Player(Box<Player>),
    Unknown(Box<dyn Entity>),
}
impl BoxedEntity {
    pub fn new(entity: impl Entity) -> Self {
        BoxedEntity::Unknown(Box::new(entity))
            .into_known()
    }

    pub fn is_player(&self) -> bool {
        match self {
            BoxedEntity::Player(..) => true,
            _ => false
        }
    }
    pub fn is_unknown(&self) -> bool {
        match self {
            BoxedEntity::Unknown(..) => true,
            _ => false
        }
    }

    pub fn as_player(&self) -> Result<&Box<Player>> {
        match self {
            BoxedEntity::Player(p) => Ok(p),
            _ => Err(Error::msg("Entity is not a player"))
        }
    }
    pub fn as_player_mut(&mut self) -> Result<&mut Box<Player>> {
        match self {
            BoxedEntity::Player(p) => Ok(p),
            _ => Err(Error::msg("Entity is not a player"))
        }
    }
    pub fn into_known(self) -> BoxedEntity {
        if let BoxedEntity::Unknown(entity) = self {
            if entity.is::<Player>() {
                BoxedEntity::Player(
                    entity.downcast::<Player>().map_err(|_| Error::msg("")).unwrap()
                )
            }
            else {
                BoxedEntity::Unknown(
                    entity
                )
            }
        }
        else {
            self
        }
    }

    pub fn as_entity(&self) -> &dyn Entity {
        match self {
            BoxedEntity::Player(player) => player.as_ref(),
            BoxedEntity::Unknown(entity) => entity.as_ref(),
        }
    }
    pub fn as_entity_mut(&mut self) -> &mut dyn Entity {
        match self {
            BoxedEntity::Player(player) => player.as_mut(),
            BoxedEntity::Unknown(entity) => entity.as_mut(),
        }
    }
}
impl Deref for BoxedEntity {
    type Target = dyn Entity;

    fn deref(&self) -> &Self::Target {
        self.as_entity()
    }
}
impl DerefMut for BoxedEntity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_entity_mut()
    }
}