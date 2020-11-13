use mc_networking::map;
use mc_server_lib::{chunk_holder::ChunkProvider, resource_manager::ResourceManager};
use mc_utils::ChunkData;

use async_trait::async_trait;
use noise::{NoiseFn, Perlin};
use std::{path::PathBuf, sync::Arc};
use tokio::{io::AsyncWriteExt, task::spawn_blocking};

pub struct Generator {
    grass: bool,
    noise: Perlin,
    noise_scale: f64,
    base_height: i32,
    height_diff: i32,
    resource_manager: Arc<ResourceManager>,
    world_folder: PathBuf,
}
impl Generator {
    pub fn new(grass: bool, resource_manager: Arc<ResourceManager>) -> Self {
        let world_folder = std::env::current_dir().unwrap().join("world");
        std::fs::create_dir_all(&world_folder).unwrap();
        Self {
            grass,
            noise: Perlin::new(),
            noise_scale: 1.0 / 200.0,
            base_height: 80,
            height_diff: 100,
            resource_manager,
            world_folder,
        }
    }

    async fn generate_chunk_data(&self, chunk_x: i32, chunk_z: i32) -> Box<ChunkData> {
        let stone = self
            .resource_manager
            .get_block_state_id("minecraft:stone".into(), None)
            .await
            .unwrap() as u16;
        let snow_block = self
            .resource_manager
            .get_block_state_id("minecraft:snow_block".into(), None)
            .await
            .unwrap() as u16;

        let mut data = Box::new(ChunkData::new());
        for local_x in 0..16 {
            let global_x = chunk_x * 16 + local_x;
            let noise_x = global_x as f64 * self.noise_scale;
            for local_z in 0..16 {
                let global_z = chunk_z * 16 + local_z;
                let noise_z = global_z as f64 * self.noise_scale;
                let target_height = self.base_height as f64
                    + (self.noise.get([noise_x, noise_z]) * (self.height_diff as f64 / 2.0));
                let block_height = target_height.floor() as u8;
                let remaining_height = target_height.fract();
                for y in 0..(block_height - 2) {
                    data.set_block(local_x as u8, y, local_z as u8, stone);
                }
                if self.grass {
                    for y in (block_height - 2)..=block_height {
                        data.set_block(local_x as u8, y, local_z as u8, snow_block);
                    }
                    data.set_block(local_x as u8, block_height + 1, local_z as u8, {
                        self.resource_manager
                            .get_block_state_id(
                                "minecraft:snow".into(),
                                Some(map! {
                                    "layers".to_string() => (
                                        (remaining_height * 7.0).ceil() + 1.0
                                    ).to_string()
                                }),
                            )
                            .await
                            .unwrap() as u16
                    });
                }
            }
        }
        data
    }
}
#[async_trait]
impl ChunkProvider for Generator {
    async fn load_chunk_data(&self, x: i32, z: i32) -> Option<Box<ChunkData>> {
        let world_folder = self.world_folder.clone();
        let chunk_data = spawn_blocking(move || {
            let chunk_file_path = world_folder.join(format!("{}-{}.chunk", x, z));
            if chunk_file_path.exists() {
                let bytes = std::fs::read(&chunk_file_path).unwrap();
                let chunk_data = Box::new(bincode::deserialize::<ChunkData>(&bytes).unwrap());
                Some(chunk_data)
            }
            else {
                None
            }
        })
        .await
        .unwrap();
        match chunk_data {
            Some(c) => Some(c),
            None => Some(self.generate_chunk_data(x, z).await),
        }
    }
    async fn save_chunk_data(&self, x: i32, z: i32, chunk_data: Box<ChunkData>) {
        let chunk_file_path = self.world_folder.join(format!("{}-{}.chunk", x, z));
        let mut chunk_file = tokio::fs::File::create(&chunk_file_path).await.unwrap();
        let bytes = bincode::serialize(chunk_data.as_ref()).unwrap();
        chunk_file.write_all(&bytes).await.unwrap();
    }
}
