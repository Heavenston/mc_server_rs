mod chunk_loader;
mod client_handler;
mod event_handler;
mod registry_codec;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use client_handler::*;
use event_handler::MyEventHandler;
use mc_ecs_server_lib::mc_schedule::McSchedule;
use mc_networking::client::Client;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use legion::{Schedule, World, system, systems::CommandBuffer};
use std::{sync::{Arc, RwLock}, time::Duration};
use tokio::{net::*, runtime};

#[system]
fn client_pusher(cmd: &mut CommandBuffer, #[state] clients: &Arc<RwLock<Vec<ClientComponent>>>) {
    for c in clients.write().unwrap().drain(..) {
        cmd.push((c,));
    }
}
async fn start_network_server(addr: impl ToSocketAddrs, clients: Arc<RwLock<Vec<ClientComponent>>>) {
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (socket, ..) = listener.accept().await.unwrap();
        let (client, event_receiver) = Client::new(socket, 100, 500);
        clients.write().unwrap().push(ClientComponent {
            client, event_receiver
        });
    }
}

fn main() {
    let pending_clients = Default::default();

    // Starts legion in a nes thread
    std::thread::spawn({
        let pending_clients = Arc::clone(&pending_clients);
        || {
            let chunk_provider = Arc::new(StoneChunkProvider::new());

            let mut world: World = World::default();
            let mut schedule = McSchedule::new(MyEventHandler);
            schedule.resources.insert(Arc::clone(&chunk_provider));

            schedule.set_custom_schedule(
                Schedule::builder()
                    .add_system(stone_chunk_provider_system(Arc::clone(&chunk_provider)))
                    .add_system(client_pusher_system(pending_clients))
                    .add_system(handle_clients_system())
                    .build(),
            );

            TickScheduler::builder()
                .profiling_interval(Duration::from_secs(3))
                .build()
                .start(
                    move || {
                        schedule.tick(&mut world);
                    },
                    Some(|profiler: &TickProfiler| {
                        println!("TPS: {:.0}", profiler.tick_per_seconds());
                        println!("DPT: {:?}", profiler.duration_per_tick());
                    }),
                );
        }
    });

    let tokio_runtime = runtime::Builder::new_multi_thread()
        .enable_all().build().unwrap();
    tokio_runtime.enter();
    tokio_runtime.block_on(start_network_server("0.0.0.0:25565", pending_clients));
}
