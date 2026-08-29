use mclone_core::HorizontalTopology;
use mclone_worldgen::levelgen::{
    WILDLIFE_POPULATION_CELL_BLOCKS, WILDLIFE_POPULATION_REVISION, WildlifeCellPlan,
    WildlifeHabitatSample, WildlifeHabitatSource, WildlifePopulationCell,
    WildlifePopulationPlanner,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

const WILDLIFE_POPULATION_SCHEMA: &str = "mclone-wildlife-population-lab-v2";
const MAX_VISIBLE_CELLS: i64 = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WildlifeAdapterStatus {
    Available,
    RequiresPublishedBlocks,
}

impl WildlifeAdapterStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::RequiresPublishedBlocks => "requires-published-blocks",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WildlifePopulationSummary {
    schema: &'static str,
    seed: String,
    profile: &'static str,
    adapter: &'static str,
    adapter_status: WildlifeAdapterStatus,
    revision: u16,
    cell_blocks: i32,
    min_cell_x: i32,
    min_cell_z: i32,
    width_cells: u32,
    depth_cells: u32,
    occupied_cells: u32,
    animal_count: u32,
    species_counts: [u32; 7],
    checksum: String,
    cells: Vec<WildlifePopulationCellReceipt>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WildlifePopulationCellReceipt {
    cell_x: i32,
    cell_z: i32,
    center_x: i32,
    center_z: i32,
    surface_y: i32,
    habitat: &'static str,
    evidence_source: &'static str,
    evidence_status: &'static str,
    available_evidence: Vec<&'static str>,
    supported: bool,
    land: u16,
    productivity: u16,
    openness: u16,
    low_cover: u16,
    forest_cover: u16,
    forest_edge: u16,
    wetland: u16,
    water: u16,
    inland_water: u16,
    shore: u16,
    bank: u16,
    flowering: u16,
    seeds_and_soft_mast: u16,
    mature_trees: u16,
    rabbit_weight: u16,
    deer_weight: u16,
    mallard_weight: u16,
    bee_weight: u16,
    squirrel_weight: u16,
    cow_weight: u16,
    chicken_weight: u16,
    desired_density: u16,
    occupancy_roll: u16,
    species_roll: u32,
    selected_x: i32,
    selected_z: i32,
    species: Option<&'static str>,
    group_size: u8,
    owner_chunk_x: Option<i32>,
    owner_chunk_z: Option<i32>,
}

#[wasm_bindgen(js_name = WildlifePopulationCompiler)]
pub struct TerrainLabWildlifePopulationCompiler {
    seed: i64,
    profile: &'static str,
    adapter_status: WildlifeAdapterStatus,
    planner: WildlifePopulationPlanner,
}

#[wasm_bindgen(js_class = WildlifePopulationCompiler)]
impl TerrainLabWildlifePopulationCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String, profile: String) -> Result<Self, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let (profile, source, adapter_status) = match profile.trim() {
            "mclone-overworld-v1" => (
                "mclone-overworld-v1",
                WildlifeHabitatSource::McloneOverworldV1,
                WildlifeAdapterStatus::Available,
            ),
            "mclone-overworld-v2" => (
                "mclone-overworld-v2",
                WildlifeHabitatSource::McloneOverworldV2,
                WildlifeAdapterStatus::Available,
            ),
            "mclone-overworld-v3" => (
                "mclone-overworld-v3",
                WildlifeHabitatSource::McloneOverworldV3,
                WildlifeAdapterStatus::Available,
            ),
            "overworld" => (
                "overworld",
                WildlifeHabitatSource::PublishedBlocks,
                WildlifeAdapterStatus::RequiresPublishedBlocks,
            ),
            other => {
                return Err(js_error(format!(
                    "wildlife population profile {other:?} is unsupported"
                )));
            }
        };
        let planner =
            WildlifePopulationPlanner::for_habitat(seed, HorizontalTopology::UNBOUNDED, source)
                .map_err(|error| js_error(error.to_string()))?;
        Ok(Self {
            seed,
            profile,
            adapter_status,
            planner,
        })
    }

    pub fn compile(
        &self,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        aspect_ratio: f64,
    ) -> Result<String, JsValue> {
        if blocks_across == 0 || !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
            return Err(js_error(
                "wildlife view requires positive blocksAcross and aspectRatio",
            ));
        }
        let width = i64::from(blocks_across);
        let height = ((width as f64) / aspect_ratio).ceil().max(1.0) as i64;
        let min_x = i64::from(center_x) - width / 2;
        let min_z = i64::from(center_z) - height / 2;
        let max_x = min_x
            .checked_add(width)
            .ok_or_else(|| js_error("wildlife view X bounds overflow"))?;
        let max_z = min_z
            .checked_add(height)
            .ok_or_else(|| js_error("wildlife view Z bounds overflow"))?;
        let cell_blocks = i64::from(WILDLIFE_POPULATION_CELL_BLOCKS);
        let min_cell_x = min_x.div_euclid(cell_blocks);
        let min_cell_z = min_z.div_euclid(cell_blocks);
        let max_cell_x = (max_x - 1).div_euclid(cell_blocks);
        let max_cell_z = (max_z - 1).div_euclid(cell_blocks);
        let width_cells = max_cell_x - min_cell_x + 1;
        let depth_cells = max_cell_z - min_cell_z + 1;
        let cell_count = width_cells
            .checked_mul(depth_cells)
            .ok_or_else(|| js_error("wildlife view cell count overflow"))?;
        if cell_count > MAX_VISIBLE_CELLS {
            return Err(js_error(format!(
                "wildlife view needs {cell_count} population cells; zoom in until the visible window is at most {MAX_VISIBLE_CELLS} cells"
            )));
        }
        let min_cell_x = i32::try_from(min_cell_x)
            .map_err(|_| js_error("wildlife view minimum cell X exceeds i32"))?;
        let min_cell_z = i32::try_from(min_cell_z)
            .map_err(|_| js_error("wildlife view minimum cell Z exceeds i32"))?;
        let width_cells =
            u32::try_from(width_cells).map_err(|_| js_error("wildlife view width exceeds u32"))?;
        let depth_cells =
            u32::try_from(depth_cells).map_err(|_| js_error("wildlife view depth exceeds u32"))?;

        let mut cells = Vec::with_capacity(usize::try_from(cell_count).unwrap_or(0));
        let mut occupied_cells = 0_u32;
        let mut animal_count = 0_u32;
        let mut species_counts = [0_u32; 7];
        let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
        for offset_z in 0..depth_cells {
            for offset_x in 0..width_cells {
                let cell = WildlifePopulationCell {
                    x: min_cell_x + i32::try_from(offset_x).expect("bounded offset fits i32"),
                    z: min_cell_z + i32::try_from(offset_z).expect("bounded offset fits i32"),
                };
                let plan = self.compile_cell(cell)?;
                let encounter = plan.encounter;
                if let Some(encounter) = encounter {
                    occupied_cells += 1;
                    animal_count += u32::from(encounter.group_size);
                    species_counts[encounter.species as usize] += 1;
                }
                checksum = checksum_plan(checksum, plan, self.adapter_status);
                cells.push(receipt(plan));
            }
        }
        serde_json::to_string(&WildlifePopulationSummary {
            schema: WILDLIFE_POPULATION_SCHEMA,
            seed: self.seed.to_string(),
            profile: self.profile,
            adapter: self.planner.habitat_source().label(),
            adapter_status: self.adapter_status,
            revision: WILDLIFE_POPULATION_REVISION,
            cell_blocks: WILDLIFE_POPULATION_CELL_BLOCKS,
            min_cell_x,
            min_cell_z,
            width_cells,
            depth_cells,
            occupied_cells,
            animal_count,
            species_counts,
            checksum: format!("{checksum:016x}"),
            cells,
        })
        .map_err(|error| js_error(format!("failed to encode wildlife population: {error}")))
    }

    fn compile_cell(&self, cell: WildlifePopulationCell) -> Result<WildlifeCellPlan, JsValue> {
        if self.adapter_status == WildlifeAdapterStatus::Available {
            return self
                .planner
                .plan_cell(cell)
                .map_err(|error| js_error(error.to_string()));
        }
        let samples = self
            .planner
            .candidate_sites(cell)
            .map_err(|error| js_error(error.to_string()))?
            .into_iter()
            .map(|site| {
                WildlifeHabitatSample::unavailable(
                    site.world_x,
                    site.world_z,
                    WildlifeHabitatSource::PublishedBlocks,
                )
            })
            .collect::<Vec<_>>();
        self.planner
            .plan_cell_with_samples(cell, &samples)
            .map_err(|error| js_error(error.to_string()))
    }
}

