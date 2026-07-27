use mclone_worldgen::streamed_plan_atlas::{
    STREAMED_PLAN_ATLAS_SCHEMA_REVISION, StreamedPlanAtlasCompiler,
};
use mclone_worldgen::streamed_plan_harness::StreamedPlanTopology;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(js_name = StreamedPlanAtlasCompiler)]
pub struct TerrainLabStreamedPlanAtlasCompiler {
    compiler: StreamedPlanAtlasCompiler,
    seed: i64,
    topology: StreamedPlanTopology,
}

#[wasm_bindgen(js_class = StreamedPlanAtlasCompiler)]
impl TerrainLabStreamedPlanAtlasCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(
        seed: String,
        topology: String,
    ) -> Result<TerrainLabStreamedPlanAtlasCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let topology = parse_topology(&topology)?;
        Ok(Self {
            compiler: StreamedPlanAtlasCompiler::new(seed, topology),
            seed,
            topology,
        })
    }

    #[wasm_bindgen(getter)]
    pub fn schema(&self) -> String {
        STREAMED_PLAN_ATLAS_SCHEMA_REVISION.to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> String {
        self.seed.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn topology(&self) -> String {
        self.topology.label().to_owned()
    }

    pub fn query(
        &mut self,
        center_x: f64,
        center_z: f64,
        blocks_across: f64,
        aspect_ratio: f64,
    ) -> Result<String, JsValue> {
        let center_x = exact_i32("center X", center_x)?;
        let center_z = exact_i32("center Z", center_z)?;
        let blocks_across = exact_i32("blocks across", blocks_across)?;
        let summary = self
            .compiler
            .query(center_x, center_z, blocks_across, aspect_ratio)
            .map_err(js_error)?;
        serde_json::to_string(&summary)
            .map_err(|error| js_error(format!("serialize streamed-plan atlas: {error}")))
    }

    #[wasm_bindgen(js_name = clearCache)]
    pub fn clear_cache(&mut self) {
        self.compiler.clear_cache();
    }
}

fn parse_topology(value: &str) -> Result<StreamedPlanTopology, JsValue> {
    match value.trim().to_ascii_lowercase().as_str() {
        "plane" => Ok(StreamedPlanTopology::Plane),
        "cylinder" | "cylinder-x" | "cylinder-x-6144" => Ok(StreamedPlanTopology::CylinderX),
        "torus" | "torus-6144x6144" => Ok(StreamedPlanTopology::Torus),
        other => Err(js_error(format!(
            "unsupported streamed-plan atlas topology {other:?}; expected plane, cylinder-x, or \
             torus"
        ))),
    }
}

fn exact_i32(label: &str, value: f64) -> Result<i32, JsValue> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < f64::from(i32::MIN)
        || value > f64::from(i32::MAX)
    {
        return Err(js_error(format!(
            "streamed-plan atlas {label} must be a signed 32-bit integer"
        )));
    }
    Ok(value as i32)
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
