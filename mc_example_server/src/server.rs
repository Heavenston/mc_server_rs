use mc_networking::{
    client::{client_event::*, Client},
    map,
    packets::client_bound::*,
};
use mc_server_lib::{
    chunk_holder::{ChunkGenerator, ChunkHolder},
    entity::{player::Player, BoxedEntity},
    entity_manager::{PlayerManager, PlayerWrapper},
    entity_pool::EntityPool,
};
use mc_utils::{ChunkData, Location};

use anyhow::Result;
use async_trait::async_trait;
use log::*;
use mc_server_lib::entity_manager::EntityManager;
use noise::{NoiseFn, Perlin};
use serde_json::json;
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::{Barrier, RwLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Generator {
    noise: Perlin,
    noise_scale: f64,
}
impl Generator {
    pub fn new() -> Self {
        Self {
            noise: Perlin::new(),
            noise_scale: 1.0 / 15.0,
        }
    }
}
#[async_trait]
impl ChunkGenerator for Generator {
    async fn generate_chunk_data(&self, chunk_x: i32, chunk_z: i32) -> Box<ChunkData> {
        let mut data = Box::new(ChunkData::new());
        for local_x in 0..16 {
            let global_x = chunk_x * 16 + local_x;
            let noise_x = global_x as f64 * self.noise_scale;
            for local_z in 0..16 {
                let global_z = chunk_z * 16 + local_z;
                let noise_z = global_z as f64 * self.noise_scale;
                let height = (100.0 + (self.noise.get([noise_x, noise_z]) * 10.0 - 5.0)) as u8;
                for y in 0..(height - 5) {
                    let block = if self
                        .noise
                        .get([noise_x, y as f64 * self.noise_scale, noise_z])
                        > 0.5
                    {
                        1
                    }
                    else {
                        3
                    };
                    data.set_block(local_x as u8, y, local_z as u8, block);
                }
                for y in (height - 5)..height {
                    data.set_block(local_x as u8, y, local_z as u8, 10);
                }
                data.set_block(local_x as u8, height, local_z as u8, 9);
            }
        }
        data
    }
}

pub struct Server {
    entity_pool: Arc<RwLock<EntityPool>>,
    chunk_holder: Arc<ChunkHolder<Generator>>,
    players: RwLock<PlayerManager>,
    entity_id_counter: AtomicI32,
    spawn_location: RwLock<Location>,
    tps: RwLock<f64>,
    max_players: u16,
    view_distance: u16,
    brand: String,
}

impl Server {
    pub fn new() -> Self {
        Self {
            entity_pool: Arc::new(RwLock::new(EntityPool::new(10 * 16))),
            chunk_holder: Arc::new(ChunkHolder::new(Generator::new())),
            players: RwLock::new(PlayerManager::new()),
            entity_id_counter: AtomicI32::new(0),
            max_players: 10,
            view_distance: 10,
            brand: "BEST SERVER EVER".to_string(),
            spawn_location: RwLock::new(Location {
                x: 0.0,
                y: 101.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
            }),
            tps: RwLock::new(20.0),
        }
    }

    pub async fn listen(server: Arc<Server>, addr: impl ToSocketAddrs) -> Result<()> {
        let mut listener = TcpListener::bind(addr).await?;
        loop {
            let (socket, ..) = listener.accept().await?;
            let (client, event_receiver) = Client::new(socket);
            let client = Arc::new(RwLock::new(client));

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
        server: Arc<Server>,
        client: Arc<RwLock<Client>>,
        mut event_receiver: tokio::sync::mpsc::Receiver<ClientEvent>,
    ) -> Result<()> {
        let mut player: Option<PlayerWrapper> = None;
        let mut player_eid = -1i32;
        let entity_pool = Arc::clone(&server.entity_pool);
        let chunk_holder = Arc::clone(&server.chunk_holder);

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
                                "max": server.max_players,
                                "online": server.players.read().await.size(),
                                "sample": []
                            },
                            "description": "Hi"
                        }))
                        .unwrap();
                }
                ClientEvent::LoginStart { response, username } => {
                    if (server.max_players as usize) <= server.players.read().await.size() {
                        response
                            .send(LoginStartResult::Disconnect {
                                reason: "The server is full :(".to_string(),
                            })
                            .unwrap();
                    }
                    else {
                        player_eid = server.entity_id_counter.fetch_add(1, Ordering::Relaxed);
                        let uuid = Uuid::new_v3(
                            &Uuid::new_v4(),
                            format!("OfflinePlayer:{}", username).as_bytes(),
                        );
                        let entity = Arc::new(RwLock::new(BoxedEntity::new(Player::new(
                            username.clone(),
                            player_eid,
                            uuid.clone(),
                            Arc::clone(&client),
                        ))));
                        player = Some(entity.into());

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
                        .read()
                        .await
                        .send_packet(&{
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
                        let players = {
                            let mut players = vec![my_player_info.clone()];
                            for player in server.players.read().await.entities() {
                                let player = player.read().await;
                                let player = player.as_player().unwrap();
                                players.push(C32PlayerInfoPlayerUpdate::AddPlayer {
                                    uuid: player.uuid.clone(),
                                    name: player.username.clone(),
                                    properties: vec![],
                                    gamemode: 1,
                                    ping: player.ping,
                                    display_name: None,
                                });
                            }
                            players
                        };
                        client
                            .read()
                            .await
                            .send_packet(&C32PlayerInfo { players })
                            .await
                            .unwrap();
                    }
                    // Send to all his player info
                    server
                        .players
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![my_player_info],
                        })
                        .await
                        .unwrap();

                    player.update_abilities().await.unwrap();
                    server
                        .players
                        .write()
                        .await
                        .add_entity(Arc::clone(&player))
                        .await;

                    entity_pool
                        .write()
                        .await
                        .entities
                        .add_entity(Arc::clone(player))
                        .await;
                    entity_pool
                        .write()
                        .await
                        .players
                        .add_entity(Arc::clone(player))
                        .await;

                    let spawn_location = server.spawn_location.read().await.clone();

                    chunk_holder
                        .update_player_view_position(
                            server.view_distance as i32,
                            player.clone(),
                            spawn_location.chunk_x(),
                            spawn_location.chunk_z(),
                        )
                        .await;

                    // Send server brand
                    {
                        server
                            .players
                            .read()
                            .await
                            .send_to_player(player_eid, &{
                                let mut builder =
                                    C17PluginMessageBuilder::new("minecraft:brand".to_string());
                                builder.encoder.write_string(&server.brand);
                                builder.build()
                            })
                            .await
                            .unwrap();
                    }

                    // Update position
                    entity_pool
                        .read()
                        .await
                        .teleport_entity(player_eid, spawn_location)
                        .await;
                }
                ClientEvent::Logout => {
                    server.players.write().await.remove_entity(player_eid);
                    entity_pool.write().await.entities.remove_entity(player_eid);
                    entity_pool.write().await.players.remove_entity(player_eid);
                    let uuid = player.unwrap().read().await.uuid().clone();
                    server
                        .players
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::RemovePlayer { uuid }],
                        })
                        .await
                        .unwrap();
                    break;
                }

                ClientEvent::Ping { delay } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.as_player_mut().unwrap().ping = delay as i32;
                    let uuid = player.read().await.as_player().unwrap().uuid.clone();
                    server
                        .players
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::UpdateLatency {
                                uuid: uuid.clone(),
                                ping: delay as i32,
                            }],
                        })
                        .await
                        .unwrap();
                }

                ClientEvent::ChatMessage { message } => {
                    let player = player.as_ref().unwrap();
                    if message == "gm" {
                        let current_gamemode = player.read().await.as_player().unwrap().gamemode;
                        player
                            .set_gamemode(if current_gamemode == 1 { 0 } else { 1 })
                            .await;
                    }
                    else {
                        server
                            .players
                            .read()
                            .await
                            .broadcast(&C0EChatMessage {
                                json_data: json!({
                                    "text":
                                        format!(
                                            "<{}> {}",
                                            player.read().await.as_player().unwrap().username,
                                            message
                                        )
                                }),
                                position: 0,
                                sender: Some(player.read().await.uuid().clone()),
                            })
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let player = player.as_ref().unwrap();
                    let last_location = player.read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw: last_location.yaw,
                        pitch: last_location.pitch,
                    };
                    player.write().await.set_on_ground(on_ground);
                    player.write().await.set_location(new_location.clone());
                    if new_location.chunk_x() != last_location.chunk_x()
                        || new_location.chunk_z() != last_location.chunk_z()
                    {
                        let view_distance = server.view_distance;
                        let chunk_holder = Arc::clone(&chunk_holder);
                        let player = player.clone();
                        tokio::task::spawn(async move {
                            chunk_holder
                                .update_player_view_position(
                                    view_distance as i32,
                                    player,
                                    new_location.chunk_x(),
                                    new_location.chunk_z(),
                                )
                                .await;
                        });
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
                    let player = player.as_ref().unwrap();
                    let last_location = player.read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw,
                        pitch,
                    };
                    player.write().await.set_on_ground(on_ground);
                    player.write().await.set_location(new_location.clone());
                    if new_location.chunk_x() != last_location.chunk_x()
                        || new_location.chunk_z() != last_location.chunk_z()
                    {
                        let view_distance = server.view_distance;
                        let chunk_holder = Arc::clone(&chunk_holder);
                        let player = player.clone();
                        tokio::task::spawn(async move {
                            chunk_holder
                                .update_player_view_position(
                                    view_distance as i32,
                                    player,
                                    new_location.chunk_x(),
                                    new_location.chunk_z(),
                                )
                                .await;
                        });
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
                        entity_pool
                            .read()
                            .await
                            .update_entity_metadata(player_eid)
                            .await
                            .unwrap();
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
                    entity_pool
                        .read()
                        .await
                        .update_entity_metadata(player_eid)
                        .await
                        .unwrap();
                }
                ClientEvent::Animation { hand } => {
                    let servers = server
                        .players
                        .read()
                        .await
                        .get_filtered_players(|p| p.entity_id != player_eid)
                        .await;
                    EntityManager::broadcast_to(
                        &C05EntityAnimation {
                            entity_id: player_eid,
                            animation: if hand == 0 { 0 } else { 3 },
                        },
                        servers,
                    )
                    .await;
                }
            }
        }

        Ok(())
    }

    pub async fn start_ticker(server: Arc<Server>) {
        tokio::task::spawn(async move {
            let mut tps_interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 20.0));
            let ticks = Arc::new(RwLock::new(0i32));
            let times = Arc::new(RwLock::new(0u128));
            // TPS Calculator
            tokio::task::spawn({
                let ticks = Arc::clone(&ticks);
                let server = Arc::clone(&server);
                let times = Arc::clone(&times);
                async move {
                    loop {
                        tokio::time::delay_for(Duration::from_secs(10)).await;
                        let n = (*ticks.read().await as f64) / 10f64;
                        *ticks.write().await = 0;
                        *server.tps.write().await = n;
                        info!("{} TPS (~{}ms)", n, *times.read().await / 10);
                        *times.write().await = 0;
                    }
                }
            });

            let barrier = Arc::new(Barrier::new(2));
            let finished = Arc::new(RwLock::new(false));
            // Infinite tick check
            tokio::task::spawn({
                let barrier = Arc::clone(&barrier);
                let finished = Arc::clone(&finished);
                async move {
                    loop {
                        barrier.wait().await;
                        tokio::time::delay_for(Duration::from_millis(500)).await;
                        if !*finished.read().await {
                            warn!("Tick takes more than 500ms !");
                        }
                        tokio::time::delay_for(Duration::from_millis(9500)).await;
                        if !*finished.read().await {
                            warn!("A tick take more than 10s, closing server");
                            std::process::exit(0);
                        }
                        *finished.write().await = true;
                    }
                }
            });

            loop {
                tps_interval.tick().await;

                let start = Instant::now();
                server.tick().await;
                let elapsed = start.elapsed().as_millis();
                if elapsed > 100 {
                    debug!("Tick took {}ms", elapsed);
                }
                *times.write().await += elapsed;
                *ticks.write().await += 1;
                *finished.write().await = true;
            }
        });
    }

    pub async fn tick(&self) {
        self.entity_pool.write().await.tick().await;
        self.chunk_holder.tick().await;
        self.players
            .write()
            .await
            .broadcast(&C53PlayerListHeaderAndFooter {
                header: json!({
                    "text": "\nHeavenstone\n",
                    "color": "blue"
                }),
                footer: json!({
                    "text": "TPS: ",
                    "color": "white",
                    "extra": [ {
                        "text": format!("{}", *self.tps.read().await),
                        "color": "green"
                    } ]
                }),
            })
            .await
            .unwrap();
    }
}
