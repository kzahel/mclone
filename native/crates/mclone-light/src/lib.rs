#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

mod block_engine;
mod block_storage;
mod data_layer;
mod dynamic_graph;
mod key;
mod layer;
mod level_engine;
mod packed;
mod pos;
mod section_storage;
mod sky_engine;
mod sky_storage;
mod storage_map;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type TimingSample = Instant;

#[cfg(target_arch = "wasm32")]
pub(crate) type TimingSample = ();

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timing_start() -> Option<TimingSample> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn timing_start() -> Option<TimingSample> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timing_elapsed_us(start: Option<TimingSample>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn timing_elapsed_us(_start: Option<TimingSample>) -> u128 {
    0
}

pub use block_engine::{BlockLightEngine, BlockLightWorld};
pub use block_storage::BlockLightSectionStorage;
pub use data_layer::{
    DATA_LAYER_SIZE, DATA_LAYER_VALUE_COUNT, DataLayer, DataLayerError, data_layer_index,
};
pub use dynamic_graph::{
    DynamicGraphCallbacks, DynamicGraphMinFixedPoint, DynamicGraphRunReport, NeighborCheck,
};
pub use key::LightChunkKey;
pub use layer::LightLayer;
pub use level_engine::{
    LIGHT_SECTION_PADDING, LevelLightEngine, LevelLightRunReport, LightEngineRunReport,
    LightLayerRunReport, MAX_SOURCE_LEVEL,
};
pub use packed::{
    FULL_BLOCK, FULL_BRIGHT, FULL_SKY, pack_light, packed_block_light,
    packed_light_at_local_block_or_fullbright, packed_light_section_layer, packed_sky_light,
};
pub use pos::{
    BlockPosKey, Direction, LightSectionRange, SectionPosKey, block_pos_as_long,
    block_pos_flat_index, block_pos_get_x, block_pos_get_y, block_pos_get_z, block_pos_offset,
    block_to_section_key, section_as_long, section_get_zero_node, section_offset, section_relative,
    section_to_block_coord, section_x, section_y, section_z,
};
pub use section_storage::{
    EMPTY as EMPTY_SECTION, LIGHT_AND_DATA, LIGHT_ONLY, LayerLightSectionStorage,
    SectionEdgeUpdate, SectionLevelChange,
};
pub use sky_engine::{SkyLightEngine, SkyLightWorld};
pub use sky_storage::{SkyLightSectionStorage, SkySourceUpdate, SkySourceUpdateKind};
pub use storage_map::{BlockDataLayerStorageMap, DataLayerStorageMap, SkyDataLayerStorageMap};
