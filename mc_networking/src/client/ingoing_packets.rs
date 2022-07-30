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

    // Used for saving the uuid between login packets
    let mut login_uuid = None;
    // Used for saving the player's username between login packets
    let mut login_username = None;
    // Used for saving wether to start compressing packets between login packets
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
                            packet_id: S00Handshake::PACKET_ID,
                            state: current_state,
                            packet_name: Some("Handshake".to_string()),
                            message: "invalid next state".to_string(),
                        })
                    }
                };
                trace!("New state: {:?}", state.read().await.clone());
            }

            ClientState::Status => {
                if raw_packet.packet_id == S00Request::PACKET_ID {
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
                else if raw_packet.packet_id == S01Ping::PACKET_ID {
                    let packet: S01Ping = S01Ping::decode(raw_packet)?;
                    packet_sender
                        .send_async(OutgoingPacketEvent::Packet(
                            C01Pong {
                                payload: packet.payload,
                            }.to_rawpacket(),
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
                                    }.to_rawpacket(),
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

                match raw_packet.packet_id {
                    S00LoginStart::PACKET_ID => {
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
                                                }.to_rawpacket(),
                                        ))
                                        .await?;
                                    // The login process continues after receiving the
                                    // S01EncryptionResponse packet
                                }
                                else {
                                    enable_compression!(); // Check for login_compress is done in the
                                                           // macro

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

                                    // Start sending keep alive packets
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
                                            }.to_rawpacket(),
                                    ))
                                    .await?;
                                *(state.write().await) = ClientState::Disconnected;
                                event_sender.send_async(ClientEvent::Logout).await.unwrap();
                                break;
                            }
                        };
                    }

                    S01EncryptionResponse::PACKET_ID => {
                        debug!("encryption response");
                        let (uuid, username) = match (&login_uuid, &login_username) {
                            (Some(uuid), Some(username)) => (uuid.clone(), username.clone()),
                            _ => return Err(ClientListenError::InvalidPacket {
                                packet_id: raw_packet.packet_id,
                                state: current_state,
                                packet_name: Some("encryption response".to_string()),
                                message: "this packet must follow a login start packet".to_string(),
                            }),
                        };
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
                                    match &encryption_response.verify_token {
                                        S01VerifyToken::With { verify_token } => verify_token.as_slice(),
                                        _ => return Err(ClientListenError::InvalidPacket {
                                            packet_id: S01EncryptionResponse::PACKET_ID,
                                            state: current_state,
                                            packet_name: Some("encryption response".to_string()),
                                            message: "the verify token is menatory".to_string(),
                                        }),
                                    },
                                    &mut token,
                                    Padding::PKCS1,
                                )
                                .unwrap();
                            token[0..len].try_into().unwrap()
                        };
                        if token != login_verify_token {
                            return Err(ClientListenError::InvalidPacket {
                                packet_id: S01EncryptionResponse::PACKET_ID,
                                state: current_state,
                                packet_name: Some("encryption response".to_string()),
                                message: "given verify token doesn't match".to_string(),
                            });
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

                    _ => {
                        debug!("Unknown packet id received (login state) {:02x}", raw_packet.packet_id);
                    }
                }
            }

            ClientState::Play => {
                macro_rules! match_packets {
                    ( $struct_name: ident body $variant: ident ) => {
                        event_sender
                            .send_async(ClientEvent::$variant($struct_name::decode(raw_packet)?))
                            .await
                            .unwrap()
                    };

                    ( $struct_name: ident body $body: block ) => {
                        $body
                    };

                    ( $($struct_name:ident => $arg:tt),* $(, _ $el:block)? ) => {
                        match raw_packet.packet_id {
                            $(
                            $struct_name::PACKET_ID => {
                                match_packets!($struct_name body $arg);
                            }
                            )*,
                            $(
                                _ => $el
                            )?
                        }
                    }
                }

                match_packets! {
                    S04ChatMessage => ChatMessage,
                    S06ClientCommand => { unimplemented!("S06ClientCommand") },
                    S0AClickContainer => ClickContainer,
                    S0CPluginMessage => PluginMessage,
                    S11KeepAlive => {
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
                    },
                    S13SetPlayerPositionAndRotation => SetPlayerPositionAndRotation,
                    S15SetPlayerRotation => SetPlayerRotation,
                    S1DPlayerCommand => PlayerCommand,
                    S1BPlayerAbilities => PlayerAbilities,
                    S1CPlayerAction => PlayerAction,
                    S27SetHeldItem => SetHeldItem,
                    S2ASetCreativeModeSlot => SetCreativeModeSlot,
                    S2ESwingArm => SwingArm,
                    S30UseItemOn => UseItemOn,
                    _ {
                        debug!("Unknown packet id received (play state) 0x{:02x}", raw_packet.packet_id);
                    }
                };
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
