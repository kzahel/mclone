mod caves;
mod climate;
mod noise;
mod population;

use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use crate::block::{
    AIR, BEDROCK, DIRT, GRASS_BLOCK, GRAVEL, ICE, LAVA, RawBlockId, SAND, SANDSTONE, STONE, WATER,
};
use crate::prng::SimpleRandomSource;

use super::{GeneratedChunk, MutableChunkBlockBuffer};
use caves::carve_beta_caves;
use climate::BetaClimateSource;
use noise::BetaPerlinNoise;

pub use climate::{BetaBiome, BetaClimateRegion, beta_biome_from_climate};
pub use population::{
    BetaFeatureBatchResult, BetaFeatureDependencyCache, BetaFeatureDependencyCacheReport,
    generate_beta_chunk,
};

pub const BETA_BUILD_HEIGHT: i32 = 256;
pub const BETA_ACTIVE_HEIGHT: i32 = 128;
pub const BETA_SEA_LEVEL: i32 = 64;

const BETA_DENSITY_HORIZONTAL_CELLS: usize = 4;
const BETA_DENSITY_VERTICAL_CELLS: usize = 16;
const BETA_DENSITY_SIZE_XZ: usize = BETA_DENSITY_HORIZONTAL_CELLS + 1;
const BETA_DENSITY_SIZE_Y: usize = BETA_DENSITY_VERTICAL_CELLS + 1;
const BETA_BASE_CHUNK_MULTIPLIER_X: i64 = 341_873_128_712;
const BETA_BASE_CHUNK_MULTIPLIER_Z: i64 = 132_897_987_541;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BetaGenerationStage {
    Terrain,
    Surface,
    Caves,
    Features,
}

#[derive(Clone, Debug)]
struct BetaNoiseBanks {
    min_limit: BetaPerlinNoise,
    max_limit: BetaPerlinNoise,
    selector: BetaPerlinNoise,
    surface_mask: BetaPerlinNoise,
    surface_depth: BetaPerlinNoise,
    scale: BetaPerlinNoise,
    depth: BetaPerlinNoise,
    #[allow(dead_code)]
    forest: BetaPerlinNoise,
}

impl BetaNoiseBanks {
    fn new(seed: i64) -> Self {
        let mut random = SimpleRandomSource::new(seed);
        Self {
            min_limit: BetaPerlinNoise::new(&mut random, 16),
            max_limit: BetaPerlinNoise::new(&mut random, 16),
            selector: BetaPerlinNoise::new(&mut random, 8),
            surface_mask: BetaPerlinNoise::new(&mut random, 4),
            surface_depth: BetaPerlinNoise::new(&mut random, 4),
            scale: BetaPerlinNoise::new(&mut random, 10),
            depth: BetaPerlinNoise::new(&mut random, 16),
            forest: BetaPerlinNoise::new(&mut random, 8),
        }
    }

