//! Semantic-only streamed landscape planner trials for Tactical 270.
//!
//! These candidates exercise identity, bounded dependency, boundary, cache,
//! schedule, and topology mechanics. Production terrain does not consume
//! their facts, and the facts are not yet a terrain-quality proposal.

use std::collections::BTreeMap;

use mclone_core::BlockPos;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::streamed_plan_feature_graph_trial::{FeatureGraphTrialReceipt, run_feature_graph_trial};
use crate::streamed_plan_harness::{
    CoordinatePureControl, FALLBACK_CONTROL_REVISION, HarnessCachePolicy,
    HarnessComparisonExpectation, HarnessComparisonReceipt, HarnessDependencyClaim, HarnessRunSpec,
    PlanRegion, STREAMED_PLAN_BASE_REGION_BLOCKS, STREAMED_PLAN_PHASE_ONE_SEEDS, StreamedFactId,
    StreamedPlanControl, StreamedPlanDescriptor, StreamedPlanRequest, StreamedPlanSnapshot,
    StreamedPlanTopology, StreamedSemanticFact, alternating_targets, canonical_targets,
    center_out_targets, comparison_corpus_sha256, execute_harness_case, outside_in_targets,
    random_targets, requests_with_center, reverse_targets, two_front_targets,
};

pub const STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION: &str = "mclone-streamed-plan-phase2-receipt-v1";
pub const STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256: &str =
    "d3dfa82df7de1a6c9e1d24f97b7f824b640c181cecd6295e86180344a2c081bd";
pub const HIERARCHICAL_CANDIDATE_REVISION: &str = "hierarchical-shared-boundary-facts-trial-v1";

const HIERARCHY_MID_BLOCKS: i32 = 3_072;
const HIERARCHY_ROOT_BLOCKS: i32 = 6_144;
const HIERARCHY_ROOT_LEVEL: u8 = 2;
const HIERARCHY_MID_LEVEL: u8 = 1;
const HIERARCHY_ROOT_KIND: u16 = 300;
const HIERARCHY_MID_KIND: u16 = 301;
const HIERARCHY_VERTICAL_FACET_KIND: u16 = 302;
const HIERARCHY_HORIZONTAL_FACET_KIND: u16 = 303;
const HIERARCHY_REGION_KIND: u16 = 304;

