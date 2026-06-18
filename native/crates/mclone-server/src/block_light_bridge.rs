use std::collections::BTreeMap;

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, PackedLightSection, SECTION_HEIGHT, block_to_section_coord,
    chunk_block_index, local_block_coord,
};
use mclone_light::{
    BlockLightEngine, BlockLightWorld, BlockPosKey, block_pos_as_long, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, section_as_long,
};
use mclone_worldgen::block::{RawBlockId, block_light_emission, block_light_opacity};

pub(crate) fn graph_block_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = RawChunkBlockLightWorld::new(target_pos, min_y, height, chunks);
    let active_sections = world.active_sections();
    let emission_sources = world.emission_sources();
    let mut engine = BlockLightEngine::new(world);

    for section in active_sections {
        engine.activate_section(section);
    }
    for (source, emission) in emission_sources {
        engine.on_block_emission_increase(source, emission);
    }
    engine.run_all_updates();

    let min_section_y = block_to_section_coord(min_y);
    let section_count = height / SECTION_HEIGHT;
    (0..section_count)
        .filter_map(|section_offset| {
            let section_y = min_section_y + section_offset;
            let section = section_as_long(target_pos.x, section_y, target_pos.z);
            engine
                .storage()
                .get_visible_data_layer(section)
                .and_then(|layer| {
                    layer
                        .clone()
                        .into_bytes()
                        .map(|bytes| PackedLightSection::new(section_y, None, Some(bytes)))
                })
        })
        .collect()
}

#[derive(Clone, Debug)]
struct RawChunkBlockLightWorld<'a> {
    min_y: i32,
    height: i32,
    chunks: BTreeMap<ChunkPos, &'a [RawBlockId]>,
}

impl<'a> RawChunkBlockLightWorld<'a> {
    fn new(
        target_pos: ChunkPos,
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
    ) -> Self {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "block light min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "block light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        let chunks = chunks
            .into_iter()
            .map(|(pos, blocks)| {
                assert_eq!(
                    blocks.len(),
                    expected_len,
                    "chunk {pos:?} block light input had {} blocks instead of {expected_len}",
                    blocks.len()
                );
                (pos, blocks)
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            chunks.contains_key(&target_pos),
            "block light target chunk {target_pos:?} was missing from input chunks"
        );

        Self {
            min_y,
            height,
            chunks,
        }
    }

    fn active_sections(&self) -> Vec<i64> {
        let min_section_y = block_to_section_coord(self.min_y);
        let section_count = self.height / SECTION_HEIGHT;
        self.chunks
            .keys()
            .flat_map(|chunk_pos| {
                (0..section_count).map(move |section_offset| {
                    section_as_long(chunk_pos.x, min_section_y + section_offset, chunk_pos.z)
                })
            })
            .collect()
    }

    fn emission_sources(&self) -> Vec<(BlockPosKey, u8)> {
        let mut sources = Vec::new();
        for (&chunk_pos, blocks) in &self.chunks {
            for local_y in 0..self.height {
                for local_z in 0..CHUNK_WIDTH {
                    for local_x in 0..CHUNK_WIDTH {
                        let block = blocks[chunk_block_index(local_x, local_y, local_z)];
                        let emission = block_light_emission(block);
                        if emission == 0 {
                            continue;
                        }
                        sources.push((
                            block_pos_as_long(
                                chunk_pos.min_block_x() + local_x,
                                self.min_y + local_y,
                                chunk_pos.min_block_z() + local_z,
                            ),
                            emission,
                        ));
                    }
                }
            }
        }
        sources
    }

    fn block_at(&self, pos: BlockPosKey) -> Option<RawBlockId> {
        let x = block_pos_get_x(pos);
        let y = block_pos_get_y(pos);
        let z = block_pos_get_z(pos);
        if y < self.min_y || y >= self.min_y + self.height {
            return None;
        }
        let chunk_pos = ChunkPos::from_block_coords(x, z);
        let blocks = self.chunks.get(&chunk_pos)?;
        Some(blocks[chunk_block_index(local_block_coord(x), y - self.min_y, local_block_coord(z))])
    }
}

impl BlockLightWorld for RawChunkBlockLightWorld<'_> {
    fn light_emission(&self, pos: BlockPosKey) -> u8 {
        self.block_at(pos).map_or(0, block_light_emission)
    }

    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.block_at(pos).map(block_light_opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::chunk_block_index;
    use mclone_worldgen::block::{AIR, GLOW_LICHEN, LAVA, MAGMA_BLOCK, STONE};

    #[test]
    fn graph_block_light_uses_runtime_block_emission_facts() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1, 1)] = MAGMA_BLOCK;
        blocks[chunk_block_index(7, 1, 1)] = GLOW_LICHEN;
        blocks[chunk_block_index(15, 1, 1)] = LAVA;

        let sections = graph_block_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(sample_block_light(&sections, 1, 1, 1), 3);
        assert_eq!(sample_block_light(&sections, 7, 1, 1), 7);
        assert_eq!(sample_block_light(&sections, 15, 1, 1), 15);
        assert_eq!(sample_block_light(&sections, 14, 1, 1), 14);
    }

    #[test]
    fn missing_neighbor_chunk_is_opaque_for_graph_block_light() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(15, 1, 1)] = LAVA;

        let sections = graph_block_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(sample_block_light(&sections, 15, 1, 1), 15);
        assert_eq!(sample_block_light(&sections, 14, 1, 1), 14);
    }

    #[test]
    fn opaque_runtime_blocks_stop_graph_block_light() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1, 1)] = LAVA;
        for y in 0..SECTION_HEIGHT {
            for z in 0..CHUNK_WIDTH {
                blocks[chunk_block_index(2, y, z)] = STONE;
            }
        }

        let sections = graph_block_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(sample_block_light(&sections, 1, 1, 1), 15);
        assert_eq!(sample_block_light(&sections, 2, 1, 1), 0);
        assert_eq!(sample_block_light(&sections, 3, 1, 1), 0);
    }

    fn sample_block_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        let section_y = block_to_section_coord(y);
        let Some(section) = sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return 0;
        };
        let Some(data_layer) =
            mclone_light::packed_light_section_layer(section, mclone_light::LightLayer::Block)
                .unwrap()
        else {
            return 0;
        };
        data_layer.get(local_block_coord(x), y & 15, local_block_coord(z))
    }
}
