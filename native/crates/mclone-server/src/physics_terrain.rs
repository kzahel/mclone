//! Server-side conversion from loaded chunk block facts to physics terrain.
//!
//! This stays in the authoritative server layer because terrain collision for
//! dynamic physics bodies should be derived from the same live block state that
//! block ticks, interactions, and persistence mutate.

use mclone_core::{CHUNK_WIDTH, SECTION_HEIGHT, Vec3d};
use mclone_physics::{PHYSICS_TERRAIN_SECTION_WIDTH, PhysicsTerrainSection};
use mclone_worldgen::block::{RawBlockId, material_blocks_motion};
use mclone_worldgen::levelgen::MutableChunkBlockBuffer;

pub(crate) fn physics_terrain_section_from_live_blocks(
    live_blocks: &MutableChunkBlockBuffer,
    section_y: i32,
) -> Option<PhysicsTerrainSection> {
    let section_min_y = section_y * SECTION_HEIGHT;
    let section_max_y = section_min_y + SECTION_HEIGHT;
    if section_min_y < live_blocks.min_y || section_max_y > live_blocks.min_y + live_blocks.height {
        return None;
    }

    let mut section = PhysicsTerrainSection::new(
        Vec3d::new(
            (live_blocks.chunk_x * CHUNK_WIDTH) as f64,
            section_min_y as f64,
            (live_blocks.chunk_z * CHUNK_WIDTH) as f64,
        ),
        1.0,
    );

    for local_y in 0..SECTION_HEIGHT {
        let world_y = section_min_y + local_y;
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let block_id = live_blocks.get_block_at_y(local_x, world_y, local_z);
                if physics_terrain_block_is_solid(block_id) {
                    section.set_solid(local_x as usize, local_y as usize, local_z as usize, true);
                }
            }
        }
    }

    Some(section)
}

const fn physics_terrain_block_is_solid(block_id: RawBlockId) -> bool {
    material_blocks_motion(block_id)
}

const _: () = assert!(PHYSICS_TERRAIN_SECTION_WIDTH == CHUNK_WIDTH as usize);

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Aabb;
    use mclone_worldgen::block::{
        AIR, CAVE_AIR, DANDELION, DIRT, GLOW_LICHEN, GRASS, LAVA, OAK_LEAVES, POINTED_DRIPSTONE,
        SNOW, STONE, WATER,
    };

    #[test]
    fn converts_loaded_section_blocks_to_physics_terrain_cells() {
        let mut live_blocks = MutableChunkBlockBuffer::new(-1, 2, -16, 64);
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                live_blocks.set_block_at_y(local_x, 0, local_z, STONE);
            }
        }
        live_blocks.set_block_at_y(0, 1, 0, DIRT);
        live_blocks.set_block_at_y(1, 1, 0, WATER);
        live_blocks.set_block_at_y(2, 1, 0, LAVA);
        live_blocks.set_block_at_y(3, 1, 0, SNOW);
        live_blocks.set_block_at_y(4, 1, 0, GRASS);
        live_blocks.set_block_at_y(5, 1, 0, CAVE_AIR);
        live_blocks.set_block_at_y(6, 1, 0, GLOW_LICHEN);
        live_blocks.set_block_at_y(7, 1, 0, POINTED_DRIPSTONE);
        live_blocks.set_block_at_y(8, 1, 0, OAK_LEAVES);
        live_blocks.set_block_at_y(9, 1, 0, DANDELION);

        let section = physics_terrain_section_from_live_blocks(&live_blocks, 0)
            .expect("section should be inside live chunk range");

        assert_eq!(section.origin, Vec3d::new(-16.0, 0.0, 32.0));
        assert_eq!(
            section.bounds(),
            Aabb::new(-16.0, 0.0, 32.0, 0.0, 16.0, 48.0)
        );
        assert_eq!(section.solid_cell_count(), 16 * 16 + 2);
        assert!(section.is_solid(0, 0, 0));
        assert!(section.is_solid(15, 0, 15));
        assert!(section.is_solid(0, 1, 0));
        assert!(section.is_solid(8, 1, 0));
        for x in [1, 2, 3, 4, 5, 6, 7, 9] {
            assert!(!section.is_solid(x, 1, 0), "x={x} should be non-solid");
        }
    }

    #[test]
    fn returns_empty_sections_inside_loaded_chunk_range() {
        let live_blocks = MutableChunkBlockBuffer::new(0, 0, -16, 64);

        let section = physics_terrain_section_from_live_blocks(&live_blocks, -1)
            .expect("air section should still be representable");

        assert_eq!(section.origin, Vec3d::new(0.0, -16.0, 0.0));
        assert_eq!(section.solid_cell_count(), 0);
        assert!(!section.is_solid(0, 0, 0));
    }

    #[test]
    fn ignores_sections_outside_loaded_chunk_height() {
        let live_blocks = MutableChunkBlockBuffer::new(0, 0, 0, 32);

        assert!(physics_terrain_section_from_live_blocks(&live_blocks, -1).is_none());
        assert!(physics_terrain_section_from_live_blocks(&live_blocks, 0).is_some());
        assert!(physics_terrain_section_from_live_blocks(&live_blocks, 1).is_some());
        assert!(physics_terrain_section_from_live_blocks(&live_blocks, 2).is_none());
    }

    #[test]
    fn uses_existing_motion_blocking_taxonomy_for_mvp_solidity() {
        for block_id in [
            AIR,
            CAVE_AIR,
            WATER,
            LAVA,
            SNOW,
            GRASS,
            DANDELION,
            GLOW_LICHEN,
            POINTED_DRIPSTONE,
        ] {
            assert!(
                !physics_terrain_block_is_solid(block_id),
                "{block_id} should not become a full physics terrain cell"
            );
        }

        for block_id in [STONE, DIRT, OAK_LEAVES] {
            assert!(
                physics_terrain_block_is_solid(block_id),
                "{block_id} should become a full physics terrain cell"
            );
        }
    }
}
