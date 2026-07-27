//! Research-only parent/child semantic terrain refinement witness.
//!
//! This module proves direct coarse queries, stable cross-level identity,
//! bounded owner enumeration, and exact traversal/topology invariance. It is
//! intentionally disconnected from production terrain generation.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::BlockPos;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::streamed_plan_harness::{
    HarnessComparisonReceipt, HarnessDependencyClaim, PlanRegion, STREAMED_PLAN_BASE_REGION_BLOCKS,
    STREAMED_PLAN_PHASE_ONE_SEEDS, StreamedFactId, StreamedPlanControl, StreamedPlanDescriptor,
    StreamedPlanKey, StreamedPlanRequest, StreamedPlanSnapshot, StreamedPlanTopology,
    StreamedSemanticFact, canonical_targets, comparison_corpus_sha256,
};
use crate::streamed_plan_trials::run_candidate_invariance_suite;

pub const MULTISCALE_WITNESS_SCHEMA_REVISION: &str =
    "mclone-multiscale-semantic-refinement-witness-v1";
pub const MULTISCALE_WITNESS_CANDIDATE_REVISION: &str = "multiscale-semantic-refinement-witness-v1";
pub const MULTISCALE_WITNESS_PARENT_BLOCKS: i32 = 6_144;
pub const MULTISCALE_WITNESS_REGIONAL_BLOCKS: i32 = 3_072;
pub const MULTISCALE_WITNESS_LOCAL_BLOCKS: i32 = 1_024;
pub const MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS: i32 = 1_792;
pub const MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS: u32 = 4;
pub const MULTISCALE_WITNESS_MAXIMUM_FACTS_PER_PLAN: u32 = 57;
pub const MULTISCALE_WITNESS_SHA256: &str =
    "4ac9b52c6e04697378c6ff0dfc1b9bb25a3903f9138cf4f19e633bd54db6a78d";

pub(crate) const WITNESS_RANGE_AXIS_KIND: u16 = 500;
pub(crate) const WITNESS_BASIN_ROUTE_KIND: u16 = 501;
pub(crate) const WITNESS_QUERY_KIND: u16 = 502;

const PARENT_LEVEL: u8 = 2;
const REGIONAL_LEVEL: u8 = 1;
const LOCAL_LEVEL: u8 = 0;
const FAMILY_COUNT: usize = 2;
const PARENT_SEGMENT_HALF_LENGTH: i32 = 1_200;
const PARENT_ANCHOR_MARGIN: i32 = 1_024;
const PARENT_MIDPOINT_DISPLACEMENT: i32 = 256;
const REGIONAL_MIDPOINT_DISPLACEMENT: i32 = 96;
const REGIONAL_BOUNDS_PADDING: i32 = 192;
const LOCAL_BOUNDS_PADDING: i32 = 72;

const PAYLOAD_PARENT_SLOT: usize = 0;
const PAYLOAD_START_X: usize = 1;
const PAYLOAD_START_Z: usize = 2;
const PAYLOAD_END_X: usize = 3;
const PAYLOAD_END_Z: usize = 4;
const PAYLOAD_MIN_X: usize = 5;
const PAYLOAD_MIN_Z: usize = 6;
const PAYLOAD_MAX_X: usize = 7;
const PAYLOAD_MAX_Z: usize = 8;
const PAYLOAD_WIDTH: usize = 9;
const PAYLOAD_MIN_HEIGHT: usize = 10;
const PAYLOAD_MAX_HEIGHT: usize = 11;
const PAYLOAD_TERMINAL_KIND: usize = 12;
const FEATURE_PAYLOAD_LENGTH: usize = 13;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MultiscaleWitnessDetail {
    Parent,
    Regional,
    Local,
}

impl MultiscaleWitnessDetail {
    pub const ALL: [Self; 3] = [Self::Parent, Self::Regional, Self::Local];

