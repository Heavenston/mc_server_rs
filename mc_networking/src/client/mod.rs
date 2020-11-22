pub mod client_event;

use crate::{
    data_types::Angle,
    packets::{client_bound::*, server_bound::*, PacketCompression, RawPacket},
    DecodingError,
};
use client_event::*;

use bytes::BytesMut;
use lazy_static::lazy_static;
use log::*;
use openssl::{
    aes::AesKey,
    pkey,
    rsa::{Padding, Rsa},
    symm::{Cipher, Crypter, Mode},
};
use rand::RngCore;
use serde_json::json;
use std::{convert::TryInto, sync::Arc};
use thiserror::Error;
use tokio::{
    io::Sink,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    prelude::io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot, Notify, RwLock},
    task::{block_in_place, spawn},
    time::{sleep, Duration, Instant},
};

const KEEP_ALIVE_TIMEOUT: u64 = 30_000;
const KEEP_ALIVE_INTERVAL: u64 = 15_000;

lazy_static! {
    static ref RSA_KEYPAIR: Rsa<pkey::Private> = Rsa::generate(1024).unwrap();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientState {
    Handshaking,
    Status,
    Login,
    Play,
    Disconnected,
}

type PacketSendResult = Result<(), std::io::Error>;

#[derive(Clone)]
pub struct Client {
    compression: Arc<RwLock<PacketCompression>>,
    state: Arc<RwLock<ClientState>>,
    #[allow(dead_code)]
    event_sender: mpsc::Sender<ClientEvent>,
    packet_sender: mpsc::Sender<OutgoingPacketEvent>,
    peer_addr: std::net::SocketAddr,
}
impl Client {
    pub fn new(socket: TcpStream) -> (Self, mpsc::Receiver<ClientEvent>) {
        let peer_addr = socket.peer_addr().unwrap();
        let (read, write) = socket.into_split();
        let state = Arc::new(RwLock::new(ClientState::Handshaking));
        let (event_sender, event_receiver) = mpsc::channel(100);
        let (packet_sender, packet_receiver) = mpsc::channel(100);
        let compression = Arc::default();

        spawn({
            let packet_sender = packet_sender.clone();
            let state = Arc::clone(&state);
            let listener_sender = event_sender.clone();
            let compression = Arc::clone(&compression);
            let peer_addr = peer_addr.clone();
            async move {
                if let Err(e) = listen_ingoing_packets(
                    compression,
                    read,
                    packet_sender.clone(),
                    listener_sender.clone(),
                    Arc::clone(&state),
                )
                .await
                {
                    match e {
                        ClientListenError::IoError(e)
                        | ClientListenError::DecodingError(DecodingError::IoError(e)) => {
                            if e.kind() == std::io::ErrorKind::UnexpectedEof
                                && *state.read().await == ClientState::Play
                            {
                                *state.write().await = ClientState::Disconnected;
                                listener_sender.try_send(ClientEvent::Logout).unwrap();
                            }
                            else if *state.read().await == ClientState::Play {
                                error!("Unexpected error while handling {:?}, {:#?}", peer_addr, e);
                            }
                        }
                        ClientListenError::EventSenderSendError(e) => {
                            warn!("could not send event {:?} from client {:?}", e, peer_addr);
                            *state.write().await = ClientState::Disconnected;
                            // TODO: Send a disconnect packet to client
                        }

                        e => {
                            error!("Unexpected error while handling {:?}, {:#?}", peer_addr, e);
                        }
                    }
                };
            }
        });

        spawn({
            let state = state.clone();
            async move {
                listen_outgoing_packets(write, packet_receiver, state).await;
            }
        });

        (
            Client {
                compression,
                state,
                event_sender,
                packet_sender,
                peer_addr,
            },
            event_receiver,
        )
    }

    pub async fn get_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

    pub async fn send_raw_packet(&self, packet: RawPacket) -> PacketSendResult {
        self.packet_sender
            .send(OutgoingPacketEvent::Packet(packet))
            .await
            .unwrap();
        Ok(())
    }
    pub async fn send_packet<U: ClientBoundPacket>(&self, packet: &U) -> PacketSendResult {
        let raw_packet = packet.to_rawpacket();
        self.send_raw_packet(raw_packet).await
    }

    pub async fn hold_item_change(&self, slot: i8) -> PacketSendResult {
        self.send_packet(&C3FHoldItemChange { slot }).await?;
        Ok(())
    }

    pub async fn update_view_position(&self, chunk_x: i32, chunk_z: i32) -> PacketSendResult {
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
    ) -> PacketSendResult {
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

    pub async fn destroy_entities(&self, entities: Vec<i32>) -> PacketSendResult {
        self.send_packet(&C36DestroyEntities { entities }).await?;
        Ok(())
    }

    pub async fn send_entity_head_look(&self, entity_id: i32, head_yaw: Angle) -> PacketSendResult {
        self.send_packet(&C3AEntityHeadLook {
            entity_id,
            head_yaw,
        })
        .await?;
        Ok(())
    }

    pub async fn unload_chunk(&self, chunk_x: i32, chunk_z: i32) -> PacketSendResult {
        self.send_packet(&C1CUnloadChunk { chunk_x, chunk_z })
            .await?;
        Ok(())
    }
}

struct KeepAliveData {
    pub has_responded: bool,
    pub last_id: i64,
    pub sent_at: Instant,
}

async fn handle_keep_alive(
    packet_sender: mpsc::Sender<OutgoingPacketEvent>,
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
            packet_sender
                .send(OutgoingPacketEvent::Packet(
                    C1FKeepAlive { id }.to_rawpacket(),
                ))
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
            if !data.read().await.has_responded {
                if data.read().await.sent_at.elapsed().as_millis() >= (KEEP_ALIVE_TIMEOUT as u128) {
                    debug!("Keep alive timeout, closing");
                    // TODO: Send disconnect packet
                    *state.write().await = ClientState::Disconnected;
                }
                else {
                    debug!("Keep alive miss, sending it again");
                    packet_sender
                        .send(OutgoingPacketEvent::Packet(
                            C1FKeepAlive {
                                id: data.read().await.last_id,
                            }
                            .to_rawpacket(),
                        ))
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

#[derive(Error, Debug)]
enum ClientListenError {
    #[error("decoding error: {0:?}")]
    DecodingError(#[from] DecodingError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("could not send an event down the event sender: {0:?}")]
    EventSenderSendError(#[from] mpsc::error::TrySendError<ClientEvent>),
    #[error("could not send packet")]
    PacketSenderSendError(#[from] mpsc::error::SendError<OutgoingPacketEvent>),
    #[error("could not receive an event response")]
    ResponseRecvError(#[from] oneshot::error::RecvError),
    #[error(
        "received an invalid packet (id {packet_id:x}, name {packet_name:?}) on state {state:?}: \
         {message}"
    )]
    InvalidPacket {
        packet_id: i32,
        state: ClientState,
        packet_name: Option<String>,
        message: String,
    },
}
type ClientListenResult<T> = Result<T, ClientListenError>;

async fn listen_ingoing_packets(
    compression: Arc<RwLock<PacketCompression>>,
    mut read: OwnedReadHalf,
    packet_sender: mpsc::Sender<OutgoingPacketEvent>,
    event_sender: mpsc::Sender<ClientEvent>,
    state: Arc<RwLock<ClientState>>,
) -> ClientListenResult<()> {
    let keep_alive_data = Arc::new(RwLock::new(KeepAliveData {
        has_responded: false,
        sent_at: Instant::now(),
        last_id: 0,
    }));
    let mut keep_alive_task = None;

    let mut login_uuid = None;
    let mut login_username = None;
    let mut login_compress = false;
    let login_verify_token: [u8; 4] = {
        let mut bytes = [0; 4];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes
    };

    let mut read_bytes = BytesMut::with_capacity(10);
    let mut encryption: Option<Crypter> = None;

    loop {
        if let ClientState::Disconnected = state.read().await.clone() {
            break;
        }

        trace!("Reading packet, State({:?})", state.read().await.clone());
        let packet_compression = *compression.read().await;
        let raw_packet = {
            let mut new_bytes = [0; 1024];
            let mut decrypted_new_bytes = [0; 1024];
            loop {
                match RawPacket::decode(&mut read_bytes, packet_compression) {
                    Ok(raw_packet) => break raw_packet,
                    Err(DecodingError::NotEnoughBytes) => (),
                    Err(e) => return Err(e.into()),
                }
                let received = read.read(&mut new_bytes).await?;
                let decrypted_output = if let Some(encryption) = &mut encryption {
                    encryption
                        .update(
                            &new_bytes[0..received],
                            &mut decrypted_new_bytes[0..received],
                        )
                        .unwrap();

                    &decrypted_new_bytes[0..received]
                }
                else {
                    &new_bytes[0..received]
                };
                read_bytes.extend_from_slice(&decrypted_output[0..received]);
            }
        };
        trace!(
            "Received packet 0x{:x} with data of length {}",
            raw_packet.packet_id,
            raw_packet.data.len()
        );

        let current_state = state.read().await.clone();
        match current_state {
            ClientState::Handshaking => {
                let handshake = S00Handshake::decode(raw_packet)?;
                trace!("Received Handshake: {:?}", handshake);
                *(state.write().await) = match handshake.next_state {
                    1 => ClientState::Status,
                    2 => ClientState::Login,
                    _ => {
                        return Err(ClientListenError::InvalidPacket {
                            packet_id: 0x00,
                            state: current_state,
                            packet_name: Some("Handshake".to_string()),
                            message: "invalid next state".to_string(),
                        })
                    }
                };
                trace!("New state: {:?}", state.read().await.clone());
            }

            ClientState::Status => {
                if raw_packet.packet_id == S00Request::packet_id() {
                    S00Request::decode(raw_packet)?;
                    let event_response = {
                        let (response_sender, response_receiver) = oneshot::channel();
                        event_sender.try_send(ClientEvent::ServerListPing {
                            response: response_sender,
                        })?;
                        response_receiver.await.unwrap()
                    };
                    packet_sender
                        .send(OutgoingPacketEvent::Packet(
                            C00Response {
                                json_response: event_response,
                            }
                            .to_rawpacket(),
                        ))
                        .await?;
                }
                else if raw_packet.packet_id == S01Ping::packet_id() {
                    let packet: S01Ping = S01Ping::decode(raw_packet)?;
                    packet_sender
                        .send(OutgoingPacketEvent::Packet(
                            C01Pong {
                                payload: packet.payload,
                            }
                            .to_rawpacket(),
                        ))
                        .await?;
                    *(state.write().await) = ClientState::Disconnected;
                    break;
                }
                else {
                    return Err(ClientListenError::InvalidPacket {
                        packet_id: raw_packet.packet_id,
                        state: current_state,
                        packet_name: None,
                        message: "unknown packet id".to_string(),
                    });
                }
            }

            ClientState::Login => {
                macro_rules! enable_compression {
                    () => {
                        if login_compress {
                            let new_compression = 50;
                            let compression_notify = Arc::new(Notify::new());
                            packet_sender
                                .send(OutgoingPacketEvent::PacketNow(
                                    C03SetCompression {
                                        threshold: new_compression,
                                    }
                                    .to_rawpacket(),
                                    compression_notify.clone(),
                                ))
                                .await?;
                            compression_notify.notified().await;
                            *compression.write().await = PacketCompression::new(new_compression);
                            packet_sender
                                .send(OutgoingPacketEvent::SetCompression(
                                    *compression.read().await,
                                ))
                                .await?;
                        }
                    };
                };

                if raw_packet.packet_id == S00LoginStart::packet_id() {
                    debug!("login start");
                    let login_state = S00LoginStart::decode(raw_packet)?;

                    let event_response = {
                        let (response_sender, response_receiver) = oneshot::channel();
                        event_sender.try_send(ClientEvent::LoginStart {
                            username: login_state.name.clone(),
                            response: response_sender,
                        })?;
                        response_receiver.await?
                    };
                    match event_response {
                        LoginStartResult::Accept {
                            uuid,
                            username,
                            encrypt,
                            compress,
                        } => {
                            login_compress = compress;
                            login_uuid = Some(uuid.clone());
                            login_username = Some(username.clone());
                            if encrypt {
                                packet_sender
                                    .send(OutgoingPacketEvent::Packet(
                                        C01EncryptionRequest {
                                            server_id: "".to_string(),
                                            public_key: RSA_KEYPAIR.public_key_to_der().unwrap(),
                                            verify_token: login_verify_token.to_vec(),
                                        }
                                        .to_rawpacket(),
                                    ))
                                    .await?;
                            }
                            else {
                                enable_compression!();

                                packet_sender
                                    .send(OutgoingPacketEvent::Packet(
                                        C02LoginSuccess { uuid, username }.to_rawpacket(),
                                    ))
                                    .await?;
                                *(state.write().await) = ClientState::Play;

                                event_sender.try_send(ClientEvent::LoggedIn)?;
                                keep_alive_task = Some(tokio::task::spawn({
                                    let data = Arc::clone(&keep_alive_data);
                                    let packet_sender = packet_sender.clone();
                                    let state = Arc::clone(&state);
                                    async move {
                                        handle_keep_alive(packet_sender, state, data).await;
                                    }
                                }));
                            }
                        }
                        LoginStartResult::Disconnect { reason } => {
                            packet_sender
                                .send(OutgoingPacketEvent::Packet(
                                    C00LoginDisconnect {
                                        reason: json!({ "text": reason }),
                                    }
                                    .to_rawpacket(),
                                ))
                                .await?;
                            *(state.write().await) = ClientState::Disconnected;
                            break;
                        }
                    };
                }
                else if raw_packet.packet_id == S01EncryptionResponse::packet_id() {
                    debug!("encryption response");
                    let uuid = login_uuid.unwrap();
                    let username = login_username.clone().unwrap();
                    let encryption_response = S01EncryptionResponse::decode(raw_packet)?;
                    let shared_key: [u8; 16] = {
                        let mut shared_key = [0; 128];
                        let len = RSA_KEYPAIR
                            .private_decrypt(
                                &encryption_response.shared_secret,
                                &mut shared_key,
                                Padding::PKCS1,
                            )
                            .unwrap();
                        shared_key[0..len].try_into().unwrap()
                    };

                    let token: [u8; 4] = {
                        let mut token = [0; 128];
                        let len = RSA_KEYPAIR
                            .private_decrypt(
                                &encryption_response.verify_token,
                                &mut token,
                                Padding::PKCS1,
                            )
                            .unwrap();
                        token[0..len].try_into().unwrap()
                    };
                    if token != login_verify_token {
                        panic!("NOOOOOOOOOOOOOOOOOOOOOO");
                        // TODO: Handle this case a "little" bit better
                    }

                    packet_sender
                        .send(OutgoingPacketEvent::SetEncryption(Some(shared_key)))
                        .await?;

                    enable_compression!();

                    packet_sender
                        .send(OutgoingPacketEvent::Packet(
                            C02LoginSuccess { uuid, username }.to_rawpacket(),
                        ))
                        .await?;

                    *(state.write().await) = ClientState::Play;
                    keep_alive_task = Some(tokio::task::spawn({
                        let data = Arc::clone(&keep_alive_data);
                        let packet_sender = packet_sender.clone();
                        let state = Arc::clone(&state);
                        async move {
                            handle_keep_alive(packet_sender, state, data).await;
                        }
                    }));

                    event_sender.try_send(ClientEvent::LoggedIn)?;
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
                else if raw_packet.packet_id == S09ClickWindow::packet_id() {
                    let click_window = S09ClickWindow::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::ClickWindow {
                        window_id: click_window.window_id,
                        slot_id: click_window.slot_id,
                        button: click_window.button,
                        action_number: click_window.action_number,
                        mode: click_window.mode,
                        clicked_item: click_window.clicked_item,
                    })?;
                }
                else if raw_packet.packet_id == S0BPluginMessage::packet_id() {
                    let plugin_message = S0BPluginMessage::decode(raw_packet)?;
                    event_sender.try_send(ClientEvent::PluginMessage {
                        channel: plugin_message.channel,
                        data: plugin_message.data,
                    })?;
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
                    event_sender.try_send(ClientEvent::CreativeInventoryAction {
                        slot_id: creative_inventory_action.slot_id,
                        slot: creative_inventory_action.slot,
                    })?;
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
                else {
                    debug!("Unknown packet id received {:02x}", raw_packet.packet_id);
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

#[derive(Debug)]
enum OutgoingPacketEvent {
    /// Send a packet
    Packet(RawPacket),
    /// Sends a packet and notify whe it had been sent
    PacketNow(RawPacket, Arc<Notify>),
    SetCompression(PacketCompression),
    /// Sets the shared_key to enable encryption
    SetEncryption(Option<[u8; 16]>),
}

async fn listen_outgoing_packets(
    mut write: OwnedWriteHalf,
    mut packet_receiver: mpsc::Receiver<OutgoingPacketEvent>,
    _state: Arc<RwLock<ClientState>>,
) {
    let mut packet_buffer = BytesMut::with_capacity(200);
    let mut compression = PacketCompression::default();
    let mut encryption: Option<(Cipher, Crypter)> = None;

    let dummy_notify = Arc::new(Notify::new());

    while let Some(event) = packet_receiver.recv().await {
        match (event, dummy_notify.clone()) {
            (OutgoingPacketEvent::Packet(packet), notify)
            | (OutgoingPacketEvent::PacketNow(packet, notify), ..) => {
                let packet_id = packet.packet_id;
                if packet.will_compress(compression) {
                    block_in_place(|| packet.encode(compression, &mut packet_buffer))
                }
                else {
                    packet.encode(compression, &mut packet_buffer)
                };
                if let Some((cipher, crypter)) = &mut encryption {
                    let unencrypted = packet_buffer.split();
                    packet_buffer.resize(unencrypted.len() + cipher.block_size(), 0);
                    let encrypted_length =
                        crypter.update(&unencrypted, &mut packet_buffer).unwrap();
                    packet_buffer.truncate(encrypted_length);
                }
                match write.write_all(&mut packet_buffer).await {
                    Ok(..) => (),
                    Err(e) => warn!("Error when sending packet 0x{:02x}: '{}'", packet_id, e),
                }
                write.flush().await.unwrap();
                notify.notify_one();
                packet_buffer.clear();
            }
            (OutgoingPacketEvent::SetCompression(nc), ..) => compression = nc,
            (OutgoingPacketEvent::SetEncryption(e), ..) => match e {
                Some(shared_key) => {
                    let cipher = Cipher::aes_128_cfb8();
                    encryption = Some((
                        cipher,
                        Crypter::new(cipher, Mode::Encrypt, &shared_key, Some(&shared_key))
                            .unwrap(),
                    ));
                }
                None => encryption = None,
            },
        }
    }
}
