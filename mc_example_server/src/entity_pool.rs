use crate::entity::BoxedEntity;
use crate::entity::player::Player;
use mc_networking::packets::client_bound::*;
use mc_utils::Location;

use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, Error};

pub struct EntityPool {
    pub view_distance: u16,
    /// Entities must only be on one entity pool at the same time
    entities: HashMap<i32, Arc<RwLock<BoxedEntity>>>,
    /// Players can be in multiple entity pools at the same time
    players: HashMap<i32, Arc<RwLock<BoxedEntity>>>,
    synced_entities_location: HashMap<i32, Location>,
}

impl EntityPool {
    pub fn new(view_distance: u16) -> Self {
        Self {
            view_distance,
            entities: HashMap::new(),
            players: HashMap::new(),
            synced_entities_location: HashMap::new(),
        }
    }

    pub async fn can_see_each_other(&self, first: i32, second: i32) -> bool {
        let first_location = self.entities[&first].read().await.location().clone();
        let second_location = self.entities[&second].read().await.location().clone();
        first_location.distance2(&second_location) < (self.view_distance.pow(2) as f64)
    }

    pub async fn broadcast(&self, packet: &impl ClientBoundPacket) -> Result<()> {
        for entity in self.players.values() {
            let entity = entity.read().await;
            let player = entity.downcast_ref::<Player>().unwrap();
            player
                .client
                .lock()
                .await
                .send_packet(packet)
                .await?;
        }
        Ok(())
    }
    pub async fn send_to_player(&self, player: i32, packet: &impl ClientBoundPacket) -> Result<()> {
        self.players.get(&player).ok_or(Error::msg(""))?.read().await.as_player()?.client.lock().await.send_packet(packet).await?;

        Ok(())
    }
    pub async fn get_filtered_players(&self, filter: impl Fn(&Player) -> bool) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let mut players = HashMap::new();
        for (eid, player) in self.players.iter() {
            let result = {
                let player = player.read().await;
                let player = player.as_player().unwrap().as_ref();
                filter(player)
            };
            if result {
                players.insert(*eid, Arc::clone(player));
            }
        }
        players
    }
    /// Get all players in view distance of an entity
    pub async fn get_players_around(&self, eid: i32) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let mut players = HashMap::new();
        for (player_eid, player) in self.get_filtered_players(|p| p.entity_id != eid).await.into_iter() {
            if self.can_see_each_other(player_eid, eid).await {
                players.insert(player_eid, player);
            }
        }
        players
    }

    pub async fn tick(&mut self) {
        /*
        Update entities visibilities
        */
        for (eid, entity) in self.entities.iter().map(|(a, b)| (*a, Arc::clone(b))) {
            let entity_location = entity.read().await.location().clone();

            for (player_eid, player) in self.get_filtered_players(|player| player.entity_id != eid).await
            {
                let view_distance2 = self.view_distance.pow(2) as f64;
                let player_location = player.read().await.location().clone();
                let should_be_loaded =
                    entity_location.h_distance2(&player_location) < view_distance2;
                let is_loaded = player
                    .read()
                    .await
                    .as_player()
                    .unwrap()
                    .loaded_entities
                    .contains(&eid);
                if !is_loaded && should_be_loaded {
                    match &*entity.read().await {
                        // TODO: Implement it in the entity trait... somehow
                        BoxedEntity::Player(entity) => {
                            self.send_to_player(
                                player_eid,
                                &C04SpawnPlayer {
                                    entity_id: eid,
                                    uuid: entity.uuid.clone(),
                                    x: entity.location.x,
                                    y: entity.location.y,
                                    z: entity.location.z,
                                    yaw: entity.location.yaw_angle(),
                                    pitch: entity.location.pitch_angle(),
                                },
                            )
                                .await
                                .unwrap();
                        }
                        _ => unimplemented!(),
                    }
                    player
                        .write()
                        .await
                        .as_player_mut()
                        .unwrap()
                        .loaded_entities
                        .insert(eid);
                    self.send_to_player(
                        player_eid,
                        &C3AEntityHeadLook {
                            entity_id: eid,
                            head_yaw: entity.read().await.location().yaw_angle(),
                        },
                    )
                        .await
                        .unwrap();
                }
                if is_loaded && !should_be_loaded {
                    // TODO: Cache all entities that should be destroyed in that tick and send them all in one packet
                    self.send_to_player(
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
                        .unwrap()
                        .loaded_entities
                        .remove(&eid);
                }
            }
        }
        // Remove nonexisting entities that are loaded in players
        for player in self.players.values() {
            let player_eid = player.read().await.entity_id();
            let mut to_destroy = vec![];
            for eid in player
                .read()
                .await
                .as_player()
                .unwrap()
                .loaded_entities
                .iter()
            {
                if !self.entities.contains_key(&eid) {
                    to_destroy.push(*eid);
                }
            }
            {
                let mut players_mut = player.write().await;
                let players_mut = players_mut.as_player_mut().unwrap();
                for i in to_destroy.iter() {
                    players_mut.loaded_entities.remove(i);
                }
            }
            if !to_destroy.is_empty() {
                self.send_to_player(
                    player_eid,
                    &C36DestroyEntities {
                        entities: to_destroy,
                    },
                )
                    .await
                    .unwrap();
            }
        }

        /*
        Update entities positions
        */
        let entities = self.entities.clone();
        for (eid, entity) in entities {
            let previous_location = self.get_synced_entity_location(eid).clone();
            let new_location = entity.read().await.location().clone();
            if previous_location == new_location {
                continue;
            }
            let has_rotation_changed = !previous_location.rotation_eq(&new_location);
            let has_position_changed = !previous_location.position_eq(&new_location);
            self.synced_entities_location
                .insert(eid, new_location.clone());

            let on_ground = entity.read().await.on_ground();

            let players = self.get_players_around(eid).await;

            if has_rotation_changed {
                broadcast_to(
                    &C3AEntityHeadLook {
                        entity_id: eid,
                        head_yaw: new_location.yaw_angle(),
                    },
                    players.clone(),
                )
                    .await;
            }

            if previous_location.distance2(&new_location) > 8.0 * 8.0 {
                broadcast_to(
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
            } else if has_position_changed && has_rotation_changed {
                broadcast_to(
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
            } else if has_position_changed {
                broadcast_to(
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
            } else if has_rotation_changed {
                broadcast_to(
                    &C29EntityRotation {
                        entity_id: eid,
                        yaw: new_location.yaw_angle(),
                        pitch: new_location.pitch_angle(),
                        on_ground,
                    },
                    players,
                )
                    .await;
            } else {
                broadcast_to(&C2AEntityMovement { entity_id: eid }, players)
                    .await;
            }
        }
    }

    fn get_synced_entity_location(&mut self, eid: i32) -> Location {
        if !self.synced_entities_location.contains_key(&eid) {
            self.synced_entities_location.insert(eid, Location::default());
        }

        self.synced_entities_location[&eid].clone()
    }

    pub fn get_players(&self) -> &HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        &self.players
    }
    pub fn has_player(&self, player_id: i32) -> bool {
        self.players.contains_key(&player_id)
    }
    pub fn get_player(&self, player_id: i32) -> Option<Arc<RwLock<BoxedEntity>>> {
        self.players.get(&player_id).cloned()
    }
    pub async fn add_player(&mut self, player: Arc<RwLock<BoxedEntity>>) {
        let entity_id = player.read().await.entity_id();
        self.players.insert(entity_id, player);
    }
    pub fn remove_player(&mut self, player_id: i32) -> Option<Arc<RwLock<BoxedEntity>>> {
        self.players.remove(&player_id)
    }

    pub fn get_entities(&self) -> &HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        &self.entities
    }
    pub fn has_entity(&self, entity_id: i32) -> bool {
        self.entities.contains_key(&entity_id)
    }
    pub fn get_entity(&self, entity_id: i32) -> Option<Arc<RwLock<BoxedEntity>>> {
        self.entities.get(&entity_id).cloned()
    }
    pub async fn add_entity(&mut self, entity: Arc<RwLock<BoxedEntity>>) {
        let entity_id = entity.read().await.entity_id();
        self.entities.insert(entity_id, entity);
    }
    pub fn remove_entity(&mut self, entity_id: i32) -> Option<Arc<RwLock<BoxedEntity>>> {
        self.entities.remove(&entity_id)
    }

    pub async fn teleport_entity(&self, id: i32, location: Location) {
        self.entities[&id]
            .write()
            .await
            .set_location(location.clone());
        broadcast_to(
            &C56EntityTeleport {
                entity_id: id,
                x: location.x,
                y: location.y,
                z: location.z,
                yaw: location.yaw_angle(),
                pitch: location.pitch_angle(),
                on_ground: self.entities[&id].read().await.on_ground(),
            },
            self.get_players_around(id).await,
        )
        .await;
        let is_player = self.entities[&id].write().await.is_player();
        if is_player {
            self.send_to_player(
                id,
                &C34PlayerPositionAndLook {
                    x: location.x,
                    y: location.y,
                    z: location.z,
                    yaw: location.yaw,
                    pitch: location.pitch,
                    flags: 0,
                    teleport_id: 0,
                },
            )
                .await
                .unwrap();
        }
    }
    pub async fn update_entity_metadata(&self, entity_id: i32) -> Result<()> {
        let entity = self
            .get_entity(entity_id)
            .ok_or(Error::msg("Invalid "))?;
        let metadata = entity.read().await.metadata();

        broadcast_to(
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

pub async fn broadcast_to(
    packet: &impl ClientBoundPacket,
    players: HashMap<i32, Arc<RwLock<BoxedEntity>>>,
) {
    for (.., entity) in players {
        let entity = entity.read().await;
        let player = entity.downcast_ref::<Player>().unwrap();
        player
            .client
            .lock()
            .await
            .send_packet(packet)
            .await
            .unwrap();
    }
}
