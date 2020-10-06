use mc_networking::client::listener::ClientListener;
use mc_networking::client::Client;

use anyhow::{Error, Result};
use fern::colors::{Color, ColoredLevelConfig};
use log::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::task;
use tokio::sync::Mutex;
use async_trait::async_trait;

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

struct MyClientListener;
#[async_trait]
impl ClientListener for MyClientListener {
    async fn on_slp(&self) -> Value {
        json!({
            "version": {
                "name": "1.16.3",
                "protocol": 753
            },
            "players": {
                "max": 10,
                "online": 0,
                "sample": []
            },
            "description": "Hi"
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logger();
    let mut listener = TcpListener::bind("0.0.0.0:25565").await?;
    let mut clients = Vec::new();

    loop {
        let (socket, _) = listener.accept().await?;
        let mut client = Client::new(socket);
        client.set_listener(MyClientListener).await;
        clients.push(client);
    }

    Ok(())
}
