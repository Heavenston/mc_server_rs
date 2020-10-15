mod location;
mod my_server;

use mc_networking::client::Client;

use crate::my_server::{handle_client, Server};
use anyhow::Result;
use fern::colors::{Color, ColoredLevelConfig};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

fn setup_logger() {
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
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logger();
    let mut listener = TcpListener::bind("0.0.0.0:25565").await?;
    let server = Arc::new(RwLock::new(Server::new()));

    loop {
        let (socket, _) = listener.accept().await?;
        handle_client(Arc::clone(&server), socket).await;
    }
}
