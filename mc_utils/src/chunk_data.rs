use mc_networking:: data_types::{
    bitbuffer::BitBuffer,
    bitset::BitSet,
};
use mc_networking::packets::client_bound::{
    C1FChunkDataAndUpdateLight,
    C1FSection, C1FPalettedContainer
};

use std::ops::Deref;
use std::ops::DerefMut;

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

pub type BlockState = u16;

/// Because neither serde nor big array can just use a boxed slice of 4096 u16.
/// So this type is a (de)serializable array of 4096 u16.
#[derive(Clone, Deserialize, Serialize)]
pub struct ChunkArray {
    #[serde(with = "BigArray")] 
    pub array: [BlockState; 4096],
}

impl From<[BlockState; 4096]> for ChunkArray {
    fn from(array: [BlockState; 4096]) -> Self {
        Self { array }
    }
}

impl Into<[BlockState; 4096]> for ChunkArray {
    fn into(self) -> [BlockState; 4096] {
        self.array
    }
}

impl Deref for ChunkArray {
    type Target = [BlockState; 4096];
    fn deref(&self) -> &[BlockState; 4096] {
        &self.array
    }
}
impl DerefMut for ChunkArray {
    fn deref_mut(&mut self) -> &mut [BlockState; 4096] {
        &mut self.array
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum ChunkDataSection {
    Paletted {
        blocks: Box<ChunkArray>,
        palette: Vec<i32>,
    },
    Filled(BlockState),
}
impl ChunkDataSection {
    pub fn new() -> Self {
        Self::Filled(0)
    }

    pub fn to_paletted(&mut self) {
        let fill_with = if let Self::Filled(x) = self {
            *x
        } else { return };

        let mut palette = vec![0];
        let fill_with_index = if fill_with == 0 { 0 } else {
            palette.push(fill_with as i32);
            1
        };

        *self = Self::Paletted {
            blocks: Box::new([fill_with_index; 4096].into()),
            palette,
        };
    }

    pub fn fill_with(&mut self, block: BlockState) {
        *self = Self::Filled(block);
    }

    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block: BlockState) {
        match self {
            // If the chunk is filled with the correct block there is nothing to do
            Self::Filled(filled_with) if *filled_with == block
                => return,

            Self::Filled(..) => {
                self.to_paletted();
            }

            Self::Paletted { .. } => (),
        }

        match self {
            Self::Paletted { blocks, palette } => {
                let (x, y, z) = (x as usize, y as usize, z as usize);
                if let Some((pb, _)) = palette.iter()
                    .enumerate().find(|(_, b)| **b == block as i32)
                {
                    blocks[x + (z * 16) + (y * 256)] = pb as BlockState;
                }
                else {
                    blocks[x + (z * 16) + (y * 256)] = palette.len() as BlockState;
                    palette.push(block as i32);
                }
            },

            _ => unreachable!(),
        }
    }

    pub fn get_block(&self, x: u8, y: u8, z: u8) -> BlockState {
        match self {
            Self::Filled(x) => *x,
            Self::Paletted { blocks, palette } => {
                let (x, y, z) = (x as usize, y as usize, z as usize);
                palette[blocks[x + (z * 16) + (y * 256)] as usize] as BlockState
            }
        }
    }

    fn encode(&self) -> C1FSection {
        match self {
            Self::Paletted { blocks: s_blocks, palette: s_palette } => {
                let mut block_count = 0;

                let bits_per_block = ((s_palette.len() as f64).log2().ceil() as u8).max(4);
                let mut blocks = BitBuffer::create(bits_per_block, 4096);

                debug_assert!(s_palette[0] == 0, "The way block_count is calculated must be changed");
                for (i, pb) in s_blocks.iter().enumerate() {
                    if *pb != 0
                    { block_count += 1 }
                    blocks.set_entry(i, *pb as u32);
                }

                C1FSection {
                    block_count,
                    block_states: C1FPalettedContainer::Indirect {
                        bits_per_entry: bits_per_block,
                        palette: s_palette.clone(),
                        data_array: blocks.into_buffer()
                    },
                    biomes: C1FPalettedContainer::Single(0),
                }
            }

            Self::Filled(x) => {
                C1FSection {
                    block_count: if *x != 0 { 4096 } else { 0 },
                    block_states: C1FPalettedContainer::Single(*x as _),
                    biomes: C1FPalettedContainer::Single(0),
                }
            }
        }
    }
}
impl Default for ChunkDataSection {
    fn default() -> Self {
        Self::Filled(0)
    }
}


#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ChunkData {
    sections: [ChunkDataSection; 16],
}
impl ChunkData {
    pub fn new() -> Self {
        Self {
            sections: Default::default(),
        }
    }

    /// Get a reference to a section
    pub fn get_section(&self, y: u8) -> &ChunkDataSection {
        &self.sections[y as usize]
    }
    /// Get a mutable reference to a section
    pub fn get_section_mut(&mut self, y: u8) -> &mut ChunkDataSection {
        &mut self.sections[y as usize]
    }

    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block: BlockState) {
        let section = self.get_section_mut(y / 16);
        section.set_block(x, y.rem_euclid(16), z, block);
    }
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> BlockState {
        self.get_section(y / 16)
            .get_block(x, y.rem_euclid(16), z)
    }

    pub fn encode_full(
        &self, chunk_x: i32, chunk_z: i32
    ) -> C1FChunkDataAndUpdateLight {
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
        let chunk_sections = self.sections.iter().map(|s| s.encode()).collect();

        C1FChunkDataAndUpdateLight {
            chunk_x,
            chunk_z,
            heightmaps: {
                let mut heightmaps = nbt::Blob::new();
                heightmaps
                    .insert("MOTION_BLOCKING", motion_blocking_heightmap.into_buffer())
                    .unwrap();
                heightmaps
            },
            chunk_sections,
            block_entities: vec![],
            trust_edges: true,
            sky_light_mask: BitSet::new(),
            block_light_mask: BitSet::new(),
            empty_sky_light_mask: BitSet::new(),
            empty_block_light_mask: BitSet::new(),
            sky_light_array: vec![],
            block_light_array: vec![],
        }
    }
}
