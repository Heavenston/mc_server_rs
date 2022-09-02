use crate::{ BlockState, FlooringDiv, WorldSection };
use mc_networking::packets::client_bound::{ C3DBlockChange, C3DUpdateSectionBlocks };
use mc_networking::data_types::Position;

use std::collections::hash_map::{ HashMap, Entry };
use std::convert::TryInto;

const MINI_SECTIONS_SIDES: u16 = 8;

pub trait BlockChangeMetadataTrait {
    fn is_important(&self) -> bool { false }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockChangeMetadata {
    /// Important block changes can only be overwritten by another important block change
    pub important: bool,
}
impl BlockChangeMetadataTrait for BlockChangeMetadata {
    fn is_important(&self) -> bool { self.important }
}

#[derive(Clone, Debug)]
pub struct BlockChangeAccumulator<M = BlockChangeMetadata>
    where M: BlockChangeMetadataTrait
{
    change_metadatas: HashMap<Position, M>,
    mini_sections: HashMap<(i32, i32, i32), [Option<BlockState>; MINI_SECTIONS_SIDES.pow(3) as _]>,
}

impl<M> BlockChangeAccumulator<M>
    where M: BlockChangeMetadataTrait
{
    fn get_mini_section_block_index(x: usize, y: usize, z: usize) -> usize {
        x +
        y * MINI_SECTIONS_SIDES as usize +
        z * MINI_SECTIONS_SIDES.pow(2) as usize
    }

    fn get_block_coordinates(pos: Position) -> ((i32, i32, i32), usize) {
        let mini_section_pos = (
            pos.x.flooring_div(MINI_SECTIONS_SIDES.into()),
            pos.y.flooring_div(MINI_SECTIONS_SIDES.into()),
            pos.z.flooring_div(MINI_SECTIONS_SIDES.into())
        );
        let (offset_x, offset_y, offset_z): (usize, usize, usize) = (
            pos.x.rem_euclid(MINI_SECTIONS_SIDES.into()).try_into().unwrap(),
            pos.y.rem_euclid(MINI_SECTIONS_SIDES.into()).try_into().unwrap(),
            pos.z.rem_euclid(MINI_SECTIONS_SIDES.into()).try_into().unwrap()
        );
        let block_index = Self::get_mini_section_block_index(
            offset_x, offset_y, offset_z
        );

        (mini_section_pos, block_index)
    }
    fn mini_to_full_section((x, y, z): (i32, i32, i32)) -> ((u8, u8, u8), (i32, i32, i32)) {
        const SCALE_FACTOR: i32 = (16 / MINI_SECTIONS_SIDES) as _;
        (
            (
                x.rem_euclid(SCALE_FACTOR).try_into().unwrap(),
                y.rem_euclid(SCALE_FACTOR).try_into().unwrap(),
                z.rem_euclid(SCALE_FACTOR).try_into().unwrap()
            ),
            (
                x.flooring_div(SCALE_FACTOR),
                y.flooring_div(SCALE_FACTOR),
                z.flooring_div(SCALE_FACTOR)
            )
        )
    }

    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_difference(from: &WorldSection, to: &WorldSection) -> Self {
        let bca = Self::new();

        if from.height() != to.height() {
            return bca;
        }

        for ((chunk_x, chunk_z), chunk) in from.chunks.iter() {
            if let Some(second_chunk) = to.get_chunk(*chunk_x, *chunk_z) {
                for section_index in 0..(chunk.sections_height() as u16) {
                    let first_section = chunk.get_section(section_index);
                    let second_section = second_chunk.get_section(section_index);
                    if first_section != second_section {
                        continue;
                    }

                    for dx in 0..16 {
                        for dy in 0..16 {
                            for dz in 0..16 {
                                let (a, b) = (
                                    first_section.get_block(dx, dy, dz),
                                    second_section.get_block(dx, dy, dz),
                                );
                                if a != b {
                                    bca.set_block(Position {
                                        x: chunk_x*16 + i32::from(dx),
                                        y: i32::from(section_index)*16 + i32::from(dy),
                                        z: chunk_z*16 + i32::from(dz)
                                    }, b);
                                }
                            }
                        }
                    }
                }
            }
        }

        bca
    }

    pub fn clear(&mut self) {
        self.mini_sections.clear();
    }

    pub fn filter_sections(&mut self, mut f: impl FnMut(i32, i32) -> bool) {
        self.mini_sections.retain(|&mini_pos, _| {
            let (_, pos) = Self::mini_to_full_section(mini_pos);
            f(pos.0, pos.2)
        });
    }

    pub fn set_block(&mut self, pos: Position, block_id: BlockState) {
        if self.change_metadatas.get(&pos).map(|a| a.is_important()).unwrap_or(false) 
        { return; }

        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);
        let a = self.mini_sections.entry(mini_section_pos)
            .or_insert_with(|| [None; MINI_SECTIONS_SIDES.pow(3) as _]);
        a[block_index] = Some(block_id);
    }

