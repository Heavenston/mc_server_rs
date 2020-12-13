use mc_ecs_server_lib::chunk_manager::ChunkLoader;
use mc_utils::ChunkData;

pub struct StoneChunkLoader;

impl ChunkLoader for StoneChunkLoader {
    fn load_chunk(&self, _x: i32, _z: i32) -> Box<ChunkData> {
        let mut chunk_data = Box::new(ChunkData::new());

        for x in 0..16 {
            for z in 0..16 {
                chunk_data.set_block(x, 20, z, 1);
            }
        }

        chunk_data
    }

    fn save_chunk(&self, x: i32, z: i32, data: &ChunkData) {}
}
