use mc_networking::map;
use mc_server_lib::{chunk_holder::ChunkGenerator, resource_manager::ResourceManager};
use mc_utils::ChunkData;

use async_trait::async_trait;
use noise::{NoiseFn, Perlin, Seedable};
use std::sync::Arc;

pub struct Generator {
    grass: bool,
    snow_noise: Perlin,
    snow_noise_scale: f64,
    noise: Perlin,
    noise_scale: f64,
    resource_manager: Arc<ResourceManager>,
}
impl Generator {
    pub fn new(grass: bool, resource_manager: Arc<ResourceManager>) -> Self {
        Self {
            grass,
            snow_noise: Perlin::new().set_seed(3283720),
            snow_noise_scale: 2.25,
            noise: Perlin::new(),
            noise_scale: 1.0 / 15.0,
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
        let dirt = self
            .resource_manager
            .get_block_id("minecraft:dirt".into(), None)
            .await
            .unwrap() as u16;
        let grass = self
            .resource_manager
            .get_block_id(
                "minecraft:grass_block".into(),
                Some(map! {
                    "snowy".to_string() => "true".to_string()
                }),
            )
            .await
            .unwrap() as u16;

        let mut data = Box::new(ChunkData::new());
        for local_x in 0..16 {
            let global_x = chunk_x * 16 + local_x;
            let noise_x = global_x as f64 * self.noise_scale;
            let snow_noise_x = global_x as f64 * self.snow_noise_scale;
            for local_z in 0..16 {
                let global_z = chunk_z * 16 + local_z;
                let noise_z = global_z as f64 * self.noise_scale;
                let snow_noise_z = global_z as f64 * self.snow_noise_scale;
                let height = (50.0 + (self.noise.get([noise_x, noise_z]) * 10.0 - 5.0)) as u8;
                for y in 0..(height - 2) {
                    data.set_block(local_x as u8, y, local_z as u8, stone);
                }
                if self.grass {
                    for y in (height - 2)..height {
                        data.set_block(local_x as u8, y, local_z as u8, dirt);
                    }
                    data.set_block(local_x as u8, height, local_z as u8, grass);
                    data.set_block(local_x as u8, height+1, local_z as u8, {
                        self.resource_manager
                            .get_block_id("minecraft:snow".into(), Some(map!{
                                "layers".to_string() => (
                                    ((self.snow_noise.get([snow_noise_x, snow_noise_z]) / 2.0 + 0.5) * 7.0 + 1.0).floor()
                                ).to_string()
                            }))
                            .await.unwrap() as u16
                    });
                }
            }
        }
        data
    }
}
