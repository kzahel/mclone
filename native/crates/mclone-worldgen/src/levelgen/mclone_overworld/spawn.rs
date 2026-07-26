use mclone_core::ChunkPos;

use super::coast::McloneOverworldCoastFamily;
use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler, McloneOverworldSamplingTopology,
};
use super::surface::{McloneOverworldSurfaceRecipe, mclone_overworld_surface_recipe};

const SPAWN_SEARCH_RADIUS_CHUNKS: i32 = 128;
const SPAWN_MIN_SURFACE_Y: i32 = MCLONE_OVERWORLD_SEA_LEVEL + 5;
const SPAWN_REVIEW_OFFSETS: [(i32, i32); 5] = [(8, 8), (4, 8), (12, 8), (8, 4), (8, 12)];
const SPAWN_MIN_SUITABLE_COLUMNS: usize = 3;

pub fn mclone_overworld_spawn_chunk(seed: i64) -> ChunkPos {
    mclone_overworld_spawn_chunk_with_topology(seed, McloneOverworldSamplingTopology::Unbounded)
}

pub fn mclone_overworld_spawn_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) -> ChunkPos {
    let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
    for radius in 0..=SPAWN_SEARCH_RADIUS_CHUNKS {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                if spawn_candidate_is_suitable(sampler, x, z) {
                    return ChunkPos::new(topology.canonical_chunk_x(x), z);
                }
            }
        }
    }
    ChunkPos::new(0, 0)
}

fn spawn_candidate_is_suitable(
    sampler: McloneOverworldSampler,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    SPAWN_REVIEW_OFFSETS
        .into_iter()
        .filter(|&(local_x, local_z)| {
            let sample = sampler.sample_landform(chunk_x * 16 + local_x, chunk_z * 16 + local_z);
            sample.terrain.surface_y >= SPAWN_MIN_SURFACE_Y
                && sample.terrain.coast.family == McloneOverworldCoastFamily::Inland
                && !sample.terrain.watercourse.is_water()
                && mclone_overworld_surface_recipe(sample)
                    == McloneOverworldSurfaceRecipe::GrassSoil
        })
        .take(SPAWN_MIN_SUITABLE_COLUMNS)
        .count()
        == SPAWN_MIN_SUITABLE_COLUMNS
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
            for seed in [12_345, -98_765, 8_675_309] {
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
}
