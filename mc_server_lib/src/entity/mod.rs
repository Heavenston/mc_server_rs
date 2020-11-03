pub mod living_entity;
pub mod player;

use living_entity::LivingEntity;
use mc_networking::{
    data_types::{MetadataValue, Slot},
    packets::RawPacket,
};
use mc_utils::Location;
use player::Player;

use anyhow::Error;
use downcast_rs::{impl_downcast, DowncastSync};
use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct EntityEquipment<T: Borrow<Slot>> {
    pub main_hand: T,
    pub off_hand: T,
    pub head: T,
    pub chest: T,
    pub legs: T,
    pub feet: T,
}

pub trait Entity: Send + Sync + DowncastSync {
    fn entity_id(&self) -> i32;
    fn uuid(&self) -> &Uuid;

    fn get_spawn_packet(&self) -> RawPacket;

    fn get_equipment(&self) -> EntityEquipment<&Slot>;
    fn get_equipment_mut(&mut self) -> EntityEquipment<&mut Slot>;

    fn location(&self) -> &Location;
    fn location_mut(&mut self) -> &mut Location;
    fn set_location(&mut self, new_location: Location) {
        *self.location_mut() = new_location;
    }

    fn on_ground(&self) -> bool;
    fn set_on_ground(&mut self, on_ground: bool);

    fn metadata(&self) -> HashMap<u8, MetadataValue>;
    fn metadata_value(&self, id: u8) -> Option<MetadataValue>;
    fn set_metadata_value(&mut self, id: u8, value: MetadataValue) -> bool;
}
impl_downcast!(sync Entity);

pub enum BoxedEntity {
    Player(Box<Player>),
    LivingEntity(Box<LivingEntity>),
    Unknown(Box<dyn Entity>),
}
impl BoxedEntity {
    pub fn new(entity: impl Entity) -> Self {
        BoxedEntity::Unknown(Box::new(entity)).into_known()
    }

    pub fn is_unknown(&self) -> bool {
        match self {
            BoxedEntity::Unknown(..) => true,
            _ => false,
        }
    }

    pub fn is_player(&self) -> bool {
        match self {
            BoxedEntity::Player(..) => true,
            _ => false,
        }
    }
    pub fn as_player(&self) -> &Box<Player> {
        match self {
            BoxedEntity::Player(p) => p,
            _ => panic!("Entity is not a player"),
        }
    }
    pub fn as_player_mut(&mut self) -> &mut Box<Player> {
        match self {
            BoxedEntity::Player(p) => p,
            _ => panic!("Entity is not a player"),
        }
    }
    pub fn try_as_player(&self) -> Option<&Box<Player>> {
        match self {
            BoxedEntity::Player(p) => Some(p),
            _ => None,
        }
    }
    pub fn try_as_player_mut(&mut self) -> Option<&mut Box<Player>> {
        match self {
            BoxedEntity::Player(p) => Some(p),
            _ => None,
        }
    }

    pub fn is_living_entity(&self) -> bool {
        match self {
            BoxedEntity::LivingEntity(..) => true,
            _ => false,
        }
    }
    pub fn as_living_entity(&self) -> &Box<LivingEntity> {
        match self {
            BoxedEntity::LivingEntity(p) => p,
            _ => panic!("Entity is not a living_entity"),
        }
    }
    pub fn as_living_entity_mut(&mut self) -> &mut Box<LivingEntity> {
        match self {
            BoxedEntity::LivingEntity(p) => p,
            _ => panic!("Entity is not a living_entity"),
        }
    }
    pub fn try_as_living_entity(&self) -> Option<&Box<LivingEntity>> {
        match self {
            BoxedEntity::LivingEntity(p) => Some(p),
            _ => None,
        }
    }
    pub fn try_as_living_entity_mut(&mut self) -> Option<&mut Box<LivingEntity>> {
        match self {
            BoxedEntity::LivingEntity(p) => Some(p),
            _ => None,
        }
    }

    pub fn into_known(self) -> BoxedEntity {
        if let BoxedEntity::Unknown(entity) = self {
            if entity.is::<Player>() {
                BoxedEntity::Player(
                    entity
                        .downcast::<Player>()
                        .map_err(|_| Error::msg(""))
                        .unwrap(),
                )
            }
            else {
                BoxedEntity::Unknown(entity)
            }
        }
        else {
            self
        }
    }

    pub fn as_entity(&self) -> &dyn Entity {
        match self {
            BoxedEntity::Player(player) => player.as_ref(),
            BoxedEntity::LivingEntity(entity) => entity.as_ref(),
            BoxedEntity::Unknown(entity) => entity.as_ref(),
        }
    }
    pub fn as_entity_mut(&mut self) -> &mut dyn Entity {
        match self {
            BoxedEntity::Player(player) => player.as_mut(),
            BoxedEntity::LivingEntity(entity) => entity.as_mut(),
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
