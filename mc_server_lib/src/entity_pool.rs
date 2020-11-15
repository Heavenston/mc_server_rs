use crate::{
    entity::{BoxedEntity, EntityEquipment},
    entity_manager::{BoxedEntityManager, EntityManager, PlayerManager, PlayerWrapper},
};
use mc_networking::{data_types::Slot, packets::client_bound::*};
use mc_utils::Location;

use anyhow::Result;
use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use tokio::sync::RwLock;

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

fn default_visibility_function(
    entity_bool: &EntityPool,
    first: &BoxedEntity,
    second: &BoxedEntity,
) -> bool {
    first.location().distance2(second.location()) < (entity_bool.view_distance.pow(2) as f64)
}

type EntityVisibilityFunction =
    Box<dyn Send + Sync + Fn(&EntityPool, &BoxedEntity, &BoxedEntity) -> bool>;

pub struct EntityPool {
    pub view_distance: u16,
    /// Entities must only be on one entity pool at the same time
    pub entities: RwLock<BoxedEntityManager>,
    /// Players can be in multiple entity pools at the same time
    pub players: RwLock<PlayerManager>,
    synced_entities_location: RwLock<FxIndexMap<i32, Location>>,
    synced_entities_equipments: RwLock<FxIndexMap<i32, EntityEquipment<Slot>>>,
    entity_visibility_function: EntityVisibilityFunction,
}

impl EntityPool {
    pub fn new(
        view_distance: u16,
        entity_visibility_function: Option<EntityVisibilityFunction>,
    ) -> Self {
        Self {
            view_distance,
            entities: RwLock::new(BoxedEntityManager::new()),
            players: RwLock::new(PlayerManager::new()),
            synced_entities_location: RwLock::default(),
            synced_entities_equipments: RwLock::default(),
            entity_visibility_function: entity_visibility_function
                .unwrap_or(Box::new(default_visibility_function)),
        }
    }

