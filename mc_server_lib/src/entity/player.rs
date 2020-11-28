use super::{BoxedEntity, Entity, EntityEquipment};
use mc_networking::{
    client::Client,
    data_types::{MetadataValue, Pose, Slot},
    map,
    packets::{client_bound::*, RawPacket},
};
use mc_utils::Location;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct PlayerRef {
    pub client: Arc<Client>,
    pub entity: Arc<RwLock<BoxedEntity>>,
}
impl PlayerRef {
    pub async fn new(entity: Arc<RwLock<BoxedEntity>>) -> Option<Self> {
        if !entity.read().await.is_player() {
            return None;
        }
        let client = Arc::clone(&entity.read().await.as_player().client);
        Some(Self { client, entity })
    }

    /// Sends the C1DChangeGameState packet
    /// Note that the abilities should be send
    pub async fn update_gamemode(&self) {
        self.send_packet(&C1DChangeGameState {
            reason: 3, // Change Gamemode
            value: self.entity.read().await.as_player().gamemode as f32,
        })
        .await
        .unwrap();
    }

    /// Sends the C30PlayerAbilities packet
    pub async fn update_abilities(&self) -> Result<()> {
        let player = self.entity.read().await;
        let player = player.as_player();
        player
            .client
            .send_player_abilities(
                player.invulnerable,
                player.is_flying,
                player.can_fly,
                player.gamemode == 1,
                player.flying_speed,
                player.fov_modifier,
            )
            .await?;
        Ok(())
    }
}
impl Into<Arc<RwLock<BoxedEntity>>> for PlayerRef {
    fn into(self) -> Arc<RwLock<BoxedEntity>> {
        self.entity
    }
}
impl Borrow<Arc<RwLock<BoxedEntity>>> for PlayerRef {
    fn borrow(&self) -> &Arc<RwLock<BoxedEntity>> {
        &self.entity
    }
}
impl Deref for PlayerRef {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[derive(Serialize, Deserialize)]
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
                nbt: nbt::Blob::new(),
            },
            armor_chest: Slot::default(),
            armor_legs: Slot::default(),
            armor_feet: Slot::default(),
            crafting_input: vec![Slot::default(); 4],
            crafting_output: Slot::default(),
            main_inventory: vec![Slot::default(); 27],
            hotbar: vec![Slot::default(); 9],
            offhand: Slot::default(),
        }
    }
}

pub struct PlayerEntity {
    pub username: String,
    pub entity_id: i32,
    pub uuid: Uuid,
    pub client: Arc<Client>,

    pub inventory: PlayerInventory,
    pub held_item: u8,
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
    pub view_position: Option<(i32, i32)>,
}
impl PlayerEntity {
    pub fn new(username: String, entity_id: i32, uuid: Uuid, client: Client) -> Self {
        Self {
            username,
            entity_id,
            uuid,
            client: Arc::new(client),

            inventory: PlayerInventory::default(),
            held_item: 0,
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
            view_position: None,
        }
    }
}
impl Entity for PlayerEntity {
    fn entity_id(&self) -> i32 {
        self.entity_id
    }
    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn get_spawn_packet(&self) -> RawPacket {
        C04SpawnPlayer {
            entity_id: self.entity_id(),
            uuid: self.uuid.clone(),
            x: self.location.x,
            y: self.location.y,
            z: self.location.z,
            yaw: self.location.yaw_angle(),
            pitch: self.location.pitch_angle(),
        }
        .to_rawpacket()
    }

    fn get_equipment(&self) -> EntityEquipment<&Slot> {
        EntityEquipment {
            main_hand: &self.inventory.hotbar[self.held_item as usize],
            off_hand: &self.inventory.offhand,
            head: &self.inventory.armor_head,
            chest: &self.inventory.armor_chest,
            legs: &self.inventory.armor_legs,
            feet: &self.inventory.armor_feet,
        }
    }
    fn get_equipment_mut(&mut self) -> EntityEquipment<&mut Slot> {
        EntityEquipment {
            main_hand: &mut self.inventory.hotbar[self.held_item as usize],
            off_hand: &mut self.inventory.offhand,
            head: &mut self.inventory.armor_head,
            chest: &mut self.inventory.armor_chest,
            legs: &mut self.inventory.armor_legs,
            feet: &mut self.inventory.armor_feet,
        }
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
