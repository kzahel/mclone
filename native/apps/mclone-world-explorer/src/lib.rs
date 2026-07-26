#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod native_vegetation;
mod session;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native_vegetation::NativeTerrainVegetationExecutor;
pub use session::{WorldExplorerConfig, WorldExplorerSession};
#[cfg(target_arch = "wasm32")]
pub use web::{WebWorldExplorer, mclone_world_explorer_create};