    pub fn set_block_with_metadata(
        &mut self, pos: Position, block_id: BlockState, metadata: M
    ) {
        match self.change_metadatas.entry(pos) {
            Entry::Occupied(o) if o.get().is_important() && !metadata.is_important() => return,
            Entry::Occupied(mut o) => { o.insert(metadata); },
            Entry::Vacant(o) => { o.insert(metadata); },
        }

        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);
        let a = self.mini_sections.entry(mini_section_pos)
            .or_insert_with(|| [None; MINI_SECTIONS_SIDES.pow(3) as _]);
        a[block_index] = Some(block_id);
    }

    pub fn get_block(&self, pos: Position) -> Option<BlockState> {
        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);

        self.mini_sections.get(&mini_section_pos).and_then(|a| a[block_index])
    }

    pub fn get_block_with_metadata(&self, pos: Position) -> Option<(BlockState, Option<&M>)> {
        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);

        self.mini_sections.get(&mini_section_pos)
            .and_then(|a| a[block_index])
            .map(|a| (a, self.change_metadatas.get(&pos)))
    }

    fn apply_to_accumulator_inner(&self, other: &mut BlockChangeAccumulator, allow_overwrites: bool) {
        for (mini_section_pos, blocks) in self.mini_sections.iter() {
            other.mini_sections.entry(*mini_section_pos)
                .and_modify(|other_blocks| {
                    (0..blocks.len())
                        .filter_map(|i| blocks[i].map(|b| (i, b)))
                        .for_each(|(i, b)| if allow_overwrites || other_blocks[i].is_none() {
                            other_blocks[i] = Some(b)
                        })
                })
                .or_insert_with(|| blocks.clone());
        }
    }
    pub fn apply_to_accumulator(&self, other: &mut BlockChangeAccumulator) {
        self.apply_to_accumulator_inner(other, false);
    }
    pub fn apply_to_accumulator_without_overwrite(&self, other: &mut BlockChangeAccumulator) {
        self.apply_to_accumulator_inner(other, true);
    }

    pub fn apply_to_world(&self, world_section: &mut WorldSection) {
        for (mini_section_pos, blocks) in self.mini_sections.iter() {
            let (mini_offset, section_pos) = Self::mini_to_full_section(*mini_section_pos);
            let mini_block_offset = (
                mini_offset.0 * MINI_SECTIONS_SIDES as u8,
                mini_offset.1 * MINI_SECTIONS_SIDES as u8,
                mini_offset.2 * MINI_SECTIONS_SIDES as u8,
            );
            let section = world_section.get_chunk_mut(section_pos.0, section_pos.2)
                .get_section_mut(section_pos.1.try_into().unwrap());
            for dz in 0..(MINI_SECTIONS_SIDES as usize) {
                for dy in 0..(MINI_SECTIONS_SIDES as usize) {
                    for dx in 0..(MINI_SECTIONS_SIDES as usize) {
                        let index = Self::get_mini_section_block_index(dx, dy, dz);
                        if let Some(b) = blocks[index] {
                            section.set_block(
                                mini_block_offset.0 + dx as u8,
                                mini_block_offset.1 + dy as u8,
                                mini_block_offset.2 + dz as u8,
                                b
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn metadatas(&self) -> impl Iterator<Item = (Position, &M)> {
        self.change_metadatas.iter().map(|(a, b)| (*a, b))
    }

    pub fn to_packets(&self, ignore_if_equal: Option<&WorldSection>)
        -> impl Iterator<Item = C3DUpdateSectionBlocks>
    {
        let mut sections = HashMap::<(i32, i32, i32), Vec<C3DBlockChange>>::new();

        for (mini_section_pos, blocks) in self.mini_sections.iter() {
            let (mini_offset, section_pos) = Self::mini_to_full_section(*mini_section_pos);
            let mini_block_offset = (
                mini_offset.0 * MINI_SECTIONS_SIDES as u8,
                mini_offset.1 * MINI_SECTIONS_SIDES as u8,
                mini_offset.2 * MINI_SECTIONS_SIDES as u8,
            );
            let block_change_list =
                sections.entry(section_pos).or_insert(vec![]);
            let original_section = ignore_if_equal.map(|w| {
                w.get_chunk_or_default(section_pos.0, section_pos.2)
                 .get_section(section_pos.1.try_into().unwrap())
            });

            for dz in 0..(MINI_SECTIONS_SIDES as usize) {
                for dy in 0..(MINI_SECTIONS_SIDES as usize) {
                    for dx in 0..(MINI_SECTIONS_SIDES as usize) {
                        let index = Self::get_mini_section_block_index(dx, dy, dz);
                        if let Some(b) = blocks[index] {
                            let (bx, by, bz) = (
                                mini_block_offset.0 + dx as u8,
                                mini_block_offset.1 + dy as u8,
                                mini_block_offset.2 + dz as u8,
                            );
                            // Try to ignore the block change
                            if original_section
                                .map(|oc| oc.get_block(bx, by, bz) != b)
                                .unwrap_or(true)
                            {
                                block_change_list.push(C3DBlockChange {
                                    x: bx, y: by, z: bz,
                                    block_id: b.into()
                                });
                            }
                        }
                    }
                }
            }
        }

        sections.into_iter().filter(|(_, a)| !a.is_empty()).map(|(section_pos, blocks)| C3DUpdateSectionBlocks {
            section_x: section_pos.0,
            section_y: section_pos.1,
            section_z: section_pos.2,
            inverted_trust_edges: false,
            blocks,
        })
    }
}

impl<M: BlockChangeMetadataTrait> Default for BlockChangeAccumulator<M> {
    fn default() -> Self {
        Self {
            change_metadatas: HashMap::new(),
            mini_sections: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ WorldSection, ChunkData, PositionExt };
    use super::*;

    use mc_networking::packets::client_bound::{ C3DBlockChange, C3DUpdateSectionBlocks };

    #[test]
    pub fn test_to_packets_ignore_if_equal() {
        let mut bca = BlockChangeAccumulator::<BlockChangeMetadata>::new();
        let mut world = WorldSection::new(256);
        world.set_default_chunk(Some(ChunkData::new(256 / 16)));

        let b1 = Position { x: -320, y: 10, z: -2 };
        let b2 = b1.add_x(17);
        let b3 = Position { x: 12, y: 120, z: 2 };
        let b4 = Position { x: -12, y: 120, z: -2 };

        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 0);

        world.set_block(b1, 2);
        bca.set_block(b1, 2);
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 1);

        bca.set_block(b1, 3);
        assert_eq!(bca.to_packets(Some(&world)).count(), 1);
        assert_eq!(bca.to_packets(None).count(), 1);

        bca.set_block(b2, 3);
        assert_eq!(bca.to_packets(Some(&world)).count(), 2);
        assert_eq!(bca.to_packets(None).count(), 2);

        world.set_block(b2, 3);
        assert_eq!(bca.to_packets(Some(&world)).count(), 1);
        assert_eq!(bca.to_packets(None).count(), 2);

        bca.set_block(b1, 2);
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 2);

        bca.set_block(b3, 3);
        bca.set_block(b4, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 2);
        assert_eq!(bca.to_packets(None).count(), 4);

        world.set_block(b4, 3);
        assert_eq!(bca.to_packets(Some(&world)).count(), 2);
        assert_eq!(bca.to_packets(None).count(), 4);

        world.set_block(b3, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 2);
        assert_eq!(bca.to_packets(None).count(), 4);

        bca.set_block(b3, 0);
        bca.set_block(b4, 0);
        assert_eq!(bca.to_packets(Some(&world)).count(), 2);
        assert_eq!(bca.to_packets(None).count(), 4);

        bca.clear();
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 0);

        world = WorldSection::new(256);
        world.set_default_chunk(Some(ChunkData::new(256 / 16)));
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 0);

        bca.set_block(b1, 0);
        bca.set_block(b1.add_y(1), 0);
        bca.set_block(b2, 0);
        bca.set_block(b3, 0);
        bca.set_block(b4, 0);
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 4);

        bca.clear();

        bca.set_block(b4, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 1);
        assert_eq!(bca.to_packets(None).count(), 1);

        world.set_block(b3, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 1);
        assert_eq!(bca.to_packets(None).count(), 1);

        bca.set_block(b3, 4);
        world.set_block(b3, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 1);
        assert_eq!(bca.to_packets(None).count(), 2);

        world.set_block(b4, 4);
        assert_eq!(bca.to_packets(Some(&world)).count(), 0);
        assert_eq!(bca.to_packets(None).count(), 2);
    }

    #[test]
    pub fn test_apply_to_world_bca() {
        let mut bca = BlockChangeAccumulator::<BlockChangeMetadata>::new();
        let mut world = WorldSection::new(256);
        let empty_chunk = ChunkData::new(256 / 16);
        
        let first_block_pos = Position { x: 120, y: 32, z: -368 };
        world.set_chunk(7, -23, empty_chunk.clone());
        world.set_block(first_block_pos, 69);
        assert_eq!(world.get_block(first_block_pos), 69);
        assert_eq!(bca.get_block(first_block_pos), None);

        bca.set_block(first_block_pos, 420);
        assert_eq!(world.get_block(first_block_pos), 69);
        assert_eq!(bca.get_block(first_block_pos), Some(420));

        bca.apply_to_world(&mut world);

        assert_eq!(world.get_block(first_block_pos), 420, "Apply did not change the correct block (or none at all)");
        assert_eq!(bca.get_block(first_block_pos), Some(420));
    }

    #[test]
    pub fn test_bca() {
        let mut bca = BlockChangeAccumulator::<BlockChangeMetadata>::new();
        assert_eq!(bca.to_packets(None).count(), 0);
        assert_eq!(bca.get_block(Position { x: 120, y: 32, z: 7 }), None);
        assert_eq!(bca.get_block(Position { x: 120, y: 83, z: -1 }), None);
        assert_eq!(bca.get_block(Position { x: -1, y: 8, z: -1 }), None);
        assert_eq!(bca.get_block(Position { x: -1, y: 8, z: 912 }), None);
        assert_eq!(bca.get_block(Position { x: 0, y: 39, z: 21303989 }), None);
        assert_eq!(bca.get_block(Position { x: 0, y: 0, z: 8231 }), None);
        assert_eq!(bca.get_block(Position { x: 0, y: 0, z: 0 }), None);

        bca.set_block(Position { x: 0, y: 0, z: 0 }, 10);

        let packets: Vec<_> = bca.to_packets(None).collect();
        assert_eq!(packets.len(), 1);

        bca.set_block(Position { x: 4, y: 2, z: 15 }, 3);

        let packets: Vec<_> = bca.to_packets(None).collect();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].section_x, 0);
        assert_eq!(packets[0].section_y, 0);
        assert_eq!(packets[0].section_z, 0);
        assert!(packets[0].blocks.iter().any(|a|
            a.x == 0 && a.y == 0 && a.z == 0 &&
            a.block_id == 10
        ));
        assert!(packets[0].blocks.iter().any(|a|
            a.x == 4 && a.y == 2 && a.z == 15 &&
            a.block_id == 3
        ));

        bca.set_block(Position { x: -1, y: 30, z: -4 }, 93);
        bca.set_block(Position { x: 123, y: 30, z: 12093 }, 679);

        let packets: Vec<_> = bca.to_packets(None).collect();
        println!("{packets:#?}");
        assert_eq!(packets.len(), 3);
        assert!(packets.iter().any(|p| {
            p.section_x == 7 && p.section_y == 1 && p.section_z == 755 &&
            p.blocks == vec![C3DBlockChange {
                x: 11, y: 14, z: 13,
                block_id: 679
            }]
        }));
    }
}
