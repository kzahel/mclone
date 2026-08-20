use js_sys::{Int16Array, Uint8Array, Uint16Array, Uint32Array};
use mclone_worldgen::continental_ecoregion::{
    ContinentalEcoregionTopology, MIN_SUPPORTED_CYLINDER_BLOCKS,
};
use mclone_worldgen::continental_ecoregion_atlas::{
    ContinentalEcoregionAtlas, ContinentalEcoregionAtlasRequest,
    compile_continental_ecoregion_atlas,
};
use mclone_worldgen::continental_ecoregion_harness::{
    CONTINENTAL_ECOREGION_WITNESS_SHA256, run_continental_ecoregion_suite,
};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(js_name = ContinentalEcoregionCompiler)]
pub struct TerrainLabContinentalEcoregionCompiler {
    seed: i64,
    topology: ContinentalEcoregionTopology,
    atlas: Option<ContinentalEcoregionAtlas>,
}

#[wasm_bindgen(js_class = ContinentalEcoregionCompiler)]
impl TerrainLabContinentalEcoregionCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String, topology: String) -> Result<Self, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            seed,
            topology: parse_topology(&topology)?,
            atlas: None,
        })
    }

    pub fn compile(
        &mut self,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        aspect_ratio: f64,
        samples_across: u32,
    ) -> Result<String, JsValue> {
        let atlas = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest {
            seed: self.seed,
            topology: self.topology,
            center_x,
            center_z,
            blocks_across,
            aspect_ratio,
            samples_across,
        })
        .map_err(|error| js_error(error.to_string()))?;
        let metadata = serde_json::to_string(&atlas.metadata)
            .map_err(|error| js_error(format!("serialize continental atlas metadata: {error}")))?;
        self.atlas = Some(atlas);
        Ok(metadata)
    }

    pub fn land(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.land.as_slice()))
    }

    #[wasm_bindgen(js_name = inlandDistanceQuarterBlocks)]
    pub fn inland_distance_quarter_blocks(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(
            self.atlas()?.inland_distance_quarter_blocks.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = continentStory)]
    pub fn continent_story(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.atlas()?.continent_story.as_slice()))
    }

    #[wasm_bindgen(js_name = provinceKind)]
    pub fn province_kind(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.atlas()?.province_kind.as_slice()))
    }

    #[wasm_bindgen(js_name = ecoregionKind)]
    pub fn ecoregion_kind(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.atlas()?.ecoregion_kind.as_slice()))
    }

    pub fn transition(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.transition.as_slice()))
    }

    #[wasm_bindgen(js_name = transitionPeerKind)]
    pub fn transition_peer_kind(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(
            self.atlas()?.transition_peer_kind.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = transitionWidthBlocks)]
    pub fn transition_width_blocks(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(
            self.atlas()?.transition_width_blocks.as_slice(),
        ))
    }

    pub fn openness(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.openness.as_slice()))
    }

    #[wasm_bindgen(js_name = forestCore)]
    pub fn forest_core(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.forest_core.as_slice()))
    }

    #[wasm_bindgen(js_name = clearingCore)]
    pub fn clearing_core(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.clearing_core.as_slice()))
    }

    #[wasm_bindgen(js_name = clearingCause)]
    pub fn clearing_cause(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.atlas()?.clearing_cause.as_slice()))
    }

    #[wasm_bindgen(js_name = majorWater)]
    pub fn major_water(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.major_water.as_slice()))
    }

    pub fn wetland(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.wetland.as_slice()))
    }

    pub fn corridor(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.corridor.as_slice()))
    }

    #[wasm_bindgen(js_name = corridorKind)]
    pub fn corridor_kind(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(self.atlas()?.corridor_kind.as_slice()))
    }

    #[wasm_bindgen(js_name = corridorId)]
    pub fn corridor_id(&self) -> Result<Uint32Array, JsValue> {
        Ok(Uint32Array::from(self.atlas()?.corridor_id.as_slice()))
    }

    #[wasm_bindgen(js_name = continentId)]
    pub fn continent_id(&self) -> Result<Uint32Array, JsValue> {
        Ok(Uint32Array::from(self.atlas()?.continent_id.as_slice()))
    }

    #[wasm_bindgen(js_name = provinceId)]
    pub fn province_id(&self) -> Result<Uint32Array, JsValue> {
        Ok(Uint32Array::from(self.atlas()?.province_id.as_slice()))
    }

    #[wasm_bindgen(js_name = ecoregionId)]
    pub fn ecoregion_id(&self) -> Result<Uint32Array, JsValue> {
        Ok(Uint32Array::from(self.atlas()?.ecoregion_id.as_slice()))
    }

    #[wasm_bindgen(js_name = clearingId)]
    pub fn clearing_id(&self) -> Result<Uint32Array, JsValue> {
        Ok(Uint32Array::from(self.atlas()?.clearing_id.as_slice()))
    }

    #[wasm_bindgen(js_name = productionLand)]
    pub fn production_land(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.production_land.as_slice()))
    }

    #[wasm_bindgen(js_name = productionSurfaceY)]
    pub fn production_surface_y(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(
            self.atlas()?.production_surface_y.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = productionTemperature)]
    pub fn production_temperature(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(
            self.atlas()?.production_temperature.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = productionMoisture)]
    pub fn production_moisture(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(
            self.atlas()?.production_moisture.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = productionRelief)]
    pub fn production_relief(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(self.atlas()?.production_relief.as_slice()))
    }

    #[wasm_bindgen(js_name = productionRuggedness)]
    pub fn production_ruggedness(&self) -> Result<Int16Array, JsValue> {
        Ok(Int16Array::from(
            self.atlas()?.production_ruggedness.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = productionWater)]
    pub fn production_water(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(self.atlas()?.production_water.as_slice()))
    }

    #[wasm_bindgen(js_name = productionBiomeKind)]
    pub fn production_biome_kind(&self) -> Result<Uint8Array, JsValue> {
        Ok(Uint8Array::from(
            self.atlas()?.production_biome_kind.as_slice(),
        ))
    }

    #[wasm_bindgen(js_name = productionForestCoverage)]
    pub fn production_forest_coverage(&self) -> Result<Uint16Array, JsValue> {
        Ok(Uint16Array::from(
            self.atlas()?.production_forest_coverage.as_slice(),
        ))
    }
}

