use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord, expected_chunk_biome_count};

use crate::block::{
    AIR, BEDROCK, BROWN_MUSHROOM, CACTUS, CLAY, COAL_ORE, DANDELION, DIAMOND_ORE, DIRT, GOLD_ORE,
    GRASS_BLOCK, GRAVEL, ICE, IRON_ORE, LAVA, MOSSY_COBBLESTONE, POPPY, RED_MUSHROOM, REDSTONE_ORE,
    RawBlockId, SAND, SNOW, STONE, SUGAR_CANE, WATER,
};
use crate::feature::{BasicTreeConfiguration, ConfiguredFeature, FeatureRegion, FeatureWorld};
use crate::placement::BlockPos;
use crate::prng::SimpleRandomSource;

use super::feature_batch::sorted_chunk_positions_z_major;
use super::surface_dependency_cache::{
    PreparedSurfaceDependencies, SurfaceDependencyCache, SurfaceDependencyCacheReport,
};
use super::{ChunkGenerationPlan, GeneratedChunk, MutableChunkBlockBuffer};

pub const ALPHA_BUILD_HEIGHT: i32 = 256;
pub const ALPHA_ACTIVE_HEIGHT: i32 = 128;
pub const ALPHA_SEA_LEVEL: i32 = 64;
pub const ALPHA_PLAINS_BIOME_ID: i32 = 1;

const ALPHA_DENSITY_HORIZONTAL_CELLS: usize = 4;
const ALPHA_DENSITY_VERTICAL_CELLS: usize = 16;
const ALPHA_DENSITY_SIZE_XZ: usize = ALPHA_DENSITY_HORIZONTAL_CELLS + 1;
const ALPHA_DENSITY_SIZE_Y: usize = ALPHA_DENSITY_VERTICAL_CELLS + 1;
const ALPHA_BASE_CHUNK_MULTIPLIER_X: i64 = 341_873_128_712;
const ALPHA_BASE_CHUNK_MULTIPLIER_Z: i64 = 132_897_987_541;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaGenerationStage {
    Terrain,
    Surface,
    Caves,
    Features,
}

#[derive(Clone, Debug)]
struct AlphaOctaveNoise {
    levels: Vec<AlphaImprovedNoise>,
}

impl AlphaOctaveNoise {
    fn new(random: &mut SimpleRandomSource, levels: usize) -> Self {
        Self {
            levels: (0..levels)
                .map(|_| AlphaImprovedNoise::new(random))
                .collect(),
        }
    }

    fn value_2d(&self, x: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut octave_scale = 1.0;
        for level in &self.levels {
            value += level.noise(x * octave_scale, z * octave_scale, 0.0) / octave_scale;
            octave_scale /= 2.0;
        }
        value
    }

    #[allow(clippy::too_many_arguments)]
    fn region(
        &self,
        x: f64,
        y: f64,
        z: f64,
        size_x: usize,
        size_y: usize,
        size_z: usize,
        scale_x: f64,
        scale_y: f64,
        scale_z: f64,
    ) -> Vec<f64> {
        let mut values = vec![0.0; size_x * size_y * size_z];
        let mut octave_scale = 1.0;
        for level in &self.levels {
            level.add(
                &mut values,
                x,
                y,
                z,
                size_x,
                size_y,
                size_z,
                scale_x * octave_scale,
                scale_y * octave_scale,
                scale_z * octave_scale,
                octave_scale,
            );
            octave_scale /= 2.0;
        }
        values
    }
}

#[derive(Clone, Debug)]
struct AlphaImprovedNoise {
    permutations: [usize; 512],
    offset_x: f64,
    offset_y: f64,
    offset_z: f64,
}

