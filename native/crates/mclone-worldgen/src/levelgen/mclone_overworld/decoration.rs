use std::sync::OnceLock;

use mclone_core::chunk_min_block_coord;

use crate::biome::get_layered_biome_by_id;
use crate::block::{
    ANDESITE, COARSE_DIRT, COBBLESTONE, DANDELION, DIRT, FERN, GRASS, GRASS_BLOCK, GRAVEL,
    LARGE_FERN_LOWER, MOSSY_COBBLESTONE, POPPY, RawBlockId, SAND, STONE, SWEET_BERRY_BUSH,
    TALL_GRASS_LOWER, is_water,
};
use crate::feature::{
    ConfiguredFeature, DecorationStep, FeatureRegion, FeatureWorld, PlacedFeature,
    RandomFeatureConfiguration, RandomPatchConfiguration, RandomPatchStateProvider,
    TreeConfiguration, WeightedConfiguredFeature,
    apply_feature_table_to_region_with_index_offset_timed, flower_patch, grass_patch,
};
use crate::levelgen::profile::PLAINS_BIOME_ID;
use crate::noise::SeedDomain;
use crate::placement::{BlockPos, ConfiguredDecorator, HeightmapType};
use crate::prng::WorldgenRandom;

use super::biomes::{
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
    MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID, MCLONE_OVERWORLD_TAIGA_BIOME_ID,
    McloneOverworldSteppeBand, mclone_overworld_biome_id_for_sample, mclone_overworld_steppe_band,
};
use super::fields::{McloneOverworldSampler, McloneOverworldSamplingTopology};

pub const MCLONE_OVERWORLD_DECORATION_REVISION: &str = "mclone-overworld-v1-decoration-13";

const MCLONE_OVERWORLD_DECORATION_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6465_6331);
const MCLONE_OVERWORLD_RIVER_ROCK_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_726f_636b);
const RIVER_ROCK_WATER_SITE_LIMIT: usize = 12;
const RIVER_ROCK_BANK_OFFSETS: [(i32, i32); 16] = [
    (4, 0),
    (-4, 0),
    (0, 4),
    (0, -4),
    (5, 2),
    (5, -2),
    (-5, 2),
    (-5, -2),
    (2, 5),
    (2, -5),
    (-2, 5),
    (-2, -5),
    (6, 3),
    (6, -3),
    (-6, 3),
    (-6, -3),
];

pub(super) fn decorate_mclone_overworld_center(seed: i64, region: &mut FeatureRegion) {
    decorate_mclone_overworld_center_with_topology(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        region,
    );
}

pub(super) fn decorate_mclone_overworld_center_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    region: &mut FeatureRegion,
) {
    let min_x = chunk_min_block_coord(region.decoration_chunk_x());
    let min_z = chunk_min_block_coord(region.decoration_chunk_z());
    let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
    let landform = sampler.sample_landform(min_x + 8, min_z + 8);
    place_sparse_watercourse_rock(seed, region, sampler);
    let biome_id = mclone_overworld_biome_id_for_sample(landform);
    let features = feature_table(
        biome_id,
        mclone_overworld_steppe_band(landform.terrain.climate),
    );
    if features.is_empty() {
        return;
    }
    let preserved_tree_index = i32::from(matches!(
        biome_id,
        PLAINS_BIOME_ID | MCLONE_OVERWORLD_FOREST_BIOME_ID
    ));
    apply_feature_table_to_region_with_index_offset_timed(
        MCLONE_OVERWORLD_DECORATION_DOMAIN.derive(seed),
        get_layered_biome_by_id(biome_id),
        features,
        region,
        preserved_tree_index,
    );
}

