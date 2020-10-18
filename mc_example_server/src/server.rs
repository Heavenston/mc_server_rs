use crate::entity::player::Player;
use crate::entity::BoxedEntity;
use mc_networking::client::client_event::*;
use mc_networking::client::Client;
use mc_networking::map;
use mc_networking::packets::client_bound::*;
use mc_utils::Location;

use anyhow::{Result, Error};
use log::*;
use mc_networking::data_types::bitbuffer::BitBuffer;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

pub struct Server {
    entities: HashMap<i32, Arc<RwLock<BoxedEntity>>>,
    synced_entities_locations: HashMap<i32, Location>,
    entity_id_counter: i32,
    max_players: u16,
    view_distance: u16,
    brand: String,
}

impl Server {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            synced_entities_locations: HashMap::new(),
            entity_id_counter: 0,
            max_players: 10,
            view_distance: 10,
            brand: "BEST SERVER EVER".to_string(),
        }
    }
    pub async fn listen(server: Arc<RwLock<Server>>, addr: impl ToSocketAddrs) -> Result<()> {
        let mut listener = TcpListener::bind(addr).await?;
        loop {
            let (socket, ..) = listener.accept().await?;
            let (client, event_receiver) = Client::new(socket);
            let client = Arc::new(Mutex::new(client));

            tokio::task::spawn({
                let server = Arc::clone(&server);
                let client = Arc::clone(&client);
                async move {
                    Server::handle_client(server, client, event_receiver)
                        .await
                        .unwrap();
                }
            });
        }
    }
    async fn handle_client(
        server: Arc<RwLock<Server>>,
        client: Arc<Mutex<Client>>,
        mut event_receiver: tokio::sync::mpsc::Receiver<ClientEvent>,
    ) -> Result<()> {
        let mut player: Option<Arc<RwLock<BoxedEntity>>> = None;
        let mut player_eid = -1i32;

        while let Some(event) = event_receiver.recv().await {
            match event {
                ClientEvent::ServerListPing { response } => {
                    response
                        .send(json!({
                            "version": {
                                "name": "1.16.3",
                                "protocol": 753
                            },
                            "players": {
                                "max": server.read().await.max_players,
                                "online": server.read().await.get_players().await.len(),
                                "sample": []
                            },
                            "description": "Hi"
                        }))
                        .unwrap();
                }
                ClientEvent::LoginStart { response, username } => {
                    let mut server_write = server.write().await;
                    if (server_write.max_players as usize) <= server_write.get_players().await.len()
                    {
                        response
                            .send(LoginStartResult::Disconnect {
                                reason: "The server is full :(".to_string(),
                            })
                            .unwrap();
                    } else {
                        server_write.entity_id_counter += 1;
                        player_eid = server_write.entity_id_counter;
                        let uuid = Uuid::new_v4();
                        let entity = Arc::new(RwLock::new(BoxedEntity::new(Player::new(
                            username.clone(),
                            server_write.entity_id_counter,
                            uuid.clone(),
                            Arc::clone(&client),
                        ))));
                        player = Some(entity);

                        info!(
                            "{} joined the game, EID: {}, UUID: {}",
                            username.clone(),
                            player_eid,
                            uuid.clone()
                        );

                        response
                            .send(LoginStartResult::Accept {
                                uuid: uuid.clone(),
                                username,
                            })
                            .unwrap();
                    }
                }
                ClientEvent::LoggedIn => {
                    let player = player.as_ref().unwrap();

                    // Join Game
                    client
                        .lock()
                        .await
                        .send_packet(&{
                            let server = server.read().await;
                            let player = player.read().await;
                            let player = player.as_player().unwrap();

                            C24JoinGame {
                                entity_id: player.entity_id,
                                is_hardcore: false,
                                gamemode: player.gamemode,
                                previous_gamemode: player.gamemode,
                                world_names: vec!["heav:world".to_owned()],
                                dimension_codec: C24JoinGameDimensionCodec {
                                    dimensions: map! {
                                        "heav:world".to_owned() => C24JoinGameDimensionElement {
                                            natural: 1,
                                            ambient_light: 1.0,
                                            has_ceiling: 0,
                                            has_skylight: 1,
                                            fixed_time: 6000,
                                            shrunk: 0,
                                            ultrawarm: 0,
                                            has_raids: 0,
                                            respawn_anchor_works: 0,
                                            bed_works: 0,
                                            coordinate_scale: 1.0,
                                            piglin_safe: 0,
                                            logical_height: 256,
                                            infiniburn: "".to_owned(),
                                        }
                                    },
                                    biomes: map! {
                                        "minecraft:plains".to_owned() => C24JoinGameBiomeElement {
                                            precipitation: "none".to_owned(),
                                            effects: C24JoinGameBiomeEffects {
                                                sky_color: 7907327,
                                                water_fog_color: 329011,
                                                fog_color: 12638463,
                                                water_color: 4159204,
                                                mood_sound: C24JoinGameBiomeEffectsMoodSound {
                                                    tick_delay: 6000,
                                                    offset: 2.0,
                                                    sound: "minecraft:ambient.cave".to_owned(),
                                                    block_search_extent: 8,
                                                }
                                            },
                                            depth: 0.125,
                                            temperature: 0.8,
                                            scale: 0.5,
                                            downfall: 0.4,
                                            category: "none".to_owned(),
                                        },
                                        "heav:plot".to_owned() => C24JoinGameBiomeElement {
                                            precipitation: "none".to_owned(),
                                            effects: C24JoinGameBiomeEffects {
                                                sky_color: 0x7BA4FF,
                                                water_fog_color: 0x050533,
                                                fog_color: 0xC0D8FF,
                                                water_color: 0x3F76E4,
                                                mood_sound: C24JoinGameBiomeEffectsMoodSound {
                                                    tick_delay: 6000,
                                                    offset: 2.0,
                                                    sound: "minecraft:ambient.cave".to_owned(),
                                                    block_search_extent: 8,
                                                }
                                            },
                                            depth: 0.1,
                                            temperature: 0.5,
                                            scale: 0.2,
                                            downfall: 0.5,
                                            category: "none".to_owned(),
                                        }
                                    },
                                },
                                dimension: C24JoinGameDimensionElement {
                                    natural: 1,
                                    ambient_light: 1.0,
                                    has_ceiling: 0,
                                    has_skylight: 1,
                                    fixed_time: 6000,
                                    shrunk: 0,
                                    ultrawarm: 0,
                                    has_raids: 0,
                                    respawn_anchor_works: 0,
                                    bed_works: 0,
                                    coordinate_scale: 1.0,
                                    piglin_safe: 0,
                                    logical_height: 256,
                                    infiniburn: "".to_owned(),
                                },
                                world_name: "heav:world".to_owned(),
                                hashed_seed: 0,
                                max_players: server.max_players as i32,
                                view_distance: server.view_distance as i32,
                                reduced_debug_info: false,
                                enable_respawn_screen: true,
                                is_debug: false,
                                is_flat: true,
                            }
                        })
                        .await
                        .unwrap();

                    let my_player_info = C32PlayerInfoPlayerUpdate::AddPlayer {
                        uuid: player.read().await.uuid().clone(),
                        name: player.read().await.as_player().unwrap().username.clone(),
                        properties: vec![],
                        gamemode: player.read().await.as_player().unwrap().gamemode as i32,
                        ping: player.read().await.as_player().unwrap().ping,
                        display_name: None,
                    };
                    // Send to him all players (and himself)
                    {
                        let server = server.read().await;
                        let players = {
                            let mut players = vec![my_player_info.clone()];
                            for (.., player) in server.get_players().await {
                                let player = player.read().await;
                                let player = player.as_player().unwrap();
                                players.push(C32PlayerInfoPlayerUpdate::AddPlayer {
                                    uuid: player.uuid.clone(),
                                    name: player.username.clone(),
                                    properties: vec![],
                                    gamemode: player.gamemode as i32,
                                    ping: player.ping,
                                    display_name: None,
                                });
                            }
                            players
                        };
                        client
                            .lock()
                            .await
                            .send_packet(&C32PlayerInfo { players })
                            .await
                            .unwrap();
                    }
                    // Send to all his player info
                    server
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![my_player_info],
                        })
                        .await;

                    client
                        .lock()
                        .await
                        .send_player_abilities(false, false, true, false, 0.05, 0.1)
                        .await
                        .unwrap();

                    server.write().await.add_entity(Arc::clone(player)).await;

                    // Send server brand
                    {
                        let brand = server.read().await.brand.clone();
                        server
                            .read()
                            .await
                            .send_to_player(player_eid, &{
                                let mut builder =
                                    C17PluginMessageBuilder::new("minecraft:brand".to_string());
                                builder.encoder.write_string(&brand);
                                builder.build()
                            })
                            .await
                            .unwrap();
                    }

                    server
                        .read()
                        .await
                        .update_player_view_position(player_eid)
                        .await
                        .unwrap();
                    // Update position
                    server
                        .read()
                        .await
                        .teleport_entity(
                            player_eid,
                            Location {
                                x: 0.0,
                                y: 20.0,
                                z: 0.0,
                                yaw: 0.0,
                                pitch: 0.0,
                            },
                        )
                        .await;
                }
                ClientEvent::Logout => {
                    server.write().await.remove_entity(player_eid).await;
                    let uuid = player.unwrap().read().await.uuid().clone();
                    server
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::RemovePlayer { uuid }],
                        })
                        .await;
                    break;
                }

                ClientEvent::Ping { delay } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.as_player_mut().unwrap().ping = delay as i32;
                    let uuid = player.read().await.as_player().unwrap().uuid.clone();
                    server
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::UpdateLatency {
                                uuid: uuid.clone(),
                                ping: delay as i32,
                            }],
                        })
                        .await;
                }

                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let last_location = player.as_ref().unwrap().read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw: last_location.yaw,
                        pitch: last_location.pitch,
                    };
                    {
                        let mut player = player.as_ref().unwrap().write().await;
                        player.set_on_ground(on_ground);
                        player.set_location(new_location.clone());
                    }
                    let player_id = player.as_ref().unwrap().read().await.entity_id();
                    if last_location.chunk_x() != new_location.chunk_x()
                        || last_location.chunk_z() != new_location.chunk_z()
                    {
                        server
                            .write()
                            .await
                            .update_player_view_position(player_id)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerPositionAndRotation {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let last_location = player.as_ref().unwrap().read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw,
                        pitch,
                    };
                    {
                        let mut player = player.as_ref().unwrap().write().await;
                        player.set_on_ground(on_ground);
                        player.set_location(new_location.clone());
                    }
                    let player_id = player.as_ref().unwrap().read().await.entity_id();
                    if last_location.chunk_x() != new_location.chunk_x()
                        || last_location.chunk_z() != new_location.chunk_z()
                    {
                        server
                            .write()
                            .await
                            .update_player_view_position(player_id)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerRotation {
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let mut player = player.as_ref().unwrap().write().await;
                    player.set_on_ground(on_ground);
                    let location = player.location().clone();
                    player.set_location(Location {
                        x: location.x,
                        y: location.y,
                        z: location.z,
                        yaw,
                        pitch,
                    });
                }

                ClientEvent::EntityAction {
                    entity_id,
                    action_id,
                    ..
                } => {

                    if entity_id == player_eid {
                        {
                            let mut player = player.as_ref().unwrap().write().await;
                            let mut player = player.as_player_mut().unwrap();
                            match action_id {
                                0 => {
                                    player.is_sneaking = true;
                                }
                                1 => {
                                    player.is_sneaking = false;
                                }
                                3 => {
                                    player.is_sprinting = true;
                                }
                                4 => {
                                    player.is_sprinting = false;
                                }
                                _ => (),
                            }
                        }
                        server.read().await.update_entity_metadata(player_eid).await.unwrap();
                    }
                }
                ClientEvent::PlayerAbilities { is_flying } => {
                    player
                        .as_ref()
                        .unwrap()
                        .write()
                        .await
                        .as_player_mut()
                        .unwrap()
                        .is_flying = is_flying;
                    server.read().await.update_entity_metadata(player_eid).await.unwrap();
                }
            }
        }

        Ok(())
    }

    async fn update_player_view_position(&self, player_id: i32) -> Result<()> {
        let entity = &self.entities[&player_id];
        let location = entity.read().await.location().clone();

        let (chunk_x, chunk_z) = (location.chunk_x(), location.chunk_z());

        self.send_to_player(player_id, &C40UpdateViewPosition { chunk_x, chunk_z })
            .await
            .unwrap();
        let loaded_chunks = entity.read().await.as_player()?.loaded_chunks.clone();

        let view_distance = self.view_distance as i32;
        let view_distance2 = self.view_distance.pow(2) as i32;

        for (x, z) in loaded_chunks.iter().cloned() {
            if loaded_chunks.contains(&(x, z))
                && ((chunk_x - x).pow(2) + (chunk_z - z).pow(2)) >= view_distance2
            {
                self.send_to_player(
                    player_id,
                    &C1CUnloadChunk {
                        chunk_x: x,
                        chunk_z: z,
                    },
                )
                .await
                .unwrap();
                entity
                    .write()
                    .await
                    .as_player_mut()?
                    .loaded_chunks
                    .remove(&(x, z));
            }
        }
        for x in chunk_x - (view_distance * 2)..=chunk_x + (view_distance * 2) {
            for z in chunk_z - (view_distance * 2)..=chunk_z + (view_distance * 2) {
                if !loaded_chunks.contains(&(x, z))
                    && (chunk_x - x).pow(2) + (chunk_z - z).pow(2) <= view_distance2
                {
                    let chunk_data = self.get_chunk(x, z).await;
                    self.send_to_player(player_id, &chunk_data).await.unwrap();
                    entity
                        .write()
                        .await
                        .as_player_mut()?
                        .loaded_chunks
                        .insert((x, z));
                }
            }
        }
        Ok(())
    }
    async fn get_chunk(&self, chunk_x: i32, chunk_z: i32) -> C20ChunkData {
        let mut motion_blocking_heightmap = BitBuffer::create(9, 256);
        for x in 0..16 {
            for z in 0..16 {
                motion_blocking_heightmap.set_entry((x * 16) + z, 10);
            }
        }
        let mut section_blocks = BitBuffer::create(4, 4096);
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    section_blocks.set_entry(x + (z * 16) + (y * 256), 0);
                }
            }
        }
        for x in 0..16 {
            for z in 0..16 {
                section_blocks.set_entry(x + (z * 16), 1);
            }
        }
        let mut heightmaps = nbt::Blob::new();
        heightmaps
            .insert("MOTION_BLOCKING", motion_blocking_heightmap.into_buffer())
            .unwrap();
        let chunk_data = C20ChunkData {
            chunk_x,
            chunk_z,
            full_chunk: true,
            primary_bit_mask: 0b0000000000000010,
            heightmaps,
            biomes: Some(vec![1; 1024]),
            chunk_sections: vec![C20ChunkDataSection {
                block_count: 256,
                bits_per_block: 4,
                palette: Some(vec![0, 1]),
                data_array: section_blocks.into_buffer(),
            }],
            block_entities: vec![],
        };

        chunk_data
    }

    async fn update_entity_metadata(&self, entity_id: i32) -> Result<()> {
        let entity = self.entities.get(&entity_id).ok_or(Error::msg("Invalid "))?;
        let metadata = entity.read().await.metadata();

        self.broadcast_to(&C44EntityMetadata {
            entity_id,
            metadata
        }, self.get_players_around(entity_id).await).await;

        Ok(())
    }

    fn get_synced_entity_location(&mut self, entity: i32) -> &Location {
        if !self.synced_entities_locations.contains_key(&entity) {
            let loc = Location::default();
            self.synced_entities_locations.insert(entity, loc);
        }
        &self.synced_entities_locations[&entity]
    }

    pub async fn start_ticker(server: Arc<RwLock<Server>>) {
        tokio::task::spawn(async move {
            let expected_tps = 20.0f64;

            loop {
                let next_tps = Instant::now() + Duration::from_secs_f64(1.0 / expected_tps);
                server.write().await.tick().await;
                tokio::time::delay_until(next_tps).await;
            }
        });
    }
    pub async fn tick(&mut self) {
        /*
        Update entities visibilities
        */
        for (eid, entity) in self.entities.iter().map(|(a, b)| (*a, Arc::clone(b))) {
            let entity_location = entity.read().await.location().clone();

            for (player_eid, player) in self.get_players_ifs(|player| player.entity_id != eid).await
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
        // Remove unexisting entities that are loaded in players
        for player in self.get_players().await.values() {
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
            self.synced_entities_locations
                .insert(eid, new_location.clone());

            let on_ground = entity.read().await.on_ground();

            let players = self.get_players_around(eid).await;

            if has_rotation_changed {
                self.broadcast_to(
                    &C3AEntityHeadLook {
                        entity_id: eid,
                        head_yaw: new_location.yaw_angle(),
                    },
                    players.clone(),
                )
                .await;
            }

            if previous_location.distance2(&new_location) > 8.0 * 8.0 {
                self.broadcast_to(
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
                self.broadcast_to(
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
                self.broadcast_to(
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
                self.broadcast_to(
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
                self.broadcast_to(&C2AEntityMovement { entity_id: eid }, players)
                    .await;
            }
        }

        // TODO: Update metadata and entities propeties
    }

    pub async fn get_players(&self) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let mut players = HashMap::new();
        for (eid, entity) in self
            .entities
            .iter()
            .map(|(eid, entity)| (*eid, Arc::clone(entity)))
        {
            if entity.read().await.is_player() {
                players.insert(eid, entity);
            }
        }
        players
    }
    pub async fn get_players_ifs(
        &self,
        tester: impl Fn(&Player) -> bool,
    ) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let mut players = HashMap::new();
        for (eid, entity) in self.entities.iter() {
            let entity_read = entity.read().await;
            if entity.read().await.is_player() && tester(entity_read.as_player().unwrap()) {
                players.insert(*eid, Arc::clone(entity));
            }
        }
        players
    }
    /// Get all players in view distance of an entity
    pub async fn get_players_around(
        &self,
        entity_id: i32,
    ) -> HashMap<i32, Arc<RwLock<BoxedEntity>>> {
        let location = self
            .get_entity(entity_id)
            .await
            .read()
            .await
            .location()
            .clone();
        let view_distance2 = self.view_distance.pow(2) as f64;
        self.get_players_ifs({
            move |player: &Player| {
                player.entity_id != entity_id
                    && player.location.h_distance2(&location) <= view_distance2
            }
        })
        .await
    }

    pub async fn broadcast(&self, packet: &impl ClientBoundPacket) {
        for entity in self.get_players().await.values() {
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
    pub async fn broadcast_to(
        &self,
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

    pub async fn send_to_player(&self, player: i32, packet: &impl ClientBoundPacket) -> Result<()> {
        let player = self.entities[&player].read().await;
        let player = player.as_player()?;
        player.client.lock().await.send_packet(packet).await?;
        Ok(())
    }

    pub async fn add_entity(&mut self, entity: Arc<RwLock<BoxedEntity>>) {
        let entity_id = entity.read().await.entity_id();
        self.entities.insert(entity_id, entity);
    }
    pub async fn get_entity(&self, id: i32) -> Arc<RwLock<BoxedEntity>> {
        Arc::clone(&self.entities[&id])
    }
    pub async fn remove_entity(&mut self, id: i32) -> Arc<RwLock<BoxedEntity>> {
        self.entities.remove(&id).unwrap()
    }

    pub async fn teleport_entity(&self, id: i32, location: Location) {
        self.get_entity(id)
            .await
            .write()
            .await
            .set_location(location.clone());
        self.broadcast_to(
            &C56EntityTeleport {
                entity_id: id,
                x: location.x,
                y: location.y,
                z: location.z,
                yaw: location.yaw_angle(),
                pitch: location.pitch_angle(),
                on_ground: self.get_entity(id).await.read().await.on_ground(),
            },
            self.get_players_around(id).await,
        )
        .await;
        let is_player = self.get_entity(id).await.write().await.is_player();
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
}