    pub const fn level(self) -> u8 {
        match self {
            Self::Parent => PARENT_LEVEL,
            Self::Regional => REGIONAL_LEVEL,
            Self::Local => LOCAL_LEVEL,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::Regional => "regional",
            Self::Local => "local",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MultiscaleWitnessFamily {
    RangeAxis,
    BasinRoute,
}

impl MultiscaleWitnessFamily {
    const ALL: [Self; FAMILY_COUNT] = [Self::RangeAxis, Self::BasinRoute];

    pub const fn label(self) -> &'static str {
        match self {
            Self::RangeAxis => "range-axis",
            Self::BasinRoute => "basin-route",
        }
    }

    const fn kind(self) -> u16 {
        match self {
            Self::RangeAxis => WITNESS_RANGE_AXIS_KIND,
            Self::BasinRoute => WITNESS_BASIN_ROUTE_KIND,
        }
    }

    fn from_kind(kind: u16) -> Option<Self> {
        match kind {
            WITNESS_RANGE_AXIS_KIND => Some(Self::RangeAxis),
            WITNESS_BASIN_ROUTE_KIND => Some(Self::BasinRoute),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MultiscaleWitnessBuild {
    pub snapshot: StreamedPlanSnapshot,
    pub detail: MultiscaleWitnessDetail,
    pub parent_owners_examined: u32,
    pub parent_fact_count: u32,
    pub regional_fact_count: u32,
    pub local_fact_count: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleWitnessCorpusReceipt {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub target_count: u32,
    pub direct_parent_projection_mismatch_count: u32,
    pub direct_regional_projection_mismatch_count: u32,
    pub unresolved_parent_count: u32,
    pub containment_failure_count: u32,
    pub continuity_failure_count: u32,
    pub terminal_failure_count: u32,
    pub unexpected_detail_count: u32,
    pub maximum_parent_owners_examined: u32,
    pub maximum_facts_per_plan: u32,
    pub corpus_sha256: String,
    pub first_failure: Option<String>,
    pub passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleWitnessReceipt {
    pub receipt_schema: &'static str,
    pub candidate_revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub terrain_reconstruction_measured: bool,
    pub hierarchy_blocks: [i32; 3],
    pub feature_families: [&'static str; FAMILY_COUNT],
    pub dependency_claim: HarnessDependencyClaim,
    pub exact_comparison_count: u32,
    pub comparisons: Vec<HarnessComparisonReceipt>,
    pub comparison_corpus_sha256: String,
    pub corpora: Vec<MultiscaleWitnessCorpusReceipt>,
    pub witness_sha256: String,
    pub suite_passed: bool,
}

pub fn run_multiscale_witness_suite() -> Result<MultiscaleWitnessReceipt, String> {
    let comparisons = run_candidate_invariance_suite::<MultiscaleSemanticRefinementControl>()?;
    let mut corpora = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            corpora.push(inspect_multiscale_corpus(seed, topology)?);
        }
    }
    let comparison_corpus_sha256 = comparison_corpus_sha256(&comparisons);
    let witness_sha256 = complete_witness_sha256(&comparison_corpus_sha256, &corpora);
    let suite_passed = comparisons.iter().all(|comparison| comparison.passed)
        && corpora.iter().all(|corpus| corpus.passed);
    Ok(MultiscaleWitnessReceipt {
        receipt_schema: MULTISCALE_WITNESS_SCHEMA_REVISION,
        candidate_revision: MULTISCALE_WITNESS_CANDIDATE_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        terrain_reconstruction_measured: false,
        hierarchy_blocks: [
            MULTISCALE_WITNESS_LOCAL_BLOCKS,
            MULTISCALE_WITNESS_REGIONAL_BLOCKS,
            MULTISCALE_WITNESS_PARENT_BLOCKS,
        ],
        feature_families: [
            MultiscaleWitnessFamily::RangeAxis.label(),
            MultiscaleWitnessFamily::BasinRoute.label(),
        ],
        dependency_claim: HarnessDependencyClaim {
            control: MULTISCALE_WITNESS_CANDIDATE_REVISION,
            maximum_depth: 3,
            maximum_provider_fanout: MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS,
            maximum_owner_radius_blocks: MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS as u32,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "fixed parent owners; two children per feature at each refinement level",
        },
        exact_comparison_count: comparisons.len() as u32,
        comparisons,
        comparison_corpus_sha256,
        corpora,
        witness_sha256,
        suite_passed,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct MultiscaleSemanticRefinementControl {
    detail: MultiscaleWitnessDetail,
}

impl MultiscaleSemanticRefinementControl {
    pub const fn at_detail(detail: MultiscaleWitnessDetail) -> Self {
        Self { detail }
    }

    pub(crate) fn construct_with_receipt(
        &self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<MultiscaleWitnessBuild, String> {
        let key = descriptor.plan_key(self.candidate_revision(), request.requested_region)?;
        let target = key.region;
        let owners = possible_parent_owners(descriptor, target)?;
        let parent_owners_examined = owners.len() as u32;
        if parent_owners_examined > MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS {
            return Err(format!(
                "multiscale witness examined {parent_owners_examined} parent owners, declared \
                 maximum is {MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS}"
            ));
        }
        let mut facts = Vec::new();
        let mut parent_fact_count = 0_u32;
        let mut regional_fact_count = 0_u32;
        let mut local_fact_count = 0_u32;
        for owner in owners {
            for family in MultiscaleWitnessFamily::ALL {
                let hierarchy = build_feature_hierarchy(descriptor, owner, family)?;
                if !fact_overlaps_target(descriptor, target, &hierarchy[0])? {
                    continue;
                }
                for fact in hierarchy {
                    if fact.id.owner.level < self.detail.level() {
                        continue;
                    }
                    match fact.id.owner.level {
                        PARENT_LEVEL => parent_fact_count = parent_fact_count.saturating_add(1),
                        REGIONAL_LEVEL => {
                            regional_fact_count = regional_fact_count.saturating_add(1)
                        }
                        LOCAL_LEVEL => local_fact_count = local_fact_count.saturating_add(1),
                        level => {
                            return Err(format!("unexpected multiscale feature level {level}"));
                        }
                    }
                    facts.push(fact);
                }
            }
        }
        facts.push(StreamedSemanticFact {
            id: StreamedFactId {
                owner: key.clone(),
                kind: WITNESS_QUERY_KIND,
                local_index: 0,
            },
            world_x: target.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            world_z: target.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            payload: vec![
                i64::from(parent_owners_examined),
                i64::from(parent_fact_count),
                i64::from(regional_fact_count),
                i64::from(local_fact_count),
                i64::from(self.detail.level()),
            ],
        });
        if facts.len() as u32 > MULTISCALE_WITNESS_MAXIMUM_FACTS_PER_PLAN {
            return Err(format!(
                "multiscale witness produced {} facts, declared maximum is {}",
                facts.len(),
                MULTISCALE_WITNESS_MAXIMUM_FACTS_PER_PLAN
            ));
        }
        Ok(MultiscaleWitnessBuild {
            snapshot: StreamedPlanSnapshot::new(key, facts)?,
            detail: self.detail,
            parent_owners_examined,
            parent_fact_count,
            regional_fact_count,
            local_fact_count,
        })
    }
}

impl Default for MultiscaleSemanticRefinementControl {
    fn default() -> Self {
        Self::at_detail(MultiscaleWitnessDetail::Local)
    }
}

impl StreamedPlanControl for MultiscaleSemanticRefinementControl {
    fn candidate_revision(&self) -> &'static str {
        MULTISCALE_WITNESS_CANDIDATE_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        Ok(self.construct_with_receipt(descriptor, request)?.snapshot)
    }
}

pub(crate) fn is_multiscale_feature_fact(fact: &StreamedSemanticFact) -> bool {
    MultiscaleWitnessFamily::from_kind(fact.id.kind).is_some()
}

pub(crate) fn multiscale_feature_family(
    fact: &StreamedSemanticFact,
) -> Option<MultiscaleWitnessFamily> {
    MultiscaleWitnessFamily::from_kind(fact.id.kind)
}

pub(crate) fn multiscale_fact_identity(fact: &StreamedSemanticFact) -> String {
    format!(
        "{}:l{}:{}:{}:{}",
        multiscale_feature_family(fact)
            .map(MultiscaleWitnessFamily::label)
            .unwrap_or("non-feature"),
        fact.id.owner.level,
        fact.id.owner.region.x,
        fact.id.owner.region.z,
        fact.id.local_index
    )
}

pub(crate) fn multiscale_parent_identity(fact: &StreamedSemanticFact) -> Option<String> {
    parent_fact_id(fact).map(|parent| {
        format!(
            "{}:l{}:{}:{}:{}",
            multiscale_feature_family(fact)
                .map(MultiscaleWitnessFamily::label)
                .unwrap_or("non-feature"),
            parent.owner.level,
            parent.owner.region.x,
            parent.owner.region.z,
            parent.local_index
        )
    })
}

pub(crate) fn multiscale_parent_projection_sha256(
    snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
) -> String {
    projection_sha256(snapshots, PARENT_LEVEL)
}

pub(crate) fn multiscale_regional_projection_sha256(
    snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
) -> String {
    projection_sha256(snapshots, REGIONAL_LEVEL)
}

pub(crate) fn multiscale_feature_offsets(
    fact: &StreamedSemanticFact,
) -> Result<MultiscaleFeatureOffsets, String> {
    if !is_multiscale_feature_fact(fact) || fact.payload.len() != FEATURE_PAYLOAD_LENGTH {
        return Err(format!(
            "fact {} is not a valid multiscale feature payload",
            multiscale_fact_identity(fact)
        ));
    }
    Ok(MultiscaleFeatureOffsets {
        start_x: fact.payload[PAYLOAD_START_X] as i32,
        start_z: fact.payload[PAYLOAD_START_Z] as i32,
        end_x: fact.payload[PAYLOAD_END_X] as i32,
        end_z: fact.payload[PAYLOAD_END_Z] as i32,
        min_x: fact.payload[PAYLOAD_MIN_X] as i32,
        min_z: fact.payload[PAYLOAD_MIN_Z] as i32,
        max_x: fact.payload[PAYLOAD_MAX_X] as i32,
        max_z: fact.payload[PAYLOAD_MAX_Z] as i32,
        width: fact.payload[PAYLOAD_WIDTH] as u32,
        min_height: fact.payload[PAYLOAD_MIN_HEIGHT] as i32,
        max_height: fact.payload[PAYLOAD_MAX_HEIGHT] as i32,
        terminal_kind: fact.payload[PAYLOAD_TERMINAL_KIND] as u8,
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MultiscaleFeatureOffsets {
    pub start_x: i32,
    pub start_z: i32,
    pub end_x: i32,
    pub end_z: i32,
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
    pub width: u32,
    pub min_height: i32,
    pub max_height: i32,
    pub terminal_kind: u8,
}

fn build_feature_hierarchy(
    descriptor: &StreamedPlanDescriptor,
    root_owner: PlanRegion,
    family: MultiscaleWitnessFamily,
) -> Result<Vec<StreamedSemanticFact>, String> {
    let root_key = feature_owner_key(descriptor, root_owner, PARENT_LEVEL)?;
    let hash = typed_hash(
        descriptor.seed,
        family.kind(),
        i64::from(root_owner.x),
        i64::from(root_owner.z),
    );
    let raw_anchor_x = root_owner
        .x
        .checked_mul(MULTISCALE_WITNESS_PARENT_BLOCKS)
        .and_then(|minimum| {
            minimum.checked_add(
                PARENT_ANCHOR_MARGIN
                    + (hash % (MULTISCALE_WITNESS_PARENT_BLOCKS - PARENT_ANCHOR_MARGIN * 2) as u64)
                        as i32,
            )
        })
        .ok_or_else(|| "multiscale parent anchor X overflow".to_owned())?;
    let raw_anchor_z = root_owner
        .z
        .checked_mul(MULTISCALE_WITNESS_PARENT_BLOCKS)
        .and_then(|minimum| {
            minimum.checked_add(
                PARENT_ANCHOR_MARGIN
                    + (hash.rotate_left(31)
                        % (MULTISCALE_WITNESS_PARENT_BLOCKS - PARENT_ANCHOR_MARGIN * 2) as u64)
                        as i32,
            )
        })
        .ok_or_else(|| "multiscale parent anchor Z overflow".to_owned())?;
    let canonical_anchor = canonical_block(descriptor, raw_anchor_x, raw_anchor_z)?;
    let direction = DIRECTIONS[((hash >> 16) % DIRECTIONS.len() as u64) as usize];
    let (start, end) = match family {
        MultiscaleWitnessFamily::RangeAxis => (
            (
                -direction.0 * PARENT_SEGMENT_HALF_LENGTH,
                -direction.1 * PARENT_SEGMENT_HALF_LENGTH,
            ),
            (
                direction.0 * PARENT_SEGMENT_HALF_LENGTH,
                direction.1 * PARENT_SEGMENT_HALF_LENGTH,
            ),
        ),
        MultiscaleWitnessFamily::BasinRoute => (
            (-direction.0 * 1_000, -direction.1 * 1_000),
            (direction.0 * 1_150, direction.1 * 1_150),
        ),
    };
    let height_center = match family {
        MultiscaleWitnessFamily::RangeAxis => 128 + ((hash >> 40) % 56) as i32,
        MultiscaleWitnessFamily::BasinRoute => 58 + ((hash >> 40) % 18) as i32,
    };
    let root_terminal = (family == MultiscaleWitnessFamily::BasinRoute) as u8;
    let mut facts = Vec::with_capacity(7);
    facts.push(feature_fact(
        root_key,
        family,
        0,
        -1,
        canonical_anchor,
        start,
        end,
        (
            -MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS,
            -MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS,
            MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS,
            MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS,
        ),
        256,
        height_center - 32,
        height_center + 32,
        root_terminal,
    ));

    let root_midpoint = displaced_midpoint(
        start,
        end,
        signed_bucket(hash.rotate_left(11), PARENT_MIDPOINT_DISPLACEMENT),
    );
    let regional_segments = [(start, root_midpoint), (root_midpoint, end)];
    for (regional_slot, (regional_start, regional_end)) in regional_segments.into_iter().enumerate()
    {
        let regional_key = feature_owner_key(descriptor, root_owner, REGIONAL_LEVEL)?;
        let regional_bounds = padded_bounds(regional_start, regional_end, REGIONAL_BOUNDS_PADDING);
        facts.push(feature_fact(
            regional_key,
            family,
            regional_slot as u32,
            0,
            canonical_anchor,
            regional_start,
            regional_end,
            regional_bounds,
            128,
            height_center - 24,
            height_center + 24,
            if root_terminal != 0 && regional_slot == 1 {
                root_terminal
            } else {
                0
            },
        ));

        let local_midpoint = displaced_midpoint(
            regional_start,
            regional_end,
            signed_bucket(
                hash.rotate_left(23 + regional_slot as u32 * 9),
                REGIONAL_MIDPOINT_DISPLACEMENT,
            ),
        );
        let local_segments = [
            (regional_start, local_midpoint),
            (local_midpoint, regional_end),
        ];
        for (local_in_parent, (local_start, local_end)) in local_segments.into_iter().enumerate() {
            let local_slot = regional_slot * 2 + local_in_parent;
            let local_key = feature_owner_key(descriptor, root_owner, LOCAL_LEVEL)?;
            facts.push(feature_fact(
                local_key,
                family,
                local_slot as u32,
                regional_slot as i64,
                canonical_anchor,
                local_start,
                local_end,
                padded_bounds(local_start, local_end, LOCAL_BOUNDS_PADDING),
                64,
                height_center - 18,
                height_center + 18,
                if root_terminal != 0 && local_slot == 3 {
                    root_terminal
                } else {
                    0
                },
            ));
        }
    }
    Ok(facts)
}

#[allow(clippy::too_many_arguments)]
fn feature_fact(
    owner: StreamedPlanKey,
    family: MultiscaleWitnessFamily,
    local_index: u32,
    parent_slot: i64,
    canonical_anchor: BlockPos,
    start: (i32, i32),
    end: (i32, i32),
    bounds: (i32, i32, i32, i32),
    width: u32,
    min_height: i32,
    max_height: i32,
    terminal_kind: u8,
) -> StreamedSemanticFact {
    StreamedSemanticFact {
        id: StreamedFactId {
            owner,
            kind: family.kind(),
            local_index,
        },
        world_x: canonical_anchor.x,
        world_z: canonical_anchor.z,
        payload: vec![
            parent_slot,
            i64::from(start.0),
            i64::from(start.1),
            i64::from(end.0),
            i64::from(end.1),
            i64::from(bounds.0),
            i64::from(bounds.1),
            i64::from(bounds.2),
            i64::from(bounds.3),
            i64::from(width),
            i64::from(min_height),
            i64::from(max_height),
            i64::from(terminal_kind),
        ],
    }
}

fn feature_owner_key(
    descriptor: &StreamedPlanDescriptor,
    root_owner: PlanRegion,
    level: u8,
) -> Result<StreamedPlanKey, String> {
    let scale = MULTISCALE_WITNESS_PARENT_BLOCKS / STREAMED_PLAN_BASE_REGION_BLOCKS;
    let base_region = PlanRegion::new(
        root_owner
            .x
            .checked_mul(scale)
            .ok_or_else(|| "multiscale root owner X overflow".to_owned())?,
        root_owner
            .z
            .checked_mul(scale)
            .ok_or_else(|| "multiscale root owner Z overflow".to_owned())?,
    );
    let mut key = descriptor.plan_key(MULTISCALE_WITNESS_CANDIDATE_REVISION, base_region)?;
    key.level = level;
    key.region = root_owner;
    Ok(key)
}

fn possible_parent_owners(
    descriptor: &StreamedPlanDescriptor,
    target: PlanRegion,
) -> Result<Vec<PlanRegion>, String> {
    let minimum_x = target
        .min_block_x()
        .checked_sub(MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS)
        .ok_or_else(|| "multiscale target minimum X overflow".to_owned())?;
    let minimum_z = target
        .min_block_z()
        .checked_sub(MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS)
        .ok_or_else(|| "multiscale target minimum Z overflow".to_owned())?;
    let maximum_x = target
        .min_block_x()
        .checked_add(STREAMED_PLAN_BASE_REGION_BLOCKS - 1)
        .and_then(|maximum| maximum.checked_add(MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS))
        .ok_or_else(|| "multiscale target maximum X overflow".to_owned())?;
    let maximum_z = target
        .min_block_z()
        .checked_add(STREAMED_PLAN_BASE_REGION_BLOCKS - 1)
        .and_then(|maximum| maximum.checked_add(MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS))
        .ok_or_else(|| "multiscale target maximum Z overflow".to_owned())?;
    let mut owners = BTreeSet::new();
    for z in minimum_z.div_euclid(MULTISCALE_WITNESS_PARENT_BLOCKS)
        ..=maximum_z.div_euclid(MULTISCALE_WITNESS_PARENT_BLOCKS)
    {
        for x in minimum_x.div_euclid(MULTISCALE_WITNESS_PARENT_BLOCKS)
            ..=maximum_x.div_euclid(MULTISCALE_WITNESS_PARENT_BLOCKS)
        {
            owners.insert(canonical_parent_owner(descriptor, PlanRegion::new(x, z))?);
        }
    }
    Ok(owners.into_iter().collect())
}

fn canonical_parent_owner(
    descriptor: &StreamedPlanDescriptor,
    owner: PlanRegion,
) -> Result<PlanRegion, String> {
    let scale = MULTISCALE_WITNESS_PARENT_BLOCKS / STREAMED_PLAN_BASE_REGION_BLOCKS;
    let canonical_base = descriptor.canonical_region(PlanRegion::new(
        owner
            .x
            .checked_mul(scale)
            .ok_or_else(|| "canonical parent owner X overflow".to_owned())?,
        owner
            .z
            .checked_mul(scale)
            .ok_or_else(|| "canonical parent owner Z overflow".to_owned())?,
    ))?;
    if canonical_base.x.rem_euclid(scale) != 0 || canonical_base.z.rem_euclid(scale) != 0 {
        return Err(format!(
            "canonical parent owner lost {}-block alignment: {:?}",
            MULTISCALE_WITNESS_PARENT_BLOCKS, canonical_base
        ));
    }
    Ok(PlanRegion::new(
        canonical_base.x.div_euclid(scale),
        canonical_base.z.div_euclid(scale),
    ))
}

fn fact_overlaps_target(
    descriptor: &StreamedPlanDescriptor,
    target: PlanRegion,
    fact: &StreamedSemanticFact,
) -> Result<bool, String> {
    let offsets = multiscale_feature_offsets(fact)?;
    let observer_x = target.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2;
    let observer_z = target.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2;
    let topology = descriptor.topology.horizontal();
    let anchor_x = i64::from(observer_x)
        + topology
            .x
            .shortest_block_displacement(observer_x, fact.world_x);
    let anchor_z = i64::from(observer_z)
        + topology
            .z
            .shortest_block_displacement(observer_z, fact.world_z);
    let target_min_x = i64::from(target.min_block_x());
    let target_min_z = i64::from(target.min_block_z());
    let target_max_x = target_min_x + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let target_max_z = target_min_z + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
    Ok(anchor_x + i64::from(offsets.max_x) >= target_min_x
        && anchor_x + i64::from(offsets.min_x) < target_max_x
        && anchor_z + i64::from(offsets.max_z) >= target_min_z
        && anchor_z + i64::from(offsets.min_z) < target_max_z)
}

fn inspect_multiscale_corpus(
    seed: i64,
    topology: StreamedPlanTopology,
) -> Result<MultiscaleWitnessCorpusReceipt, String> {
    let descriptor = StreamedPlanDescriptor::new(seed, topology);
    let targets = canonical_targets(topology);
    let mut parent_projection_mismatches = 0_u32;
    let mut regional_projection_mismatches = 0_u32;
    let mut unresolved_parent_count = 0_u32;
    let mut containment_failure_count = 0_u32;
    let mut continuity_failure_count = 0_u32;
    let mut terminal_failure_count = 0_u32;
    let mut unexpected_detail_count = 0_u32;
    let mut maximum_parent_owners_examined = 0_u32;
    let mut maximum_facts_per_plan = 0_u32;
    let mut first_failure = None;
    let mut digest = Sha256::new();
    digest.update(MULTISCALE_WITNESS_SCHEMA_REVISION.as_bytes());
    digest.update(seed.to_le_bytes());
    digest.update([topology_tag(topology)]);
    for target in &targets {
        let request = StreamedPlanRequest::new(*target, PlanRegion::new(0, 0));
        let parent =
            MultiscaleSemanticRefinementControl::at_detail(MultiscaleWitnessDetail::Parent)
                .construct_with_receipt(&descriptor, request)?;
        let regional =
            MultiscaleSemanticRefinementControl::at_detail(MultiscaleWitnessDetail::Regional)
                .construct_with_receipt(&descriptor, request)?;
        let local = MultiscaleSemanticRefinementControl::at_detail(MultiscaleWitnessDetail::Local)
            .construct_with_receipt(&descriptor, request)?;
        maximum_parent_owners_examined = maximum_parent_owners_examined
            .max(parent.parent_owners_examined)
            .max(regional.parent_owners_examined)
            .max(local.parent_owners_examined);
        maximum_facts_per_plan = maximum_facts_per_plan
            .max(parent.snapshot.facts.len() as u32)
            .max(regional.snapshot.facts.len() as u32)
            .max(local.snapshot.facts.len() as u32);

        let parent_facts = projected_feature_facts(&parent.snapshot, PARENT_LEVEL);
        let regional_parent_facts = projected_feature_facts(&regional.snapshot, PARENT_LEVEL);
        let local_parent_facts = projected_feature_facts(&local.snapshot, PARENT_LEVEL);
        if parent_facts != regional_parent_facts || parent_facts != local_parent_facts {
            parent_projection_mismatches = parent_projection_mismatches.saturating_add(1);
            set_first_failure(
                &mut first_failure,
                format!("target {target:?} changed its direct parent projection"),
            );
        }
        let regional_facts = projected_feature_facts(&regional.snapshot, REGIONAL_LEVEL);
        let local_regional_facts = projected_feature_facts(&local.snapshot, REGIONAL_LEVEL);
        if regional_facts != local_regional_facts {
            regional_projection_mismatches = regional_projection_mismatches.saturating_add(1);
            set_first_failure(
                &mut first_failure,
                format!("target {target:?} changed its direct regional projection"),
            );
        }
        if parent.regional_fact_count != 0
            || parent.local_fact_count != 0
            || regional.local_fact_count != 0
        {
            unexpected_detail_count = unexpected_detail_count.saturating_add(1);
            set_first_failure(
                &mut first_failure,
                format!("target {target:?} constructed detail below the requested level"),
            );
        }
        let validation = validate_local_snapshot(&local.snapshot)?;
        unresolved_parent_count =
            unresolved_parent_count.saturating_add(validation.unresolved_parent_count);
        containment_failure_count =
            containment_failure_count.saturating_add(validation.containment_failure_count);
        continuity_failure_count =
            continuity_failure_count.saturating_add(validation.continuity_failure_count);
        terminal_failure_count =
            terminal_failure_count.saturating_add(validation.terminal_failure_count);
        if let Some(failure) = validation.first_failure {
            set_first_failure(&mut first_failure, format!("target {target:?}: {failure}"));
        }
        digest.update(target.x.to_le_bytes());
        digest.update(target.z.to_le_bytes());
        digest.update(parent.snapshot.semantic_sha256().as_bytes());
        digest.update(regional.snapshot.semantic_sha256().as_bytes());
        digest.update(local.snapshot.semantic_sha256().as_bytes());
    }
    let corpus_sha256 = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let passed = parent_projection_mismatches == 0
        && regional_projection_mismatches == 0
        && unresolved_parent_count == 0
        && containment_failure_count == 0
        && continuity_failure_count == 0
        && terminal_failure_count == 0
        && unexpected_detail_count == 0
        && maximum_parent_owners_examined <= MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS
        && maximum_facts_per_plan <= MULTISCALE_WITNESS_MAXIMUM_FACTS_PER_PLAN;
    Ok(MultiscaleWitnessCorpusReceipt {
        seed,
        topology,
        target_count: targets.len() as u32,
        direct_parent_projection_mismatch_count: parent_projection_mismatches,
        direct_regional_projection_mismatch_count: regional_projection_mismatches,
        unresolved_parent_count,
        containment_failure_count,
        continuity_failure_count,
        terminal_failure_count,
        unexpected_detail_count,
        maximum_parent_owners_examined,
        maximum_facts_per_plan,
        corpus_sha256,
        first_failure,
        passed,
    })
}

#[derive(Default)]
pub(crate) struct LocalValidation {
    pub unresolved_parent_count: u32,
    pub containment_failure_count: u32,
    pub continuity_failure_count: u32,
    pub terminal_failure_count: u32,
    pub first_failure: Option<String>,
}

pub(crate) fn validate_local_snapshot(
    snapshot: &StreamedPlanSnapshot,
) -> Result<LocalValidation, String> {
    let facts = snapshot
        .facts
        .iter()
        .filter(|fact| is_multiscale_feature_fact(fact))
        .map(|fact| (fact.id.clone(), fact))
        .collect::<BTreeMap<_, _>>();
    let mut validation = LocalValidation::default();
    for (id, fact) in &facts {
        let Some(parent_id) = parent_fact_id(fact) else {
            continue;
        };
        let Some(parent) = facts.get(&parent_id) else {
            validation.unresolved_parent_count =
                validation.unresolved_parent_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!(
                    "{} has no parent {parent_id:?}",
                    multiscale_fact_identity(fact)
                ),
            );
            continue;
        };
        let child_offsets = multiscale_feature_offsets(fact)?;
        let parent_offsets = multiscale_feature_offsets(parent)?;
        if child_offsets.min_x < parent_offsets.min_x
            || child_offsets.min_z < parent_offsets.min_z
            || child_offsets.max_x > parent_offsets.max_x
            || child_offsets.max_z > parent_offsets.max_z
        {
            validation.containment_failure_count =
                validation.containment_failure_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!("{} exceeds parent bounds", multiscale_fact_identity(fact)),
            );
        }
        if fact.payload[PAYLOAD_PARENT_SLOT] as u32 != parent_id.local_index {
            validation.unresolved_parent_count =
                validation.unresolved_parent_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!(
                    "{} records the wrong parent slot",
                    multiscale_fact_identity(fact)
                ),
            );
        }
        let _ = id;
    }

    for (id, parent) in facts.iter().filter(|(id, _)| id.owner.level > LOCAL_LEVEL) {
        let mut left_id = id.clone();
        left_id.owner.level -= 1;
        left_id.local_index = id.local_index * 2;
        let mut right_id = left_id.clone();
        right_id.local_index += 1;
        let (Some(left), Some(right)) = (facts.get(&left_id), facts.get(&right_id)) else {
            validation.continuity_failure_count =
                validation.continuity_failure_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!(
                    "{} does not have two child segments",
                    multiscale_fact_identity(parent)
                ),
            );
            continue;
        };
        let parent_offsets = multiscale_feature_offsets(parent)?;
        let left_offsets = multiscale_feature_offsets(left)?;
        let right_offsets = multiscale_feature_offsets(right)?;
        if (parent_offsets.start_x, parent_offsets.start_z)
            != (left_offsets.start_x, left_offsets.start_z)
            || (left_offsets.end_x, left_offsets.end_z)
                != (right_offsets.start_x, right_offsets.start_z)
            || (right_offsets.end_x, right_offsets.end_z)
                != (parent_offsets.end_x, parent_offsets.end_z)
        {
            validation.continuity_failure_count =
                validation.continuity_failure_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!(
                    "{} child endpoints are discontinuous",
                    multiscale_fact_identity(parent)
                ),
            );
        }
        let family = multiscale_feature_family(parent).expect("feature family");
        let expected_parent_terminal = family == MultiscaleWitnessFamily::BasinRoute
            && (id.owner.level == PARENT_LEVEL || id.local_index == 1);
        if (parent_offsets.terminal_kind != 0) != expected_parent_terminal
            || left_offsets.terminal_kind != 0
            || (right_offsets.terminal_kind != 0) != expected_parent_terminal
        {
            validation.terminal_failure_count = validation.terminal_failure_count.saturating_add(1);
            set_first_failure(
                &mut validation.first_failure,
                format!(
                    "{} has inconsistent explicit sink propagation",
                    multiscale_fact_identity(parent)
                ),
            );
        }
    }
    Ok(validation)
}

fn parent_fact_id(fact: &StreamedSemanticFact) -> Option<StreamedFactId> {
    if !is_multiscale_feature_fact(fact) || fact.id.owner.level == PARENT_LEVEL {
        return None;
    }
    let mut parent = fact.id.clone();
    parent.owner.level += 1;
    parent.local_index = fact.payload[PAYLOAD_PARENT_SLOT] as u32;
    Some(parent)
}

fn projected_feature_facts(
    snapshot: &StreamedPlanSnapshot,
    minimum_level: u8,
) -> Vec<StreamedSemanticFact> {
    snapshot
        .facts
        .iter()
        .filter(|fact| is_multiscale_feature_fact(fact) && fact.id.owner.level >= minimum_level)
        .cloned()
        .collect()
}

fn projection_sha256(
    snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
    minimum_level: u8,
) -> String {
    let mut digest = Sha256::new();
    digest.update(MULTISCALE_WITNESS_SCHEMA_REVISION.as_bytes());
    digest.update([minimum_level]);
    for snapshot in snapshots.values() {
        digest.update(snapshot.key.region.x.to_le_bytes());
        digest.update(snapshot.key.region.z.to_le_bytes());
        for fact in projected_feature_facts(snapshot, minimum_level) {
            digest.update(fact.semantic_component_bytes());
        }
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

trait SemanticComponentBytes {
    fn semantic_component_bytes(&self) -> Vec<u8>;
}

impl SemanticComponentBytes for StreamedSemanticFact {
    fn semantic_component_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.id.owner.candidate_revision.as_bytes());
        bytes.push(0);
        bytes.push(self.id.owner.level);
        bytes.extend_from_slice(&self.id.owner.region.x.to_le_bytes());
        bytes.extend_from_slice(&self.id.owner.region.z.to_le_bytes());
        bytes.extend_from_slice(&self.id.kind.to_le_bytes());
        bytes.extend_from_slice(&self.id.local_index.to_le_bytes());
        bytes.extend_from_slice(&self.world_x.to_le_bytes());
        bytes.extend_from_slice(&self.world_z.to_le_bytes());
        bytes.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        for value in &self.payload {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}

fn complete_witness_sha256(
    comparison_corpus_sha256: &str,
    corpora: &[MultiscaleWitnessCorpusReceipt],
) -> String {
    let mut digest = Sha256::new();
    digest.update(MULTISCALE_WITNESS_SCHEMA_REVISION.as_bytes());
    digest.update(comparison_corpus_sha256.as_bytes());
    for corpus in corpora {
        digest.update(corpus.seed.to_le_bytes());
        digest.update([topology_tag(corpus.topology)]);
        digest.update(corpus.corpus_sha256.as_bytes());
        digest.update([corpus.passed as u8]);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn displaced_midpoint(start: (i32, i32), end: (i32, i32), displacement: i32) -> (i32, i32) {
    let midpoint = ((start.0 + end.0) / 2, (start.1 + end.1) / 2);
    let direction = ((end.0 - start.0).signum(), (end.1 - start.1).signum());
    (
        midpoint.0 - direction.1 * displacement,
        midpoint.1 + direction.0 * displacement,
    )
}

fn padded_bounds(start: (i32, i32), end: (i32, i32), padding: i32) -> (i32, i32, i32, i32) {
    (
        start.0.min(end.0) - padding,
        start.1.min(end.1) - padding,
        start.0.max(end.0) + padding,
        start.1.max(end.1) + padding,
    )
}

fn canonical_block(
    descriptor: &StreamedPlanDescriptor,
    world_x: i32,
    world_z: i32,
) -> Result<BlockPos, String> {
    descriptor
        .topology
        .horizontal()
        .canonicalize_block(BlockPos::new(world_x, 0, world_z))
        .ok_or_else(|| format!("block ({world_x}, {world_z}) lies outside topology"))
}

fn typed_hash(seed: i64, domain: u16, x: i64, z: i64) -> u64 {
    stable_mix64(
        seed as u64
            ^ u64::from(domain).rotate_left(7)
            ^ (x as u64).rotate_left(23)
            ^ (z as u64).rotate_left(47),
    )
}

fn stable_mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn signed_bucket(hash: u64, magnitude: i32) -> i32 {
    let span = i64::from(magnitude) * 2 + 1;
    (hash % span as u64) as i32 - magnitude
}

fn topology_tag(topology: StreamedPlanTopology) -> u8 {
    match topology {
        StreamedPlanTopology::Plane => 0,
        StreamedPlanTopology::CylinderX => 1,
        StreamedPlanTopology::Torus => 2,
    }
}

fn set_first_failure(first_failure: &mut Option<String>, failure: String) {
    if first_failure.is_none() {
        *first_failure = Some(failure);
    }
}

const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::thread;

    use super::*;
    use crate::streamed_plan_harness::{
        HarnessRunSpec, execute_harness_case, requests_with_center,
    };

    #[test]
    fn direct_queries_preserve_parent_and_regional_projections() {
        let receipt = run_multiscale_witness_suite().expect("multiscale witness suite");
        assert!(receipt.suite_passed);
        assert_eq!(receipt.witness_sha256, MULTISCALE_WITNESS_SHA256);
        assert!(
            receipt
                .corpora
                .iter()
                .all(|corpus| corpus.direct_parent_projection_mismatch_count == 0)
        );
        assert!(
            receipt
                .corpora
                .iter()
                .all(|corpus| corpus.direct_regional_projection_mismatch_count == 0)
        );
    }

    #[test]
    fn child_relationships_are_complete_contained_and_continuous() {
        for topology in StreamedPlanTopology::ALL {
            let descriptor = StreamedPlanDescriptor::new(-98_765, topology);
            let request = StreamedPlanRequest::new(PlanRegion::new(0, 0), PlanRegion::new(0, 0));
            let build = MultiscaleSemanticRefinementControl::default()
                .construct_with_receipt(&descriptor, request)
                .unwrap();
            let validation = validate_local_snapshot(&build.snapshot).unwrap();
            assert_eq!(
                validation.unresolved_parent_count, 0,
                "{:?}",
                validation.first_failure
            );
            assert_eq!(
                validation.containment_failure_count, 0,
                "{:?}",
                validation.first_failure
            );
            assert_eq!(
                validation.continuity_failure_count, 0,
                "{:?}",
                validation.first_failure
            );
            assert_eq!(
                validation.terminal_failure_count, 0,
                "{:?}",
                validation.first_failure
            );
        }
    }

    #[test]
    fn coarse_queries_do_not_construct_hidden_children() {
        let descriptor = StreamedPlanDescriptor::new(12_345, StreamedPlanTopology::Plane);
        let request = StreamedPlanRequest::new(PlanRegion::new(0, 0), PlanRegion::new(37, -19));
        let parent =
            MultiscaleSemanticRefinementControl::at_detail(MultiscaleWitnessDetail::Parent)
                .construct_with_receipt(&descriptor, request)
                .unwrap();
        let regional =
            MultiscaleSemanticRefinementControl::at_detail(MultiscaleWitnessDetail::Regional)
                .construct_with_receipt(&descriptor, request)
                .unwrap();
        assert!(parent.parent_fact_count > 0);
        assert_eq!(parent.regional_fact_count, 0);
        assert_eq!(parent.local_fact_count, 0);
        assert!(regional.regional_fact_count > 0);
        assert_eq!(regional.local_fact_count, 0);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn serial_and_parallel_construction_publish_one_exact_corpus() {
        let descriptor = StreamedPlanDescriptor::new(8_675_309, StreamedPlanTopology::CylinderX);
        let targets = canonical_targets(StreamedPlanTopology::CylinderX);
        let serial = execute_harness_case(
            &descriptor,
            MultiscaleSemanticRefinementControl::default(),
            HarnessRunSpec::in_request_order(
                "serial",
                requests_with_center(&targets, PlanRegion::new(0, 0)),
            ),
        )
        .unwrap();
        let parallel = thread::scope(|scope| {
            let mut handles = Vec::new();
            for target in &targets {
                let descriptor = descriptor.clone();
                let target = *target;
                handles.push(scope.spawn(move || {
                    let request = StreamedPlanRequest::new(target, PlanRegion::new(0, 0));
                    MultiscaleSemanticRefinementControl::default()
                        .construct_with_receipt(&descriptor, request)
                        .map(|build| build.snapshot)
                }));
            }
            let mut snapshots = BTreeMap::new();
            for handle in handles {
                let snapshot = handle.join().expect("parallel witness thread").unwrap();
                snapshots.insert(snapshot.key.clone(), snapshot);
            }
            snapshots
        });
        assert_eq!(serial.snapshots, parallel);
    }

    #[test]
    fn owner_and_fact_bounds_are_declared() {
        let receipt = run_multiscale_witness_suite().expect("multiscale witness suite");
        assert!(receipt.corpora.iter().all(|corpus| {
            corpus.maximum_parent_owners_examined <= MULTISCALE_WITNESS_MAXIMUM_PARENT_OWNERS
                && corpus.maximum_facts_per_plan <= MULTISCALE_WITNESS_MAXIMUM_FACTS_PER_PLAN
        }));
    }
}
