use mc_networking::packets::client_bound::C20ChunkData;
use mc_utils::ChunkData;

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub data: Box<ChunkData>,
}
impl Chunk {
    pub fn new(x: i32, z: i32, data: Box<ChunkData>) -> Self {
        Self { x, z, data }
    }

    pub fn encode(&self) -> C20ChunkData {
        self.data.encode_full(self.x, self.z, true, !0)
    }
    pub fn encode_partial(&self, sections: u16) -> C20ChunkData {
        self.data.encode_full(self.x, self.z, false, sections)
    }
}
