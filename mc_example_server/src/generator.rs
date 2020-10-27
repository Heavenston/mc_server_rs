use mc_server_lib::chunk_holder::ChunkGenerator;
use mc_utils::ChunkData;

use async_trait::async_trait;
use noise::{NoiseFn, Perlin};

pub struct Generator {
    noise: Perlin,
    noise_scale: f64,
}
impl Generator {
    pub fn new() -> Self {
        Self {
            noise: Perlin::new(),
            noise_scale: 1.0 / 15.0,
        }
    }
}
#[async_trait]
impl ChunkGenerator for Generator {
    async fn generate_chunk_data(&self, chunk_x: i32, chunk_z: i32) -> Box<ChunkData> {
        let mut data = Box::new(ChunkData::new());
        for local_x in 0..16 {
            let global_x = chunk_x * 16 + local_x;
            let noise_x = global_x as f64 * self.noise_scale;
            for local_z in 0..16 {
                let global_z = chunk_z * 16 + local_z;
                let noise_z = global_z as f64 * self.noise_scale;
                let height = (50.0 + (self.noise.get([noise_x, noise_z]) * 10.0 - 5.0)) as u8;
                for y in 0..(height - 2) {
                    data.set_block(local_x as u8, y, local_z as u8, 1);
                }
                for y in (height - 2)..height {
                    data.set_block(local_x as u8, y, local_z as u8, 10);
                }
                data.set_block(local_x as u8, height, local_z as u8, 9);
            }
        }
        data
    }
}
