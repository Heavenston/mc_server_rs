use super::Entity;
use mc_networking::{
    client::Client,
    data_types::{MetadataValue, Pose},
    map,
};
use mc_utils::Location;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;
use mc_networking::data_types::Slot;
use nbt::Map;

pub struct PlayerInventory {
    pub armor_head: Slot,
    pub armor_chest: Slot,
    pub armor_legs: Slot,
    pub armor_feet: Slot,
    pub crafting_input: Vec<Slot>,
    pub crafting_output: Slot,
    pub main_inventory: Vec<Slot>,
    pub hotbar: Vec<Slot>,
    pub offhand: Slot,
}
impl Default for PlayerInventory {
    fn default() -> Self {
        Self {
            armor_head: Slot::Present {
                item_id: 630,
                item_count: 1,
                nbt: nbt::Value::Compound(Map::new())
            },
            armor_chest: Slot::default(),
            armor_legs: Slot::default(),
            armor_feet: Slot::default(),
            crafting_input: vec![Slot::default(); 4],
            crafting_output: Slot::default(),
            main_inventory: vec![Slot::default(); 27],
            hotbar: vec![Slot::default(); 9],
            offhand: Slot::default()
        }
    }
}

pub struct Player {
    pub username: String,
    pub entity_id: i32,
    pub uuid: Uuid,
    pub client: Arc<RwLock<Client>>,

    pub inventory: PlayerInventory,
    pub location: Location,
    pub ping: i32,
    pub gamemode: u8,
    pub on_ground: bool,
    pub is_sneaking: bool,
    pub is_sprinting: bool,
    pub is_flying: bool,

    pub invulnerable: bool,
    pub can_fly: bool,
    pub flying_speed: f32,
    pub fov_modifier: f32,

    pub loaded_entities: HashSet<i32>,
    pub loaded_chunks: HashSet<(i32, i32)>,
}
impl Player {
    pub fn new(username: String, entity_id: i32, uuid: Uuid, client: Arc<RwLock<Client>>) -> Self {
        Self {
            username,
            entity_id,
            uuid,
            client,

            inventory: PlayerInventory::default(),
            location: Location::default(),
            ping: 0,
            gamemode: 0,
            on_ground: false,
            is_sneaking: false,
            is_sprinting: false,
            is_flying: false,

            invulnerable: false,
            can_fly: false,
            flying_speed: 0.05,
            fov_modifier: 0.1,

            loaded_entities: HashSet::new(),
            loaded_chunks: HashSet::new(),
        }
    }
}
impl Entity for Player {
    fn entity_id(&self) -> i32 {
        self.entity_id
    }
    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn location(&self) -> &Location {
        &self.location
    }
    fn location_mut(&mut self) -> &mut Location {
        &mut self.location
    }

    fn on_ground(&self) -> bool {
        self.on_ground
    }
    fn set_on_ground(&mut self, on_ground: bool) {
        self.on_ground = on_ground
    }

    fn metadata(&self) -> HashMap<u8, MetadataValue> {
        map! {
            0 => self.metadata_value(0).unwrap().clone(),
            6 => self.metadata_value(6).unwrap().clone()
        }
    }
    fn metadata_value(&self, id: u8) -> Option<MetadataValue> {
        Some(match id {
            0 => MetadataValue::Byte(
                (self.is_sneaking as u8) * 0x02 | (self.is_sprinting as u8) * 0x08,
            ),
            6 => MetadataValue::Pose(if self.is_sneaking && !self.is_flying {
                Pose::Sneaking
            }
            else {
                Pose::Standing
            }),
            _ => return None,
        })
    }
    fn set_metadata_value(&mut self, id: u8, value: MetadataValue) -> bool {
        match id {
            0 => {
                if let MetadataValue::Byte(flags) = value {
                    self.is_sneaking = flags & 0x02 == 0x02;
                    self.is_sprinting = flags & 0x08 == 0x08;
                    return true;
                }
            }
            _ => (),
        }

        false
    }
}