#[wasm_bindgen(js_name = continentalEcoregionSuiteSha256)]
pub fn continental_ecoregion_suite_sha256() -> Result<String, JsValue> {
    let receipt = run_continental_ecoregion_suite().map_err(js_error)?;
    if !receipt.suite_passed || receipt.witness_sha256 != CONTINENTAL_ECOREGION_WITNESS_SHA256 {
        return Err(js_error(format!(
            "continental/ecoregion Wasm suite diverged: expected {}, got {}",
            CONTINENTAL_ECOREGION_WITNESS_SHA256, receipt.witness_sha256
        )));
    }
    Ok(receipt.witness_sha256)
}

impl TerrainLabContinentalEcoregionCompiler {
    fn atlas(&self) -> Result<&ContinentalEcoregionAtlas, JsValue> {
        self.atlas
            .as_ref()
            .ok_or_else(|| js_error("compile continental atlas before requesting arrays"))
    }
}

fn parse_topology(value: &str) -> Result<ContinentalEcoregionTopology, JsValue> {
    match value.trim().to_ascii_lowercase().as_str() {
        "plane" => Ok(ContinentalEcoregionTopology::Plane),
        "cylinder-x-196608" | "continental-cylinder" => {
            Ok(ContinentalEcoregionTopology::CylinderX {
                period_blocks: 196_608,
            })
        }
        "cylinder-x" | "cylinder-x-6144" => Err(js_error(format!(
            "the 6,144-block proof cylinder is smaller than the Revision 2 minimum of {MIN_SUPPORTED_CYLINDER_BLOCKS} blocks"
        ))),
        other => Err(js_error(format!(
            "unsupported continental atlas topology {other:?}; expected plane or cylinder-x-196608"
        ))),
    }
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
