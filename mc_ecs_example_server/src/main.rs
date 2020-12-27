mod chunk_loader;
mod client_handler;
mod event_handler;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use client_handler::*;
use event_handler::MyEventHandler;
use mc_ecs_server_lib::mc_schedule::McSchedule;
use mc_networking::client::Client;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use legion::{Schedule, World};
use std::{sync::Arc, time::Duration};
use tokio::{net::*, runtime};

fn start_legion_world(client_list: Arc<ClientList>) {
    let chunk_provider = Arc::new(StoneChunkProvider::new());

    let mut world: World = World::default();
    let mut schedule = McSchedule::new(MyEventHandler);

    schedule.set_custom_schedule(
        Schedule::builder()
            .add_system(stone_chunk_provider_system(Arc::clone(&chunk_provider)))
            .add_system(handle_clients_system(client_list))
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

async fn start_network_server(addr: impl ToSocketAddrs, client_list: Arc<ClientList>) {
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (socket, ..) = listener.accept().await.unwrap();
        let (client, event_receiver) = Client::new(socket, 100, 500);
        client_list.lock().unwrap().push(HandledClient {
            client,
            event_receiver,
        });
    }
}

fn main() {
    let client_list = Arc::new(ClientList::default());

    std::thread::spawn({
        let client_list = client_list.clone();
        move || start_legion_world(client_list)
    });

    let tokio_runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio_runtime.enter();
    tokio_runtime.block_on({
        let client_list = client_list.clone();
        async move { start_network_server("0.0.0.0:25565", client_list).await }
    });
}
