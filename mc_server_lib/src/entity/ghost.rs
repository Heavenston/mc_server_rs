use super::{BoxedEntity, Entity, EntityEquipment};
use mc_networking::{
    data_types::{MetadataValue, Slot},
    packets::{client_bound::*, RawPacket},
};
use mc_utils::Location;

use std::{
    collections::HashMap,
    f64,
    sync::{Arc, Weak},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use uuid::Uuid;

fn tick(entity: Arc<RwLock<BoxedEntity>>) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut entity = entity.write().await;
        let entity = entity.as_ghost_mut();
        let target_player = entity.target_player.upgrade();
        if target_player.is_none() {
            // TODO: Add dead entities
            return;
        }
        let target_player = target_player.unwrap();
        let distance = target_player.read().await.as_player().held_item as f64 * 0.3;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64;
        let current_sec = current_time / 1000.0;

        let player_pos = target_player.read().await.location().clone();
        let rotation =
            (current_sec / 3.0 + entity.entity_id as f64 / 10.0).fract() * f64::consts::PI * 2.0;
        entity.location.x = player_pos.x + rotation.cos() * distance;
        entity.location.y = player_pos.y + 1.5;
        entity.location.z = player_pos.z + rotation.sin() * distance;
        entity.location.yaw = rotation.to_degrees() as f32 + 90.0;

        *entity.get_equipment_mut().main_hand =
            target_player.read().await.get_equipment().main_hand.clone();
        *entity.get_equipment_mut().off_hand =
            target_player.read().await.get_equipment().off_hand.clone();
    })
}

pub struct GhostEntity {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub location: Location,
    pub velocity: (i16, i16, i16),
    pub on_ground: bool,
    pub metadata: HashMap<u8, MetadataValue>,

    pub armor_head: Slot,
    pub armor_chest: Slot,
    pub armor_legs: Slot,
    pub armor_feet: Slot,
    pub main_hand: Slot,
    pub off_hand: Slot,

    pub target_player: Weak<RwLock<BoxedEntity>>,
    pub freeze_time: f64,
}
impl GhostEntity {
    pub fn new(eid: i32, uuid: Uuid, target_player: Weak<RwLock<BoxedEntity>>) -> Self {
        Self {
            entity_id: eid,
            uuid,
            location: Location {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 90.0,
            },
            velocity: (0, 0, 0),
            on_ground: true,
            metadata: HashMap::new(),

            armor_head: Slot::default(),
            armor_chest: Slot::default(),
            armor_legs: Slot::default(),
            armor_feet: Slot::default(),
            main_hand: Slot::default(),
            off_hand: Slot::default(),

            target_player,
            freeze_time: 0.0,
        }
    }
}
impl Entity for GhostEntity {
    fn entity_id(&self) -> i32 {
        self.entity_id
    }
    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn tick_fn(
        &self,
    ) -> Option<fn(entity: Arc<RwLock<BoxedEntity>>) -> tokio::task::JoinHandle<()>> {
        Some(tick)
    }
    fn get_spawn_packet(&self) -> RawPacket {
        C02SpawnLivingEntity {
            entity_id: self.entity_id(),
            entity_uuid: self.uuid.clone(),
            kind: 92,
            x: self.location.x,
            y: self.location.y,
            z: self.location.z,
            yaw: self.location.yaw_angle(),
            pitch: self.location.pitch_angle(),
            head_pitch: self.location.pitch_angle(),
            velocity_x: self.velocity.0,
            velocity_y: self.velocity.1,
            velocity_z: self.velocity.2,
        }
        .to_rawpacket()
    }

    fn get_equipment(&self) -> EntityEquipment<&Slot> {
        EntityEquipment {
            main_hand: &self.main_hand,
            off_hand: &self.off_hand,
            head: &self.armor_head,
            chest: &self.armor_chest,
            legs: &self.armor_legs,
            feet: &self.armor_feet,
        }
    }
    fn get_equipment_mut(&mut self) -> EntityEquipment<&mut Slot> {
        EntityEquipment {
            main_hand: &mut self.main_hand,
            off_hand: &mut self.off_hand,
            head: &mut self.armor_head,
            chest: &mut self.armor_chest,
            legs: &mut self.armor_legs,
            feet: &mut self.armor_feet,
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
        self.metadata.clone()
    }
    fn metadata_value(&self, id: u8) -> Option<MetadataValue> {
        self.metadata.get(&id).map(|v| v.clone())
    }
    fn set_metadata_value(&mut self, id: u8, value: MetadataValue) -> bool {
        self.metadata.insert(id, value);
        true
    }
}
