pub mod listener;

use crate::packets::client_bound::*;
use crate::packets::server_bound::*;
use crate::packets::RawPacket;
use listener::*;

use anyhow::{Error, Result};
use log::*;
use serde_json::json;
use std::convert::{TryFrom, TryInto};
use std::net::Shutdown;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;
use crate::data_types::Angle;

#[derive(Clone, Debug)]
enum ClientMessage {
    Init,
}

#[derive(Clone, Debug)]
pub enum ClientState {
    Handshaking,
    Status,
    Login,
    Play,
    Disconnected,
}

pub struct Client<T: ClientListener> {
    write: Arc<Mutex<OwnedWriteHalf>>,
    receiver: mpsc::Receiver<ClientMessage>,
    state: Arc<RwLock<ClientState>>,
    listener: Arc<Mutex<Option<T>>>,
}

impl<T: 'static + ClientListener> Client<T> {
    pub fn new(socket: TcpStream) -> Self {
        let (read, write) = socket.into_split();
        let write = Arc::new(Mutex::new(write));
        let (sender, receiver) = mpsc::channel(10);
        let state = Arc::new(RwLock::new(ClientState::Handshaking));
        let listener = Arc::new(Mutex::new(None));

        tokio::spawn({
            let write = Arc::clone(&write);
            let state = Arc::clone(&state);
            let listener = Arc::clone(&listener);
            async move {
                if let Err(e) =
                    listen_client_packets(read, Arc::clone(&write), sender, listener, state).await
                {
                    error!(
                        "Error while handling {:?} packet: {:#?}",
                        write.lock().await.as_ref().peer_addr().unwrap(),
                        e
                    );
                };
            }
        });

        Client {
            write,
            receiver,
            state,
            listener,
        }
    }
    pub async fn set_listener(&mut self, listener: T) {
        *(self.listener.lock().await) = Some(listener);
    }
    pub async fn get_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

    pub async fn send_packet<U: ClientBoundPacket>(&self, packet: &U) -> Result<()> {
        let raw_packet = packet.to_rawpacket();
        self.write
            .lock()
            .await
            .write_all(&raw_packet.encode())
            .await?;
        Ok(())
    }

    pub async fn spawn_entity(
        &self,
        entity_id: i32,
        object_uuid: Uuid,
        kind: i32,
        x: f64,
        y: f64,
        z: f64,
        pitch: Angle,
        yaw: Angle,
        data: i32,
        velocity_x: i16,
        velocity_y: i16,
        velocity_z: i16,
    ) -> Result<()> {
        let spawn_entity_packet = C00SpawnEntity {
            entity_id,
            object_uuid,
            kind,
            x,
            y,
            z,
            pitch,
            yaw,
            data,
            velocity_x,
            velocity_y,
            velocity_z,
        };
        self.send_packet(&spawn_entity_packet).await?;
        Ok(())
    }
    pub async fn join_game(
        &self,
        entity_id: i32,
        is_hardcore: bool,
        gamemode: u8,
        world_names: Vec<String>,
        dimension_codec: C24JoinGameDimensionCodec,
        dimension: C24JoinGameDimensionElement,
        world_name: String,
        hashed_seed: u64,
        view_distance: u8,
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        is_debug: bool,
        is_flat: bool,
    ) -> Result<()> {
        if !((2..=32).contains(&view_distance)) {
            return Err(Error::msg("Invalid render distance"));
        }
        let join_game_packet = C24JoinGame {
            entity_id,
            is_hardcore,
            gamemode,
            previous_gamemode: gamemode,
            world_names,
            dimension_codec,
            dimension,
            world_name,
            hashed_seed,
            max_players: 0,
            view_distance: view_distance as i32,
            reduced_debug_info,
            enable_respawn_screen,
            is_debug,
            is_flat,
        };
        self.send_packet(&join_game_packet).await?;
        Ok(())
    }
}

async fn listen_client_packets<T: ClientListener>(
    mut read: OwnedReadHalf,
    write: Arc<Mutex<OwnedWriteHalf>>,
    _sender: mpsc::Sender<ClientMessage>,
    listener: Arc<Mutex<Option<T>>>,
    state: Arc<RwLock<ClientState>>,
) -> Result<()> {
    loop {
        if let ClientState::Disconnected = state.read().await.clone() {
            break;
        }

        debug!("Reading packet, State({:?})", state.read().await.clone());
        let raw_packet = RawPacket::decode_async(&mut read).await?;
        debug!(
            "Received packet {} with data of length {}",
            raw_packet.packet_id,
            raw_packet.data.len()
        );

        let current_state = state.read().await.clone();
        match current_state {
            ClientState::Handshaking => {
                let handshake: S00Handshake = raw_packet.try_into()?;
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
                    S00Request::try_from(raw_packet)?;
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    let response = ResponsePacket {
                        json_response: listener.on_slp().await,
                    }.to_rawpacket();
                    write
                        .lock()
                        .await
                        .write_all(response.encode().as_ref())
                        .await?;
                } else if raw_packet.packet_id == S01Ping::packet_id() {
                    let packet: S01Ping = raw_packet.try_into()?;
                    let pong = PongPacket {
                        payload: packet.payload,
                    }.to_rawpacket();
                    write.lock().await.write_all(pong.encode().as_ref()).await?;
                    read.as_ref().shutdown(Shutdown::Both)?;
                    *(state.write().await) = ClientState::Disconnected;
                    break;
                } else {
                    return Err(Error::msg("Invalid packet_id"));
                }
            }

            ClientState::Login => {
                if raw_packet.packet_id == S00LoginStart::packet_id() {
                    let login_state = S00LoginStart::try_from(raw_packet)?;
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    match listener.on_login_start(login_state.name).await {
                        LoginStartResult::Accept { uuid, username } => {
                            let login_success =
                                LoginSuccessPacket { uuid, username }.to_rawpacket();
                            write
                                .lock()
                                .await
                                .write_all(login_success.encode().as_ref())
                                .await?;
                            *(state.write().await) = ClientState::Play;
                        }
                        LoginStartResult::Disconnect { reason } => {
                            let disconnect = LoginDisconnectPacket {
                                reason: json!({ "text": reason }),
                            }.to_rawpacket();
                            write
                                .lock()
                                .await
                                .write_all(disconnect.encode().as_ref())
                                .await?;
                            read.as_ref().shutdown(Shutdown::Both)?;
                            *(state.write().await) = ClientState::Disconnected;
                            break;
                        }
                    };
                    listener.on_ready().await;
                }
            }

            ClientState::Disconnected => {
                break;
            }
            s => return Err(Error::msg(format!("Unimplemented client state: {:?}", s))),
        }
    }
    Ok(())
}
