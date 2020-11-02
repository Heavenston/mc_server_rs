use super::Entity;
use mc_networking::{
    data_types::MetadataValue,
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
            entity_uuid: self.uuid.clone(),
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
