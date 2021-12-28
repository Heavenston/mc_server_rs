use super::*;
use crate::packets::{PacketCompression, RawPacket};

use bytes::BytesMut;

use log::*;
use openssl::symm::{Cipher, Crypter, Mode};

use std::sync::Arc;

use tokio::{
    io::AsyncWriteExt,
    net::tcp::OwnedWriteHalf,
    sync::{Notify, RwLock},
    task::block_in_place,
};

#[derive(Debug)]
pub(super) enum OutgoingPacketEvent {
    /// Send a packet
    Packet(RawPacket),
    /// Sends a packet and notify whe it had been sent
    PacketNow(RawPacket, Arc<Notify>),
    SetCompression(PacketCompression),
    /// Sets the shared_key to enable encryption
    SetEncryption(Option<[u8; 16]>),
}

pub(super) async fn listen_outgoing_packets(
    mut write: OwnedWriteHalf,
    packet_receiver: flume::Receiver<OutgoingPacketEvent>,
    _state: Arc<RwLock<ClientState>>,
) {
    let mut packet_buffer = BytesMut::with_capacity(200);
    let mut compression = PacketCompression::default();
    let mut encryption: Option<(Cipher, Crypter)> = None;

    let dummy_notify = Arc::new(Notify::new());

    while let Ok(event) = packet_receiver.recv_async().await {
        match (event, dummy_notify.clone()) {
            (OutgoingPacketEvent::Packet(packet), notify)
            | (OutgoingPacketEvent::PacketNow(packet, notify), ..) => {
                let packet_id = packet.packet_id;
                if packet.will_compress(compression) {
                    block_in_place(|| packet.encode(compression, &mut packet_buffer))
                } else {
                    packet.encode(compression, &mut packet_buffer)
                };
                if let Some((cipher, crypter)) = &mut encryption {
                    let unencrypted = packet_buffer.split();
                    packet_buffer.resize(unencrypted.len() + cipher.block_size(), 0);
                    let encrypted_length =
                        crypter.update(&unencrypted, &mut packet_buffer).unwrap();
                    packet_buffer.truncate(encrypted_length);
                }
                match write.write_all(&packet_buffer).await {
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
