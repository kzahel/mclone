//! Candidate C: feature-owned deterministic bounded graph trial.
//!
//! Each canonical owner cell constructs one complete short graph ending at
//! an explicit sink. A target enumerates a fixed owner neighborhood and
//! observes every overlapping graph. Graphs never discover or mutate one
//! another.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::BlockPos;
use serde::Serialize;

use crate::streamed_plan_harness::{
    HarnessComparisonReceipt, HarnessDependencyClaim, HarnessRunSpec, PlanRegion,
    STREAMED_PLAN_BASE_REGION_BLOCKS, STREAMED_PLAN_PHASE_ONE_SEEDS, StreamedFactId,
    StreamedPlanControl, StreamedPlanDescriptor, StreamedPlanKey, StreamedPlanRequest,
    StreamedPlanSnapshot, StreamedPlanTopology, StreamedSemanticFact, canonical_targets,
    comparison_corpus_sha256, execute_harness_case, requests_with_center,
};
use crate::streamed_plan_trials::{
    STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION, run_candidate_invariance_suite,
};

pub const FEATURE_GRAPH_CANDIDATE_REVISION: &str = "feature-owned-bounded-graph-trial-v1";
pub const FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS: i32 = 1_536;

const GRAPH_HEADER_KIND: u16 = 400;
const GRAPH_NODE_KIND: u16 = 401;
const GRAPH_EDGE_KIND: u16 = 402;
const GRAPH_SINK_KIND: u16 = 403;
const GRAPH_QUERY_KIND: u16 = 404;
const GRAPH_NODE_COUNT: usize = 4;
const GRAPH_EDGE_COUNT: usize = GRAPH_NODE_COUNT - 1;
const GRAPH_STEP_BLOCKS: i32 = 192;
const GRAPH_ANCHOR_JITTER_BLOCKS: i32 = 192;

