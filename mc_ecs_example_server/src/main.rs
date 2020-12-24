mod chunk_loader;
mod client_handler;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use mc_ecs_server_lib::mc_schedule::McSchedule;
use mc_networking::client::Client;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use legion::{Schedule, World};
use std::{sync::Arc, time::Duration};
use tokio::{net::*, runtime};

fn start_legion_world() {
    let chunk_provider = Arc::new(StoneChunkProvider::new());

    let mut world: World = World::default();
    let mut schedule = McSchedule::new();

    schedule.set_custom_schedule(
        Schedule::builder()
            .add_system(stone_chunk_provider_system(Arc::clone(&chunk_provider)))
            .build(),
    );

    TickScheduler::builder()
        .profiling_interval(Duration::from_secs(10))
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

async fn start_network_server(addr: impl ToSocketAddrs) {
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (socket, ..) = listener.accept().await.unwrap();
        let (client, event_receiver) = Client::new(
            socket, 
            100, 
            500
        );
        tokio::spawn(client_handler::handle_client(client, event_receiver));
    }
}

fn main() {
    std::thread::spawn(start_legion_world);

    let tokio_runtime = runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    tokio_runtime.enter();
    tokio_runtime.block_on(start_network_server("0.0.0.0:25565"));
}
