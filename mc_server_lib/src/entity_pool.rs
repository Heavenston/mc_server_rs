use crate::{
    entity::{BoxedEntity, EntityEquipment},
    entity_manager::{BoxedEntityManager, EntityManager, PlayerManager},
};
use mc_networking::{data_types::Slot, packets::client_bound::*};
use mc_utils::Location;

use anyhow::{Error, Result};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

pub struct EntityPool {
    pub view_distance: u16,
    /// Entities must only be on one entity pool at the same time
    pub entities: RwLock<BoxedEntityManager>,
    /// Players can be in multiple entity pools at the same time
    pub players: RwLock<PlayerManager>,
    synced_entities_location: RwLock<HashMap<i32, Location>>,
    synced_entities_equipments: RwLock<HashMap<i32, EntityEquipment<Slot>>>,
}

impl EntityPool {
    pub fn new(view_distance: u16) -> Self {
        Self {
            view_distance,
            entities: RwLock::new(BoxedEntityManager::new()),
            players: RwLock::new(PlayerManager::new()),
            synced_entities_location: RwLock::default(),
            synced_entities_equipments: RwLock::default(),
        }
    }

    pub async fn can_see_each_other(&self, first: i32, second: i32) -> bool {
        let first_location = self.entities.read().await[first]
            .read()
            .await
            .location()
            .clone();
        let second_location = self.entities.read().await[second]
            .read()
            .await
            .location()
            .clone();
        first_location.distance2(&second_location) < (self.view_distance.pow(2) as f64)
    }
    /// Get all players in view distance of an entity
    pub async fn get_players_around(&self, eid: i32) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let mut players = HashMap::new();
        for (player_eid, player) in self
            .players
            .read()
            .await
            .get_filtered_players(|p| p.entity_id != eid)
            .await
            .into_iter()
        {
            if self.can_see_each_other(player_eid, eid).await {
                players.insert(player_eid, player);
            }
        }
        players
    }
    pub async fn tick(&self) {
        /*
        TICK ALL ENTITIES
        */
        for (_, entity) in self.entities.read().await.iter() {
            let tick_fn = entity.read().await.tick_fn();
            if let Some(tick_fn) = tick_fn {
                tick_fn(Arc::clone(entity)).await.unwrap();
            }
        }

        /*
        Update entities positions
        */
        for (eid, entity) in self.entities.read().await.iter() {
            let previous_location = self.get_synced_entity_location(eid).await.clone();
            let new_location = entity.read().await.location().clone();
            if previous_location == new_location {
                continue;
            }
            let has_rotation_changed = !previous_location.rotation_eq(&new_location);
            let has_position_changed = !previous_location.position_eq(&new_location);
            self.synced_entities_location
                .write()
                .await
                .insert(eid, new_location.clone());

            let on_ground = entity.read().await.on_ground();

            let players = self.get_players_around(eid).await;

            if has_rotation_changed {
                EntityManager::broadcast_to(
                    &C3AEntityHeadLook {
                        entity_id: eid,
                        head_yaw: new_location.yaw_angle(),
                    },
                    players.clone(),
                )
                .await;
            }

            if previous_location.distance2(&new_location) > 8.0 * 8.0 {
                EntityManager::broadcast_to(
                    &C56EntityTeleport {
                        entity_id: eid,
                        x: new_location.x,
                        y: new_location.y,
                        z: new_location.z,
                        yaw: new_location.yaw_angle(),
                        pitch: new_location.pitch_angle(),
                        on_ground,
                    },
                    players,
                )
                .await;
            }
            else if has_position_changed && has_rotation_changed {
                EntityManager::broadcast_to(
                    &C28EntityPositionAndRotation {
                        entity_id: eid,
                        delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64) * 128f64)
                            .ceil() as i16,
                        delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64) * 128f64)
                            .ceil() as i16,
                        delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64) * 128f64)
                            .ceil() as i16,
                        yaw: new_location.yaw_angle(),
                        pitch: new_location.pitch_angle(),
                        on_ground,
                    },
                    players,
                )
                .await;
            }
            else if has_position_changed {
                EntityManager::broadcast_to(
                    &C27EntityPosition {
                        entity_id: eid,
                        delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64) * 128f64)
                            .ceil() as i16,
                        delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64) * 128f64)
                            .ceil() as i16,
                        delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64) * 128f64)
                            .ceil() as i16,
                        on_ground,
                    },
                    players,
                )
                .await;
            }
            else if has_rotation_changed {
                EntityManager::broadcast_to(
                    &C29EntityRotation {
                        entity_id: eid,
                        yaw: new_location.yaw_angle(),
                        pitch: new_location.pitch_angle(),
                        on_ground,
                    },
                    players,
                )
                .await;
            }
            else {
                EntityManager::broadcast_to(&C2AEntityMovement { entity_id: eid }, players).await;
            }
        }

        /*
        Update entities equipments
        */
        for (eid, entity) in self.entities.read().await.iter() {
            let packet = {
                let synced_equipment = self.get_synced_entity_equipment(eid).await;
                let synced_equipment = synced_equipment.to_ref();
                let entity = entity.read().await;
                let equipment = entity.get_equipment();
                if synced_equipment != equipment {
                    let mut packet = C47EntityEquipment {
                        entity_id: eid,
                        equipment: vec![],
                    };
                    if synced_equipment.main_hand != equipment.main_hand {
                        packet.equipment.push((
                            C47EntityEquipmentSlot::MainHand,
                            equipment.main_hand.clone(),
                        ));
                    }
                    if synced_equipment.off_hand != equipment.off_hand {
                        packet
                            .equipment
                            .push((C47EntityEquipmentSlot::OffHand, equipment.off_hand.clone()));
                    }
                    if synced_equipment.head != equipment.head {
                        packet
                            .equipment
                            .push((C47EntityEquipmentSlot::Head, equipment.head.clone()));
                    }
                    if synced_equipment.chest != equipment.chest {
                        packet
                            .equipment
                            .push((C47EntityEquipmentSlot::Chest, equipment.chest.clone()));
                    }
                    if synced_equipment.legs != equipment.legs {
                        packet
                            .equipment
                            .push((C47EntityEquipmentSlot::Legs, equipment.legs.clone()));
                    }
                    if synced_equipment.feet != equipment.feet {
                        packet
                            .equipment
                            .push((C47EntityEquipmentSlot::Feet, equipment.feet.clone()));
                    }
                    self.synced_entities_equipments
                        .write()
                        .await
                        .insert(eid, equipment.to_owned());
                    Some(packet)
                }
                else {
                    None
                }
            };
            if let Some(packet) = packet {
                PlayerManager::broadcast_to(&packet, self.get_players_around(eid).await).await;
            }
        }

        /*
        Update entities visibilities
        */
        for (eid, entity) in self.entities.read().await.iter() {
            let entity_location = entity.read().await.location().clone();

            let players = self
                .players
                .read()
                .await
                .get_filtered_players(|player| player.entity_id != eid)
                .await;
            for (player_eid, player) in players {
                let view_distance2 = self.view_distance.pow(2) as f64;
                let player_location = player.read().await.location().clone();
                if let Some(view_position) = player.read().await.as_player().view_position {
                    if view_position.0 != player_location.chunk_x()
                        || view_position.1 != player_location.chunk_z()
                    {
                        continue;
                    }
                }
                let should_be_loaded =
                    entity_location.h_distance2(&player_location) < view_distance2;
                let is_loaded = player
                    .read()
                    .await
                    .as_player()
                    .loaded_entities
                    .contains(&eid);
                if !is_loaded && should_be_loaded {
                    self.players
                        .write()
                        .await
                        .send_raw_to_player(player_eid, &entity.read().await.get_spawn_packet())
                        .await
                        .unwrap();
                    player
                        .write()
                        .await
                        .as_player_mut()
                        .loaded_entities
                        .insert(eid);
                    // Send head look
                    self.players
                        .read()
                        .await
                        .send_to_player(
                            player_eid,
                            &C3AEntityHeadLook {
                                entity_id: eid,
                                head_yaw: entity.read().await.location().yaw_angle(),
                            },
                        )
                        .await
                        .unwrap();
                    // Send entity equipment
                    {
                        let equipment = entity.read().await.get_equipment().to_owned();
                        let mut packet = C47EntityEquipment {
                            entity_id: eid,
                            equipment: vec![],
                        };
                        if equipment.main_hand.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::MainHand, equipment.main_hand));
                        }
                        if equipment.off_hand.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::OffHand, equipment.off_hand));
                        }
                        if equipment.head.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::Head, equipment.head));
                        }
                        if equipment.chest.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::Chest, equipment.chest));
                        }
                        if equipment.legs.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::Legs, equipment.legs));
                        }
                        if equipment.feet.is_present() {
                            packet
                                .equipment
                                .push((C47EntityEquipmentSlot::Feet, equipment.feet));
                        }
                        if packet.equipment.len() > 0 {
                            self.players
                                .read()
                                .await
                                .send_to_player(player_eid, &packet)
                                .await
                                .unwrap();
                        }
                    }
                }
                if is_loaded && !should_be_loaded {
                    // TODO: Cache all entities that should be destroyed in that tick and send them all in one packet
                    self.players
                        .read()
                        .await
                        .send_to_player(
                            player_eid,
                            &C36DestroyEntities {
                                entities: vec![eid],
                            },
                        )
                        .await
                        .unwrap();
                    player
                        .write()
                        .await
                        .as_player_mut()
                        .loaded_entities
                        .remove(&eid);
                }
            }
        }
        // Remove nonexisting entities that are loaded in players
        for player in self.players.read().await.entities() {
            let player_eid = player.read().await.entity_id();
            let mut to_destroy = vec![];
            for eid in player
                .read()
                .await
                .as_player()
                .loaded_entities
                .iter()
                .cloned()
            {
                if !self.entities.read().await.has_entity(eid) {
                    to_destroy.push(eid);
                }
            }
            {
                let mut players_mut = player.write().await;
                let players_mut = players_mut.as_player_mut();
                for i in to_destroy.iter() {
                    players_mut.loaded_entities.remove(i);
                }
            }
            if !to_destroy.is_empty() {
                self.players
                    .read()
                    .await
                    .send_to_player(
                        player_eid,
                        &C36DestroyEntities {
                            entities: to_destroy,
                        },
                    )
                    .await
                    .unwrap();
            }
        }
    }

    async fn get_synced_entity_location(&self, eid: i32) -> Location {
        if !self
            .synced_entities_location
            .read()
            .await
            .contains_key(&eid)
        {
            self.synced_entities_location
                .write()
                .await
                .insert(eid, Location::default());
        }

        self.synced_entities_location.read().await[&eid].clone()
    }
    async fn get_synced_entity_equipment(&self, eid: i32) -> EntityEquipment<Slot> {
        if !self
            .synced_entities_equipments
            .read()
            .await
            .contains_key(&eid)
        {
            let entity_equipment = self
                .entities
                .read()
                .await
                .get_entity(eid)
                .unwrap()
                .read()
                .await
                .get_equipment()
                .to_owned();
            self.synced_entities_equipments
                .write()
                .await
                .insert(eid, entity_equipment);
        }

        self.synced_entities_equipments.read().await[&eid]
            .to_ref()
            .to_owned()
    }

    pub async fn teleport_entity(&self, id: i32, location: Location) {
        self.entities.read().await[id]
            .write()
            .await
            .set_location(location.clone());
        EntityManager::broadcast_to(
            &C56EntityTeleport {
                entity_id: id,
                x: location.x,
                y: location.y,
                z: location.z,
                yaw: location.yaw_angle(),
                pitch: location.pitch_angle(),
                on_ground: self.entities.read().await[id].read().await.on_ground(),
            },
            self.get_players_around(id).await,
        )
        .await;
        if let Some(player) = self.entities.read().await[id].read().await.try_as_player() {
            player
                .client
                .read()
                .await
                .send_packet(&C34PlayerPositionAndLook {
                    x: location.x,
                    y: location.y,
                    z: location.z,
                    yaw: location.yaw,
                    pitch: location.pitch,
                    flags: 0,
                    teleport_id: 0,
                })
                .await
                .unwrap();
        }
    }
    pub async fn update_entity_metadata(&self, entity_id: i32) -> Result<()> {
        let entity = self
            .entities
            .read()
            .await
            .get_entity(entity_id)
            .ok_or(Error::msg("Invalid entity id"))?
            .clone();
        let metadata = entity.read().await.metadata();

        EntityManager::broadcast_to(
            &C44EntityMetadata {
                entity_id,
                metadata,
            },
            self.get_players_around(entity_id).await,
        )
        .await;

        Ok(())
    }
}
