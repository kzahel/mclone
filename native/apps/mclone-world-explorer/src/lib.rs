#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
mod web_exact;

#[cfg(not(target_arch = "wasm32"))]
pub use mclone_terrain_view::NativeTerrainVegetationExecutor;
pub use mclone_terrain_view::{
    TerrainRuntimeCompositionMode as WorldExplorerCompositionMode,
    TerrainRuntimeConfig as WorldExplorerConfig,
    TerrainRuntimeExactAnchor as WorldExplorerExactAnchor,
    TerrainRuntimeSession as WorldExplorerSession,
};

pub const WORLD_EXPLORER_TERRAIN_FRONTIER: &str = "single-owner-exact-smooth-stitch";
pub use mclone_terrain_view::{
    TerrainRuntimeExactRenderer as ExplorerExactTerrain,
    TerrainRuntimeExactStats as ExplorerExactStats,
};

pub const fn world_explorer_vegetation_enabled(
    profile: mclone_worldgen::terrain_preview::TerrainPreviewProfile,
) -> bool {
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    matches!(
        profile,
        TerrainPreviewProfile::McloneOverworldV1
            | TerrainPreviewProfile::ContinentalEcoregionCandidate
            | TerrainPreviewProfile::McloneOverworldV2
            | TerrainPreviewProfile::McloneOverworldV3
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    #[test]
    fn every_explorer_source_with_tree_records_enables_vegetation() {
        for profile in [
            TerrainPreviewProfile::McloneOverworldV1,
            TerrainPreviewProfile::ContinentalEcoregionCandidate,
            TerrainPreviewProfile::McloneOverworldV2,
            TerrainPreviewProfile::McloneOverworldV3,
        ] {
            assert!(world_explorer_vegetation_enabled(profile));
        }
        assert!(!world_explorer_vegetation_enabled(
            TerrainPreviewProfile::VanillaOverworld
        ));
    }
}
#[cfg(target_arch = "wasm32")]
pub use web::{WebWorldExplorer, mclone_world_explorer_create};
#[cfg(target_arch = "wasm32")]
pub use web_exact::{WorldExplorerExactWorkerActor, WorldExplorerExactWorkerDispatch};
