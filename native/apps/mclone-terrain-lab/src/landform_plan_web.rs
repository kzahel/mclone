use mclone_worldgen::landform_plan::{
    MCLONE_LANDFORM_PLAN_CELL_BLOCKS, MCLONE_LANDFORM_PLAN_SCHEMA_REVISION,
    McloneLandformPlanSummary,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(js_name = LandformPlanCompiler)]
pub struct TerrainLabLandformPlanCompiler {
    summary: McloneLandformPlanSummary,
}

#[wasm_bindgen(js_class = LandformPlanCompiler)]
impl TerrainLabLandformPlanCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String) -> Result<TerrainLabLandformPlanCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            summary: McloneLandformPlanSummary::build_plane(seed),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn schema(&self) -> String {
        MCLONE_LANDFORM_PLAN_SCHEMA_REVISION.to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> String {
        self.summary.seed.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn topology(&self) -> String {
        "plane".to_owned()
    }

    #[wasm_bindgen(getter, js_name = minX)]
    pub fn min_x(&self) -> i32 {
        self.summary.min_x
    }

    #[wasm_bindgen(getter, js_name = minZ)]
    pub fn min_z(&self) -> i32 {
        self.summary.min_z
    }

    #[wasm_bindgen(getter, js_name = cellBlocks)]
    pub fn cell_blocks(&self) -> i32 {
        MCLONE_LANDFORM_PLAN_CELL_BLOCKS
    }

    #[wasm_bindgen(getter, js_name = widthCells)]
    pub fn width_cells(&self) -> u32 {
        u32::from(self.summary.width_cells)
    }

    #[wasm_bindgen(getter, js_name = depthCells)]
    pub fn depth_cells(&self) -> u32 {
        u32::from(self.summary.depth_cells)
    }

    #[wasm_bindgen(getter, js_name = basinIds)]
    pub fn basin_ids(&self) -> js_sys::Uint16Array {
        js_sys::Uint16Array::from(self.summary.basin_ids.as_slice())
    }

    #[wasm_bindgen(getter, js_name = receiverIds)]
    pub fn receiver_ids(&self) -> js_sys::Uint32Array {
        js_sys::Uint32Array::from(self.summary.receiver_ids.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn accumulation(&self) -> js_sys::Uint32Array {
        js_sys::Uint32Array::from(self.summary.accumulation.as_slice())
    }

    #[wasm_bindgen(getter, js_name = streamOrder)]
    pub fn stream_order(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.summary.stream_order.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn flags(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.summary.flags.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn uplift(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.summary.uplift.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn quiet(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.summary.quiet.as_slice())
    }

    #[wasm_bindgen(getter, js_name = broadLow)]
    pub fn broad_low(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.summary.broad_low.as_slice())
    }

    #[wasm_bindgen(getter, js_name = baseY)]
    pub fn base_y(&self) -> js_sys::Int16Array {
        js_sys::Int16Array::from(self.summary.base_y.as_slice())
    }

    #[wasm_bindgen(getter, js_name = segmentCoordinates)]
    pub fn segment_coordinates(&self) -> js_sys::Float32Array {
        let coordinates = self
            .summary
            .segments
            .iter()
            .flat_map(|segment| [segment.a_x, segment.a_z, segment.b_x, segment.b_z])
            .collect::<Vec<_>>();
        js_sys::Float32Array::from(coordinates.as_slice())
    }

    #[wasm_bindgen(getter, js_name = segmentKinds)]
    pub fn segment_kinds(&self) -> js_sys::Uint8Array {
        let kinds = self
            .summary
            .segments
            .iter()
            .map(|segment| segment.kind as u8)
            .collect::<Vec<_>>();
        js_sys::Uint8Array::from(kinds.as_slice())
    }

    #[wasm_bindgen(getter, js_name = segmentOrders)]
    pub fn segment_orders(&self) -> js_sys::Uint8Array {
        let orders = self
            .summary
            .segments
            .iter()
            .map(|segment| segment.order)
            .collect::<Vec<_>>();
        js_sys::Uint8Array::from(orders.as_slice())
    }

    #[wasm_bindgen(getter, js_name = segmentAccumulation)]
    pub fn segment_accumulation(&self) -> js_sys::Uint32Array {
        let accumulation = self
            .summary
            .segments
            .iter()
            .map(|segment| segment.accumulation)
            .collect::<Vec<_>>();
        js_sys::Uint32Array::from(accumulation.as_slice())
    }

    #[wasm_bindgen(getter, js_name = sinkCoordinates)]
    pub fn sink_coordinates(&self) -> js_sys::Int32Array {
        let coordinates = self
            .summary
            .sinks
            .iter()
            .flat_map(|sink| [sink.world_x, sink.world_z])
            .collect::<Vec<_>>();
        js_sys::Int32Array::from(coordinates.as_slice())
    }

    #[wasm_bindgen(getter, js_name = sinkKinds)]
    pub fn sink_kinds(&self) -> js_sys::Uint8Array {
        let kinds = self
            .summary
            .sinks
            .iter()
            .map(|sink| sink.kind as u8)
            .collect::<Vec<_>>();
        js_sys::Uint8Array::from(kinds.as_slice())
    }

    #[wasm_bindgen(getter, js_name = sinkIds)]
    pub fn sink_ids(&self) -> js_sys::Uint16Array {
        let ids = self
            .summary
            .sinks
            .iter()
            .map(|sink| sink.id)
            .collect::<Vec<_>>();
        js_sys::Uint16Array::from(ids.as_slice())
    }

    #[wasm_bindgen(getter, js_name = sinkLevels)]
    pub fn sink_levels(&self) -> js_sys::Float32Array {
        let levels = self
            .summary
            .sinks
            .iter()
            .flat_map(|sink| {
                [
                    sink.source_level,
                    sink.spill_level.unwrap_or(f32::NAN),
                    sink.contributing_cells as f32,
                ]
            })
            .collect::<Vec<_>>();
        js_sys::Float32Array::from(levels.as_slice())
    }

    #[wasm_bindgen(getter, js_name = channelCells)]
    pub fn channel_cells(&self) -> u32 {
        self.summary.metrics.channel_cells
    }

    #[wasm_bindgen(getter)]
    pub fn confluences(&self) -> u32 {
        self.summary.metrics.confluences
    }

    #[wasm_bindgen(getter, js_name = drainageSegments)]
    pub fn drainage_segments(&self) -> u32 {
        self.summary.metrics.drainage_segments
    }

    #[wasm_bindgen(getter, js_name = divideSegments)]
    pub fn divide_segments(&self) -> u32 {
        self.summary.metrics.divide_segments
    }

    #[wasm_bindgen(getter, js_name = protectedSinks)]
    pub fn protected_sinks(&self) -> u32 {
        self.summary.metrics.protected_sinks
    }

    #[wasm_bindgen(getter, js_name = approximateBytes)]
    pub fn approximate_bytes(&self) -> u32 {
        self.summary.approximate_bytes().min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter)]
    pub fn checksum(&self) -> String {
        format!("{:016x}", self.summary.checksum)
    }

    pub fn inspect(&self, world_x: f64, world_z: f64) -> Result<String, JsValue> {
        let receipt =
            self.summary
                .point(world_x, world_z)
                .map(|point| TerrainLabLandformPlanPointReceipt {
                    world_x: point.world_x,
                    world_z: point.world_z,
                    grid_x: point.grid_x,
                    grid_z: point.grid_z,
                    basin_id: point.basin_id,
                    receiver_id: point.receiver_id,
                    accumulation: point.accumulation,
                    stream_order: point.stream_order,
                    ocean: point.flags & 1 != 0,
                    channel: point.flags & 2 != 0,
                    confluence: point.flags & 4 != 0,
                    protected_basin: point.flags & 8 != 0,
                    quiet_core: point.flags & 16 != 0,
                    crop_edge: point.flags & 32 != 0,
                    uplift: point.uplift,
                    quiet: point.quiet,
                    broad_low: point.broad_low,
                    base_y: point.base_y,
                });
        serde_json::to_string(&receipt)
            .map_err(|error| js_error(format!("serialize landform-plan point: {error}")))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabLandformPlanPointReceipt {
    world_x: i32,
    world_z: i32,
    grid_x: u16,
    grid_z: u16,
    basin_id: u16,
    receiver_id: u32,
    accumulation: u32,
    stream_order: u8,
    ocean: bool,
    channel: bool,
    confluence: bool,
    protected_basin: bool,
    quiet_core: bool,
    crop_edge: bool,
    uplift: f32,
    quiet: f32,
    broad_low: f32,
    base_y: i16,
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
