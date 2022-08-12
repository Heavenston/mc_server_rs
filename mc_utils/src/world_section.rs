use crate::{ BlockState, ChunkData, FlooringDiv };
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
        assert_eq!(world_height % 16, 0, "World height must be a multiple of 16");
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

    pub fn get_chunk(&self, x: i32, z: i32) -> Option<&ChunkData> {
        self.chunks.get(&(x, z))
    }
    pub fn get_chunk_or_default(&self, x: i32, z: i32) -> &ChunkData {
        self.chunks.get(&(x, z)).unwrap_or(self.default_chunk.as_ref().expect("No default chunk was set"))
    }

    pub fn set_block(&mut self, position: Position, block: BlockState) {
        let WorldSection { chunks, default_chunk, .. } = self;

        let (chunk_x, chunk_z) = (position.x.flooring_div(16), position.z.flooring_div(16));
        println!("{chunk_x} {chunk_z}");
        let chunk = if let Some(default_chunk) = &default_chunk {
            chunks.entry((chunk_x, chunk_z)).or_insert_with(|| default_chunk.clone())
        } else if let Some(c) = chunks.get_mut(&(chunk_x, chunk_z))
        { c } else { panic!("No default chunk was set") };

        chunk.set_block(
            position.x.rem_euclid(16).try_into().unwrap(),
            position.y.try_into().unwrap(),
            position.z.rem_euclid(16).try_into().unwrap(),
            block
        );
    }
    pub fn get_block(&self, position: Position) -> BlockState {
        let (chunk_x, chunk_z) = (position.y.flooring_div(16), position.z.flooring_div(16));

        self.chunks.get(&(chunk_x, chunk_z)).map(|c| c.get_block(
            position.x.rem_euclid(16).try_into().unwrap(),
            position.y.try_into().unwrap(),
            position.z.rem_euclid(16).try_into().unwrap(),
        )).unwrap_or(0)
    }
}

#[test]
#[should_panic(expected = "No default chunk was set")]
fn test_wrong_chunk_panic() {
    let mut wc = WorldSection::new(128);
    assert_eq!(wc.chunks.len(), 0);
    wc.set_block(Position { x: 30, y: 10, z: 230 }, 1);
}

#[test]
fn test_chunk_length() {
    let mut wc = WorldSection::new(256);
    wc.set_default_chunk(Some(ChunkData::new(256 / 16)));
    assert_eq!(wc.chunks.len(), 0);
    wc.set_block(Position { x: -12, y: 10, z: 230 }, 1);
    assert_eq!(wc.chunks.len(), 1);
    wc.set_block(Position { x: 0, y: 10, z: 230 }, 15);
    assert_eq!(wc.chunks.len(), 2);
    wc.set_block(Position { x: 0, y: 12, z: 230 }, 7);
    assert_eq!(wc.chunks.len(), 2);
    wc.set_block(Position { x: -12, y: 12, z: 231 }, 4);
    assert_eq!(wc.chunks.len(), 2);
    wc.set_block(Position { x: -12, y: 85, z: 231 }, 0);
    assert_eq!(wc.chunks.len(), 2);
    wc.set_block(Position { x: 13, y: 30, z: 331231 }, 9);
    assert_eq!(wc.chunks.len(), 3);
}
