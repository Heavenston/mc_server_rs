use mc_networking::{
    data_types::{MetadataValue, Slot},
    packets::{client_bound::*, RawPacket},
};
use mc_server_lib::entity::{BoxedEntity, Entity, EntityEquipment};
use mc_utils::Location;

use lazy_static::lazy_static;
use std::{collections::HashMap, f32, f64, future::Future, pin::Pin, sync::Weak};
use tokio::{sync::RwLock, time::Instant};
use uuid::Uuid;

lazy_static! {
    static ref START_INSTANT: Instant = Instant::now();
}

async fn tick<'a>(entity: &'a mut GhostEntity) {
    let target_player = entity.target_player.upgrade();
    if target_player.is_none() {
        // TODO: Add dead entities
        return;
    }
    let target_player = target_player.unwrap();

    let target_player = target_player.read().await;
    let distance = target_player.as_player().held_item as f64 * 0.3;
    let player_pos = target_player.location().clone();
    drop(target_player);

    let current_sec = START_INSTANT.elapsed().as_millis() as f64 / 1000.0;

    let rotation =
        (current_sec / 3.0 + entity.entity_id as f64 / 10.0).fract() * f64::consts::PI * 2.0;
    entity.location.x = player_pos.x + rotation.cos() * distance;
    entity.location.y = player_pos.y + 1.7;
    entity.location.z = player_pos.z + rotation.sin() * distance;
    entity.location.yaw = rotation.to_degrees() as f32 + 90.0;
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

    fn tick_fn<'a>(&'a mut self) -> Option<Pin<Box<dyn 'a + Send + Sync + Future<Output = ()>>>> {
        Some(Box::pin(tick(self)))
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
