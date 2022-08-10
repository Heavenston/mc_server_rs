use crate::{ BlockState, ChunkData };
use mc_networking::data_types::Position;

use std::convert::TryInto;
use std::collections::HashMap;

pub struct WorldSection {
    world_height: usize,

    default_chunk: Option<ChunkData>,
    chunks: HashMap<(i32, i32), ChunkData>,
}

impl WorldSection {
    pub fn new(world_height: usize) -> Self {
        Self {
            world_height,

            default_chunk: None,
            chunks: Default::default(),
        }
    }

    pub fn set_default_chunk(&mut self, data: Option<ChunkData>) {
        assert_eq!(
            data.as_ref().map(|a| a.block_height()).unwrap_or(self.world_height),
            self.world_height
        );
        self.default_chunk = data;
    }

    pub fn set_chunk(&mut self, x: i32, z: i32, data: ChunkData) {
        assert_eq!(data.block_height(), self.world_height);
        self.chunks.insert((x, z), data);
    }
    pub fn set_chunk_to_default(&mut self, x: i32, z: i32) {
        self.chunks.insert((x, z), self.default_chunk.clone().expect("No default chunk was set"));
    }

    pub fn get_chunk(&mut self, x: i32, z: i32) -> Option<&ChunkData> {
        self.chunks.get(&(x, z))
    }

    pub fn set_block(&mut self, position: Position, block: BlockState) {
        let WorldSection { chunks, default_chunk, .. } = self;

        let (chunk_x, chunk_z) = (position.y / 16, position.z / 16);
        let chunk = if let Some(default_chunk) = &default_chunk {
            chunks.entry((chunk_x, chunk_z)).or_insert_with(|| default_chunk.clone())
        } else if let Some(c) = chunks.get_mut(&(chunk_x, chunk_z))
        { c } else { return };

        chunk.set_block(
            position.x.rem_euclid(16) as u8,
            position.y.try_into().unwrap(),
            position.z.rem_euclid(16) as u8,
            block
        );
    }
    pub fn get_block(&self, position: Position) -> BlockState {
        let (chunk_x, chunk_z) = (position.y / 16, position.z / 16);

        self.chunks.get(&(chunk_x, chunk_z)).map(|c| c.get_block(
            position.x.rem_euclid(16) as u8,
            position.y.try_into().unwrap(),
            position.z.rem_euclid(16) as u8,
        )).unwrap_or(0)
    }
}