fn place_sparse_watercourse_rock(
    seed: i64,
    region: &mut FeatureRegion,
    sampler: McloneOverworldSampler,
) {
    let identity_min_x = chunk_min_block_coord(region.decoration_chunk_x());
    let identity_min_z = chunk_min_block_coord(region.decoration_chunk_z());
    let work_min_x = chunk_min_block_coord(region.center_chunk_x());
    let work_min_z = chunk_min_block_coord(region.center_chunk_z());
    let mut random = WorldgenRandom::default();
    random.set_decoration_seed(
        MCLONE_OVERWORLD_RIVER_ROCK_DOMAIN.derive(seed),
        identity_min_x,
        identity_min_z,
    );
    let mut water_sites = Vec::new();
    for local_z in (1..16).step_by(2) {
        for local_x in (1..16).step_by(2) {
            let x = work_min_x + local_x;
            let z = work_min_z + local_z;
            let Some(ocean_floor_y) = region.height_at(HeightmapType::OceanFloorWg, x, z) else {
                continue;
            };
            let Some(world_surface_y) = region.height_at(HeightmapType::WorldSurfaceWg, x, z)
            else {
                continue;
            };
            if world_surface_y > ocean_floor_y
                && region
                    .block_at_world(BlockPos::new(x, world_surface_y - 1, z))
                    .is_some_and(is_water)
            {
                water_sites.push((x, world_surface_y, z));
            }
        }
    }
    if water_sites.is_empty() {
        return;
    }
    if random.next_int_bound(2) != 0 {
        return;
    }
    let water_start = random.next_int_bound(water_sites.len() as i32) as usize;
    let offset_start = random.next_int_bound(RIVER_ROCK_BANK_OFFSETS.len() as i32) as usize;
    for water_offset in 0..water_sites.len().min(RIVER_ROCK_WATER_SITE_LIMIT) {
        let (water_x, water_surface_y, water_z) =
            water_sites[(water_start + water_offset) % water_sites.len()];
        for bank_offset in 0..RIVER_ROCK_BANK_OFFSETS.len() {
            let (dx, dz) = RIVER_ROCK_BANK_OFFSETS
                [(offset_start + bank_offset) % RIVER_ROCK_BANK_OFFSETS.len()];
            let x = water_x + dx;
            let z = water_z + dz;
            let Some(ground_y) = region.height_at(HeightmapType::OceanFloorWg, x, z) else {
                continue;
            };
            let Some(world_surface_y) = region.height_at(HeightmapType::WorldSurfaceWg, x, z)
            else {
                continue;
            };
            if ground_y != world_surface_y || (ground_y - water_surface_y).abs() > 4 {
                continue;
            }
            let support = region.block_at_world(BlockPos::new(x, ground_y - 1, z));
            if !support.is_some_and(|block| {
                matches!(
                    block,
                    GRASS_BLOCK | DIRT | COARSE_DIRT | STONE | ANDESITE | SAND | GRAVEL
                )
            }) {
                continue;
            }
            let terrain = sampler.sample(x, z);
            if terrain.continentalness <= 0.0 {
                continue;
            }
            let state = if terrain.climate.moisture >= 0.18 && random.next_boolean() {
                MOSSY_COBBLESTONE
            } else if random.next_int_bound(4) == 0 {
                ANDESITE
            } else {
                COBBLESTONE
            };
            if place_watercourse_rock_blob(
                region,
                &mut random,
                BlockPos::new(x, ground_y, z),
                state,
            ) {
                return;
            }
        }
    }
}

fn place_watercourse_rock_blob(
    region: &mut FeatureRegion,
    random: &mut WorldgenRandom,
    origin: BlockPos,
    state: RawBlockId,
) -> bool {
    let mut center = origin;
    let mut placed = false;
    for _ in 0..3 {
        let radius_x = random.next_int_bound(2);
        let radius_y = random.next_int_bound(2);
        let radius_z = random.next_int_bound(2);
        let radius = (radius_x + radius_y + radius_z) as f32 * 0.333 + 0.5;
        let radius_squared = radius * radius;
        for x in center.x - radius_x..=center.x + radius_x {
            for y in center.y - radius_y..=center.y + radius_y {
                for z in center.z - radius_z..=center.z + radius_z {
                    let dx = x as f32 + 0.5 - center.x as f32;
                    let dy = y as f32 + 0.5 - center.y as f32;
                    let dz = z as f32 + 0.5 - center.z as f32;
                    if dx * dx + dy * dy + dz * dz > radius_squared {
                        continue;
                    }
                    let pos = BlockPos::new(x, y, z);
                    if region.block_at_world(pos).is_some_and(is_water) {
                        continue;
                    }
                    placed |= region.set_block_world(pos, state);
                }
            }
        }
        center = BlockPos::new(
            center.x - 1 + random.next_int_bound(2),
            center.y - random.next_int_bound(2),
            center.z - 1 + random.next_int_bound(2),
        );
    }
    placed
}

fn feature_table(
    biome_id: i32,
    steppe_band: McloneOverworldSteppeBand,
) -> &'static [PlacedFeature] {
    match biome_id {
        PLAINS_BIOME_ID => open_lowland_features(),
        MCLONE_OVERWORLD_FOREST_BIOME_ID => wooded_upland_features(),
        MCLONE_OVERWORLD_TAIGA_BIOME_ID => cool_wet_conifer_features(),
        MCLONE_OVERWORLD_SAVANNA_BIOME_ID => match steppe_band {
            McloneOverworldSteppeBand::Core => warm_dry_steppe_core_features(),
            McloneOverworldSteppeBand::Shoulder => warm_dry_steppe_shoulder_features(),
            McloneOverworldSteppeBand::Outside => &[],
        },
        MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID => &[],
        _ => &[],
    }
}

