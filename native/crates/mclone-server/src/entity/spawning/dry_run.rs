use std::collections::BTreeSet;

use mclone_core::{BlockPos, ChunkPos};
use mclone_worldgen::biome::BiomeDefinition;
use mclone_worldgen::block::{RawBlockId, has_fluid, is_leaves, material_blocks_motion};

use crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT;

use super::biome_tables::farm_animal_spawns_for_biome;
use super::placements::{SpawnPlacementFailure, check_farm_animal_natural_spawn};

const JAVA_OVERWORLD_MIN_BUILD_HEIGHT: i32 = 0;
const NATURAL_SPAWN_DRY_RUN_MAX_CHUNKS: usize = 8;
const NATURAL_SPAWN_DRY_RUN_SAMPLE_COLUMNS: [(i32, i32); 4] = [(3, 3), (12, 3), (3, 12), (12, 12)];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NaturalSpawnDryRunDiagnostics {
    pub(crate) chunks_checked: usize,
    pub(crate) chunk_budget_exhausted: bool,
    pub(crate) positions_checked: usize,
    pub(crate) biome_supported_positions: usize,
    pub(crate) implemented_entries_checked: usize,
    pub(crate) valid_candidates: usize,
    pub(crate) blocked_by_biome: usize,
    pub(crate) blocked_missing_block_data: usize,
    pub(crate) blocked_missing_brightness: usize,
    pub(crate) blocked_invalid_floor: usize,
    pub(crate) blocked_space: usize,
    pub(crate) blocked_collision: usize,
    pub(crate) blocked_too_dark: usize,
    pub(crate) blocked_unsupported: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceProbeFailure {
    MissingBlockData,
    NoSurface,
}

pub(crate) fn dry_run_creature_spawn_eligibility<F, B, L>(
    eligible_chunks: &BTreeSet<ChunkPos>,
    mut block_at: F,
    mut biome_at: B,
    mut raw_brightness_at: L,
) -> NaturalSpawnDryRunDiagnostics
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
    B: FnMut(i32, i32) -> BiomeDefinition,
    L: FnMut(BlockPos) -> Option<u8>,
{
    let mut diagnostics = NaturalSpawnDryRunDiagnostics {
        chunk_budget_exhausted: eligible_chunks.len() > NATURAL_SPAWN_DRY_RUN_MAX_CHUNKS,
        ..NaturalSpawnDryRunDiagnostics::default()
    };

    for chunk in eligible_chunks
        .iter()
        .take(NATURAL_SPAWN_DRY_RUN_MAX_CHUNKS)
    {
        diagnostics.chunks_checked += 1;

        for (dx, dz) in NATURAL_SPAWN_DRY_RUN_SAMPLE_COLUMNS {
            let x = chunk.min_block_x() + dx;
            let z = chunk.min_block_z() + dz;
            let spawn_entries = farm_animal_spawns_for_biome(biome_at(x, z));
            if spawn_entries.is_empty() {
                diagnostics.blocked_by_biome += 1;
                continue;
            }
            diagnostics.biome_supported_positions += 1;

            let feet_y = match top_motion_blocking_no_leaves_feet_y(x, z, &mut block_at) {
                Ok(feet_y) => feet_y,
                Err(SurfaceProbeFailure::MissingBlockData) => {
                    diagnostics.blocked_missing_block_data += 1;
                    continue;
                }
                Err(SurfaceProbeFailure::NoSurface) => {
                    diagnostics.blocked_invalid_floor += 1;
                    continue;
                }
            };
            let pos = BlockPos::new(x, feet_y, z);
            diagnostics.positions_checked += 1;

            for entry in spawn_entries
                .iter()
                .copied()
                .filter(|entry| entry.is_implemented())
            {
                let Some(kind) = entry.entity.implemented_kind() else {
                    continue;
                };
                diagnostics.implemented_entries_checked += 1;
                match check_farm_animal_natural_spawn(
                    kind,
                    pos,
                    |block_pos| block_at(block_pos),
                    |brightness_pos| raw_brightness_at(brightness_pos),
                ) {
                    Ok(()) => diagnostics.valid_candidates += 1,
                    Err(failure) => diagnostics.count_failure(failure),
                }
            }
        }
    }

    diagnostics
}

impl NaturalSpawnDryRunDiagnostics {
    fn count_failure(&mut self, failure: SpawnPlacementFailure) {
        match failure {
            SpawnPlacementFailure::MissingBlockData => self.blocked_missing_block_data += 1,
            SpawnPlacementFailure::MissingBrightness => self.blocked_missing_brightness += 1,
            SpawnPlacementFailure::InvalidFloor | SpawnPlacementFailure::NotGrassBlock => {
                self.blocked_invalid_floor += 1;
            }
            SpawnPlacementFailure::BlockedFeet | SpawnPlacementFailure::BlockedHead => {
                self.blocked_space += 1;
            }
            SpawnPlacementFailure::CollisionBlocked => self.blocked_collision += 1,
            SpawnPlacementFailure::TooDark => self.blocked_too_dark += 1,
            SpawnPlacementFailure::UnsupportedEntity
            | SpawnPlacementFailure::UnsupportedPlacement => {
                self.blocked_unsupported += 1;
            }
        }
    }
}

fn top_motion_blocking_no_leaves_feet_y(
    x: i32,
    z: i32,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<i32, SurfaceProbeFailure> {
    for y in (JAVA_OVERWORLD_MIN_BUILD_HEIGHT..JAVA_OVERWORLD_MAX_BUILD_HEIGHT).rev() {
        let block =
            block_at(BlockPos::new(x, y, z)).ok_or(SurfaceProbeFailure::MissingBlockData)?;
        if is_motion_blocking_no_leaves_heightmap_block(block) {
            let feet_y = y + 1;
            if feet_y >= JAVA_OVERWORLD_MAX_BUILD_HEIGHT {
                return Err(SurfaceProbeFailure::NoSurface);
            }
            return Ok(feet_y);
        }
    }

    Err(SurfaceProbeFailure::NoSurface)
}

fn is_motion_blocking_no_leaves_heightmap_block(block: RawBlockId) -> bool {
    !is_leaves(block) && (material_blocks_motion(block) || has_fluid(block))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::biome::get_layered_biome_by_id;
    use mclone_worldgen::block::{AIR, GRASS_BLOCK};

    use super::*;

    fn block_map_at(blocks: &BTreeMap<BlockPos, RawBlockId>, pos: BlockPos) -> Option<RawBlockId> {
        Some(*blocks.get(&pos).unwrap_or(&AIR))
    }

    fn sampled_grass_surface_blocks(chunk: ChunkPos, y: i32) -> BTreeMap<BlockPos, RawBlockId> {
        NATURAL_SPAWN_DRY_RUN_SAMPLE_COLUMNS
            .into_iter()
            .map(|(dx, dz)| {
                (
                    BlockPos::new(chunk.min_block_x() + dx, y, chunk.min_block_z() + dz),
                    GRASS_BLOCK,
                )
            })
            .collect()
    }

    #[test]
    fn dry_run_reports_missing_brightness_without_counting_valid_candidates() {
        let chunk = ChunkPos::new(0, 0);
        let chunks = BTreeSet::from([chunk]);
        let blocks = sampled_grass_surface_blocks(chunk, 63);
        let plains = get_layered_biome_by_id(1);

        let diagnostics = dry_run_creature_spawn_eligibility(
            &chunks,
            |pos| block_map_at(&blocks, pos),
            |_, _| plains,
            |_| None,
        );

        assert_eq!(diagnostics.chunks_checked, 1);
        assert!(!diagnostics.chunk_budget_exhausted);
        assert_eq!(diagnostics.biome_supported_positions, 4);
        assert_eq!(diagnostics.positions_checked, 4);
        assert_eq!(diagnostics.implemented_entries_checked, 8);
        assert_eq!(diagnostics.valid_candidates, 0);
        assert_eq!(diagnostics.blocked_missing_brightness, 8);
    }

    #[test]
    fn dry_run_counts_valid_candidates_when_world_predicates_pass() {
        let chunk = ChunkPos::new(0, 0);
        let chunks = BTreeSet::from([chunk]);
        let blocks = sampled_grass_surface_blocks(chunk, 63);
        let plains = get_layered_biome_by_id(1);

        let diagnostics = dry_run_creature_spawn_eligibility(
            &chunks,
            |pos| block_map_at(&blocks, pos),
            |_, _| plains,
            |_| Some(15),
        );

        assert_eq!(diagnostics.positions_checked, 4);
        assert_eq!(diagnostics.implemented_entries_checked, 8);
        assert_eq!(diagnostics.valid_candidates, 8);
        assert_eq!(diagnostics.blocked_missing_brightness, 0);
    }

    #[test]
    fn dry_run_skips_biomes_without_farm_animal_spawn_tables() {
        let chunk = ChunkPos::new(0, 0);
        let chunks = BTreeSet::from([chunk]);
        let blocks = sampled_grass_surface_blocks(chunk, 63);
        let desert = get_layered_biome_by_id(2);

        let diagnostics = dry_run_creature_spawn_eligibility(
            &chunks,
            |pos| block_map_at(&blocks, pos),
            |_, _| desert,
            |_| Some(15),
        );

        assert_eq!(diagnostics.blocked_by_biome, 4);
        assert_eq!(diagnostics.positions_checked, 0);
        assert_eq!(diagnostics.implemented_entries_checked, 0);
    }

    #[test]
    fn dry_run_bounds_chunk_work() {
        let chunks = (-4..=4).map(|x| ChunkPos::new(x, 0)).collect();

        let diagnostics = dry_run_creature_spawn_eligibility(
            &chunks,
            |_| None,
            |_, _| get_layered_biome_by_id(1),
            |_| Some(15),
        );

        assert_eq!(diagnostics.chunks_checked, NATURAL_SPAWN_DRY_RUN_MAX_CHUNKS);
        assert!(diagnostics.chunk_budget_exhausted);
    }
}
