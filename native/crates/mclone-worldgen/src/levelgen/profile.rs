use std::collections::BTreeMap;

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord, expected_chunk_biome_count};

use crate::biome::get_layered_biome_by_id;
use crate::block::{BEDROCK, DIRT, GRASS_BLOCK, SAND, STONE, WATER};
use crate::feature::{
    FeatureRegion, apply_feature_table_to_region_timed, small_island_feature_table,
};
use crate::noise::{SeedDomain, ValueNoise2d};

use super::chunk::sample_column_biome_payload;
use super::feature_batch::sorted_chunk_positions_z_major;
use super::surface_dependency_cache::{
    PreparedSurfaceDependencies, SurfaceDependencyCache, SurfaceDependencyCacheReport,
};
use super::{ChunkGenerationPlan, GeneratedChunk, MutableChunkBlockBuffer};

pub const FLAT_GRASS_MIN_Y: i32 = 0;
pub const FLAT_GRASS_HEIGHT: i32 = 256;
pub const FLAT_GRASS_SURFACE_Y: i32 = 3;
pub const PLAINS_BIOME_ID: i32 = 1;
pub const OCEAN_BIOME_ID: i32 = 0;
pub const BEACH_BIOME_ID: i32 = 16;

pub const SMALL_ISLAND_SEA_LEVEL: i32 = 63;
pub const SMALL_ISLAND_OCEAN_FLOOR_Y: i32 = 48;
pub const SMALL_ISLAND_SPAWN_SURFACE_Y: i32 = 80;
pub const SMALL_ISLAND_SPAWN_PATCH_MIN: i32 = -8;
pub const SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE: i32 = 8;
pub const SMALL_ISLAND_ENVELOPE_RADIUS: f64 = 152.0;
pub const SMALL_ISLAND_SUPPORT_RADIUS: f64 = 192.0;

const SMALL_ISLAND_HEIGHT_SPAN: f64 = 36.0;
const SMALL_ISLAND_SHORE_NOISE_SCALE: i32 = 64;
const SMALL_ISLAND_DETAIL_NOISE_SCALE: i32 = 24;
const SMALL_ISLAND_RELIEF_NOISE_SCALE: i32 = 32;
const SMALL_ISLAND_SHORE_NOISE_AMPLITUDE: f64 = 20.0;
const SMALL_ISLAND_DETAIL_NOISE_AMPLITUDE: f64 = 7.0;
const SMALL_ISLAND_RELIEF_AMPLITUDE: f64 = 5.0;
const SMALL_ISLAND_SPAWN_BLEND_DISTANCE: f64 = 20.0;
const SMALL_ISLAND_SHORE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c6f_6e65_6973);
const SMALL_ISLAND_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x736d_616c_6c2d_7631);
const SMALL_ISLAND_RELIEF_DOMAIN: SeedDomain = SeedDomain::new(0x7265_6c69_6566_7631);
const SMALL_ISLAND_DECORATION_DOMAIN: SeedDomain = SeedDomain::new(0x6465_636f_722d_7631);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SmallIslandNoise {
    shore: ValueNoise2d,
    detail: ValueNoise2d,
    relief: ValueNoise2d,
}

impl SmallIslandNoise {
    fn new(seed: i64) -> Self {
        Self {
            shore: ValueNoise2d::new(
                seed,
                SMALL_ISLAND_SHORE_DOMAIN,
                SMALL_ISLAND_SHORE_NOISE_SCALE,
            ),
            detail: ValueNoise2d::new(
                seed,
                SMALL_ISLAND_DETAIL_DOMAIN,
                SMALL_ISLAND_DETAIL_NOISE_SCALE,
            ),
            relief: ValueNoise2d::new(
                seed,
                SMALL_ISLAND_RELIEF_DOMAIN,
                SMALL_ISLAND_RELIEF_NOISE_SCALE,
            ),
        }
    }
}

/// Generate the immutable `flat-grass-v1` column stack.
///
/// This profile is intentionally seed-independent and target-only: every
/// world-coordinate chunk has the same layers and needs no neighboring terrain
/// to produce its canonical block or biome payload.
pub fn generate_flat_grass_chunk(chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            buffer.set_block_at_y(local_x, 0, local_z, BEDROCK);
            buffer.set_block_at_y(local_x, 1, local_z, DIRT);
            buffer.set_block_at_y(local_x, 2, local_z, DIRT);
            buffer.set_block_at_y(local_x, FLAT_GRASS_SURFACE_Y, local_z, GRASS_BLOCK);
        }
    }

    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        vec![PLAINS_BIOME_ID; expected_chunk_biome_count(FLAT_GRASS_HEIGHT)],
    )
}