impl AlphaImprovedNoise {
    fn new(random: &mut SimpleRandomSource) -> Self {
        let offset_x = random.next_double() * 256.0;
        let offset_y = random.next_double() * 256.0;
        let offset_z = random.next_double() * 256.0;
        let mut permutations = [0; 512];
        for (index, value) in permutations[..256].iter_mut().enumerate() {
            *value = index;
        }
        for index in 0..256 {
            let other = index + random.next_int_bound((256 - index) as i32) as usize;
            permutations.swap(index, other);
            permutations[index + 256] = permutations[index];
        }
        Self {
            permutations,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut delta_x = x + self.offset_x;
        let mut delta_y = y + self.offset_y;
        let mut delta_z = z + self.offset_z;
        let grid_x = alpha_floor(delta_x);
        let grid_y = alpha_floor(delta_y);
        let grid_z = alpha_floor(delta_z);
        let perm_x = (grid_x & 0xff) as usize;
        let perm_y = (grid_y & 0xff) as usize;
        let perm_z = (grid_z & 0xff) as usize;
        delta_x -= f64::from(grid_x);
        delta_y -= f64::from(grid_y);
        delta_z -= f64::from(grid_z);
        let fade_x = alpha_fade(delta_x);
        let fade_y = alpha_fade(delta_y);
        let fade_z = alpha_fade(delta_z);
        let x0 = self.permutations[perm_x] + perm_y;
        let x0_y0 = self.permutations[x0] + perm_z;
        let x0_y1 = self.permutations[x0 + 1] + perm_z;
        let x1 = self.permutations[perm_x + 1] + perm_y;
        let x1_y0 = self.permutations[x1] + perm_z;
        let x1_y1 = self.permutations[x1 + 1] + perm_z;
        alpha_lerp(
            fade_z,
            alpha_lerp(
                fade_y,
                alpha_lerp(
                    fade_x,
                    alpha_gradient(self.permutations[x0_y0], delta_x, delta_y, delta_z),
                    alpha_gradient(self.permutations[x1_y0], delta_x - 1.0, delta_y, delta_z),
                ),
                alpha_lerp(
                    fade_x,
                    alpha_gradient(self.permutations[x0_y1], delta_x, delta_y - 1.0, delta_z),
                    alpha_gradient(
                        self.permutations[x1_y1],
                        delta_x - 1.0,
                        delta_y - 1.0,
                        delta_z,
                    ),
                ),
            ),
            alpha_lerp(
                fade_y,
                alpha_lerp(
                    fade_x,
                    alpha_gradient(
                        self.permutations[x0_y0 + 1],
                        delta_x,
                        delta_y,
                        delta_z - 1.0,
                    ),
                    alpha_gradient(
                        self.permutations[x1_y0 + 1],
                        delta_x - 1.0,
                        delta_y,
                        delta_z - 1.0,
                    ),
                ),
                alpha_lerp(
                    fade_x,
                    alpha_gradient(
                        self.permutations[x0_y1 + 1],
                        delta_x,
                        delta_y - 1.0,
                        delta_z - 1.0,
                    ),
                    alpha_gradient(
                        self.permutations[x1_y1 + 1],
                        delta_x - 1.0,
                        delta_y - 1.0,
                        delta_z - 1.0,
                    ),
                ),
            ),
        )
    }

    #[allow(clippy::too_many_arguments, unused_assignments)]
    fn add(
        &self,
        values: &mut [f64],
        x: f64,
        y: f64,
        z: f64,
        size_x: usize,
        size_y: usize,
        size_z: usize,
        scale_x: f64,
        scale_y: f64,
        scale_z: f64,
        noise_scale: f64,
    ) {
        let mut index = 0;
        let amplitude = 1.0 / noise_scale;
        let mut previous_grid_y = -1;
        let mut x0_y0 = 0;
        let mut x0_y1 = 0;
        let mut x1_y0 = 0;
        let mut x1_y1 = 0;
        let mut lower_x0 = 0.0;
        let mut lower_x1 = 0.0;
        let mut upper_x0 = 0.0;
        let mut upper_x1 = 0.0;

        for local_x in 0..size_x {
            let mut delta_x = (x + local_x as f64) * scale_x + self.offset_x;
            let grid_x = alpha_floor(delta_x);
            let perm_x = (grid_x & 0xff) as usize;
            delta_x -= f64::from(grid_x);
            let fade_x = alpha_fade(delta_x);
            for local_z in 0..size_z {
                let mut delta_z = (z + local_z as f64) * scale_z + self.offset_z;
                let grid_z = alpha_floor(delta_z);
                let perm_z = (grid_z & 0xff) as usize;
                delta_z -= f64::from(grid_z);
                let fade_z = alpha_fade(delta_z);
                for local_y in 0..size_y {
                    let mut delta_y = (y + local_y as f64) * scale_y + self.offset_y;
                    let grid_y = alpha_floor(delta_y);
                    let perm_y = (grid_y & 0xff) as usize;
                    delta_y -= f64::from(grid_y);
                    let fade_y = alpha_fade(delta_y);
                    if local_y == 0 || perm_y as i32 != previous_grid_y {
                        previous_grid_y = perm_y as i32;
                        let p_x0 = self.permutations[perm_x] + perm_y;
                        x0_y0 = self.permutations[p_x0] + perm_z;
                        x0_y1 = self.permutations[p_x0 + 1] + perm_z;
                        let p_x1 = self.permutations[perm_x + 1] + perm_y;
                        x1_y0 = self.permutations[p_x1] + perm_z;
                        x1_y1 = self.permutations[p_x1 + 1] + perm_z;
                        lower_x0 = alpha_lerp(
                            fade_x,
                            alpha_gradient(self.permutations[x0_y0], delta_x, delta_y, delta_z),
                            alpha_gradient(
                                self.permutations[x1_y0],
                                delta_x - 1.0,
                                delta_y,
                                delta_z,
                            ),
                        );
                        lower_x1 = alpha_lerp(
                            fade_x,
                            alpha_gradient(
                                self.permutations[x0_y1],
                                delta_x,
                                delta_y - 1.0,
                                delta_z,
                            ),
                            alpha_gradient(
                                self.permutations[x1_y1],
                                delta_x - 1.0,
                                delta_y - 1.0,
                                delta_z,
                            ),
                        );
                        upper_x0 = alpha_lerp(
                            fade_x,
                            alpha_gradient(
                                self.permutations[x0_y0 + 1],
                                delta_x,
                                delta_y,
                                delta_z - 1.0,
                            ),
                            alpha_gradient(
                                self.permutations[x1_y0 + 1],
                                delta_x - 1.0,
                                delta_y,
                                delta_z - 1.0,
                            ),
                        );
                        upper_x1 = alpha_lerp(
                            fade_x,
                            alpha_gradient(
                                self.permutations[x0_y1 + 1],
                                delta_x,
                                delta_y - 1.0,
                                delta_z - 1.0,
                            ),
                            alpha_gradient(
                                self.permutations[x1_y1 + 1],
                                delta_x - 1.0,
                                delta_y - 1.0,
                                delta_z - 1.0,
                            ),
                        );
                    }
                    let lower = alpha_lerp(fade_y, lower_x0, lower_x1);
                    let upper = alpha_lerp(fade_y, upper_x0, upper_x1);
                    values[index] += alpha_lerp(fade_z, lower, upper) * amplitude;
                    index += 1;
                }
            }
        }
    }
}

fn alpha_fade(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn alpha_lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn alpha_gradient(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    let masked = hash & 15;
    let first = if masked < 8 { x } else { y };
    let second = if masked < 4 {
        y
    } else if masked != 12 && masked != 14 {
        z
    } else {
        x
    };
    (if masked & 1 == 0 { first } else { -first }) + if masked & 2 == 0 { second } else { -second }
}

#[derive(Clone, Debug)]
struct AlphaNoiseBanks {
    min_limit: AlphaOctaveNoise,
    max_limit: AlphaOctaveNoise,
    selector: AlphaOctaveNoise,
    surface_mask: AlphaOctaveNoise,
    surface_depth: AlphaOctaveNoise,
    scale: AlphaOctaveNoise,
    depth: AlphaOctaveNoise,
    #[allow(dead_code)]
    forest: AlphaOctaveNoise,
}

impl AlphaNoiseBanks {
    fn new(seed: i64) -> Self {
        let mut random = SimpleRandomSource::new(seed);
        Self {
            min_limit: AlphaOctaveNoise::new(&mut random, 16),
            max_limit: AlphaOctaveNoise::new(&mut random, 16),
            selector: AlphaOctaveNoise::new(&mut random, 8),
            surface_mask: AlphaOctaveNoise::new(&mut random, 4),
            surface_depth: AlphaOctaveNoise::new(&mut random, 4),
            scale: AlphaOctaveNoise::new(&mut random, 10),
            depth: AlphaOctaveNoise::new(&mut random, 16),
            forest: AlphaOctaveNoise::new(&mut random, 8),
        }
    }

    fn density_lattice(&self, chunk_x: i32, chunk_z: i32) -> Vec<f64> {
        let start_x = f64::from(chunk_x) * ALPHA_DENSITY_HORIZONTAL_CELLS as f64;
        let start_z = f64::from(chunk_z) * ALPHA_DENSITY_HORIZONTAL_CELLS as f64;
        let horizontal = 684.412;
        let vertical = 684.412;
        let scale_noise = self.scale.region(
            start_x,
            0.0,
            start_z,
            ALPHA_DENSITY_SIZE_XZ,
            1,
            ALPHA_DENSITY_SIZE_XZ,
            1.0,
            0.0,
            1.0,
        );
        let depth_noise = self.depth.region(
            start_x,
            0.0,
            start_z,
            ALPHA_DENSITY_SIZE_XZ,
            1,
            ALPHA_DENSITY_SIZE_XZ,
            100.0,
            0.0,
            100.0,
        );
        let selector = self.selector.region(
            start_x,
            0.0,
            start_z,
            ALPHA_DENSITY_SIZE_XZ,
            ALPHA_DENSITY_SIZE_Y,
            ALPHA_DENSITY_SIZE_XZ,
            horizontal / 80.0,
            vertical / 160.0,
            horizontal / 80.0,
        );
        let min_limit = self.min_limit.region(
            start_x,
            0.0,
            start_z,
            ALPHA_DENSITY_SIZE_XZ,
            ALPHA_DENSITY_SIZE_Y,
            ALPHA_DENSITY_SIZE_XZ,
            horizontal,
            vertical,
            horizontal,
        );
        let max_limit = self.max_limit.region(
            start_x,
            0.0,
            start_z,
            ALPHA_DENSITY_SIZE_XZ,
            ALPHA_DENSITY_SIZE_Y,
            ALPHA_DENSITY_SIZE_XZ,
            horizontal,
            vertical,
            horizontal,
        );

        let mut lattice =
            vec![0.0; ALPHA_DENSITY_SIZE_XZ * ALPHA_DENSITY_SIZE_Y * ALPHA_DENSITY_SIZE_XZ];
        let mut density_index = 0;
        let mut column_index = 0;
        for _local_x in 0..ALPHA_DENSITY_SIZE_XZ {
            for _local_z in 0..ALPHA_DENSITY_SIZE_XZ {
                let mut terrain_scale = (scale_noise[column_index] + 256.0) / 512.0;
                if terrain_scale > 1.0 {
                    terrain_scale = 1.0;
                }

                let lower_fade_target = 0.0;
                let mut terrain_depth = depth_noise[column_index] / 8000.0;
                if terrain_depth < 0.0 {
                    terrain_depth = -terrain_depth;
                }
                terrain_depth = terrain_depth * 3.0 - 3.0;
                if terrain_depth < 0.0 {
                    terrain_depth /= 2.0;
                    if terrain_depth < -1.0 {
                        terrain_depth = -1.0;
                    }
                    terrain_depth /= 1.4;
                    terrain_depth /= 2.0;
                    terrain_scale = 0.0;
                } else {
                    if terrain_depth > 1.0 {
                        terrain_depth = 1.0;
                    }
                    terrain_depth /= 6.0;
                }

                terrain_scale += 0.5;
                terrain_depth = terrain_depth * ALPHA_DENSITY_SIZE_Y as f64 / 16.0;
                let vertical_center = ALPHA_DENSITY_SIZE_Y as f64 / 2.0 + terrain_depth * 4.0;
                column_index += 1;

                for local_y in 0..ALPHA_DENSITY_SIZE_Y {
                    let mut vertical_gradient =
                        (local_y as f64 - vertical_center) * 12.0 / terrain_scale;
                    if vertical_gradient < 0.0 {
                        vertical_gradient *= 4.0;
                    }

                    let lower = min_limit[density_index] / 512.0;
                    let upper = max_limit[density_index] / 512.0;
                    let blend = (selector[density_index] / 10.0 + 1.0) / 2.0;
                    let mut density = if blend < 0.0 {
                        lower
                    } else if blend > 1.0 {
                        upper
                    } else {
                        lower + (upper - lower) * blend
                    };
                    density -= vertical_gradient;

                    if local_y > ALPHA_DENSITY_SIZE_Y - 4 {
                        let fade = (local_y - (ALPHA_DENSITY_SIZE_Y - 4)) as f64 / 3.0;
                        density = density * (1.0 - fade) + -10.0 * fade;
                    }
                    if (local_y as f64) < lower_fade_target {
                        let fade = ((lower_fade_target - local_y as f64) / 4.0).clamp(0.0, 1.0);
                        density = density * (1.0 - fade) + -10.0 * fade;
                    }

                    lattice[density_index] = density;
                    density_index += 1;
                }
            }
        }
        lattice
    }
}

pub fn generate_alpha_stage_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    winter: bool,
    stage: AlphaGenerationStage,
) -> GeneratedChunk {
    if stage == AlphaGenerationStage::Features {
        return generate_alpha_chunk(seed, chunk_x, chunk_z, winter);
    }
    let banks = AlphaNoiseBanks::new(seed);
    let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, ALPHA_BUILD_HEIGHT);
    build_alpha_terrain(&banks, &mut chunk, winter);
    if !matches!(stage, AlphaGenerationStage::Terrain) {
        build_alpha_surfaces(&banks, &mut chunk);
    }
    if stage == AlphaGenerationStage::Caves {
        carve_alpha_caves(seed, &mut chunk);
    }
    chunk.prime_worldgen_heightmaps();
    GeneratedChunk::from_mutable_buffer_with_biomes(
        chunk,
        vec![ALPHA_PLAINS_BIOME_ID; expected_chunk_biome_count(ALPHA_BUILD_HEIGHT)],
    )
}

