pub mod client_event;

use crate::packets::{client_bound::*, server_bound::*, PacketCompression, RawPacket};
use client_event::*;

use crate::data_types::Angle;
use anyhow::{Error, Result};
use log::*;
use serde_json::json;
use std::{convert::TryInto, net::Shutdown, sync::Arc};
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    prelude::io::AsyncWriteExt,
    sync::{mpsc, oneshot, Mutex, RwLock},
    time::{sleep, Duration, Instant},
};

const KEEP_ALIVE_TIMEOUT: u64 = 30_000;
const KEEP_ALIVE_INTERVAL: u64 = 15_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientState {
    Handshaking,
    Status,
    Login,
    Play,
    Disconnected,
}

pub struct Client {
    compression: Arc<RwLock<PacketCompression>>,
    write: Arc<Mutex<OwnedWriteHalf>>,
    state: Arc<RwLock<ClientState>>,
    #[allow(dead_code)]
    event_sender: mpsc::Sender<ClientEvent>,
}

impl Client {
    pub fn new(socket: TcpStream) -> (Self, mpsc::Receiver<ClientEvent>) {
        let (read, write) = socket.into_split();
        let write = Arc::new(Mutex::new(write));
        let state = Arc::new(RwLock::new(ClientState::Handshaking));
        let (event_sender, event_receiver) = mpsc::channel(100);
        let compression = Arc::default();

        tokio::spawn({
            let write = Arc::clone(&write);
            let state = Arc::clone(&state);
            let listener_sender = event_sender.clone();
            let compression = Arc::clone(&compression);
            async move {
                if let Err(e) = listen_client_packets(
                    compression,
                    read,
                    Arc::clone(&write),
                    listener_sender.clone(),
                    Arc::clone(&state),
                )
                .await
                {
                    if let Some(e) = e.downcast_ref::<std::io::Error>() {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof
                            && *state.read().await == ClientState::Play
                        {
                            *state.write().await = ClientState::Disconnected;
                            listener_sender.try_send(ClientEvent::Logout).unwrap();
                        }
                        else {
                            error!(
                                "Unexpected error while handling {:?}, {:#?}",
                                write.lock().await.as_ref().peer_addr().unwrap(),
                                e
                            );
                        }
                    }
                    else {
                        error!(
                            "Unexpected error while handling {:?}, {:#?}",
                            write.lock().await.as_ref().peer_addr().unwrap(),
                            e
                        );
                    }
                };
            }
        });

        (
            Client {
                compression,
                write,
                state,
                event_sender,
            },
            event_receiver,
        )
    }

    pub async fn get_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

    pub async fn send_raw_packet(&self, packet: &RawPacket) -> Result<()> {
        self.write
            .lock()
            .await
            .write_all(&packet.encode(*self.compression.read().await))
            .await?;
        Ok(())
    }
    pub async fn send_packet<U: ClientBoundPacket>(&self, packet: &U) -> Result<()> {
        let raw_packet = packet.to_rawpacket();
        self.write
            .lock()
            .await
            .write_all(&raw_packet.encode(*self.compression.read().await))
            .await?;
        Ok(())
    }

    pub async fn hold_item_change(&self, slot: i8) -> Result<()> {
        self.send_packet(&C3FHoldItemChange { slot }).await?;
        Ok(())
    }

    pub async fn update_view_position(&self, chunk_x: i32, chunk_z: i32) -> Result<()> {
        self.send_packet(&C40UpdateViewPosition { chunk_x, chunk_z })
            .await?;
        Ok(())
    }

    pub async fn send_player_abilities(
        &self,
        invulnerable: bool,
        flying: bool,
        allow_flying: bool,
        creative_mode: bool,
        flying_speed: f32,
        fov_modifier: f32,
    ) -> Result<()> {
        self.send_packet(&C30PlayerAbilities {
            flags: (invulnerable as u8) * 0x01
                | (flying as u8) * 0x02
                | (allow_flying as u8) * 0x04
                | (creative_mode as u8) * 0x08,
            flying_speed,
            fov_modifier,
        })
        .await?;
        Ok(())
    }

