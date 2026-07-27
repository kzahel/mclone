#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod native_vegetation;
mod session;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
mod web_exact;
#[cfg(target_arch = "wasm32")]
mod web_vegetation;

pub use mclone_terrain_view::{
    TerrainRuntimeExactRenderer as ExplorerExactTerrain,
    TerrainRuntimeExactStats as ExplorerExactStats,
};
#[cfg(not(target_arch = "wasm32"))]
pub use native_vegetation::NativeTerrainVegetationExecutor;
pub use session::{
    WorldExplorerCompositionMode, WorldExplorerConfig, WorldExplorerExactAnchor,
    WorldExplorerSession,
};
#[cfg(target_arch = "wasm32")]
pub use web::{WebWorldExplorer, mclone_world_explorer_create};
#[cfg(target_arch = "wasm32")]
pub use web_exact::{WorldExplorerExactWorkerActor, WorldExplorerExactWorkerDispatch};