    fn density_lattice(&self, chunk_x: i32, chunk_z: i32, climate: &BetaClimateRegion) -> Vec<f64> {
        let start_x = chunk_x * BETA_DENSITY_HORIZONTAL_CELLS as i32;
        let start_z = chunk_z * BETA_DENSITY_HORIZONTAL_CELLS as i32;
        let horizontal = 684.412;
        let vertical = 684.412;
        let scale_noise = self.scale.region_2d(
            start_x,
            start_z,
            BETA_DENSITY_SIZE_XZ,
            BETA_DENSITY_SIZE_XZ,
            1.121,
            1.121,
        );
        let depth_noise = self.depth.region_2d(
            start_x,
            start_z,
            BETA_DENSITY_SIZE_XZ,
            BETA_DENSITY_SIZE_XZ,
            200.0,
            200.0,
        );
        let selector = self.selector.region(
            f64::from(start_x),
            0.0,
            f64::from(start_z),
            BETA_DENSITY_SIZE_XZ,
            BETA_DENSITY_SIZE_Y,
            BETA_DENSITY_SIZE_XZ,
            horizontal / 80.0,
            vertical / 160.0,
            horizontal / 80.0,
        );
        let min_limit = self.min_limit.region(
            f64::from(start_x),
            0.0,
            f64::from(start_z),
            BETA_DENSITY_SIZE_XZ,
            BETA_DENSITY_SIZE_Y,
            BETA_DENSITY_SIZE_XZ,
            horizontal,
            vertical,
            horizontal,
        );
        let max_limit = self.max_limit.region(
            f64::from(start_x),
            0.0,
            f64::from(start_z),
            BETA_DENSITY_SIZE_XZ,
            BETA_DENSITY_SIZE_Y,
            BETA_DENSITY_SIZE_XZ,
            horizontal,
            vertical,
            horizontal,
        );

        let mut lattice =
            vec![0.0; BETA_DENSITY_SIZE_XZ * BETA_DENSITY_SIZE_Y * BETA_DENSITY_SIZE_XZ];
        let mut density_index = 0;
        let mut column_index = 0;
        let climate_stride = 16 / BETA_DENSITY_SIZE_XZ;
        for local_x in 0..BETA_DENSITY_SIZE_XZ {
            let climate_x = local_x * climate_stride + climate_stride / 2;
            for local_z in 0..BETA_DENSITY_SIZE_XZ {
                let climate_z = local_z * climate_stride + climate_stride / 2;
                let temperature = climate.temperature(climate_x, climate_z);
                let moisture = climate.downfalls[climate_x * 16 + climate_z] * temperature;
                let mut climate_scale = 1.0 - moisture;
                climate_scale *= climate_scale;
                climate_scale *= climate_scale;
                climate_scale = 1.0 - climate_scale;

                let mut terrain_scale = (scale_noise[column_index] + 256.0) / 512.0;
                terrain_scale *= climate_scale;
                if terrain_scale > 1.0 {
                    terrain_scale = 1.0;
                }

                let mut terrain_depth = depth_noise[column_index] / 8000.0;
                if terrain_depth < 0.0 {
                    terrain_depth = -terrain_depth * 0.3;
                }
                terrain_depth = terrain_depth * 3.0 - 2.0;
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
                    terrain_depth /= 8.0;
                }

                if terrain_scale < 0.0 {
                    terrain_scale = 0.0;
                }
                terrain_scale += 0.5;
                terrain_depth = terrain_depth * BETA_DENSITY_SIZE_Y as f64 / 16.0;
                let vertical_center = BETA_DENSITY_SIZE_Y as f64 / 2.0 + terrain_depth * 4.0;
                column_index += 1;

                for local_y in 0..BETA_DENSITY_SIZE_Y {
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
                    if local_y > BETA_DENSITY_SIZE_Y - 4 {
                        let fade = (local_y - (BETA_DENSITY_SIZE_Y - 4)) as f64 / 3.0_f32 as f64;
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

pub fn generate_beta_climate_region(
    seed: i64,
    x: i32,
    z: i32,
    size_x: usize,
    size_z: usize,
) -> BetaClimateRegion {
    BetaClimateSource::new(seed).region(x, z, size_x, size_z)
}

pub fn beta_biome_id(seed: i64, x: i32, z: i32) -> i32 {
    BetaClimateSource::new(seed).region(x, z, 1, 1).biomes[0].native_biome_id()
}

pub fn generate_beta_stage_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    stage: BetaGenerationStage,
) -> GeneratedChunk {
    if stage == BetaGenerationStage::Features {
        return generate_beta_chunk(seed, chunk_x, chunk_z);
    }
    let banks = BetaNoiseBanks::new(seed);
    let climate_source = BetaClimateSource::new(seed);
    let climate = climate_source.region(
        chunk_min_block_coord(chunk_x),
        chunk_min_block_coord(chunk_z),
        CHUNK_WIDTH as usize,
        CHUNK_WIDTH as usize,
    );
    let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, BETA_BUILD_HEIGHT);
    build_beta_terrain(&banks, &climate, &mut chunk);
    if !matches!(stage, BetaGenerationStage::Terrain) {
        build_beta_surfaces(&banks, &climate, &mut chunk);
    }
    if matches!(stage, BetaGenerationStage::Caves) {
        carve_beta_caves(seed, &mut chunk);
    }
    chunk.prime_worldgen_heightmaps();
    GeneratedChunk::from_mutable_buffer_with_biomes(chunk, beta_biome_payload(&climate))
}

fn generate_beta_surface_buffer_with_core(
    banks: &BetaNoiseBanks,
    climate_source: &BetaClimateSource,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> MutableChunkBlockBuffer {
    let climate = climate_source.region(
        chunk_min_block_coord(chunk_x),
        chunk_min_block_coord(chunk_z),
        16,
        16,
    );
    let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, BETA_BUILD_HEIGHT);
    build_beta_terrain(banks, &climate, &mut chunk);
    build_beta_surfaces(banks, &climate, &mut chunk);
    carve_beta_caves(seed, &mut chunk);
    chunk.prime_worldgen_heightmaps();
    chunk
}

fn beta_generated_chunk(
    chunk: MutableChunkBlockBuffer,
    climate_source: &BetaClimateSource,
) -> GeneratedChunk {
    let climate = climate_source.region(
        chunk_min_block_coord(chunk.chunk_x),
        chunk_min_block_coord(chunk.chunk_z),
        16,
        16,
    );
    GeneratedChunk::from_mutable_buffer_with_biomes(chunk, beta_biome_payload(&climate))
}

fn build_beta_terrain(
    banks: &BetaNoiseBanks,
    climate: &BetaClimateRegion,
    chunk: &mut MutableChunkBlockBuffer,
) {
    let lattice = banks.density_lattice(chunk.chunk_x, chunk.chunk_z, climate);
    for cell_x in 0..BETA_DENSITY_HORIZONTAL_CELLS {
        for cell_z in 0..BETA_DENSITY_HORIZONTAL_CELLS {
            for cell_y in 0..BETA_DENSITY_VERTICAL_CELLS {
                let mut d000 = lattice[beta_density_index(cell_x, cell_z, cell_y)];
                let mut d001 = lattice[beta_density_index(cell_x, cell_z + 1, cell_y)];
                let mut d100 = lattice[beta_density_index(cell_x + 1, cell_z, cell_y)];
                let mut d101 = lattice[beta_density_index(cell_x + 1, cell_z + 1, cell_y)];
                let dy000 =
                    (lattice[beta_density_index(cell_x, cell_z, cell_y + 1)] - d000) * 0.125;
                let dy001 =
                    (lattice[beta_density_index(cell_x, cell_z + 1, cell_y + 1)] - d001) * 0.125;
                let dy100 =
                    (lattice[beta_density_index(cell_x + 1, cell_z, cell_y + 1)] - d100) * 0.125;
                let dy101 = (lattice[beta_density_index(cell_x + 1, cell_z + 1, cell_y + 1)]
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
                            let temperature =
                                climate.temperature(local_x as usize, local_z as usize);
                            let block = if density > 0.0 {
                                STONE
                            } else if y < BETA_SEA_LEVEL {
                                if temperature < 0.5 && y >= BETA_SEA_LEVEL - 1 {
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

fn build_beta_surfaces(
    banks: &BetaNoiseBanks,
    climate: &BetaClimateRegion,
    chunk: &mut MutableChunkBlockBuffer,
) {
    let chunk_seed = i64::from(chunk.chunk_x)
        .wrapping_mul(BETA_BASE_CHUNK_MULTIPLIER_X)
        .wrapping_add(i64::from(chunk.chunk_z).wrapping_mul(BETA_BASE_CHUNK_MULTIPLIER_Z));
    let mut random = SimpleRandomSource::new(chunk_seed);
    let start_x = f64::from(chunk_min_block_coord(chunk.chunk_x));
    let start_z = f64::from(chunk_min_block_coord(chunk.chunk_z));
    let mask_scale = 0.03125;
    let sand = banks.surface_mask.region(
        start_x, start_z, 0.0, 16, 16, 1, mask_scale, mask_scale, 1.0,
    );
    let gravel = banks.surface_mask.region(
        start_x, 109.0134, start_z, 16, 1, 16, mask_scale, 1.0, mask_scale,
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

    // Beta's raw loop is Z-major, then X-major. That order is observable
    // because every column consumes random values during the bedrock scan.
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let mask_index = (local_z + local_x * CHUNK_WIDTH) as usize;
            let biome = climate.biome(local_x as usize, local_z as usize);
            let sand_column = sand[mask_index] + random.next_double() * 0.2 > 0.0;
            let gravel_column = gravel[mask_index] + random.next_double() * 0.2 > 3.0;
            let layer_depth = (depth[mask_index] / 3.0 + 3.0 + random.next_double() * 0.25) as i32;
            let mut remaining = -1;
            let (biome_top, biome_filler) = if biome.is_desert() {
                (SAND, SAND)
            } else {
                (GRASS_BLOCK, DIRT)
            };
            let mut top = biome_top;
            let mut filler = biome_filler;

            for y in (0..BETA_ACTIVE_HEIGHT).rev() {
                if y <= random.next_int_bound(5) {
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
                        } else if (BETA_SEA_LEVEL - 4..=BETA_SEA_LEVEL + 1).contains(&y) {
                            top = biome_top;
                            filler = biome_filler;
                            if gravel_column {
                                top = AIR;
                                filler = GRAVEL;
                            }
                            if sand_column {
                                top = SAND;
                                filler = SAND;
                            }
                        }
                        if y < BETA_SEA_LEVEL && top == AIR {
                            top = WATER;
                        }
                        remaining = layer_depth;
                        chunk.set_block_at_y(
                            local_x,
                            y,
                            local_z,
                            if y >= BETA_SEA_LEVEL - 1 { top } else { filler },
                        );
                    } else if remaining > 0 {
                        remaining -= 1;
                        chunk.set_block_at_y(local_x, y, local_z, filler);
                        if remaining == 0 && filler == SAND {
                            remaining = random.next_int_bound(4);
                            filler = SANDSTONE;
                        }
                    }
                }
            }
        }
    }
}

fn beta_biome_payload(climate: &BetaClimateRegion) -> Vec<i32> {
    let quart_width = CHUNK_WIDTH / 4;
    let quart_height = BETA_BUILD_HEIGHT / 4;
    let mut biomes = Vec::with_capacity((quart_width * quart_width * quart_height) as usize);
    for _quart_y in 0..quart_height {
        for quart_z in 0..quart_width {
            for quart_x in 0..quart_width {
                let local_x = (quart_x * 4 + 2) as usize;
                let local_z = (quart_z * 4 + 2) as usize;
                biomes.push(climate.biome(local_x, local_z).native_biome_id());
            }
        }
    }
    biomes
}

/// Convert a native Beta chunk into Beta 1.7.3's X/Z/Y byte order.
pub fn beta_semantic_bytes(chunk: &GeneratedChunk) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((CHUNK_WIDTH * CHUNK_WIDTH * BETA_ACTIVE_HEIGHT) as usize);
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            for y in 0..BETA_ACTIVE_HEIGHT {
                let native = chunk.block_at_y(x, y, z).0;
                bytes.push(beta_semantic_block_id(native).unwrap_or_else(|| {
                    panic!("native block {native} has no Beta semantic mapping")
                }));
            }
        }
    }
    bytes
}

pub const fn beta_semantic_block_id(native: RawBlockId) -> Option<u8> {
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
        SANDSTONE => Some(24),
        ICE => Some(79),
        crate::block::GOLD_ORE => Some(14),
        crate::block::IRON_ORE => Some(15),
        crate::block::COAL_ORE => Some(16),
        crate::block::OAK_LOG => Some(17),
        crate::block::SPRUCE_LOG => Some(17),
        crate::block::BIRCH_LOG => Some(17),
        crate::block::OAK_LEAVES => Some(18),
        crate::block::SPRUCE_LEAVES => Some(18),
        crate::block::BIRCH_LEAVES => Some(18),
        crate::block::DANDELION => Some(37),
        crate::block::POPPY => Some(38),
        crate::block::BROWN_MUSHROOM => Some(39),
        crate::block::RED_MUSHROOM => Some(40),
        crate::block::DIAMOND_ORE => Some(56),
        crate::block::REDSTONE_ORE => Some(73),
        crate::block::SNOW => Some(78),
        crate::block::CACTUS => Some(81),
        crate::block::CLAY => Some(82),
        crate::block::SUGAR_CANE => Some(83),
        crate::block::PUMPKIN => Some(86),
        crate::block::LAPIS_ORE => Some(21),
        crate::block::GRASS => Some(31),
        crate::block::FERN => Some(31),
        crate::block::DEAD_BUSH => Some(32),
        crate::block::CAVE_AIR => Some(0),
        _ => None,
    }
}

fn beta_density_index(x: usize, z: usize, y: usize) -> usize {
    (x * BETA_DENSITY_SIZE_XZ + z) * BETA_DENSITY_SIZE_Y + y
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn beta_origin_climate_matches_the_reference_probe() {
        let climate = generate_beta_climate_region(12_345, 0, 0, 16, 16);
        let biome_bytes = climate
            .biomes
            .iter()
            .map(|biome| biome.semantic_id())
            .collect::<Vec<_>>();
        assert_eq!(
            hex_digest(&biome_bytes),
            "62e467d88bd1a744bbe69de118c40f38720d2398f9b9b12b00d04b9eb5eb7f46"
        );
        assert_eq!(
            hex_digest(&double_bytes(&climate.temperatures)),
            "0e6845828c0fc3b51155032c1b2401cf0665a8c1cd8b6f94477ffea4b3ccb991"
        );
        assert_eq!(
            hex_digest(&double_bytes(&climate.downfalls)),
            "e5a00aa9991ac8a5ee3109844d84a55583bd20572ad3ffcd42792f3c36b183ad"
        );
        assert_eq!(
            climate
                .biomes
                .iter()
                .filter(|biome| **biome == BetaBiome::Savanna)
                .count(),
            122
        );
        assert_eq!(
            climate
                .biomes
                .iter()
                .filter(|biome| **biome == BetaBiome::Desert)
                .count(),
            134
        );
    }

    #[test]
    fn beta_origin_terrain_and_surface_match_the_reference_probe() {
        let terrain = generate_beta_stage_chunk(12_345, 0, 0, BetaGenerationStage::Terrain);
        assert_eq!(terrain.block_count(AIR), 11_168 + 128 * 16 * 16);
        assert_eq!(terrain.block_count(STONE), 21_600);
        assert_eq!(
            semantic_sha256(&terrain),
            "790e4588b113757ea26e75a60dc6cb4b966e7732387f36e57753d2a894986a88"
        );

        let surface = generate_beta_stage_chunk(12_345, 0, 0, BetaGenerationStage::Surface);
        assert_eq!(surface.block_count(GRASS_BLOCK), 122);
        assert_eq!(surface.block_count(SAND), 625);
        assert_eq!(surface.block_count(SANDSTONE), 198);
        assert_eq!(surface.block_count(BEDROCK), 780);
        assert_eq!(
            semantic_sha256(&surface),
            "a8d110df56e2cc10e57aa943427f9307be8f04e7a68e8fb0fa04f4e839487ce2"
        );
    }

    #[test]
    fn beta_caves_and_mixed_sign_chunk_match_the_reference_probe() {
        let caves = generate_beta_stage_chunk(12_345, 0, 0, BetaGenerationStage::Caves);
        assert_eq!(caves.block_count(LAVA), 31);
        assert_eq!(
            semantic_sha256(&caves),
            "15d17bb7e95e712fb64d797583a1a00a2f511422381f1f0f589e3b6ae3e0f930"
        );

        let negative = generate_beta_stage_chunk(12_345, -3, 5, BetaGenerationStage::Caves);
        assert_eq!(
            semantic_sha256(&negative),
            "8eb348ee2438cbd02aa6ce58701cefe9b91db92d920154df1bacc7a949844a2c"
        );
    }

    #[test]
    fn beta_population_adds_the_representative_ore_set() {
        let chunk = generate_beta_chunk(12_345, 0, 0);
        assert!(chunk.block_count(crate::block::COAL_ORE) > 0);
        assert!(chunk.block_count(crate::block::IRON_ORE) > 0);
        assert!(chunk.block_count(crate::block::GOLD_ORE) > 0);
        assert!(chunk.block_count(crate::block::REDSTONE_ORE) > 0);
        assert!(chunk.block_count(crate::block::DIAMOND_ORE) > 0);
        assert!(chunk.block_count(crate::block::LAPIS_ORE) > 0);
    }

    #[test]
    fn beta_feature_batches_are_partition_independent() {
        let left = mclone_core::ChunkPos::new(0, 0);
        let right = mclone_core::ChunkPos::new(1, 0);
        let mut batched_cache = BetaFeatureDependencyCache::new();
        let batched = batched_cache.generate_features_chunks(12_345, [right, left]);

        let mut left_cache = BetaFeatureDependencyCache::new();
        let isolated_left = left_cache.generate_features_chunks(12_345, [left]);
        let mut right_cache = BetaFeatureDependencyCache::new();
        let isolated_right = right_cache.generate_features_chunks(12_345, [right]);

        assert_eq!(
            batched.chunks[&left].blocks(),
            isolated_left.chunks[&left].blocks()
        );
        assert_eq!(
            batched.chunks[&right].blocks(),
            isolated_right.chunks[&right].blocks()
        );
    }

    fn semantic_sha256(chunk: &GeneratedChunk) -> String {
        hex_digest(&beta_semantic_bytes(chunk))
    }

    fn double_bytes(values: &[f64]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect()
    }

    fn hex_digest(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        format!("{digest:x}")
    }
}