pub fn generate_alpha_chunk(seed: i64, chunk_x: i32, chunk_z: i32, winter: bool) -> GeneratedChunk {
    let pos = ChunkPos::new(chunk_x, chunk_z);
    AlphaFeatureDependencyCache::new()
        .generate_features_chunks(seed, winter, [pos])
        .chunks
        .remove(&pos)
        .unwrap_or_else(|| panic!("alpha feature batch omitted ({chunk_x}, {chunk_z})"))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AlphaFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaFeatureBatchResult {
    pub chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub cache_report: AlphaFeatureDependencyCacheReport,
}

#[derive(Debug, Default)]
pub struct AlphaFeatureDependencyCache {
    descriptor: Option<(i64, bool)>,
    banks: Option<AlphaNoiseBanks>,
    cache: SurfaceDependencyCache,
}

impl AlphaFeatureDependencyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.cache.retained_chunk_count()
    }

    pub fn resident_positions(&self) -> BTreeSet<ChunkPos> {
        self.cache.resident_positions()
    }

    pub fn clear(&mut self) {
        self.descriptor = None;
        self.banks = None;
        self.cache.clear();
    }

    pub fn generate_features_chunks(
        &mut self,
        seed: i64,
        winter: bool,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> AlphaFeatureBatchResult {
        self.generate_features_chunks_with_dependencies(seed, winter, targets, std::iter::empty())
    }

    pub fn generate_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        winter: bool,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> AlphaFeatureBatchResult {
        if self.descriptor != Some((seed, winter)) {
            self.clear();
            self.descriptor = Some((seed, winter));
            self.banks = Some(AlphaNoiseBanks::new(seed));
        }

        let plan = ChunkGenerationPlan::alpha_features(targets);
        let banks = self.banks.as_ref().expect("alpha banks initialized");
        let PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies,
            report,
        } = self.cache.prepare(seed, &plan, dependencies, |pos| {
            generate_alpha_surface_buffer_with_banks(banks, seed, pos.x, pos.z, winter)
        });
        let cache_report = alpha_cache_report(report);

        if plan.output_chunks().is_empty() {
            return AlphaFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies,
                cache_report,
            };
        }

        let first_target = *plan
            .output_chunks()
            .iter()
            .next()
            .expect("non-empty alpha targets");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, region_chunks);
        for center in sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()) {
            region.set_center(center.x, center.z);
            populate_alpha_center(banks, seed, center, &mut region);
        }

        let (targets, _, _) = plan.into_parts();
        let mut chunks = BTreeMap::new();
        for target in targets {
            let mut chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
                panic!(
                    "alpha feature region omitted target ({}, {})",
                    target.x, target.z
                )
            });
            if winter {
                apply_alpha_snow(&mut chunk);
            }
            chunk.prime_worldgen_heightmaps();
            chunks.insert(target, alpha_generated_chunk(chunk));
        }

        AlphaFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report,
        }
    }
}

fn alpha_cache_report(report: SurfaceDependencyCacheReport) -> AlphaFeatureDependencyCacheReport {
    AlphaFeatureDependencyCacheReport {
        requested_dependency_chunks: report.requested_dependency_chunks,
        cache_hits: report.cache_hits,
        generated_dependency_chunks: report.generated_dependency_chunks,
        retained_dependency_chunks: report.retained_dependency_chunks,
    }
}

fn alpha_generated_chunk(chunk: MutableChunkBlockBuffer) -> GeneratedChunk {
    GeneratedChunk::from_mutable_buffer_with_biomes(
        chunk,
        vec![ALPHA_PLAINS_BIOME_ID; expected_chunk_biome_count(ALPHA_BUILD_HEIGHT)],
    )
}

/// Convert a native Alpha chunk into the historical 16x16x128 X/Z/Y byte
/// order used by the Java oracle. Fluid state distinctions intentionally map
/// to Alpha's still-water and flowing-lava generation IDs.
pub fn alpha_semantic_bytes(chunk: &GeneratedChunk) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((CHUNK_WIDTH * CHUNK_WIDTH * ALPHA_ACTIVE_HEIGHT) as usize);
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            for y in 0..ALPHA_ACTIVE_HEIGHT {
                let native = chunk.block_at_y(x, y, z).0;
                bytes.push(alpha_semantic_block_id(native).unwrap_or_else(|| {
                    panic!("native block {native} has no Alpha semantic mapping")
                }));
            }
        }
    }
    bytes
}

