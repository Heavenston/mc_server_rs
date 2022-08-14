use crate::{ BlockState, FlooringDiv, WorldSection };
use mc_networking::packets::client_bound::{ C3DBlockChange, C3DUpdateSectionBlocks };
use mc_networking::data_types::Position;

use std::collections::HashMap;
use std::convert::TryInto;

const MINI_SECTIONS_SIDES: u16 = 8;

#[derive(Clone, Debug, Default)]
pub struct BlockChangeAccumulator {
    mini_sections: HashMap<(i32, i32, i32), [Option<BlockState>; MINI_SECTIONS_SIDES.pow(3) as _]>,
}

impl BlockChangeAccumulator {
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

    pub fn set_block(&mut self, pos: Position, block_id: BlockState) {
        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);

        let a = self.mini_sections.entry(mini_section_pos)
            .or_insert_with(|| [None; MINI_SECTIONS_SIDES.pow(3) as _]);
        a[block_index] = Some(block_id);
    }

    pub fn get_block(&self, pos: Position) -> Option<BlockState> {
        let (mini_section_pos, block_index) = Self::get_block_coordinates(pos);

        self.mini_sections.get(&mini_section_pos).and_then(|a| a[block_index])
    }

    pub fn apply_to(&self, world_section: &mut WorldSection) {
        for (mini_section_pos, blocks) in self.mini_sections.iter() {
            let (mini_offset, section_pos) = Self::mini_to_full_section(*mini_section_pos);
            let mini_block_offset = (
                mini_offset.0 * MINI_SECTIONS_SIDES as u8,
                mini_offset.1 * MINI_SECTIONS_SIDES as u8,
                mini_offset.2 * MINI_SECTIONS_SIDES as u8,
            );
            let section = world_section.get_chunk_mut(section_pos.0, section_pos.1)
                .get_section_mut(mini_section_pos.1.try_into().unwrap());
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
                w.get_chunk_or_default(section_pos.0, section_pos.1)
                 .get_section(mini_section_pos.1.try_into().unwrap())
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

#[cfg(test)]
mod tests {
    use mc_networking::packets::client_bound::{ C3DBlockChange, C3DUpdateSectionBlocks };
    use super::*;

    #[test]
    pub fn test_bca() {
        let mut bca = BlockChangeAccumulator::new();
        assert_eq!(bca.to_packets(None).count(), 0);
        assert_eq!(bca.get_block(Position { x: 120, y: 32, z: 7 }), None);
        assert_eq!(bca.get_block(Position { x: 120, y: 83, z: -332 }), None);
        assert_eq!(bca.get_block(Position { x: -438, y: 8, z: -332 }), None);
        assert_eq!(bca.get_block(Position { x: -438, y: 8, z: 912 }), None);
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

        bca.set_block(Position { x: 123, y: 30, z: 12093 }, 679);

        let packets: Vec<_> = bca.to_packets(None).collect();
        println!("{packets:#?}");
        assert_eq!(packets.len(), 2);
        assert!(packets.iter().any(|p| {
            p.section_x == 7 && p.section_y == 1 && p.section_z == 755 &&
            p.blocks == vec![C3DBlockChange {
                x: 11, y: 14, z: 13,
                block_id: 679
            }]
        }));
    }
}