/// Generate one undecorated Surface-stage chunk from the small-island field.
pub fn generate_small_island_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    let buffer = generate_small_island_surface_buffer(seed, chunk_x, chunk_z);
    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        small_island_chunk_biomes(seed, chunk_x, chunk_z),
    )
}

fn generate_small_island_surface_buffer(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> MutableChunkBlockBuffer {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let noise = SmallIslandNoise::new(seed);

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let world_x = min_x + local_x;
            let world_z = min_z + local_z;
            let surface_y = small_island_surface_height_with_noise(noise, world_x, world_z);
            buffer.set_block_at_y(local_x, 0, local_z, BEDROCK);

            if surface_y >= SMALL_ISLAND_SEA_LEVEL - 4 && surface_y <= SMALL_ISLAND_SEA_LEVEL + 3 {
                let sand_min_y = (surface_y - 3).max(1);
                for y in 1..sand_min_y {
                    buffer.set_block_at_y(local_x, y, local_z, STONE);
                }
                for y in sand_min_y..=surface_y {
                    buffer.set_block_at_y(local_x, y, local_z, SAND);
                }
            } else if surface_y > SMALL_ISLAND_SEA_LEVEL + 3 {
                for y in 1..surface_y - 2 {
                    buffer.set_block_at_y(local_x, y, local_z, STONE);
                }
                buffer.set_block_at_y(local_x, surface_y - 2, local_z, DIRT);
                buffer.set_block_at_y(local_x, surface_y - 1, local_z, DIRT);
                buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
            } else {
                for y in 1..=surface_y {
                    buffer.set_block_at_y(local_x, y, local_z, STONE);
                }
            }

            for y in surface_y + 1..=SMALL_ISLAND_SEA_LEVEL {
                buffer.set_block_at_y(local_x, y, local_z, WATER);
            }
        }
    }

    buffer.prime_worldgen_heightmaps();
    buffer
}

/// Generate one fully decorated small-island chunk through the same
/// dependency-bearing batch path used by scheduler workers.
pub fn generate_small_island_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let pos = ChunkPos::new(chunk_x, chunk_z);
    SmallIslandFeatureDependencyCache::new()
        .generate_features_chunks(seed, [pos])
        .chunks
        .remove(&pos)
        .unwrap_or_else(|| panic!("small-island feature batch omitted ({chunk_x}, {chunk_z})"))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SmallIslandFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmallIslandFeatureBatchResult {
    pub chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub cache_report: SmallIslandFeatureDependencyCacheReport,
}

#[derive(Debug, Default)]
pub struct SmallIslandFeatureDependencyCache {
    cache: SurfaceDependencyCache,
}

impl SmallIslandFeatureDependencyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.cache.retained_chunk_count()
    }

    pub fn resident_positions(&self) -> std::collections::BTreeSet<ChunkPos> {
        self.cache.resident_positions()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn generate_features_chunks(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> SmallIslandFeatureBatchResult {
        self.generate_features_chunks_with_dependencies(seed, targets, std::iter::empty())
    }

    pub fn generate_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> SmallIslandFeatureBatchResult {
        let plan = ChunkGenerationPlan::small_island_features(targets);
        let PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies,
            report,
        } = self.cache.prepare(seed, &plan, dependencies, |pos| {
            generate_small_island_surface_buffer(seed, pos.x, pos.z)
        });
        let cache_report = small_island_cache_report(report);

        if plan.output_chunks().is_empty() {
            return SmallIslandFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies,
                cache_report,
            };
        }

        let first_target = *plan
            .output_chunks()
            .iter()
            .next()
            .expect("non-empty small-island targets");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, region_chunks);
        let decoration_seed = SMALL_ISLAND_DECORATION_DOMAIN.derive(seed);
        let plains = get_layered_biome_by_id(PLAINS_BIOME_ID);
        for center in sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()) {
            region.set_center(center.x, center.z);
            apply_feature_table_to_region_timed(
                decoration_seed,
                plains,
                small_island_feature_table(),
                &mut region,
            );
        }

        let (targets, _, _) = plan.into_parts();
        let mut chunks = BTreeMap::new();
        for target in targets {
            let mut chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
                panic!(
                    "small-island feature region omitted target ({}, {})",
                    target.x, target.z
                )
            });
            restore_small_island_spawn_patch(&mut chunk);
            chunks.insert(
                target,
                GeneratedChunk::from_mutable_buffer_with_biomes(
                    chunk,
                    small_island_chunk_biomes(seed, target.x, target.z),
                ),
            );
        }

        SmallIslandFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report,
        }
    }
}

