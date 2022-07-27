use super::{keep_alive::*, *};
use crate::{
    packets::{client_bound::*, server_bound::*, PacketCompression, RawPacket},
    DecodingError,
};

use bytes::BytesMut;

use log::*;
use openssl::{
    rsa::Padding,
    symm::{Cipher, Crypter, Mode},
};
use rand::RngCore;
use serde_json::json;
use std::{convert::TryInto, sync::Arc};
use thiserror::Error;
use tokio::{
    io::AsyncReadExt,
    net::tcp::OwnedReadHalf,
    sync::{oneshot, Notify, RwLock},
    time::Instant,
};

#[derive(Error, Debug)]
pub(super) enum ClientListenError {
    #[error("decoding error: {0:?}")]
    DecodingError(#[from] DecodingError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("could not send packet")]
    PacketSenderSendError(#[from] flume::SendError<OutgoingPacketEvent>),
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
pub(super) type ClientListenResult<T> = Result<T, ClientListenError>;

pub(super) async fn listen_ingoing_packets(
    compression: Arc<RwLock<PacketCompression>>,
    mut read: OwnedReadHalf,
    packet_sender: flume::Sender<OutgoingPacketEvent>,
    event_sender: flume::Sender<ClientEvent>,
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
                    let encrypted = encryption
                        .update(&new_bytes[0..received], &mut decrypted_new_bytes)
                        .unwrap();

                    &decrypted_new_bytes[0..encrypted]
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
                        event_sender
                            .send_async(ClientEvent::ServerListPing {
                                response: response_sender,
                            })
                            .await
                            .unwrap();
                        response_receiver.await.unwrap()
                    };
                    packet_sender
                        .send_async(OutgoingPacketEvent::Packet(
                            C00StatusResponse {
                                json_response: event_response,
                            }
                            .to_rawpacket(),
                        ))
                        .await?;
                }
                else if raw_packet.packet_id == S01Ping::packet_id() {
                    let packet: S01Ping = S01Ping::decode(raw_packet)?;
                    packet_sender
                        .send_async(OutgoingPacketEvent::Packet(
                            C01Pong {
                                payload: packet.payload,
                            }
                            .to_rawpacket(),
                        ))
                        .await?;
                    *(state.write().await) = ClientState::Disconnected;
                    event_sender.send_async(ClientEvent::Logout).await.unwrap();
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
                                .send_async(OutgoingPacketEvent::PacketNow(
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
                                .send_async(OutgoingPacketEvent::SetCompression(
                                    *compression.read().await,
                                ))
                                .await?;
                        }
                    };
                }

                if raw_packet.packet_id == S00LoginStart::packet_id() {
                    debug!("login start");
                    let login_state = S00LoginStart::decode(raw_packet)?;

                    let event_response = {
                        let (response_sender, response_receiver) = oneshot::channel();
                        event_sender
                            .send_async(ClientEvent::LoginStart {
                                username: login_state.name.clone(),
                                response: response_sender,
                            })
                            .await
                            .unwrap();
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
                            login_uuid = Some(uuid);
                            login_username = Some(username.clone());
                            if encrypt {
                                packet_sender
                                    .send_async(OutgoingPacketEvent::Packet(
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
                                    .send_async(OutgoingPacketEvent::Packet(
                                        C02LoginSuccess { uuid, username }.to_rawpacket(),
                                    ))
                                    .await?;
                                *(state.write().await) = ClientState::Play;

                                event_sender
                                    .send_async(ClientEvent::LoggedIn)
                                    .await
                                    .unwrap();
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
                                .send_async(OutgoingPacketEvent::Packet(
                                    C00LoginDisconnect {
                                        reason: json!({ "text": reason }),
                                    }
                                    .to_rawpacket(),
                                ))
                                .await?;
                            *(state.write().await) = ClientState::Disconnected;
                            event_sender.send_async(ClientEvent::Logout).await.unwrap();
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

                    encryption = Some(
                        Crypter::new(
                            Cipher::aes_128_cfb8(),
                            Mode::Decrypt,
                            &shared_key,
                            Some(&shared_key),
                        )
                        .unwrap(),
                    );
                    packet_sender
                        .send_async(OutgoingPacketEvent::SetEncryption(Some(shared_key)))
                        .await?;

                    enable_compression!();

                    packet_sender
                        .send_async(OutgoingPacketEvent::Packet(
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

                    event_sender
                        .send_async(ClientEvent::LoggedIn)
                        .await
                        .unwrap();
                }
            }

            ClientState::Play => {
                if raw_packet.packet_id == S04ChatMessage::packet_id() {
                    let chat_message = S04ChatMessage::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::ChatMessage {
                            message: chat_message.message,
                        })
                        .await
                        .unwrap()
                }
                else if raw_packet.packet_id == S06ClientCommand::packet_id() {
                    unimplemented!()
                }
                else if raw_packet.packet_id == S0AClickContainer::packet_id() {
                    let click_window = S0AClickContainer::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::ClickWindow {
                            window_id: click_window.window_id,
                            slot_id: click_window.slot_id,
                            button: click_window.button,
                            action_number: click_window.action_number,
                            mode: click_window.mode,
                            clicked_item: click_window.clicked_item,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S0CPluginMessage::packet_id() {
                    let plugin_message = S0CPluginMessage::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PluginMessage {
                            channel: plugin_message.channel,
                            data: plugin_message.data,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S11KeepAlive::packet_id() {
                    debug!("Received keep alive");
                    let mut data = keep_alive_data.write().await;
                    let keep_alive = S11KeepAlive::decode(raw_packet)?;

                    if keep_alive.id == data.last_id {
                        data.has_responded = true;
                    }
                    event_sender
                        .send_async(ClientEvent::Ping {
                            delay: data.sent_at.elapsed().as_millis(),
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S13SetPlayerPosition::packet_id() {
                    let player_position = S13SetPlayerPosition::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerPosition {
                            x: player_position.x,
                            y: player_position.feet_y,
                            z: player_position.z,
                            on_ground: player_position.on_ground,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S13SetPlayerPositionAndRotation::packet_id() {
                    let packet = S13SetPlayerPositionAndRotation::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerPositionAndRotation {
                            x: packet.x,
                            y: packet.feet_y,
                            z: packet.z,
                            yaw: packet.yaw,
                            pitch: packet.pitch,
                            on_ground: packet.on_ground,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S15SetPlayerRotation::packet_id() {
                    let player_rotation = S15SetPlayerRotation::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerRotation {
                            yaw: player_rotation.yaw,
                            pitch: player_rotation.pitch,
                            on_ground: player_rotation.on_ground,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S1DPlayerCommand::packet_id() {
                    let entity_action = S1DPlayerCommand::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::EntityAction {
                            entity_id: entity_action.entity_id,
                            action_id: entity_action.action_id,
                            jump_boost: entity_action.jump_boost,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S1BPlayerAbilities::packet_id() {
                    let player_abilities = S1BPlayerAbilities::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerAbilities {
                            is_flying: player_abilities.flags & 0x02 == 0x02,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S1CPlayerAction::packet_id() {
                    let player_digging = S1CPlayerAction::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerDigging {
                            status: player_digging.status,
                            position: player_digging.position,
                            face: player_digging.face,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S25HeldItemChange::packet_id() {
                    let held_item_change = S25HeldItemChange::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::HeldItemChange {
                            slot: held_item_change.slot,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S28CreativeInventoryAction::packet_id() {
                    let creative_inventory_action = S28CreativeInventoryAction::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::CreativeInventoryAction {
                            slot_id: creative_inventory_action.slot_id,
                            slot: creative_inventory_action.slot,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S2CAnimation::packet_id() {
                    let animation = S2CAnimation::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::Animation {
                            hand: animation.hand,
                        })
                        .await
                        .unwrap();
                }
                else if raw_packet.packet_id == S2EPlayerBlockPlacement::packet_id() {
                    let player_block_placement = S2EPlayerBlockPlacement::decode(raw_packet)?;
                    event_sender
                        .send_async(ClientEvent::PlayerBlockPlacement {
                            hand: player_block_placement.hand,
                            position: player_block_placement.position,
                            face: player_block_placement.face,
                            cursor_position_x: player_block_placement.cursor_position_x,
                            cursor_position_y: player_block_placement.cursor_position_y,
                            cursor_position_z: player_block_placement.cursor_position_z,
                            inside_block: player_block_placement.inside_block,
                        })
                        .await
                        .unwrap();
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
