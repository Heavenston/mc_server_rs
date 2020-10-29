
use tokio::fs;
use std::path::Path;
use anyhow::{Result, Error};
use tokio::prelude::io::*;
use log::*;
use tokio::process::Command;
use std::process::Stdio;

const SERVER_JAR_URL: &'static str = "https://launcher.mojang.com/v1/objects/f02f4473dbf152c23d7d484952121db0b36698cb/server.jar";

pub struct ResourceManager {

}
impl ResourceManager {
    pub fn new() -> Self {
        Self {

        }
    }

    pub async fn get_from_generator(&self) -> Result<()> {
        let temp_folder = {
            let mut temp = std::env::temp_dir();
            temp.push("mc_server_generator");
            temp
        };
        let server_jar_file_path = {
            let mut path = temp_folder.clone();
            path.push("server.jar");
            path
        };
        fs::create_dir_all(temp_folder.clone()).await?;
        if !Path::new(&server_jar_file_path).exists() {
            let mut server_jar_file = fs::File::create(server_jar_file_path.clone()).await?;
            info!("Downloading server.jar...");
            let mut server_jar_response = reqwest::get(SERVER_JAR_URL).await?;
            let size = server_jar_response.content_length().unwrap_or(0);
            let mut last_remaining = 0;
            while let Some(chunk) = server_jar_response.chunk().await? {
                let remaining = server_jar_response.content_length().unwrap_or(0);
                if (last_remaining as i64 - remaining as i64).abs() > 3000000 {
                    info!("{}MB/{}MB", (size - remaining) / 1000000, size / 1000000);
                    last_remaining = remaining;
                }
                server_jar_file.write_all(&chunk).await?;
            }
        }

        info!("Generating minecraft data...");
        let exit_status = Command::new("java")
            .current_dir(temp_folder)
            .arg("-cp")
            .arg("server.jar")
            .arg("net.minecraft.data.Main")
            .arg("--reports")
            .stdout(Stdio::null())
            .spawn()?.await?;
        if !exit_status.success() {
            return Err(Error::msg("Java process returned with an error"));
        }
        info!("Generation finished");

        Ok(())
    }
}
