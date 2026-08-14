use mclone_worldgen::levelgen::{
    MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS, MCLONE_WILDLIFE_POPULATION_REVISION,
    McloneOverworldSamplingTopology, McloneOverworldWildlifePlanner, McloneWildlifePopulationCell,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

const WILDLIFE_POPULATION_SCHEMA: &str = "mclone-wildlife-population-lab-v1";
const MAX_VISIBLE_CELLS: i64 = 16_384;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WildlifePopulationSummary {
    schema: &'static str,
    seed: String,
    revision: u16,
    cell_blocks: i32,
    min_cell_x: i32,
    min_cell_z: i32,
    width_cells: u32,
    depth_cells: u32,
    occupied_cells: u32,
    animal_count: u32,
    species_counts: [u32; 4],
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
    biome: &'static str,
    landform: &'static str,
    land: u16,
    productivity: u16,
    openness: u16,
    forest_cover: u16,
    wetland: u16,
    water: u16,
    rabbit_weight: u16,
    deer_weight: u16,
    mallard_weight: u16,
    bee_weight: u16,
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
    planner: McloneOverworldWildlifePlanner,
}

#[wasm_bindgen(js_class = WildlifePopulationCompiler)]
impl TerrainLabWildlifePopulationCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String) -> Result<Self, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            seed,
            planner: McloneOverworldWildlifePlanner::new(
                seed,
                McloneOverworldSamplingTopology::Unbounded,
            ),
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
        let cell_blocks = i64::from(MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS);
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
                "wildlife view needs {cell_count} population cells; zoom inside {} blocks across (maximum {MAX_VISIBLE_CELLS} cells)",
                MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS * 128
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
        let mut species_counts = [0_u32; 4];
        let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
        for offset_z in 0..depth_cells {
            for offset_x in 0..width_cells {
                let cell = McloneWildlifePopulationCell {
                    x: min_cell_x + i32::try_from(offset_x).expect("bounded offset fits i32"),
                    z: min_cell_z + i32::try_from(offset_z).expect("bounded offset fits i32"),
                };
                let plan = self
                    .planner
                    .plan_cell(cell)
                    .map_err(|error| js_error(error.to_string()))?;
                let encounter = plan.encounter;
                if let Some(encounter) = encounter {
                    occupied_cells += 1;
                    animal_count += u32::from(encounter.group_size);
                    species_counts[encounter.species as usize] += 1;
                }
                checksum = fnv_mix(checksum, plan.cell.x as u32 as u64);
                checksum = fnv_mix(checksum, plan.cell.z as u32 as u64);
                checksum = fnv_mix(checksum, plan.representative.world_x as u32 as u64);
                checksum = fnv_mix(checksum, plan.representative.world_z as u32 as u64);
                checksum = fnv_mix(checksum, plan.representative.surface_y as u32 as u64);
                checksum = fnv_mix_str(checksum, plan.representative.biome.label());
                checksum = fnv_mix_str(checksum, plan.representative.landform.label());
                checksum = fnv_mix(checksum, u64::from(plan.representative.land));
                checksum = fnv_mix(checksum, u64::from(plan.representative.productivity));
                checksum = fnv_mix(checksum, u64::from(plan.representative.openness));
                checksum = fnv_mix(checksum, u64::from(plan.representative.forest_cover));
                checksum = fnv_mix(checksum, u64::from(plan.representative.wetland));
                checksum = fnv_mix(checksum, u64::from(plan.representative.water));
                checksum = fnv_mix(checksum, u64::from(plan.suitability.rabbit));
                checksum = fnv_mix(checksum, u64::from(plan.suitability.deer));
                checksum = fnv_mix(checksum, u64::from(plan.suitability.mallard));
                checksum = fnv_mix(checksum, u64::from(plan.suitability.bee));
                checksum = fnv_mix(checksum, u64::from(plan.desired_density));
                checksum = fnv_mix(checksum, u64::from(plan.occupancy_roll));
                checksum = fnv_mix(checksum, u64::from(plan.species_roll));
                checksum = fnv_mix(checksum, plan.selected_habitat.world_x as u32 as u64);
                checksum = fnv_mix(checksum, plan.selected_habitat.world_z as u32 as u64);
                checksum = fnv_mix(
                    checksum,
                    encounter
                        .map(|encounter| {
                            (u64::from(encounter.species as u8) << 8)
                                | u64::from(encounter.group_size)
                        })
                        .unwrap_or(u64::MAX),
                );
                cells.push(WildlifePopulationCellReceipt {
                    cell_x: plan.cell.x,
                    cell_z: plan.cell.z,
                    center_x: plan.representative.world_x,
                    center_z: plan.representative.world_z,
                    surface_y: plan.representative.surface_y,
                    biome: plan.representative.biome.label(),
                    landform: plan.representative.landform.label(),
                    land: plan.representative.land,
                    productivity: plan.representative.productivity,
                    openness: plan.representative.openness,
                    forest_cover: plan.representative.forest_cover,
                    wetland: plan.representative.wetland,
                    water: plan.representative.water,
                    rabbit_weight: plan.suitability.rabbit,
                    deer_weight: plan.suitability.deer,
                    mallard_weight: plan.suitability.mallard,
                    bee_weight: plan.suitability.bee,
                    desired_density: plan.desired_density,
                    occupancy_roll: plan.occupancy_roll,
                    species_roll: plan.species_roll,
                    selected_x: plan.selected_habitat.world_x,
                    selected_z: plan.selected_habitat.world_z,
                    species: encounter.map(|encounter| encounter.species.label()),
                    group_size: encounter.map_or(0, |encounter| encounter.group_size),
                    owner_chunk_x: encounter.map(|encounter| encounter.owner_chunk.x),
                    owner_chunk_z: encounter.map(|encounter| encounter.owner_chunk.z),
                });
            }
        }
        serde_json::to_string(&WildlifePopulationSummary {
            schema: WILDLIFE_POPULATION_SCHEMA,
            seed: self.seed.to_string(),
            revision: MCLONE_WILDLIFE_POPULATION_REVISION,
            cell_blocks: MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS,
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
