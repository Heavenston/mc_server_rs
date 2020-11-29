use super::{Entity, EntityEquipment};
use mc_networking::{
    data_types::{MetadataValue, Slot},
    packets::{client_bound::*, RawPacket},
};
use mc_utils::Location;

use std::collections::HashMap;
use uuid::Uuid;

pub struct LivingEntity {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub kind: i32,
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
}
impl LivingEntity {
    pub fn new(eid: i32, uuid: Uuid, kind: i32) -> Self {
        Self {
            entity_id: eid,
            uuid,
            kind,
            location: Location::default(),
            velocity: (0, 0, 0),
            on_ground: true,
            metadata: HashMap::new(),

            armor_head: Slot::default(),
            armor_chest: Slot::default(),
            armor_legs: Slot::default(),
            armor_feet: Slot::default(),
            main_hand: Slot::default(),
            off_hand: Slot::default(),
        }
    }
}
impl Entity for LivingEntity {
    fn entity_id(&self) -> i32 {
        self.entity_id
    }
    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn get_spawn_packet(&self) -> RawPacket {
        C02SpawnLivingEntity {
            entity_id: self.entity_id(),
            entity_uuid: self.uuid,
            kind: self.kind,
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
        self.metadata.get(&id).cloned()
    }
    fn set_metadata_value(&mut self, id: u8, value: MetadataValue) -> bool {
        self.metadata.insert(id, value);
        true
    }
}
