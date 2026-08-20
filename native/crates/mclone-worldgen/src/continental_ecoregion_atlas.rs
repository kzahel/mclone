//! Direct coarse atlas representation and metrics for the continental plan.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::continental_ecoregion::{
    ClearingCause, ContinentalEcoregionDescriptor, ContinentalEcoregionError,
    ContinentalEcoregionPlan, ContinentalEcoregionTopology, ContinentalStory, EcoregionKind,
    LandscapePlanDetail, LandscapePlanSample, LandscapeWindowRequest, PhysiographicProvinceKind,
    PlanConstructionCounts,
};
use crate::continental_ecoregion_harness::CONTINENTAL_ECOREGION_WITNESS_SHA256;
use crate::levelgen::{
    MCLONE_OVERWORLD_FIELD_REVISION, McloneOverworldBiomeRecipe, McloneOverworldLandformSample,
    McloneOverworldSampler, McloneOverworldTerrainSample, mclone_overworld_biome_recipe,
};

pub const CONTINENTAL_ECOREGION_ATLAS_SCHEMA_REVISION: &str =
    "mclone-continental-ecoregion-atlas-v2";
pub const CONTINENTAL_ECOREGION_ATLAS_DEFAULT_SAMPLES_ACROSS: u32 = 256;
pub const CONTINENTAL_ECOREGION_ATLAS_MAX_SAMPLES: usize = 262_144;
pub const CONTINENTAL_ECOREGION_ATLAS_NONE: u8 = u8::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinentalEcoregionAtlasRequest {
    pub seed: i64,
    pub topology: ContinentalEcoregionTopology,
    pub center_x: i32,
    pub center_z: i32,
    pub blocks_across: u32,
    pub aspect_ratio: f64,
    pub samples_across: u32,
}

