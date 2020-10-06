pub mod listener;

use crate::packets::RawPacket;
use crate::packets::server_bound::{ServerBoundPacket, HandshakePacket, RequestPacket, PingPacket};
use crate::packets::client_bound::*;
use listener::ClientListener;

use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedWriteHalf, OwnedReadHalf};
use anyhow::{Result, Error};
use std::convert::{TryInto, TryFrom};
use std::sync::Arc;
use tokio::prelude::io::AsyncWriteExt;
use std::net::Shutdown;
use log::*;

#[derive(Clone, Debug)]
enum ClientMessage {
    Init,
}

pub struct Client {
    write: Arc<Mutex<OwnedWriteHalf>>,
    receiver: mpsc::Receiver<ClientMessage>,
    state: Arc<RwLock<ClientState>>,
}

#[derive(Clone, Debug)]
pub enum ClientState {
    Handshaking,
    Status,
    Login,
    Play,
    Disconnected,
}

impl Client {

    pub fn new<T: 'static +  ClientListener>(socket: TcpStream, listener: Arc<T>) -> Self {
        let (read, write) = socket.into_split();
        let write = Arc::new(Mutex::new(write));
        let (sender, receiver) = mpsc::channel(10);
        let state = Arc::new(RwLock::new(ClientState::Handshaking));

        tokio::spawn({
            let write = Arc::clone(&write);
            let state = Arc::clone(&state);
            async move {
                if let Err(e) = listen_client_packets(
                    read,
                    Arc::clone(&write),
                    sender,
                    listener,
                    state,
                ).await {
                    error!("Error while handling {:?} packet: {:#?}", write.lock().await.as_ref().peer_addr().unwrap(), e);
                };
            }
        });

        Client {
            write,
            receiver,
            state,
        }
    }

    pub async fn get_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

}

async fn listen_client_packets<T: ClientListener>(
    mut read: OwnedReadHalf,
    write: Arc<Mutex<OwnedWriteHalf>>,
    sender: mpsc::Sender<ClientMessage>,
    listener: Arc<T>,
    state: Arc<RwLock<ClientState>>
) -> Result<()> {
    loop {
        if let ClientState::Disconnected = state.read().await.clone() {
            break;
        }

        debug!(
            "Reading packet, State({:?})",
            state.read().await.clone()
        );
        let raw_packet = RawPacket::decode_async(&mut read).await?;
        debug!(
            "Received packet {} with data of length {}",
            raw_packet.packet_id,
            raw_packet.data.len()
        );

        let current_state = state.read().await.clone();
        match current_state {
            ClientState::Handshaking => {
                let handshake: HandshakePacket = raw_packet.try_into()?;
                debug!("Received Handshake: {:?}", handshake);
                *(state.write().await) = match handshake.next_state {
                    1 => ClientState::Status,
                    2 => ClientState::Login,
                    _ => return Err(Error::msg("Invalid handshake packet"))
                };
                debug!("New state: {:?}", state.read().await.clone());
            }

            ClientState::Status => {
                if raw_packet.packet_id == RequestPacket::packet_id() {
                    RequestPacket::try_from(raw_packet)?;
                    let response: RawPacket = ResponsePacket::new(listener.on_slp()).into();
                    write.lock().await.write_all(response.encode().as_ref()).await?;
                }
                else if raw_packet.packet_id == PingPacket::packet_id() {
                    let packet: PingPacket = raw_packet.try_into()?;
                    let pong: RawPacket = PongPacket::new(packet.payload).into();
                    write.lock().await.write_all(pong.encode().as_ref()).await?;
                    read.as_ref().shutdown(Shutdown::Both)?;
                    break;
                }
                else {
                    return Err(Error::msg("Invalid packet_id"))
                }
            }

            ClientState::Disconnected => {
                break;
            }
            s => return Err(Error::msg(format!("Unimplemented client state: {:?}", s)))
        }
    }
    Ok(())
}
