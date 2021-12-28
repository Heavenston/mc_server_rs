mod minecraft_data_generator;
mod prismarine_minecraft_data;
mod utils;

use minecraft_data_generator::*;
use prismarine_minecraft_data::*;
use utils::*;

use anyhow::{Error, Result};
use fxhash::FxHashMap;
use log::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    cell::RefCell,
    io::Read,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";
const ENABLE_CACHE_COMPRESSION: bool = true;

std::thread_local! {
    static BLOCK_STATES_CACHE: RefCell<FxHashMap<String, i32>> = RefCell::new(FxHashMap::default());
    static REGISTRY_CACHE: RefCell<FxHashMap<String, i32>> = RefCell::new(FxHashMap::default());
    static REGISTRY_KEY_CACHE: RefCell<FxHashMap<String, String>> = RefCell::new(FxHashMap::default());
}

async fn read_compressed_bincode_file<T: DeserializeOwned>(path: &PathBuf) -> Result<T> {
    let data = fs::read(path).await?;
    if ENABLE_CACHE_COMPRESSION {
        let decompress_stream = flate2::read::ZlibDecoder::new(&data[..]);
        Ok(bincode::deserialize_from(decompress_stream)?)
    } else {
        Ok(bincode::deserialize(&data[..])?)
    }
}
async fn write_compressed_bincode_file(path: &PathBuf, data: &impl Serialize) -> Result<()> {
    let uncompressed_bincode = bincode::serialize(data)?;
    if ENABLE_CACHE_COMPRESSION {
        let mut compressed = vec![];
        flate2::read::ZlibEncoder::new(&uncompressed_bincode[..], flate2::Compression::fast())
            .read_to_end(&mut compressed)?;
        File::create(path).await?.write_all(&compressed).await?;
    } else {
        File::create(path)
            .await?
            .write_all(&uncompressed_bincode)
            .await?;
    }
    Ok(())
}

async fn get_server_jar_url(minecraft_version: &str) -> Result<String> {
    let version_manifest = download_file_to_json(VERSION_MANIFEST_URL)
        .await?
        .as_object()
        .unwrap()
        .clone();
    let mut version_url = None;
    for version in version_manifest["versions"].as_array().unwrap() {
        let version = version.as_object().unwrap();
        if version["id"].as_str().unwrap() == minecraft_version {
            version_url = Some(version["url"].as_str().unwrap().to_owned());
        }
    }
    let version_url = version_url.unwrap_or_else(|| {
        panic!(
            "The version expected ({}) does not exist",
            minecraft_version
        )
    });

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

#[derive(Clone, Debug)]
pub struct ResourceManagerConfig<McVer, CacheDir>
where
    McVer: AsRef<str>,
    CacheDir: AsRef<Path>,
{
    pub minecraft_version: McVer,
    pub load_from_cache: bool,
    pub save_in_cache: bool,
    pub cache_directory: Option<CacheDir>,
}
impl<McVer, CacheDir> ResourceManagerConfig<McVer, CacheDir>
where
    McVer: AsRef<str>,
    CacheDir: AsRef<Path>,
{
    pub fn borrow<'a>(&'a self) -> ResourceManagerConfig<&'a McVer, &'a CacheDir> {
        ResourceManagerConfig {
            minecraft_version: &self.minecraft_version,
            load_from_cache: self.load_from_cache,
            save_in_cache: self.save_in_cache,
            cache_directory: self.cache_directory.as_ref(),
        }
    }

    pub fn with_minecraft_version<NewMcVer: AsRef<str>>(
        self,
        minecraft_version: NewMcVer,
    ) -> ResourceManagerConfig<NewMcVer, CacheDir> {
        ResourceManagerConfig {
            minecraft_version,
            load_from_cache: self.load_from_cache,
            save_in_cache: self.save_in_cache,
            cache_directory: self.cache_directory,
        }
    }
    pub fn with_load_from_cache(self, load_from_cache: bool) -> Self {
        Self {
            load_from_cache,
            ..self
        }
    }
    pub fn with_save_in_cahce(self, save_in_cache: bool) -> Self {
        Self {
            save_in_cache,
            ..self
        }
    }
    pub fn with_cache_directory<NewCacheDir: AsRef<Path>>(
        self,
        cache_directory: Option<NewCacheDir>,
    ) -> ResourceManagerConfig<McVer, NewCacheDir> {
        ResourceManagerConfig {
            minecraft_version: self.minecraft_version,
            load_from_cache: self.load_from_cache,
            save_in_cache: self.save_in_cache,
            cache_directory,
        }
    }
}
impl Default for ResourceManagerConfig<&'static str, PathBuf> {
    fn default() -> Self {
        Self {
            minecraft_version: "1.16.4",
            load_from_cache: true,
            save_in_cache: true,
            cache_directory: std::env::current_dir().ok().map(|a| a.join("cache")),
        }
    }
}

/**
 * Download/generate and provide minecraft data extracted from various sources
 */
pub struct ResourceManager {
    prismarine_minecraft_data: PrimarineMinecraftData,
    minecraft_data_generator: MinecraftDataGenerator,
}
impl ResourceManager {
    pub async fn load(
        config: &ResourceManagerConfig<impl AsRef<str>, impl AsRef<Path>>,
    ) -> Result<Self> {
        let cache_folder = config
            .cache_directory
            .as_ref()
            .map(|a| a.as_ref().join(config.minecraft_version.as_ref()));
        if let Some(cache_folder) = &cache_folder {
            fs::create_dir_all(&cache_folder).await?;
        }

        let load_config = config.borrow().with_cache_directory(cache_folder.as_ref());

        let (prismarine_minecraft_data, minecraft_data_generator) = tokio::join!(
            Self::load_primsmarine_data(&load_config),
            Self::load_generator_data(&load_config)
        );

        Ok(Self {
            prismarine_minecraft_data: prismarine_minecraft_data?,
            minecraft_data_generator: minecraft_data_generator?,
        })
    }

