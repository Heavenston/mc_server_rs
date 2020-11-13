use mc_networking::{
    data_types::bitbuffer::BitBuffer,
    packets::client_bound::{C20ChunkData, C20ChunkDataSection},
};

use serde::{Deserialize, Serialize};
use serde_big_array::*;

big_array! { BigArray; }

#[derive(Clone, Deserialize, Serialize)]
pub struct ChunkDataSection {
    #[serde(with = "BigArray")]
    blocks: [usize; 4096],
    palette: Vec<i32>,
}
impl ChunkDataSection {
    pub fn new() -> Self {
        Self {
            blocks: [0; 4096],
            palette: vec![0],
        }
    }

    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block: u16) {
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if let Some((pb, _)) = self
            .palette
            .iter()
            .enumerate()
            .find(|(_, b)| **b == block as i32)
        {
            self.blocks[x + (z * 16) + (y * 256)] = pb;
        }
        else {
            self.blocks[x + (z * 16) + (y * 256)] = self.palette.len();
            self.palette.push(block as i32);
        }
    }

    pub fn get_block(&self, x: u8, y: u8, z: u8) -> u16 {
        let (x, y, z) = (x as usize, y as usize, z as usize);
        self.palette[self.blocks[x + (z * 16) + (y * 256)]] as u16
    }

    fn encode(&self) -> C20ChunkDataSection {
        let mut block_count = 0;

        let bits_per_block = ((self.palette.len() as f64).log2().ceil() as u8).max(4);
        let mut blocks = BitBuffer::create(bits_per_block, 4096);
        for (i, pb) in self.blocks.iter().enumerate() {
            block_count += (*pb != 0) as i16;
            blocks.set_entry(i, *pb as u32);
        }

        C20ChunkDataSection {
            block_count,
            bits_per_block,
            palette: Some(self.palette.clone()),
            data_array: blocks.into_buffer(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ChunkData {
    sections: [Option<Box<ChunkDataSection>>; 17],
    // TODO: Make biomes mutable
    #[serde(with = "BigArray")]
    biomes: [i32; 1024],
}
impl ChunkData {
    pub fn new() -> Self {
        Self {
            sections: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None, None,
            ],
            biomes: [1; 1024],
        }
    }

    /// Get a reference to a section, returns None id it doesn't exist
    pub fn get_section(&self, y: u8) -> Option<&ChunkDataSection> {
        self.sections[y as usize].as_ref().map(|ds| ds.as_ref())
    }
    /// Get a mutable reference to a section, create the section if it doesn't exist
    pub fn get_section_mut(&mut self, y: u8) -> &mut ChunkDataSection {
        if self.sections[y as usize].is_none() {
            self.sections[y as usize] = Some(Box::new(ChunkDataSection::new()));
        }

        self.sections[y as usize]
            .as_mut()
            .map(|s| s.as_mut())
            .unwrap()
    }

    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block: u16) {
        let section = self.get_section_mut(y / 16);
        section.set_block(x, y.rem_euclid(16), z, block);
    }
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> u16 {
        self.get_section(y / 16)
            .map(|s| s.get_block(x, y.rem_euclid(16), z))
            .unwrap_or(0)
    }

    pub fn encode_full(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        full: bool,
        sections: u16,
    ) -> C20ChunkData {
        let primary_bit_mask = {
            let mut primary_bit_mask = 0;
            for section in 0..16 {
                if ((1 << section) & sections) != 0 && self.sections[section as usize].is_some() {
                    primary_bit_mask |= 1 << section;
                }
            }
            primary_bit_mask
        };
        let motion_blocking_heightmap = {
            let mut motion_blocking_heightmap = BitBuffer::create(9, 256);
            for x in 0..16 {
                for z in 0..16 {
                    'height_loop: for y in 255..=0 {
                        if self.get_block(x, y, z) != 0 {
                            motion_blocking_heightmap.set_entry(((x * 16) + z) as usize, y as u32);
                            break 'height_loop;
                        }
                    }
                }
            }
            motion_blocking_heightmap
        };
        let chunk_sections = {
            let mut chunk_sections = vec![];
            for section_y in 0..=16 {
                if self.sections[section_y as usize].is_some() {
                    chunk_sections
                        .push(self.sections[section_y as usize].as_ref().unwrap().encode());
                }
            }
            chunk_sections
        };

        C20ChunkData {
            chunk_x,
            chunk_z,
            full_chunk: full,
            biomes: if full {
                Some(self.biomes.to_vec())
            }
            else {
                None
            },
            primary_bit_mask,
            heightmaps: {
                let mut heightmaps = nbt::Blob::new();
                heightmaps
                    .insert("MOTION_BLOCKING", motion_blocking_heightmap.into_buffer())
                    .unwrap();
                heightmaps
            },
            chunk_sections,
            block_entities: vec![],
        }
    }
}