pub const fn alpha_semantic_block_id(native: RawBlockId) -> Option<u8> {
    match native {
        AIR => Some(0),
        STONE => Some(1),
        GRASS_BLOCK => Some(2),
        DIRT => Some(3),
        BEDROCK => Some(7),
        WATER => Some(9),
        LAVA => Some(10),
        SAND => Some(12),
        GRAVEL => Some(13),
        GOLD_ORE => Some(14),
        IRON_ORE => Some(15),
        COAL_ORE => Some(16),
        crate::block::OAK_LOG => Some(17),
        crate::block::OAK_LEAVES => Some(18),
        DANDELION => Some(37),
        POPPY => Some(38),
        BROWN_MUSHROOM => Some(39),
        RED_MUSHROOM => Some(40),
        MOSSY_COBBLESTONE => Some(48),
        DIAMOND_ORE => Some(56),
        REDSTONE_ORE => Some(73),
        SNOW => Some(78),
        ICE => Some(79),
        CACTUS => Some(81),
        CLAY => Some(82),
        SUGAR_CANE => Some(83),
        _ => None,
    }
}

fn generate_alpha_surface_buffer_with_banks(
    banks: &AlphaNoiseBanks,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    winter: bool,
) -> MutableChunkBlockBuffer {
    let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, ALPHA_BUILD_HEIGHT);
    build_alpha_terrain(banks, &mut chunk, winter);
    build_alpha_surfaces(banks, &mut chunk);
    carve_alpha_caves(seed, &mut chunk);
    chunk.prime_worldgen_heightmaps();
    chunk
}

fn build_alpha_terrain(banks: &AlphaNoiseBanks, chunk: &mut MutableChunkBlockBuffer, winter: bool) {
    let lattice = banks.density_lattice(chunk.chunk_x, chunk.chunk_z);
    for cell_x in 0..ALPHA_DENSITY_HORIZONTAL_CELLS {
        for cell_z in 0..ALPHA_DENSITY_HORIZONTAL_CELLS {
            for cell_y in 0..ALPHA_DENSITY_VERTICAL_CELLS {
                let mut d000 = lattice[alpha_density_index(cell_x, cell_z, cell_y)];
                let mut d001 = lattice[alpha_density_index(cell_x, cell_z + 1, cell_y)];
                let mut d100 = lattice[alpha_density_index(cell_x + 1, cell_z, cell_y)];
                let mut d101 = lattice[alpha_density_index(cell_x + 1, cell_z + 1, cell_y)];
                let dy000 =
                    (lattice[alpha_density_index(cell_x, cell_z, cell_y + 1)] - d000) * 0.125;
                let dy001 =
                    (lattice[alpha_density_index(cell_x, cell_z + 1, cell_y + 1)] - d001) * 0.125;
                let dy100 =
                    (lattice[alpha_density_index(cell_x + 1, cell_z, cell_y + 1)] - d100) * 0.125;
                let dy101 = (lattice[alpha_density_index(cell_x + 1, cell_z + 1, cell_y + 1)]
                    - d101)
                    * 0.125;

                for sub_y in 0..8 {
                    let mut left = d000;
                    let mut right = d001;
                    let dx_left = (d100 - d000) * 0.25;
                    let dx_right = (d101 - d001) * 0.25;
                    for sub_x in 0..4 {
                        let mut density = left;
                        let dz = (right - left) * 0.25;
                        for sub_z in 0..4 {
                            let local_x = (cell_x * 4 + sub_x) as i32;
                            let y = (cell_y * 8 + sub_y) as i32;
                            let local_z = (cell_z * 4 + sub_z) as i32;
                            let block = if density > 0.0 {
                                STONE
                            } else if y < ALPHA_SEA_LEVEL {
                                if winter && y >= ALPHA_SEA_LEVEL - 1 {
                                    ICE
                                } else {
                                    WATER
                                }
                            } else {
                                AIR
                            };
                            chunk.set_block_at_y(local_x, y, local_z, block);
                            density += dz;
                        }
                        left += dx_left;
                        right += dx_right;
                    }
                    d000 += dy000;
                    d001 += dy001;
                    d100 += dy100;
                    d101 += dy101;
                }
            }
        }
    }
}

fn build_alpha_surfaces(banks: &AlphaNoiseBanks, chunk: &mut MutableChunkBlockBuffer) {
    let chunk_seed = i64::from(chunk.chunk_x)
        .wrapping_mul(ALPHA_BASE_CHUNK_MULTIPLIER_X)
        .wrapping_add(i64::from(chunk.chunk_z).wrapping_mul(ALPHA_BASE_CHUNK_MULTIPLIER_Z));
    let mut random = SimpleRandomSource::new(chunk_seed);
    let start_x = f64::from(chunk_min_block_coord(chunk.chunk_x));
    let start_z = f64::from(chunk_min_block_coord(chunk.chunk_z));
    let mask_scale = 0.03125;
    let sand = banks.surface_mask.region(
        start_x, start_z, 0.0, 16, 16, 1, mask_scale, mask_scale, 1.0,
    );
    let gravel = banks.surface_mask.region(
        start_z, 109.0134, start_x, 16, 1, 16, mask_scale, 1.0, mask_scale,
    );
    let depth = banks.surface_depth.region(
        start_x,
        start_z,
        0.0,
        16,
        16,
        1,
        mask_scale * 2.0,
        mask_scale * 2.0,
        mask_scale * 2.0,
    );

    for local_x in 0..CHUNK_WIDTH {
        for local_z in 0..CHUNK_WIDTH {
            let mask_index = (local_x + local_z * CHUNK_WIDTH) as usize;
            let sand_column = sand[mask_index] + random.next_double() * 0.2 > 0.0;
            let gravel_column = gravel[mask_index] + random.next_double() * 0.2 > 3.0;
            let layer_depth = (depth[mask_index] / 3.0 + 3.0 + random.next_double() * 0.25) as i32;
            let mut remaining = -1;
            let mut top = GRASS_BLOCK;
            let mut filler = DIRT;

            for y in (0..ALPHA_ACTIVE_HEIGHT).rev() {
                if y <= random.next_int_bound(6) - 1 {
                    chunk.set_block_at_y(local_x, y, local_z, BEDROCK);
                    continue;
                }
                let current = chunk.get_block_at_y(local_x, y, local_z);
                if current == AIR {
                    remaining = -1;
                } else if current == STONE {
                    if remaining == -1 {
                        if layer_depth <= 0 {
                            top = AIR;
                            filler = STONE;
                        } else if (ALPHA_SEA_LEVEL - 4..=ALPHA_SEA_LEVEL + 1).contains(&y) {
                            top = GRASS_BLOCK;
                            filler = DIRT;
                            if gravel_column {
                                top = AIR;
                                filler = GRAVEL;
                            }
                            if sand_column {
                                top = SAND;
                                filler = SAND;
                            }
                        }
                        if y < ALPHA_SEA_LEVEL && top == AIR {
                            top = WATER;
                        }
                        remaining = layer_depth;
                        chunk.set_block_at_y(
                            local_x,
                            y,
                            local_z,
                            if y >= ALPHA_SEA_LEVEL - 1 {
                                top
                            } else {
                                filler
                            },
                        );
                    } else if remaining > 0 {
                        remaining -= 1;
                        chunk.set_block_at_y(local_x, y, local_z, filler);
                    }
                }
            }
        }
    }
}

