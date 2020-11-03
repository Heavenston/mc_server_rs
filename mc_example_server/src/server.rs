use crate::{commands::*, generator::Generator};
use mc_networking::{
    client::{client_event::*, Client},
    map,
    packets::{client_bound::*, server_bound::S1BPlayerDiggingStatus},
};
use mc_server_lib::{
    chat_manager::ChatManager,
    chunk_holder::ChunkHolder,
    entity::{player::Player, BoxedEntity},
    entity_manager::{EntityManager, PlayerManager, PlayerWrapper},
    entity_pool::EntityPool,
    resource_manager::ResourceManager,
};
use mc_utils::Location;

use anyhow::Result;
use log::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::{Barrier, RwLock},
    time::{sleep, Duration, Instant},
};
use uuid::Uuid;

pub static ENTITY_ID_COUNTER: AtomicI32 = AtomicI32::new(0);

pub struct Server {
    entity_pool: Arc<RwLock<EntityPool>>,
    chunk_holder: Arc<ChunkHolder<Generator>>,
    chat_manager: Arc<ChatManager>,
    #[allow(dead_code)]
    resource_manager: Arc<ResourceManager>,
    players: RwLock<PlayerManager>,
    spawn_location: RwLock<Location>,
    tps: RwLock<f64>,
    max_players: u16,
    view_distance: u16,
    brand: String,
}

impl Server {
    pub async fn new() -> Self {
        let resource_manager = Arc::new(ResourceManager::new());
        resource_manager.load_from_server_generator().await.unwrap();

        let view_distance = 10u16;
        let chunk_holder = Arc::new(ChunkHolder::new(
            Generator::new(true, resource_manager.clone()),
            view_distance as i32,
        ));

        let entity_pool = Arc::new(RwLock::new(EntityPool::new(10 * 16)));

        let chat_manager = Arc::new(ChatManager::new());
        chat_manager
            .register_command(Arc::new(GamemodeCommand))
            .await;
        chat_manager
            .register_command(Arc::new(RegenCommand {
                chunk_holder: Arc::clone(&chunk_holder),
                resource_manager: resource_manager.clone(),
            }))
            .await;
        chat_manager
            .register_command(Arc::new(TpCommand {
                entity_pool: Arc::clone(&entity_pool),
            }))
            .await;
        chat_manager
            .register_command(Arc::new(RefreshCommand {
                chunk_holder: Arc::clone(&chunk_holder),
                resource_manager: resource_manager.clone(),
            }))
            .await;
        chat_manager
            .register_command(Arc::new(SummonCommand {
                entity_pool: Arc::clone(&entity_pool),
                resource_manager: resource_manager.clone(),
            }))
            .await;
        chat_manager.register_command(Arc::new(FlyCommand)).await;
        Self {
            entity_pool,
            chunk_holder,
            chat_manager,
            resource_manager,
            players: RwLock::new(PlayerManager::new()),
            max_players: 10,
            view_distance,
            brand: "BEST SERVER EVER".to_string(),
            spawn_location: RwLock::new(Location {
                x: 0.0,
                y: 150.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
            }),
            tps: RwLock::new(20.0),
        }
    }

    pub async fn listen(server: Arc<Server>, addr: impl ToSocketAddrs) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
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
        let chat_manager = Arc::clone(&server.chat_manager);