    pub async fn destroy_entities(&self, entities: Vec<i32>) -> Result<()> {
        self.send_packet(&C36DestroyEntities { entities }).await?;
        Ok(())
    }

    pub async fn send_entity_head_look(&self, entity_id: i32, head_yaw: Angle) -> Result<()> {
        self.send_packet(&C3AEntityHeadLook {
            entity_id,
            head_yaw,
        })
        .await?;
        Ok(())
    }

    pub async fn unload_chunk(&self, chunk_x: i32, chunk_z: i32) -> Result<()> {
        self.send_packet(&C1CUnloadChunk { chunk_x, chunk_z })
            .await?;
        Ok(())
    }
}

struct KeepAliveData {
    pub compression: Arc<RwLock<PacketCompression>>,
    pub has_responded: bool,
    pub last_id: i64,
    pub sent_at: Instant,
}

async fn handle_keep_alive(
    write: Arc<Mutex<OwnedWriteHalf>>,
    state: Arc<RwLock<ClientState>>,
    data: Arc<RwLock<KeepAliveData>>,
) {
    let start = Instant::now();
    loop {
        if *state.read().await == ClientState::Disconnected {
            break;
        }
        {
            let id = Instant::now().duration_since(start).as_millis() as i64;
            data.write().await.last_id = id;
            data.write().await.has_responded = false;
            data.write().await.sent_at = Instant::now();
            let packet_compression = *data.read().await.compression.read().await;
            write
                .lock()
                .await
                .write_all(
                    C1FKeepAlive { id }
                        .to_rawpacket()
                        .encode(packet_compression)
                        .as_ref(),
                )
                .await
                .unwrap();
            debug!("Sent keep alive");
        }
        if *state.read().await == ClientState::Disconnected {
            break;
        }
        loop {
            sleep(Duration::from_millis(1_000)).await;
            if *state.read().await == ClientState::Disconnected {
                break;
            }
            let packet_compression = *data.read().await.compression.read().await;
            if !data.read().await.has_responded {
                if data.read().await.sent_at.elapsed().as_millis() >= (KEEP_ALIVE_TIMEOUT as u128) {
                    debug!("Keep alive timeout, closing");
                    // TODO: Send disconnect packet
                    write
                        .lock()
                        .await
                        .as_ref()
                        .shutdown(Shutdown::Both)
                        .unwrap();
                    *state.write().await = ClientState::Disconnected;
                }
                else {
                    debug!("Keep alive miss, sending it again");
                    write
                        .lock()
                        .await
                        .write_all(
                            C1FKeepAlive {
                                id: data.read().await.last_id,
                            }
                            .to_rawpacket()
                            .encode(packet_compression)
                            .as_ref(),
                        )
                        .await
                        .unwrap();
                }
            }
            else {
                break;
            }
        }
        sleep(Duration::from_millis(KEEP_ALIVE_INTERVAL)).await;
    }
}

