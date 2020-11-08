mod minecraft_data_generator;
mod prismarine_minecraft_data;
mod utils;

use minecraft_data_generator::*;
use prismarine_minecraft_data::*;
use utils::*;

use anyhow::{Error, Result};
use std::{cell::RefCell, collections::HashMap, path::Path};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    sync::RwLock,
};

const VERSION_MANIFEST_URL: &'static str =
    "https://launchermeta.mojang.com/mc/game/version_manifest.json";
const MINECRAFT_VERSION: &'static str = "1.16.4";

std::thread_local! {
    static BLOCK_STATES_CACHE: RefCell<HashMap<String, i32>> = RefCell::new(HashMap::new());
    static REGISTRY_CACHE: RefCell<HashMap<String, i32>> = RefCell::new(HashMap::new());
}

async fn get_server_jar_url() -> Result<String> {
    let version_manifest = download_file_to_json(VERSION_MANIFEST_URL)
        .await?
        .as_object()
        .unwrap()
        .clone();
    let mut version_url = None;
    for version in version_manifest["versions"].as_array().unwrap() {
        let version = version.as_object().unwrap();
        if version["id"].as_str().unwrap() == MINECRAFT_VERSION {
            version_url = Some(version["url"].as_str().unwrap().to_owned());
        }
    }
    let version_url = version_url.expect(&format!(
        "The version expected ({}) does not exist",
        MINECRAFT_VERSION,
    ));

    let version_json = download_file_to_json(&version_url)
        .await?
        .as_object()
        .unwrap()
        .clone();

    Ok(version_json["downloads"].as_object().unwrap()["server"]
        .as_object()
        .unwrap()["url"]
        .as_str()
        .unwrap()
        .to_owned())
}

pub struct ResourceManager {
    prismarine_minecraft_data: RwLock<Option<PrimarineMinecraftData>>,
    minecraft_data_generator: RwLock<Option<MinecraftDataGenerator>>,
}
impl ResourceManager {
    pub fn new() -> Self {
        Self {
            prismarine_minecraft_data: RwLock::new(None),
            minecraft_data_generator: RwLock::new(None),
        }
    }
    pub async fn load(&self) -> Result<()> {
        let cache_folder = std::env::current_dir()?.join("cache");
        fs::create_dir_all(&cache_folder).await?;

        let (prismarine_minecraft_data, minecraft_data_generator) = tokio::join!(
            async {
                let file_path = &cache_folder.join("primarine_minecraft_data.json");
                if Path::new(&file_path).exists() {
                    let bytes = fs::read(&file_path).await.unwrap();
                    serde_json::from_slice::<PrimarineMinecraftData>(&bytes).unwrap()
                }
                else {
                    let primarine_minecraft_data =
                        PrimarineMinecraftData::download().await.unwrap();
                    File::create(file_path)
                        .await
                        .unwrap()
                        .write_all(&serde_json::to_vec(&primarine_minecraft_data).unwrap())
                        .await
                        .unwrap();
                    primarine_minecraft_data
                }
            },
            async {
                let file_path = &cache_folder.join("minecraft_data_generator.json");
                if Path::new(&file_path).exists() {
                    let bytes = fs::read(&file_path).await.unwrap();
                    serde_json::from_slice::<MinecraftDataGenerator>(&bytes).unwrap()
                }
                else {
                    let minecraft_data_generator =
                        MinecraftDataGenerator::download(get_server_jar_url().await.unwrap())
                            .await
                            .unwrap();
                    File::create(file_path)
                        .await
                        .unwrap()
                        .write_all(&serde_json::to_vec(&minecraft_data_generator).unwrap())
                        .await
                        .unwrap();
                    minecraft_data_generator
                }
            }
        );
        *self.prismarine_minecraft_data.write().await = Some(prismarine_minecraft_data);
        *self.minecraft_data_generator.write().await = Some(minecraft_data_generator);
        Ok(())
    }

    pub async fn get_block_state_id(
        &self,
        block_name: &str,
        block_properties: Option<HashMap<String, String>>,
    ) -> Result<i32> {
        let cache_key = block_properties
            .as_ref()
            .unwrap_or(&HashMap::new())
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("")
            + block_name;

        if let Some(id) = BLOCK_STATES_CACHE.with(|c| c.borrow().get(&cache_key).cloned()) {
            return Ok(id);
        }

        let minecraft_data_generator = self.minecraft_data_generator.read().await;
        let minecraft_data_generator = minecraft_data_generator.as_ref().unwrap();
        let block_states = minecraft_data_generator
            .blocks_states
            .get(block_name)
            .ok_or(Error::msg("No such block name"))?;

        let mut id = -1;
        if let Some(block_properties) = block_properties {
            if block_states.properties.is_none() {
                return Err(Error::msg(
                    "Cannot check properties of a block that doesn't have properties",
                ));
            }
            for state in &block_states.states {
                let state_properties = state.properties.as_ref().unwrap();
                if block_properties
                    .iter()
                    .all(|(k, v)| &state_properties[k] == v)
                {
                    id = state.id;
                }
            }
        }
        else {
            id = block_states.states[block_states.default].id;
        }
        BLOCK_STATES_CACHE.with(|cache| cache.borrow_mut().insert(cache_key, id));

        Ok(id)
    }
    pub async fn get_registry(&self, registry_name: &str, value_name: Option<&str>) -> Option<i32> {
        let cache_key = registry_name.to_string() + value_name.unwrap_or("");
        if let Some(id) = REGISTRY_CACHE.with(|c| c.borrow().get(&cache_key).cloned()) {
            return Some(id);
        }

        let minecraft_data_generator = self.minecraft_data_generator.read().await;
        let minecraft_data_generator = minecraft_data_generator.as_ref().unwrap();

        let registry = match minecraft_data_generator.registries.get(registry_name) {
            Some(n) => n,
            None => return None,
        };

        let id;
        if let Some(value_name) = value_name {
            match registry.entries.get(value_name) {
                Some(n) => {
                    id = *n;
                }
                None => return None,
            }
        }
        else {
            let default = match registry.default.as_ref() {
                Some(default) => default,
                None => return None,
            };
            id = registry.entries[default];
        }
        Some(id)
    }

    pub async fn get_protocol_version(&self) -> i32 {
        self.prismarine_minecraft_data
            .read()
            .await
            .as_ref()
            .unwrap()
            .protocol_version
    }
    pub async fn get_minecraft_version(&self) -> String {
        self.prismarine_minecraft_data
            .read()
            .await
            .as_ref()
            .unwrap()
            .minecraft_version
            .clone()
    }
    pub async fn get_minecraft_major_version(&self) -> String {
        self.prismarine_minecraft_data
            .read()
            .await
            .as_ref()
            .unwrap()
            .major_version
            .clone()
    }
}
