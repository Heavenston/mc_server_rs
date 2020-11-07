use super::{utils::*, MINECRAFT_VERSION};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

const PRIMARINE_MINECRAFT_DATA_BASE_URL: &'static str =
    "https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master";

async fn download_minecraft_data_file(file: impl Into<PathBuf>) -> Result<serde_json::Value> {
    let mut url = Url::parse(PRIMARINE_MINECRAFT_DATA_BASE_URL).unwrap();
    url.set_path(
        PathBuf::from(url.path())
            .join(&file.into())
            .to_str()
            .unwrap(),
    );

    Ok(download_file_to_json(&url.to_string()).await?)
}

#[derive(Serialize, Deserialize)]
pub struct PrimarineMinecraftData {
    pub minecraft_version: String,
    pub protocol_version: i32,
    pub major_version: String,
}
impl PrimarineMinecraftData {
    pub async fn download() -> Result<Self> {
        let data_paths = download_minecraft_data_file("data/dataPaths.json").await?;

        let version_data_paths = data_paths["pc"].as_object().unwrap()[MINECRAFT_VERSION]
            .as_object()
            .unwrap();

        let version = &download_minecraft_data_file(
            PathBuf::from("data")
                .join(version_data_paths["version"].as_str().unwrap())
                .join("version.json"),
        )
        .await?
        .as_object()
        .unwrap()
        .clone();

        let minecraft_version = version["minecraftVersion"].as_str().unwrap().to_owned();
        let protocol_version = version["version"].as_i64().unwrap().to_owned() as i32;
        let major_version = version["majorVersion"].as_str().unwrap().to_owned();

        Ok(Self {
            minecraft_version,
            protocol_version,
            major_version,
        })
    }
}
