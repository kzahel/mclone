//! Candidate-neutral streamed-plan invariance research harness.
//!
//! This module is intentionally separate from production terrain generation.
//! It supplies canonical identities, exact semantic records, request/cache/
//! schedule permutations, and Phase 1 controls for Tactical 270.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{AxisTopology, BlockPos, CHUNK_WIDTH, ChunkPos, HorizontalTopology};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::landform_plan::{
    MCLONE_LANDFORM_PLAN_CELL_BLOCKS, MCLONE_LANDFORM_PLAN_CELL_CROP_EDGE,
    MCLONE_LANDFORM_PLAN_STUDY_BLOCKS, McloneLandformPlanSummary,
};
use crate::levelgen::TopologyProbeSource;

pub const STREAMED_PLAN_HARNESS_SCHEMA_REVISION: &str = "mclone-streamed-plan-harness-v1";
pub const STREAMED_PLAN_RECEIPT_SCHEMA_REVISION: &str = "mclone-streamed-plan-phase1-receipt-v1";
pub const STREAMED_PLAN_STORED_PROFILE_REVISION: &str = "research-only-no-stored-profile-v1";
pub const STREAMED_PLAN_DIMENSION_ID: &str = "mclone:tactical-270-phase1";
pub const STREAMED_PLAN_FAMILY: &str = "macro-landscape-research";
pub const STREAMED_PLAN_BASE_REGION_BLOCKS: i32 = 1_024;
pub const STREAMED_PLAN_PERIOD_BLOCKS: i32 = 6_144;
pub const STREAMED_PLAN_PERIOD_REGIONS: i32 =
    STREAMED_PLAN_PERIOD_BLOCKS / STREAMED_PLAN_BASE_REGION_BLOCKS;
pub const STREAMED_PLAN_PERIOD_CHUNKS: u32 = (STREAMED_PLAN_PERIOD_BLOCKS / CHUNK_WIDTH) as u32;
pub const STREAMED_PLAN_PHASE_ONE_SEEDS: [i64; 3] = [12_345, 8_675_309, -98_765];
pub const STREAMED_PLAN_PHASE_ONE_FALLBACK_SHA256: &str =
    "e5e329515e9638044db802f67433ba40b8b1f4aaf179e11e45c04fc2e03a1fde";
pub const STREAMED_PLAN_PHASE_ONE_COMPARISON_SHA256: &str =
    "2f7bd2f9fb3e54fe9cf66900e32ca985a5de7fbba1a6d0ca379678b9c5eafc12";

const FALLBACK_CONTROL_REVISION: &str = "coordinate-pure-bounded-start-control-v1";
const FIXED_WINDOW_CONTROL_REVISION: &str = "fixed-bounded-landform-control-v1";
const RECENTER_CONTROL_REVISION: &str = "recentered-landform-negative-control-v1";
const DISCOVERY_CONTROL_REVISION: &str = "discovery-state-negative-control-v1";
const FALLBACK_SAMPLE_KIND: u16 = 1;
const FALLBACK_FEATURE_KIND: u16 = 2;
const FALLBACK_REGION_KIND: u16 = 3;
const LANDFORM_CELL_KIND: u16 = 100;
const DISCOVERY_CELL_KIND: u16 = 200;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamedPlanTopology {
    Plane,
    CylinderX,
    Torus,
}

