use anyhow::{Error, Result};
use log::*;
use std::{collections::HashMap, path::Path, process::Stdio};
use tokio::{fs, prelude::io::*, process::Command, sync::RwLock};

const SERVER_JAR_URL: &'static str =
    "https://launcher.mojang.com/v1/objects/f02f4473dbf152c23d7d484952121db0b36698cb/server.jar";

pub struct ResourceManager {
    vanilla_blocks: RwLock<Option<serde_json::Value>>,
    block_cache: RwLock<HashMap<String, i32>>,
}
impl ResourceManager {
    pub fn new() -> Self {
        Self {
            vanilla_blocks: RwLock::new(None),
            block_cache: RwLock::default(),
        }
    }

    pub async fn get_block_id(
        &self,
        block_identifier: String,
        properties: Option<HashMap<String, String>>,
    ) -> Result<i32> {
        let vanilla_blocks = self.vanilla_blocks.read().await;
        let vanilla_blocks = vanilla_blocks
            .as_ref()
            .ok_or(Error::msg("no blocks registered"))?;

        let properties_string = properties
            .clone()
            .map(|properties| format!("{:?}", properties));
        let cache_key = properties_string.unwrap_or_default() + &block_identifier;

        if let Some(cached_id) = self.block_cache.read().await.get(&cache_key) {
            return Ok(*cached_id);
        }

        let block = vanilla_blocks
            .as_object()
            .unwrap()
            .get(&block_identifier)
            .ok_or(Error::msg("no block with this identifier were found"))?
            .as_object()
            .unwrap();

        if let Some(properties) = properties {
            if let Some(props) = block.get("properties").map(|v| v.as_object().unwrap()) {
                for key in properties.keys() {
                    if !props.contains_key(key) {
                        return Err(Error::msg("invalid property key"));
                    }
                    if !props[key]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| properties[key] == v.as_str().unwrap())
                    {
                        return Err(Error::msg(format!(
                            "invalid property value '{}'",
                            properties[key]
                        )));
                    }
                }
            }
            else {
                if properties.len() != 0 {
                    return Err(Error::msg("invalid properties"));
                }
            }
            for state in block["states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_object().unwrap())
            {
                let all_properties_match = state["properties"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (key, value.as_str().unwrap()))
                    .all(|(key, value)| properties[key] == value);
                if all_properties_match {
                    let id = state["id"].as_i64().unwrap() as i32;
                    self.block_cache.write().await.insert(cache_key, id);
                    return Ok(id);
                }
            }
            Ok(0)
        }
        else {
            for state in block["states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_object().unwrap())
            {
                if state
                    .get("default")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false)
                {
                    let id = state["id"].as_i64().unwrap() as i32;
                    self.block_cache.write().await.insert(cache_key, id);
                    return Ok(id);
                }
            }
            Err(Error::msg("the block does not have a default"))
        }
    }

    pub async fn load_from_server_generator(&self) -> Result<()> {
        let temp_folder = std::env::temp_dir().join("mc_server_generator");
        let server_jar_file_path = temp_folder.join("server.jar");
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
            .current_dir(temp_folder.clone())
            .arg("-cp")
            .arg("server.jar")
            .arg("net.minecraft.data.Main")
            .arg("--reports")
            .stdout(Stdio::null())
            .spawn()?
            .wait()
            .await?;
        if !exit_status.success() {
            return Err(Error::msg("Java process returned with an error"));
        }
        info!("Generation finished");
        let generated_folder = temp_folder.join("generated");
        let reports_folder = generated_folder.join("reports");

        let vanilla_blocks_text = fs::read_to_string(reports_folder.join("blocks.json")).await?;
        let vanilla_blocks = serde_json::from_str(&vanilla_blocks_text)?;
        *self.vanilla_blocks.write().await = Some(vanilla_blocks);

        Ok(())
    }
}