    pub async fn load_primsmarine_data(
        config: &ResourceManagerConfig<impl AsRef<str>, impl AsRef<Path>>,
    ) -> Result<PrimarineMinecraftData> {
        let file_path = config
            .cache_directory
            .as_ref()
            .map(|a| a.as_ref().join("primarine_minecraft_data"));

        // Try to load cache data
        let cache_data = match &file_path {
            Some(file_path) => {
                match read_compressed_bincode_file::<PrimarineMinecraftData>(&file_path).await {
                    Ok(c) => Some(c),
                    Err(e) => {
                        debug!("Error while loading primarine cache data: {}", e);
                        None
                    }
                }
            }
            None => None,
        };

        // If no cache data was loaded, download the data
        match cache_data {
            Some(c) => Ok(c),
            None => {
                debug!("No data from cache, downloading primarine data");
                let primarine_minecraft_data =
                    PrimarineMinecraftData::download(config.minecraft_version.as_ref()).await?;
                if let Some(file_path) = &file_path {
                    write_compressed_bincode_file(&file_path, &primarine_minecraft_data)
                        .await
                        .map_err(|e| error!("Could not save primarine cache data: {}", e))
                        .ok();
                }
                debug!("Downloaded primarine data");
                Ok(primarine_minecraft_data)
            }
        }
    }
    pub async fn load_generator_data(
        config: &ResourceManagerConfig<impl AsRef<str>, impl AsRef<Path>>,
    ) -> Result<MinecraftDataGenerator> {
        let file_path = config
            .cache_directory
            .as_ref()
            .map(|a| a.as_ref().join("minecraft_data_generator"));

        // Try to load cache data
        let cache_data = match &file_path {
            Some(file_path) if config.load_from_cache => {
                match read_compressed_bincode_file::<MinecraftDataGenerator>(&file_path).await {
                    Ok(c) => Some(c),
                    Err(e) => {
                        debug!("Error while loading minecraft generator cache data: {}", e);
                        None
                    }
                }
            }
            _ => None,
        };

        // If no cache data was loaded, download the data
        match cache_data {
            Some(c) => Ok(c),
            None => {
                debug!("No data from cache, generating minecraft data");
                let data = MinecraftDataGenerator::download(
                    config.minecraft_version.as_ref(),
                    &get_server_jar_url(config.minecraft_version.as_ref())
                        .await
                        .unwrap(),
                )
                .await
                .unwrap();
                if config.save_in_cache {
                    if let Some(file_path) = &file_path {
                        write_compressed_bincode_file(&file_path, &data)
                            .await
                            .map_err(|e| {
                                debug!("Could not save generator minecraft data to cache: {}", e)
                            });
                    }
                }
                debug!("Minecraft data generated");
                Ok(data)
            }
        }
    }

    pub async fn get_block_state_id(
        &self,
        block_name: &str,
        block_properties: Option<FxHashMap<String, String>>,
    ) -> Result<i32> {
        let cache_key = block_properties
            .as_ref()
            .unwrap_or(&FxHashMap::default())
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("")
            + block_name;

        if let Some(id) = BLOCK_STATES_CACHE.with(|c| c.borrow().get(&cache_key).cloned()) {
            return Ok(id);
        }

        let block_states = self
            .minecraft_data_generator
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
        } else {
            id = block_states.states[block_states.default].id;
        }
        BLOCK_STATES_CACHE.with(|cache| cache.borrow_mut().insert(cache_key, id));

        Ok(id)
    }
    /// Get the id of a registry value
    /// Uses registry default (if any) if value_name is None
    pub async fn get_registry(&self, registry_name: &str, value_name: Option<&str>) -> Option<i32> {
        let cache_key = registry_name.to_string() + value_name.unwrap_or("");
        if let Some(id) = REGISTRY_CACHE.with(|c| c.borrow().get(&cache_key).cloned()) {
            return Some(id);
        }

        let registry = match self.minecraft_data_generator.registries.get(registry_name) {
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
        } else {
            let default = match registry.default.as_ref() {
                Some(default) => default,
                None => return None,
            };
            id = registry.entries[default];
        }
        Some(id)
    }

    /// Get the name of a registry value from it's id
    /// Uses registry default (if any) if id is None
    pub async fn get_registry_value_name(
        &self,
        registry_name: &str,
        id: Option<i32>,
    ) -> Option<String> {
        let cache_key = id.map(|n| n.to_string()).unwrap_or("".into()) + "-key-" + registry_name;
        if let Some(value_name) = REGISTRY_KEY_CACHE.with(|c| c.borrow().get(&cache_key).cloned()) {
            return Some(value_name);
        }

        let registry = match self.minecraft_data_generator.registries.get(registry_name) {
            Some(n) => n,
            None => return None,
        };

        let value_name;
        if let Some(id) = id {
            match registry.inverted_entries.get(&id) {
                Some(n) => {
                    value_name = n.clone();
                }
                None => return None,
            }
        } else {
            let default = match registry.inverted_default.as_ref() {
                Some(default) => default,
                None => return None,
            };
            value_name = registry.inverted_entries[default].clone();
        }
        Some(value_name)
    }

    pub fn get_protocol_version(&self) -> i32 {
        self.prismarine_minecraft_data.protocol_version
    }
    pub fn get_minecraft_version(&self) -> &str {
        &self.prismarine_minecraft_data.minecraft_version
    }
    pub async fn get_minecraft_major_version(&self) -> &str {
        &self.prismarine_minecraft_data.major_version
    }
}