impl StreamedPlanTopology {
    pub const ALL: [Self; 3] = [Self::Plane, Self::CylinderX, Self::Torus];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::CylinderX => "cylinder-x-6144",
            Self::Torus => "torus-6144x6144",
        }
    }

    pub const fn horizontal(self) -> HorizontalTopology {
        match self {
            Self::Plane => HorizontalTopology::UNBOUNDED,
            Self::CylinderX => HorizontalTopology::cylinder_x(0, STREAMED_PLAN_PERIOD_CHUNKS),
            Self::Torus => HorizontalTopology::new(
                AxisTopology::periodic(0, STREAMED_PLAN_PERIOD_CHUNKS),
                AxisTopology::periodic(0, STREAMED_PLAN_PERIOD_CHUNKS),
            ),
        }
    }

    pub fn descriptor_sha256(self) -> String {
        let descriptor = match self {
            Self::Plane => "horizontal-topology-v1|x=unbounded|z=unbounded",
            Self::CylinderX => "horizontal-topology-v1|x=periodic:0:384|z=unbounded",
            Self::Torus => "horizontal-topology-v1|x=periodic:0:384|z=periodic:0:384",
        };
        sha256_hex(descriptor.as_bytes())
    }

    pub(crate) fn lifted_region(self, region: PlanRegion) -> PlanRegion {
        match self {
            Self::Plane => region,
            Self::CylinderX => PlanRegion::new(region.x + STREAMED_PLAN_PERIOD_REGIONS, region.z),
            Self::Torus => PlanRegion::new(
                region.x + STREAMED_PLAN_PERIOD_REGIONS,
                region.z - STREAMED_PLAN_PERIOD_REGIONS,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PlanRegion {
    pub x: i32,
    pub z: i32,
}

impl PlanRegion {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub const fn min_block_x(self) -> i32 {
        self.x * STREAMED_PLAN_BASE_REGION_BLOCKS
    }

    pub const fn min_block_z(self) -> i32 {
        self.z * STREAMED_PLAN_BASE_REGION_BLOCKS
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamedPlanKey {
    pub harness_revision: &'static str,
    pub stored_profile_revision: &'static str,
    pub dimension_id: &'static str,
    pub topology_descriptor_sha256: String,
    pub candidate_revision: &'static str,
    pub plan_family: &'static str,
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub level: u8,
    pub region: PlanRegion,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamedFactId {
    pub owner: StreamedPlanKey,
    pub kind: u16,
    pub local_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StreamedSemanticFact {
    pub id: StreamedFactId,
    pub world_x: i32,
    pub world_z: i32,
    pub payload: Vec<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamedPlanSnapshot {
    pub key: StreamedPlanKey,
    pub facts: Vec<StreamedSemanticFact>,
}

impl StreamedPlanSnapshot {
    pub fn new(key: StreamedPlanKey, mut facts: Vec<StreamedSemanticFact>) -> Result<Self, String> {
        facts.sort_by(|left, right| left.id.cmp(&right.id));
        if facts.windows(2).any(|window| window[0].id == window[1].id) {
            return Err(format!("duplicate semantic FactId in plan {:?}", key));
        }
        Ok(Self { key, facts })
    }

    pub fn semantic_sha256(&self) -> String {
        sha256_hex(&self.canonical_bytes())
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_plan_key(&mut bytes, &self.key);
        write_u32(&mut bytes, self.facts.len() as u32);
        for fact in &self.facts {
            write_plan_key(&mut bytes, &fact.id.owner);
            write_u16(&mut bytes, fact.id.kind);
            write_u32(&mut bytes, fact.id.local_index);
            write_i32(&mut bytes, fact.world_x);
            write_i32(&mut bytes, fact.world_z);
            write_u32(&mut bytes, fact.payload.len() as u32);
            for value in &fact.payload {
                write_i64(&mut bytes, *value);
            }
        }
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessCachePolicy {
    Retain,
    EvictAfterBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamedPlanRequest {
    pub requested_region: PlanRegion,
    pub window_center_region: PlanRegion,
}

impl StreamedPlanRequest {
    pub const fn new(requested_region: PlanRegion, window_center_region: PlanRegion) -> Self {
        Self {
            requested_region,
            window_center_region,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StreamedPlanDescriptor {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
}

impl StreamedPlanDescriptor {
    pub const fn new(seed: i64, topology: StreamedPlanTopology) -> Self {
        Self { seed, topology }
    }

    pub fn canonical_region(&self, requested: PlanRegion) -> Result<PlanRegion, String> {
        let chunks_per_region = STREAMED_PLAN_BASE_REGION_BLOCKS / CHUNK_WIDTH;
        let requested_chunk = ChunkPos::new(
            requested.x * chunks_per_region,
            requested.z * chunks_per_region,
        );
        let canonical = self
            .topology
            .horizontal()
            .canonicalize_chunk(requested_chunk)
            .ok_or_else(|| format!("plan region {:?} lies outside topology", requested))?;
        if canonical.x.rem_euclid(chunks_per_region) != 0
            || canonical.z.rem_euclid(chunks_per_region) != 0
        {
            return Err(format!(
                "topology canonicalization broke plan-region alignment: {:?}",
                canonical
            ));
        }
        Ok(PlanRegion::new(
            canonical.x.div_euclid(chunks_per_region),
            canonical.z.div_euclid(chunks_per_region),
        ))
    }

    pub fn plan_key(
        &self,
        candidate_revision: &'static str,
        requested: PlanRegion,
    ) -> Result<StreamedPlanKey, String> {
        Ok(StreamedPlanKey {
            harness_revision: STREAMED_PLAN_HARNESS_SCHEMA_REVISION,
            stored_profile_revision: STREAMED_PLAN_STORED_PROFILE_REVISION,
            dimension_id: STREAMED_PLAN_DIMENSION_ID,
            topology_descriptor_sha256: self.topology.descriptor_sha256(),
            candidate_revision,
            plan_family: STREAMED_PLAN_FAMILY,
            seed: self.seed,
            topology: self.topology,
            level: 0,
            region: self.canonical_region(requested)?,
        })
    }
}

pub trait StreamedPlanControl {
    fn candidate_revision(&self) -> &'static str;

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String>;

    fn construct_batch(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        requests: &[StreamedPlanRequest],
    ) -> Result<Vec<StreamedPlanSnapshot>, String> {
        requests
            .iter()
            .copied()
            .map(|request| self.construct(descriptor, request))
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct HarnessRunSpec {
    pub case_id: String,
    pub requests: Vec<StreamedPlanRequest>,
    pub completion_order: Vec<usize>,
    pub batch_size: usize,
    pub cache_policy: HarnessCachePolicy,
}

impl HarnessRunSpec {
    pub(crate) fn in_request_order(
        case_id: impl Into<String>,
        requests: Vec<StreamedPlanRequest>,
    ) -> Self {
        let completion_order = (0..requests.len()).collect();
        let batch_size = requests.len().max(1);
        Self {
            case_id: case_id.into(),
            requests,
            completion_order,
            batch_size,
            cache_policy: HarnessCachePolicy::Retain,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HarnessRunResult {
    pub case_id: String,
    pub request_order: Vec<PlanRegion>,
    pub completion_order: Vec<usize>,
    pub batch_size: usize,
    pub cache_policy: HarnessCachePolicy,
    pub snapshots: BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
    pub semantic_sha256: String,
    pub construction_count: u32,
    pub cache_hit_count: u32,
    pub internal_conflict_count: u32,
    pub first_internal_conflict: Option<String>,
}

pub fn execute_harness_case<C: StreamedPlanControl>(
    descriptor: &StreamedPlanDescriptor,
    mut control: C,
    spec: HarnessRunSpec,
) -> Result<HarnessRunResult, String> {
    if spec.batch_size == 0 {
        return Err("harness batch size must be nonzero".to_owned());
    }
    if spec.completion_order.len() != spec.requests.len() {
        return Err("completion order must contain one entry per request".to_owned());
    }
    let expected_indices = (0..spec.requests.len()).collect::<BTreeSet<_>>();
    let actual_indices = spec
        .completion_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if actual_indices != expected_indices {
        return Err("completion order must be a permutation of request indices".to_owned());
    }

    let mut cache = BTreeMap::<StreamedPlanKey, StreamedPlanSnapshot>::new();
    let mut final_snapshots = BTreeMap::<StreamedPlanKey, StreamedPlanSnapshot>::new();
    let mut construction_count = 0_u32;
    let mut cache_hit_count = 0_u32;
    let mut internal_conflict_count = 0_u32;
    let mut first_internal_conflict = None;

    for batch in spec.completion_order.chunks(spec.batch_size) {
        let mut resolved = vec![None; batch.len()];
        let mut misses = Vec::new();
        let mut miss_slots = Vec::new();
        for (slot, request_index) in batch.iter().copied().enumerate() {
            let request = spec.requests[request_index];
            let key =
                descriptor.plan_key(control.candidate_revision(), request.requested_region)?;
            if let Some(snapshot) = cache.get(&key) {
                cache_hit_count = cache_hit_count.saturating_add(1);
                resolved[slot] = Some(snapshot.clone());
            } else {
                misses.push(request);
                miss_slots.push(slot);
            }
        }

        if !misses.is_empty() {
            let built = control.construct_batch(descriptor, &misses)?;
            if built.len() != misses.len() {
                return Err(format!(
                    "{} returned {} plans for {} requests",
                    control.candidate_revision(),
                    built.len(),
                    misses.len()
                ));
            }
            construction_count = construction_count.saturating_add(built.len() as u32);
            for ((slot, request), snapshot) in miss_slots.into_iter().zip(misses).zip(built) {
                let expected =
                    descriptor.plan_key(control.candidate_revision(), request.requested_region)?;
                if snapshot.key != expected {
                    return Err(format!(
                        "{} returned plan key {:?}, expected {:?}",
                        control.candidate_revision(),
                        snapshot.key,
                        expected
                    ));
                }
                cache.insert(snapshot.key.clone(), snapshot.clone());
                resolved[slot] = Some(snapshot);
            }
        }

        for snapshot in resolved {
            let snapshot = snapshot
                .ok_or_else(|| "cache hit or construction left a request unresolved".to_owned())?;
            if let Some(existing) = final_snapshots.insert(snapshot.key.clone(), snapshot.clone())
                && existing != snapshot
            {
                internal_conflict_count = internal_conflict_count.saturating_add(1);
                if first_internal_conflict.is_none() {
                    first_internal_conflict = Some(format!(
                        "plan {:?} was published with two different semantic records",
                        snapshot.key
                    ));
                }
            }
        }
        if spec.cache_policy == HarnessCachePolicy::EvictAfterBatch {
            cache.clear();
        }
    }

    let semantic_sha256 = snapshots_sha256(&final_snapshots);
    Ok(HarnessRunResult {
        case_id: spec.case_id,
        request_order: spec
            .requests
            .iter()
            .map(|request| request.requested_region)
            .collect(),
        completion_order: spec.completion_order,
        batch_size: spec.batch_size,
        cache_policy: spec.cache_policy,
        snapshots: final_snapshots,
        semantic_sha256,
        construction_count,
        cache_hit_count,
        internal_conflict_count,
        first_internal_conflict,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct HarnessComparisonReceipt {
    pub property: String,
    pub control: &'static str,
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub left_case: String,
    pub right_case: String,
    pub expected: HarnessComparisonExpectation,
    pub left_sha256: String,
    pub right_sha256: String,
    pub left_request_order: Vec<PlanRegion>,
    pub right_request_order: Vec<PlanRegion>,
    pub left_completion_order: Vec<usize>,
    pub right_completion_order: Vec<usize>,
    pub left_batch_size: usize,
    pub right_batch_size: usize,
    pub left_cache_policy: HarnessCachePolicy,
    pub right_cache_policy: HarnessCachePolicy,
    pub left_plan_count: u32,
    pub right_plan_count: u32,
    pub left_record_count: u32,
    pub right_record_count: u32,
    pub exact_mismatch_count: u64,
    pub first_mismatch: Option<String>,
    pub left_construction_count: u32,
    pub right_construction_count: u32,
    pub left_cache_hit_count: u32,
    pub right_cache_hit_count: u32,
    pub left_internal_conflict_count: u32,
    pub right_internal_conflict_count: u32,
    pub first_internal_conflict: Option<String>,
    pub passed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessComparisonExpectation {
    Equal,
    Different,
}

impl HarnessComparisonReceipt {
    pub(crate) fn compare(
        property: impl Into<String>,
        control: &'static str,
        descriptor: &StreamedPlanDescriptor,
        expected: HarnessComparisonExpectation,
        left: &HarnessRunResult,
        right: &HarnessRunResult,
    ) -> Self {
        let (exact_mismatch_count, first_mismatch) = compare_snapshots(left, right);
        let has_internal_conflict =
            left.internal_conflict_count > 0 || right.internal_conflict_count > 0;
        let passed = match expected {
            HarnessComparisonExpectation::Equal => {
                exact_mismatch_count == 0 && !has_internal_conflict
            }
            HarnessComparisonExpectation::Different => {
                exact_mismatch_count > 0 || has_internal_conflict
            }
        };
        Self {
            property: property.into(),
            control,
            seed: descriptor.seed,
            topology: descriptor.topology,
            left_case: left.case_id.clone(),
            right_case: right.case_id.clone(),
            expected,
            left_sha256: left.semantic_sha256.clone(),
            right_sha256: right.semantic_sha256.clone(),
            left_request_order: left.request_order.clone(),
            right_request_order: right.request_order.clone(),
            left_completion_order: left.completion_order.clone(),
            right_completion_order: right.completion_order.clone(),
            left_batch_size: left.batch_size,
            right_batch_size: right.batch_size,
            left_cache_policy: left.cache_policy,
            right_cache_policy: right.cache_policy,
            left_plan_count: left.snapshots.len() as u32,
            right_plan_count: right.snapshots.len() as u32,
            left_record_count: semantic_record_count(left),
            right_record_count: semantic_record_count(right),
            exact_mismatch_count,
            first_mismatch,
            left_construction_count: left.construction_count,
            right_construction_count: right.construction_count,
            left_cache_hit_count: left.cache_hit_count,
            right_cache_hit_count: right.cache_hit_count,
            left_internal_conflict_count: left.internal_conflict_count,
            right_internal_conflict_count: right.internal_conflict_count,
            first_internal_conflict: left
                .first_internal_conflict
                .clone()
                .or_else(|| right.first_internal_conflict.clone()),
            passed,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HarnessDependencyClaim {
    pub control: &'static str,
    pub maximum_depth: u32,
    pub maximum_provider_fanout: u32,
    pub maximum_owner_radius_blocks: u32,
    pub maximum_point_lookup_traversal_steps: u32,
    pub undeclared_fetch_count: u32,
    pub note: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StreamedPlanPhaseOneReceipt {
    pub receipt_schema: &'static str,
    pub harness_revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub geography_measured: bool,
    pub reconstruction_measured: bool,
    pub cost_measured: bool,
    pub stored_profile_revision: &'static str,
    pub dimension_id: &'static str,
    pub plan_family: &'static str,
    pub numeric_policy: &'static str,
    pub tie_domains: [&'static str; 2],
    pub random_domains: [&'static str; 2],
    pub semantic_cell_blocks: i32,
    pub base_region_blocks: i32,
    pub period_blocks: i32,
    pub seeds: [i64; 3],
    pub topologies: [StreamedPlanTopology; 3],
    pub coordinate_pure_corpus_sha256: String,
    pub comparison_corpus_sha256: String,
    pub dependency_claims: Vec<HarnessDependencyClaim>,
    pub comparisons: Vec<HarnessComparisonReceipt>,
    pub exact_comparison_count: u32,
    pub expected_failure_canary_count: u32,
    pub suite_passed: bool,
    pub artifacts: Vec<String>,
}

pub fn run_streamed_plan_phase_one_suite() -> Result<StreamedPlanPhaseOneReceipt, String> {
    let mut comparisons = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            let descriptor = StreamedPlanDescriptor::new(seed, topology);
            let targets = canonical_targets(topology);
            let baseline_requests = requests_with_center(&targets, PlanRegion::new(0, 0));
            let baseline = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec::in_request_order("row-major", baseline_requests.clone()),
            )?;

            for (case_id, property, permutation) in [
                (
                    "reverse",
                    "request-order/reverse",
                    reverse_targets(&targets),
                ),
                (
                    "center-out",
                    "path/center-out",
                    center_out_targets(&targets),
                ),
                (
                    "outside-in",
                    "path/outside-in",
                    outside_in_targets(&targets),
                ),
                (
                    "fixed-random",
                    "request-order/fixed-random",
                    random_targets(seed, &targets),
                ),
                ("teleport", "path/teleport", alternating_targets(&targets)),
                ("two-front", "path/two-front", two_front_targets(&targets)),
            ] {
                let run = execute_harness_case(
                    &descriptor,
                    CoordinatePureControl,
                    HarnessRunSpec {
                        batch_size: 1,
                        ..HarnessRunSpec::in_request_order(
                            case_id,
                            requests_with_center(&permutation, PlanRegion::new(0, 0)),
                        )
                    },
                )?;
                comparisons.push(HarnessComparisonReceipt::compare(
                    property,
                    FALLBACK_CONTROL_REVISION,
                    &descriptor,
                    HarnessComparisonExpectation::Equal,
                    &baseline,
                    &run,
                ));
            }

            let schedule_reversed = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec {
                    case_id: "schedule-reversed".to_owned(),
                    requests: baseline_requests.clone(),
                    completion_order: (0..baseline_requests.len()).rev().collect(),
                    batch_size: 1,
                    cache_policy: HarnessCachePolicy::Retain,
                },
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "schedule/reversed-completion",
                FALLBACK_CONTROL_REVISION,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &baseline,
                &schedule_reversed,
            ));

            let singleton_batches = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec {
                    batch_size: 1,
                    ..HarnessRunSpec::in_request_order(
                        "singleton-batches",
                        baseline_requests.clone(),
                    )
                },
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "partition/singleton-batches",
                FALLBACK_CONTROL_REVISION,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &baseline,
                &singleton_batches,
            ));

            let recentered_requests = targets
                .iter()
                .copied()
                .map(|target| StreamedPlanRequest::new(target, target))
                .collect::<Vec<_>>();
            let recentered = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec::in_request_order("window-follows-target", recentered_requests),
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "window/viewport-center",
                FALLBACK_CONTROL_REVISION,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &baseline,
                &recentered,
            ));

            if topology != StreamedPlanTopology::Plane {
                let lifted = targets
                    .iter()
                    .copied()
                    .map(|target| {
                        StreamedPlanRequest::new(
                            topology.lifted_region(target),
                            PlanRegion::new(0, 0),
                        )
                    })
                    .collect::<Vec<_>>();
                let lifted = execute_harness_case(
                    &descriptor,
                    CoordinatePureControl,
                    HarnessRunSpec::in_request_order("periodic-lifts", lifted),
                )?;
                comparisons.push(HarnessComparisonReceipt::compare(
                    "topology/lift",
                    FALLBACK_CONTROL_REVISION,
                    &descriptor,
                    HarnessComparisonExpectation::Equal,
                    &baseline,
                    &lifted,
                ));
            }

            let repeated_requests = baseline_requests
                .iter()
                .copied()
                .chain(baseline_requests.iter().copied())
                .collect::<Vec<_>>();
            let warm_cache = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec {
                    batch_size: 1,
                    ..HarnessRunSpec::in_request_order("warm-cache", repeated_requests.clone())
                },
            )?;
            let evicted_cache = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec {
                    case_id: "evict-each-request".to_owned(),
                    completion_order: (0..repeated_requests.len()).collect(),
                    requests: repeated_requests,
                    batch_size: 1,
                    cache_policy: HarnessCachePolicy::EvictAfterBatch,
                },
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "cache/warm-versus-evicted",
                FALLBACK_CONTROL_REVISION,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &warm_cache,
                &evicted_cache,
            ));
        }
    }

    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        let descriptor = StreamedPlanDescriptor::new(seed, StreamedPlanTopology::Plane);
        let target = PlanRegion::new(0, 0);
        let fixed_request = vec![StreamedPlanRequest::new(target, PlanRegion::new(0, 0))];
        let fixed_left = execute_harness_case(
            &descriptor,
            FixedWindowLandformControl,
            HarnessRunSpec::in_request_order("fixed-rebuild-a", fixed_request.clone()),
        )?;
        let fixed_right = execute_harness_case(
            &descriptor,
            FixedWindowLandformControl,
            HarnessRunSpec::in_request_order("fixed-rebuild-b", fixed_request),
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "rebuild/fixed-bounded-plan",
            FIXED_WINDOW_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Equal,
            &fixed_left,
            &fixed_right,
        ));

        let centered = execute_harness_case(
            &descriptor,
            RecenteredLandformControl,
            HarnessRunSpec::in_request_order(
                "center-region-0",
                vec![StreamedPlanRequest::new(target, PlanRegion::new(0, 0))],
            ),
        )?;
        let shifted = execute_harness_case(
            &descriptor,
            RecenteredLandformControl,
            HarnessRunSpec::in_request_order(
                "center-region-1",
                vec![StreamedPlanRequest::new(target, PlanRegion::new(1, 0))],
            ),
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "window/recentered-bounded-solve-canary",
            RECENTER_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Different,
            &centered,
            &shifted,
        ));

        let recentered_publications = vec![
            StreamedPlanRequest::new(target, PlanRegion::new(0, 0)),
            StreamedPlanRequest::new(target, PlanRegion::new(1, 0)),
        ];
        let last_writer_forward = execute_harness_case(
            &descriptor,
            RecenteredLandformControl,
            HarnessRunSpec {
                case_id: "recentered-last-writer-forward".to_owned(),
                completion_order: vec![0, 1],
                requests: recentered_publications.clone(),
                batch_size: 2,
                cache_policy: HarnessCachePolicy::Retain,
            },
        )?;
        let last_writer_reverse = execute_harness_case(
            &descriptor,
            RecenteredLandformControl,
            HarnessRunSpec {
                case_id: "recentered-last-writer-reverse".to_owned(),
                completion_order: vec![1, 0],
                requests: recentered_publications,
                batch_size: 2,
                cache_policy: HarnessCachePolicy::Retain,
            },
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "publication/recentered-last-writer-canary",
            RECENTER_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Different,
            &last_writer_forward,
            &last_writer_reverse,
        ));

        let discovery_targets = canonical_targets(StreamedPlanTopology::Plane)
            .into_iter()
            .filter(|target| (-2..=2).contains(&target.x) && (-2..=2).contains(&target.z))
            .collect::<Vec<_>>();
        let discovery_forward = execute_harness_case(
            &descriptor,
            DiscoveryStateControl::default(),
            HarnessRunSpec {
                batch_size: 1,
                ..HarnessRunSpec::in_request_order(
                    "discovery-forward",
                    requests_with_center(&discovery_targets, PlanRegion::new(0, 0)),
                )
            },
        )?;
        let discovery_reverse = execute_harness_case(
            &descriptor,
            DiscoveryStateControl::default(),
            HarnessRunSpec {
                batch_size: 1,
                ..HarnessRunSpec::in_request_order(
                    "discovery-reverse",
                    requests_with_center(
                        &reverse_targets(&discovery_targets),
                        PlanRegion::new(0, 0),
                    ),
                )
            },
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "request-order/discovery-state-canary",
            DISCOVERY_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Different,
            &discovery_forward,
            &discovery_reverse,
        ));

        let discovery_requests = requests_with_center(&discovery_targets, PlanRegion::new(0, 0));
        let discovery_schedule = execute_harness_case(
            &descriptor,
            DiscoveryStateControl::default(),
            HarnessRunSpec {
                case_id: "discovery-schedule-reversed".to_owned(),
                completion_order: (0..discovery_requests.len()).rev().collect(),
                requests: discovery_requests,
                batch_size: 1,
                cache_policy: HarnessCachePolicy::Retain,
            },
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "schedule/discovery-state-canary",
            DISCOVERY_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Different,
            &discovery_forward,
            &discovery_schedule,
        ));

        let repeated_target = vec![
            StreamedPlanRequest::new(target, PlanRegion::new(0, 0)),
            StreamedPlanRequest::new(target, PlanRegion::new(0, 0)),
        ];
        let discovery_warm = execute_harness_case(
            &descriptor,
            DiscoveryStateControl::default(),
            HarnessRunSpec {
                batch_size: 1,
                ..HarnessRunSpec::in_request_order("discovery-warm-cache", repeated_target.clone())
            },
        )?;
        let discovery_evicted = execute_harness_case(
            &descriptor,
            DiscoveryStateControl::default(),
            HarnessRunSpec {
                case_id: "discovery-evicted-cache".to_owned(),
                completion_order: vec![0, 1],
                requests: repeated_target,
                batch_size: 1,
                cache_policy: HarnessCachePolicy::EvictAfterBatch,
            },
        )?;
        comparisons.push(HarnessComparisonReceipt::compare(
            "cache/discovery-state-canary",
            DISCOVERY_CONTROL_REVISION,
            &descriptor,
            HarnessComparisonExpectation::Different,
            &discovery_warm,
            &discovery_evicted,
        ));
    }

    let expected_failure_canary_count = comparisons
        .iter()
        .filter(|comparison| comparison.expected == HarnessComparisonExpectation::Different)
        .count() as u32;
    let exact_comparison_count = comparisons.len() as u32;
    let suite_passed = comparisons.iter().all(|comparison| comparison.passed);
    let comparison_corpus_sha256 = comparison_corpus_sha256(&comparisons);
    Ok(StreamedPlanPhaseOneReceipt {
        receipt_schema: STREAMED_PLAN_RECEIPT_SCHEMA_REVISION,
        harness_revision: STREAMED_PLAN_HARNESS_SCHEMA_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        geography_measured: false,
        reconstruction_measured: false,
        cost_measured: false,
        stored_profile_revision: STREAMED_PLAN_STORED_PROFILE_REVISION,
        dimension_id: STREAMED_PLAN_DIMENSION_ID,
        plan_family: STREAMED_PLAN_FAMILY,
        numeric_policy: "canonical-little-endian-fixed-width-integers",
        tie_domains: ["fact-id", "euclidean-coordinate"],
        random_domains: ["world-seed", "fact-id-and-choice-index"],
        semantic_cell_blocks: MCLONE_LANDFORM_PLAN_CELL_BLOCKS,
        base_region_blocks: STREAMED_PLAN_BASE_REGION_BLOCKS,
        period_blocks: STREAMED_PLAN_PERIOD_BLOCKS,
        seeds: STREAMED_PLAN_PHASE_ONE_SEEDS,
        topologies: StreamedPlanTopology::ALL,
        coordinate_pure_corpus_sha256: coordinate_pure_corpus_sha256()?,
        comparison_corpus_sha256,
        dependency_claims: phase_one_dependency_claims(),
        comparisons,
        exact_comparison_count,
        expected_failure_canary_count,
        suite_passed,
        artifacts: Vec::new(),
    })
}

pub fn coordinate_pure_corpus_sha256() -> Result<String, String> {
    let mut bytes = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            let descriptor = StreamedPlanDescriptor::new(seed, topology);
            let targets = canonical_targets(topology);
            let run = execute_harness_case(
                &descriptor,
                CoordinatePureControl,
                HarnessRunSpec::in_request_order(
                    "canonical-corpus",
                    requests_with_center(&targets, PlanRegion::new(0, 0)),
                ),
            )?;
            write_i64(&mut bytes, seed);
            bytes.push(topology_tag(topology));
            write_string(&mut bytes, &run.semantic_sha256);
        }
    }
    Ok(sha256_hex(&bytes))
}

pub(crate) fn comparison_corpus_sha256(comparisons: &[HarnessComparisonReceipt]) -> String {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, comparisons.len() as u32);
    for comparison in comparisons {
        write_string(&mut bytes, &comparison.property);
        write_string(&mut bytes, comparison.control);
        write_i64(&mut bytes, comparison.seed);
        bytes.push(topology_tag(comparison.topology));
        write_string(&mut bytes, &comparison.left_case);
        write_string(&mut bytes, &comparison.right_case);
        bytes.push(match comparison.expected {
            HarnessComparisonExpectation::Equal => 0,
            HarnessComparisonExpectation::Different => 1,
        });
        write_string(&mut bytes, &comparison.left_sha256);
        write_string(&mut bytes, &comparison.right_sha256);
        write_regions(&mut bytes, &comparison.left_request_order);
        write_regions(&mut bytes, &comparison.right_request_order);
        write_usizes(&mut bytes, &comparison.left_completion_order);
        write_usizes(&mut bytes, &comparison.right_completion_order);
        bytes.extend_from_slice(&(comparison.left_batch_size as u64).to_le_bytes());
        bytes.extend_from_slice(&(comparison.right_batch_size as u64).to_le_bytes());
        bytes.push(cache_policy_tag(comparison.left_cache_policy));
        bytes.push(cache_policy_tag(comparison.right_cache_policy));
        write_u32(&mut bytes, comparison.left_plan_count);
        write_u32(&mut bytes, comparison.right_plan_count);
        write_u32(&mut bytes, comparison.left_record_count);
        write_u32(&mut bytes, comparison.right_record_count);
        bytes.extend_from_slice(&comparison.exact_mismatch_count.to_le_bytes());
        match &comparison.first_mismatch {
            Some(first_mismatch) => {
                bytes.push(1);
                write_string(&mut bytes, first_mismatch);
            }
            None => bytes.push(0),
        }
        write_u32(&mut bytes, comparison.left_construction_count);
        write_u32(&mut bytes, comparison.right_construction_count);
        write_u32(&mut bytes, comparison.left_cache_hit_count);
        write_u32(&mut bytes, comparison.right_cache_hit_count);
        write_u32(&mut bytes, comparison.left_internal_conflict_count);
        write_u32(&mut bytes, comparison.right_internal_conflict_count);
        match &comparison.first_internal_conflict {
            Some(first_internal_conflict) => {
                bytes.push(1);
                write_string(&mut bytes, first_internal_conflict);
            }
            None => bytes.push(0),
        }
        bytes.push(comparison.passed as u8);
    }
    sha256_hex(&bytes)
}

fn semantic_record_count(result: &HarnessRunResult) -> u32 {
    result
        .snapshots
        .values()
        .map(|snapshot| snapshot.facts.len() as u32)
        .sum()
}

fn write_regions(bytes: &mut Vec<u8>, regions: &[PlanRegion]) {
    write_u32(bytes, regions.len() as u32);
    for region in regions {
        write_i32(bytes, region.x);
        write_i32(bytes, region.z);
    }
}

fn write_usizes(bytes: &mut Vec<u8>, values: &[usize]) {
    write_u32(bytes, values.len() as u32);
    for value in values {
        bytes.extend_from_slice(&(*value as u64).to_le_bytes());
    }
}

fn cache_policy_tag(policy: HarnessCachePolicy) -> u8 {
    match policy {
        HarnessCachePolicy::Retain => 0,
        HarnessCachePolicy::EvictAfterBatch => 1,
    }
}

fn phase_one_dependency_claims() -> Vec<HarnessDependencyClaim> {
    vec![
        HarnessDependencyClaim {
            control: FALLBACK_CONTROL_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 1,
            maximum_owner_radius_blocks: 8,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "coordinate-pure samples plus one bounded canonical feature owner",
        },
        HarnessDependencyClaim {
            control: FIXED_WINDOW_CONTROL_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 0,
            maximum_owner_radius_blocks: 0,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "complete fixed 6144-block research solve; not streamed",
        },
        HarnessDependencyClaim {
            control: RECENTER_CONTROL_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 0,
            maximum_owner_radius_blocks: 0,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "deliberately invalid viewport-centered plan identity",
        },
        HarnessDependencyClaim {
            control: DISCOVERY_CONTROL_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 0,
            maximum_owner_radius_blocks: 0,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "deliberately invalid mutable discovery counter",
        },
    ]
}

#[derive(Clone, Copy, Debug)]
struct CoordinatePureControl;

impl StreamedPlanControl for CoordinatePureControl {
    fn candidate_revision(&self) -> &'static str {
        FALLBACK_CONTROL_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        let key = descriptor.plan_key(self.candidate_revision(), request.requested_region)?;
        let region = key.region;
        let source = TopologyProbeSource::new(descriptor.seed, descriptor.topology.horizontal())?;
        let offsets = [16, 496, 1_008];
        let mut facts = Vec::new();
        for (sample_index, (offset_x, offset_z)) in offsets
            .into_iter()
            .flat_map(|z| offsets.into_iter().map(move |x| (x, z)))
            .enumerate()
        {
            let world_x = region.min_block_x() + offset_x;
            let world_z = region.min_block_z() + offset_z;
            let sample = source.sample_column(world_x, world_z);
            facts.push(StreamedSemanticFact {
                id: StreamedFactId {
                    owner: key.clone(),
                    kind: FALLBACK_SAMPLE_KIND,
                    local_index: sample_index as u32,
                },
                world_x,
                world_z,
                payload: vec![
                    i64::from(sample.surface_y),
                    i64::from(sample.water_surface_y.unwrap_or(i32::MIN)),
                    i64::from(sample.ridge_lift),
                    sample.is_channel as i64,
                    sample.is_island as i64,
                    sample.is_pond as i64,
                    i64::from(sample.top_block),
                ],
            });
        }

        let region_hash = stable_mix64(
            descriptor.seed as u64
                ^ (region.x as u32 as u64).rotate_left(17)
                ^ (region.z as u32 as u64).rotate_left(41),
        );
        facts.push(StreamedSemanticFact {
            id: StreamedFactId {
                owner: key.clone(),
                kind: FALLBACK_REGION_KIND,
                local_index: 0,
            },
            world_x: region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            world_z: region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            payload: vec![region_hash as i64],
        });

        let center = BlockPos::new(
            region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            0,
            region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
        );
        let work_plan = source.arch_plan_near(center);
        let radius = work_plan.canonical.influence_radius_blocks as i64;
        let region_min_x = i64::from(region.min_block_x());
        let region_min_z = i64::from(region.min_block_z());
        let region_max_x = region_min_x + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
        let region_max_z = region_min_z + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
        if work_plan.work_anchor_x + radius >= region_min_x
            && work_plan.work_anchor_x - radius < region_max_x
            && work_plan.work_anchor_z + radius >= region_min_z
            && work_plan.work_anchor_z - radius < region_max_z
        {
            let owner_region = descriptor.canonical_region(PlanRegion::new(
                work_plan
                    .canonical
                    .anchor
                    .x
                    .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
                work_plan
                    .canonical
                    .anchor
                    .z
                    .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
            ))?;
            let mut owner = key.clone();
            owner.region = owner_region;
            facts.push(StreamedSemanticFact {
                id: StreamedFactId {
                    owner,
                    kind: FALLBACK_FEATURE_KIND,
                    local_index: 0,
                },
                world_x: work_plan.canonical.anchor.x,
                world_z: work_plan.canonical.anchor.z,
                payload: vec![
                    work_plan.canonical.id as i64,
                    i64::from(work_plan.canonical.owner.x),
                    i64::from(work_plan.canonical.owner.z),
                    i64::from(work_plan.canonical.influence_radius_blocks),
                ],
            });
        }
        StreamedPlanSnapshot::new(key, facts)
    }
}

#[derive(Clone, Copy, Debug)]
struct FixedWindowLandformControl;

impl StreamedPlanControl for FixedWindowLandformControl {
    fn candidate_revision(&self) -> &'static str {
        FIXED_WINDOW_CONTROL_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        require_plane(descriptor, self.candidate_revision())?;
        let summary = McloneLandformPlanSummary::build_plane(descriptor.seed);
        landform_region_snapshot(descriptor, request, self.candidate_revision(), &summary)
    }
}

#[derive(Clone, Copy, Debug)]
struct RecenteredLandformControl;

impl StreamedPlanControl for RecenteredLandformControl {
    fn candidate_revision(&self) -> &'static str {
        RECENTER_CONTROL_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        require_plane(descriptor, self.candidate_revision())?;
        let min_x =
            request.window_center_region.min_block_x() - MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2;
        let min_z =
            request.window_center_region.min_block_z() - MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2;
        let summary = McloneLandformPlanSummary::build_plane_window(descriptor.seed, min_x, min_z);
        landform_region_snapshot(descriptor, request, self.candidate_revision(), &summary)
    }
}

#[derive(Clone, Debug, Default)]
struct DiscoveryStateControl {
    discovery_count: u32,
    seen: BTreeSet<PlanRegion>,
}

impl StreamedPlanControl for DiscoveryStateControl {
    fn candidate_revision(&self) -> &'static str {
        DISCOVERY_CONTROL_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        let key = descriptor.plan_key(self.candidate_revision(), request.requested_region)?;
        let region = key.region;
        let neighbor_count = [
            PlanRegion::new(region.x - 1, region.z),
            PlanRegion::new(region.x + 1, region.z),
            PlanRegion::new(region.x, region.z - 1),
            PlanRegion::new(region.x, region.z + 1),
        ]
        .into_iter()
        .filter(|neighbor| self.seen.contains(neighbor))
        .count() as i64;
        let discovery_count = self.discovery_count;
        self.discovery_count = self.discovery_count.saturating_add(1);
        self.seen.insert(region);
        let fact = StreamedSemanticFact {
            id: StreamedFactId {
                owner: key.clone(),
                kind: DISCOVERY_CELL_KIND,
                local_index: 0,
            },
            world_x: region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            world_z: region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            payload: vec![i64::from(discovery_count), neighbor_count],
        };
        StreamedPlanSnapshot::new(key, vec![fact])
    }
}

fn landform_region_snapshot(
    descriptor: &StreamedPlanDescriptor,
    request: StreamedPlanRequest,
    candidate_revision: &'static str,
    summary: &McloneLandformPlanSummary,
) -> Result<StreamedPlanSnapshot, String> {
    let key = descriptor.plan_key(candidate_revision, request.requested_region)?;
    let region = key.region;
    let cells_per_region = STREAMED_PLAN_BASE_REGION_BLOCKS / MCLONE_LANDFORM_PLAN_CELL_BLOCKS;
    let summary_cell_count = usize::from(summary.width_cells) * usize::from(summary.depth_cells);
    let mut facts = Vec::with_capacity((cells_per_region * cells_per_region) as usize);
    for local_z in 0..cells_per_region {
        for local_x in 0..cells_per_region {
            let world_x = region.min_block_x()
                + local_x * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2;
            let world_z = region.min_block_z()
                + local_z * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2;
            let point = summary
                .point(f64::from(world_x), f64::from(world_z))
                .ok_or_else(|| {
                    format!(
                        "target region {:?} is outside landform window [{}, {}) x [{}, {})",
                        region,
                        summary.min_x,
                        summary.min_x + MCLONE_LANDFORM_PLAN_STUDY_BLOCKS,
                        summary.min_z,
                        summary.min_z + MCLONE_LANDFORM_PLAN_STUDY_BLOCKS
                    )
                })?;
            let basin_sink = summary
                .sinks
                .get(usize::from(point.basin_id))
                .ok_or_else(|| format!("invalid basin id {}", point.basin_id))?;
            let receiver = usize::try_from(point.receiver_id)
                .map_err(|_| format!("receiver id {} does not fit usize", point.receiver_id))?;
            let (receiver_x, receiver_z) = if receiver < summary_cell_count {
                let grid_x = receiver % usize::from(summary.width_cells);
                let grid_z = receiver / usize::from(summary.width_cells);
                (
                    summary.min_x
                        + grid_x as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                        + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
                    summary.min_z
                        + grid_z as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                        + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
                )
            } else {
                let sink_id = receiver - summary_cell_count;
                let sink = summary
                    .sinks
                    .get(sink_id)
                    .ok_or_else(|| format!("invalid receiver sink id {sink_id}"))?;
                (sink.world_x, sink.world_z)
            };
            let local_index = (local_z * cells_per_region + local_x) as u32;
            facts.push(StreamedSemanticFact {
                id: StreamedFactId {
                    owner: key.clone(),
                    kind: LANDFORM_CELL_KIND,
                    local_index,
                },
                world_x,
                world_z,
                payload: vec![
                    i64::from(basin_sink.world_x),
                    i64::from(basin_sink.world_z),
                    i64::from(receiver_x),
                    i64::from(receiver_z),
                    i64::from(point.accumulation),
                    i64::from(point.stream_order),
                    i64::from(point.flags & !MCLONE_LANDFORM_PLAN_CELL_CROP_EDGE),
                    i64::from(point.base_y),
                ],
            });
        }
    }
    StreamedPlanSnapshot::new(key, facts)
}

fn require_plane(descriptor: &StreamedPlanDescriptor, control: &'static str) -> Result<(), String> {
    if descriptor.topology == StreamedPlanTopology::Plane {
        Ok(())
    } else {
        Err(format!("{control} supports only the plane control corpus"))
    }
}

pub(crate) fn canonical_targets(topology: StreamedPlanTopology) -> Vec<PlanRegion> {
    let mut targets = Vec::new();
    match topology {
        StreamedPlanTopology::Plane => {
            for z in -2..=2 {
                for x in -2..=2 {
                    targets.push(PlanRegion::new(x, z));
                }
            }
            targets.extend([
                PlanRegion::new(-23, 17),
                PlanRegion::new(41, -29),
                PlanRegion::new(-64, -64),
                PlanRegion::new(63, 63),
            ]);
        }
        StreamedPlanTopology::CylinderX => {
            for z in -2..=2 {
                for x in 0..STREAMED_PLAN_PERIOD_REGIONS {
                    targets.push(PlanRegion::new(x, z));
                }
            }
        }
        StreamedPlanTopology::Torus => {
            for z in 0..STREAMED_PLAN_PERIOD_REGIONS {
                for x in 0..STREAMED_PLAN_PERIOD_REGIONS {
                    targets.push(PlanRegion::new(x, z));
                }
            }
        }
    }
    targets
}

pub(crate) fn requests_with_center(
    targets: &[PlanRegion],
    center: PlanRegion,
) -> Vec<StreamedPlanRequest> {
    targets
        .iter()
        .copied()
        .map(|target| StreamedPlanRequest::new(target, center))
        .collect()
}

pub(crate) fn reverse_targets(targets: &[PlanRegion]) -> Vec<PlanRegion> {
    targets.iter().copied().rev().collect()
}

pub(crate) fn center_out_targets(targets: &[PlanRegion]) -> Vec<PlanRegion> {
    let mut targets = targets.to_vec();
    targets.sort_by_key(|target| {
        (
            i64::from(target.x).pow(2) + i64::from(target.z).pow(2),
            target.z,
            target.x,
        )
    });
    targets
}

pub(crate) fn outside_in_targets(targets: &[PlanRegion]) -> Vec<PlanRegion> {
    center_out_targets(targets).into_iter().rev().collect()
}

pub(crate) fn random_targets(seed: i64, targets: &[PlanRegion]) -> Vec<PlanRegion> {
    let mut targets = targets.to_vec();
    targets.sort_by_key(|target| {
        stable_mix64(
            seed as u64
                ^ (target.x as u32 as u64).rotate_left(13)
                ^ (target.z as u32 as u64).rotate_left(37),
        )
    });
    targets
}

pub(crate) fn alternating_targets(targets: &[PlanRegion]) -> Vec<PlanRegion> {
    let mut result = Vec::with_capacity(targets.len());
    let mut left = 0;
    let mut right = targets.len();
    while left < right {
        result.push(targets[left]);
        left += 1;
        if left < right {
            right -= 1;
            result.push(targets[right]);
        }
    }
    result
}

pub(crate) fn two_front_targets(targets: &[PlanRegion]) -> Vec<PlanRegion> {
    let mut sorted = targets.to_vec();
    sorted.sort_by_key(|target| (target.x, target.z));
    alternating_targets(&sorted)
}

fn compare_snapshots(left: &HarnessRunResult, right: &HarnessRunResult) -> (u64, Option<String>) {
    let mut mismatch_count = 0_u64;
    let mut first_mismatch = None;
    let plan_keys = left
        .snapshots
        .keys()
        .chain(right.snapshots.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in plan_keys {
        match (left.snapshots.get(&key), right.snapshots.get(&key)) {
            (Some(left_plan), Some(right_plan)) => {
                let fact_ids = left_plan
                    .facts
                    .iter()
                    .map(|fact| fact.id.clone())
                    .chain(right_plan.facts.iter().map(|fact| fact.id.clone()))
                    .collect::<BTreeSet<_>>();
                let left_facts = left_plan
                    .facts
                    .iter()
                    .map(|fact| (&fact.id, fact))
                    .collect::<BTreeMap<_, _>>();
                let right_facts = right_plan
                    .facts
                    .iter()
                    .map(|fact| (&fact.id, fact))
                    .collect::<BTreeMap<_, _>>();
                for fact_id in fact_ids {
                    let left_fact = left_facts.get(&fact_id);
                    let right_fact = right_facts.get(&fact_id);
                    if left_fact != right_fact {
                        mismatch_count = mismatch_count.saturating_add(1);
                        if first_mismatch.is_none() {
                            first_mismatch = Some(format!(
                                "plan {:?}, fact {:?}: left={:?}, right={:?}",
                                key, fact_id, left_fact, right_fact
                            ));
                        }
                    }
                }
            }
            (left_plan, right_plan) => {
                mismatch_count = mismatch_count.saturating_add(
                    left_plan
                        .map_or(0, |plan| plan.facts.len())
                        .max(right_plan.map_or(0, |plan| plan.facts.len()))
                        as u64,
                );
                if first_mismatch.is_none() {
                    first_mismatch = Some(format!(
                        "plan {:?} missing: left={}, right={}",
                        key,
                        left_plan.is_some(),
                        right_plan.is_some()
                    ));
                }
            }
        }
    }
    (mismatch_count, first_mismatch)
}

fn snapshots_sha256(snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>) -> String {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, snapshots.len() as u32);
    for snapshot in snapshots.values() {
        let snapshot_bytes = snapshot.canonical_bytes();
        write_u32(&mut bytes, snapshot_bytes.len() as u32);
        bytes.extend_from_slice(&snapshot_bytes);
    }
    sha256_hex(&bytes)
}

fn write_plan_key(bytes: &mut Vec<u8>, key: &StreamedPlanKey) {
    write_string(bytes, key.harness_revision);
    write_string(bytes, key.stored_profile_revision);
    write_string(bytes, key.dimension_id);
    write_string(bytes, &key.topology_descriptor_sha256);
    write_string(bytes, key.candidate_revision);
    write_string(bytes, key.plan_family);
    write_i64(bytes, key.seed);
    bytes.push(topology_tag(key.topology));
    bytes.push(key.level);
    write_i32(bytes, key.region.x);
    write_i32(bytes, key.region.z);
}

fn topology_tag(topology: StreamedPlanTopology) -> u8 {
    match topology {
        StreamedPlanTopology::Plane => 0,
        StreamedPlanTopology::CylinderX => 1,
        StreamedPlanTopology::Torus => 2,
    }
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(bytes, value.len() as u32);
    bytes.extend_from_slice(value.as_bytes());
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stable_mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_one_suite_detects_canaries_and_accepts_pure_control() {
        let receipt = run_streamed_plan_phase_one_suite().expect("Phase 1 suite");
        assert!(receipt.suite_passed);
        assert_eq!(
            receipt.coordinate_pure_corpus_sha256,
            STREAMED_PLAN_PHASE_ONE_FALLBACK_SHA256
        );
        assert_eq!(
            receipt.comparison_corpus_sha256,
            STREAMED_PLAN_PHASE_ONE_COMPARISON_SHA256
        );
        assert_eq!(receipt.expected_failure_canary_count, 15);
        assert!(
            receipt
                .comparisons
                .iter()
                .filter(|comparison| comparison.expected == HarnessComparisonExpectation::Equal)
                .all(|comparison| comparison.exact_mismatch_count == 0
                    && comparison.left_internal_conflict_count == 0
                    && comparison.right_internal_conflict_count == 0)
        );
        assert!(
            receipt
                .comparisons
                .iter()
                .filter(|comparison| comparison.expected == HarnessComparisonExpectation::Different)
                .all(|comparison| comparison.exact_mismatch_count > 0
                    || comparison.left_internal_conflict_count > 0
                    || comparison.right_internal_conflict_count > 0)
        );
        assert_eq!(
            receipt
                .comparisons
                .iter()
                .filter(|comparison| {
                    comparison.property == "publication/recentered-last-writer-canary"
                        && comparison.left_internal_conflict_count > 0
                        && comparison.right_internal_conflict_count > 0
                })
                .count(),
            STREAMED_PLAN_PHASE_ONE_SEEDS.len()
        );
    }

    #[test]
    fn plan_region_canonicalization_uses_shared_topology() {
        let cylinder = StreamedPlanDescriptor::new(12_345, StreamedPlanTopology::CylinderX);
        assert_eq!(
            cylinder.canonical_region(PlanRegion::new(-1, 7)).unwrap(),
            PlanRegion::new(5, 7)
        );
        assert_eq!(
            cylinder.canonical_region(PlanRegion::new(6, -3)).unwrap(),
            PlanRegion::new(0, -3)
        );

        let torus = StreamedPlanDescriptor::new(12_345, StreamedPlanTopology::Torus);
        assert_eq!(
            torus.canonical_region(PlanRegion::new(-1, 6)).unwrap(),
            PlanRegion::new(5, 0)
        );
    }
}