impl ContinentalEcoregionAtlasRequest {
    pub const fn plane(seed: i64, center_x: i32, center_z: i32, blocks_across: u32) -> Self {
        Self {
            seed,
            topology: ContinentalEcoregionTopology::Plane,
            center_x,
            center_z,
            blocks_across,
            aspect_ratio: 1.0,
            samples_across: CONTINENTAL_ECOREGION_ATLAS_DEFAULT_SAMPLES_ACROSS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDistribution {
    pub component_count: u32,
    pub covered_samples: u32,
    pub minimum_samples: u32,
    pub median_samples: u32,
    pub p90_samples: u32,
    pub maximum_samples: u32,
    pub maximum_area_square_km: f64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjacencyPair {
    pub left_kind: u8,
    pub right_kind: u8,
    pub boundary_edges: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyReceipt {
    pub label: &'static str,
    pub distance_blocks: u32,
    pub run_count: u32,
    pub repeated_scene_alarms: u32,
    pub mean_dwell_blocks: u32,
    pub longest_dwell_blocks: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuantityDistribution {
    pub observation_count: u32,
    pub minimum: u64,
    pub median: u64,
    pub p90: u64,
    pub maximum: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearingPlanDistribution {
    pub clearing_count: u32,
    pub covered_samples: u32,
    pub area_square_meters: QuantityDistribution,
    pub center_isolation_blocks: QuantityDistribution,
    pub edge_length_blocks: QuantityDistribution,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HabitatConnectivityMetrics {
    pub patch_count: u32,
    pub corridor_component_count: u32,
    pub bridging_corridor_count: u32,
    pub graph_edge_count: u32,
    pub network_count: u32,
    pub isolated_patch_count: u32,
    pub largest_network_patches: u32,
    pub connected_patch_fraction: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalEcoregionAtlasMetrics {
    pub land_fraction: f32,
    pub ocean_fraction: f32,
    pub quiet_space_fraction: f32,
    pub transition_fraction: f32,
    pub continent_components: ComponentDistribution,
    pub open_components: ComponentDistribution,
    pub forest_components: ComponentDistribution,
    pub wetland_components: ComponentDistribution,
    pub clearing_components: ComponentDistribution,
    pub habitat_network_components: ComponentDistribution,
    pub ecoregion_components: ComponentDistribution,
    pub transition_width_blocks: QuantityDistribution,
    pub clearing_plans: ClearingPlanDistribution,
    pub regional_signature_recurrence_blocks: QuantityDistribution,
    pub habitat_connectivity: HabitatConnectivityMetrics,
    pub province_kind_counts: Vec<u32>,
    pub ecoregion_kind_counts: Vec<u32>,
    pub clearing_cause_counts: Vec<u32>,
    pub ecoregion_adjacencies: Vec<AdjacencyPair>,
    pub journeys: Vec<JourneyReceipt>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionControlWork {
    pub requested_samples: u64,
    pub field_samples: u64,
    pub exact_chunks: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionControlMetrics {
    pub land_fraction: f32,
    pub ocean_fraction: f32,
    pub land_components: ComponentDistribution,
    pub biome_components: ComponentDistribution,
    pub biome_kind_counts: Vec<u32>,
    pub journeys: Vec<JourneyReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalEcoregionAtlasMetadata {
    pub receipt_schema: &'static str,
    pub plan_schema: &'static str,
    pub witness_sha256: &'static str,
    pub production_terrain_unchanged: bool,
    pub seed: String,
    pub topology: &'static str,
    pub center_x: i32,
    pub center_z: i32,
    pub min_x: i32,
    pub min_z: i32,
    pub blocks_across: u32,
    pub blocks_tall: u32,
    pub sample_step_blocks: u32,
    pub columns: u32,
    pub rows: u32,
    pub sample_count: u32,
    pub semantic_sha256: String,
    pub work: PlanConstructionCounts,
    pub production_control_revision: &'static str,
    pub production_control_topology: &'static str,
    pub production_control_sha256: String,
    pub production_control_work: ProductionControlWork,
    pub production_control_metrics: ProductionControlMetrics,
    pub continent_stories: Vec<&'static str>,
    pub province_kinds: Vec<&'static str>,
    pub ecoregion_kinds: Vec<&'static str>,
    pub clearing_causes: Vec<&'static str>,
    pub metrics: ContinentalEcoregionAtlasMetrics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContinentalEcoregionAtlas {
    pub metadata: ContinentalEcoregionAtlasMetadata,
    pub land: Vec<u16>,
    pub inland_distance_quarter_blocks: Vec<i16>,
    pub continent_story: Vec<u8>,
    pub province_kind: Vec<u8>,
    pub ecoregion_kind: Vec<u8>,
    pub transition: Vec<u16>,
    pub openness: Vec<u16>,
    pub forest_core: Vec<u16>,
    pub clearing_core: Vec<u16>,
    pub clearing_cause: Vec<u8>,
    pub major_water: Vec<u16>,
    pub wetland: Vec<u16>,
    pub corridor: Vec<u16>,
    pub continent_id: Vec<u32>,
    pub province_id: Vec<u32>,
    pub ecoregion_id: Vec<u32>,
    pub clearing_id: Vec<u32>,
    pub production_land: Vec<u16>,
    pub production_surface_y: Vec<i16>,
    pub production_temperature: Vec<i16>,
    pub production_moisture: Vec<i16>,
    pub production_relief: Vec<i16>,
    pub production_ruggedness: Vec<i16>,
    pub production_water: Vec<u16>,
    pub production_biome_kind: Vec<u8>,
}

pub fn compile_continental_ecoregion_atlas(
    request: ContinentalEcoregionAtlasRequest,
) -> Result<ContinentalEcoregionAtlas, ContinentalEcoregionError> {
    validate_request(request)?;
    let columns = request.samples_across;
    let rows = ((f64::from(columns) / request.aspect_ratio).ceil() as u32).max(1);
    let sample_count = (columns as usize)
        .checked_mul(rows as usize)
        .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
    if sample_count > CONTINENTAL_ECOREGION_ATLAS_MAX_SAMPLES {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "atlas sample count exceeds its direct-query cap",
        ));
    }
    let step_blocks = request.blocks_across.div_ceil(columns).max(1);
    let width_blocks = columns
        .checked_mul(step_blocks)
        .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
    let height_blocks = rows
        .checked_mul(step_blocks)
        .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
    let min_x = centered_minimum(request.center_x, width_blocks)?;
    let min_z = centered_minimum(request.center_z, height_blocks)?;
    let plan = ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::new(
        request.seed,
        request.topology,
    ))?;
    let window = plan.query_window(LandscapeWindowRequest::new(
        min_x,
        min_z,
        columns,
        rows,
        step_blocks,
        LandscapePlanDetail::Mosaic,
    ))?;
    let mut arrays = AtlasArrays::with_capacity(sample_count);
    let production_sampler = McloneOverworldSampler::new(request.seed);
    for sample in &window.samples {
        arrays.push(sample);
        arrays.push_production(production_sampler.sample(sample.world_x, sample.world_z));
    }
    let metrics = atlas_metrics(columns, rows, step_blocks, &window.samples, &arrays);
    let production_control_metrics =
        production_control_metrics(columns, rows, step_blocks, &arrays);
    let production_control_sha256 = production_control_sha256(request, &arrays);
    Ok(ContinentalEcoregionAtlas {
        metadata: ContinentalEcoregionAtlasMetadata {
            receipt_schema: CONTINENTAL_ECOREGION_ATLAS_SCHEMA_REVISION,
            plan_schema: crate::continental_ecoregion::CONTINENTAL_ECOREGION_SCHEMA_REVISION,
            witness_sha256: CONTINENTAL_ECOREGION_WITNESS_SHA256,
            production_terrain_unchanged: true,
            seed: request.seed.to_string(),
            topology: topology_label(request.topology),
            center_x: request.center_x,
            center_z: request.center_z,
            min_x,
            min_z,
            blocks_across: width_blocks,
            blocks_tall: height_blocks,
            sample_step_blocks: step_blocks,
            columns,
            rows,
            sample_count: sample_count as u32,
            semantic_sha256: window.semantic_sha256,
            work: window.work,
            production_control_revision: MCLONE_OVERWORLD_FIELD_REVISION,
            production_control_topology: "unbounded-plane",
            production_control_sha256,
            production_control_work: ProductionControlWork {
                requested_samples: sample_count as u64,
                field_samples: sample_count as u64,
                exact_chunks: 0,
            },
            production_control_metrics,
            continent_stories: ContinentalStory::ALL
                .iter()
                .map(|kind| kind.label())
                .collect(),
            province_kinds: PhysiographicProvinceKind::ALL
                .iter()
                .map(|kind| kind.label())
                .collect(),
            ecoregion_kinds: EcoregionKind::ALL.iter().map(|kind| kind.label()).collect(),
            clearing_causes: ClearingCause::ALL.iter().map(|kind| kind.label()).collect(),
            metrics,
        },
        land: arrays.land,
        inland_distance_quarter_blocks: arrays.inland_distance_quarter_blocks,
        continent_story: arrays.continent_story,
        province_kind: arrays.province_kind,
        ecoregion_kind: arrays.ecoregion_kind,
        transition: arrays.transition,
        openness: arrays.openness,
        forest_core: arrays.forest_core,
        clearing_core: arrays.clearing_core,
        clearing_cause: arrays.clearing_cause,
        major_water: arrays.major_water,
        wetland: arrays.wetland,
        corridor: arrays.corridor,
        continent_id: arrays.continent_id,
        province_id: arrays.province_id,
        ecoregion_id: arrays.ecoregion_id,
        clearing_id: arrays.clearing_id,
        production_land: arrays.production_land,
        production_surface_y: arrays.production_surface_y,
        production_temperature: arrays.production_temperature,
        production_moisture: arrays.production_moisture,
        production_relief: arrays.production_relief,
        production_ruggedness: arrays.production_ruggedness,
        production_water: arrays.production_water,
        production_biome_kind: arrays.production_biome_kind,
    })
}

struct AtlasArrays {
    land: Vec<u16>,
    inland_distance_quarter_blocks: Vec<i16>,
    continent_story: Vec<u8>,
    province_kind: Vec<u8>,
    ecoregion_kind: Vec<u8>,
    transition: Vec<u16>,
    openness: Vec<u16>,
    forest_core: Vec<u16>,
    clearing_core: Vec<u16>,
    clearing_cause: Vec<u8>,
    major_water: Vec<u16>,
    wetland: Vec<u16>,
    corridor: Vec<u16>,
    continent_id: Vec<u32>,
    province_id: Vec<u32>,
    ecoregion_id: Vec<u32>,
    clearing_id: Vec<u32>,
    production_land: Vec<u16>,
    production_surface_y: Vec<i16>,
    production_temperature: Vec<i16>,
    production_moisture: Vec<i16>,
    production_relief: Vec<i16>,
    production_ruggedness: Vec<i16>,
    production_water: Vec<u16>,
    production_biome_kind: Vec<u8>,
}

impl AtlasArrays {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            land: Vec::with_capacity(capacity),
            inland_distance_quarter_blocks: Vec::with_capacity(capacity),
            continent_story: Vec::with_capacity(capacity),
            province_kind: Vec::with_capacity(capacity),
            ecoregion_kind: Vec::with_capacity(capacity),
            transition: Vec::with_capacity(capacity),
            openness: Vec::with_capacity(capacity),
            forest_core: Vec::with_capacity(capacity),
            clearing_core: Vec::with_capacity(capacity),
            clearing_cause: Vec::with_capacity(capacity),
            major_water: Vec::with_capacity(capacity),
            wetland: Vec::with_capacity(capacity),
            corridor: Vec::with_capacity(capacity),
            continent_id: Vec::with_capacity(capacity),
            province_id: Vec::with_capacity(capacity),
            ecoregion_id: Vec::with_capacity(capacity),
            clearing_id: Vec::with_capacity(capacity),
            production_land: Vec::with_capacity(capacity),
            production_surface_y: Vec::with_capacity(capacity),
            production_temperature: Vec::with_capacity(capacity),
            production_moisture: Vec::with_capacity(capacity),
            production_relief: Vec::with_capacity(capacity),
            production_ruggedness: Vec::with_capacity(capacity),
            production_water: Vec::with_capacity(capacity),
            production_biome_kind: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, sample: &LandscapePlanSample) {
        self.land.push(quantize_unit(sample.land_weight));
        self.inland_distance_quarter_blocks.push(
            (sample.inland_distance_blocks / 4.0)
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16,
        );
        self.continent_story.push(
            sample
                .continent
                .map_or(CONTINENTAL_ECOREGION_ATLAS_NONE, |fact| fact.story as u8),
        );
        self.province_kind.push(
            sample
                .province
                .map_or(CONTINENTAL_ECOREGION_ATLAS_NONE, |fact| fact.kind as u8),
        );
        self.ecoregion_kind.push(
            sample
                .ecoregion
                .map_or(CONTINENTAL_ECOREGION_ATLAS_NONE, |fact| fact.kind as u8),
        );
        self.transition.push(quantize_unit(
            sample.ecoregion.map_or(0.0, |fact| fact.transition_weight),
        ));
        self.openness.push(quantize_unit(
            sample.mosaic.map_or(0.0, |fact| fact.openness),
        ));
        self.forest_core.push(quantize_unit(
            sample.mosaic.map_or(0.0, |fact| fact.forest_core),
        ));
        self.clearing_core.push(quantize_unit(
            sample.mosaic.map_or(0.0, |fact| fact.clearing_core),
        ));
        self.clearing_cause.push(
            sample
                .mosaic
                .and_then(|fact| fact.clearing_cause)
                .map_or(CONTINENTAL_ECOREGION_ATLAS_NONE, |cause| cause as u8),
        );
        self.major_water.push(quantize_unit(
            sample.province.map_or(0.0, |fact| fact.major_water),
        ));
        self.wetland.push(quantize_unit(
            sample.mosaic.map_or(0.0, |fact| fact.wetland),
        ));
        self.corridor.push(quantize_unit(
            sample.mosaic.map_or(0.0, |fact| fact.corridor),
        ));
        self.continent_id
            .push(sample.continent.map_or(0, |fact| fact.id.hash as u32));
        self.province_id
            .push(sample.province.map_or(0, |fact| fact.id.hash as u32));
        self.ecoregion_id
            .push(sample.ecoregion.map_or(0, |fact| fact.id.hash as u32));
        self.clearing_id.push(
            sample
                .mosaic
                .and_then(|fact| fact.clearing_id)
                .map_or(0, |id| id.hash as u32),
        );
    }

    fn push_production(&mut self, terrain: McloneOverworldTerrainSample) {
        let land = terrain.continentalness > 0.0;
        let water = if !land {
            1.0
        } else {
            terrain
                .watercourse
                .channel_influence
                .max(terrain.watercourse.wetland_influence)
                .max(terrain.watercourse.wetland_pool_influence)
        };
        let recipe = mclone_overworld_biome_recipe(McloneOverworldLandformSample {
            terrain,
            slope: 0.0,
        });
        self.production_land
            .push(quantize_unit((terrain.continentalness * 0.5 + 0.5) as f32));
        self.production_surface_y.push(terrain.surface_y as i16);
        self.production_temperature
            .push(quantize_signed(terrain.climate.temperature));
        self.production_moisture
            .push(quantize_signed(terrain.climate.moisture));
        self.production_relief.push(quantize_signed(terrain.relief));
        self.production_ruggedness
            .push(quantize_signed(terrain.ruggedness));
        self.production_water.push(quantize_unit(water as f32));
        self.production_biome_kind.push(biome_recipe_code(recipe));
    }
}

fn atlas_metrics(
    columns: u32,
    rows: u32,
    step_blocks: u32,
    samples: &[LandscapePlanSample],
    arrays: &AtlasArrays,
) -> ContinentalEcoregionAtlasMetrics {
    let sample_count = samples.len().max(1) as f32;
    let land_mask = arrays
        .land
        .iter()
        .map(|value| *value >= 32_768)
        .collect::<Vec<_>>();
    let open_mask = arrays
        .openness
        .iter()
        .map(|value| *value >= 39_321)
        .collect::<Vec<_>>();
    let forest_mask = arrays
        .forest_core
        .iter()
        .map(|value| *value >= 26_214)
        .collect::<Vec<_>>();
    let wetland_mask = arrays
        .wetland
        .iter()
        .map(|value| *value >= 19_661)
        .collect::<Vec<_>>();
    let clearing_mask = arrays
        .clearing_core
        .iter()
        .map(|value| *value >= 13_107)
        .collect::<Vec<_>>();
    let habitat_mask = arrays
        .openness
        .iter()
        .zip(&arrays.forest_core)
        .zip(&arrays.wetland)
        .zip(&arrays.corridor)
        .map(|(((open, forest), wetland), corridor)| {
            *open >= 39_321 || *forest >= 32_768 || *wetland >= 19_661 || *corridor >= 16_384
        })
        .collect::<Vec<_>>();
    let quiet_samples = samples
        .iter()
        .filter(|sample| {
            sample
                .ecoregion
                .is_some_and(|fact| fact.kind == EcoregionKind::QuietTransition)
                && sample.mosaic.is_some_and(|fact| fact.clearing_core < 0.2)
        })
        .count();
    let transition_samples = arrays
        .transition
        .iter()
        .filter(|value| **value >= 13_107)
        .count();
    let mut province_kind_counts = vec![0_u32; PhysiographicProvinceKind::ALL.len()];
    let mut ecoregion_kind_counts = vec![0_u32; EcoregionKind::ALL.len()];
    let mut clearing_cause_counts = vec![0_u32; ClearingCause::ALL.len()];
    for sample in samples {
        if let Some(province) = sample.province {
            province_kind_counts[province.kind as usize] += 1;
        }
        if let Some(ecoregion) = sample.ecoregion {
            ecoregion_kind_counts[ecoregion.kind as usize] += 1;
        }
        if let Some(cause) = sample.mosaic.and_then(|fact| fact.clearing_cause) {
            clearing_cause_counts[cause as usize] += 1;
        }
    }
    ContinentalEcoregionAtlasMetrics {
        land_fraction: land_mask.iter().filter(|value| **value).count() as f32 / sample_count,
        ocean_fraction: land_mask.iter().filter(|value| !**value).count() as f32 / sample_count,
        quiet_space_fraction: quiet_samples as f32 / sample_count,
        transition_fraction: transition_samples as f32 / sample_count,
        continent_components: component_distribution(&land_mask, columns, rows, step_blocks),
        open_components: component_distribution(&open_mask, columns, rows, step_blocks),
        forest_components: component_distribution(&forest_mask, columns, rows, step_blocks),
        wetland_components: component_distribution(&wetland_mask, columns, rows, step_blocks),
        clearing_components: component_distribution(&clearing_mask, columns, rows, step_blocks),
        habitat_network_components: component_distribution(
            &habitat_mask,
            columns,
            rows,
            step_blocks,
        ),
        ecoregion_components: kind_component_distribution(
            &arrays.ecoregion_kind,
            columns,
            rows,
            step_blocks,
            Some(CONTINENTAL_ECOREGION_ATLAS_NONE),
        ),
        transition_width_blocks: transition_width_distribution(
            &arrays.transition,
            columns,
            rows,
            step_blocks,
        ),
        clearing_plans: clearing_plan_distribution(arrays, columns, rows, step_blocks),
        regional_signature_recurrence_blocks: regional_signature_recurrence_distribution(
            arrays,
            columns,
            step_blocks,
        ),
        habitat_connectivity: habitat_connectivity_metrics(arrays, columns, rows),
        province_kind_counts,
        ecoregion_kind_counts,
        clearing_cause_counts,
        ecoregion_adjacencies: adjacency_pairs(&arrays.ecoregion_kind, columns, rows),
        journeys: journey_receipts(arrays, columns, rows, step_blocks),
    }
}

fn production_control_metrics(
    columns: u32,
    rows: u32,
    step_blocks: u32,
    arrays: &AtlasArrays,
) -> ProductionControlMetrics {
    let sample_count = arrays.production_land.len().max(1) as f32;
    let land_mask = arrays
        .production_land
        .iter()
        .map(|value| *value >= 32_768)
        .collect::<Vec<_>>();
    let mut biome_kind_counts = vec![0_u32; 8];
    for kind in &arrays.production_biome_kind {
        biome_kind_counts[*kind as usize] += 1;
    }
    ProductionControlMetrics {
        land_fraction: land_mask.iter().filter(|value| **value).count() as f32 / sample_count,
        ocean_fraction: land_mask.iter().filter(|value| !**value).count() as f32 / sample_count,
        land_components: component_distribution(&land_mask, columns, rows, step_blocks),
        biome_components: kind_component_distribution(
            &arrays.production_biome_kind,
            columns,
            rows,
            step_blocks,
            None,
        ),
        biome_kind_counts,
        journeys: production_journey_receipts(arrays, columns, rows, step_blocks),
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SampledFeatureGeometry {
    samples: u32,
    boundary_edges: u32,
    column_sum: u64,
    row_sum: u64,
}

fn transition_width_distribution(
    transition: &[u16],
    columns: u32,
    rows: u32,
    step_blocks: u32,
) -> QuantityDistribution {
    let mask = transition
        .iter()
        .map(|value| *value >= 13_107)
        .collect::<Vec<_>>();
    let mut horizontal_runs = vec![0_u32; mask.len()];
    for row in 0..rows as usize {
        let mut column = 0_usize;
        while column < columns as usize {
            let start = column;
            while column < columns as usize && mask[row * columns as usize + column] {
                column += 1;
            }
            let length = (column - start) as u32;
            for member in start..column {
                horizontal_runs[row * columns as usize + member] = length;
            }
            column += usize::from(column == start);
        }
    }
    let mut vertical_runs = vec![0_u32; mask.len()];
    for column in 0..columns as usize {
        let mut row = 0_usize;
        while row < rows as usize {
            let start = row;
            while row < rows as usize && mask[row * columns as usize + column] {
                row += 1;
            }
            let length = (row - start) as u32;
            for member in start..row {
                vertical_runs[member * columns as usize + column] = length;
            }
            row += usize::from(row == start);
        }
    }
    let widths = mask
        .iter()
        .enumerate()
        .filter(|(_, active)| **active)
        .map(|(index, _)| {
            u64::from(horizontal_runs[index].min(vertical_runs[index])) * u64::from(step_blocks)
        })
        .collect();
    quantity_distribution(widths)
}

fn clearing_plan_distribution(
    arrays: &AtlasArrays,
    columns: u32,
    rows: u32,
    step_blocks: u32,
) -> ClearingPlanDistribution {
    let mut clearings = BTreeMap::<u32, SampledFeatureGeometry>::new();
    for index in 0..arrays.clearing_id.len() {
        let clearing_id = arrays.clearing_id[index];
        if clearing_id == 0 || arrays.clearing_core[index] < 13_107 {
            continue;
        }
        let column = index % columns as usize;
        let row = index / columns as usize;
        let geometry = clearings.entry(clearing_id).or_default();
        geometry.samples += 1;
        geometry.column_sum += column as u64;
        geometry.row_sum += row as u64;
        geometry.boundary_edges += four_neighbor_slots(index, columns, rows)
            .into_iter()
            .filter(|neighbor| {
                neighbor.is_none_or(|neighbor| {
                    arrays.clearing_id[neighbor] != clearing_id
                        || arrays.clearing_core[neighbor] < 13_107
                })
            })
            .count() as u32;
    }
    let area_per_sample = u64::from(step_blocks).pow(2);
    let areas = clearings
        .values()
        .map(|geometry| u64::from(geometry.samples) * area_per_sample)
        .collect();
    let edges = clearings
        .values()
        .map(|geometry| u64::from(geometry.boundary_edges) * u64::from(step_blocks))
        .collect();
    let geometries = clearings.values().copied().collect::<Vec<_>>();
    let isolations = nearest_matching_distances(&geometries, step_blocks, |_, _| true);
    ClearingPlanDistribution {
        clearing_count: clearings.len() as u32,
        covered_samples: clearings.values().map(|geometry| geometry.samples).sum(),
        area_square_meters: quantity_distribution(areas),
        center_isolation_blocks: quantity_distribution(isolations),
        edge_length_blocks: quantity_distribution(edges),
    }
}

fn regional_signature_recurrence_distribution(
    arrays: &AtlasArrays,
    columns: u32,
    step_blocks: u32,
) -> QuantityDistribution {
    let mut instances = BTreeMap::<u32, ((u8, u8, u8), SampledFeatureGeometry)>::new();
    for index in 0..arrays.ecoregion_id.len() {
        let ecoregion_id = arrays.ecoregion_id[index];
        if ecoregion_id == 0 || arrays.ecoregion_kind[index] == CONTINENTAL_ECOREGION_ATLAS_NONE {
            continue;
        }
        let signature = (
            arrays.continent_story[index],
            arrays.province_kind[index],
            arrays.ecoregion_kind[index],
        );
        let (_, geometry) = instances
            .entry(ecoregion_id)
            .or_insert((signature, SampledFeatureGeometry::default()));
        let column = index % columns as usize;
        let row = index / columns as usize;
        geometry.samples += 1;
        geometry.column_sum += column as u64;
        geometry.row_sum += row as u64;
    }
    let values = instances.values().copied().collect::<Vec<_>>();
    let geometries = values
        .iter()
        .map(|(_, geometry)| *geometry)
        .collect::<Vec<_>>();
    let distances = nearest_matching_distances(&geometries, step_blocks, |left, right| {
        values[left].0 == values[right].0
    });
    quantity_distribution(distances)
}

fn habitat_connectivity_metrics(
    arrays: &AtlasArrays,
    columns: u32,
    rows: u32,
) -> HabitatConnectivityMetrics {
    let habitat_kind = arrays
        .openness
        .iter()
        .zip(&arrays.forest_core)
        .zip(&arrays.wetland)
        .map(|((open, forest), wetland)| {
            if *wetland >= 19_661 {
                3_u8
            } else if *open >= 39_321 {
                1
            } else if *forest >= 32_768 {
                2
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let mut patch_labels = vec![u32::MAX; habitat_kind.len()];
    let mut patch_count = 0_u32;
    for start in 0..habitat_kind.len() {
        if habitat_kind[start] == 0 || patch_labels[start] != u32::MAX {
            continue;
        }
        let kind = habitat_kind[start];
        patch_labels[start] = patch_count;
        let mut queue = VecDeque::from([start]);
        while let Some(index) = queue.pop_front() {
            for neighbor in four_neighbors(index, columns, rows) {
                if habitat_kind[neighbor] == kind && patch_labels[neighbor] == u32::MAX {
                    patch_labels[neighbor] = patch_count;
                    queue.push_back(neighbor);
                }
            }
        }
        patch_count += 1;
    }

    let corridor_mask = arrays
        .corridor
        .iter()
        .map(|value| *value >= 16_384)
        .collect::<Vec<_>>();
    let mut corridor_visited = vec![false; corridor_mask.len()];
    let mut corridor_component_count = 0_u32;
    let mut bridging_corridor_count = 0_u32;
    let mut graph_edges = BTreeSet::<(u32, u32)>::new();
    for start in 0..corridor_mask.len() {
        if !corridor_mask[start] || corridor_visited[start] {
            continue;
        }
        corridor_component_count += 1;
        corridor_visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut touched_patches = BTreeSet::new();
        while let Some(index) = queue.pop_front() {
            for sample in std::iter::once(index).chain(four_neighbors(index, columns, rows)) {
                if patch_labels[sample] != u32::MAX {
                    touched_patches.insert(patch_labels[sample]);
                }
            }
            for neighbor in four_neighbors(index, columns, rows) {
                if corridor_mask[neighbor] && !corridor_visited[neighbor] {
                    corridor_visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        if touched_patches.len() >= 2 {
            bridging_corridor_count += 1;
        }
        let touched = touched_patches.into_iter().collect::<Vec<_>>();
        for left in 0..touched.len() {
            for right in left + 1..touched.len() {
                graph_edges.insert((touched[left], touched[right]));
            }
        }
    }

    let mut parents = (0..patch_count).collect::<Vec<_>>();
    let mut degrees = vec![0_u32; patch_count as usize];
    for &(left, right) in &graph_edges {
        union_sets(&mut parents, left, right);
        degrees[left as usize] += 1;
        degrees[right as usize] += 1;
    }
    let mut network_sizes = BTreeMap::<u32, u32>::new();
    for patch in 0..patch_count {
        let root = find_set(&mut parents, patch);
        *network_sizes.entry(root).or_default() += 1;
    }
    let isolated_patch_count = degrees.iter().filter(|degree| **degree == 0).count() as u32;
    HabitatConnectivityMetrics {
        patch_count,
        corridor_component_count,
        bridging_corridor_count,
        graph_edge_count: graph_edges.len() as u32,
        network_count: network_sizes.len() as u32,
        isolated_patch_count,
        largest_network_patches: network_sizes.values().copied().max().unwrap_or(0),
        connected_patch_fraction: if patch_count == 0 {
            0.0
        } else {
            (patch_count - isolated_patch_count) as f32 / patch_count as f32
        },
    }
}

fn nearest_matching_distances(
    geometries: &[SampledFeatureGeometry],
    step_blocks: u32,
    matches: impl Fn(usize, usize) -> bool,
) -> Vec<u64> {
    let mut distances = Vec::new();
    for left in 0..geometries.len() {
        let nearest = (0..geometries.len())
            .filter(|right| *right != left && matches(left, *right))
            .map(|right| centroid_distance_blocks(geometries[left], geometries[right], step_blocks))
            .min();
        if let Some(distance) = nearest {
            distances.push(distance);
        }
    }
    distances
}

fn centroid_distance_blocks(
    left: SampledFeatureGeometry,
    right: SampledFeatureGeometry,
    step_blocks: u32,
) -> u64 {
    if left.samples == 0 || right.samples == 0 {
        return 0;
    }
    let denominator = u128::from(left.samples) * u128::from(right.samples);
    let delta_column = (u128::from(left.column_sum) * u128::from(right.samples))
        .abs_diff(u128::from(right.column_sum) * u128::from(left.samples));
    let delta_row = (u128::from(left.row_sum) * u128::from(right.samples))
        .abs_diff(u128::from(right.row_sum) * u128::from(left.samples));
    let numerator = integer_square_root(
        delta_column
            .saturating_mul(delta_column)
            .saturating_add(delta_row.saturating_mul(delta_row)),
    );
    u64::try_from(numerator.saturating_mul(u128::from(step_blocks)) / denominator)
        .unwrap_or(u64::MAX)
}

fn quantity_distribution(mut values: Vec<u64>) -> QuantityDistribution {
    values.sort_unstable();
    QuantityDistribution {
        observation_count: values.len() as u32,
        minimum: values.first().copied().unwrap_or(0),
        median: percentile_u64(&values, 0.5),
        p90: percentile_u64(&values, 0.9),
        maximum: values.last().copied().unwrap_or(0),
    }
}

fn percentile_u64(values: &[u64], percentile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}

fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value / 2 + 1;
    while low <= high {
        let middle = low + (high - low) / 2;
        if middle <= value / middle {
            low = middle + 1;
        } else {
            high = middle - 1;
        }
    }
    high
}

fn four_neighbor_slots(index: usize, columns: u32, rows: u32) -> [Option<usize>; 4] {
    let columns = columns as usize;
    let rows = rows as usize;
    let column = index % columns;
    let row = index / columns;
    [
        column.checked_sub(1).map(|next| row * columns + next),
        (column + 1 < columns).then_some(index + 1),
        row.checked_sub(1).map(|next| next * columns + column),
        (row + 1 < rows).then_some(index + columns),
    ]
}

fn four_neighbors(index: usize, columns: u32, rows: u32) -> impl Iterator<Item = usize> {
    four_neighbor_slots(index, columns, rows)
        .into_iter()
        .flatten()
}

fn find_set(parents: &mut [u32], node: u32) -> u32 {
    let parent = parents[node as usize];
    if parent == node {
        node
    } else {
        let root = find_set(parents, parent);
        parents[node as usize] = root;
        root
    }
}

fn union_sets(parents: &mut [u32], left: u32, right: u32) {
    let left_root = find_set(parents, left);
    let right_root = find_set(parents, right);
    if left_root != right_root {
        let (minimum, maximum) = if left_root < right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        parents[maximum as usize] = minimum;
    }
}

fn component_distribution(
    mask: &[bool],
    columns: u32,
    rows: u32,
    step_blocks: u32,
) -> ComponentDistribution {
    let mut visited = vec![false; mask.len()];
    let mut sizes = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut size = 0_u32;
        while let Some(index) = queue.pop_front() {
            size += 1;
            let column = index % columns as usize;
            let row = index / columns as usize;
            for neighbor in [
                column
                    .checked_sub(1)
                    .map(|next| row * columns as usize + next),
                (column + 1 < columns as usize).then_some(row * columns as usize + column + 1),
                row.checked_sub(1)
                    .map(|next| next * columns as usize + column),
                (row + 1 < rows as usize).then_some((row + 1) * columns as usize + column),
            ]
            .into_iter()
            .flatten()
            {
                if mask[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        sizes.push(size);
    }
    sizes.sort_unstable();
    let maximum_samples = sizes.last().copied().unwrap_or(0);
    let sample_area = f64::from(step_blocks).powi(2) / 1_000_000.0;
    ComponentDistribution {
        component_count: sizes.len() as u32,
        covered_samples: sizes.iter().sum(),
        minimum_samples: sizes.first().copied().unwrap_or(0),
        median_samples: percentile(&sizes, 0.5),
        p90_samples: percentile(&sizes, 0.9),
        maximum_samples,
        maximum_area_square_km: f64::from(maximum_samples) * sample_area,
    }
}

fn kind_component_distribution(
    kinds: &[u8],
    columns: u32,
    rows: u32,
    step_blocks: u32,
    ignored_kind: Option<u8>,
) -> ComponentDistribution {
    let mut visited = vec![false; kinds.len()];
    let mut sizes = Vec::new();
    for start in 0..kinds.len() {
        if visited[start] || ignored_kind == Some(kinds[start]) {
            visited[start] = true;
            continue;
        }
        visited[start] = true;
        let kind = kinds[start];
        let mut queue = VecDeque::from([start]);
        let mut size = 0_u32;
        while let Some(index) = queue.pop_front() {
            size += 1;
            let column = index % columns as usize;
            let row = index / columns as usize;
            for neighbor in [
                column
                    .checked_sub(1)
                    .map(|next| row * columns as usize + next),
                (column + 1 < columns as usize).then_some(row * columns as usize + column + 1),
                row.checked_sub(1)
                    .map(|next| next * columns as usize + column),
                (row + 1 < rows as usize).then_some((row + 1) * columns as usize + column),
            ]
            .into_iter()
            .flatten()
            {
                if !visited[neighbor] && kinds[neighbor] == kind {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        sizes.push(size);
    }
    sizes.sort_unstable();
    let maximum_samples = sizes.last().copied().unwrap_or(0);
    let sample_area = f64::from(step_blocks).powi(2) / 1_000_000.0;
    ComponentDistribution {
        component_count: sizes.len() as u32,
        covered_samples: sizes.iter().sum(),
        minimum_samples: sizes.first().copied().unwrap_or(0),
        median_samples: percentile(&sizes, 0.5),
        p90_samples: percentile(&sizes, 0.9),
        maximum_samples,
        maximum_area_square_km: f64::from(maximum_samples) * sample_area,
    }
}

fn percentile(values: &[u32], percentile: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}

fn adjacency_pairs(kinds: &[u8], columns: u32, rows: u32) -> Vec<AdjacencyPair> {
    let mut pairs = BTreeMap::<(u8, u8), u32>::new();
    for row in 0..rows as usize {
        for column in 0..columns as usize {
            let index = row * columns as usize + column;
            for neighbor in [
                (column + 1 < columns as usize).then_some(index + 1),
                (row + 1 < rows as usize).then_some(index + columns as usize),
            ]
            .into_iter()
            .flatten()
            {
                let left = kinds[index];
                let right = kinds[neighbor];
                if left == right
                    || left == CONTINENTAL_ECOREGION_ATLAS_NONE
                    || right == CONTINENTAL_ECOREGION_ATLAS_NONE
                {
                    continue;
                }
                let pair = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                *pairs.entry(pair).or_default() += 1;
            }
        }
    }
    let mut result = pairs
        .into_iter()
        .map(|((left_kind, right_kind), boundary_edges)| AdjacencyPair {
            left_kind,
            right_kind,
            boundary_edges,
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .boundary_edges
            .cmp(&left.boundary_edges)
            .then_with(|| left.left_kind.cmp(&right.left_kind))
            .then_with(|| left.right_kind.cmp(&right.right_kind))
    });
    result
}

fn journey_receipts(
    arrays: &AtlasArrays,
    columns: u32,
    rows: u32,
    step_blocks: u32,
) -> Vec<JourneyReceipt> {
    let mut paths = Vec::new();
    for (label, row) in [
        ("west-east north", rows / 4),
        ("west-east center", rows / 2),
        ("west-east south", rows * 3 / 4),
    ] {
        paths.push((
            label,
            (0..columns).map(|column| row * columns + column).collect(),
        ));
    }
    for (label, column) in [
        ("north-south west", columns / 3),
        ("north-south east", columns * 2 / 3),
    ] {
        paths.push((label, (0..rows).map(|row| row * columns + column).collect()));
    }
    let diagonal_len = columns.min(rows);
    paths.push((
        "northwest-southeast",
        (0..diagonal_len)
            .map(|offset| {
                let denominator = diagonal_len.saturating_sub(1).max(1);
                let column = offset * columns.saturating_sub(1) / denominator;
                let row = offset * rows.saturating_sub(1) / denominator;
                row * columns + column
            })
            .collect(),
    ));
    paths
        .into_iter()
        .map(|(label, indices): (&'static str, Vec<u32>)| {
            journey_receipt(label, &indices, arrays, step_blocks)
        })
        .collect()
}

fn production_journey_receipts(
    arrays: &AtlasArrays,
    columns: u32,
    rows: u32,
    step_blocks: u32,
) -> Vec<JourneyReceipt> {
    let paths = journey_paths(columns, rows);
    paths
        .into_iter()
        .map(|(label, indices)| {
            let signatures = indices
                .iter()
                .map(|index| {
                    let index = *index as usize;
                    (
                        arrays.production_biome_kind[index],
                        arrays.production_land[index] / 16_384,
                        ((i32::from(arrays.production_relief[index]) + 32_768) / 8_192) as u8,
                        ((i32::from(arrays.production_moisture[index]) + 32_768) / 8_192) as u8,
                    )
                })
                .collect::<Vec<_>>();
            receipt_from_signatures(label, &signatures, step_blocks)
        })
        .collect()
}

fn journey_paths(columns: u32, rows: u32) -> Vec<(&'static str, Vec<u32>)> {
    let mut paths = Vec::new();
    for (label, row) in [
        ("west-east north", rows / 4),
        ("west-east center", rows / 2),
        ("west-east south", rows * 3 / 4),
    ] {
        paths.push((
            label,
            (0..columns).map(|column| row * columns + column).collect(),
        ));
    }
    for (label, column) in [
        ("north-south west", columns / 3),
        ("north-south east", columns * 2 / 3),
    ] {
        paths.push((label, (0..rows).map(|row| row * columns + column).collect()));
    }
    let diagonal_len = columns.min(rows);
    paths.push((
        "northwest-southeast",
        (0..diagonal_len)
            .map(|offset| {
                let denominator = diagonal_len.saturating_sub(1).max(1);
                let column = offset * columns.saturating_sub(1) / denominator;
                let row = offset * rows.saturating_sub(1) / denominator;
                row * columns + column
            })
            .collect(),
    ));
    paths
}

fn receipt_from_signatures<T: Copy + Ord>(
    label: &'static str,
    signatures: &[T],
    step_blocks: u32,
) -> JourneyReceipt {
    let mut runs = Vec::new();
    for signature in signatures {
        match runs.last_mut() {
            Some((current, length)) if current == signature => *length += 1,
            _ => runs.push((*signature, 1_u32)),
        }
    }
    let mut seen = BTreeSet::new();
    let repeated_scene_alarms = runs
        .iter()
        .filter(|(signature, _)| !seen.insert(*signature))
        .count() as u32;
    let dwell_total = runs.iter().map(|(_, length)| *length).sum::<u32>();
    JourneyReceipt {
        label,
        distance_blocks: signatures.len().saturating_sub(1) as u32 * step_blocks,
        run_count: runs.len() as u32,
        repeated_scene_alarms,
        mean_dwell_blocks: if runs.is_empty() {
            0
        } else {
            dwell_total * step_blocks / runs.len() as u32
        },
        longest_dwell_blocks: runs.iter().map(|(_, length)| *length).max().unwrap_or(0)
            * step_blocks,
    }
}

fn journey_receipt(
    label: &'static str,
    indices: &[u32],
    arrays: &AtlasArrays,
    step_blocks: u32,
) -> JourneyReceipt {
    let signatures = indices
        .iter()
        .map(|index| {
            let index = *index as usize;
            (
                arrays.continent_story[index],
                arrays.province_kind[index],
                arrays.ecoregion_kind[index],
                arrays.openness[index] / 16_384,
            )
        })
        .collect::<Vec<_>>();
    let mut runs = Vec::new();
    for signature in signatures {
        match runs.last_mut() {
            Some((current, length)) if *current == signature => *length += 1,
            _ => runs.push((signature, 1_u32)),
        }
    }
    let mut seen = BTreeSet::new();
    let repeated_scene_alarms = runs
        .iter()
        .filter(|(signature, _)| !seen.insert(*signature))
        .count() as u32;
    let dwell_total = runs.iter().map(|(_, length)| *length).sum::<u32>();
    JourneyReceipt {
        label,
        distance_blocks: indices.len().saturating_sub(1) as u32 * step_blocks,
        run_count: runs.len() as u32,
        repeated_scene_alarms,
        mean_dwell_blocks: if runs.is_empty() {
            0
        } else {
            dwell_total * step_blocks / runs.len() as u32
        },
        longest_dwell_blocks: runs.iter().map(|(_, length)| *length).max().unwrap_or(0)
            * step_blocks,
    }
}

fn validate_request(
    request: ContinentalEcoregionAtlasRequest,
) -> Result<(), ContinentalEcoregionError> {
    if request.blocks_across == 0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "atlas blocksAcross must be non-zero",
        ));
    }
    if !request.aspect_ratio.is_finite() || request.aspect_ratio <= 0.0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "atlas aspect ratio must be positive and finite",
        ));
    }
    if request.samples_across < 16 || request.samples_across > 512 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "atlas samplesAcross must be between 16 and 512",
        ));
    }
    Ok(())
}

fn centered_minimum(center: i32, extent: u32) -> Result<i32, ContinentalEcoregionError> {
    i32::try_from(i64::from(center) - i64::from(extent) / 2)
        .map_err(|_| ContinentalEcoregionError::CoordinateOverflow)
}

fn quantize_unit(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16
}

fn quantize_signed(value: f64) -> i16 {
    (value.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16
}

const fn biome_recipe_code(recipe: McloneOverworldBiomeRecipe) -> u8 {
    match recipe {
        McloneOverworldBiomeRecipe::Ocean => 0,
        McloneOverworldBiomeRecipe::Shore => 1,
        McloneOverworldBiomeRecipe::River => 2,
        McloneOverworldBiomeRecipe::SnowyAlpine => 3,
        McloneOverworldBiomeRecipe::CoolWetConifer => 4,
        McloneOverworldBiomeRecipe::WarmDrySteppe => 5,
        McloneOverworldBiomeRecipe::TemperateWoodland => 6,
        McloneOverworldBiomeRecipe::TemperateMeadow => 7,
    }
}

fn production_control_sha256(
    request: ContinentalEcoregionAtlasRequest,
    arrays: &AtlasArrays,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mclone-continental-ecoregion-production-control-v1");
    digest.update(MCLONE_OVERWORLD_FIELD_REVISION.as_bytes());
    digest.update(request.seed.to_le_bytes());
    digest.update(request.center_x.to_le_bytes());
    digest.update(request.center_z.to_le_bytes());
    digest.update(request.blocks_across.to_le_bytes());
    digest.update(request.aspect_ratio.to_bits().to_le_bytes());
    digest.update(request.samples_across.to_le_bytes());
    for value in &arrays.production_land {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_surface_y {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_temperature {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_moisture {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_relief {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_ruggedness {
        digest.update(value.to_le_bytes());
    }
    for value in &arrays.production_water {
        digest.update(value.to_le_bytes());
    }
    digest.update(&arrays.production_biome_kind);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn topology_label(topology: ContinentalEcoregionTopology) -> &'static str {
    match topology {
        ContinentalEcoregionTopology::Plane => "plane",
        ContinentalEcoregionTopology::CylinderX {
            period_blocks: 196_608,
        } => "cylinder-x-196608",
        ContinentalEcoregionTopology::CylinderX { .. } => "cylinder-x-custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_resolution_atlas_contains_typed_land_and_ocean() {
        let atlas = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest {
            aspect_ratio: 1.5,
            ..ContinentalEcoregionAtlasRequest::plane(12_345, 0, 0, 65_536)
        })
        .unwrap();
        assert_eq!(atlas.metadata.columns, 256);
        assert_eq!(atlas.metadata.sample_count, atlas.land.len() as u32);
        assert!(atlas.metadata.metrics.land_fraction > 0.2);
        assert!(atlas.metadata.metrics.ocean_fraction > 0.01);
        assert!(
            atlas
                .metadata
                .metrics
                .ecoregion_kind_counts
                .iter()
                .filter(|count| **count > 0)
                .count()
                >= 5
        );
        assert_eq!(atlas.metadata.work.exact_chunks, 0);
        assert_eq!(atlas.metadata.production_control_work.exact_chunks, 0);
        assert_eq!(atlas.production_land.len(), atlas.land.len());
        assert_eq!(atlas.production_biome_kind.len(), atlas.land.len());
        assert_eq!(
            atlas
                .metadata
                .production_control_metrics
                .biome_kind_counts
                .iter()
                .sum::<u32>(),
            atlas.metadata.sample_count
        );
        assert_eq!(
            atlas.metadata.production_control_revision,
            MCLONE_OVERWORLD_FIELD_REVISION
        );
        let metrics = &atlas.metadata.metrics;
        assert!(metrics.transition_width_blocks.observation_count > 0);
        assert!(metrics.transition_width_blocks.median > 0);
        assert!(metrics.clearing_plans.clearing_count > 0);
        assert_eq!(
            metrics.clearing_plans.clearing_count,
            metrics.clearing_plans.area_square_meters.observation_count
        );
        assert_eq!(
            metrics.clearing_plans.clearing_count,
            metrics.clearing_plans.edge_length_blocks.observation_count
        );
        assert!(
            metrics
                .regional_signature_recurrence_blocks
                .observation_count
                > 0
        );
        assert!(metrics.habitat_connectivity.patch_count > 0);
        assert!(
            metrics.habitat_connectivity.graph_edge_count
                <= metrics.habitat_connectivity.patch_count.pow(2)
        );
    }

    #[test]
    fn doubling_extent_keeps_direct_sample_count_fixed() {
        let small = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest::plane(
            12_345, 0, 0, 65_536,
        ))
        .unwrap();
        let large = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest::plane(
            12_345, 0, 0, 131_072,
        ))
        .unwrap();
        assert_eq!(small.metadata.sample_count, large.metadata.sample_count);
        assert_eq!(
            small.metadata.work.requested_samples,
            large.metadata.work.requested_samples
        );
        assert_eq!(
            small.metadata.sample_step_blocks * 2,
            large.metadata.sample_step_blocks
        );
        assert_ne!(
            small.metadata.semantic_sha256,
            large.metadata.semantic_sha256
        );
    }

    #[test]
    fn metrics_and_arrays_are_exactly_repeatable() {
        let request = ContinentalEcoregionAtlasRequest {
            center_x: -18_000,
            center_z: 24_000,
            blocks_across: 131_072,
            aspect_ratio: 1.7,
            ..ContinentalEcoregionAtlasRequest::plane(-98_765, 0, 0, 1)
        };
        let first = compile_continental_ecoregion_atlas(request).unwrap();
        let second = compile_continental_ecoregion_atlas(request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.metadata.metrics.journeys.len(), 6);
        assert!(!first.metadata.metrics.ecoregion_adjacencies.is_empty());
    }

    #[test]
    fn production_control_is_independent_of_candidate_topology() {
        let plane_request = ContinentalEcoregionAtlasRequest {
            aspect_ratio: 1.4,
            ..ContinentalEcoregionAtlasRequest::plane(12_345, 0, 0, 65_536)
        };
        let plane = compile_continental_ecoregion_atlas(plane_request).unwrap();
        let cylinder = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest {
            topology: ContinentalEcoregionTopology::CylinderX {
                period_blocks: 196_608,
            },
            ..plane_request
        })
        .unwrap();
        assert_eq!(plane.production_land, cylinder.production_land);
        assert_eq!(plane.production_biome_kind, cylinder.production_biome_kind);
        assert_eq!(
            plane.metadata.production_control_sha256,
            cylinder.metadata.production_control_sha256
        );
        assert_ne!(
            plane.metadata.semantic_sha256,
            cylinder.metadata.semantic_sha256
        );
    }

    #[test]
    fn production_control_matches_the_existing_reference_grid() {
        use crate::terrain_preview::{TerrainPreviewReferenceGrid, TerrainPreviewRequest};

        let atlas = compile_continental_ecoregion_atlas(ContinentalEcoregionAtlasRequest {
            samples_across: 128,
            ..ContinentalEcoregionAtlasRequest::plane(12_345, 0, 0, 65_536)
        })
        .unwrap();
        let reference = TerrainPreviewReferenceGrid::compile(TerrainPreviewRequest {
            cells_per_axis: 128,
            sample_spacing: 512,
            ..TerrainPreviewRequest::new(12_345, 0, 0, 512)
        })
        .unwrap();
        for row in 0..128_u32 {
            for column in 0..128_u32 {
                let index = (row * 128 + column) as usize;
                let expected = reference.sample(column, row).unwrap();
                assert!(
                    atlas.production_land[index]
                        .abs_diff(quantize_unit(expected.continentalness * 0.5 + 0.5))
                        <= 1
                );
                assert_eq!(atlas.production_surface_y[index], expected.surface_y as i16);
                assert_eq!(
                    atlas.production_biome_kind[index],
                    expected.biome_recipe as u8
                );
            }
        }
    }
}
