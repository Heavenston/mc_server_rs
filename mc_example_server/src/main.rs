mod commands;
mod entities;
mod generator;
mod server;

use server::Server;

use clap::{App, Arg};
use fern::colors::{Color, ColoredLevelConfig};
use log::*;
use std::sync::Arc;

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

#[tokio::main]
async fn main() {
    let clap_matches = App::new("Mc Example Server")
        .author("Heavenstone")
        .arg(
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .multiple(true)
                .help("Enable debug logs, or Trace logs when present twice or more"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("port")
                .default_value("25565")
                .validator(|p| match p.parse::<i64>() {
                    Err(..) => Err("Must be a valid number".to_string()),
                    Ok(v) => {
                        if v > 65353 {
                            Err("Cannot be higher than 65353".to_string())
                        }
                        else {
                            Ok(())
                        }
                    }
                })
                .takes_value(true),
        )
        .get_matches();

    setup_logger(match clap_matches.occurrences_of("debug") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    });

    info!("Loading server...");
    let server = Arc::new(Server::new().await);
    Server::start_ticker(Arc::clone(&server)).await;
    info!("Server loaded");

    let port = clap_matches.value_of("port").unwrap();

    info!("Starting server on port {}", port);
    match Server::listen(Arc::clone(&server), format!("0.0.0.0:{}", port)).await {
        Ok(j) => {
            info!("Server started");
            if let Err(error) = j.await {
                error!("Server stopped with error: {}", error)
            }
        }
        Err(error) => error!("Could not start server: {}", error),
    }
}