fn open_lowland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                grass_patch(GRASS, 4),
                occasional_flower_patch(DANDELION, 3),
                occasional_flower_patch(POPPY, 5),
            ]
        })
        .as_slice()
}

fn wooded_upland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                grass_patch(GRASS, 2),
                occasional_flower_patch(DANDELION, 4),
                occasional_flower_patch(POPPY, 6),
            ]
        })
        .as_slice()
}

fn cool_wet_conifer_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                conifer_tree_feature(),
                grass_patch(FERN, 3),
                grass_patch(GRASS, 1),
                rare_large_fern_patch(),
                rare_berry_patch(),
            ]
        })
        .as_slice()
}

fn warm_dry_steppe_core_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                acacia_tree_feature(1, 0.25),
                tall_grass_patch(2),
                grass_patch(GRASS, 5),
                occasional_flower_patch(DANDELION, 5),
                occasional_flower_patch(POPPY, 8),
            ]
        })
        .as_slice()
}

fn warm_dry_steppe_shoulder_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                acacia_tree_feature(0, 0.45),
                tall_grass_patch(1),
                grass_patch(GRASS, 5),
                occasional_flower_patch(DANDELION, 6),
                occasional_flower_patch(POPPY, 9),
            ]
        })
        .as_slice()
}

fn conifer_tree_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [WeightedConfiguredFeature::new(
                ConfiguredFeature::tree(TreeConfiguration::pine()),
                0.36,
            )],
            ConfiguredFeature::tree(TreeConfiguration::spruce()),
        )),
        tree_decorators(6, 0.2, 1),
    )
}

fn acacia_tree_feature(count: i32, extra_chance: f32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::tree(TreeConfiguration::acacia()),
        tree_decorators(count, extra_chance, 1),
    )
}

fn tree_decorators(count: i32, extra_chance: f32, extra_count: i32) -> Vec<ConfiguredDecorator> {
    vec![
        ConfiguredDecorator::count_extra(count, extra_chance, extra_count),
        ConfiguredDecorator::square(),
        ConfiguredDecorator::water_depth_threshold(0),
        ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
    ]
}

fn tall_grass_patch(count: i32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: TALL_GRASS_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 48,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT],
        }),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn rare_large_fern_patch() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 32,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT],
        }),
        vec![
            ConfiguredDecorator::chance(3),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn rare_berry_patch() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: SWEET_BERRY_BUSH,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        }),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ],
    )
}

fn occasional_flower_patch(block_id: RawBlockId, rarity: i32) -> PlacedFeature {
    let mut patch = flower_patch(block_id, 1);
    patch
        .decorators
        .insert(0, ConfiguredDecorator::chance(rarity));
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID};

    #[test]
    fn mclone_tables_own_temperate_conifer_and_steppe_language() {
        assert_eq!(
            feature_table(PLAINS_BIOME_ID, McloneOverworldSteppeBand::Outside).len(),
            3
        );
        assert_eq!(
            feature_table(
                MCLONE_OVERWORLD_FOREST_BIOME_ID,
                McloneOverworldSteppeBand::Outside,
            )
            .len(),
            3
        );
        assert_eq!(
            feature_table(
                MCLONE_OVERWORLD_TAIGA_BIOME_ID,
                McloneOverworldSteppeBand::Outside,
            )
            .len(),
            5
        );
        assert_eq!(
            feature_table(
                MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
                McloneOverworldSteppeBand::Core,
            )
            .len(),
            5
        );
        assert_eq!(
            feature_table(
                MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
                McloneOverworldSteppeBand::Shoulder,
            )
            .len(),
            5
        );
        assert!(
            feature_table(
                MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
                McloneOverworldSteppeBand::Outside,
            )
            .is_empty()
        );
        assert!(
            feature_table(
                MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID,
                McloneOverworldSteppeBand::Outside,
            )
            .is_empty()
        );
        assert!(feature_table(OCEAN_BIOME_ID, McloneOverworldSteppeBand::Outside).is_empty());
        assert!(feature_table(BEACH_BIOME_ID, McloneOverworldSteppeBand::Outside).is_empty());
    }
}
