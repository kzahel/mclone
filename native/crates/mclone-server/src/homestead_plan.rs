//! Durable realized plan for the version-one intro homestead.
//!
//! Site selection remains profile-neutral in `mclone-worldgen`. This module
//! binds one selected site to authoritative world identity, exact composition
//! slots, grading/path/water facts, and content fingerprints before any
//! plan-owned chunk can be materialized.

use std::collections::BTreeSet;

use mclone_core::{AxisTopology, BlockPos, ChunkPos, HorizontalTopology};
use mclone_worldgen::homestead_site::{
    FlatGrassHomesteadSurveySource, HomesteadBounds2d, HomesteadCompositionTier, HomesteadRotation,
    HomesteadScoutReceipt, HomesteadScoutRequest, HomesteadSurveySource,
    McloneOverworldHomesteadSurveySource, SelectedHomesteadSite, scout_homestead_site,
};
use mclone_worldgen::levelgen::McloneOverworldSamplingTopology;
use mclone_worldgen::structure_json::{CanonicalStructureRecord, load_canonical_structure_json};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    REALIZED_STARTER_PLAN_SAVED_DATA_KEY, RealizedStarterPlanIdentity, SavedDataRecord,
    StarterContentDescriptor, WorldGenerationProfile,
};

pub const INTRO_HOMESTEAD_PLAN_SCHEMA_VERSION: u32 = 1;
pub const INTRO_HOMESTEAD_PLANNER_REVISION: u32 = 1;
pub const INTRO_HOMESTEAD_COMPOSITION_REVISION: u32 = 1;
pub const INTRO_HOMESTEAD_PLAN_RECORD_REVISION: u64 = 1;

const COTTAGE_STANDARD_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-cottage-a-v2.structure.json"
));
const COTTAGE_SNUG_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-cottage-snug-canopy-porch-v1.structure.json"
));
const BARN_STANDARD_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-barn-core-a-v2.structure.json"
));
const BARN_SHORT_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-barn-core-short-v1.structure.json"
));
const LEAN_TO_STANDARD_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-barn-lean-to-a-v2.structure.json"
));
const LEAN_TO_SHORT_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-barn-lean-to-short-v1.structure.json"
));
const COOP_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/structures/farmstead-rosehip-chicken-coop-v1.structure.json"
));

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadPlanAxisTopology {
    Unbounded,
    Finite {
        minimum_chunk: i32,
        maximum_chunk_exclusive: i32,
    },
    Periodic {
        minimum_chunk: i32,
        period_chunks: u32,
    },
}

