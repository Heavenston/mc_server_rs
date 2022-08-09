mod chunk_loader;
mod client_handler;
mod registry_codec;
mod game_systems;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use client_handler::{ ClientEventsComponent, handle_clients };
use mc_ecs_server_lib::mc_app::{ McApp, McAppStage };
use mc_ecs_server_lib::entity::ClientComponent;
use mc_networking::client::Client;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use std::{ sync::{ Arc, RwLock }, time::Duration };

use bevy_ecs::system::Commands;
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

fn client_pusher_system(
    clients: Arc<RwLock<Vec<(ClientComponent, ClientEventsComponent)>>>,
) -> impl FnMut(Commands) {
    move |mut commands: Commands| {
        for (a, b) in clients.write().unwrap().drain(..) {
            commands.spawn()
                .insert(a)
                .insert(b);
        }
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

    setup_logger(if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info });

    // Starts legion in a nes thread
    std::thread::spawn({
        let pending_clients = Arc::clone(&pending_clients);
        || {
            let chunk_provider = Arc::new(StoneChunkProvider::new());

            let mut app = McApp::new();
            app.world.insert_resource(Arc::clone(&chunk_provider));

            app.add_system(McAppStage::BeforeTick, client_pusher_system(pending_clients));

            app.add_system(McAppStage::Tick, stone_chunk_provider);
            app.add_system(McAppStage::Tick, handle_clients);
            app.add_system_set(McAppStage::Tick, game_systems::game_systems());

            TickScheduler::builder()
                .profiling_interval(Duration::from_secs(3))
                .build()
                .start(
                    move || {
                        app.tick();
                    },
                    Some(|profiler: &TickProfiler| {
                        if let Some(dpt) = profiler.duration_per_tick() {
                            info!("TPS: {:.0}", profiler.tick_per_seconds());
                            info!("DPT: {:?}", dpt);
                        }
                    }),
                );
        }
    });

    let tokio_runtime = runtime::Builder::new_multi_thread()
        .enable_all().build().unwrap();
    let _ = tokio_runtime.enter();
    tokio_runtime.block_on(start_network_server("0.0.0.0:25565", pending_clients));
}
