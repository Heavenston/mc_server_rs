use super::utils::*;

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::FromIterator, process::Stdio};
use tokio::{
    fs::{self, File},
    process::Command,
};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Registry {
    pub default: Option<String>,
    pub entries: HashMap<String, i32>,
    pub inverted_default: Option<i32>,
    pub inverted_entries: HashMap<i32, String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BlockState {
    pub properties: Option<HashMap<String, String>>,
    pub id: i32,
    pub default: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BlockStates {
    pub properties: Option<HashMap<String, Vec<String>>>,
    pub default: usize,
    pub states: Vec<BlockState>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct MinecraftDataGenerator {
    pub blocks_states: HashMap<String, BlockStates>,
    pub registries: HashMap<String, Registry>,
}
impl MinecraftDataGenerator {
    pub async fn download(server_url: String) -> Result<Self> {
        let temp_folder = std::env::temp_dir().join("mc_server_minecraft_data_generator");
        fs::create_dir_all(&temp_folder).await?;
        let mut server_jar_file = File::create(temp_folder.join("server.jar")).await?;
        download_file_to_writer(&mut server_jar_file, &server_url).await?;

        let child = Command::new("java")
            .args(&[
                "-cp",
                "server.jar",
                "net.minecraft.data.Main",
                "--reports",
                "--server",
            ])
            .current_dir(&temp_folder)
            .stdout(Stdio::null())
            .spawn()?
            .wait()
            .await?;
        if !child.success() {
            return Err(Error::msg(child.to_string()));
        }

        let blocks: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(temp_folder.join("generated/reports/blocks.json")).await?,
        )?;
        let mut blocks_states = HashMap::new();
        for (block_name, value) in blocks.as_object().unwrap() {
            let value = value.as_object().unwrap();
            let properties = if value.contains_key("properties") {
                let raw_properties = value["properties"].as_object().unwrap();
                let mut properties = HashMap::new();
                for (prop_name, prop_values) in raw_properties {
                    let prop_values = prop_values
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().to_owned())
                        .collect::<Vec<_>>();
                    properties.insert(prop_name.clone(), prop_values);
                }
                Some(properties)
            }
            else {
                None
            };

            let raw_states = value["states"].as_array().unwrap();
            let mut default_state = 0;
            let mut states = Vec::with_capacity(raw_states.len());
            for state in raw_states {
                let state = state.as_object().unwrap();
                let id_default = state
                    .get("default")
                    .map(|d| d.as_bool().unwrap())
                    .unwrap_or(false);
                if id_default {
                    default_state = states.len();
                }
                states.push(BlockState {
                    properties: if state.contains_key("properties") {
                        Some(HashMap::from_iter(
                            state["properties"]
                                .as_object()
                                .unwrap()
                                .iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned())),
                        ))
                    }
                    else {
                        None
                    },
                    id: state["id"].as_i64().unwrap() as i32,
                    default: id_default,
                });
            }
            blocks_states.insert(
                block_name.clone(),
                BlockStates {
                    properties,
                    default: default_state,
                    states,
                },
            );
        }

        let mut registries = HashMap::new();
        for (k, registry) in serde_json::from_str::<HashMap<String, serde_json::Value>>(
            &fs::read_to_string(temp_folder.join("generated/reports/registries.json")).await?,
        )? {
            let registry = registry.as_object().unwrap();
            let entries = HashMap::from_iter(
                registry["entries"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v["protocol_id"].as_i64().unwrap() as i32)),
            );
            let default = registry
                .get("default")
                .map(|s| s.as_str().unwrap().to_string());
            registries.insert(
                k,
                Registry {
                    inverted_default: default.as_ref().map(|d| entries[d]),
                    inverted_entries: {
                        let mut ie = HashMap::new();
                        for (k, v) in &entries {
                            ie.insert(*v, k.clone());
                        }
                        ie
                    },
                    default,
                    entries,
                },
            );
        }

        fs::remove_dir_all(temp_folder).await?;

        Ok(Self {
            blocks_states,
            registries,
        })
    }
}