fn populate_alpha_center(
    banks: &AlphaNoiseBanks,
    seed: i64,
    center: ChunkPos,
    world: &mut FeatureRegion,
) {
    let origin_x = center.x * 16;
    let origin_z = center.z * 16;
    let mut seed_random = SimpleRandomSource::new(seed);
    let multiplier_x = odd_multiplier(seed_random.next_long());
    let multiplier_z = odd_multiplier(seed_random.next_long());
    let population_seed = i64::from(center.x)
        .wrapping_mul(multiplier_x)
        .wrapping_add(i64::from(center.z).wrapping_mul(multiplier_z))
        ^ seed;
    let mut random = SimpleRandomSource::new(population_seed);

    for _ in 0..8 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let y = random.next_int_bound(ALPHA_ACTIVE_HEIGHT);
        let z = origin_z + random.next_int_bound(16) + 8;
        place_alpha_dungeon(world, &mut random, BlockPos::new(x, y, z));
    }
    for _ in 0..10 {
        let origin = BlockPos::new(
            origin_x + random.next_int_bound(16),
            random.next_int_bound(ALPHA_ACTIVE_HEIGHT),
            origin_z + random.next_int_bound(16),
        );
        place_alpha_vein(world, &mut random, origin, CLAY, 32, SAND, true);
    }
    place_alpha_vein_attempts(world, &mut random, origin_x, origin_z, DIRT, 32, 20, 128);
    place_alpha_vein_attempts(world, &mut random, origin_x, origin_z, GRAVEL, 32, 10, 128);
    place_alpha_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        COAL_ORE,
        16,
        20,
        128,
    );
    place_alpha_vein_attempts(world, &mut random, origin_x, origin_z, IRON_ORE, 8, 20, 64);
    place_alpha_vein_attempts(world, &mut random, origin_x, origin_z, GOLD_ORE, 8, 2, 32);
    place_alpha_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        REDSTONE_ORE,
        7,
        8,
        16,
    );
    place_alpha_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        DIAMOND_ORE,
        7,
        1,
        16,
    );

    let mut tree_count = ((banks
        .forest
        .value_2d(f64::from(origin_x) * 0.5, f64::from(origin_z) * 0.5)
        / 8.0
        + random.next_double() * 4.0
        + 4.0)
        / 3.0) as i32;
    tree_count = tree_count.max(0);
    if random.next_int_bound(10) == 0 {
        tree_count += 1;
    }
    let tree_config = if random.next_int_bound(10) == 0 {
        BasicTreeConfiguration::new(crate::block::OAK_LOG, crate::block::OAK_LEAVES, 6, 4)
    } else {
        BasicTreeConfiguration::oak()
    };
    let tree = ConfiguredFeature::basic_tree(tree_config);
    for _ in 0..tree_count {
        let x = origin_x + random.next_int_bound(16) + 8;
        let z = origin_z + random.next_int_bound(16) + 8;
        tree.place(world, &mut random, BlockPos::new(x, 0, z));
    }

    for _ in 0..2 {
        let origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
        place_alpha_plant_patch(world, &mut random, origin, DANDELION);
    }
    if random.next_int_bound(2) == 0 {
        let origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
        place_alpha_plant_patch(world, &mut random, origin, POPPY);
    }
    if random.next_int_bound(4) == 0 {
        let origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
        place_alpha_plant_patch(world, &mut random, origin, BROWN_MUSHROOM);
    }
    if random.next_int_bound(8) == 0 {
        let origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
        place_alpha_plant_patch(world, &mut random, origin, RED_MUSHROOM);
    }
    for _ in 0..10 {
        let origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
        place_alpha_sugar_cane(world, &mut random, origin);
    }
    let cactus_origin = alpha_offset_feature_origin(&mut random, origin_x, origin_z);
    place_alpha_cactus(world, &mut random, cactus_origin);

    for _ in 0..50 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let y_bound = random.next_int_bound(120) + 8;
        let y = random.next_int_bound(y_bound);
        let z = origin_z + random.next_int_bound(16) + 8;
        place_alpha_spring(world, BlockPos::new(x, y, z), WATER);
    }
    for _ in 0..20 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let first = random.next_int_bound(112) + 8;
        let second = random.next_int_bound(first) + 8;
        let y = random.next_int_bound(second);
        let z = origin_z + random.next_int_bound(16) + 8;
        place_alpha_spring(world, BlockPos::new(x, y, z), LAVA);
    }
}

fn alpha_offset_feature_origin(
    random: &mut SimpleRandomSource,
    origin_x: i32,
    origin_z: i32,
) -> BlockPos {
    BlockPos::new(
        origin_x + random.next_int_bound(16) + 8,
        random.next_int_bound(ALPHA_ACTIVE_HEIGHT),
        origin_z + random.next_int_bound(16) + 8,
    )
}

#[allow(clippy::too_many_arguments)]
fn place_alpha_vein_attempts(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin_x: i32,
    origin_z: i32,
    block: RawBlockId,
    size: i32,
    attempts: i32,
    y_bound: i32,
) {
    for _ in 0..attempts {
        let origin = BlockPos::new(
            origin_x + random.next_int_bound(16),
            random.next_int_bound(y_bound),
            origin_z + random.next_int_bound(16),
        );
        place_alpha_vein(world, random, origin, block, size, STONE, false);
    }
}

fn place_alpha_vein(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
    block: RawBlockId,
    size: i32,
    replace: RawBlockId,
    require_water_origin: bool,
) -> bool {
    if require_water_origin && alpha_block(world, origin) != WATER {
        return false;
    }
    let angle = random.next_float() * std::f32::consts::PI;
    let start_x = f64::from(origin.x + 8) + f64::from(alpha_sin(angle) * size as f32 / 8.0);
    let end_x = f64::from(origin.x + 8) - f64::from(alpha_sin(angle) * size as f32 / 8.0);
    let start_z = f64::from(origin.z + 8) + f64::from(alpha_cos(angle) * size as f32 / 8.0);
    let end_z = f64::from(origin.z + 8) - f64::from(alpha_cos(angle) * size as f32 / 8.0);
    let start_y = f64::from(origin.y + random.next_int_bound(3) + 2);
    let end_y = f64::from(origin.y + random.next_int_bound(3) + 2);
    for step in 0..=size {
        let progress = f64::from(step) / f64::from(size);
        let center_x = start_x + (end_x - start_x) * progress;
        let center_y = start_y + (end_y - start_y) * progress;
        let center_z = start_z + (end_z - start_z) * progress;
        let scale = random.next_double() * f64::from(size) / 16.0;
        let radius_xz =
            f64::from(alpha_sin(step as f32 * std::f32::consts::PI / size as f32) + 1.0) * scale
                + 1.0;
        let radius_y = radius_xz;
        for x in (center_x - radius_xz / 2.0) as i32..=(center_x + radius_xz / 2.0) as i32 {
            for y in (center_y - radius_y / 2.0) as i32..=(center_y + radius_y / 2.0) as i32 {
                for z in (center_z - radius_xz / 2.0) as i32..=(center_z + radius_xz / 2.0) as i32 {
                    let dx = (f64::from(x) + 0.5 - center_x) / (radius_xz / 2.0);
                    let dy = (f64::from(y) + 0.5 - center_y) / (radius_y / 2.0);
                    let dz = (f64::from(z) + 0.5 - center_z) / (radius_xz / 2.0);
                    let pos = BlockPos::new(x, y, z);
                    if dx * dx + dy * dy + dz * dz < 1.0 && alpha_block(world, pos) == replace {
                        world.set_block_world(pos, block);
                    }
                }
            }
        }
    }
    true
}

