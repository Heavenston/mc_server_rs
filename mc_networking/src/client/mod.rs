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

/// Handles TCPStreams as minecraft clients into a stream of events
#[derive(Clone)]
#[allow(dead_code)] // TODO: Some fields aren't *yet* used, but if they are never used, please
                    // remove them
pub struct Client {
    compression: Arc<RwLock<PacketCompression>>,
    state: Arc<RwLock<ClientState>>,
    event_sender: flume::Sender<ClientEvent>,
    packet_sender: flume::Sender<OutgoingPacketEvent>,
    peer_addr: std::net::SocketAddr,
}
impl Client {
    /// Creates a new [Client] from a tokio socket
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

        // Packet sending task
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
                        | ClientListenError::DecodingError(DecodingError::IoError(e))
                            if (e.kind() == std::io::ErrorKind::UnexpectedEof
                                || e.kind() == std::io::ErrorKind::Interrupted
                                || e.kind() == std::io::ErrorKind::ConnectionReset
                                || e.kind() == std::io::ErrorKind::ConnectionAborted)
                                && *state.read().await == ClientState::Play =>
                        {
                            ()
                        }

                        e => {
                            *state.write().await = ClientState::Disconnected;
                            packet_sender
                                .send_async(OutgoingPacketEvent::Packet(
                                    C17Disconnect {
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
                    *state.write().await = ClientState::Disconnected;
                    listener_sender.try_send(ClientEvent::Logout).unwrap();
                };
            }
        });

        // Packet from client receiving task
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

    /// Return the current connection state
    pub async fn get_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

    /// Add a raw packet to the send buffer
    /// Block asynchronously if the buffer is full
    pub async fn send_raw_packet_async(&self, packet: RawPacket) {
        self.packet_sender
            .send_async(OutgoingPacketEvent::Packet(packet))
            .await
            .unwrap();
    }
    /// Add a raw packet to the send buffer
    /// Block the current thread if the buffer is full
    pub fn send_raw_packet_sync(&self, packet: RawPacket) {
        self.packet_sender
            .send(OutgoingPacketEvent::Packet(packet))
            .unwrap();
    }
    /// Add a packet to the send buffer
    /// Block asynchronously if the buffer is full
    pub async fn send_packet_async<U: ClientBoundPacket>(&self, packet: &U) {
        let raw_packet = packet.to_rawpacket();
        self.send_raw_packet_async(raw_packet).await;
    }
    /// Add a packet to the send buffer
    /// Block the current thread if the buffer is full
    pub fn send_packet_sync<U: ClientBoundPacket>(&self, packet: &U) {
        let raw_packet = packet.to_rawpacket();
        self.send_raw_packet_sync(raw_packet);
    }
}