fn receipt(plan: WildlifeCellPlan) -> WildlifePopulationCellReceipt {
    let sample = plan.representative;
    let encounter = plan.encounter;
    let available_evidence = sample.available_evidence.labels().collect::<Vec<_>>();
    let evidence_status = if !sample.supported {
        "unavailable"
    } else if available_evidence.len() == 14 {
        "complete"
    } else {
        "partial"
    };
    WildlifePopulationCellReceipt {
        cell_x: plan.cell.x,
        cell_z: plan.cell.z,
        center_x: sample.world_x,
        center_z: sample.world_z,
        surface_y: sample.surface_y,
        habitat: habitat_label(sample),
        evidence_source: sample.source.label(),
        evidence_status,
        available_evidence,
        supported: sample.supported,
        land: sample.land,
        productivity: sample.productivity,
        openness: sample.openness,
        low_cover: sample.low_cover,
        forest_cover: sample.forest_cover,
        forest_edge: sample.forest_edge,
        wetland: sample.wetland,
        water: sample.water,
        inland_water: sample.inland_water,
        shore: sample.shore,
        bank: sample.bank,
        flowering: sample.flowering,
        seeds_and_soft_mast: sample.seeds_and_soft_mast,
        mature_trees: sample.mature_trees,
        rabbit_weight: plan.suitability.rabbit,
        deer_weight: plan.suitability.deer,
        mallard_weight: plan.suitability.mallard,
        bee_weight: plan.suitability.bee,
        squirrel_weight: plan.suitability.squirrel,
        cow_weight: plan.suitability.cow,
        chicken_weight: plan.suitability.chicken,
        desired_density: plan.desired_density,
        occupancy_roll: plan.occupancy_roll,
        species_roll: plan.species_roll,
        selected_x: plan.selected_habitat.world_x,
        selected_z: plan.selected_habitat.world_z,
        species: encounter.map(|encounter| encounter.species.label()),
        group_size: encounter.map_or(0, |encounter| encounter.group_size),
        owner_chunk_x: encounter.map(|encounter| encounter.owner_chunk.x),
        owner_chunk_z: encounter.map(|encounter| encounter.owner_chunk.z),
    }
}

