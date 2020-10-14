pub mod listener;

use crate::packets::client_bound::*;
use crate::packets::server_bound::*;
use crate::packets::RawPacket;
use listener::*;

use anyhow::{Error, Result};
use log::*;
use serde_json::json;
use std::convert::TryInto;
use std::net::Shutdown;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{Duration, Instant};

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

    /// Send a packet to the client
    /// This is unsafe because the client may send a response packet that must be handled
    pub async unsafe fn send_packet<U: ClientBoundPacket>(&self, packet: &U) -> Result<()> {
        let raw_packet = packet.to_rawpacket();
        self.write
            .lock()
            .await
            .write_all(&raw_packet.encode())
            .await?;
        Ok(())
    }

    pub async fn spawn_entity(&self, packet: &C00SpawnEntity) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn spawn_experience_orb(&self, packet: &C01SpawnExperienceOrb) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn send_window_items(&self, packet: &C13WindowItems) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn send_plugin_message(&self, packet: &C17PluginMessage) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn join_game(&self, packet: &C24JoinGame) -> Result<()> {
        if !((2..=32).contains(&packet.view_distance)) {
            return Err(Error::msg("Invalid render distance"));
        }
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn send_player_info(&self, packet: &C32PlayerInfo) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn player_position_and_look(&self, packet: &C34PlayerPositionAndLook) -> Result<()> {
        unsafe { self.send_packet(packet) }.await?;
        Ok(())
    }
    pub async fn hold_item_change(&self, slot: i8) -> Result<()> {
        unsafe { self.send_packet(&C3FHoldItemChange { slot }) }.await?;
        Ok(())
    }
    pub async fn update_view_position(&self, chunk_x: i32, chunk_z: i32) -> Result<()> {
        unsafe { self.send_packet(&C40UpdateViewPosition { chunk_x, chunk_z }) }.await?;
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
        unsafe {
            self.send_packet(&C30PlayerAbilities {
                flags: (invulnerable as u8) * 0x01
                    | (flying as u8) * 0x02
                    | (allow_flying as u8) * 0x04
                    | (creative_mode as u8) * 0x08,
                flying_speed,
                fov_modifier,
            })
        }
        .await?;
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
    let keep_alive_timeout = Arc::new(Mutex::new(Instant::now() + Duration::from_secs(10)));
    let has_responded_to_keep_alive = Arc::new(RwLock::new(false));
    let last_keep_alive_id = Arc::new(RwLock::new(0i64));

    let keep_alive_check_task = tokio::task::spawn({
        let keep_alive_timeout = Arc::clone(&keep_alive_timeout);
        let has_responded_to_keep_alive = Arc::clone(&has_responded_to_keep_alive);
        let write = Arc::clone(&write);
        let state = Arc::clone(&state);
        async move {
            loop {
                let timeout = keep_alive_timeout.lock().await.clone();
                tokio::time::delay_until(timeout).await;
                if *has_responded_to_keep_alive.read().await {
                    *has_responded_to_keep_alive.write().await = false;
                } else {
                    debug!("30s since keep alive, closing connection");
                    match write
                        .lock()
                        .await
                        .as_ref()
                        .shutdown(Shutdown::Both) {
                        Err(..) => break,
                        _ => ()
                    }
                    *(state.write().await) = ClientState::Disconnected;
                }
            }
        }
    });
    let keep_alive_send_task = tokio::task::spawn({
        let keep_alive_timeout = Arc::clone(&keep_alive_timeout);
        let has_responded_to_keep_alive = Arc::clone(&has_responded_to_keep_alive);
        let write = Arc::clone(&write);
        let state = Arc::clone(&state);
        let last_keep_alive_id = Arc::clone(&last_keep_alive_id);
        async move {
            let start_instant = Instant::now();
            loop {
                match *state.read().await {
                    ClientState::Play => {
                        if *has_responded_to_keep_alive.read().await {
                            debug!("Keep alive miss");
                            tokio::time::delay_for(Duration::from_millis(500)).await;
                            continue;
                        }
                    }
                    ClientState::Disconnected => break,

                    _ => {
                        tokio::time::delay_for(Duration::from_millis(500)).await;
                        continue;
                    }
                };
                let mut write = write.lock().await;
                let keep_alive_id = Instant::now().duration_since(start_instant).as_millis() as i64;
                *last_keep_alive_id.write().await = keep_alive_id;
                let keep_alive_packet = C1FKeepAlive { id: keep_alive_id };
                write
                    .write_all(&keep_alive_packet.to_rawpacket().encode().as_ref())
                    .await
                    .unwrap();
                debug!("Sent keep alive");
                *has_responded_to_keep_alive.write().await = false;
                *keep_alive_timeout.lock().await = Instant::now() + Duration::from_secs(30);
                tokio::time::delay_for(Duration::from_secs(10)).await;
            }
        }
    });

    loop {
        if let ClientState::Disconnected = state.read().await.clone() {
            break;
        }

        debug!("Reading packet, State({:?})", state.read().await.clone());
        let raw_packet = RawPacket::decode_async(&mut read).await?;
        debug!(
            "Received packet 0x{:x} with data of length {}",
            raw_packet.packet_id,
            raw_packet.data.len()
        );

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
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    let response = C00Response {
                        json_response: listener.on_slp().await,
                    }
                    .to_rawpacket();
                    write
                        .lock()
                        .await
                        .write_all(response.encode().as_ref())
                        .await?;
                } else if raw_packet.packet_id == S01Ping::packet_id() {
                    let packet: S01Ping = raw_packet.try_into()?;
                    let pong = C01Pong {
                        payload: packet.payload,
                    }
                    .to_rawpacket();
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
                    let login_state = S00LoginStart::decode(raw_packet)?;
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    match listener.on_login_start(login_state.name).await {
                        LoginStartResult::Accept { uuid, username } => {
                            let login_success = C02LoginSuccess { uuid, username }.to_rawpacket();
                            write
                                .lock()
                                .await
                                .write_all(login_success.encode().as_ref())
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

            ClientState::Play => {
                if raw_packet.packet_id == S04ClientStatus::packet_id() {
                    let client_status = S04ClientStatus::decode(raw_packet)?;
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    match client_status.action_id {
                        0 => listener.on_perform_respawn().await,
                        1 => listener.on_request_stats().await,
                        _ => return Err(Error::msg("Invalid client status action id")),
                    }
                } else if raw_packet.packet_id == S10KeepAlive::packet_id() {
                    debug!("Received keep alive");
                    let keep_alive = S10KeepAlive::decode(raw_packet)?;
                    if keep_alive.id == *last_keep_alive_id.read().await {
                        *has_responded_to_keep_alive.write().await = true;
                    }
                } else if raw_packet.packet_id == S12PlayerPosition::packet_id() {
                    let player_position = S12PlayerPosition::decode(raw_packet)?;
                    let listener = listener.lock().await;
                    if listener.is_none() {
                        return Err(Error::msg("No listener registered"));
                    }
                    let listener = listener.as_ref().unwrap();
                    listener
                        .on_player_position(
                            player_position.x,
                            player_position.feet_y,
                            player_position.z,
                            player_position.on_ground,
                        )
                        .await;
                }
            }

            ClientState::Disconnected => {
                break;
            }
        }
    }

    keep_alive_check_task.await.unwrap();
    keep_alive_send_task.await.unwrap();
    Ok(())
}
