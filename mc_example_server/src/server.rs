use crate::{commands::*, generator::Generator};
use mc_networking::{
    client::{client_event::*, Client},
    data_types::Slot,
    map,
    packets::{
        client_bound::*,
        server_bound::{S1BPlayerDiggingFace, S1BPlayerDiggingStatus},
    },
};
use mc_server_lib::{
    chat_manager::ChatManager,
    chunk_holder::ChunkHolder,
    entity::{
        player::{PlayerEntity, PlayerRef},
        BoxedEntity,
    },
    entity_manager::{EntityManager, PlayerManager},
    entity_pool::EntityPool,
    resource_manager::ResourceManager,
};
use mc_utils::Location;

use anyhow::Result;
use log::*;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    },
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::RwLock,
    task::JoinHandle,
    time::{sleep, sleep_until, Duration, Instant},
};
use uuid::Uuid;

pub static ENTITY_ID_COUNTER: AtomicI32 = AtomicI32::new(0);

pub struct Server {
    entity_pool: Arc<EntityPool>,
    chunk_holder: Arc<ChunkHolder<Generator>>,
    chat_manager: Arc<ChatManager>,
    #[allow(dead_code)]
    resource_manager: Arc<ResourceManager>,
    players: RwLock<PlayerManager>,
    spawn_location: RwLock<Location>,
    tps: RwLock<f64>,
    average_tick_duration: RwLock<Duration>,
    tick_counter: AtomicI32,
    max_players: u16,
    view_distance: u16,
    brand: String,
    tick_stage: AtomicI32,
    world_folder: PathBuf,
}