fn small_island_cache_report(
    report: SurfaceDependencyCacheReport,
) -> SmallIslandFeatureDependencyCacheReport {
    SmallIslandFeatureDependencyCacheReport {
        requested_dependency_chunks: report.requested_dependency_chunks,
        cache_hits: report.cache_hits,
        generated_dependency_chunks: report.generated_dependency_chunks,
        retained_dependency_chunks: report.retained_dependency_chunks,
    }
}

fn restore_small_island_spawn_patch(chunk: &mut MutableChunkBlockBuffer) {
    let min_x = chunk_min_block_coord(chunk.chunk_x);
    let min_z = chunk_min_block_coord(chunk.chunk_z);
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let world_x = min_x + local_x;
            let world_z = min_z + local_z;
            if !(SMALL_ISLAND_SPAWN_PATCH_MIN..SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE)
                .contains(&world_x)
                || !(SMALL_ISLAND_SPAWN_PATCH_MIN..SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE)
                    .contains(&world_z)
            {
                continue;
            }
            chunk.set_block_at_y(local_x, 0, local_z, BEDROCK);
            for y in 1..SMALL_ISLAND_SPAWN_SURFACE_Y - 2 {
                chunk.set_block_at_y(local_x, y, local_z, STONE);
            }
            chunk.set_block_at_y(local_x, SMALL_ISLAND_SPAWN_SURFACE_Y - 2, local_z, DIRT);
            chunk.set_block_at_y(local_x, SMALL_ISLAND_SPAWN_SURFACE_Y - 1, local_z, DIRT);
            chunk.set_block_at_y(local_x, SMALL_ISLAND_SPAWN_SURFACE_Y, local_z, GRASS_BLOCK);
            for y in SMALL_ISLAND_SPAWN_SURFACE_Y + 1..FLAT_GRASS_HEIGHT {
                chunk.set_block_at_y(local_x, y, local_z, crate::block::AIR);
            }
        }
    }
}

/// Single-valued absolute-coordinate surface field for `small-island-v1`.
pub fn small_island_surface_height(seed: i64, world_x: i32, world_z: i32) -> i32 {
    small_island_surface_height_with_noise(SmallIslandNoise::new(seed), world_x, world_z)
}

fn small_island_surface_height_with_noise(
    noise: SmallIslandNoise,
    world_x: i32,
    world_z: i32,
) -> i32 {
    let x = f64::from(world_x);
    let z = f64::from(world_z);
    let distance = (x * x + z * z).sqrt();
    if distance >= SMALL_ISLAND_SUPPORT_RADIUS {
        return SMALL_ISLAND_OCEAN_FLOOR_Y;
    }

    let shore_noise = noise.shore.sample(world_x, world_z);
    let detail_noise = noise.detail.sample(world_x, world_z);
    let distorted_distance = distance
        - shore_noise * SMALL_ISLAND_SHORE_NOISE_AMPLITUDE
        - detail_noise * SMALL_ISLAND_DETAIL_NOISE_AMPLITUDE;
    let strength = (1.0 - distorted_distance / SMALL_ISLAND_ENVELOPE_RADIUS).clamp(0.0, 1.0);
    let envelope = smoothstep(strength);
    let relief = noise.relief.sample(world_x, world_z) * SMALL_ISLAND_RELIEF_AMPLITUDE * envelope;
    let base_height =
        (f64::from(SMALL_ISLAND_OCEAN_FLOOR_Y) + SMALL_ISLAND_HEIGHT_SPAN * envelope + relief)
            .round()
            .clamp(
                f64::from(SMALL_ISLAND_OCEAN_FLOOR_Y),
                f64::from(FLAT_GRASS_HEIGHT - 1),
            );
    let outside_patch_x = ((x + 0.5).abs() - 8.0).max(0.0);
    let outside_patch_z = ((z + 0.5).abs() - 8.0).max(0.0);
    let distance_outside_patch =
        (outside_patch_x * outside_patch_x + outside_patch_z * outside_patch_z).sqrt();
    if distance_outside_patch >= SMALL_ISLAND_SPAWN_BLEND_DISTANCE {
        return base_height as i32;
    }
    let patch_weight = 1.0 - smoothstep(distance_outside_patch / SMALL_ISLAND_SPAWN_BLEND_DISTANCE);
    lerp(
        base_height,
        f64::from(SMALL_ISLAND_SPAWN_SURFACE_Y),
        patch_weight,
    )
    .round() as i32
}

