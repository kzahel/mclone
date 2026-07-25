#![forbid(unsafe_code)]

mod session;
#[cfg(target_arch = "wasm32")]
mod web;

pub use session::{WorldExplorerConfig, WorldExplorerSession};
#[cfg(target_arch = "wasm32")]
pub use web::{WebWorldExplorer, mclone_world_explorer_create};