        while let Some(event) = event_receiver.recv().await {
            match event {
                ClientEvent::ServerListPing { response } => {
                    response
                        .send(json!({
                            "version": {
                                "name": "1.16.4",
                                "protocol": 754
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
                        player_eid = ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
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
                            let player = player.as_player();

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
                        name: player.read().await.as_player().username.clone(),
                        properties: vec![],
                        gamemode: player.read().await.as_player().gamemode as i32,
                        ping: player.read().await.as_player().ping,
                        display_name: None,
                    };
                    // Send to him all players (and himself)
                    {
                        let players = {
                            let mut players = vec![my_player_info.clone()];
                            for player in server.players.read().await.entities() {
                                let player = player.read().await;
                                let player = player.as_player();
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

                    chunk_holder
                        .players
                        .write()
                        .await
                        .add_entity(Arc::clone(player))
                        .await;

                    chat_manager
                        .players
                        .write()
                        .await
                        .add_entity(Arc::clone(player))
                        .await;
                    chat_manager.declare_commands_to_player(player_eid).await;

                    let spawn_location = server.spawn_location.read().await.clone();

                    chunk_holder
                        .update_player_view_position(
                            player_eid,
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

                    // Send inventory
                    let player_inventory_slots = {
                        let mut slots = vec![];
                        let player = player.read().await;
                        let player_inventory = &player.as_player().inventory;
                        slots.push(player_inventory.crafting_output.clone());
                        slots.append(&mut player_inventory.crafting_input.clone());
                        slots.push(player_inventory.armor_head.clone());
                        slots.push(player_inventory.armor_chest.clone());
                        slots.push(player_inventory.armor_legs.clone());
                        slots.push(player_inventory.armor_feet.clone());
                        slots.append(&mut player_inventory.main_inventory.clone());
                        slots.append(&mut player_inventory.hotbar.clone());
                        slots
                    };
                    player
                        .send_packet(&C13WindowItems {
                            window_id: 0,
                            slots: player_inventory_slots,
                        })
                        .await
                        .unwrap();
                }
                ClientEvent::Logout => {
                    server.players.write().await.remove_entity(player_eid);
                    entity_pool.write().await.entities.remove_entity(player_eid);
                    entity_pool.write().await.players.remove_entity(player_eid);
                    chunk_holder.players.write().await.remove_entity(player_eid);
                    chat_manager.players.write().await.remove_entity(player_eid);
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
                    player.write().await.as_player_mut().ping = delay as i32;
                    let uuid = player.read().await.as_player().uuid.clone();
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
                    server
                        .chat_manager
                        .player_message(player.clone().into(), message)
                        .await;
                }
                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let player = player.as_ref().unwrap();
                    let last_location = player.read().await.location().clone();
                    player.write().await.set_on_ground(on_ground);
                    player.write().await.set_location(Location {
                        x,
                        y,
                        z,
                        yaw: last_location.yaw,
                        pitch: last_location.pitch,
                    });
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
                    player.write().await.set_on_ground(on_ground);
                    player.write().await.set_location(Location {
                        x,
                        y,
                        z,
                        yaw,
                        pitch,
                    });
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
                            let mut player = player.as_player_mut();
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
                ClientEvent::PlayerDigging {
                    position, status, ..
                } => {
                    let player = player.as_ref().unwrap();
                    if status == S1BPlayerDiggingStatus::StartedDigging
                        && player.read().await.as_player().gamemode == 1
                    {
                        chunk_holder
                            .set_block(position.x, position.y as u8, position.z, 0)
                            .await;
                    }
                    if status == S1BPlayerDiggingStatus::FinishedDigging {
                        chunk_holder
                            .set_block(position.x, position.y as u8, position.z, 0)
                            .await;
                    }
                }
                ClientEvent::PlayerBlockPlacement { .. } => {
                    todo!();
                }
                ClientEvent::CreativeInventoryAction { slot_id, slot } => {
                    let player = player.as_ref().unwrap();
                    if player.read().await.as_player().gamemode != 1 {
                        continue;
                    }
                    let mut player = player.write().await;
                    let inventory = &mut player.as_player_mut().inventory;
                    match slot_id {
                        0 => inventory.crafting_output = slot,
                        1..=4 => inventory.crafting_input[slot_id as usize - 1] = slot,
                        5 => inventory.armor_head = slot,
                        6 => inventory.armor_chest = slot,
                        7 => inventory.armor_legs = slot,
                        8 => inventory.armor_feet = slot,
                        9..=35 => inventory.main_inventory[slot_id as usize - 9] = slot,
                        36..=44 => inventory.hotbar[slot_id as usize - 36] = slot,
                        45 => inventory.offhand = slot,

                        _ => (),
                    }
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
                        sleep(Duration::from_secs(10)).await;
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
                        sleep(Duration::from_millis(500)).await;
                        if !*finished.read().await {
                            warn!("Tick takes more than 500ms !");
                        }
                        sleep(Duration::from_millis(9500)).await;
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
        ChunkHolder::tick(Arc::clone(&self.chunk_holder)).await;
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
