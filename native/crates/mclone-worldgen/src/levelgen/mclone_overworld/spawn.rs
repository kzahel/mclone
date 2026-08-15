use mclone_core::ChunkPos;

use super::coast::McloneOverworldCoastFamily;
use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler, McloneOverworldSamplingTopology,
};
use super::surface::{McloneOverworldSurfaceRecipe, mclone_overworld_surface_recipe};

const SPAWN_SEARCH_RADIUS_CHUNKS: i32 = 256;
const SPAWN_SEARCH_STRIDE_CHUNKS: i32 = 4;
const HOMESTEAD_SCOUT_ORIGIN_SEARCH_RADIUS_CHUNKS: i32 = 128;
const SPAWN_MIN_SURFACE_Y: i32 = MCLONE_OVERWORLD_SEA_LEVEL + 5;
const SPAWN_REVIEW_OFFSETS: [(i32, i32); 5] = [(8, 8), (4, 8), (12, 8), (8, 4), (8, 12)];
const SPAWN_MIN_SUITABLE_COLUMNS: usize = 3;
const SPAWN_NEIGHBORHOOD_STRIDE_BLOCKS: i32 = 32;
const SPAWN_NEIGHBORHOOD_RADIUS: i32 = 2;
const SPAWN_MIN_SUITABLE_NEIGHBORHOOD_COLUMNS: usize = 17;

pub fn mclone_overworld_spawn_chunk(seed: i64) -> ChunkPos {
    mclone_overworld_spawn_chunk_with_topology(seed, McloneOverworldSamplingTopology::Unbounded)
}

pub fn mclone_overworld_spawn_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) -> ChunkPos {
    let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
    let mut fallback = None;
    for radius in 0..=SPAWN_SEARCH_RADIUS_CHUNKS / SPAWN_SEARCH_STRIDE_CHUNKS {
        for grid_z in -radius..=radius {
            for grid_x in -radius..=radius {
                if radius > 0 && grid_x.abs() != radius && grid_z.abs() != radius {
                    continue;
                }
                let x = grid_x * SPAWN_SEARCH_STRIDE_CHUNKS;
                let z = grid_z * SPAWN_SEARCH_STRIDE_CHUNKS;
                if !spawn_candidate_has_local_surface(sampler, x, z) {
                    continue;
                }
                let candidate = ChunkPos::new(topology.canonical_chunk_x(x), z);
                fallback.get_or_insert(candidate);
                if spawn_candidate_has_land_neighborhood(sampler, x, z) {
                    return candidate;
                }
            }
        }
    }
    fallback.unwrap_or(ChunkPos::new(0, 0))
}

/// Preserve the accepted `intro-homestead-v1` scout origin independently of
/// the Wild-start region-quality policy. The homestead planner owns its exact
/// arrival after this provisional origin is chosen.
pub fn mclone_overworld_homestead_scout_origin_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) -> ChunkPos {
    let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
    for radius in 0..=HOMESTEAD_SCOUT_ORIGIN_SEARCH_RADIUS_CHUNKS {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                if spawn_candidate_has_local_surface(sampler, x, z) {
                    return ChunkPos::new(topology.canonical_chunk_x(x), z);
                }
            }
        }
    }
    ChunkPos::new(0, 0)
}

#[cfg(test)]
fn spawn_candidate_is_suitable(
    sampler: McloneOverworldSampler,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    spawn_candidate_has_local_surface(sampler, chunk_x, chunk_z)
        && spawn_candidate_has_land_neighborhood(sampler, chunk_x, chunk_z)
}

fn spawn_candidate_has_local_surface(
    sampler: McloneOverworldSampler,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    let min_x = chunk_x * 16;
    let min_z = chunk_z * 16;
    SPAWN_REVIEW_OFFSETS
        .into_iter()
        .filter(|&(local_x, local_z)| {
            spawn_column_is_suitable(sampler, min_x + local_x, min_z + local_z)
        })
        .take(SPAWN_MIN_SUITABLE_COLUMNS)
        .count()
        == SPAWN_MIN_SUITABLE_COLUMNS
}

fn spawn_candidate_has_land_neighborhood(
    sampler: McloneOverworldSampler,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    let min_x = chunk_x * 16;
    let min_z = chunk_z * 16;
    (-SPAWN_NEIGHBORHOOD_RADIUS..=SPAWN_NEIGHBORHOOD_RADIUS)
        .flat_map(|z| (-SPAWN_NEIGHBORHOOD_RADIUS..=SPAWN_NEIGHBORHOOD_RADIUS).map(move |x| (x, z)))
        .filter(|&(x, z)| {
            spawn_column_is_suitable(
                sampler,
                min_x + 8 + x * SPAWN_NEIGHBORHOOD_STRIDE_BLOCKS,
                min_z + 8 + z * SPAWN_NEIGHBORHOOD_STRIDE_BLOCKS,
            )
        })
        .take(SPAWN_MIN_SUITABLE_NEIGHBORHOOD_COLUMNS)
        .count()
        == SPAWN_MIN_SUITABLE_NEIGHBORHOOD_COLUMNS
}

fn spawn_column_is_suitable(sampler: McloneOverworldSampler, x: i32, z: i32) -> bool {
    let sample = sampler.sample_landform(x, z);
    sample.terrain.surface_y >= SPAWN_MIN_SURFACE_Y
        && sample.terrain.coast.family == McloneOverworldCoastFamily::Inland
        && !sample.terrain.watercourse.is_water()
        && mclone_overworld_surface_recipe(sample) == McloneOverworldSurfaceRecipe::GrassSoil
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_seeds_find_a_supported_inland_spawn_chunk() {
        for topology in [
            McloneOverworldSamplingTopology::Unbounded,
            McloneOverworldSamplingTopology::PeriodicX,
        ] {
            for seed in [
                i64::MIN,
                -8_675_309,
                -98_765,
                -42,
                -1,
                0,
                1,
                42,
                12_345,
                98_765,
                8_675_309,
                i64::MAX,
            ] {
                let spawn = mclone_overworld_spawn_chunk_with_topology(seed, topology);
                assert!(
                    spawn_candidate_is_suitable(
                        McloneOverworldSampler::new_with_topology(seed, topology),
                        spawn.x,
                        spawn.z,
                    ),
                    "{topology:?} seed {seed}: {spawn:?}"
                );
            }
        }
    }

    #[test]
    fn untouched_seed_selects_a_pinned_land_dominant_region() {
        assert_eq!(mclone_overworld_spawn_chunk(0), ChunkPos::new(-24, -72));
        assert_eq!(
            mclone_overworld_spawn_chunk_with_topology(
                0,
                McloneOverworldSamplingTopology::PeriodicX,
            ),
            ChunkPos::new(356, -72)
        );
    }

    #[test]
    fn homestead_scout_origin_preserves_the_accepted_seed_zero_plan_input() {
        assert_eq!(
            mclone_overworld_homestead_scout_origin_chunk_with_topology(
                0,
                McloneOverworldSamplingTopology::Unbounded,
            ),
            ChunkPos::new(-21, -65)
        );
    }
}
