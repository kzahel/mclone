use js_sys::{Float32Array, Uint8Array};
use mclone_worldgen::semantic_terrain_sandbox::{
    SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION, SEMANTIC_TERRAIN_SANDBOX_SUITE_SHA256,
    SemanticTerrainFeatureMode, SemanticTerrainSandboxGrid, SemanticTerrainSandboxRequest,
    SemanticTerrainSubstrate, compile_semantic_terrain_sandbox, run_semantic_terrain_sandbox_suite,
};
use mclone_worldgen::streamed_plan_harness::StreamedPlanTopology;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(js_name = SemanticTerrainSandboxCompiler)]
pub struct TerrainLabSemanticTerrainSandboxCompiler {
    seed: i64,
    topology: StreamedPlanTopology,
    substrate: SemanticTerrainSubstrate,
    features: SemanticTerrainFeatureMode,
    grid: Option<SemanticTerrainSandboxGrid>,
}

#[wasm_bindgen(js_class = SemanticTerrainSandboxCompiler)]
impl TerrainLabSemanticTerrainSandboxCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(
        seed: String,
        topology: String,
        substrate: String,
        features: String,
    ) -> Result<TerrainLabSemanticTerrainSandboxCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            seed,
            topology: parse_topology(&topology)?,
            substrate: SemanticTerrainSubstrate::parse(&substrate).ok_or_else(|| {
                js_error(format!(
                    "unsupported semantic terrain substrate {substrate:?}; expected flat or quiet"
                ))
            })?,
            features: SemanticTerrainFeatureMode::parse(&features).ok_or_else(|| {
                js_error(format!(
                    "unsupported semantic terrain features {features:?}; expected range, basin, or \
                     combined"
                ))
            })?,
            grid: None,
        })
    }

    #[wasm_bindgen(getter)]
    pub fn schema(&self) -> String {
        SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.to_owned()
    }

    pub fn compile(
        &mut self,
        center_x: f64,
        center_z: f64,
        blocks_across: f64,
        aspect_ratio: f64,
        samples_across: u32,
    ) -> Result<String, JsValue> {
        let request = SemanticTerrainSandboxRequest::new(
            self.seed,
            self.topology,
            center_x,
            center_z,
            blocks_across,
            aspect_ratio,
            samples_across,
            self.substrate,
            self.features,
        );
        let grid = compile_semantic_terrain_sandbox(request).map_err(js_error)?;
        let metadata = serde_json::to_string(&grid.metadata)
            .map_err(|error| js_error(format!("serialize semantic terrain metadata: {error}")))?;
        self.grid = Some(grid);
        Ok(metadata)
    }

    #[wasm_bindgen(js_name = foundation)]
    pub fn foundation(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(self.grid()?.foundation.as_slice()))
    }

    #[wasm_bindgen(js_name = parentHeights)]
    pub fn parent_heights(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(self.grid()?.parent_heights.as_slice()))
    }

    #[wasm_bindgen(js_name = regionalHeights)]
    pub fn regional_heights(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(self.grid()?.regional_heights.as_slice()))
    }

    #[wasm_bindgen(js_name = localHeights)]
    pub fn local_heights(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(self.grid()?.local_heights.as_slice()))
    }

    #[wasm_bindgen(js_name = regionalCorrection)]
    pub fn regional_correction(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(
            self.grid()?.regional_correction.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = localCorrection)]
    pub fn local_correction(&self) -> Result<Float32Array, JsValue> {
        Ok(Float32Array::from(self.grid()?.local_correction.as_slice()))
    }

    #[wasm_bindgen(js_name = parentWater)]
    pub fn parent_water(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.grid()?.parent_water.as_slice()))
    }

    #[wasm_bindgen(js_name = regionalWater)]
    pub fn regional_water(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.grid()?.regional_water.as_slice()))
    }

    #[wasm_bindgen(js_name = localWater)]
    pub fn local_water(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.grid()?.local_water.as_slice()))
    }
}

#[wasm_bindgen(js_name = semanticTerrainSandboxSuiteSha256)]
pub fn semantic_terrain_sandbox_suite_sha256() -> Result<String, JsValue> {
    let receipt = run_semantic_terrain_sandbox_suite().map_err(js_error)?;
    if !receipt.passed || receipt.suite_sha256 != SEMANTIC_TERRAIN_SANDBOX_SUITE_SHA256 {
        return Err(js_error(format!(
            "semantic terrain Wasm suite diverged: expected {}, got {}",
            SEMANTIC_TERRAIN_SANDBOX_SUITE_SHA256, receipt.suite_sha256
        )));
    }
    Ok(receipt.suite_sha256)
}

impl TerrainLabSemanticTerrainSandboxCompiler {
    fn grid(&self) -> Result<&SemanticTerrainSandboxGrid, JsValue> {
        self.grid
            .as_ref()
            .ok_or_else(|| js_error("compile semantic terrain before requesting arrays"))
    }
}

fn parse_topology(value: &str) -> Result<StreamedPlanTopology, JsValue> {
    match value.trim().to_ascii_lowercase().as_str() {
        "plane" => Ok(StreamedPlanTopology::Plane),
        "cylinder" | "cylinder-x" | "cylinder-x-6144" => Ok(StreamedPlanTopology::CylinderX),
        "torus" | "torus-6144x6144" => Ok(StreamedPlanTopology::Torus),
        other => Err(js_error(format!(
            "unsupported semantic terrain topology {other:?}; expected plane, cylinder-x, or torus"
        ))),
    }
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
