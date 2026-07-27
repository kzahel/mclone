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
pub use mclone_terrain_view::{
    TerrainRuntimeExactRenderer as ExplorerExactTerrain,
    TerrainRuntimeExactStats as ExplorerExactStats,
};
#[cfg(target_arch = "wasm32")]
pub use web::{WebWorldExplorer, mclone_world_explorer_create};
#[cfg(target_arch = "wasm32")]
pub use web_exact::{WorldExplorerExactWorkerActor, WorldExplorerExactWorkerDispatch};