fn habitat_label(sample: WildlifeHabitatSample) -> &'static str {
    if !sample.supported {
        "unavailable"
    } else if sample.inland_water >= 350 {
        "inland-water"
    } else if sample.wetland >= 300 || sample.bank >= 300 {
        "wetland-margin"
    } else if sample.mature_trees >= 350 || sample.forest_cover >= 500 {
        "woodland"
    } else if sample.land >= 500 && sample.openness >= 500 {
        "open-land"
    } else {
        "mixed-or-marginal"
    }
}

fn checksum_plan(mut checksum: u64, plan: WildlifeCellPlan, status: WildlifeAdapterStatus) -> u64 {
    let sample = plan.representative;
    for value in [
        plan.cell.x as u32 as u64,
        plan.cell.z as u32 as u64,
        sample.world_x as u32 as u64,
        sample.world_z as u32 as u64,
        sample.surface_y as u32 as u64,
        u64::from(sample.supported),
        u64::from(sample.land),
        u64::from(sample.productivity),
        u64::from(sample.openness),
        u64::from(sample.low_cover),
        u64::from(sample.forest_cover),
        u64::from(sample.forest_edge),
        u64::from(sample.wetland),
        u64::from(sample.water),
        u64::from(sample.inland_water),
        u64::from(sample.shore),
        u64::from(sample.bank),
        u64::from(sample.flowering),
        u64::from(sample.seeds_and_soft_mast),
        u64::from(sample.mature_trees),
        u64::from(plan.suitability.rabbit),
        u64::from(plan.suitability.deer),
        u64::from(plan.suitability.mallard),
        u64::from(plan.suitability.bee),
        u64::from(plan.suitability.squirrel),
        u64::from(plan.suitability.cow),
        u64::from(plan.suitability.chicken),
        u64::from(plan.desired_density),
        u64::from(plan.occupancy_roll),
        u64::from(plan.species_roll),
    ] {
        checksum = fnv_mix(checksum, value);
    }
    checksum = fnv_mix_str(checksum, sample.source.label());
    checksum = fnv_mix_str(checksum, status.label());
    fnv_mix(
        checksum,
        plan.encounter
            .map(|encounter| {
                (u64::from(encounter.species as u8) << 8) | u64::from(encounter.group_size)
            })
            .unwrap_or(u64::MAX),
    )
}

fn fnv_mix(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv_mix_str(mut hash: u64, value: &str) -> u64 {
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&message.into()).into()
}