impl Server {
    pub async fn new() -> Self {
        let world_folder = std::env::current_dir().unwrap().join("world");

        info!("Loading minecraft resources...");
        let resource_manager = Arc::new(ResourceManager::new());
        resource_manager.load().await.unwrap();
        info!("Loaded minecraft resources successfully");

        let view_distance = 10u16;
        let chunk_holder = Arc::new(ChunkHolder::new(
            Generator::new(true, resource_manager.clone(), world_folder.join("chunks")),
            view_distance as i32,
        ));

        let entity_pool = Arc::new(EntityPool::new(view_distance * 16, None));

        let chat_manager = Arc::new(ChatManager::new());
        chat_manager
            .register_command(Arc::new(GamemodeCommand))
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
        chat_manager
            .register_command(Arc::new(AdiosCommand {
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
            average_tick_duration: RwLock::new(Duration::from_millis(0)),
            tick_stage: AtomicI32::new(0),
            tick_counter: AtomicI32::new(0),
            world_folder,
        }
    }

    pub async fn listen(
        server: Arc<Server>,
        addr: impl ToSocketAddrs,
    ) -> Result<JoinHandle<Result<()>>> {
        let listener = TcpListener::bind(addr).await?;
        let join_handle = tokio::task::spawn(async move {
            loop {
                let (socket, ..) = listener.accept().await?;
                let (client, event_receiver) = Client::new(socket, 100, 500);
                let client = client;
                tokio::task::spawn({
                    let server = Arc::clone(&server);
                    let client = client.clone();
                    async move {
                        Server::handle_client(server, client, event_receiver)
                            .await
                            .unwrap();
                    }
                });
            }
        });
        Ok(join_handle)
    }

    async fn handle_client(
        server: Arc<Server>,
        client: Client,
        event_receiver: flume::Receiver<ClientEvent>,
    ) -> Result<()> {
        let mut player_ref: Option<PlayerRef> = None;
        let mut player_eid = -1i32;
        let entity_pool = Arc::clone(&server.entity_pool);
        let chunk_holder = Arc::clone(&server.chunk_holder);
        let chat_manager = Arc::clone(&server.chat_manager);
        let resource_manager = Arc::clone(&server.resource_manager);
        let players_folder = server.world_folder.join("players");
        std::fs::create_dir_all(&players_folder).unwrap();

        while let Ok(event) = event_receiver.recv_async().await {
            match event {
                ClientEvent::ServerListPing { response } => {
                    response
                        .send(json!({
                            "version": {
                                "name": resource_manager.get_minecraft_version().await,
                                "protocol": resource_manager.get_protocol_version().await
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
                        let entity = Arc::new(RwLock::new(BoxedEntity::new(PlayerEntity::new(
                            username.clone(),
                            player_eid,
                            uuid,
                            client.clone(),
                        ))));
                        player_ref = Some(PlayerRef::new(entity).await.unwrap());

                        info!(
                            "{} joined the game, EID: {}, UUID: {}",
                            username.clone(),
                            player_eid,
                            uuid.clone()
                        );

                        response
                            .send(LoginStartResult::Accept {
                                uuid,
                                username,
                                encrypt: true,
                                compress: true,
                            })
                            .unwrap();
                    }
                }
                ClientEvent::LoggedIn => {
                    let player_ref = player_ref.as_ref().unwrap();

                    // Join Game
                    client
                        .send_packet(&{
                            let player_entity = player_ref.entity.read().await;
                            let player_entity = player_entity.as_player();

                            C24JoinGame {
                                entity_id: player_entity.entity_id,
                                is_hardcore: false,
                                gamemode: player_entity.gamemode,
                                previous_gamemode: player_entity.gamemode,
                                world_names: vec!["heav:world".into()],
                                dimension_codec: C24JoinGameDimensionCodec {
                                    dimensions: map! {
                                        "heav:world".into() => C24JoinGameDimensionElement {
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
                                        "minecraft:plains".into() => C24JoinGameBiomeElement {
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
                                        "heav:plot".into() => C24JoinGameBiomeElement {
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
                        uuid: *player_ref.entity.read().await.uuid(),
                        name: player_ref.entity.read().await.as_player().username.clone(),
                        properties: vec![],
                        gamemode: player_ref.entity.read().await.as_player().gamemode as i32,
                        ping: player_ref.entity.read().await.as_player().ping,
                        display_name: None,
                    };
                    // Send to him all players (and himself)
                    {
                        let players = {
                            let mut players = vec![my_player_info.clone()];
                            for other_player_ref in server.players.read().await.entities() {
                                let other_player_entity = other_player_ref.entity.read().await;
                                let other_player_entity = other_player_entity.as_player();
                                players.push(C32PlayerInfoPlayerUpdate::AddPlayer {
                                    uuid: other_player_entity.uuid,
                                    name: other_player_entity.username.clone(),
                                    properties: vec![],
                                    gamemode: 1,
                                    ping: other_player_entity.ping,
                                    display_name: None,
                                });
                            }
                            players
                        };
                        client
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

                    player_ref.update_abilities().await.unwrap();
                    server
                        .players
                        .write()
                        .await
                        .add_entity(player_ref.clone())
                        .await;

                    entity_pool
                        .entities
                        .write()
                        .await
                        .add_entity(player_ref.clone())
                        .await;
                    entity_pool
                        .players
                        .write()
                        .await
                        .add_entity(player_ref.clone())
                        .await;

                    chunk_holder.add_player(player_ref.clone()).await;

                    chat_manager
                        .players
                        .write()
                        .await
                        .add_entity(player_ref.clone())
                        .await;
                    chat_manager.declare_commands_to_player(player_eid).await;

                    let spawn_location = server.spawn_location.read().await.clone();

                    chunk_holder
                        .update_player_view_position(
                            player_eid,
                            spawn_location.chunk_x(),
                            spawn_location.chunk_z(),
                            false,
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
                                    C17PluginMessageBuilder::new("minecraft:brand".into());
                                builder.encoder.write_string(&server.brand);
                                builder.build()
                            })
                            .await
                            .unwrap();
                    }

                    // Update position
                    entity_pool
                        .teleport_entity(&mut *player_ref.entity.write().await, spawn_location)
                        .await;

                    // Send inventory
                    let player_inventory_slots = {
                        let mut slots = vec![];
                        let player = player_ref.entity.read().await;
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
                    player_ref
                        .send_packet(&C13WindowItems {
                            window_id: 0,
                            slots: player_inventory_slots,
                        })
                        .await
                        .unwrap();
                }
                ClientEvent::Logout => {
                    server.players.write().await.remove_entity(player_eid);
                    entity_pool.entities.write().await.remove_entity(player_eid);
                    entity_pool.players.write().await.remove_entity(player_eid);
                    chunk_holder.remove_player(player_eid).await;
                    chat_manager.players.write().await.remove_entity(player_eid);
                    let uuid = player_ref
                        .as_ref()
                        .unwrap()
                        .entity
                        .read()
                        .await
                        .uuid()
                        .clone();
                    let username = player_ref
                        .as_ref()
                        .unwrap()
                        .entity
                        .read()
                        .await
                        .as_player()
                        .username
                        .clone();
                    server
                        .players
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::RemovePlayer { uuid }],
                        })
                        .await
                        .unwrap();
                    info!("{} left the game", username);
                    break;
                }

                ClientEvent::Ping { delay } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    player_ref.entity.write().await.as_player_mut().ping = delay as i32;
                    let uuid = player_ref.entity.read().await.as_player().uuid;
                    server
                        .players
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::UpdateLatency {
                                uuid,
                                ping: delay as i32,
                            }],
                        })
                        .await
                        .unwrap();
                }

                ClientEvent::ChatMessage { message } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    server
                        .chat_manager
                        .player_message(player_ref.clone(), message)
                        .await;
                }
                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    let last_location = player_ref.entity.read().await.location().clone();
                    player_ref.entity.write().await.set_on_ground(on_ground);
                    player_ref.entity.write().await.set_location(Location {
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
                    let player_ref = player_ref.as_ref().unwrap();
                    player_ref.entity.write().await.set_on_ground(on_ground);
                    player_ref.entity.write().await.set_location(Location {
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
                    let mut player_entity = player_ref.as_ref().unwrap().entity.write().await;
                    player_entity.set_on_ground(on_ground);
                    let location = player_entity.location().clone();
                    player_entity.set_location(Location {
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
                        let mut player_write = player_ref.as_ref().unwrap().entity.write().await;
                        let mut player = player_write.as_player_mut();
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
                        entity_pool
                            .sync_entity_metadata(&*player_write)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerAbilities { is_flying } => {
                    let mut player_ref = player_ref.as_ref().unwrap().entity.write().await;

                    player_ref.as_player_mut().is_flying = is_flying;
                    entity_pool
                        .sync_entity_metadata(&*player_ref)
                        .await
                        .unwrap();
                }
                ClientEvent::Animation { hand } => {
                    let servers = server
                        .players
                        .read()
                        .await
                        .get_players_except(player_eid)
                        .await;
                    EntityManager::broadcast_to(
                        &C05EntityAnimation {
                            entity_id: player_eid,
                            animation: if hand == 0 { 0 } else { 3 },
                        },
                        &servers,
                    )
                    .await;
                }
                ClientEvent::PlayerDigging {
                    position,
                    status,
                    face: _,
                } => {
                    if status == S1BPlayerDiggingStatus::StartedDigging
                        || status == S1BPlayerDiggingStatus::FinishedDigging
                        || status == S1BPlayerDiggingStatus::CancelledDigging
                    {
                        let mut successful = true;

                        let player_ref = player_ref.as_ref().unwrap();
                        if status == S1BPlayerDiggingStatus::StartedDigging {
                            if player_ref.entity.read().await.as_player().gamemode == 1 {
                                chunk_holder
                                    .set_block(position.x, position.y as u8, position.z, 0)
                                    .await;
                            }
                            else {
                                successful = false;
                            }
                        }

                        if status == S1BPlayerDiggingStatus::FinishedDigging {
                            successful = false;
                        }
                        let block = chunk_holder
                            .get_block(position.x, position.y as u8, position.z)
                            .await;
                        player_ref
                            .send_packet(&C07AcknowledgePlayerDigging {
                                position: position.clone(),
                                block: block as i32,
                                status: S1BPlayerDiggingStatus::CancelledDigging,
                                successful,
                            })
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerBlockPlacement {
                    hand,
                    position,
                    face,
                    cursor_position_x: _,
                    cursor_position_y: _,
                    cursor_position_z: _,
                    inside_block: _,
                } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    let mut player_entity = player_ref.entity.write().await;
                    let equipment = player_entity.get_equipment_mut();
                    let slot = match hand {
                        0 => equipment.main_hand, // Main hand
                        _ => equipment.off_hand,  // Off hand (2)
                    };
                    if let Slot::Present {
                        item_id,
                        item_count: _,
                        nbt: _,
                    } = slot
                    {
                        let item_name = resource_manager
                            .get_registry_value_name("minecraft:item", Some(*item_id))
                            .await
                            .unwrap();
                        let block_id = resource_manager.get_block_state_id(&item_name, None).await;
                        if let Ok(block_id) = block_id {
                            let mut new_block_pos = position.clone();
                            match face {
                                S1BPlayerDiggingFace::Top => {
                                    new_block_pos.y += 1;
                                }
                                S1BPlayerDiggingFace::Bottom => {
                                    new_block_pos.y -= 1;
                                }
                                S1BPlayerDiggingFace::North => {
                                    new_block_pos.z -= 1;
                                }
                                S1BPlayerDiggingFace::East => {
                                    new_block_pos.x += 1;
                                }
                                S1BPlayerDiggingFace::South => {
                                    new_block_pos.z += 1;
                                }
                                S1BPlayerDiggingFace::West => {
                                    new_block_pos.x -= 1;
                                }
                            }
                            chunk_holder
                                .set_block(
                                    new_block_pos.x,
                                    new_block_pos.y as u8,
                                    new_block_pos.z,
                                    block_id as u16,
                                )
                                .await;
                        }
                    }
                }
                ClientEvent::CreativeInventoryAction { slot_id, slot } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    if player_ref.entity.read().await.as_player().gamemode != 1 {
                        continue;
                    }
                    let mut player = player_ref.entity.write().await;
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
                ClientEvent::HeldItemChange { slot } => {
                    let player_ref = player_ref.as_ref().unwrap();
                    player_ref.entity.write().await.as_player_mut().held_item = slot as u8;
                }
                ClientEvent::ClickWindow {
                    window_id: _,
                    slot_id: _,
                    button: _,
                    action_number: _,
                    mode: _,
                    clicked_item: _,
                } => todo!(),
                ClientEvent::PluginMessage { channel, data } => {
                    debug!(
                        "Plugin Message, Channel: {}, Data: {}",
                        channel,
                        String::from_utf8_lossy(&data)
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn start_ticker(server: Arc<Server>) {
        tokio::task::spawn(async move {
            let target_tps = 20.0;
            let tps_delay = Duration::from_secs_f64(1.0 / target_tps);
            let mut interval = tokio::time::interval(tps_delay);
            let ticks = Arc::new(RwLock::new(0i32));
            let times = Arc::new(RwLock::new(Duration::from_secs(0)));
            // TPS Calculator
            tokio::task::spawn({
                let ticks = Arc::clone(&ticks);
                let server = Arc::clone(&server);
                let times = Arc::clone(&times);
                async move {
                    let monitor_time = Duration::from_secs_f64(2.5);
                    loop {
                        sleep(monitor_time).await;
                        let n = (*ticks.read().await as f64) / monitor_time.as_secs_f64();
                        *server.tps.write().await = n;
                        let average_time = times
                            .read()
                            .await
                            .div_f64((*ticks.read().await as f64).max(1.0));
                        *ticks.write().await = 0;
                        debug!("{} TPS (~{}ms)", n, average_time.as_millis());
                        *server.average_tick_duration.write().await = average_time;
                        *times.write().await = Duration::from_secs(0);
                    }
                }
            });

            let (ticks_send, ticks_receive) = flume::bounded(10);
            // Infinite tick check
            tokio::task::spawn({
                let server = Arc::clone(&server);
                async move {
                    loop {
                        let finished: Arc<AtomicBool> = ticks_receive.recv_async().await.unwrap();
                        let start = Instant::now();
                        let server = Arc::clone(&server);
                        tokio::task::spawn(async move {
                            sleep_until(start + Duration::from_millis(500)).await;
                            if !finished.load(Ordering::SeqCst) {
                                warn!("Tick takes more than 500ms !");
                            }
                            sleep_until(start + Duration::from_millis(9500)).await;
                            if !finished.load(Ordering::SeqCst) {
                                error!(
                                    "A tick take more than 10s at stage {}, closing server",
                                    server.tick_stage.load(Ordering::SeqCst)
                                );
                                std::process::exit(0);
                            }
                        });
                    }
                }
            });

            loop {
                interval.tick().await;

                let finished = Arc::new(AtomicBool::new(false));
                ticks_send.try_send(finished.clone()).unwrap();
                let start = Instant::now();
                server.tick().await;
                let elapsed = start.elapsed();
                if elapsed > tps_delay.mul_f64(2.0) {
                    debug!("Tick took {}ms", elapsed.as_millis());
                }
                *times.write().await += elapsed;
                *ticks.write().await += 1;
                finished.store(true, Ordering::SeqCst);
            }
        });
    }

    pub async fn tick(&self) {
        let tick_counter = self.tick_counter.fetch_add(1, Ordering::Relaxed);

        self.tick_stage.store(1, Ordering::SeqCst);
        self.entity_pool.tick(tick_counter).await;
        self.tick_stage.store(2, Ordering::SeqCst);
        ChunkHolder::tick(Arc::clone(&self.chunk_holder)).await;
        self.tick_stage.store(3, Ordering::SeqCst);
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
                    }, {
                        "text": format!("\n{:?}", self.average_tick_duration.read().await),
                        "color": "green"
                    }, {
                        "text": format!("\n{}", self.entity_pool.entities.read().await.size()),
                        "color": "green"
                    }, {
                        "text": " entities",
                        "color": "white"
                    } ]
                }),
            })
            .await
            .unwrap();
        self.tick_stage.store(0, Ordering::SeqCst);
    }
}
