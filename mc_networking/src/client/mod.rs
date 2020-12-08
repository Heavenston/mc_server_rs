pub mod client_event;
mod ingoing_packets;
mod keep_alive;
mod outgoing_packets;

use crate::{
    packets::{client_bound::*, PacketCompression, RawPacket},
    DecodingError,
};
use client_event::*;
use ingoing_packets::*;
use outgoing_packets::*;

use lazy_static::lazy_static;
use log::*;
use openssl::{self, pkey, rsa::Rsa};
use serde_json::json;
use std::sync::Arc;
use tokio::{self, net::TcpStream, sync::RwLock, task::spawn};

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

#[derive(Clone)]
pub struct Client {
    compression: Arc<RwLock<PacketCompression>>,
    state: Arc<RwLock<ClientState>>,
    #[allow(dead_code)]
    event_sender: flume::Sender<ClientEvent>,
    packet_sender: flume::Sender<OutgoingPacketEvent>,
    peer_addr: std::net::SocketAddr,
}
impl Client {
    pub fn new(
        socket: TcpStream,
        event_buffer: usize,
        packet_buffer: usize,
    ) -> (Self, flume::Receiver<ClientEvent>) {
        let peer_addr = socket.peer_addr().unwrap();
        let (read, write) = socket.into_split();
        let state = Arc::new(RwLock::new(ClientState::Handshaking));
        let (event_sender, event_receiver) = flume::bounded(event_buffer);
        let (packet_sender, packet_receiver) = flume::bounded(packet_buffer);
        let compression = Arc::default();

        spawn({
            let packet_sender = packet_sender.clone();
            let state = Arc::clone(&state);
            let listener_sender = event_sender.clone();
            let compression = Arc::clone(&compression);
            let peer_addr = peer_addr;
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
                            if (e.kind() == std::io::ErrorKind::UnexpectedEof
                                || e.kind() == std::io::ErrorKind::Interrupted
                                || e.kind() == std::io::ErrorKind::ConnectionReset
                                || e.kind() == std::io::ErrorKind::ConnectionAborted)
                                && *state.read().await == ClientState::Play
                            {
                                *state.write().await = ClientState::Disconnected;
                                listener_sender.try_send(ClientEvent::Logout).unwrap();
                            }
                            else if *state.read().await == ClientState::Play {
                                *state.write().await = ClientState::Disconnected;
                                listener_sender.try_send(ClientEvent::Logout).unwrap();
                                packet_sender
                                    .send_async(OutgoingPacketEvent::Packet(
                                        C19PlayDisconnect {
                                            reason: json!({
                                                "text": "Unexpected io error"
                                            }),
                                        }
                                        .to_rawpacket(),
                                    ))
                                    .await
                                    .unwrap();
                                error!(
                                    "Unexpected io error while handling {:?}, {:#?}",
                                    peer_addr, e
                                );
                            }
                        }
                        ClientListenError::EventSenderSendError(e) => {
                            *state.write().await = ClientState::Disconnected;
                            panic!("could not send event {:?} from client {:?}", e, peer_addr);
                        }

                        e => {
                            *state.write().await = ClientState::Disconnected;
                            listener_sender.try_send(ClientEvent::Logout).unwrap();
                            packet_sender
                                .send_async(OutgoingPacketEvent::Packet(
                                    C19PlayDisconnect {
                                        reason: json!({
                                            "text": "Unexpected error"
                                        }),
                                    }
                                    .to_rawpacket(),
                                ))
                                .await
                                .unwrap();
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

    pub async fn send_raw_packet_async(&self, packet: RawPacket) {
        self.packet_sender
            .send_async(OutgoingPacketEvent::Packet(packet))
            .await
            .unwrap();
    }
    pub fn send_raw_packet_sync(&self, packet: RawPacket) {
        self.packet_sender
            .send(OutgoingPacketEvent::Packet(packet))
            .unwrap();
    }
    pub async fn send_packet_async<U: ClientBoundPacket>(&self, packet: &U) {
        let raw_packet = packet.to_rawpacket();
        self.send_raw_packet_async(raw_packet).await;
    }
    pub fn send_packet_sync<U: ClientBoundPacket>(&self, packet: &U) {
        let raw_packet = packet.to_rawpacket();
        self.send_raw_packet_sync(raw_packet);
    }
}
