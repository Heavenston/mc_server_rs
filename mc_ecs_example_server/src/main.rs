mod chunk_loader;
mod client_handler;
mod event_handler;
mod registry_codec;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use client_handler::{ ClientEventsComponent, handle_clients_system };
use event_handler::MyEventHandler;
use mc_ecs_server_lib::mc_schedule::McSchedule;
use mc_ecs_server_lib::entity::ClientComponent;
use mc_networking::client::Client;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use std::{ sync::{ Arc, RwLock }, time::Duration };

use legion::{ Schedule, World, system, systems::CommandBuffer };
use tokio::{ net::*, runtime };
use fern::colors::{Color, ColoredLevelConfig};
use log::*;

fn setup_logger(log_filter: log::LevelFilter) {
    let colors_line = ColoredLevelConfig::new()
        .debug(Color::BrightBlack)
        .info(Color::Green)
        .warn(Color::Yellow)
        .error(Color::Red);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = colors_line.color(record.level()),
                message = message,
            ))
        })
        .level(log_filter)
        .level_for("hyper", log::LevelFilter::Info)
        .level_for("reqwest", log::LevelFilter::Info)
        .level_for("mio", log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

#[system]
fn client_pusher(cmd: &mut CommandBuffer, #[state] clients: &Arc<RwLock<Vec<(ClientComponent, ClientEventsComponent)>>>) {
    for (a, b) in clients.write().unwrap().drain(..) {
        cmd.push((a, b));
    }
}
async fn start_network_server(addr: impl ToSocketAddrs, clients: Arc<RwLock<Vec<(ClientComponent, ClientEventsComponent)>>>) {
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (socket, ..) = listener.accept().await.unwrap();
        let (client, event_receiver) = Client::new(socket, 100, 500);
        clients.write().unwrap().push((
            ClientComponent(client), ClientEventsComponent(event_receiver)
        ));
    }
}

fn main() {
    let pending_clients = Default::default();

    setup_logger(LevelFilter::Debug);

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
                    //.add_system(test_clients_system())
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
                        //world.pack(legion::storage::PackOptions::force());
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
    let _ = tokio_runtime.enter();
    tokio_runtime.block_on(start_network_server("0.0.0.0:25565", pending_clients));
}