#[derive(Clone, Debug, Serialize)]
pub struct PhaseTwoFallbackReceipt {
    pub candidate_revision: &'static str,
    pub research_only: bool,
    pub geography_quality_measured: bool,
    pub dependency_claim: HarnessDependencyClaim,
    pub comparisons: Vec<HarnessComparisonReceipt>,
    pub comparison_corpus_sha256: String,
    pub exact_comparison_count: u32,
    pub suite_passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct StreamedPlanPhaseTwoReceipt {
    pub receipt_schema: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub candidate_d_implemented: bool,
    pub reconstruction_measured: bool,
    pub cost_timing_measured: bool,
    pub fallback: PhaseTwoFallbackReceipt,
    pub hierarchical: HierarchicalTrialReceipt,
    pub feature_graph: FeatureGraphTrialReceipt,
    pub exact_comparison_count: u32,
    pub phase_two_witness_sha256: String,
    pub suite_passed: bool,
    pub artifacts: Vec<String>,
}

pub fn run_streamed_plan_phase_two_suite() -> Result<StreamedPlanPhaseTwoReceipt, String> {
    let fallback_comparisons = run_candidate_invariance_suite::<CoordinatePureControl>()?;
    let fallback = PhaseTwoFallbackReceipt {
        candidate_revision: FALLBACK_CONTROL_REVISION,
        research_only: true,
        geography_quality_measured: false,
        dependency_claim: HarnessDependencyClaim {
            control: FALLBACK_CONTROL_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 1,
            maximum_owner_radius_blocks: 8,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "coordinate-pure samples plus one bounded canonical feature owner",
        },
        comparison_corpus_sha256: comparison_corpus_sha256(&fallback_comparisons),
        exact_comparison_count: fallback_comparisons.len() as u32,
        suite_passed: fallback_comparisons
            .iter()
            .all(|comparison| comparison.passed),
        comparisons: fallback_comparisons,
    };
    let hierarchical = run_hierarchical_trial()?;
    let feature_graph = run_feature_graph_trial()?;
    let exact_comparison_count = fallback
        .exact_comparison_count
        .saturating_add(hierarchical.exact_comparison_count)
        .saturating_add(feature_graph.exact_comparison_count);
    let phase_two_witness_sha256 =
        phase_two_witness_sha256(&fallback, &hierarchical, &feature_graph);
    let suite_passed =
        fallback.suite_passed && hierarchical.suite_passed && feature_graph.suite_passed;
    Ok(StreamedPlanPhaseTwoReceipt {
        receipt_schema: STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        candidate_d_implemented: false,
        reconstruction_measured: false,
        cost_timing_measured: false,
        fallback,
        hierarchical,
        feature_graph,
        exact_comparison_count,
        phase_two_witness_sha256,
        suite_passed,
        artifacts: Vec::new(),
    })
}

fn phase_two_witness_sha256(
    fallback: &PhaseTwoFallbackReceipt,
    hierarchical: &HierarchicalTrialReceipt,
    feature_graph: &FeatureGraphTrialReceipt,
) -> String {
    let mut bytes = Vec::new();
    write_string(&mut bytes, STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION);
    write_string(&mut bytes, fallback.candidate_revision);
    write_string(&mut bytes, &fallback.comparison_corpus_sha256);
    write_u32(&mut bytes, fallback.exact_comparison_count);
    bytes.push(fallback.suite_passed as u8);

    write_string(&mut bytes, hierarchical.candidate_revision);
    write_string(&mut bytes, &hierarchical.comparison_corpus_sha256);
    write_u32(&mut bytes, hierarchical.exact_comparison_count);
    for boundary in &hierarchical.boundaries {
        write_i64(&mut bytes, boundary.seed);
        bytes.push(topology_tag(boundary.topology));
        write_u32(&mut bytes, boundary.expected_shared_facets);
        write_u32(&mut bytes, boundary.observed_shared_facets);
        write_u32(&mut bytes, boundary.exact_mismatch_count);
        write_u32(&mut bytes, boundary.maximum_observers_per_fact);
        bytes.push(boundary.passed as u8);
    }
    bytes.push(hierarchical.suite_passed as u8);

    write_string(&mut bytes, feature_graph.candidate_revision);
    write_string(&mut bytes, &feature_graph.comparison_corpus_sha256);
    write_u32(&mut bytes, feature_graph.exact_comparison_count);
    write_u32(
        &mut bytes,
        feature_graph.maximum_feature_reach_blocks as u32,
    );
    for corpus in &feature_graph.corpora {
        write_i64(&mut bytes, corpus.seed);
        bytes.push(topology_tag(corpus.topology));
        write_u32(&mut bytes, corpus.unique_graph_count);
        write_u32(&mut bytes, corpus.crossing_graph_count);
        write_u32(&mut bytes, corpus.seam_crossing_graph_count);
        write_u32(&mut bytes, corpus.explicit_sink_count);
        write_u32(&mut bytes, corpus.cycle_count);
        write_u32(&mut bytes, corpus.incomplete_graph_count);
        write_u32(&mut bytes, corpus.exact_observation_mismatch_count);
        write_u32(&mut bytes, corpus.maximum_actual_reach_blocks);
        write_u32(&mut bytes, corpus.maximum_owner_cells_examined);
        write_u32(&mut bytes, corpus.maximum_facts_per_plan);
        bytes.push(corpus.passed as u8);
    }
    bytes.push(feature_graph.suite_passed as u8);
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(bytes, value.len() as u32);
    bytes.extend_from_slice(value.as_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn topology_tag(topology: StreamedPlanTopology) -> u8 {
    match topology {
        StreamedPlanTopology::Plane => 0,
        StreamedPlanTopology::CylinderX => 1,
        StreamedPlanTopology::Torus => 2,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HierarchicalBoundaryReceipt {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub expected_shared_facets: u32,
    pub observed_shared_facets: u32,
    pub exact_mismatch_count: u32,
    pub first_mismatch: Option<String>,
    pub maximum_observers_per_fact: u32,
    pub passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HierarchicalTrialReceipt {
    pub receipt_schema: &'static str,
    pub candidate_revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub semantic_only: bool,
    pub geography_quality_measured: bool,
    pub reconstruction_measured: bool,
    pub cost_timing_measured: bool,
    pub hierarchy_blocks: [i32; 3],
    pub facts_per_base_plan: u32,
    pub dependency_claim: HarnessDependencyClaim,
    pub comparisons: Vec<HarnessComparisonReceipt>,
    pub comparison_corpus_sha256: String,
    pub boundaries: Vec<HierarchicalBoundaryReceipt>,
    pub exact_comparison_count: u32,
    pub suite_passed: bool,
}

pub fn run_hierarchical_trial() -> Result<HierarchicalTrialReceipt, String> {
    let comparisons = run_candidate_invariance_suite::<HierarchicalSharedFactsControl>()?;
    let mut boundaries = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            boundaries.push(inspect_hierarchical_boundaries(seed, topology)?);
        }
    }
    let suite_passed = comparisons.iter().all(|comparison| comparison.passed)
        && boundaries.iter().all(|boundary| boundary.passed);
    Ok(HierarchicalTrialReceipt {
        receipt_schema: STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION,
        candidate_revision: HIERARCHICAL_CANDIDATE_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        semantic_only: true,
        geography_quality_measured: false,
        reconstruction_measured: false,
        cost_timing_measured: false,
        hierarchy_blocks: [
            STREAMED_PLAN_BASE_REGION_BLOCKS,
            HIERARCHY_MID_BLOCKS,
            HIERARCHY_ROOT_BLOCKS,
        ],
        facts_per_base_plan: 7,
        dependency_claim: HarnessDependencyClaim {
            control: HIERARCHICAL_CANDIDATE_REVISION,
            maximum_depth: 3,
            maximum_provider_fanout: 6,
            maximum_owner_radius_blocks: 0,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "one fixed root, one fixed mid provider, and four canonical facets",
        },
        comparison_corpus_sha256: comparison_corpus_sha256(&comparisons),
        exact_comparison_count: comparisons.len() as u32,
        comparisons,
        boundaries,
        suite_passed,
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct HierarchicalSharedFactsControl;

impl StreamedPlanControl for HierarchicalSharedFactsControl {
    fn candidate_revision(&self) -> &'static str {
        HIERARCHICAL_CANDIDATE_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        let key = descriptor.plan_key(self.candidate_revision(), request.requested_region)?;
        let region = key.region;
        let center_x = region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2;
        let center_z = region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2;
        let root = hierarchy_provider_fact(
            descriptor,
            center_x,
            center_z,
            HIERARCHY_ROOT_LEVEL,
            HIERARCHY_ROOT_BLOCKS,
            HIERARCHY_ROOT_KIND,
        )?;
        let mid = hierarchy_provider_fact(
            descriptor,
            center_x,
            center_z,
            HIERARCHY_MID_LEVEL,
            HIERARCHY_MID_BLOCKS,
            HIERARCHY_MID_KIND,
        )?;
        let west = hierarchy_facet_fact(
            descriptor,
            region.min_block_x(),
            center_z,
            HIERARCHY_VERTICAL_FACET_KIND,
        )?;
        let east = hierarchy_facet_fact(
            descriptor,
            region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS,
            center_z,
            HIERARCHY_VERTICAL_FACET_KIND,
        )?;
        let north = hierarchy_facet_fact(
            descriptor,
            center_x,
            region.min_block_z(),
            HIERARCHY_HORIZONTAL_FACET_KIND,
        )?;
        let south = hierarchy_facet_fact(
            descriptor,
            center_x,
            region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS,
            HIERARCHY_HORIZONTAL_FACET_KIND,
        )?;
        let port_mask = (west.payload[0] != 0) as i64
            | (((east.payload[0] != 0) as i64) << 1)
            | (((north.payload[0] != 0) as i64) << 2)
            | (((south.payload[0] != 0) as i64) << 3);
        let region_hash = typed_hash(
            descriptor.seed,
            HIERARCHY_REGION_KIND,
            i64::from(region.x),
            i64::from(region.z),
        );
        let region_fact = StreamedSemanticFact {
            id: StreamedFactId {
                owner: key.clone(),
                kind: HIERARCHY_REGION_KIND,
                local_index: 0,
            },
            world_x: center_x,
            world_z: center_z,
            payload: vec![
                root.payload[0],
                mid.payload[0],
                port_mask,
                (region_hash % 4) as i64,
            ],
        };
        StreamedPlanSnapshot::new(key, vec![root, mid, west, east, north, south, region_fact])
    }
}

fn hierarchy_provider_fact(
    descriptor: &StreamedPlanDescriptor,
    world_x: i32,
    world_z: i32,
    level: u8,
    extent_blocks: i32,
    kind: u16,
) -> Result<StreamedSemanticFact, String> {
    let canonical = canonical_block(descriptor, world_x, world_z)?;
    let coordinate = PlanRegion::new(
        canonical.x.div_euclid(extent_blocks),
        canonical.z.div_euclid(extent_blocks),
    );
    let mut owner = descriptor.plan_key(
        HIERARCHICAL_CANDIDATE_REVISION,
        PlanRegion::new(
            coordinate.x * extent_blocks / STREAMED_PLAN_BASE_REGION_BLOCKS,
            coordinate.z * extent_blocks / STREAMED_PLAN_BASE_REGION_BLOCKS,
        ),
    )?;
    owner.level = level;
    owner.region = coordinate;
    let hash = typed_hash(
        descriptor.seed,
        kind,
        i64::from(coordinate.x),
        i64::from(coordinate.z),
    );
    let center_x = coordinate.x * extent_blocks + extent_blocks / 2;
    let center_z = coordinate.z * extent_blocks + extent_blocks / 2;
    let center = canonical_block(descriptor, center_x, center_z)?;
    Ok(StreamedSemanticFact {
        id: StreamedFactId {
            owner,
            kind,
            local_index: 0,
        },
        world_x: center.x,
        world_z: center.z,
        payload: vec![
            signed_bucket(hash, 2_048),
            ((hash >> 12) % 4) as i64,
            ((hash >> 20) % 8) as i64,
            ((hash >> 28) % 8) as i64,
        ],
    })
}

fn hierarchy_facet_fact(
    descriptor: &StreamedPlanDescriptor,
    world_x: i32,
    world_z: i32,
    kind: u16,
) -> Result<StreamedSemanticFact, String> {
    let canonical = canonical_block(descriptor, world_x, world_z)?;
    let facet_coordinate = PlanRegion::new(
        canonical.x.div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
        canonical.z.div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
    );
    let owner = descriptor.plan_key(HIERARCHICAL_CANDIDATE_REVISION, facet_coordinate)?;
    let root = hierarchy_provider_fact(
        descriptor,
        canonical.x,
        canonical.z,
        HIERARCHY_ROOT_LEVEL,
        HIERARCHY_ROOT_BLOCKS,
        HIERARCHY_ROOT_KIND,
    )?;
    let mid = hierarchy_provider_fact(
        descriptor,
        canonical.x,
        canonical.z,
        HIERARCHY_MID_LEVEL,
        HIERARCHY_MID_BLOCKS,
        HIERARCHY_MID_KIND,
    )?;
    let hash = typed_hash(
        descriptor.seed,
        kind,
        i64::from(facet_coordinate.x),
        i64::from(facet_coordinate.z),
    );
    let port_open = ((hash ^ root.payload[0] as u64 ^ mid.payload[0] as u64) % 5 < 2) as i64;
    Ok(StreamedSemanticFact {
        id: StreamedFactId {
            owner,
            kind,
            local_index: 0,
        },
        world_x: canonical.x,
        world_z: canonical.z,
        payload: vec![
            port_open,
            (root.payload[0] + mid.payload[0]) / 2,
            if hash & 1 == 0 { -1 } else { 1 },
            1 + ((hash >> 8) % 4) as i64,
        ],
    })
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

fn inspect_hierarchical_boundaries(
    seed: i64,
    topology: StreamedPlanTopology,
) -> Result<HierarchicalBoundaryReceipt, String> {
    let descriptor = StreamedPlanDescriptor::new(seed, topology);
    let targets = canonical_targets(topology);
    let run = execute_harness_case(
        &descriptor,
        HierarchicalSharedFactsControl,
        HarnessRunSpec::in_request_order(
            "boundary-observations",
            requests_with_center(&targets, PlanRegion::new(0, 0)),
        ),
    )?;
    let mut observations = BTreeMap::<StreamedFactId, (StreamedSemanticFact, u32, bool)>::new();
    for snapshot in run.snapshots.values() {
        for fact in &snapshot.facts {
            if !matches!(
                fact.id.kind,
                HIERARCHY_VERTICAL_FACET_KIND | HIERARCHY_HORIZONTAL_FACET_KIND
            ) {
                continue;
            }
            let observation =
                observations
                    .entry(fact.id.clone())
                    .or_insert((fact.clone(), 0, false));
            observation.1 = observation.1.saturating_add(1);
            if observation.0 != *fact {
                observation.2 = true;
            }
        }
    }
    let observed_shared_facets = observations
        .values()
        .filter(|(_, observer_count, _)| *observer_count >= 2)
        .count() as u32;
    let maximum_observers_per_fact = observations
        .values()
        .map(|(_, observer_count, _)| *observer_count)
        .max()
        .unwrap_or(0);
    let mismatches = observations
        .iter()
        .filter(|(_, (_, _, mismatch))| *mismatch)
        .collect::<Vec<_>>();
    let exact_mismatch_count = mismatches.len() as u32;
    let first_mismatch = mismatches
        .first()
        .map(|(id, _)| format!("shared facet {id:?} had conflicting observations"));
    let expected_shared_facets = match topology {
        StreamedPlanTopology::Plane => 40,
        StreamedPlanTopology::CylinderX => 54,
        StreamedPlanTopology::Torus => 72,
    };
    Ok(HierarchicalBoundaryReceipt {
        seed,
        topology,
        expected_shared_facets,
        observed_shared_facets,
        exact_mismatch_count,
        first_mismatch,
        maximum_observers_per_fact,
        passed: observed_shared_facets == expected_shared_facets
            && exact_mismatch_count == 0
            && maximum_observers_per_fact == 2,
    })
}

pub(crate) fn run_candidate_invariance_suite<C>() -> Result<Vec<HarnessComparisonReceipt>, String>
where
    C: Default + StreamedPlanControl,
{
    let control = C::default();
    let control_revision = control.candidate_revision();
    let mut comparisons = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            let descriptor = StreamedPlanDescriptor::new(seed, topology);
            let targets = canonical_targets(topology);
            let baseline_requests = requests_with_center(&targets, PlanRegion::new(0, 0));
            let baseline = execute_harness_case(
                &descriptor,
                C::default(),
                HarnessRunSpec::in_request_order("row-major", baseline_requests.clone()),
            )?;
            let rebuilt = execute_harness_case(
                &descriptor,
                C::default(),
                HarnessRunSpec::in_request_order("cold-rebuild", baseline_requests.clone()),
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "rebuild/cold",
                control_revision,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &baseline,
                &rebuilt,
            ));

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
                    C::default(),
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
                    control_revision,
                    &descriptor,
                    HarnessComparisonExpectation::Equal,
                    &baseline,
                    &run,
                ));
            }

            let schedule_reversed = execute_harness_case(
                &descriptor,
                C::default(),
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
                control_revision,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &baseline,
                &schedule_reversed,
            ));

            let singleton_batches = execute_harness_case(
                &descriptor,
                C::default(),
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
                control_revision,
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
                C::default(),
                HarnessRunSpec::in_request_order("window-follows-target", recentered_requests),
            )?;
            comparisons.push(HarnessComparisonReceipt::compare(
                "window/viewport-center",
                control_revision,
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
                    C::default(),
                    HarnessRunSpec::in_request_order("periodic-lifts", lifted),
                )?;
                comparisons.push(HarnessComparisonReceipt::compare(
                    "topology/lift",
                    control_revision,
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
                C::default(),
                HarnessRunSpec {
                    batch_size: 1,
                    ..HarnessRunSpec::in_request_order("warm-cache", repeated_requests.clone())
                },
            )?;
            let evicted_cache = execute_harness_case(
                &descriptor,
                C::default(),
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
                control_revision,
                &descriptor,
                HarnessComparisonExpectation::Equal,
                &warm_cache,
                &evicted_cache,
            ));
        }
    }
    Ok(comparisons)
}

fn typed_hash(seed: i64, domain: u16, x: i64, z: i64) -> u64 {
    stable_mix64(
        seed as u64
            ^ u64::from(domain).rotate_left(7)
            ^ (x as u64).rotate_left(23)
            ^ (z as u64).rotate_left(47),
    )
}

fn signed_bucket(hash: u64, magnitude: u64) -> i64 {
    (hash % (magnitude * 2 + 1)) as i64 - magnitude as i64
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
    fn hierarchical_trial_is_exact_and_shares_every_expected_facet() {
        let receipt = run_hierarchical_trial().expect("hierarchical trial");
        assert!(receipt.suite_passed);
        assert_eq!(receipt.exact_comparison_count, 105);
        assert!(
            receipt
                .comparisons
                .iter()
                .all(|comparison| comparison.exact_mismatch_count == 0
                    && comparison.left_internal_conflict_count == 0
                    && comparison.right_internal_conflict_count == 0)
        );
        assert!(receipt.boundaries.iter().all(|boundary| {
            boundary.observed_shared_facets == boundary.expected_shared_facets
                && boundary.exact_mismatch_count == 0
                && boundary.maximum_observers_per_fact == 2
        }));
    }

    #[test]
    fn phase_two_suite_compares_both_candidates_with_the_fallback() {
        let receipt = run_streamed_plan_phase_two_suite().expect("Phase 2 suite");
        assert!(receipt.suite_passed);
        assert_eq!(receipt.exact_comparison_count, 315);
        assert_eq!(
            receipt.phase_two_witness_sha256,
            STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256
        );
        assert!(!receipt.candidate_d_implemented);
        assert!(!receipt.reconstruction_measured);
        assert!(!receipt.cost_timing_measured);
    }
}