async fn listen_client_packets(
    compression: Arc<RwLock<PacketCompression>>,
    mut read: OwnedReadHalf,
    write: Arc<Mutex<OwnedWriteHalf>>,
    event_sender: mpsc::Sender<ClientEvent>,
    state: Arc<RwLock<ClientState>>,
) -> Result<()> {
    let keep_alive_data = Arc::new(RwLock::new(KeepAliveData {
        compression: Arc::clone(&compression),
        has_responded: false,
        sent_at: Instant::now(),
        last_id: 0,
    }));
    let mut keep_alive_task = None;

    loop {
        if let ClientState::Disconnected = state.read().await.clone() {
            break;
        }

        //debug!("Reading packet, State({:?})", state.read().await.clone());
        let packet_compression = *compression.read().await;
        let raw_packet = RawPacket::decode_async(&mut read, packet_compression).await?;
        /*debug!(
            "Received packet 0x{:x} with data of length {}",
            raw_packet.packet_id,
            raw_packet.data.len()
        );*/

        let current_state = state.read().await.clone();
        match current_state {
            ClientState::Handshaking => {
                let handshake = S00Handshake::decode(raw_packet)?;
                debug!("Received Handshake: {:?}", handshake);
                *(state.write().await) = match handshake.next_state {
                    1 => ClientState::Status,
                    2 => ClientState::Login,
                    _ => return Err(Error::msg("Invalid handshake packet")),
                };
                debug!("New state: {:?}", state.read().await.clone());
            }

            ClientState::Status => {
                if raw_packet.packet_id == S00Request::packet_id() {
                    S00Request::decode(raw_packet)?;
                    let event_response = {
                        let (response_sender, response_receiver) = oneshot::channel();
                        event_sender.try_send(ClientEvent::ServerListPing {
                            response: response_sender,
                        })?;
                        response_receiver.await?
                    };
                    let response = C00Response {
                        json_response: event_response,
                    }
                    .to_rawpacket();
                    write
                        .lock()
                        .await
                        .write_all(response.encode(*compression.read().await).as_ref())
                        .await?;
                }
                else if raw_packet.packet_id == S01Ping::packet_id() {
                    let packet: S01Ping = raw_packet.try_into()?;
                    let pong = C01Pong {
                        payload: packet.payload,
                    }
                    .to_rawpacket();
                    write
                        .lock()
                        .await
                        .write_all(pong.encode(*compression.read().await).as_ref())
                        .await?;
                    read.as_ref().shutdown(Shutdown::Both)?;
                    *(state.write().await) = ClientState::Disconnected;
                    break;
                }
                else {
                    return Err(Error::msg("Invalid packet_id"));
                }
            }

            ClientState::Login => {
                if raw_packet.packet_id == S00LoginStart::packet_id() {
                    let login_state = S00LoginStart::decode(raw_packet)?;

                    let new_compression = 20;
                    write
                        .lock()
                        .await
                        .write_all(
                            C03SetCompression {
                                threshold: new_compression,
                            }
                            .to_rawpacket()
                            .encode(PacketCompression::default())
                            .as_ref(),
                        )
                        .await?;
                    *compression.write().await = PacketCompression::new(new_compression);

                    let event_response = {
                        let (response_sender, response_receiver) = oneshot::channel();
                        event_sender.try_send(ClientEvent::LoginStart {
                            username: login_state.name.clone(),
                            response: response_sender,
                        })?;
                        response_receiver.await?
                    };
                    match event_response {
                        LoginStartResult::Accept { uuid, username } => {
                            let login_success = C02LoginSuccess { uuid, username }.to_rawpacket();
                            write
                                .lock()
                                .await
                                .write_all(login_success.encode(*compression.read().await).as_ref())
                                .await?;
                            *(state.write().await) = ClientState::Play;
                        }
                        LoginStartResult::Disconnect { reason } => {
                            let disconnect = C00LoginDisconnect {
                                reason: json!({ "text": reason }),
                            }
                            .to_rawpacket();
                            write
                                .lock()
                                .await
                                .write_all(disconnect.encode(*compression.read().await).as_ref())
                                .await?;
                            read.as_ref().shutdown(Shutdown::Both)?;
                            *(state.write().await) = ClientState::Disconnected;
                            break;
                        }
                    };
                    event_sender.try_send(ClientEvent::LoggedIn)?;
                    keep_alive_task = Some(tokio::task::spawn({
                        let data = Arc::clone(&keep_alive_data);
                        let write = Arc::clone(&write);
                        let state = Arc::clone(&state);
                        async move {
                            handle_keep_alive(write, state, data).await;
                        }
                    }));
                }
            }

            ClientState::Play => {
                if raw_packet.packet_id == S03ChatMessage::packet_id() {
                    let chat_message = S03ChatMessage::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::ChatMessage {
                        message: chat_message.message,
                    })?
                }
                else if raw_packet.packet_id == S04ClientStatus::packet_id() {
                    unimplemented!()
                }
                else if raw_packet.packet_id == S10KeepAlive::packet_id() {
                    debug!("Received keep alive");
                    let mut data = keep_alive_data.write().await;
                    let keep_alive = S10KeepAlive::decode(raw_packet)?;

                    if keep_alive.id == data.last_id {
                        data.has_responded = true;
                    }
                    event_sender.try_send(ClientEvent::Ping {
                        delay: data.sent_at.elapsed().as_millis(),
                    })?;
                }
                else if raw_packet.packet_id == S12PlayerPosition::packet_id() {
                    let player_position = S12PlayerPosition::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerPosition {
                        x: player_position.x,
                        y: player_position.feet_y,
                        z: player_position.z,
                        on_ground: player_position.on_ground,
                    })?;
                }
                else if raw_packet.packet_id == S13PlayerPositionAndRotation::packet_id() {
                    let packet = S13PlayerPositionAndRotation::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerPositionAndRotation {
                        x: packet.x,
                        y: packet.feet_y,
                        z: packet.z,
                        yaw: packet.yaw,
                        pitch: packet.pitch,
                        on_ground: packet.on_ground,
                    })?;
                }
                else if raw_packet.packet_id == S14PlayerRotation::packet_id() {
                    let player_rotation = S14PlayerRotation::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerRotation {
                        yaw: player_rotation.yaw,
                        pitch: player_rotation.pitch,
                        on_ground: player_rotation.on_ground,
                    })?;
                }
                else if raw_packet.packet_id == S1CEntityAction::packet_id() {
                    let entity_action = S1CEntityAction::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::EntityAction {
                        entity_id: entity_action.entity_id,
                        action_id: entity_action.action_id,
                        jump_boost: entity_action.jump_boost,
                    })?;
                }
                else if raw_packet.packet_id == S1APlayerAbilities::packet_id() {
                    let player_abilities = S1APlayerAbilities::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerAbilities {
                        is_flying: player_abilities.flags & 0x02 == 0x02,
                    })?;
                }
                else if raw_packet.packet_id == S1BPlayerDigging::packet_id() {
                    let player_digging = S1BPlayerDigging::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerDigging {
                        status: player_digging.status,
                        position: player_digging.position,
                        face: player_digging.face,
                    })?;
                }
                else if raw_packet.packet_id == S25HeldItemChange::packet_id() {
                    let held_item_change = S25HeldItemChange::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::HeldItemChange {
                        slot: held_item_change.slot,
                    })?;
                }
                else if raw_packet.packet_id == S28CreativeInventoryAction::packet_id() {
                    let creative_inventory_action = S28CreativeInventoryAction::decode(raw_packet)?;
                    event_sender
                        .try_send(ClientEvent::CreativeInventoryAction {
                            slot_id: creative_inventory_action.slot_id,
                            slot: creative_inventory_action.slot,
                        })
                        .unwrap();
                }
                else if raw_packet.packet_id == S2CAnimation::packet_id() {
                    let animation = S2CAnimation::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::Animation {
                        hand: animation.hand,
                    })?;
                }
                else if raw_packet.packet_id == S2EPlayerBlockPlacement::packet_id() {
                    let player_block_placement = S2EPlayerBlockPlacement::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PlayerBlockPlacement {
                        hand: player_block_placement.hand,
                        position: player_block_placement.position,
                        face: player_block_placement.face,
                        cursor_position_x: player_block_placement.cursor_position_x,
                        cursor_position_y: player_block_placement.cursor_position_y,
                        cursor_position_z: player_block_placement.cursor_position_z,
                        inside_block: player_block_placement.inside_block,
                    })?;
                }
            }

            ClientState::Disconnected => {
                break;
            }
        }
    }

    if let Some(keep_alive_task) = keep_alive_task {
        keep_alive_task.await.unwrap();
    }
    Ok(())
}
