use mc_utils::ChunkData;
use mc_networking::packets::client_bound::C20ChunkData;

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub data: ChunkData,
}
impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x, z,
            data: ChunkData::new()
        }
    }

    pub fn encode(&self) -> C20ChunkData {
        self.data.encode(self.x, self.z)
    }
}