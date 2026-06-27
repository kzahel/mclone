use std::collections::BTreeMap;

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, PackedLightSection, SECTION_HEIGHT, block_to_section_coord,
    chunk_block_index, local_block_coord,
};
use mclone_light::{
    BlockPosKey, SkyLightEngine, SkyLightWorld, block_pos_as_long, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, section_as_long,
};
use mclone_worldgen::block::{RawBlockId, block_light_opacity, is_air_like};

pub(crate) fn graph_sky_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = RawChunkSkyLightWorld::new(target_pos, min_y, height, chunks);
    let section_statuses = world.section_statuses();
    let source_blocks = world.source_blocks();
    let mut engine = SkyLightEngine::new(world);

    for (section, is_empty) in section_statuses {
        if is_empty {
            engine.activate_section(section);
        } else {
            engine.update_section_status(section, false);
        }
        engine.enable_light_sources(section, true);
    }
    for source in source_blocks {
        engine.check_sky_source(source);
    }
    engine.run_all_updates();

    let min_section_y = block_to_section_coord(min_y);
    let section_count = height / SECTION_HEIGHT;
    let mut sections = (0..section_count)
        .filter_map(|section_offset| {
            let section_y = min_section_y + section_offset;
            let section = section_as_long(target_pos.x, section_y, target_pos.z);
            engine
                .storage()
                .get_visible_data_layer(section)
                .map(|layer| {
                    PackedLightSection::new(section_y, Some(layer.to_packed_bytes()), None)
                })
        })
        .collect::<Vec<_>>();
    if sections.is_empty() && section_count > 0 {
        sections.push(PackedLightSection::new(
            min_section_y,
            Some(vec![0; mclone_light::DATA_LAYER_SIZE]),
            None,
        ));
    }
    sections
}

#[derive(Clone, Debug)]
struct RawChunkSkyLightWorld<'a> {
    min_y: i32,
    height: i32,
    chunks: BTreeMap<ChunkPos, &'a [RawBlockId]>,
}

impl<'a> RawChunkSkyLightWorld<'a> {
    fn new(
        target_pos: ChunkPos,
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
    ) -> Self {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "sky light min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "sky light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        let chunks = chunks
            .into_iter()
            .map(|(pos, blocks)| {
                assert_eq!(
                    blocks.len(),
                    expected_len,
                    "chunk {pos:?} sky light input had {} blocks instead of {expected_len}",
                    blocks.len()
                );
                (pos, blocks)
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            chunks.contains_key(&target_pos),
            "sky light target chunk {target_pos:?} was missing from input chunks"
        );

        Self {
            min_y,
            height,
            chunks,
        }
    }

    fn section_statuses(&self) -> Vec<(i64, bool)> {
        let min_section_y = block_to_section_coord(self.min_y);
        let section_count = self.height / SECTION_HEIGHT;
        self.chunks
            .iter()
            .flat_map(|(chunk_pos, blocks)| {
                (0..section_count).map(move |section_offset| {
                    (
                        section_as_long(chunk_pos.x, min_section_y + section_offset, chunk_pos.z),
                        section_is_empty(blocks, section_offset),
                    )
                })
            })
            .collect()
    }

    fn source_blocks(&self) -> Vec<BlockPosKey> {
        let top_local_y = self.height - 1;
        let mut sources = Vec::new();
        for (&chunk_pos, blocks) in &self.chunks {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let block = blocks[chunk_block_index(local_x, top_local_y, local_z)];
                    if block_light_opacity(block) < 15 {
                        sources.push(block_pos_as_long(
                            chunk_pos.min_block_x() + local_x,
                            self.min_y + top_local_y,
                            chunk_pos.min_block_z() + local_z,
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

fn section_is_empty(blocks: &[RawBlockId], section_offset: i32) -> bool {
    let base_y = section_offset * SECTION_HEIGHT;
    (0..SECTION_HEIGHT).all(|dy| {
        (0..CHUNK_WIDTH).all(|local_z| {
            (0..CHUNK_WIDTH).all(|local_x| {
                is_air_like(blocks[chunk_block_index(local_x, base_y + dy, local_z)])
            })
        })
    })
}

impl SkyLightWorld for RawChunkSkyLightWorld<'_> {
    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.block_at(pos).map(block_light_opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::chunk_block_index;
    use mclone_worldgen::block::{AIR, STONE};

    #[test]
    fn graph_sky_light_falls_down_open_column_without_decay() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];

        let sections = graph_sky_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(sample_sky_light(&sections, 1, 15, 1), 15);
        assert_eq!(sample_sky_light(&sections, 1, 1, 1), 15);
    }

    #[test]
    fn graph_sky_light_stops_at_solid_roof() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 14, 1)] = STONE;

        let sections = graph_sky_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(sample_sky_light(&sections, 1, 14, 1), 0);
        assert_eq!(sample_sky_light(&sections, 1, 1, 1), 14);
    }

    #[test]
    fn graph_sky_light_can_enter_from_neighbor_chunk() {
        let min_y = 0;
        let height = SECTION_HEIGHT * 2;
        let mut target = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        for z in 0..CHUNK_WIDTH {
            for x in 0..CHUNK_WIDTH {
                target[chunk_block_index(x, 14, z)] = STONE;
            }
        }
        let neighbor = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];

        let sections = graph_sky_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [
                (ChunkPos::new(0, 0), target.as_slice()),
                (ChunkPos::new(1, 0), neighbor.as_slice()),
            ],
        );

        assert_eq!(sample_sky_light(&sections, 15, 13, 1), 14);
    }

    fn sample_sky_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        let section_y = block_to_section_coord(y);
        let Some(section) = sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return 0;
        };
        let Some(data_layer) =
            mclone_light::packed_light_section_layer(section, mclone_light::LightLayer::Sky)
                .unwrap()
        else {
            return 0;
        };
        data_layer.get(local_block_coord(x), y & 15, local_block_coord(z))
    }
}