impl From<AxisTopology> for HomesteadPlanAxisTopology {
    fn from(value: AxisTopology) -> Self {
        match value {
            AxisTopology::Unbounded => Self::Unbounded,
            AxisTopology::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => Self::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            },
            AxisTopology::Periodic {
                minimum_chunk,
                period_chunks,
            } => Self::Periodic {
                minimum_chunk,
                period_chunks,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPlanTopology {
    pub x: HomesteadPlanAxisTopology,
    pub z: HomesteadPlanAxisTopology,
}

impl From<HorizontalTopology> for HomesteadPlanTopology {
    fn from(value: HorizontalTopology) -> Self {
        Self {
            x: value.x.into(),
            z: value.z.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPlanBaseDescriptor {
    pub seed: i64,
    pub generation_profile: WorldGenerationProfile,
    pub topology: HomesteadPlanTopology,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadPlanPieceKind {
    Grading,
    ArrivalPath,
    AuthoredWater,
    Cottage,
    Barn,
    BarnLeanTo,
    ChickenCoop,
    Garden,
    AnimalYard,
    FocalOak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadPlanTemplateRotation {
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl HomesteadPlanTemplateRotation {
    const fn quarter_turns(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Clockwise90 => 1,
            Self::Clockwise180 => 2,
            Self::CounterClockwise90 => 3,
        }
    }

    const fn from_quarter_turns(turns: u8) -> Self {
        match turns % 4 {
            0 => Self::None,
            1 => Self::Clockwise90,
            2 => Self::Clockwise180,
            _ => Self::CounterClockwise90,
        }
    }

    const fn compose(self, other: Self) -> Self {
        Self::from_quarter_turns(self.quarter_turns() + other.quarter_turns())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPlanBounds3d {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPlanPiece {
    pub piece_id: String,
    pub kind: HomesteadPlanPieceKind,
    pub content_id: String,
    pub origin: [i32; 3],
    pub rotation: HomesteadPlanTemplateRotation,
    pub bounds: HomesteadPlanBounds3d,
    pub touched_chunks: Vec<[i32; 2]>,
    pub semantic_sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadGradeRegion {
    pub region_id: String,
    pub bounds: HomesteadBounds2d,
    pub target_surface_y: i32,
    pub feather_blocks: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPathControl {
    pub pos: [i32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPathPlan {
    pub path_id: String,
    pub width_blocks: u8,
    pub feather_blocks: u8,
    pub controls: Vec<HomesteadPathControl>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadWaterPlan {
    pub water_id: String,
    pub authored_fallback: bool,
    pub bounds: HomesteadBounds2d,
    pub surface_y: i32,
    pub maximum_depth: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadPlantingPlan {
    pub focal_oak: [i32; 3],
    pub generic_decoration_reservation: HomesteadBounds2d,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadResidentMarkerPlan {
    pub marker_id: String,
    pub entity_kind: String,
    pub persistent_id: [u8; 16],
    pub pos: [i32; 3],
    pub count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomesteadContentFingerprint {
    pub content_id: String,
    pub compiler_id: String,
    pub source_sha256: String,
    pub semantic_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroHomesteadPlanRecord {
    pub schema_version: u32,
    pub base_descriptor: HomesteadPlanBaseDescriptor,
    pub starter_content: StarterContentDescriptor,
    pub planner_revision: u32,
    pub composition_revision: u32,
    pub provisional_spawn: [i32; 3],
    pub selected_site: SelectedHomesteadSite,
    pub pieces: Vec<HomesteadPlanPiece>,
    pub grade_regions: Vec<HomesteadGradeRegion>,
    pub arrival_path: HomesteadPathPlan,
    pub water: HomesteadWaterPlan,
    pub planting: HomesteadPlantingPlan,
    pub resident_markers: Vec<HomesteadResidentMarkerPlan>,
    pub source_fingerprints: Vec<HomesteadContentFingerprint>,
    pub fallback_facts: Vec<String>,
    pub scout_checksum_sha256: String,
    pub checksum_sha256: String,
}

impl IntroHomesteadPlanRecord {
    pub fn identity(&self) -> Result<RealizedStarterPlanIdentity, String> {
        Ok(RealizedStarterPlanIdentity::new(
            self.planner_revision,
            self.composition_revision,
            decode_sha256(&self.checksum_sha256)?,
        ))
    }

    pub fn verify_checksum(&self) -> Result<(), String> {
        let expected = plan_checksum(self)?;
        if self.checksum_sha256 != expected {
            return Err(format!(
                "realized homestead plan checksum {} did not match {expected}",
                self.checksum_sha256
            ));
        }
        Ok(())
    }

    pub fn saved_data_record(&self) -> Result<SavedDataRecord, String> {
        self.verify_checksum()?;
        SavedDataRecord::new(
            REALIZED_STARTER_PLAN_SAVED_DATA_KEY,
            INTRO_HOMESTEAD_PLAN_SCHEMA_VERSION,
            INTRO_HOMESTEAD_PLAN_RECORD_REVISION,
            serde_json::to_vec(self).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }
}

pub fn decode_intro_homestead_plan(
    record: &SavedDataRecord,
) -> Result<IntroHomesteadPlanRecord, String> {
    if record.key != REALIZED_STARTER_PLAN_SAVED_DATA_KEY {
        return Err(format!(
            "unexpected realized starter plan key {}",
            record.key
        ));
    }
    if record.codec_version != INTRO_HOMESTEAD_PLAN_SCHEMA_VERSION {
        return Err(format!(
            "unsupported realized homestead plan codec {}",
            record.codec_version
        ));
    }
    if record.revision != INTRO_HOMESTEAD_PLAN_RECORD_REVISION {
        return Err(format!(
            "unsupported realized homestead plan record revision {}",
            record.revision
        ));
    }
    let plan: IntroHomesteadPlanRecord =
        serde_json::from_slice(&record.bytes).map_err(|error| error.to_string())?;
    if plan.schema_version != INTRO_HOMESTEAD_PLAN_SCHEMA_VERSION
        || plan.planner_revision != INTRO_HOMESTEAD_PLANNER_REVISION
        || plan.composition_revision != INTRO_HOMESTEAD_COMPOSITION_REVISION
    {
        return Err("realized homestead plan revisions are unsupported".to_owned());
    }
    plan.verify_checksum()?;
    Ok(plan)
}

pub fn realize_intro_homestead_plan(
    seed: i64,
    generation_profile: WorldGenerationProfile,
    topology: HorizontalTopology,
) -> Result<IntroHomesteadPlanRecord, String> {
    let spawn_chunk =
        crate::initial_spawn_center_for_descriptor(seed, generation_profile, topology);
    let provisional_spawn = BlockPos::new(
        spawn_chunk.min_block_x() + 8,
        if generation_profile == WorldGenerationProfile::FlatGrassV1 {
            4
        } else {
            0
        },
        spawn_chunk.min_block_z() + 8,
    );
    let request = HomesteadScoutRequest {
        seed,
        provisional_spawn,
        topology,
    };
    match generation_profile {
        WorldGenerationProfile::McloneOverworldV1 => {
            let sampling_topology =
                McloneOverworldSamplingTopology::from_horizontal_topology(topology)?;
            let mut source = McloneOverworldHomesteadSurveySource::new(seed, sampling_topology);
            let receipt =
                scout_homestead_site(&mut source, request).map_err(|error| error.to_string())?;
            compile_intro_homestead_plan(&mut source, generation_profile, topology, &receipt)
        }
        WorldGenerationProfile::FlatGrassV1 => {
            let mut source = FlatGrassHomesteadSurveySource;
            let receipt =
                scout_homestead_site(&mut source, request).map_err(|error| error.to_string())?;
            compile_intro_homestead_plan(&mut source, generation_profile, topology, &receipt)
        }
        _ => Err(format!(
            "starter content {} does not yet admit world generation profile {}",
            StarterContentDescriptor::IntroHomesteadV1.id(),
            generation_profile.label()
        )),
    }
}

pub fn validate_intro_homestead_plan_for_world(
    plan: &IntroHomesteadPlanRecord,
    seed: i64,
    generation_profile: WorldGenerationProfile,
    topology: HorizontalTopology,
) -> Result<(), String> {
    if plan.base_descriptor
        != (HomesteadPlanBaseDescriptor {
            seed,
            generation_profile,
            topology: topology.into(),
        })
    {
        return Err("realized homestead plan base descriptor does not match the world".to_owned());
    }
    if plan.starter_content != StarterContentDescriptor::IntroHomesteadV1 {
        return Err("realized homestead plan has the wrong starter-content identity".to_owned());
    }
    if plan.scout_checksum_sha256 != plan.selected_site.checksum_sha256 {
        return Err("realized homestead plan scout checksum is internally inconsistent".to_owned());
    }
    plan.verify_checksum()
}

pub fn compile_intro_homestead_plan(
    source: &mut impl HomesteadSurveySource,
    generation_profile: WorldGenerationProfile,
    topology: HorizontalTopology,
    receipt: &HomesteadScoutReceipt,
) -> Result<IntroHomesteadPlanRecord, String> {
    let selected = receipt
        .selected
        .clone()
        .ok_or_else(|| "cannot realize a homestead plan without a selected site".to_owned())?;
    let assets = composition_assets(selected.tier)?;
    let grade_regions = grade_regions(source, &selected, selected.tier)?;
    let layout_rotation = layout_rotation(selected.facing);
    let mut pieces = Vec::new();
    let surface_y = selected.metrics.target_surface_y;
    let core_half = match selected.tier {
        HomesteadCompositionTier::FullV1 => 48,
        HomesteadCompositionTier::CompactV1 => 32,
    };

    pieces.push(semantic_piece(
        "grade-v1",
        HomesteadPlanPieceKind::Grading,
        "mclone:intro-homestead-grade-v1",
        local_bounds(&selected, -core_half, core_half, -core_half, core_half),
        surface_y - 7,
        surface_y + 7,
        topology,
    )?);
    pieces.push(semantic_piece(
        "arrival-path-v1",
        HomesteadPlanPieceKind::ArrivalPath,
        "mclone:intro-homestead-arrival-path-v1",
        local_bounds(&selected, tier_min_forward(selected.tier), 2, -5, 5),
        surface_y - 2,
        surface_y + 1,
        topology,
    )?);

    for asset in &assets {
        let foundation_region_id = match asset.kind {
            HomesteadPlanPieceKind::Cottage => "cottage-foundation-v1",
            HomesteadPlanPieceKind::Barn | HomesteadPlanPieceKind::BarnLeanTo => {
                "barn-foundation-v1"
            }
            HomesteadPlanPieceKind::ChickenCoop => "coop-foundation-v1",
            _ => unreachable!("composition assets are all buildings"),
        };
        let foundation_y = grade_regions
            .iter()
            .find(|region| region.region_id == foundation_region_id)
            .expect("all building assets have a declared foundation region")
            .target_surface_y;
        pieces.push(template_piece(
            &selected,
            asset,
            layout_rotation,
            foundation_y,
            topology,
        )?);
    }
    for (piece_id, kind, content_id, forward_min, forward_max, right_min, right_max) in
        semantic_layout(selected.tier)
    {
        pieces.push(semantic_piece(
            piece_id,
            kind,
            content_id,
            local_bounds(&selected, forward_min, forward_max, right_min, right_max),
            surface_y - 3,
            surface_y + 8,
            topology,
        )?);
    }
    pieces.sort_by(|left, right| left.piece_id.cmp(&right.piece_id));

    let arrival_path = arrival_path(source, &selected, selected.tier)?;
    let (pond_right_min, pond_right_max) = if selected.tier == HomesteadCompositionTier::FullV1 {
        (22, 35)
    } else {
        (18, 28)
    };
    let pond_bounds = local_bounds(&selected, -12, 3, pond_right_min, pond_right_max);
    let pond_center = local_pos(&selected, -4, (pond_right_min + pond_right_max) / 2);
    let pond_surface = source
        .sample_column(pond_center.x, pond_center.z)?
        .surface_y;
    let focal_oak = local_pos(&selected, -6, -12);
    let residents = resident_markers(&selected);
    let source_fingerprints = assets
        .iter()
        .map(|asset| HomesteadContentFingerprint {
            content_id: asset.record.template.id().to_owned(),
            compiler_id: asset.record.provenance.compiler_id.clone(),
            source_sha256: asset.record.provenance.source_sha256.clone(),
            semantic_sha256: asset.record.provenance.semantic_sha256.clone(),
        })
        .collect();
    let mut plan = IntroHomesteadPlanRecord {
        schema_version: INTRO_HOMESTEAD_PLAN_SCHEMA_VERSION,
        base_descriptor: HomesteadPlanBaseDescriptor {
            seed: receipt.seed,
            generation_profile,
            topology: topology.into(),
        },
        starter_content: StarterContentDescriptor::IntroHomesteadV1,
        planner_revision: INTRO_HOMESTEAD_PLANNER_REVISION,
        composition_revision: INTRO_HOMESTEAD_COMPOSITION_REVISION,
        provisional_spawn: receipt.provisional_spawn,
        selected_site: selected.clone(),
        pieces,
        grade_regions,
        arrival_path,
        water: HomesteadWaterPlan {
            water_id: "authored-pond-v1".to_owned(),
            authored_fallback: true,
            bounds: pond_bounds,
            surface_y: pond_surface,
            maximum_depth: 2,
        },
        planting: HomesteadPlantingPlan {
            focal_oak: [focal_oak.x, surface_y + 1, focal_oak.z],
            generic_decoration_reservation: selected.reservation_bounds,
        },
        resident_markers: residents,
        source_fingerprints,
        fallback_facts: vec![
            "authored-pond-v1 selected; natural hydrology attachment deferred".to_owned(),
            "generic decoration suppressed inside the explicit reservation".to_owned(),
        ],
        scout_checksum_sha256: selected.checksum_sha256.clone(),
        checksum_sha256: String::new(),
    };
    plan.checksum_sha256 = plan_checksum(&plan)?;
    Ok(plan)
}

struct CompositionAsset {
    piece_id: &'static str,
    kind: HomesteadPlanPieceKind,
    record: CanonicalStructureRecord,
    forward: i32,
    right: i32,
    base_rotation: HomesteadPlanTemplateRotation,
}

fn composition_assets(tier: HomesteadCompositionTier) -> Result<Vec<CompositionAsset>, String> {
    let load = |json| load_canonical_structure_json(json).map_err(|error| error.to_string());
    let (
        cottage_json,
        barn_json,
        lean_json,
        cottage_forward,
        cottage_right,
        barn_right,
        lean_right,
    ) = match tier {
        HomesteadCompositionTier::FullV1 => (
            COTTAGE_STANDARD_JSON,
            BARN_STANDARD_JSON,
            LEAN_TO_STANDARD_JSON,
            -34,
            -43,
            -44,
            -42,
        ),
        HomesteadCompositionTier::CompactV1 => (
            COTTAGE_SNUG_JSON,
            BARN_SHORT_JSON,
            LEAN_TO_SHORT_JSON,
            -29,
            -30,
            -30,
            -28,
        ),
    };
    Ok(vec![
        CompositionAsset {
            piece_id: "building-cottage-v1",
            kind: HomesteadPlanPieceKind::Cottage,
            record: load(cottage_json)?,
            forward: cottage_forward,
            right: cottage_right,
            base_rotation: HomesteadPlanTemplateRotation::None,
        },
        CompositionAsset {
            piece_id: "building-barn-v1",
            kind: HomesteadPlanPieceKind::Barn,
            record: load(barn_json)?,
            forward: 5,
            right: barn_right,
            base_rotation: HomesteadPlanTemplateRotation::None,
        },
        CompositionAsset {
            piece_id: "building-barn-lean-to-v1",
            kind: HomesteadPlanPieceKind::BarnLeanTo,
            record: load(lean_json)?,
            forward: 25,
            right: lean_right,
            base_rotation: HomesteadPlanTemplateRotation::None,
        },
        CompositionAsset {
            piece_id: "building-chicken-coop-v1",
            kind: HomesteadPlanPieceKind::ChickenCoop,
            record: load(COOP_JSON)?,
            forward: 12,
            right: if tier == HomesteadCompositionTier::FullV1 {
                19
            } else {
                15
            },
            base_rotation: HomesteadPlanTemplateRotation::Clockwise180,
        },
    ])
}

fn template_piece(
    selected: &SelectedHomesteadSite,
    asset: &CompositionAsset,
    layout_rotation: HomesteadPlanTemplateRotation,
    foundation_y: i32,
    topology: HorizontalTopology,
) -> Result<HomesteadPlanPiece, String> {
    let size = asset.record.template.size();
    let base_rotation = asset.base_rotation;
    let (base_width, base_depth) = if base_rotation.quarter_turns() % 2 == 0 {
        (size[0], size[2])
    } else {
        (size[2], size[0])
    };
    let horizontal = local_bounds(
        selected,
        asset.forward,
        asset.forward + base_width - 1,
        asset.right,
        asset.right + base_depth - 1,
    );
    let origin = [horizontal.min_x, foundation_y, horizontal.min_z];
    let bounds = HomesteadPlanBounds3d {
        min: origin,
        max: [horizontal.max_x, origin[1] + size[1] - 1, horizontal.max_z],
    };
    Ok(HomesteadPlanPiece {
        piece_id: asset.piece_id.to_owned(),
        kind: asset.kind,
        content_id: asset.record.template.id().to_owned(),
        origin,
        rotation: base_rotation.compose(layout_rotation),
        touched_chunks: touched_chunks(bounds, topology)?,
        bounds,
        semantic_sha256: Some(asset.record.provenance.semantic_sha256.clone()),
    })
}

fn semantic_piece(
    piece_id: &str,
    kind: HomesteadPlanPieceKind,
    content_id: &str,
    horizontal: HomesteadBounds2d,
    min_y: i32,
    max_y: i32,
    topology: HorizontalTopology,
) -> Result<HomesteadPlanPiece, String> {
    let bounds = HomesteadPlanBounds3d {
        min: [horizontal.min_x, min_y, horizontal.min_z],
        max: [horizontal.max_x, max_y, horizontal.max_z],
    };
    Ok(HomesteadPlanPiece {
        piece_id: piece_id.to_owned(),
        kind,
        content_id: content_id.to_owned(),
        origin: bounds.min,
        rotation: HomesteadPlanTemplateRotation::None,
        touched_chunks: touched_chunks(bounds, topology)?,
        bounds,
        semantic_sha256: None,
    })
}

fn semantic_layout(
    tier: HomesteadCompositionTier,
) -> [(
    &'static str,
    HomesteadPlanPieceKind,
    &'static str,
    i32,
    i32,
    i32,
    i32,
); 4] {
    let compact = tier == HomesteadCompositionTier::CompactV1;
    [
        (
            "garden-v1",
            HomesteadPlanPieceKind::Garden,
            "mclone:intro-homestead-garden-v1",
            if compact { -28 } else { -38 },
            if compact { -12 } else { -17 },
            if compact { 10 } else { 14 },
            if compact { 27 } else { 31 },
        ),
        (
            "animal-yard-v1",
            HomesteadPlanPieceKind::AnimalYard,
            "mclone:intro-homestead-animal-yard-v1",
            if compact { 8 } else { 8 },
            if compact { 30 } else { 35 },
            if compact { 11 } else { 15 },
            if compact { 30 } else { 35 },
        ),
        (
            "authored-pond-v1",
            HomesteadPlanPieceKind::AuthoredWater,
            "mclone:intro-homestead-pond-v1",
            -12,
            3,
            if compact { 18 } else { 22 },
            if compact { 28 } else { 35 },
        ),
        (
            "focal-oak-v1",
            HomesteadPlanPieceKind::FocalOak,
            "mclone:intro-homestead-focal-oak-v1",
            -10,
            -2,
            -16,
            -8,
        ),
    ]
}

fn grade_regions(
    source: &mut impl HomesteadSurveySource,
    selected: &SelectedHomesteadSite,
    tier: HomesteadCompositionTier,
) -> Result<Vec<HomesteadGradeRegion>, String> {
    let compact = tier == HomesteadCompositionTier::CompactV1;
    let layouts = [
        (
            "cottage-foundation-v1",
            if compact { -31 } else { -36 },
            if compact { -13 } else { -16 },
            if compact { -32 } else { -45 },
            if compact { -14 } else { -25 },
            3,
        ),
        (
            "barn-foundation-v1",
            3,
            if compact { 31 } else { 33 },
            if compact { -32 } else { -46 },
            if compact { -15 } else { -25 },
            3,
        ),
        (
            "coop-foundation-v1",
            10,
            if compact { 29 } else { 29 },
            if compact { 13 } else { 17 },
            if compact { 28 } else { 32 },
            2,
        ),
    ];
    layouts
        .into_iter()
        .map(|(id, f0, f1, r0, r1, feather)| {
            let bounds = local_bounds(selected, f0, f1, r0, r1);
            Ok(HomesteadGradeRegion {
                region_id: id.to_owned(),
                target_surface_y: median_surface(source, bounds)?,
                bounds,
                feather_blocks: feather,
            })
        })
        .collect()
}

fn median_surface(
    source: &mut impl HomesteadSurveySource,
    bounds: HomesteadBounds2d,
) -> Result<i32, String> {
    let mut heights = Vec::new();
    for z in (bounds.min_z..=bounds.max_z).step_by(4) {
        for x in (bounds.min_x..=bounds.max_x).step_by(4) {
            heights.push(source.sample_column(x, z)?.surface_y);
        }
    }
    heights.sort_unstable();
    heights
        .get(heights.len() / 2)
        .copied()
        .ok_or_else(|| "homestead grade region sampled no columns".to_owned())
}

fn arrival_path(
    source: &mut impl HomesteadSurveySource,
    selected: &SelectedHomesteadSite,
    tier: HomesteadCompositionTier,
) -> Result<HomesteadPathPlan, String> {
    let start = tier_min_forward(tier);
    let mut controls = Vec::new();
    let mut previous = None;
    for forward in (start..=0).step_by(8) {
        let point = local_pos(selected, forward, 0);
        let natural = source.sample_column(point.x, point.z)?.surface_y;
        let target = previous.map_or(natural, |previous: i32| {
            natural.clamp(previous - 1, previous + 1)
        });
        previous = Some(target);
        controls.push(HomesteadPathControl {
            pos: [point.x, target + 1, point.z],
        });
    }
    Ok(HomesteadPathPlan {
        path_id: "arrival-path-v1".to_owned(),
        width_blocks: 5,
        feather_blocks: 2,
        controls,
    })
}

fn resident_markers(selected: &SelectedHomesteadSite) -> Vec<HomesteadResidentMarkerPlan> {
    let cow = local_pos(selected, 29, 25);
    let chicken = local_pos(selected, 20, 23);
    vec![
        HomesteadResidentMarkerPlan {
            marker_id: "resident-cows-v1".to_owned(),
            entity_kind: "minecraft:cow".to_owned(),
            persistent_id: [0x43, 0x4f, 0x57, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            pos: [cow.x, selected.metrics.target_surface_y + 1, cow.z],
            count: 2,
        },
        HomesteadResidentMarkerPlan {
            marker_id: "resident-chickens-v1".to_owned(),
            entity_kind: "minecraft:chicken".to_owned(),
            persistent_id: [
                0x43, 0x48, 0x49, 0x43, 0x4b, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
            pos: [chicken.x, selected.metrics.target_surface_y + 1, chicken.z],
            count: 3,
        },
    ]
}

fn local_bounds(
    selected: &SelectedHomesteadSite,
    forward_min: i32,
    forward_max: i32,
    right_min: i32,
    right_max: i32,
) -> HomesteadBounds2d {
    let corners = [
        local_pos(selected, forward_min, right_min),
        local_pos(selected, forward_min, right_max),
        local_pos(selected, forward_max, right_min),
        local_pos(selected, forward_max, right_max),
    ];
    HomesteadBounds2d {
        min_x: corners.iter().map(|pos| pos.x).min().unwrap(),
        min_z: corners.iter().map(|pos| pos.z).min().unwrap(),
        max_x: corners.iter().map(|pos| pos.x).max().unwrap(),
        max_z: corners.iter().map(|pos| pos.z).max().unwrap(),
    }
}

fn local_pos(selected: &SelectedHomesteadSite, forward: i32, right: i32) -> BlockPos {
    let anchor_x = selected.candidate.anchor_x;
    let anchor_z = selected.candidate.anchor_z;
    match selected.facing {
        HomesteadRotation::East => BlockPos::new(anchor_x + forward, 0, anchor_z + right),
        HomesteadRotation::South => BlockPos::new(anchor_x - right, 0, anchor_z + forward),
        HomesteadRotation::West => BlockPos::new(anchor_x - forward, 0, anchor_z - right),
        HomesteadRotation::North => BlockPos::new(anchor_x + right, 0, anchor_z - forward),
    }
}

fn layout_rotation(facing: HomesteadRotation) -> HomesteadPlanTemplateRotation {
    match facing {
        HomesteadRotation::East => HomesteadPlanTemplateRotation::None,
        HomesteadRotation::South => HomesteadPlanTemplateRotation::Clockwise90,
        HomesteadRotation::West => HomesteadPlanTemplateRotation::Clockwise180,
        HomesteadRotation::North => HomesteadPlanTemplateRotation::CounterClockwise90,
    }
}

fn tier_min_forward(tier: HomesteadCompositionTier) -> i32 {
    match tier {
        HomesteadCompositionTier::FullV1 => -48,
        HomesteadCompositionTier::CompactV1 => -32,
    }
}

fn touched_chunks(
    bounds: HomesteadPlanBounds3d,
    topology: HorizontalTopology,
) -> Result<Vec<[i32; 2]>, String> {
    let min = ChunkPos::new(bounds.min[0].div_euclid(16), bounds.min[2].div_euclid(16));
    let max = ChunkPos::new(bounds.max[0].div_euclid(16), bounds.max[2].div_euclid(16));
    let mut chunks = BTreeSet::new();
    for z in min.z..=max.z {
        for x in min.x..=max.x {
            let chunk = topology
                .canonicalize_chunk(ChunkPos::new(x, z))
                .ok_or_else(|| {
                    format!("homestead plan piece touches out-of-topology chunk ({x},{z})")
                })?;
            chunks.insert([chunk.x, chunk.z]);
        }
    }
    Ok(chunks.into_iter().collect())
}

fn plan_checksum(plan: &IntroHomesteadPlanRecord) -> Result<String, String> {
    let mut unsigned = plan.clone();
    unsigned.checksum_sha256.clear();
    let bytes = serde_json::to_vec(&unsigned).map_err(|error| error.to_string())?;
    Ok(hex_sha256(Sha256::digest(bytes).into()))
}

fn hex_sha256(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn decode_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("SHA-256 value must contain 64 lowercase hex characters".to_owned());
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| "SHA-256 value contains invalid hex".to_owned())?;
    }
    if hex_sha256(bytes) != value {
        return Err("SHA-256 value must use lowercase hex".to_owned());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    use crate::{DimensionDefinition, RealmServer, SqliteWorldStore, WorldStore};
    use mclone_worldgen::homestead_site::{
        FlatGrassHomesteadSurveySource, HomesteadScoutRequest, scout_homestead_site,
    };

    fn flat_plan() -> IntroHomesteadPlanRecord {
        let request = HomesteadScoutRequest {
            seed: 8_675_309,
            provisional_spawn: BlockPos::new(8, 4, 8),
            topology: HorizontalTopology::UNBOUNDED,
        };
        let receipt = scout_homestead_site(&mut FlatGrassHomesteadSurveySource, request).unwrap();
        compile_intro_homestead_plan(
            &mut FlatGrassHomesteadSurveySource,
            WorldGenerationProfile::FlatGrassV1,
            request.topology,
            &receipt,
        )
        .unwrap()
    }

    #[test]
    fn flat_realized_plan_is_stable_bounded_and_codec_round_trips() {
        let plan = flat_plan();
        assert_eq!(plan.selected_site.tier, HomesteadCompositionTier::FullV1);
        assert_eq!(plan.pieces.len(), 10);
        assert_eq!(plan.grade_regions.len(), 3);
        assert_eq!(plan.resident_markers.len(), 2);
        assert!(plan.saved_data_record().unwrap().bytes.len() < 64 * 1024);
        let decoded = decode_intro_homestead_plan(&plan.saved_data_record().unwrap()).unwrap();
        assert_eq!(decoded, plan);
        assert_eq!(
            decoded.identity().unwrap().checksum,
            plan.identity().unwrap().checksum
        );
    }

    #[test]
    fn realized_plan_rebuild_is_source_cache_and_order_independent() {
        assert_eq!(flat_plan(), flat_plan());
    }

    #[test]
    fn accepted_mclone_showcase_compiles_through_the_production_scout() {
        let plan = realize_intro_homestead_plan(
            8_675_309,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        assert_eq!(plan.selected_site.candidate.anchor_x, 744);
        assert_eq!(plan.selected_site.candidate.anchor_z, -376);
        assert_eq!(plan.selected_site.facing, HomesteadRotation::East);
        assert_eq!(plan.selected_site.arrival, [696, 63, -376]);
        assert_eq!(
            plan.scout_checksum_sha256,
            "127c1066b6994cfc5a056882511bd6e26addb455c9fa75298f523217e62d3c32"
        );
        assert_eq!(
            plan,
            realize_intro_homestead_plan(
                8_675_309,
                WorldGenerationProfile::McloneOverworldV1,
                HorizontalTopology::UNBOUNDED,
            )
            .unwrap()
        );
        assert_eq!(
            plan.checksum_sha256,
            "d6fa4657806865193d688dfba6d743895e630ef60b34eb75cfac2711a7faa8ec"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn accepted_plan_body_precedes_metadata_binding_and_reopens_from_sqlite() {
        let root = std::env::temp_dir().join(format!(
            "mclone-homestead-plan-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let definition =
            DimensionDefinition::overworld(8_675_309, WorldGenerationProfile::McloneOverworldV1);

        let first_checksum = {
            let store = SqliteWorldStore::open_world_dir(&root).unwrap();
            let mut server = RealmServer::with_world_store_and_dimension_definition(
                definition.clone(),
                Box::new(store),
            );
            server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
            let metadata = server.initialize_world_metadata_blocking().unwrap();
            let plan = server.intro_homestead_plan().unwrap().clone();
            assert_eq!(
                metadata.realized_starter_plan,
                Some(plan.identity().unwrap())
            );
            server.shutdown_persistence().unwrap();
            plan.checksum_sha256
        };

        {
            let mut store = SqliteWorldStore::open_world_dir(&root).unwrap();
            let body = store
                .load_saved_data(REALIZED_STARTER_PLAN_SAVED_DATA_KEY)
                .unwrap()
                .unwrap();
            assert_eq!(
                decode_intro_homestead_plan(&body).unwrap().checksum_sha256,
                first_checksum
            );
            store.close().unwrap();
        }

        {
            let store = SqliteWorldStore::open_world_dir(&root).unwrap();
            let mut reopened =
                RealmServer::with_world_store_and_dimension_definition(definition, Box::new(store));
            reopened.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
            reopened.initialize_world_metadata_blocking().unwrap();
            assert_eq!(
                reopened.intro_homestead_plan().unwrap().checksum_sha256,
                first_checksum
            );
            reopened.shutdown_persistence().unwrap();
        }

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tampered_realized_plan_fails_closed() {
        let plan = flat_plan();
        let mut record = plan.saved_data_record().unwrap();
        let index = record.bytes.len() / 2;
        record.bytes[index] ^= 1;
        assert!(decode_intro_homestead_plan(&record).is_err());
    }
}