#[derive(Clone, Debug, Serialize)]
pub struct FeatureGraphCorpusReceipt {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub unique_graph_count: u32,
    pub crossing_graph_count: u32,
    pub seam_crossing_graph_count: u32,
    pub explicit_sink_count: u32,
    pub cycle_count: u32,
    pub incomplete_graph_count: u32,
    pub exact_observation_mismatch_count: u32,
    pub maximum_actual_reach_blocks: u32,
    pub maximum_owner_cells_examined: u32,
    pub maximum_facts_per_plan: u32,
    pub first_failure: Option<String>,
    pub passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FeatureGraphTrialReceipt {
    pub receipt_schema: &'static str,
    pub candidate_revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub semantic_only: bool,
    pub geography_quality_measured: bool,
    pub reconstruction_measured: bool,
    pub cost_timing_measured: bool,
    pub maximum_feature_reach_blocks: i32,
    pub graph_node_count: u32,
    pub graph_edge_count: u32,
    pub dependency_claim: HarnessDependencyClaim,
    pub comparisons: Vec<HarnessComparisonReceipt>,
    pub comparison_corpus_sha256: String,
    pub corpora: Vec<FeatureGraphCorpusReceipt>,
    pub exact_comparison_count: u32,
    pub suite_passed: bool,
}

pub fn run_feature_graph_trial() -> Result<FeatureGraphTrialReceipt, String> {
    let comparisons = run_candidate_invariance_suite::<FeatureOwnedGraphControl>()?;
    let mut corpora = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            corpora.push(inspect_feature_graph_corpus(seed, topology)?);
        }
    }
    let suite_passed = comparisons.iter().all(|comparison| comparison.passed)
        && corpora.iter().all(|corpus| corpus.passed);
    Ok(FeatureGraphTrialReceipt {
        receipt_schema: STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION,
        candidate_revision: FEATURE_GRAPH_CANDIDATE_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        semantic_only: true,
        geography_quality_measured: false,
        reconstruction_measured: false,
        cost_timing_measured: false,
        maximum_feature_reach_blocks: FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS,
        graph_node_count: GRAPH_NODE_COUNT as u32,
        graph_edge_count: GRAPH_EDGE_COUNT as u32,
        dependency_claim: HarnessDependencyClaim {
            control: FEATURE_GRAPH_CANDIDATE_REVISION,
            maximum_depth: 1,
            maximum_provider_fanout: 25,
            maximum_owner_radius_blocks: FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS as u32,
            maximum_point_lookup_traversal_steps: 0,
            undeclared_fetch_count: 0,
            note: "fixed owner cells in target bounds expanded by declared reach",
        },
        comparison_corpus_sha256: comparison_corpus_sha256(&comparisons),
        exact_comparison_count: comparisons.len() as u32,
        comparisons,
        corpora,
        suite_passed,
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FeatureOwnedGraphControl;

impl StreamedPlanControl for FeatureOwnedGraphControl {
    fn candidate_revision(&self) -> &'static str {
        FEATURE_GRAPH_CANDIDATE_REVISION
    }

    fn construct(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        request: StreamedPlanRequest,
    ) -> Result<StreamedPlanSnapshot, String> {
        let key = descriptor.plan_key(self.candidate_revision(), request.requested_region)?;
        let owner_regions = possible_owner_regions(descriptor, key.region)?;
        let owner_cells_examined = owner_regions.len() as i64;
        let mut accepted_graphs = 0_i64;
        let mut facts = Vec::new();
        for owner_region in owner_regions {
            let graph = OwnedFeatureGraph::build(descriptor, owner_region)?;
            if graph.overlaps_target(descriptor, key.region) {
                accepted_graphs += 1;
                facts.extend(graph.facts);
            }
        }
        facts.push(StreamedSemanticFact {
            id: StreamedFactId {
                owner: key.clone(),
                kind: GRAPH_QUERY_KIND,
                local_index: 0,
            },
            world_x: key.region.min_block_x() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            world_z: key.region.min_block_z() + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
            payload: vec![owner_cells_examined, accepted_graphs, facts.len() as i64],
        });
        StreamedPlanSnapshot::new(key, facts)
    }
}

#[derive(Clone, Debug)]
struct OwnedFeatureGraph {
    canonical_anchor: BlockPos,
    minimum_offset_x: i32,
    maximum_offset_x: i32,
    minimum_offset_z: i32,
    maximum_offset_z: i32,
    facts: Vec<StreamedSemanticFact>,
}

impl OwnedFeatureGraph {
    fn build(
        descriptor: &StreamedPlanDescriptor,
        owner_region: PlanRegion,
    ) -> Result<Self, String> {
        let owner = descriptor.plan_key(FEATURE_GRAPH_CANDIDATE_REVISION, owner_region)?;
        let seed_hash = typed_hash(
            descriptor.seed,
            GRAPH_HEADER_KIND,
            i64::from(owner.region.x),
            i64::from(owner.region.z),
        );
        let raw_anchor_x = owner.region.min_block_x()
            + STREAMED_PLAN_BASE_REGION_BLOCKS / 2
            + signed_bucket(seed_hash, GRAPH_ANCHOR_JITTER_BLOCKS);
        let raw_anchor_z = owner.region.min_block_z()
            + STREAMED_PLAN_BASE_REGION_BLOCKS / 2
            + signed_bucket(seed_hash.rotate_left(29), GRAPH_ANCHOR_JITTER_BLOCKS);
        let canonical_anchor = canonical_block(descriptor, raw_anchor_x, raw_anchor_z)?;
        let mut direction_index = (seed_hash % 8) as i32;
        let mut offsets = Vec::with_capacity(GRAPH_NODE_COUNT);
        offsets.push((0_i32, 0_i32));
        for step in 0..GRAPH_EDGE_COUNT {
            let turn = match (seed_hash >> (8 + step * 2)) & 3 {
                0 => -1,
                1 | 2 => 0,
                _ => 1,
            };
            direction_index = (direction_index + turn).rem_euclid(8);
            let (direction_x, direction_z) = DIRECTIONS[direction_index as usize];
            let previous = offsets[step];
            offsets.push((
                previous.0 + direction_x * GRAPH_STEP_BLOCKS,
                previous.1 + direction_z * GRAPH_STEP_BLOCKS,
            ));
        }
        let minimum_offset_x = offsets.iter().map(|offset| offset.0).min().unwrap_or(0);
        let maximum_offset_x = offsets.iter().map(|offset| offset.0).max().unwrap_or(0);
        let minimum_offset_z = offsets.iter().map(|offset| offset.1).min().unwrap_or(0);
        let maximum_offset_z = offsets.iter().map(|offset| offset.1).max().unwrap_or(0);
        let maximum_reach = offsets
            .iter()
            .map(|(x, z)| x.unsigned_abs().max(z.unsigned_abs()))
            .max()
            .unwrap_or(0);
        if maximum_reach >= FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS as u32 {
            return Err(format!(
                "owned graph {:?} reaches {} blocks, limit is strictly below {}",
                owner, maximum_reach, FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS
            ));
        }

        let mut facts = Vec::with_capacity(1 + GRAPH_NODE_COUNT + GRAPH_EDGE_COUNT + 1);
        facts.push(StreamedSemanticFact {
            id: fact_id(&owner, GRAPH_HEADER_KIND, 0),
            world_x: canonical_anchor.x,
            world_z: canonical_anchor.z,
            payload: vec![
                i64::from(minimum_offset_x),
                i64::from(maximum_offset_x),
                i64::from(minimum_offset_z),
                i64::from(maximum_offset_z),
                i64::from(maximum_reach),
                GRAPH_NODE_COUNT as i64,
                GRAPH_EDGE_COUNT as i64,
            ],
        });
        let base_potential = 4_096 + (seed_hash % 256) as i64;
        for (index, (offset_x, offset_z)) in offsets.iter().copied().enumerate() {
            let node = canonical_block(
                descriptor,
                canonical_anchor.x + offset_x,
                canonical_anchor.z + offset_z,
            )?;
            facts.push(StreamedSemanticFact {
                id: fact_id(&owner, GRAPH_NODE_KIND, index as u32),
                world_x: node.x,
                world_z: node.z,
                payload: vec![
                    i64::from(offset_x),
                    i64::from(offset_z),
                    base_potential - index as i64 * 512,
                ],
            });
        }
        for edge_index in 0..GRAPH_EDGE_COUNT {
            let (offset_x, offset_z) = offsets[edge_index];
            let edge_start = canonical_block(
                descriptor,
                canonical_anchor.x + offset_x,
                canonical_anchor.z + offset_z,
            )?;
            facts.push(StreamedSemanticFact {
                id: fact_id(&owner, GRAPH_EDGE_KIND, edge_index as u32),
                world_x: edge_start.x,
                world_z: edge_start.z,
                payload: vec![
                    edge_index as i64,
                    edge_index as i64 + 1,
                    2 + ((seed_hash >> (32 + edge_index * 3)) % 5) as i64,
                    base_potential - edge_index as i64 * 512,
                    base_potential - (edge_index as i64 + 1) * 512,
                ],
            });
        }
        let sink_offset = offsets[GRAPH_NODE_COUNT - 1];
        let sink = canonical_block(
            descriptor,
            canonical_anchor.x + sink_offset.0,
            canonical_anchor.z + sink_offset.1,
        )?;
        facts.push(StreamedSemanticFact {
            id: fact_id(&owner, GRAPH_SINK_KIND, 0),
            world_x: sink.x,
            world_z: sink.z,
            payload: vec![
                (GRAPH_NODE_COUNT - 1) as i64,
                ((seed_hash >> 52) % 4) as i64,
                base_potential - (GRAPH_NODE_COUNT as i64 - 1) * 512,
            ],
        });
        Ok(Self {
            canonical_anchor,
            minimum_offset_x,
            maximum_offset_x,
            minimum_offset_z,
            maximum_offset_z,
            facts,
        })
    }

    fn overlaps_target(&self, descriptor: &StreamedPlanDescriptor, target: PlanRegion) -> bool {
        let target_min_x = i64::from(target.min_block_x());
        let target_min_z = i64::from(target.min_block_z());
        let target_max_x = target_min_x + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
        let target_max_z = target_min_z + i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
        let observer_x = target_min_x as f64 + f64::from(STREAMED_PLAN_BASE_REGION_BLOCKS) * 0.5;
        let observer_z = target_min_z as f64 + f64::from(STREAMED_PLAN_BASE_REGION_BLOCKS) * 0.5;
        let topology = descriptor.topology.horizontal();
        let anchor_x = topology
            .x
            .nearest_position_lift(f64::from(self.canonical_anchor.x), observer_x)
            .round() as i64;
        let anchor_z = topology
            .z
            .nearest_position_lift(f64::from(self.canonical_anchor.z), observer_z)
            .round() as i64;
        let graph_min_x = anchor_x + i64::from(self.minimum_offset_x);
        let graph_max_x = anchor_x + i64::from(self.maximum_offset_x);
        let graph_min_z = anchor_z + i64::from(self.minimum_offset_z);
        let graph_max_z = anchor_z + i64::from(self.maximum_offset_z);
        graph_max_x >= target_min_x
            && graph_min_x < target_max_x
            && graph_max_z >= target_min_z
            && graph_min_z < target_max_z
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

fn possible_owner_regions(
    descriptor: &StreamedPlanDescriptor,
    target: PlanRegion,
) -> Result<Vec<PlanRegion>, String> {
    let target_min_x = target.min_block_x();
    let target_min_z = target.min_block_z();
    let minimum_owner_x = (target_min_x - FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS)
        .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let minimum_owner_z = (target_min_z - FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS)
        .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let maximum_owner_x =
        (target_min_x + STREAMED_PLAN_BASE_REGION_BLOCKS + FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS - 1)
            .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let maximum_owner_z =
        (target_min_z + STREAMED_PLAN_BASE_REGION_BLOCKS + FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS - 1)
            .div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let mut owners = BTreeSet::new();
    for z in minimum_owner_z..=maximum_owner_z {
        for x in minimum_owner_x..=maximum_owner_x {
            owners.insert(descriptor.canonical_region(PlanRegion::new(x, z))?);
        }
    }
    Ok(owners.into_iter().collect())
}

fn inspect_feature_graph_corpus(
    seed: i64,
    topology: StreamedPlanTopology,
) -> Result<FeatureGraphCorpusReceipt, String> {
    let descriptor = StreamedPlanDescriptor::new(seed, topology);
    let targets = canonical_targets(topology);
    let run = execute_harness_case(
        &descriptor,
        FeatureOwnedGraphControl,
        HarnessRunSpec::in_request_order(
            "feature-graph-corpus",
            requests_with_center(&targets, PlanRegion::new(0, 0)),
        ),
    )?;
    let mut facts = BTreeMap::<StreamedFactId, StreamedSemanticFact>::new();
    let mut observers = BTreeMap::<StreamedPlanKey, BTreeSet<PlanRegion>>::new();
    let mut mismatch_count = 0_u32;
    let mut first_failure = None;
    let mut maximum_owner_cells_examined = 0_u32;
    let mut maximum_facts_per_plan = 0_u32;
    for snapshot in run.snapshots.values() {
        maximum_facts_per_plan = maximum_facts_per_plan.max(snapshot.facts.len() as u32);
        for fact in &snapshot.facts {
            if fact.id.kind == GRAPH_QUERY_KIND {
                maximum_owner_cells_examined =
                    maximum_owner_cells_examined.max(fact.payload[0] as u32);
                continue;
            }
            observers
                .entry(fact.id.owner.clone())
                .or_default()
                .insert(snapshot.key.region);
            if let Some(existing) = facts.insert(fact.id.clone(), fact.clone())
                && existing != *fact
            {
                mismatch_count = mismatch_count.saturating_add(1);
                first_failure.get_or_insert_with(|| {
                    format!("fact {:?} differed across target observations", fact.id)
                });
            }
        }
    }

    let mut by_owner = BTreeMap::<StreamedPlanKey, Vec<&StreamedSemanticFact>>::new();
    for fact in facts.values() {
        by_owner
            .entry(fact.id.owner.clone())
            .or_default()
            .push(fact);
    }
    let unique_graph_count = by_owner.len() as u32;
    let crossing_graph_count = observers
        .values()
        .filter(|targets| targets.len() > 1)
        .count() as u32;
    let seam_crossing_graph_count = observers
        .values()
        .filter(|targets| graph_crosses_periodic_seam(topology, targets))
        .count() as u32;
    let mut explicit_sink_count = 0_u32;
    let mut cycle_count = 0_u32;
    let mut incomplete_graph_count = 0_u32;
    let mut maximum_actual_reach_blocks = 0_u32;
    for (owner, graph_facts) in &by_owner {
        let headers = graph_facts
            .iter()
            .filter(|fact| fact.id.kind == GRAPH_HEADER_KIND)
            .collect::<Vec<_>>();
        let nodes = graph_facts
            .iter()
            .filter(|fact| fact.id.kind == GRAPH_NODE_KIND)
            .collect::<Vec<_>>();
        let edges = graph_facts
            .iter()
            .filter(|fact| fact.id.kind == GRAPH_EDGE_KIND)
            .collect::<Vec<_>>();
        let sinks = graph_facts
            .iter()
            .filter(|fact| fact.id.kind == GRAPH_SINK_KIND)
            .collect::<Vec<_>>();
        explicit_sink_count = explicit_sink_count.saturating_add(sinks.len() as u32);
        if headers.len() != 1
            || nodes.len() != GRAPH_NODE_COUNT
            || edges.len() != GRAPH_EDGE_COUNT
            || sinks.len() != 1
        {
            incomplete_graph_count = incomplete_graph_count.saturating_add(1);
            first_failure.get_or_insert_with(|| format!("graph {owner:?} is incomplete"));
            continue;
        }
        maximum_actual_reach_blocks = maximum_actual_reach_blocks.max(headers[0].payload[4] as u32);
        for edge in edges {
            let from = edge.payload[0];
            let to = edge.payload[1];
            let from_potential = edge.payload[3];
            let to_potential = edge.payload[4];
            if to != from + 1 || to_potential >= from_potential {
                cycle_count = cycle_count.saturating_add(1);
                first_failure.get_or_insert_with(|| {
                    format!("graph {owner:?} has a non-descending or cyclic edge")
                });
            }
        }
    }
    let needs_seam_crossing = topology != StreamedPlanTopology::Plane;
    let passed = mismatch_count == 0
        && unique_graph_count > 0
        && crossing_graph_count > 0
        && (!needs_seam_crossing || seam_crossing_graph_count > 0)
        && explicit_sink_count == unique_graph_count
        && cycle_count == 0
        && incomplete_graph_count == 0
        && maximum_actual_reach_blocks < FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS as u32
        && maximum_owner_cells_examined <= 25;
    Ok(FeatureGraphCorpusReceipt {
        seed,
        topology,
        unique_graph_count,
        crossing_graph_count,
        seam_crossing_graph_count,
        explicit_sink_count,
        cycle_count,
        incomplete_graph_count,
        exact_observation_mismatch_count: mismatch_count,
        maximum_actual_reach_blocks,
        maximum_owner_cells_examined,
        maximum_facts_per_plan,
        first_failure,
        passed,
    })
}

fn graph_crosses_periodic_seam(
    topology: StreamedPlanTopology,
    targets: &BTreeSet<PlanRegion>,
) -> bool {
    match topology {
        StreamedPlanTopology::Plane => false,
        StreamedPlanTopology::CylinderX => targets.iter().any(|left| {
            left.x == 0 && targets.contains(&PlanRegion::new(5, left.z))
                || left.x == 5 && targets.contains(&PlanRegion::new(0, left.z))
        }),
        StreamedPlanTopology::Torus => {
            let crosses_x = targets.iter().any(|left| {
                left.x == 0 && targets.contains(&PlanRegion::new(5, left.z))
                    || left.x == 5 && targets.contains(&PlanRegion::new(0, left.z))
            });
            let crosses_z = targets.iter().any(|left| {
                left.z == 0 && targets.contains(&PlanRegion::new(left.x, 5))
                    || left.z == 5 && targets.contains(&PlanRegion::new(left.x, 0))
            });
            crosses_x || crosses_z
        }
    }
}

fn fact_id(owner: &StreamedPlanKey, kind: u16, local_index: u32) -> StreamedFactId {
    StreamedFactId {
        owner: owner.clone(),
        kind,
        local_index,
    }
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

fn signed_bucket(hash: u64, magnitude: i32) -> i32 {
    (hash % (u64::try_from(magnitude).unwrap_or(0) * 2 + 1)) as i32 - magnitude
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
    fn feature_graph_trial_is_exact_bounded_complete_and_seam_aware() {
        let receipt = run_feature_graph_trial().expect("feature graph trial");
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
        assert!(receipt.corpora.iter().all(|corpus| {
            corpus.unique_graph_count > 0
                && corpus.crossing_graph_count > 0
                && corpus.explicit_sink_count == corpus.unique_graph_count
                && corpus.cycle_count == 0
                && corpus.incomplete_graph_count == 0
                && corpus.exact_observation_mismatch_count == 0
                && corpus.maximum_actual_reach_blocks < FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS as u32
                && (corpus.topology == StreamedPlanTopology::Plane
                    || corpus.seam_crossing_graph_count > 0)
        }));
    }
}