pub fn small_island_biome_id(seed: i64, world_x: i32, world_z: i32) -> i32 {
    let surface_y = small_island_surface_height(seed, world_x, world_z);
    if surface_y <= SMALL_ISLAND_SEA_LEVEL - 2 {
        OCEAN_BIOME_ID
    } else if surface_y <= SMALL_ISLAND_SEA_LEVEL + 3 {
        BEACH_BIOME_ID
    } else {
        PLAINS_BIOME_ID
    }
}

fn small_island_chunk_biomes(seed: i64, chunk_x: i32, chunk_z: i32) -> Vec<i32> {
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    sample_column_biome_payload(min_x, min_z, FLAT_GRASS_HEIGHT, |world_x, world_z| {
        small_island_biome_id(seed, world_x, world_z)
    })
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(from: f64, to: f64, amount: f64) -> f64 {
    from + (to - from) * amount
}

#[cfg(test)]
mod tests {
    use crate::block::{
        AIR, BEDROCK, DANDELION, DIRT, GRASS, GRASS_BLOCK, OAK_LOG, POPPY, RawBlockId, SAND, STONE,
        WATER,
    };

    use super::*;

    #[test]
    fn flat_grass_v1_has_exact_layers_biomes_and_no_ticks() {
        let chunk = generate_flat_grass_chunk(0, 0);

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                assert_eq!(chunk.block_at_y(local_x, 0, local_z).0, BEDROCK);
                assert_eq!(chunk.block_at_y(local_x, 1, local_z).0, DIRT);
                assert_eq!(chunk.block_at_y(local_x, 2, local_z).0, DIRT);
                assert_eq!(
                    chunk.block_at_y(local_x, FLAT_GRASS_SURFACE_Y, local_z).0,
                    GRASS_BLOCK
                );
                assert_eq!(chunk.block_at_y(local_x, 4, local_z).0, AIR);
                assert_eq!(chunk.block_at_y(local_x, 255, local_z).0, AIR);
            }
        }
        assert_eq!(chunk.block_count(BEDROCK), 256);
        assert_eq!(chunk.block_count(DIRT), 512);
        assert_eq!(chunk.block_count(GRASS_BLOCK), 256);
        assert_eq!(chunk.non_air_block_count(), 1_024);
        assert_eq!(
            chunk.biomes(),
            vec![PLAINS_BIOME_ID; expected_chunk_biome_count(FLAT_GRASS_HEIGHT)]
        );
        assert!(chunk.block_ticks().is_empty());
        assert!(chunk.liquid_ticks().is_empty());
    }

    #[test]
    fn flat_grass_v1_is_coordinate_and_seed_independent_by_construction() {
        let origin = generate_flat_grass_chunk(0, 0);
        for pos in [(-1, -1), (1, 2), (30_000_000 / 16, -30_000_000 / 16)] {
            let chunk = generate_flat_grass_chunk(pos.0, pos.1);
            assert_eq!(chunk.blocks(), origin.blocks());
            assert_eq!(chunk.biomes(), origin.biomes());
        }
    }

    #[test]
    fn small_island_v1_guarantees_a_safe_sixteen_by_sixteen_spawn_patch() {
        for world_z in SMALL_ISLAND_SPAWN_PATCH_MIN..SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE {
            for world_x in SMALL_ISLAND_SPAWN_PATCH_MIN..SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE {
                assert_eq!(
                    small_island_surface_height(i64::MIN, world_x, world_z),
                    SMALL_ISLAND_SPAWN_SURFACE_Y
                );
            }
        }
        for (world_x, world_z) in [(-8_i32, -8_i32), (-8, 7), (7, -8), (7, 7), (0, 0)] {
            let source = generate_small_island_chunk(
                i64::MIN,
                world_x.div_euclid(CHUNK_WIDTH),
                world_z.div_euclid(CHUNK_WIDTH),
            );
            let local_x = world_x.rem_euclid(CHUNK_WIDTH);
            let local_z = world_z.rem_euclid(CHUNK_WIDTH);
            assert_eq!(
                source
                    .block_at_y(local_x, SMALL_ISLAND_SPAWN_SURFACE_Y, local_z)
                    .0,
                GRASS_BLOCK
            );
            assert_eq!(
                source
                    .block_at_y(local_x, SMALL_ISLAND_SPAWN_SURFACE_Y + 1, local_z)
                    .0,
                AIR
            );
        }
    }

    #[test]
    fn small_island_v1_materials_and_biomes_follow_column_classes() {
        let seed = 12_345;
        let origin = generate_small_island_chunk(seed, 0, 0);
        assert_eq!(origin.block_at_y(0, 0, 0).0, BEDROCK);
        assert_eq!(origin.block_at_y(0, 78, 0).0, DIRT);
        assert_eq!(origin.block_at_y(0, 79, 0).0, DIRT);
        assert_eq!(origin.block_at_y(0, 80, 0).0, GRASS_BLOCK);
        assert_eq!(origin.block_at_y(0, 81, 0).0, AIR);
        assert!(origin.block_ticks().is_empty());
        assert!(origin.liquid_ticks().is_empty());

        let ocean = generate_small_island_chunk(seed, 20, 20);
        assert_eq!(ocean.block_at_y(0, 0, 0).0, BEDROCK);
        assert_eq!(ocean.block_at_y(0, SMALL_ISLAND_OCEAN_FLOOR_Y, 0).0, STONE);
        assert_eq!(
            ocean.block_at_y(0, SMALL_ISLAND_OCEAN_FLOOR_Y + 1, 0).0,
            WATER
        );
        assert_eq!(ocean.block_at_y(0, SMALL_ISLAND_SEA_LEVEL, 0).0, WATER);
        assert_eq!(ocean.block_at_y(0, SMALL_ISLAND_SEA_LEVEL + 1, 0).0, AIR);
        assert!(ocean.biomes().iter().all(|biome| *biome == OCEAN_BIOME_ID));

        let mut saw_sand = false;
        let mut saw_beach = false;
        for chunk_x in -8..=8 {
            for chunk_z in -8..=8 {
                let chunk = generate_small_island_chunk(seed, chunk_x, chunk_z);
                saw_sand |= chunk.block_count(SAND) > 0;
                saw_beach |= chunk.biomes().contains(&BEACH_BIOME_ID);
            }
        }
        assert!(saw_sand);
        assert!(saw_beach);
    }

    #[test]
    fn small_island_v1_chunk_edges_sample_one_continuous_world_field() {
        let seed = -9_223_372_036_854_775;
        for (left_chunk_x, chunk_z) in [(-2, -1), (-1, 0), (0, 0), (3, -4)] {
            let left = generate_small_island_chunk(seed, left_chunk_x, chunk_z);
            let right = generate_small_island_chunk(seed, left_chunk_x + 1, chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                let world_z = chunk_min_block_coord(chunk_z) + local_z;
                let left_world_x = chunk_min_block_coord(left_chunk_x) + 15;
                let right_world_x = left_world_x + 1;
                let left_height = small_island_surface_height(seed, left_world_x, world_z);
                let right_height = small_island_surface_height(seed, right_world_x, world_z);
                assert_eq!(
                    left.block_at_y(15, left_height, local_z).0,
                    if left_height > SMALL_ISLAND_SEA_LEVEL + 3 {
                        GRASS_BLOCK
                    } else if left_height >= SMALL_ISLAND_SEA_LEVEL - 4 {
                        SAND
                    } else {
                        STONE
                    }
                );
                assert_eq!(
                    right.block_at_y(0, right_height, local_z).0,
                    if right_height > SMALL_ISLAND_SEA_LEVEL + 3 {
                        GRASS_BLOCK
                    } else if right_height >= SMALL_ISLAND_SEA_LEVEL - 4 {
                        SAND
                    } else {
                        STONE
                    }
                );
                assert!((left_height - right_height).abs() <= 4);
            }
        }

        for (chunk_x, top_chunk_z) in [(-1, -2), (0, -1), (0, 0), (-4, 3)] {
            let top = generate_small_island_chunk(seed, chunk_x, top_chunk_z);
            let bottom = generate_small_island_chunk(seed, chunk_x, top_chunk_z + 1);
            for local_x in 0..CHUNK_WIDTH {
                let world_x = chunk_min_block_coord(chunk_x) + local_x;
                let top_world_z = chunk_min_block_coord(top_chunk_z) + 15;
                let bottom_world_z = top_world_z + 1;
                let top_height = small_island_surface_height(seed, world_x, top_world_z);
                let bottom_height = small_island_surface_height(seed, world_x, bottom_world_z);
                assert_eq!(
                    top.block_at_y(local_x, top_height, 15).0,
                    expected_surface_block(top_height)
                );
                assert_eq!(
                    bottom.block_at_y(local_x, bottom_height, 0).0,
                    expected_surface_block(bottom_height)
                );
                assert!((top_height - bottom_height).abs() <= 4);
            }
        }
    }

    #[test]
    fn small_island_v1_seed_fingerprints_are_pinned_and_distinct() {
        assert_eq!(small_island_fingerprint(12_345), 14_357_595_377_438_549_354);
        assert_eq!(
            small_island_fingerprint(-98_765),
            12_105_951_125_863_982_310
        );
    }

    #[test]
    fn small_island_feature_cache_reuses_overlapping_surface_inputs() {
        let mut cache = SmallIslandFeatureDependencyCache::new();
        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        assert_eq!(first.cache_report.requested_dependency_chunks, 25);
        assert_eq!(first.cache_report.cache_hits, 0);
        assert_eq!(first.cache_report.generated_dependency_chunks, 25);

        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(1, 0)]);
        assert_eq!(second.cache_report.requested_dependency_chunks, 25);
        assert_eq!(second.cache_report.cache_hits, 20);
        assert_eq!(second.cache_report.generated_dependency_chunks, 5);
        assert_eq!(second.cache_report.retained_dependency_chunks, 25);
    }

    #[test]
    fn small_island_neighbor_features_are_partition_independent() {
        let targets = [ChunkPos::new(0, 0), ChunkPos::new(1, 0)];
        let combined = SmallIslandFeatureDependencyCache::new()
            .generate_features_chunks(12_345, targets)
            .chunks;
        let partitioned = targets
            .into_iter()
            .map(|target| {
                let chunk = generate_small_island_chunk(12_345, target.x, target.z);
                (target, chunk)
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(combined, partitioned);
    }

    #[test]
    fn small_island_feature_stage_places_real_decorations() {
        let targets = (-4..=4)
            .flat_map(|z| (-4..=4).map(move |x| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        let chunks = SmallIslandFeatureDependencyCache::new()
            .generate_features_chunks(12_345, targets)
            .chunks;
        let decoration_count = chunks
            .values()
            .map(|chunk| {
                [OAK_LOG, GRASS, DANDELION, POPPY]
                    .into_iter()
                    .map(|block| chunk.block_count(block))
                    .sum::<usize>()
            })
            .sum::<usize>();

        assert!(decoration_count > 0);
    }

    fn small_island_fingerprint(seed: i64) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for world_z in (-192_i32..=192).step_by(4) {
            for world_x in (-192_i32..=192).step_by(4) {
                for byte in world_x
                    .to_le_bytes()
                    .into_iter()
                    .chain(world_z.to_le_bytes())
                    .chain(small_island_surface_height(seed, world_x, world_z).to_le_bytes())
                    .chain(small_island_biome_id(seed, world_x, world_z).to_le_bytes())
                {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        hash
    }

    fn expected_surface_block(surface_y: i32) -> RawBlockId {
        if surface_y > SMALL_ISLAND_SEA_LEVEL + 3 {
            GRASS_BLOCK
        } else if surface_y >= SMALL_ISLAND_SEA_LEVEL - 4 {
            SAND
        } else {
            STONE
        }
    }
}
