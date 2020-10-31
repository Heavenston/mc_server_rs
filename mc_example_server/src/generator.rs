use mc_networking::map;
use mc_server_lib::{chunk_holder::ChunkGenerator, resource_manager::ResourceManager};
use mc_utils::ChunkData;

use async_trait::async_trait;
use noise::{NoiseFn, Perlin};
use std::sync::Arc;

pub struct Generator {
    grass: bool,
    noise: Perlin,
    noise_scale: f64,
    resource_manager: Arc<ResourceManager>,
}
impl Generator {
    pub fn new(grass: bool, resource_manager: Arc<ResourceManager>) -> Self {
        Self {
            grass,
            noise: Perlin::new(),
            noise_scale: 1.0 / 40.0,
            resource_manager,
        }
    }
}
#[async_trait]
impl ChunkGenerator for Generator {
    async fn generate_chunk_data(&self, chunk_x: i32, chunk_z: i32) -> Box<ChunkData> {
        let stone = self
            .resource_manager
            .get_block_id("minecraft:stone".into(), None)
            .await
            .unwrap() as u16;
        let snow_block = self
            .resource_manager
            .get_block_id("minecraft:snow_block".into(), None)
            .await
            .unwrap() as u16;

        let mut data = Box::new(ChunkData::new());
        for local_x in 0..16 {
            let global_x = chunk_x * 16 + local_x;
            let noise_x = global_x as f64 * self.noise_scale;
            for local_z in 0..16 {
                let global_z = chunk_z * 16 + local_z;
                let noise_z = global_z as f64 * self.noise_scale;
                let target_height = 50.0 + (self.noise.get([noise_x, noise_z]) * 10.0 - 5.0);
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
                            .get_block_id(
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