fn place_alpha_dungeon(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) -> bool {
    let radius_x = random.next_int_bound(2) + 2;
    let radius_z = random.next_int_bound(2) + 2;
    let mut openings = 0;
    for x in origin.x - radius_x - 1..=origin.x + radius_x + 1 {
        for y in origin.y - 1..=origin.y + 4 {
            for z in origin.z - radius_z - 1..=origin.z + radius_z + 1 {
                let block = alpha_block(world, BlockPos::new(x, y, z));
                if (y == origin.y - 1 || y == origin.y + 4) && !alpha_is_solid(block) {
                    return false;
                }
                if (x == origin.x - radius_x - 1
                    || x == origin.x + radius_x + 1
                    || z == origin.z - radius_z - 1
                    || z == origin.z + radius_z + 1)
                    && y == origin.y
                    && block == AIR
                    && alpha_block(world, BlockPos::new(x, y + 1, z)) == AIR
                {
                    openings += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&openings) {
        return false;
    }

    for x in origin.x - radius_x - 1..=origin.x + radius_x + 1 {
        for y in (origin.y - 1..=origin.y + 3).rev() {
            for z in origin.z - radius_z - 1..=origin.z + radius_z + 1 {
                let boundary = x == origin.x - radius_x - 1
                    || x == origin.x + radius_x + 1
                    || y == origin.y - 1
                    || y == origin.y + 3
                    || z == origin.z - radius_z - 1
                    || z == origin.z + radius_z + 1;
                let pos = BlockPos::new(x, y, z);
                if !boundary {
                    world.set_block_world(pos, AIR);
                } else if y >= 0 && !alpha_is_solid(alpha_block(world, BlockPos::new(x, y - 1, z)))
                {
                    world.set_block_world(pos, AIR);
                } else if alpha_is_solid(alpha_block(world, pos)) {
                    let shell = if y == origin.y - 1 && random.next_int_bound(4) != 0 {
                        MOSSY_COBBLESTONE
                    } else {
                        // Cobblestone has no dedicated native registry entry;
                        // stone preserves the room silhouette.
                        STONE
                    };
                    world.set_block_world(pos, shell);
                }
            }
        }
    }
    true
}

fn place_alpha_plant_patch(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
    plant: RawBlockId,
) {
    for _ in 0..64 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(8) - random.next_int_bound(8),
            origin.y + random.next_int_bound(4) - random.next_int_bound(4),
            origin.z + random.next_int_bound(8) - random.next_int_bound(8),
        );
        if alpha_block(world, pos) == AIR
            && matches!(
                alpha_block(world, BlockPos::new(pos.x, pos.y - 1, pos.z)),
                GRASS_BLOCK | DIRT
            )
        {
            world.set_block_world(pos, plant);
        }
    }
}

fn place_alpha_sugar_cane(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) {
    for _ in 0..20 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(4) - random.next_int_bound(4),
            origin.y,
            origin.z + random.next_int_bound(4) - random.next_int_bound(4),
        );
        let below_y = pos.y - 1;
        let near_water = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .any(|(dx, dz)| {
                alpha_block(world, BlockPos::new(pos.x + dx, below_y, pos.z + dz)) == WATER
            });
        if alpha_block(world, pos) == AIR && near_water {
            let height_bound = random.next_int_bound(3) + 1;
            let height = 2 + random.next_int_bound(height_bound);
            for dy in 0..height {
                let target = BlockPos::new(pos.x, pos.y + dy, pos.z);
                if alpha_block(world, target) != AIR {
                    break;
                }
                world.set_block_world(target, SUGAR_CANE);
            }
        }
    }
}

fn place_alpha_cactus(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) {
    for _ in 0..10 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(8) - random.next_int_bound(8),
            origin.y + random.next_int_bound(4) - random.next_int_bound(4),
            origin.z + random.next_int_bound(8) - random.next_int_bound(8),
        );
        if alpha_block(world, pos) != AIR {
            continue;
        }
        let height_bound = random.next_int_bound(3) + 1;
        let height = 1 + random.next_int_bound(height_bound);
        for dy in 0..height {
            let target = BlockPos::new(pos.x, pos.y + dy, pos.z);
            let below = alpha_block(world, BlockPos::new(target.x, target.y - 1, target.z));
            let sides_clear = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .all(|(dx, dz)| {
                    alpha_block(world, BlockPos::new(target.x + dx, target.y, target.z + dz)) == AIR
                });
            if alpha_block(world, target) == AIR && matches!(below, SAND | CACTUS) && sides_clear {
                world.set_block_world(target, CACTUS);
            }
        }
    }
}

fn place_alpha_spring(world: &mut FeatureRegion, origin: BlockPos, liquid: RawBlockId) -> bool {
    if alpha_block(world, BlockPos::new(origin.x, origin.y + 1, origin.z)) != STONE
        || alpha_block(world, BlockPos::new(origin.x, origin.y - 1, origin.z)) != STONE
        || !matches!(alpha_block(world, origin), AIR | STONE)
    {
        return false;
    }
    let neighbours = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let stone = neighbours
        .into_iter()
        .filter(|(dx, dz)| {
            alpha_block(world, BlockPos::new(origin.x + dx, origin.y, origin.z + dz)) == STONE
        })
        .count();
    let air = neighbours
        .into_iter()
        .filter(|(dx, dz)| {
            alpha_block(world, BlockPos::new(origin.x + dx, origin.y, origin.z + dz)) == AIR
        })
        .count();
    if stone == 3 && air == 1 {
        world.set_block_world(origin, liquid);
    }
    true
}

fn apply_alpha_snow(chunk: &mut MutableChunkBlockBuffer) {
    for local_x in 0..CHUNK_WIDTH {
        for local_z in 0..CHUNK_WIDTH {
            let mut surface_y = 0;
            for y in (0..ALPHA_ACTIVE_HEIGHT).rev() {
                if chunk.get_block_at_y(local_x, y, local_z) != AIR {
                    surface_y = y + 1;
                    break;
                }
            }
            if surface_y > 0 && surface_y < ALPHA_ACTIVE_HEIGHT {
                let below = chunk.get_block_at_y(local_x, surface_y - 1, local_z);
                if alpha_is_solid(below) && below != ICE {
                    chunk.set_block_at_y(local_x, surface_y, local_z, SNOW);
                }
            }
        }
    }
}

fn alpha_block(world: &mut FeatureRegion, pos: BlockPos) -> RawBlockId {
    world.block_at_world(pos).unwrap_or(AIR)
}

fn alpha_is_solid(block: RawBlockId) -> bool {
    !matches!(
        block,
        AIR | WATER | LAVA | SNOW | DANDELION | POPPY | BROWN_MUSHROOM | RED_MUSHROOM | SUGAR_CANE
    )
}

fn carve_alpha_caves(seed: i64, chunk: &mut MutableChunkBlockBuffer) {
    let range = 8_i32;
    let mut random = SimpleRandomSource::new(seed);
    let multiplier_x = odd_multiplier(random.next_long());
    let multiplier_z = odd_multiplier(random.next_long());
    let mut carver = AlphaCaveCarver::new(seed);

    for source_x in chunk.chunk_x - range..=chunk.chunk_x + range {
        for source_z in chunk.chunk_z - range..=chunk.chunk_z + range {
            let source_seed = i64::from(source_x)
                .wrapping_mul(multiplier_x)
                .wrapping_add(i64::from(source_z).wrapping_mul(multiplier_z))
                ^ seed;
            carver.random.set_seed(source_seed);
            carver.carve_source(source_x, source_z, chunk);
        }
    }
}

fn odd_multiplier(value: i64) -> i64 {
    value.wrapping_div(2).wrapping_mul(2).wrapping_add(1)
}

struct AlphaCaveCarver {
    random: SimpleRandomSource,
}

impl AlphaCaveCarver {
    fn new(seed: i64) -> Self {
        Self {
            random: SimpleRandomSource::new(seed),
        }
    }

