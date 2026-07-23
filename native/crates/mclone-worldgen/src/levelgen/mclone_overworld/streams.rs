use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{CHUNK_WIDTH, ChunkPos};

use crate::procedural_structure::{
    ProceduralStructureError, ProceduralStructurePiece, ProceduralStructureStart,
    StructureBoundingBox, StructurePlacement, StructurePlacementTopology, StructureStartCandidate,
    StructureStartKey,
};

use super::fields::{
    MCLONE_OVERWORLD_PERIOD_CHUNKS, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldMajorRiverGeometry,
    McloneOverworldSampler, McloneOverworldSamplingTopology,
};

pub const MCLONE_OVERWORLD_STREAM_STRUCTURE_TYPE: &str = "mclone:raised_stream";
pub const MCLONE_OVERWORLD_STREAM_PLACEMENT_SPACING_CHUNKS: u32 = 8;
pub const MCLONE_OVERWORLD_STREAM_PLACEMENT_SEPARATION_CHUNKS: u32 = 3;
pub const MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS: u8 = 8;
pub const MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS: u32 = 48;
pub const MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS: u32 = 96;
pub const MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS: i32 = 4;
pub const MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES: u32 = 4_096;

const STREAM_PLACEMENT_SALT: i32 = 1_901_147;
const SINK_SCAN_STEP_BLOCKS: i32 = 2;
const ROUTE_MAX_STEPS: usize = MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS as usize
    / MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS as usize;
const ROUTE_MIN_STEPS: usize = MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS as usize
    / MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS as usize;