    /// Calls the provided entity visibility function pointer
    pub async fn can_see_each_other(&self, first: &BoxedEntity, second: &BoxedEntity) -> bool {
        (self.entity_visibility_function)(self, &*first, &*second)
    }
    /// Get all players in view distance of an entity
    pub async fn get_players_around(&self, source: &BoxedEntity) -> Vec<PlayerWrapper> {
        let mut players_around = vec![];
        let source_id = source.entity_id();
        let players = self.players.read().await;
        for (player_eid, player_wrapper) in players.iter() {
            if player_eid == source_id {
                continue;
            }
            let player = player_wrapper.read().await;
            if self.can_see_each_other(source, &*player).await {
                players_around.push(player_wrapper.clone());
            }
        }
        players_around
    }
    pub async fn tick(&self) {
        let entities = self.entities.read().await.clone();
        let mut synced_entities_location = self.synced_entities_location.write().await;
        let players_ids = self
            .players
            .read()
            .await
            .iter()
            .map(|(k, v)| (k, v.clone()))
            .collect::<Vec<_>>();
        for (eid, entity_arc) in entities {
            let mut entity = entity_arc.write().await;
            // Remove if remove_scheduled is true
            {
                if entity.remove_scheduled() {
                    self.entities.write().await.remove_entity(eid);
                    continue;
                }
            }
            // TICK THE ENTITY
            {
                let tick_future = entity.tick_fn();
                if let Some(tick_future) = tick_future {
                    tick_future.await;
                }
            }
            // Sync entity position to players
            {
                let has_location_changed = synced_entities_location
                    .get(&eid)
                    .map(|l| l != entity.location())
                    .unwrap_or(true);
                if has_location_changed {
                    let previous_location = synced_entities_location
                        .get(&eid)
                        .unwrap_or(entity.location())
                        .clone();
                    let new_location = entity.location();

                    let has_rotation_changed = !previous_location.rotation_eq(&new_location);
                    let has_position_changed = !previous_location.position_eq(&new_location);
                    synced_entities_location.insert(eid, new_location.clone());

                    let on_ground = entity.on_ground();
                    let players = self.get_players_around(&*entity).await;

                    if has_rotation_changed {
                        EntityManager::broadcast_to(
                            &C3AEntityHeadLook {
                                entity_id: eid,
                                head_yaw: new_location.yaw_angle(),
                            },
                            &players,
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
                            &players,
                        )
                        .await;
                    }
                    else if has_position_changed && has_rotation_changed {
                        EntityManager::broadcast_to(
                            &C28EntityPositionAndRotation {
                                entity_id: eid,
                                delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                yaw: new_location.yaw_angle(),
                                pitch: new_location.pitch_angle(),
                                on_ground,
                            },
                            &players,
                        )
                        .await;
                    }
                    else if has_position_changed {
                        EntityManager::broadcast_to(
                            &C27EntityPosition {
                                entity_id: eid,
                                delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64)
                                    * 128f64)
                                    .round() as i16,
                                on_ground,
                            },
                            &players,
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
                            &players,
                        )
                        .await;
                    }
                    else {
                        EntityManager::broadcast_to(
                            &C2AEntityMovement { entity_id: eid },
                            &players,
                        )
                        .await;
                    }
                }
            }
            // Sync equipment to players
            {
                let packet = {
                    drop(entity);
                    let synced_equipment = self.get_synced_entity_equipment(eid).await;
                    let synced_equipment = synced_equipment.to_ref();
                    entity = entity_arc.write().await;
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
                            packet.equipment.push((
                                C47EntityEquipmentSlot::OffHand,
                                equipment.off_hand.clone(),
                            ));
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
                    PlayerManager::broadcast_to(&packet, &self.get_players_around(&*entity).await)
                        .await;
                }
            }
            // Sync visibility to players
            {
                for (player_eid, player) in players_ids.iter().cloned() {
                    // Avoid sending player entity to itself
                    if player_eid == eid {
                        continue;
                    }
                    let view_distance2 = self.view_distance.pow(2) as f64;
                    let player_location = player.read().await.location().clone();
                    let is_loaded = player
                        .read()
                        .await
                        .as_player()
                        .loaded_entities
                        .contains(&eid);
                    // TODO: Remove limit
                    let should_be_loaded =
                        entity.location().h_distance2(&player_location) < view_distance2;
                    if !is_loaded && should_be_loaded {
                        player
                            .send_raw_packet(&entity.get_spawn_packet())
                            .await
                            .unwrap();
                        player
                            .write()
                            .await
                            .as_player_mut()
                            .loaded_entities
                            .insert(eid);
                        // Send head look
                        player
                            .send_packet(&C3AEntityHeadLook {
                                entity_id: eid,
                                head_yaw: entity.location().yaw_angle(),
                            })
                            .await
                            .unwrap();
                        // Send entity equipment
                        {
                            let equipment = entity.get_equipment().to_owned();
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
                        player
                            .send_packet(&C36DestroyEntities {
                                entities: vec![eid],
                            })
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
        }

        // Remove nonexisting entities that are still loaded
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

    pub async fn teleport_entity(&self, entity: &mut BoxedEntity, location: Location) {
        entity.set_location(location.clone());
        EntityManager::broadcast_to(
            &C56EntityTeleport {
                entity_id: entity.entity_id(),
                x: entity.location().x,
                y: entity.location().y,
                z: entity.location().z,
                yaw: entity.location().yaw_angle(),
                pitch: entity.location().pitch_angle(),
                on_ground: entity.on_ground(),
            },
            &self.get_players_around(entity).await,
        )
        .await;
        if let Some(player) = entity.try_as_player() {
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
    pub async fn sync_entity_metadata(&self, entity: &BoxedEntity) -> Result<()> {
        EntityManager::broadcast_to(
            &C44EntityMetadata {
                entity_id: entity.entity_id(),
                metadata: entity.metadata(),
            },
            &self.get_players_around(entity).await,
        )
        .await;

        Ok(())
    }
}