    fn carve_source(
        &mut self,
        source_chunk_x: i32,
        source_chunk_z: i32,
        target: &mut MutableChunkBlockBuffer,
    ) {
        let inner = self.random.next_int_bound(40) + 1;
        let middle = self.random.next_int_bound(inner) + 1;
        let mut cave_count = self.random.next_int_bound(middle);
        if self.random.next_int_bound(15) != 0 {
            cave_count = 0;
        }

        for _ in 0..cave_count {
            let x = f64::from(source_chunk_x * 16 + self.random.next_int_bound(16));
            let y_bound = self.random.next_int_bound(120) + 8;
            let y = f64::from(self.random.next_int_bound(y_bound));
            let z = f64::from(source_chunk_z * 16 + self.random.next_int_bound(16));
            let mut tunnel_count = 1;
            if self.random.next_int_bound(4) == 0 {
                let width = 1.0 + self.random.next_float() * 6.0;
                self.carve_tunnel(target, x, y, z, width, 0.0, 0.0, -1, -1, 0.5);
                tunnel_count += self.random.next_int_bound(4);
            }

            for _ in 0..tunnel_count {
                let yaw = self.random.next_float() * std::f32::consts::PI * 2.0;
                let pitch = (self.random.next_float() - 0.5) * 2.0 / 8.0;
                let width = self.random.next_float() * 2.0 + self.random.next_float();
                self.carve_tunnel(target, x, y, z, width, yaw, pitch, 0, 0, 1.0);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn carve_tunnel(
        &mut self,
        target: &mut MutableChunkBlockBuffer,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        base_width: f32,
        mut yaw: f32,
        mut pitch: f32,
        mut tunnel: i32,
        mut tunnel_count: i32,
        width_height_ratio: f64,
    ) {
        let target_center_x = f64::from(target.chunk_x * 16 + 8);
        let target_center_z = f64::from(target.chunk_z * 16 + 8);
        let mut yaw_velocity = 0.0_f32;
        let mut pitch_velocity = 0.0_f32;
        let mut random = SimpleRandomSource::new(self.random.next_long());

        if tunnel_count <= 0 {
            let span = 8 * 16 - 16;
            tunnel_count = span - random.next_int_bound(span / 4);
        }

        let mut room = false;
        if tunnel == -1 {
            tunnel = tunnel_count / 2;
            room = true;
        }

        let branch_at = random.next_int_bound(tunnel_count / 2) + tunnel_count / 4;
        let gentle_pitch = random.next_int_bound(6) == 0;

        while tunnel < tunnel_count {
            let horizontal_radius = 1.5
                + f64::from(
                    alpha_sin(tunnel as f32 * std::f32::consts::PI / tunnel_count as f32)
                        * base_width,
                );
            let vertical_radius = horizontal_radius * width_height_ratio;
            let horizontal_pitch = alpha_cos(pitch);
            let vertical_pitch = alpha_sin(pitch);
            x += f64::from(alpha_cos(yaw) * horizontal_pitch);
            y += f64::from(vertical_pitch);
            z += f64::from(alpha_sin(yaw) * horizontal_pitch);
            pitch *= if gentle_pitch { 0.92 } else { 0.7 };
            pitch += pitch_velocity * 0.1;
            yaw += yaw_velocity * 0.1;
            pitch_velocity *= 0.9;
            yaw_velocity *= 0.75;
            pitch_velocity +=
                (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            yaw_velocity += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if !room && tunnel == branch_at && base_width > 1.0 {
                let left_width = random.next_float() * 0.5 + 0.5;
                self.carve_tunnel(
                    target,
                    x,
                    y,
                    z,
                    left_width,
                    yaw - std::f32::consts::FRAC_PI_2,
                    pitch / 3.0,
                    tunnel,
                    tunnel_count,
                    1.0,
                );
                let right_width = random.next_float() * 0.5 + 0.5;
                self.carve_tunnel(
                    target,
                    x,
                    y,
                    z,
                    right_width,
                    yaw + std::f32::consts::FRAC_PI_2,
                    pitch / 3.0,
                    tunnel,
                    tunnel_count,
                    1.0,
                );
                return;
            }

            if room || random.next_int_bound(4) != 0 {
                let distance_x = x - target_center_x;
                let distance_z = z - target_center_z;
                let remaining = f64::from(tunnel_count - tunnel);
                let reach = f64::from(base_width + 18.0);
                if distance_x * distance_x + distance_z * distance_z - remaining * remaining
                    > reach * reach
                {
                    return;
                }

                if x >= target_center_x - 16.0 - horizontal_radius * 2.0
                    && z >= target_center_z - 16.0 - horizontal_radius * 2.0
                    && x <= target_center_x + 16.0 + horizontal_radius * 2.0
                    && z <= target_center_z + 16.0 + horizontal_radius * 2.0
                {
                    let mut min_x = alpha_floor(x - horizontal_radius) - target.chunk_x * 16 - 1;
                    let mut max_x = alpha_floor(x + horizontal_radius) - target.chunk_x * 16 + 1;
                    let mut min_y = alpha_floor(y - vertical_radius) - 1;
                    let mut max_y = alpha_floor(y + vertical_radius) + 1;
                    let mut min_z = alpha_floor(z - horizontal_radius) - target.chunk_z * 16 - 1;
                    let mut max_z = alpha_floor(z + horizontal_radius) - target.chunk_z * 16 + 1;
                    min_x = min_x.max(0);
                    max_x = max_x.min(16);
                    min_y = min_y.max(1);
                    max_y = max_y.min(120);
                    min_z = min_z.max(0);
                    max_z = max_z.min(16);

                    let mut found_water = false;
                    'water_scan: for local_x in min_x..max_x {
                        for local_z in min_z..max_z {
                            let mut scan_y = max_y + 1;
                            while scan_y >= min_y - 1 {
                                if (0..ALPHA_ACTIVE_HEIGHT).contains(&scan_y)
                                    && target.get_block_at_y(local_x, scan_y, local_z) == WATER
                                {
                                    found_water = true;
                                    break 'water_scan;
                                }
                                if scan_y != min_y - 1
                                    && local_x != min_x
                                    && local_x != max_x - 1
                                    && local_z != min_z
                                    && local_z != max_z - 1
                                {
                                    // Java's for-loop applies its trailing
                                    // decrement after assigning `min_y`.
                                    scan_y = min_y - 1;
                                } else {
                                    scan_y -= 1;
                                }
                            }
                        }
                    }

                    if !found_water {
                        for local_x in min_x..max_x {
                            let normalized_x = (f64::from(local_x + target.chunk_x * 16) + 0.5 - x)
                                / horizontal_radius;
                            for local_z in min_z..max_z {
                                let normalized_z = (f64::from(local_z + target.chunk_z * 16) + 0.5
                                    - z)
                                    / horizontal_radius;
                                let mut exposed_grass = false;
                                for block_y in (min_y..max_y).rev() {
                                    let normalized_y =
                                        (f64::from(block_y) + 0.5 - y) / vertical_radius;
                                    if normalized_y > -0.7
                                        && normalized_x * normalized_x
                                            + normalized_y * normalized_y
                                            + normalized_z * normalized_z
                                            < 1.0
                                    {
                                        // The original raw-array cursor starts
                                        // one Y above its geometry loop.
                                        let storage_y = block_y + 1;
                                        let block =
                                            target.get_block_at_y(local_x, storage_y, local_z);
                                        if block == GRASS_BLOCK {
                                            exposed_grass = true;
                                        }
                                        if matches!(block, STONE | DIRT | GRASS_BLOCK) {
                                            if block_y < 10 {
                                                target.set_block_at_y(
                                                    local_x, storage_y, local_z, LAVA,
                                                );
                                            } else {
                                                target.set_block_at_y(
                                                    local_x, storage_y, local_z, AIR,
                                                );
                                                if exposed_grass
                                                    && target.get_block_at_y(
                                                        local_x,
                                                        storage_y - 1,
                                                        local_z,
                                                    ) == DIRT
                                                {
                                                    target.set_block_at_y(
                                                        local_x,
                                                        storage_y - 1,
                                                        local_z,
                                                        GRASS_BLOCK,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if room {
                            break;
                        }
                    }
                }
            }

            tunnel += 1;
        }
    }
}

fn alpha_floor(value: f64) -> i32 {
    value.floor() as i32
}

fn alpha_sin(value: f32) -> f32 {
    let index = ((value * 10_430.378).trunc() as i32 & 65_535) as usize;
    alpha_sin_table()[index]
}

fn alpha_cos(value: f32) -> f32 {
    let index = ((value * 10_430.378 + 16_384.0).trunc() as i32 & 65_535) as usize;
    alpha_sin_table()[index]
}

fn alpha_sin_table() -> &'static [f32] {
    static SIN_TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    SIN_TABLE
        .get_or_init(|| {
            (0..65_536)
                .map(|index| ((index as f64) * std::f64::consts::TAU / 65_536.0).sin() as f32)
                .collect()
        })
        .as_slice()
}

fn alpha_density_index(x: usize, z: usize, y: usize) -> usize {
    (x * ALPHA_DENSITY_SIZE_XZ + z) * ALPHA_DENSITY_SIZE_Y + y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DIRT, GRASS_BLOCK, STONE};
    use sha2::{Digest, Sha256};

    #[test]
    fn alpha_stage_uses_only_the_historical_active_height() {
        let chunk = generate_alpha_stage_chunk(12_345, 0, 0, false, AlphaGenerationStage::Surface);
        for y in ALPHA_ACTIVE_HEIGHT..ALPHA_BUILD_HEIGHT {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    assert_eq!(chunk.block_at_y(x, y, z).0, AIR);
                }
            }
        }
        assert!(chunk.block_count(STONE) > 0);
        assert!(chunk.block_count(GRASS_BLOCK) > 0);
        assert!(chunk.block_count(DIRT) > 0);
    }

    #[test]
    fn alpha_terrain_and_surface_match_the_reference_probe() {
        let terrain =
            generate_alpha_stage_chunk(12_345, 0, 0, false, AlphaGenerationStage::Terrain);
        assert_eq!(active_block_count(&terrain, AIR), 8_321);
        assert_eq!(active_block_count(&terrain, STONE), 24_447);
        assert_eq!(
            semantic_sha256(&terrain),
            "7442e144fdf4819e3b960d1ece8e0a43ce427e785fc7117354fc1204e5464105"
        );

        let surface =
            generate_alpha_stage_chunk(12_345, 0, 0, false, AlphaGenerationStage::Surface);
        assert_eq!(active_block_count(&surface, AIR), 8_321);
        assert_eq!(active_block_count(&surface, STONE), 22_627);
        assert_eq!(active_block_count(&surface, GRASS_BLOCK), 256);
        assert_eq!(active_block_count(&surface, DIRT), 947);
        assert_eq!(active_block_count(&surface, BEDROCK), 617);
        assert_eq!(
            semantic_sha256(&surface),
            "e07275f18a1e42bce6a078b06f469c01663559bfe3d317413a177a995468ee6f"
        );
    }

    #[test]
    fn alpha_caves_match_the_reference_probe_block_counts() {
        let caves = generate_alpha_stage_chunk(12_345, 0, 0, false, AlphaGenerationStage::Caves);
        assert_eq!(active_block_count(&caves, AIR), 8_652);
        assert_eq!(active_block_count(&caves, STONE), 22_247);
        assert_eq!(active_block_count(&caves, LAVA), 49);
        assert_eq!(
            semantic_sha256(&caves),
            "947b3a034360da67c83baef0fc486fd05f8373d57080abc2fe952662db55e833"
        );
    }

    #[test]
    fn alpha_negative_and_winter_oracle_fixtures_match() {
        let negative =
            generate_alpha_stage_chunk(12_345, -3, 5, false, AlphaGenerationStage::Caves);
        assert_eq!(
            semantic_sha256(&negative),
            "84849bd3df13751903fd519b92eeff0db698fcefc0bb8f4989f38707173c0444"
        );

        let winter = generate_alpha_stage_chunk(12_345, 5, 5, true, AlphaGenerationStage::Terrain);
        assert_eq!(active_block_count(&winter, ICE), 84);
        assert_eq!(
            semantic_sha256(&winter),
            "b6438418692cbe93c555ee442a17500455f8ab49e0db9348f66e89c736a0dc04"
        );
    }

    #[test]
    fn alpha_winter_is_explicit_and_changes_only_the_requested_profile() {
        let mut observed_ice = 0;
        for chunk_x in -8..=8 {
            for chunk_z in -8..=8 {
                let temperate = generate_alpha_stage_chunk(
                    98_765,
                    chunk_x,
                    chunk_z,
                    false,
                    AlphaGenerationStage::Terrain,
                );
                let winter = generate_alpha_stage_chunk(
                    98_765,
                    chunk_x,
                    chunk_z,
                    true,
                    AlphaGenerationStage::Terrain,
                );
                assert_eq!(temperate.block_count(ICE), 0);
                for y in 0..ALPHA_ACTIVE_HEIGHT {
                    for z in 0..CHUNK_WIDTH {
                        for x in 0..CHUNK_WIDTH {
                            let temperate_block = temperate.block_at_y(x, y, z).0;
                            let winter_block = winter.block_at_y(x, y, z).0;
                            if temperate_block != winter_block {
                                assert_eq!(temperate_block, WATER);
                                assert_eq!(winter_block, ICE);
                                observed_ice += 1;
                            }
                        }
                    }
                }
                if observed_ice > 0 {
                    break;
                }
            }
            if observed_ice > 0 {
                break;
            }
        }
        assert!(observed_ice > 0);
    }

    #[test]
    fn alpha_features_add_classic_ores_trees_and_explicit_snow() {
        let temperate = generate_alpha_chunk(12_345, 0, 0, false);
        assert!(temperate.block_count(COAL_ORE) > 0);
        assert!(temperate.block_count(IRON_ORE) > 0);
        assert!(temperate.block_count(crate::block::OAK_LOG) > 0);
        assert_eq!(temperate.block_count(SNOW), 0);

        let winter = generate_alpha_chunk(12_345, 0, 0, true);
        assert!(winter.block_count(SNOW) > 0);
    }

    #[test]
    fn alpha_feature_batches_are_partition_independent() {
        let left = ChunkPos::new(0, 0);
        let right = ChunkPos::new(1, 0);
        let mut batched_cache = AlphaFeatureDependencyCache::new();
        let batched = batched_cache.generate_features_chunks(12_345, false, [right, left]);

        let mut left_cache = AlphaFeatureDependencyCache::new();
        let isolated_left = left_cache.generate_features_chunks(12_345, false, [left]);
        let mut right_cache = AlphaFeatureDependencyCache::new();
        let isolated_right = right_cache.generate_features_chunks(12_345, false, [right]);

        assert_eq!(
            batched.chunks[&left].blocks(),
            isolated_left.chunks[&left].blocks()
        );
        assert_eq!(
            batched.chunks[&right].blocks(),
            isolated_right.chunks[&right].blocks()
        );
    }

    fn active_block_count(chunk: &GeneratedChunk, block: RawBlockId) -> usize {
        let mut count = 0;
        for y in 0..ALPHA_ACTIVE_HEIGHT {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    count += usize::from(chunk.block_at_y(x, y, z).0 == block);
                }
            }
        }
        count
    }

    fn semantic_sha256(chunk: &GeneratedChunk) -> String {
        let digest = Sha256::digest(alpha_semantic_bytes(chunk));
        format!("{digest:x}")
    }
}
