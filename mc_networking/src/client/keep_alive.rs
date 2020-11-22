use super::*;
use crate::packets::client_bound::*;

use log::*;
use std::sync::Arc;
use tokio::{
    self,
    sync::{mpsc, RwLock},
    time::{sleep, Duration, Instant},
};

pub(super) struct KeepAliveData {
    pub has_responded: bool,
    pub last_id: i64,
    pub sent_at: Instant,
}

pub(super) async fn handle_keep_alive(
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
