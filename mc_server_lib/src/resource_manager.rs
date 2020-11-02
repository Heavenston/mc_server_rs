use anyhow::{Error, Result};
use log::*;
use sha2::{digest::FixedOutput, Digest, Sha256};
use std::{collections::HashMap, path::Path, process::Stdio};
use tokio::{fs, prelude::io::*, process::Command, sync::RwLock};
use tokio_compat_02::FutureExt;

const SERVER_JAR_URL: &'static str =
    "https://launcher.mojang.com/v1/objects/f02f4473dbf152c23d7d484952121db0b36698cb/server.jar";
const SERVER_JAR_HASH: [u64; 4] = [
    0x32e450e74c081aecu64.to_be(),
    0x06dcfbadfa5ba9aau64.to_be(),
    0x1c7f370bd869e658u64.to_be(),
    0xcaec0c3004f7ad5bu64.to_be(),
];

pub struct ResourceManager {
    blocks: RwLock<Option<serde_json::Value>>,
    block_cache: RwLock<HashMap<String, i32>>,
    registries: RwLock<Option<serde_json::Value>>,
    registry_cache: RwLock<HashMap<String, i32>>,
}
impl ResourceManager {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(None),
            block_cache: RwLock::default(),
            registries: RwLock::new(None),
            registry_cache: RwLock::default(),
        }
    }

    pub async fn get_block_id(
        &self,
        block_identifier: String,
        properties: Option<HashMap<String, String>>,
    ) -> Result<i32> {
        let vanilla_blocks = self.blocks.read().await;
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

    pub async fn get_registry(&self, registry: &str, entry_name: Option<&str>) -> Option<i32> {
        let registry = if registry.contains(":") {
            registry.to_string()
        }
        else {
            "minecraft:".to_string() + registry
        };
        let cache_key = registry.to_string() + "=" + entry_name.unwrap_or("");
        if let Some(id) = self.registry_cache.read().await.get(&cache_key) {
            return Some(*id);
        }

        let registries = self.registries.read().await;
        let registries = registries.as_ref().unwrap();
        let registry = match registries.as_object().unwrap().get(&registry) {
            Some(a) => a,
            None => return None,
        }
        .as_object()
        .unwrap();
        let default = registry.get("default").map(|a| a.as_str().unwrap());
        if default.is_none() && entry_name.is_none() {
            return None;
        }
        let entry_name = entry_name.unwrap_or(default.unwrap());
        let entry = registry
            .get("entries")
            .unwrap()
            .as_object()
            .unwrap()
            .get(entry_name);
        if entry.is_none() {
            return None;
        }
        let protocol_id = entry
            .unwrap()
            .as_object()
            .unwrap()
            .get("protocol_id")
            .unwrap()
            .as_i64()
            .unwrap() as i32;
        self.registry_cache
            .write()
            .await
            .insert(cache_key, protocol_id);
        Some(protocol_id)
    }

    pub async fn load_from_server_generator(&self) -> Result<()> {
        let temp_folder = std::env::temp_dir().join("mc_server_generator");
        let server_jar_file_path = temp_folder.join("server.jar");
        fs::create_dir_all(temp_folder.clone()).await?;
        let should_download_server_jar = !Path::new(&server_jar_file_path).exists() || {
            let mut hasher = Sha256::new();
            info!("Validating server jar...");
            let mut file = fs::File::open(server_jar_file_path.clone()).await?;
            loop {
                let mut buffer = [0; 2048];
                let read = file.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            let hash = hasher.finalize_fixed();
            if hash.as_slice()
                != &unsafe { std::mem::transmute::<[u64; 4], [u8; 32]>(SERVER_JAR_HASH) }
            {
                info!("The server jar could not be validated");
                true
            }
            else {
                info!("Server jar successfully validated");
                false
            }
        };
        if should_download_server_jar {
            let mut server_jar_file = fs::File::create(server_jar_file_path.clone()).await?;
            info!("Downloading server.jar...");
            let mut server_jar_response = reqwest::get(SERVER_JAR_URL).compat().await?;
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
        *self.blocks.write().await = Some(vanilla_blocks);

        let registries_text = fs::read_to_string(reports_folder.join("registries.json")).await?;
        let registries = serde_json::from_str(&registries_text)?;
        *self.registries.write().await = Some(registries);

        Ok(())
    }
}
