use std::sync::OnceLock;

use mclone_core::chunk_min_block_coord;

use crate::biome::get_layered_biome_by_id;
use crate::block::{DANDELION, GRASS, POPPY};
use crate::feature::{
    BasicTreeConfiguration, FeatureRegion, FeatureWorld, PlacedFeature,
    apply_feature_table_to_region_timed, flower_patch, grass_patch, tree_feature,
};
use crate::levelgen::profile::PLAINS_BIOME_ID;
use crate::noise::SeedDomain;

use super::biomes::{MCLONE_OVERWORLD_FOREST_BIOME_ID, mclone_overworld_biome_id};

pub const MCLONE_OVERWORLD_DECORATION_REVISION: &str = "mclone-overworld-v1-decoration-1";

const MCLONE_OVERWORLD_DECORATION_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6465_6331);

pub(super) fn decorate_mclone_overworld_center(seed: i64, region: &mut FeatureRegion) {
    let min_x = chunk_min_block_coord(region.center_chunk_x());
    let min_z = chunk_min_block_coord(region.center_chunk_z());
    let biome_id = mclone_overworld_biome_id(seed, min_x + 8, min_z + 8);
    let features = feature_table(biome_id);
    if features.is_empty() {
        return;
    }
    apply_feature_table_to_region_timed(
        MCLONE_OVERWORLD_DECORATION_DOMAIN.derive(seed),
        get_layered_biome_by_id(biome_id),
        features,
        region,
    );
}

fn feature_table(biome_id: i32) -> &'static [PlacedFeature] {
    match biome_id {
        PLAINS_BIOME_ID => open_lowland_features(),
        MCLONE_OVERWORLD_FOREST_BIOME_ID => wooded_upland_features(),
        _ => &[],
    }
}

fn open_lowland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                tree_feature(BasicTreeConfiguration::oak(), 0, 0.20, 1),
                grass_patch(GRASS, 4),
                flower_patch(DANDELION, 1),
                flower_patch(POPPY, 1),
            ]
        })
        .as_slice()
}

fn wooded_upland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                tree_feature(BasicTreeConfiguration::oak(), 4, 0.35, 1),
                grass_patch(GRASS, 2),
                flower_patch(DANDELION, 1),
                flower_patch(POPPY, 1),
            ]
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID};

    #[test]
    fn mclone_tables_decorate_only_open_and_wooded_land() {
        assert_eq!(feature_table(PLAINS_BIOME_ID).len(), 4);
        assert_eq!(feature_table(MCLONE_OVERWORLD_FOREST_BIOME_ID).len(), 4);
        assert!(feature_table(OCEAN_BIOME_ID).is_empty());
        assert!(feature_table(BEACH_BIOME_ID).is_empty());
    }
}