const ROUTE_BEAM_WIDTH: usize = 20;
const STREAM_HALF_WIDTH_BLOCKS: f64 = 2.25;
const STREAM_SHOULDER_SAMPLE_BLOCKS: f64 = 5.5;
const STREAM_PLAN_ENVELOPE_BLOCKS: i32 = 18;
const STREAM_MAX_TOTAL_RISE_BLOCKS: i32 = 6;
const STREAM_FIRST_REACH_BLOCKS: u32 = 12;
const STREAM_REACH_RUN_BLOCKS: u32 = 16;
const STREAM_MAX_CUT_BLOCKS: i32 = 8;

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneOverworldStreamNode {
    pub x: i32,
    pub z: i32,
    pub base_surface_y: i32,
    pub water_y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McloneOverworldStreamPiece {
    Headwater {
        node: usize,
    },
    Reach {
        first_node: usize,
        last_node: usize,
    },
    Transition {
        upper_node: usize,
        lower_node: usize,
        drop_height: i32,
    },
    Confluence {
        node: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldStreamPlanMetrics {
    pub route_length_blocks: u32,
    pub expanded_nodes: u32,
    pub flat_reaches: u32,
    pub transitions: u32,
    pub total_rise_blocks: i32,
    pub maximum_cut_blocks: i32,
    pub maximum_required_fill_blocks: i32,
    pub minimum_bank_clearance_blocks: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McloneOverworldStreamPlan {
    pub structure: ProceduralStructureStart<McloneOverworldStreamPiece>,
    pub nodes: Vec<McloneOverworldStreamNode>,
    pub metrics: McloneOverworldStreamPlanMetrics,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldStreamColumnSample {
    pub distance: f64,
    pub station_blocks: f64,
    pub water_y: i32,
    pub half_width: f64,
    pub tangent_x: f64,
    pub tangent_z: f64,
    pub is_headwater: bool,
    pub is_confluence: bool,
    pub transition: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldStreamRejection {
    NoMajorRiverInStartChunk,
    NoBoundedRoute,
    RouteTooShort,
    RequiresTerrainFill,
    InsufficientBankClearance,
    ExcessiveCut,
    NoRaisedReach,
    StructureBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldStreamPlanAttempt {
    pub candidate: StructureStartCandidate,
    pub rejection: Option<McloneOverworldStreamRejection>,
    pub expanded_nodes: u32,
}

#[derive(Clone, Debug)]
pub struct McloneOverworldStreamPlanner {
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    sampler: McloneOverworldSampler,
    placement: StructurePlacement,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneOverworldStreamPlanCacheReport {
    pub requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub accepted_plans: usize,
    pub rejected_candidates: usize,
}

#[derive(Clone, Debug)]
pub struct McloneOverworldStreamPlanCache {
    planner: McloneOverworldStreamPlanner,
    plans: BTreeMap<StructureStartCandidate, Option<McloneOverworldStreamPlan>>,
    requests: u64,
    hits: u64,
}

impl McloneOverworldStreamPlanCache {
    pub fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self {
            planner: McloneOverworldStreamPlanner::new(seed, topology),
            plans: BTreeMap::new(),
            requests: 0,
            hits: 0,
        }
    }

    pub fn planner(&self) -> &McloneOverworldStreamPlanner {
        &self.planner
    }

    pub fn plan_start(
        &mut self,
        candidate: StructureStartCandidate,
    ) -> Result<Option<McloneOverworldStreamPlan>, ProceduralStructureError> {
        self.requests += 1;
        if let Some(plan) = self.plans.get(&candidate) {
            self.hits += 1;
            return Ok(plan.clone());
        }
        let plan = self.planner.plan_start(candidate)?;
        self.plans.insert(candidate, plan.clone());
        Ok(plan)
    }

    pub fn report(&self) -> McloneOverworldStreamPlanCacheReport {
        let accepted_plans = self.plans.values().filter(|plan| plan.is_some()).count();
        McloneOverworldStreamPlanCacheReport {
            requests: self.requests,
            hits: self.hits,
            misses: self.requests - self.hits,
            accepted_plans,
            rejected_candidates: self.plans.len() - accepted_plans,
        }
    }
}

impl McloneOverworldStreamPlanner {
    pub fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self {
            seed,
            topology,
            sampler: McloneOverworldSampler::new_with_topology(seed, topology),
            placement: stream_placement(),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn topology(&self) -> McloneOverworldSamplingTopology {
        self.topology
    }

    pub const fn placement(&self) -> StructurePlacement {
        self.placement
    }

    pub fn potential_start(
        &self,
        query: ChunkPos,
    ) -> Result<StructureStartCandidate, ProceduralStructureError> {
        self.placement
            .potential_start_chunk(self.seed, query, placement_topology(self.topology))
    }

    pub fn plan_start(
        &self,
        candidate: StructureStartCandidate,
    ) -> Result<Option<McloneOverworldStreamPlan>, ProceduralStructureError> {
        Ok(self.plan_start_with_attempt(candidate)?.0)
    }

    pub fn plan_start_with_attempt(
        &self,
        candidate: StructureStartCandidate,
    ) -> Result<
        (
            Option<McloneOverworldStreamPlan>,
            McloneOverworldStreamPlanAttempt,
        ),
        ProceduralStructureError,
    > {
        let Some(sink) = self.find_sink(candidate.work_start) else {
            return Ok(attempt(
                candidate,
                McloneOverworldStreamRejection::NoMajorRiverInStartChunk,
                0,
            ));
        };
        let (route, expanded_nodes) = self.solve_route(sink);
        let Some(route) = route else {
            return Ok(attempt(
                candidate,
                McloneOverworldStreamRejection::NoBoundedRoute,
                expanded_nodes,
            ));
        };
        let route = trim_route_to_max_length(route);
        if route.len() < ROUTE_MIN_STEPS + 1
            || route_node_length_blocks(&route) < MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS
        {
            return Ok(attempt(
                candidate,
                McloneOverworldStreamRejection::RouteTooShort,
                expanded_nodes,
            ));
        }
        match self.finish_plan(candidate, route, expanded_nodes) {
            Ok(plan) => Ok((
                Some(plan),
                McloneOverworldStreamPlanAttempt {
                    candidate,
                    rejection: None,
                    expanded_nodes,
                },
            )),
            Err(rejection) => Ok(attempt(candidate, rejection, expanded_nodes)),
        }
    }

    pub fn plans_intersecting_chunks(
        &self,
        min_chunk: ChunkPos,
        max_chunk: ChunkPos,
    ) -> Result<Vec<McloneOverworldStreamPlan>, ProceduralStructureError> {
        let radius = i32::from(self.placement.reference_radius());
        let attempts = self.plan_candidates_in_chunks(
            ChunkPos::new(min_chunk.x - radius, min_chunk.z - radius),
            ChunkPos::new(max_chunk.x + radius, max_chunk.z + radius),
        )?;
        let mut plans = attempts
            .into_iter()
            .filter_map(|(plan, _)| plan)
            .collect::<Vec<_>>();
        plans.retain(|plan| {
            plan.structure.bounds.max_x >= min_chunk.min_block_x()
                && plan.structure.bounds.min_x <= max_chunk.min_block_x() + CHUNK_WIDTH - 1
                && plan.structure.bounds.max_z >= min_chunk.min_block_z()
                && plan.structure.bounds.min_z <= max_chunk.min_block_z() + CHUNK_WIDTH - 1
        });
        plans.sort_by(|left, right| left.structure.key.cmp(&right.structure.key));
        Ok(plans)
    }

    pub fn plan_candidates_in_chunks(
        &self,
        min_chunk: ChunkPos,
        max_chunk: ChunkPos,
    ) -> Result<
        Vec<(
            Option<McloneOverworldStreamPlan>,
            McloneOverworldStreamPlanAttempt,
        )>,
        ProceduralStructureError,
    > {
        let mut candidates = BTreeSet::new();
        for chunk_z in min_chunk.z..=max_chunk.z {
            for chunk_x in min_chunk.x..=max_chunk.x {
                let query = ChunkPos::new(chunk_x, chunk_z);
                let candidate = self.potential_start(query)?;
                if candidate.work_start == query {
                    candidates.insert(candidate);
                }
            }
        }
        candidates
            .into_iter()
            .map(|candidate| self.plan_start_with_attempt(candidate))
            .collect()
    }

    fn find_sink(&self, start: ChunkPos) -> Option<SinkSite> {
        let min_x = start.min_block_x();
        let min_z = start.min_block_z();
        let center_x = min_x + CHUNK_WIDTH / 2;
        let center_z = min_z + CHUNK_WIDTH / 2;
        let mut best = None;
        for z in (min_z..min_z + CHUNK_WIDTH).step_by(SINK_SCAN_STEP_BLOCKS as usize) {
            for x in (min_x..min_x + CHUNK_WIDTH).step_by(SINK_SCAN_STEP_BLOCKS as usize) {
                let terrain = self.sampler.sample(x, z);
                let geometry = self
                    .sampler
                    .sample_major_river_geometry(f64::from(x), f64::from(z));
                if terrain.continentalness <= 0.02 || geometry.distance > geometry.half_width - 0.75
                {
                    continue;
                }
                let score = (geometry.distance * 1_024.0).round() as i64
                    + i64::from((x - center_x).abs() + (z - center_z).abs()) * 8
                    + i64::from((terrain.base_surface_y - (MCLONE_OVERWORLD_SEA_LEVEL + 7)).abs());
                let site = SinkSite {
                    x: geometry.center_x.round() as i32,
                    z: geometry.center_z.round() as i32,
                    geometry,
                };
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score < *best_score)
                {
                    best = Some((score, site));
                }
            }
        }
        best.map(|(_, site)| site)
    }

    fn solve_route(&self, sink: SinkSite) -> (Option<Vec<RouteNode>>, u32) {
        let mut all = Vec::new();
        let mut expanded = 0;
        for side in [-1.0, 1.0] {
            if expanded >= MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES {
                break;
            }
            let origin_distance = sink.geometry.half_width + 4.0;
            let origin_x = (f64::from(sink.x) + sink.geometry.normal_x * side * origin_distance)
                .round() as i32;
            let origin_z = (f64::from(sink.z) + sink.geometry.normal_z * side * origin_distance)
                .round() as i32;
            let direction =
                nearest_direction(sink.geometry.normal_x * side, sink.geometry.normal_z * side);
            let terrain = self.sampler.sample(origin_x, origin_z);
            if terrain.continentalness <= 0.0 {
                continue;
            }
            let initial = RouteState {
                nodes: vec![
                    route_node(self.sampler, sink.x, sink.z),
                    route_node(self.sampler, origin_x, origin_z),
                ],
                direction,
                cost: 0,
            };
            let mut beam = vec![initial];
            for _ in 1..ROUTE_MAX_STEPS {
                let mut next = Vec::new();
                for state in std::mem::take(&mut beam) {
                    if expanded >= MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES {
                        break;
                    }
                    for turn in [-1, 0, 1] {
                        expanded += 1;
                        if expanded > MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES {
                            break;
                        }
                        let direction = (state.direction as i32 + turn).rem_euclid(8) as usize;
                        let (dx, dz) = DIRECTIONS[direction];
                        let previous = state.nodes.last().expect("route state has nodes");
                        let x = previous.x + dx * MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS;
                        let z = previous.z + dz * MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS;
                        if state.nodes.iter().any(|node| node.x == x && node.z == z) {
                            continue;
                        }
                        let node = route_node(self.sampler, x, z);
                        if node.continentalness <= 0.0 {
                            continue;
                        }
                        let river = self
                            .sampler
                            .sample_major_river_geometry(f64::from(x), f64::from(z));
                        let previous_river = self.sampler.sample_major_river_geometry(
                            f64::from(previous.x),
                            f64::from(previous.z),
                        );
                        if river.distance + 1.0 < previous_river.distance {
                            continue;
                        }
                        let step_cost = self.route_step_cost(&state, node, direction, turn);
                        let mut nodes = state.nodes.clone();
                        nodes.push(node);
                        next.push(RouteState {
                            nodes,
                            direction,
                            cost: state.cost.saturating_add(step_cost),
                        });
                    }
                }
                if next.is_empty() {
                    break;
                }
                next.sort_by_key(route_state_sort_key);
                next.truncate(ROUTE_BEAM_WIDTH);
                beam = next;
            }
            all.extend(beam);
        }
        all.into_iter()
            .filter(|state| state.nodes.len() >= ROUTE_MIN_STEPS + 1)
            .min_by_key(route_state_sort_key)
            .map_or((None, expanded), |state| (Some(state.nodes), expanded))
    }

    fn route_step_cost(
        &self,
        state: &RouteState,
        node: RouteNode,
        direction: usize,
        turn: i32,
    ) -> i64 {
        let previous = state.nodes.last().expect("route state has nodes");
        let height_delta = node.base_surface_y - previous.base_surface_y;
        let (dx, dz) = DIRECTIONS[direction];
        let length = f64::from(dx * dx + dz * dz).sqrt();
        let normal_x = -f64::from(dz) / length;
        let normal_z = f64::from(dx) / length;
        let left = self.sampler.sample(
            (f64::from(node.x) + normal_x * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
            (f64::from(node.z) + normal_z * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
        );
        let right = self.sampler.sample(
            (f64::from(node.x) - normal_x * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
            (f64::from(node.z) - normal_z * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
        );
        let valley_depth =
            ((left.base_surface_y + right.base_surface_y) / 2 - node.base_surface_y).clamp(-8, 8);
        let slope_cost = if height_delta > 3 {
            i64::from(height_delta - 3) * 600
        } else if height_delta < -2 {
            i64::from(-height_delta - 2) * 280
        } else {
            i64::from(height_delta.abs()) * 18
        };
        let high_cut_cost =
            i64::from((node.base_surface_y - (MCLONE_OVERWORLD_SEA_LEVEL + 8)).max(0)) * 22;
        let valley_credit = i64::from(valley_depth) * 34;
        let ridge_cost = (node.ridges * 180.0).round() as i64;
        let curvature_cost = i64::from(turn.abs()) * 22;
        let diagonal_cost = if dx != 0 && dz != 0 { 2 } else { 0 };
        200 + slope_cost + high_cut_cost + ridge_cost + curvature_cost + diagonal_cost
            - valley_credit
    }

    fn finish_plan(
        &self,
        candidate: StructureStartCandidate,
        route: Vec<RouteNode>,
        expanded_nodes: u32,
    ) -> Result<McloneOverworldStreamPlan, McloneOverworldStreamRejection> {
        let mut capacities = Vec::with_capacity(route.len());
        let mut maximum_cut = 0;
        let mut maximum_required_fill = 0;
        for (index, node) in route.iter().enumerate() {
            let (tangent_x, tangent_z) = route_tangent(&route, index);
            let left = self.sampler.sample(
                (f64::from(node.x) - tangent_z * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
                (f64::from(node.z) + tangent_x * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
            );
            let right = self.sampler.sample(
                (f64::from(node.x) + tangent_z * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
                (f64::from(node.z) - tangent_x * STREAM_SHOULDER_SAMPLE_BLOCKS).round() as i32,
            );
            capacities.push(left.base_surface_y.min(right.base_surface_y) - 1);
        }
        let mut suffix_capacity = vec![i32::MAX; route.len()];
        let mut minimum = i32::MAX;
        for index in (0..route.len()).rev() {
            minimum = minimum.min(capacities[index]);
            suffix_capacity[index] = minimum;
        }
        let mut sink_to_source = Vec::with_capacity(route.len());
        let mut previous_water_y = MCLONE_OVERWORLD_SEA_LEVEL;
        let mut minimum_clearance = i32::MAX;
        for (index, node) in route.into_iter().enumerate() {
            let station = u32::try_from(index).expect("route index fits u32")
                * MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS as u32;
            let target_rise =
                station.saturating_sub(STREAM_FIRST_REACH_BLOCKS) / STREAM_REACH_RUN_BLOCKS;
            let target_water_y = MCLONE_OVERWORLD_SEA_LEVEL
                + i32::try_from(target_rise)
                    .expect("bounded stream route rise fits i32")
                    .min(STREAM_MAX_TOTAL_RISE_BLOCKS);
            let water_y = target_water_y
                .min(suffix_capacity[index])
                .max(previous_water_y);
            if water_y > suffix_capacity[index] {
                return Err(McloneOverworldStreamRejection::InsufficientBankClearance);
            }
            let bed_y = water_y - 2;
            if node.base_surface_y < bed_y - 1 {
                return Err(McloneOverworldStreamRejection::RequiresTerrainFill);
            }
            maximum_required_fill = maximum_required_fill.max(bed_y - node.base_surface_y);
            let cut = node.base_surface_y - bed_y;
            maximum_cut = maximum_cut.max(cut);
            if cut > STREAM_MAX_CUT_BLOCKS {
                return Err(McloneOverworldStreamRejection::ExcessiveCut);
            }
            minimum_clearance = minimum_clearance.min(capacities[index] - water_y);
            sink_to_source.push(McloneOverworldStreamNode {
                x: node.x,
                z: node.z,
                base_surface_y: node.base_surface_y,
                water_y,
            });
            previous_water_y = water_y;
        }
        let total_rise =
            sink_to_source.last().expect("route has nodes").water_y - MCLONE_OVERWORLD_SEA_LEVEL;
        if total_rise < 2 {
            return Err(McloneOverworldStreamRejection::NoRaisedReach);
        }
        sink_to_source.reverse();
        let nodes = sink_to_source;
        let pieces = stream_pieces(&nodes)?;
        let structure = ProceduralStructureStart::new(
            StructureStartKey::new(
                MCLONE_OVERWORLD_STREAM_STRUCTURE_TYPE,
                candidate.canonical_start,
            ),
            candidate.work_start,
            pieces,
        )
        .map_err(|_| McloneOverworldStreamRejection::StructureBounds)?;
        let transitions = nodes
            .windows(2)
            .filter(|pair| pair[0].water_y != pair[1].water_y)
            .count() as u32;
        Ok(McloneOverworldStreamPlan {
            metrics: McloneOverworldStreamPlanMetrics {
                route_length_blocks: route_length_blocks(&nodes),
                expanded_nodes,
                flat_reaches: transitions + 1,
                transitions,
                total_rise_blocks: total_rise,
                maximum_cut_blocks: maximum_cut,
                maximum_required_fill_blocks: maximum_required_fill,
                minimum_bank_clearance_blocks: minimum_clearance,
            },
            structure,
            nodes,
        })
    }
}

impl McloneOverworldStreamPlan {
    pub fn sample_column(&self, world_x: i32, world_z: i32) -> McloneOverworldStreamColumnSample {
        let mut station = 0.0;
        let mut best = None;
        let mut traversed = 0.0;
        for (index, pair) in self.nodes.windows(2).enumerate() {
            let start = pair[0];
            let end = pair[1];
            let dx = f64::from(end.x - start.x);
            let dz = f64::from(end.z - start.z);
            let length_squared = dx * dx + dz * dz;
            let length = length_squared.sqrt();
            let projection = if length_squared == 0.0 {
                0.0
            } else {
                ((f64::from(world_x - start.x) * dx + f64::from(world_z - start.z) * dz)
                    / length_squared)
                    .clamp(0.0, 1.0)
            };
            let closest_x = f64::from(start.x) + dx * projection;
            let closest_z = f64::from(start.z) + dz * projection;
            let distance = (f64::from(world_x) - closest_x).hypot(f64::from(world_z) - closest_z);
            let candidate_station = traversed + length * projection;
            let transition = (start.water_y != end.water_y).then_some((
                start.water_y.max(end.water_y),
                start.water_y.min(end.water_y),
            ));
            let candidate = (
                distance,
                candidate_station,
                start.water_y,
                dx / length.max(1.0),
                dz / length.max(1.0),
                transition,
                index,
            );
            if best.as_ref().is_none_or(
                |current: &(f64, f64, i32, f64, f64, Option<(i32, i32)>, usize)| {
                    distance.total_cmp(&current.0).is_lt()
                },
            ) {
                best = Some(candidate);
                station = candidate_station;
            }
            traversed += length;
        }
        let (distance, _, water_y, tangent_x, tangent_z, transition, index) =
            best.expect("stream plan has at least two nodes");
        McloneOverworldStreamColumnSample {
            distance,
            station_blocks: station,
            water_y,
            half_width: STREAM_HALF_WIDTH_BLOCKS,
            tangent_x,
            tangent_z,
            is_headwater: index == 0 && station <= 7.0,
            is_confluence: index + 2 == self.nodes.len() && (traversed - station).max(0.0) <= 8.0,
            transition,
        }
    }
}

pub fn stream_placement() -> StructurePlacement {
    StructurePlacement::new(
        MCLONE_OVERWORLD_STREAM_PLACEMENT_SPACING_CHUNKS,
        MCLONE_OVERWORLD_STREAM_PLACEMENT_SEPARATION_CHUNKS,
        STREAM_PLACEMENT_SALT,
        MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS,
    )
    .expect("Mclone stream placement constants are valid")
}

fn placement_topology(topology: McloneOverworldSamplingTopology) -> StructurePlacementTopology {
    match topology {
        McloneOverworldSamplingTopology::Unbounded => StructurePlacementTopology::Unbounded,
        McloneOverworldSamplingTopology::PeriodicX => StructurePlacementTopology::PeriodicX {
            period_chunks: MCLONE_OVERWORLD_PERIOD_CHUNKS,
        },
    }
}

fn attempt(
    candidate: StructureStartCandidate,
    rejection: McloneOverworldStreamRejection,
    expanded_nodes: u32,
) -> (
    Option<McloneOverworldStreamPlan>,
    McloneOverworldStreamPlanAttempt,
) {
    (
        None,
        McloneOverworldStreamPlanAttempt {
            candidate,
            rejection: Some(rejection),
            expanded_nodes,
        },
    )
}

#[derive(Clone, Copy, Debug)]
struct SinkSite {
    x: i32,
    z: i32,
    geometry: McloneOverworldMajorRiverGeometry,
}

#[derive(Clone, Copy, Debug)]
struct RouteNode {
    x: i32,
    z: i32,
    base_surface_y: i32,
    continentalness: f64,
    ridges: f64,
}

#[derive(Clone, Debug)]
struct RouteState {
    nodes: Vec<RouteNode>,
    direction: usize,
    cost: i64,
}

fn route_node(sampler: McloneOverworldSampler, x: i32, z: i32) -> RouteNode {
    let sample = sampler.sample(x, z);
    RouteNode {
        x,
        z,
        base_surface_y: sample.base_surface_y,
        continentalness: sample.continentalness,
        ridges: sample.ridges,
    }
}

fn nearest_direction(x: f64, z: f64) -> usize {
    DIRECTIONS
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            (f64::from(left.0) * x + f64::from(left.1) * z)
                .total_cmp(&(f64::from(right.0) * x + f64::from(right.1) * z))
        })
        .map(|(index, _)| index)
        .expect("direction table is non-empty")
}

fn route_state_sort_key(state: &RouteState) -> (i64, i32, i32, usize) {
    let end = state.nodes.last().expect("route state has nodes");
    (state.cost, end.x, end.z, state.direction)
}

fn route_tangent(route: &[RouteNode], index: usize) -> (f64, f64) {
    let before = index.saturating_sub(1);
    let after = (index + 1).min(route.len() - 1);
    let dx = f64::from(route[after].x - route[before].x);
    let dz = f64::from(route[after].z - route[before].z);
    let length = dx.hypot(dz).max(1.0);
    (dx / length, dz / length)
}

fn route_length_blocks(nodes: &[McloneOverworldStreamNode]) -> u32 {
    nodes
        .windows(2)
        .map(|pair| {
            f64::from(pair[1].x - pair[0].x)
                .hypot(f64::from(pair[1].z - pair[0].z))
                .round() as u32
        })
        .sum()
}

fn route_node_length_blocks(nodes: &[RouteNode]) -> u32 {
    nodes
        .windows(2)
        .map(|pair| {
            f64::from(pair[1].x - pair[0].x)
                .hypot(f64::from(pair[1].z - pair[0].z))
                .round() as u32
        })
        .sum()
}

fn trim_route_to_max_length(route: Vec<RouteNode>) -> Vec<RouteNode> {
    let mut trimmed: Vec<RouteNode> = Vec::with_capacity(route.len());
    let mut length = 0;
    for node in route {
        if let Some(previous) = trimmed.last() {
            let segment = f64::from(node.x - previous.x)
                .hypot(f64::from(node.z - previous.z))
                .round() as u32;
            if length + segment > MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS {
                break;
            }
            length += segment;
        }
        trimmed.push(node);
    }
    trimmed
}

fn stream_pieces(
    nodes: &[McloneOverworldStreamNode],
) -> Result<Vec<ProceduralStructurePiece<McloneOverworldStreamPiece>>, McloneOverworldStreamRejection>
{
    let mut pieces = Vec::new();
    let mut ordinal = 0;
    pieces.push(piece(
        ordinal,
        "headwater",
        bounds_for_nodes(&nodes[0..=1], STREAM_PLAN_ENVELOPE_BLOCKS)?,
        McloneOverworldStreamPiece::Headwater { node: 0 },
    ));
    ordinal += 1;
    let mut first = 0;
    for index in 1..nodes.len() {
        if nodes[index].water_y == nodes[first].water_y {
            continue;
        }
        pieces.push(piece(
            ordinal,
            "reach",
            bounds_for_nodes(&nodes[first..=index], STREAM_PLAN_ENVELOPE_BLOCKS)?,
            McloneOverworldStreamPiece::Reach {
                first_node: first,
                last_node: index,
            },
        ));
        ordinal += 1;
        pieces.push(piece(
            ordinal,
            "transition",
            bounds_for_nodes(&nodes[index - 1..=index], STREAM_PLAN_ENVELOPE_BLOCKS)?,
            McloneOverworldStreamPiece::Transition {
                upper_node: index - 1,
                lower_node: index,
                drop_height: nodes[index - 1].water_y - nodes[index].water_y,
            },
        ));
        ordinal += 1;
        first = index;
    }
    pieces.push(piece(
        ordinal,
        "reach",
        bounds_for_nodes(&nodes[first..], STREAM_PLAN_ENVELOPE_BLOCKS)?,
        McloneOverworldStreamPiece::Reach {
            first_node: first,
            last_node: nodes.len() - 1,
        },
    ));
    ordinal += 1;
    pieces.push(piece(
        ordinal,
        "confluence",
        bounds_for_nodes(
            &nodes[nodes.len().saturating_sub(2)..],
            STREAM_PLAN_ENVELOPE_BLOCKS,
        )?,
        McloneOverworldStreamPiece::Confluence {
            node: nodes.len() - 1,
        },
    ));
    Ok(pieces)
}

fn piece(
    ordinal: u32,
    kind: &str,
    bounds: StructureBoundingBox,
    payload: McloneOverworldStreamPiece,
) -> ProceduralStructurePiece<McloneOverworldStreamPiece> {
    ProceduralStructurePiece::new(ordinal, kind, bounds, payload)
}

fn bounds_for_nodes(
    nodes: &[McloneOverworldStreamNode],
    envelope: i32,
) -> Result<StructureBoundingBox, McloneOverworldStreamRejection> {
    let min_x = nodes
        .iter()
        .map(|node| node.x)
        .min()
        .expect("piece nodes are non-empty");
    let max_x = nodes
        .iter()
        .map(|node| node.x)
        .max()
        .expect("piece nodes are non-empty");
    let min_z = nodes
        .iter()
        .map(|node| node.z)
        .min()
        .expect("piece nodes are non-empty");
    let max_z = nodes
        .iter()
        .map(|node| node.z)
        .max()
        .expect("piece nodes are non-empty");
    let min_y = nodes
        .iter()
        .map(|node| node.water_y - 8)
        .min()
        .expect("piece nodes are non-empty");
    let max_y = nodes
        .iter()
        .map(|node| node.base_surface_y + 4)
        .max()
        .expect("piece nodes are non-empty");
    StructureBoundingBox::new(
        min_x - envelope,
        min_y,
        min_z - envelope,
        max_x + envelope,
        max_y,
        max_z + envelope,
    )
    .map_err(|_| McloneOverworldStreamRejection::StructureBounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_is_periodic_compatible_and_vanilla_bounded() {
        let placement = stream_placement();
        assert_eq!(placement.reference_radius(), 8);
        assert!(
            placement
                .validate_topology(StructurePlacementTopology::PeriodicX {
                    period_chunks: MCLONE_OVERWORLD_PERIOD_CHUNKS,
                })
                .is_ok()
        );
    }

    #[test]
    fn accepted_plans_are_downhill_finite_and_bounded() {
        let planner =
            McloneOverworldStreamPlanner::new(-98_765, McloneOverworldSamplingTopology::Unbounded);
        let mut accepted = None;
        for z in -240..=-120 {
            for x in 120..=240 {
                let query = ChunkPos::new(x, z);
                let candidate = planner.potential_start(query).unwrap();
                if candidate.work_start != query {
                    continue;
                }
                if let Some(plan) = planner.plan_start(candidate).unwrap() {
                    accepted = Some(plan);
                    break;
                }
            }
            if accepted.is_some() {
                break;
            }
        }
        let plan = accepted.expect("review seed region should contain an accepted stream plan");
        assert!(
            (MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS..=MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS)
                .contains(&plan.metrics.route_length_blocks)
        );
        assert!(plan.metrics.expanded_nodes <= MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES);
        assert!(plan.metrics.total_rise_blocks >= 2);
        assert!(plan.metrics.maximum_cut_blocks <= STREAM_MAX_CUT_BLOCKS);
        assert!(plan.metrics.minimum_bank_clearance_blocks >= 0);
        assert!(
            plan.nodes
                .windows(2)
                .all(|pair| pair[0].water_y >= pair[1].water_y)
        );
        assert_eq!(
            plan.nodes.last().expect("plan has sink").water_y,
            MCLONE_OVERWORLD_SEA_LEVEL
        );
    }

    #[test]
    fn plan_reconstruction_is_order_independent() {
        let planner =
            McloneOverworldStreamPlanner::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let min = ChunkPos::new(-48, -48);
        let max = ChunkPos::new(48, 48);
        let forward = planner.plans_intersecting_chunks(min, max).unwrap();
        let reverse = planner
            .plans_intersecting_chunks(ChunkPos::new(min.x, min.z), ChunkPos::new(max.x, max.z));
        assert_eq!(forward, reverse.unwrap());
    }

    #[test]
    fn plan_cache_records_accepted_and_rejected_hits() {
        let mut cache = McloneOverworldStreamPlanCache::new(
            -98_765,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let mut accepted = None;
        let mut rejected = None;
        for z in -240..=-120 {
            for x in 120..=240 {
                let query = ChunkPos::new(x, z);
                let candidate = cache.planner().potential_start(query).unwrap();
                if candidate.work_start != query {
                    continue;
                }
                let plan = cache.plan_start(candidate).unwrap();
                if plan.is_some() {
                    accepted.get_or_insert(candidate);
                } else {
                    rejected.get_or_insert(candidate);
                }
                if accepted.is_some() && rejected.is_some() {
                    break;
                }
            }
            if accepted.is_some() && rejected.is_some() {
                break;
            }
        }
        cache
            .plan_start(accepted.expect("accepted candidate"))
            .unwrap();
        cache
            .plan_start(rejected.expect("rejected candidate"))
            .unwrap();
        let report = cache.report();
        assert_eq!(report.hits, 2);
        assert!(report.accepted_plans >= 1);
        assert!(report.rejected_candidates >= 1);
        assert_eq!(report.requests, report.hits + report.misses);
    }

    #[test]
    fn periodic_seam_uses_one_canonical_start_identity() {
        let planner =
            McloneOverworldStreamPlanner::new(12_345, McloneOverworldSamplingTopology::PeriodicX);
        let left = planner.potential_start(ChunkPos::new(-1, 12)).unwrap();
        let right = planner
            .potential_start(ChunkPos::new(MCLONE_OVERWORLD_PERIOD_CHUNKS as i32 - 1, 12))
            .unwrap();
        assert_eq!(left.canonical_start, right.canonical_start);
    }
}
