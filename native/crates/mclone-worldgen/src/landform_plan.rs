//! Research-only macro-landform planning summary.
//!
//! This module promotes the semantic plan proven by Tactical 267 into a
//! compact diagnostic contract. Production terrain generation deliberately
//! does not consume it.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::levelgen::{
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler,
    McloneOverworldTerrainSample,
};
use crate::noise::{GradientNoise2d, SeedDomain, ValueNoise2d};

pub const MCLONE_LANDFORM_PLAN_SCHEMA_REVISION: &str = "mclone-landform-plan-diagnostic-v1";
pub const MCLONE_LANDFORM_PLAN_STUDY_BLOCKS: i32 = MCLONE_OVERWORLD_PERIOD_BLOCKS;
pub const MCLONE_LANDFORM_PLAN_CELL_BLOCKS: i32 = 32;
pub const MCLONE_LANDFORM_PLAN_CELLS: usize =
    (MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / MCLONE_LANDFORM_PLAN_CELL_BLOCKS) as usize;

pub const MCLONE_LANDFORM_PLAN_CELL_OCEAN: u8 = 1 << 0;
pub const MCLONE_LANDFORM_PLAN_CELL_CHANNEL: u8 = 1 << 1;
pub const MCLONE_LANDFORM_PLAN_CELL_CONFLUENCE: u8 = 1 << 2;
pub const MCLONE_LANDFORM_PLAN_CELL_PROTECTED_BASIN: u8 = 1 << 3;
pub const MCLONE_LANDFORM_PLAN_CELL_QUIET_CORE: u8 = 1 << 4;
pub const MCLONE_LANDFORM_PLAN_CELL_CROP_EDGE: u8 = 1 << 5;

const MAX_PROTECTED_SINKS: usize = 4;
const MIN_SINK_COAST_DISTANCE_CELLS: f64 = 8.0;
const MIN_SINK_SEPARATION_CELLS: f64 = 22.0;
const CHANNEL_ACCUMULATION_THRESHOLD: u32 = 28;
const UPLIFT_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7570_6c31);
const UPLIFT_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7570_6c32);
const QUIET_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7175_6965);
const BROAD_LOW_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_6c6f_7731);
const SINK_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7369_6e6b);
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum McloneLandformPlanSinkKind {
    Ocean = 0,
    ProtectedClosed = 1,
    FallbackOpen = 2,
}

impl McloneLandformPlanSinkKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::ProtectedClosed => "protected-closed",
            Self::FallbackOpen => "fallback-open",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum McloneLandformPlanSegmentKind {
    Drainage = 0,
    Divide = 1,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneLandformPlanSegment {
    pub kind: McloneLandformPlanSegmentKind,
    pub a_x: f32,
    pub a_z: f32,
    pub b_x: f32,
    pub b_z: f32,
    pub order: u8,
    pub accumulation: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneLandformPlanSink {
    pub id: u16,
    pub kind: McloneLandformPlanSinkKind,
    pub world_x: i32,
    pub world_z: i32,
    pub source_level: f32,
    pub contributing_cells: u32,
    pub spill_level: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneLandformPlanMetrics {
    pub channel_cells: u32,
    pub confluences: u32,
    pub drainage_segments: u32,
    pub divide_segments: u32,
    pub protected_sinks: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneLandformPlanPoint {
    pub world_x: i32,
    pub world_z: i32,
    pub grid_x: u16,
    pub grid_z: u16,
    pub basin_id: u16,
    pub receiver_id: u32,
    pub accumulation: u32,
    pub stream_order: u8,
    pub flags: u8,
    pub uplift: f32,
    pub quiet: f32,
    pub broad_low: f32,
    pub base_y: i16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneLandformPlanSummary {
    pub seed: i64,
    pub min_x: i32,
    pub min_z: i32,
    pub width_cells: u16,
    pub depth_cells: u16,
    pub basin_ids: Vec<u16>,
    pub receiver_ids: Vec<u32>,
    pub accumulation: Vec<u32>,
    pub stream_order: Vec<u8>,
    pub flags: Vec<u8>,
    pub uplift: Vec<u8>,
    pub quiet: Vec<u8>,
    pub broad_low: Vec<u8>,
    pub base_y: Vec<i16>,
    pub segments: Vec<McloneLandformPlanSegment>,
    pub sinks: Vec<McloneLandformPlanSink>,
    pub metrics: McloneLandformPlanMetrics,
    pub checksum: u64,
}

impl McloneLandformPlanSummary {
    pub fn build_plane(seed: i64) -> Self {
        Planner::build(seed).summary()
    }

    pub fn point(&self, world_x: f64, world_z: f64) -> Option<McloneLandformPlanPoint> {
        let local_x = world_x - f64::from(self.min_x);
        let local_z = world_z - f64::from(self.min_z);
        let width_blocks = i32::from(self.width_cells) * MCLONE_LANDFORM_PLAN_CELL_BLOCKS;
        let depth_blocks = i32::from(self.depth_cells) * MCLONE_LANDFORM_PLAN_CELL_BLOCKS;
        if !(0.0..f64::from(width_blocks)).contains(&local_x)
            || !(0.0..f64::from(depth_blocks)).contains(&local_z)
        {
            return None;
        }
        let grid_x = (local_x / f64::from(MCLONE_LANDFORM_PLAN_CELL_BLOCKS)).floor() as usize;
        let grid_z = (local_z / f64::from(MCLONE_LANDFORM_PLAN_CELL_BLOCKS)).floor() as usize;
        let index = grid_z * usize::from(self.width_cells) + grid_x;
        Some(McloneLandformPlanPoint {
            world_x: self.min_x
                + grid_x as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
            world_z: self.min_z
                + grid_z as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
            grid_x: grid_x as u16,
            grid_z: grid_z as u16,
            basin_id: self.basin_ids[index],
            receiver_id: self.receiver_ids[index],
            accumulation: self.accumulation[index],
            stream_order: self.stream_order[index],
            flags: self.flags[index],
            uplift: dequantize_unit(self.uplift[index]),
            quiet: dequantize_unit(self.quiet[index]),
            broad_low: dequantize_unit(self.broad_low[index]),
            base_y: self.base_y[index],
        })
    }

    pub fn approximate_bytes(&self) -> usize {
        self.basin_ids.len() * size_of::<u16>()
            + self.receiver_ids.len() * size_of::<u32>()
            + self.accumulation.len() * size_of::<u32>()
            + self.stream_order.len()
            + self.flags.len()
            + self.uplift.len()
            + self.quiet.len()
            + self.broad_low.len()
            + self.base_y.len() * size_of::<i16>()
            + self.segments.len() * size_of::<McloneLandformPlanSegment>()
            + self.sinks.len() * size_of::<McloneLandformPlanSink>()
    }
}

#[derive(Clone, Copy, Debug)]
struct Grid;

impl Grid {
    const fn len(self) -> usize {
        MCLONE_LANDFORM_PLAN_CELLS * MCLONE_LANDFORM_PLAN_CELLS
    }

    const fn index(self, grid_x: usize, grid_z: usize) -> usize {
        grid_z * MCLONE_LANDFORM_PLAN_CELLS + grid_x
    }

    const fn coords(self, index: usize) -> (usize, usize) {
        (
            index % MCLONE_LANDFORM_PLAN_CELLS,
            index / MCLONE_LANDFORM_PLAN_CELLS,
        )
    }

    fn world(self, index: usize) -> (i32, i32) {
        let (grid_x, grid_z) = self.coords(index);
        (
            -MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2
                + grid_x as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
            -MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2
                + grid_z as i32 * MCLONE_LANDFORM_PLAN_CELL_BLOCKS
                + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2,
        )
    }

    fn neighbor(self, grid_x: isize, grid_z: isize) -> Option<usize> {
        let grid_x = usize::try_from(grid_x)
            .ok()
            .filter(|&x| x < MCLONE_LANDFORM_PLAN_CELLS)?;
        let grid_z = usize::try_from(grid_z)
            .ok()
            .filter(|&z| z < MCLONE_LANDFORM_PLAN_CELLS)?;
        Some(self.index(grid_x, grid_z))
    }

    fn neighbors(self, index: usize) -> Vec<(usize, f64)> {
        let (grid_x, grid_z) = self.coords(index);
        let mut result = Vec::with_capacity(8);
        for delta_z in -1_isize..=1 {
            for delta_x in -1_isize..=1 {
                if delta_x == 0 && delta_z == 0 {
                    continue;
                }
                if let Some(neighbor) =
                    self.neighbor(grid_x as isize + delta_x, grid_z as isize + delta_z)
                {
                    result.push((
                        neighbor,
                        if delta_x == 0 || delta_z == 0 {
                            1.0
                        } else {
                            std::f64::consts::SQRT_2
                        },
                    ));
                }
            }
        }
        result
    }

    fn grid_distance(self, left: usize, right: usize) -> f64 {
        let (left_x, left_z) = self.coords(left);
        let (right_x, right_z) = self.coords(right);
        (left_x.abs_diff(right_x) as f64).hypot(left_z.abs_diff(right_z) as f64)
    }

    fn touches_edge(self, index: usize) -> bool {
        let (grid_x, grid_z) = self.coords(index);
        grid_x == 0
            || grid_z == 0
            || grid_x + 1 == MCLONE_LANDFORM_PLAN_CELLS
            || grid_z + 1 == MCLONE_LANDFORM_PLAN_CELLS
    }
}

#[derive(Clone, Copy, Debug)]
struct Envelope {
    uplift: f64,
    quiet: f64,
    broad_low: f64,
    sink_permission: f64,
    base_y: f64,
}

#[derive(Clone, Copy, Debug)]
struct RegionalFields {
    uplift_large: GradientNoise2d,
    uplift_detail: GradientNoise2d,
    quiet: ValueNoise2d,
    broad_low: GradientNoise2d,
    sink: ValueNoise2d,
}

impl RegionalFields {
    fn new(seed: i64) -> Self {
        Self {
            uplift_large: GradientNoise2d::new(seed, UPLIFT_LARGE_DOMAIN, 1_536),
            uplift_detail: GradientNoise2d::new(seed, UPLIFT_DETAIL_DOMAIN, 768),
            quiet: ValueNoise2d::new(seed, QUIET_DOMAIN, 2_048),
            broad_low: GradientNoise2d::new(seed, BROAD_LOW_DOMAIN, 1_024),
            sink: ValueNoise2d::new(seed, SINK_DOMAIN, 768),
        }
    }

    fn sample(self, world_x: i32, world_z: i32, control: McloneOverworldTerrainSample) -> Envelope {
        let uplift_source = self.uplift_large.sample(world_x, world_z) * 0.68
            + self.uplift_detail.sample(world_x, world_z) * 0.32;
        let uplift = smoothstep(((uplift_source + 0.30) / 1.15).clamp(0.0, 1.0));
        let quiet_source = self.quiet.sample(world_x, world_z) * 0.5 + 0.5;
        let quiet =
            smoothstep(((quiet_source - 0.42) / 0.46).clamp(0.0, 1.0)) * (1.0 - uplift * 0.72);
        let broad_low_source = self.broad_low.sample(world_x, world_z);
        let broad_low = smoothstep(((-broad_low_source - 0.08) / 0.72).clamp(0.0, 1.0));
        let sink_texture = self.sink.sample(world_x, world_z) * 0.5 + 0.5;
        let inland = smoothstep((control.continentalness / 0.22).clamp(0.0, 1.0));
        let sink_permission = inland * (quiet * 0.40 + broad_low * 0.45 + sink_texture * 0.15);
        let base_y = if control.continentalness <= 0.0 {
            f64::from(control.surface_y)
        } else {
            f64::from(MCLONE_OVERWORLD_SEA_LEVEL)
                + 4.0
                + control.continentalness.max(0.0) * 11.0
                + uplift.powf(1.35) * (22.0 + control.continentalness.max(0.0) * 18.0)
                - quiet * 4.0
                - broad_low * 5.0
        };
        Envelope {
            uplift,
            quiet,
            broad_low,
            sink_permission,
            base_y,
        }
    }
}

#[derive(Clone, Debug)]
struct Cell {
    continentalness: f64,
    envelope: Envelope,
    provisional_y: f64,
    filled_y: f64,
    parent: Option<usize>,
    basin_id: usize,
    accumulation: u32,
    channel: bool,
    stream_order: u8,
    upstream_channels: u8,
    drain_level: f64,
    receiver_id: usize,
    channel_distance: f64,
}

#[derive(Clone, Debug)]
struct Sink {
    id: usize,
    kind: McloneLandformPlanSinkKind,
    index: usize,
    source_level: f64,
    contributing_cells: usize,
    spill_level: Option<f64>,
}

#[derive(Debug)]
struct Planner {
    seed: i64,
    grid: Grid,
    cells: Vec<Cell>,
    sinks: Vec<Sink>,
    segments: Vec<McloneLandformPlanSegment>,
}

impl Planner {
    fn build(seed: i64) -> Self {
        let grid = Grid;
        let fields = RegionalFields::new(seed);
        let sampler = McloneOverworldSampler::new(seed);
        let mut cells = Vec::with_capacity(grid.len());
        for index in 0..grid.len() {
            let (world_x, world_z) = grid.world(index);
            let control = sampler.sample(world_x, world_z);
            let envelope = fields.sample(world_x, world_z, control);
            cells.push(Cell {
                continentalness: control.continentalness,
                envelope,
                provisional_y: envelope.base_y,
                filled_y: envelope.base_y,
                parent: None,
                basin_id: usize::MAX,
                accumulation: u32::from(control.continentalness > 0.0),
                channel: false,
                stream_order: 0,
                upstream_channels: 0,
                drain_level: envelope.base_y,
                receiver_id: usize::MAX,
                channel_distance: f64::INFINITY,
            });
        }
        let (mut sinks, sources) = select_sinks(seed, grid, &cells);
        let visit_order = build_drainage_forest(seed, grid, &mut cells, &sources);
        accumulate_drainage(&visit_order, &mut cells);
        classify_channels(&visit_order, &mut cells);
        assign_drain_levels(&visit_order, &sinks, &mut cells);
        classify_stream_order(&visit_order, &mut cells);
        finish_basin_facts(grid, &cells, &mut sinks);
        let channel_mask = cells.iter().map(|cell| cell.channel).collect::<Vec<_>>();
        for (cell, distance) in cells
            .iter_mut()
            .zip(distance_from_mask(grid, &channel_mask))
        {
            cell.channel_distance = distance;
        }
        assign_receiver_ids(&visit_order, &mut cells);
        let segments = extract_segments(grid, &cells);
        Self {
            seed,
            grid,
            cells,
            sinks,
            segments,
        }
    }

    fn summary(self) -> McloneLandformPlanSummary {
        debug_assert_eq!(count_parent_cycles(&self.cells), 0);
        debug_assert!(self.cells.iter().all(|cell| cell.basin_id != usize::MAX));
        let mut basin_ids = Vec::with_capacity(self.cells.len());
        let mut receiver_ids = Vec::with_capacity(self.cells.len());
        let mut accumulation = Vec::with_capacity(self.cells.len());
        let mut stream_order = Vec::with_capacity(self.cells.len());
        let mut flags = Vec::with_capacity(self.cells.len());
        let mut uplift = Vec::with_capacity(self.cells.len());
        let mut quiet = Vec::with_capacity(self.cells.len());
        let mut broad_low = Vec::with_capacity(self.cells.len());
        let mut base_y = Vec::with_capacity(self.cells.len());
        for (index, cell) in self.cells.iter().enumerate() {
            let mut cell_flags = 0;
            if cell.continentalness <= 0.0 {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_OCEAN;
            }
            if cell.channel {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_CHANNEL;
            }
            if cell.channel && cell.upstream_channels >= 2 {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_CONFLUENCE;
            }
            if cell.basin_id > 0
                && self.sinks[cell.basin_id].kind == McloneLandformPlanSinkKind::ProtectedClosed
            {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_PROTECTED_BASIN;
            }
            if cell.envelope.quiet >= 0.62 {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_QUIET_CORE;
            }
            if self.grid.touches_edge(index) {
                cell_flags |= MCLONE_LANDFORM_PLAN_CELL_CROP_EDGE;
            }
            basin_ids.push(u16::try_from(cell.basin_id).unwrap_or(u16::MAX));
            receiver_ids.push(u32::try_from(cell.receiver_id).unwrap_or(u32::MAX));
            accumulation.push(cell.accumulation);
            stream_order.push(cell.stream_order);
            flags.push(cell_flags);
            uplift.push(quantize_unit(cell.envelope.uplift));
            quiet.push(quantize_unit(cell.envelope.quiet));
            broad_low.push(quantize_unit(cell.envelope.broad_low));
            base_y.push(cell.envelope.base_y.round() as i16);
        }
        let sinks = self
            .sinks
            .iter()
            .map(|sink| {
                let (world_x, world_z) = self.grid.world(sink.index);
                McloneLandformPlanSink {
                    id: sink.id as u16,
                    kind: sink.kind,
                    world_x,
                    world_z,
                    source_level: sink.source_level as f32,
                    contributing_cells: sink.contributing_cells as u32,
                    spill_level: sink.spill_level.map(|level| level as f32),
                }
            })
            .collect::<Vec<_>>();
        let metrics = McloneLandformPlanMetrics {
            channel_cells: self.cells.iter().filter(|cell| cell.channel).count() as u32,
            confluences: self
                .cells
                .iter()
                .filter(|cell| cell.channel && cell.upstream_channels >= 2)
                .count() as u32,
            drainage_segments: self
                .segments
                .iter()
                .filter(|segment| segment.kind == McloneLandformPlanSegmentKind::Drainage)
                .count() as u32,
            divide_segments: self
                .segments
                .iter()
                .filter(|segment| segment.kind == McloneLandformPlanSegmentKind::Divide)
                .count() as u32,
            protected_sinks: sinks
                .iter()
                .filter(|sink| sink.kind == McloneLandformPlanSinkKind::ProtectedClosed)
                .count() as u32,
        };
        let checksum = summary_checksum(
            self.seed,
            &basin_ids,
            &receiver_ids,
            &accumulation,
            &stream_order,
            &flags,
            &self.segments,
            &sinks,
        );
        McloneLandformPlanSummary {
            seed: self.seed,
            min_x: -MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2,
            min_z: -MCLONE_LANDFORM_PLAN_STUDY_BLOCKS / 2,
            width_cells: MCLONE_LANDFORM_PLAN_CELLS as u16,
            depth_cells: MCLONE_LANDFORM_PLAN_CELLS as u16,
            basin_ids,
            receiver_ids,
            accumulation,
            stream_order,
            flags,
            uplift,
            quiet,
            broad_low,
            base_y,
            segments: self.segments,
            sinks,
            metrics,
            checksum,
        }
    }
}

fn select_sinks(seed: i64, grid: Grid, cells: &[Cell]) -> (Vec<Sink>, Vec<(usize, usize, f64)>) {
    let ocean_mask = cells
        .iter()
        .map(|cell| cell.continentalness <= 0.0)
        .collect::<Vec<_>>();
    let coast_distance = distance_from_mask(grid, &ocean_mask);
    let ocean_indices = ocean_mask
        .iter()
        .enumerate()
        .filter_map(|(index, &ocean)| ocean.then_some(index))
        .collect::<Vec<_>>();
    let mut sinks = Vec::new();
    let mut sources = Vec::new();
    if let Some(&representative) = ocean_indices.first() {
        sinks.push(Sink {
            id: 0,
            kind: McloneLandformPlanSinkKind::Ocean,
            index: representative,
            source_level: f64::from(MCLONE_OVERWORLD_SEA_LEVEL),
            contributing_cells: 0,
            spill_level: None,
        });
        sources.extend(
            ocean_indices
                .into_iter()
                .map(|index| (index, 0, f64::from(MCLONE_OVERWORLD_SEA_LEVEL))),
        );
    } else {
        let fallback = cells
            .iter()
            .enumerate()
            .min_by(|left, right| {
                left.1
                    .provisional_y
                    .total_cmp(&right.1.provisional_y)
                    .then_with(|| left.0.cmp(&right.0))
            })
            .map(|(index, _)| index)
            .expect("landform-plan grid is non-empty");
        sinks.push(Sink {
            id: 0,
            kind: McloneLandformPlanSinkKind::FallbackOpen,
            index: fallback,
            source_level: cells[fallback].provisional_y,
            contributing_cells: 0,
            spill_level: None,
        });
        sources.push((fallback, 0, cells[fallback].provisional_y));
    }

    let mut candidates = cells
        .iter()
        .enumerate()
        .filter(|(index, cell)| {
            cell.continentalness > 0.10
                && coast_distance[*index] >= MIN_SINK_COAST_DISTANCE_CELLS
                && grid.neighbors(*index).into_iter().all(|(neighbor, _)| {
                    cells[neighbor].envelope.sink_permission <= cell.envelope.sink_permission
                })
        })
        .map(|(index, cell)| {
            (
                index,
                cell.envelope.sink_permission + hash_unit(seed, index as u64, 0x7369_6e6b) * 0.015,
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut selected = Vec::new();
    for (index, score) in candidates {
        if score < 0.48 {
            continue;
        }
        if selected
            .iter()
            .all(|&other| grid.grid_distance(index, other) >= MIN_SINK_SEPARATION_CELLS)
        {
            selected.push(index);
        }
        if selected.len() == MAX_PROTECTED_SINKS {
            break;
        }
    }
    for index in selected {
        let id = sinks.len();
        let level =
            (cells[index].provisional_y - 4.0).max(f64::from(MCLONE_OVERWORLD_SEA_LEVEL) + 1.0);
        sinks.push(Sink {
            id,
            kind: McloneLandformPlanSinkKind::ProtectedClosed,
            index,
            source_level: level,
            contributing_cells: 0,
            spill_level: None,
        });
        sources.push((index, id, level));
    }
    (sinks, sources)
}

fn build_drainage_forest(
    seed: i64,
    grid: Grid,
    cells: &mut [Cell],
    sources: &[(usize, usize, f64)],
) -> Vec<usize> {
    let mut visited = vec![false; cells.len()];
    let mut heap = BinaryHeap::new();
    for &(index, basin_id, level) in sources {
        if visited[index] {
            continue;
        }
        visited[index] = true;
        cells[index].filled_y = level;
        cells[index].basin_id = basin_id;
        cells[index].parent = None;
        heap.push(FloodEntry {
            priority: level,
            index,
        });
    }
    let mut visit_order = Vec::with_capacity(cells.len());
    while let Some(entry) = heap.pop() {
        visit_order.push(entry.index);
        for (neighbor, distance) in grid.neighbors(entry.index) {
            if visited[neighbor] {
                continue;
            }
            visited[neighbor] = true;
            let jitter = hash_unit(seed, neighbor as u64, entry.index as u64) * 0.006;
            let priority = cells[neighbor]
                .provisional_y
                .max(entry.priority + distance * 0.008 + jitter);
            cells[neighbor].filled_y = priority;
            cells[neighbor].parent = Some(entry.index);
            cells[neighbor].basin_id = cells[entry.index].basin_id;
            heap.push(FloodEntry {
                priority,
                index: neighbor,
            });
        }
    }
    visit_order
}

fn accumulate_drainage(visit_order: &[usize], cells: &mut [Cell]) {
    for &index in visit_order.iter().rev() {
        if let Some(parent) = cells[index].parent {
            cells[parent].accumulation = cells[parent]
                .accumulation
                .saturating_add(cells[index].accumulation);
        }
    }
}

fn classify_channels(visit_order: &[usize], cells: &mut [Cell]) {
    for &index in visit_order {
        cells[index].channel = cells[index].continentalness > 0.0
            && cells[index].accumulation >= CHANNEL_ACCUMULATION_THRESHOLD;
    }
    for &index in visit_order {
        let Some(parent) = cells[index].parent else {
            continue;
        };
        if cells[index].channel && cells[parent].channel {
            cells[parent].upstream_channels = cells[parent].upstream_channels.saturating_add(1);
        }
    }
}

fn assign_drain_levels(visit_order: &[usize], sinks: &[Sink], cells: &mut [Cell]) {
    for &index in visit_order {
        let Some(parent) = cells[index].parent else {
            cells[index].drain_level = sinks[cells[index].basin_id].source_level;
            continue;
        };
        let parent_level = cells[parent].drain_level;
        let grade =
            0.08 + cells[index].envelope.uplift * 0.22 + (1.0 - cells[index].envelope.quiet) * 0.05;
        let preferred = parent_level + grade;
        let ceiling = cells[index].provisional_y - 1.5;
        cells[index].drain_level = if ceiling > parent_level + 0.04 {
            preferred.min(ceiling)
        } else {
            parent_level + 0.04
        };
    }
}

fn classify_stream_order(visit_order: &[usize], cells: &mut [Cell]) {
    let mut maximum_incoming = vec![0_u8; cells.len()];
    let mut maximum_count = vec![0_u8; cells.len()];
    for &index in visit_order.iter().rev() {
        if !cells[index].channel {
            continue;
        }
        let incoming = maximum_incoming[index];
        let order = if incoming == 0 {
            1
        } else if maximum_count[index] >= 2 {
            incoming.saturating_add(1)
        } else {
            incoming
        };
        cells[index].stream_order = order;
        let Some(parent) = cells[index].parent else {
            continue;
        };
        if !cells[parent].channel {
            continue;
        }
        match order.cmp(&maximum_incoming[parent]) {
            Ordering::Greater => {
                maximum_incoming[parent] = order;
                maximum_count[parent] = 1;
            }
            Ordering::Equal => {
                maximum_count[parent] = maximum_count[parent].saturating_add(1);
            }
            Ordering::Less => {}
        }
    }
}

fn finish_basin_facts(grid: Grid, cells: &[Cell], sinks: &mut [Sink]) {
    for sink in sinks.iter_mut() {
        sink.contributing_cells = cells
            .iter()
            .filter(|cell| cell.basin_id == sink.id && cell.continentalness > 0.0)
            .count();
    }
    for sink in sinks
        .iter_mut()
        .filter(|sink| sink.kind == McloneLandformPlanSinkKind::ProtectedClosed)
    {
        let mut spill = f64::INFINITY;
        for (index, cell) in cells.iter().enumerate() {
            if cell.basin_id != sink.id {
                continue;
            }
            for (neighbor, _) in grid.neighbors(index) {
                if cells[neighbor].basin_id != sink.id {
                    spill = spill.min(cell.filled_y.max(cells[neighbor].filled_y));
                }
            }
        }
        sink.spill_level = spill.is_finite().then_some(spill);
    }
}

fn assign_receiver_ids(visit_order: &[usize], cells: &mut [Cell]) {
    let cell_count = cells.len();
    for &index in visit_order {
        let Some(parent) = cells[index].parent else {
            cells[index].receiver_id = cell_count + cells[index].basin_id;
            continue;
        };
        let channel_node =
            cells[index].channel && (cells[index].upstream_channels != 1 || !cells[parent].channel);
        cells[index].receiver_id = if channel_node {
            index
        } else {
            cells[parent].receiver_id
        };
    }
}

fn extract_segments(grid: Grid, cells: &[Cell]) -> Vec<McloneLandformPlanSegment> {
    let mut segments = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        let Some(parent) = cell.parent else {
            continue;
        };
        if !cell.channel {
            continue;
        }
        let (a_x, a_z) = grid.world(index);
        let (b_x, b_z) = grid.world(parent);
        segments.push(McloneLandformPlanSegment {
            kind: McloneLandformPlanSegmentKind::Drainage,
            a_x: a_x as f32,
            a_z: a_z as f32,
            b_x: b_x as f32,
            b_z: b_z as f32,
            order: cell.stream_order.max(1),
            accumulation: cell.accumulation,
        });
    }
    for grid_z in 0..MCLONE_LANDFORM_PLAN_CELLS {
        for grid_x in 0..MCLONE_LANDFORM_PLAN_CELLS {
            let index = grid.index(grid_x, grid_z);
            if cells[index].continentalness <= 0.0 || cells[index].channel_distance < 2.5 {
                continue;
            }
            if let Some(east) = grid.neighbor(grid_x as isize + 1, grid_z as isize) {
                if cells[east].continentalness > 0.0
                    && cells[east].channel_distance >= 2.5
                    && cells[index].receiver_id != cells[east].receiver_id
                    && receiver_owners_diverge(
                        cells[index].receiver_id,
                        cells[east].receiver_id,
                        cells,
                    )
                {
                    let (world_x, world_z) = grid.world(index);
                    segments.push(McloneLandformPlanSegment {
                        kind: McloneLandformPlanSegmentKind::Divide,
                        a_x: (world_x + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        a_z: (world_z - MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        b_x: (world_x + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        b_z: (world_z + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        order: 0,
                        accumulation: 0,
                    });
                }
            }
            if grid_z + 1 < MCLONE_LANDFORM_PLAN_CELLS {
                let south = grid.index(grid_x, grid_z + 1);
                if cells[south].continentalness > 0.0
                    && cells[south].channel_distance >= 2.5
                    && cells[index].receiver_id != cells[south].receiver_id
                    && receiver_owners_diverge(
                        cells[index].receiver_id,
                        cells[south].receiver_id,
                        cells,
                    )
                {
                    let (world_x, world_z) = grid.world(index);
                    segments.push(McloneLandformPlanSegment {
                        kind: McloneLandformPlanSegmentKind::Divide,
                        a_x: (world_x - MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        a_z: (world_z + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        b_x: (world_x + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        b_z: (world_z + MCLONE_LANDFORM_PLAN_CELL_BLOCKS / 2) as f32,
                        order: 0,
                        accumulation: 0,
                    });
                }
            }
        }
    }
    segments
}

fn receiver_owners_diverge(left: usize, right: usize, cells: &[Cell]) -> bool {
    !receiver_is_ancestor(left, right, cells) && !receiver_is_ancestor(right, left, cells)
}

fn receiver_is_ancestor(ancestor: usize, descendant: usize, cells: &[Cell]) -> bool {
    if ancestor == descendant {
        return true;
    }
    if ancestor >= cells.len() {
        let ancestor_basin = ancestor - cells.len();
        return if descendant >= cells.len() {
            descendant - cells.len() == ancestor_basin
        } else {
            cells[descendant].basin_id == ancestor_basin
        };
    }
    if descendant >= cells.len() {
        return false;
    }
    let mut current = Some(descendant);
    while let Some(index) = current {
        if index == ancestor {
            return true;
        }
        current = cells[index].parent;
    }
    false
}

fn distance_from_mask(grid: Grid, mask: &[bool]) -> Vec<f64> {
    let mut distance = vec![f64::INFINITY; grid.len()];
    let mut heap = BinaryHeap::new();
    for (index, &source) in mask.iter().enumerate() {
        if source {
            distance[index] = 0.0;
            heap.push(FloodEntry {
                priority: 0.0,
                index,
            });
        }
    }
    while let Some(entry) = heap.pop() {
        if entry.priority > distance[entry.index] {
            continue;
        }
        for (neighbor, edge_distance) in grid.neighbors(entry.index) {
            let candidate = entry.priority + edge_distance;
            if candidate < distance[neighbor] {
                distance[neighbor] = candidate;
                heap.push(FloodEntry {
                    priority: candidate,
                    index: neighbor,
                });
            }
        }
    }
    distance
}

fn count_parent_cycles(cells: &[Cell]) -> usize {
    let mut state = vec![0_u8; cells.len()];
    let mut cycles = 0;
    for start in 0..cells.len() {
        if state[start] != 0 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current {
            match state[index] {
                0 => {
                    state[index] = 1;
                    path.push(index);
                    current = cells[index].parent;
                }
                1 => {
                    cycles += 1;
                    break;
                }
                _ => break,
            }
        }
        for index in path {
            state[index] = 2;
        }
    }
    cycles
}

#[derive(Clone, Copy, Debug)]
struct FloodEntry {
    priority: f64,
    index: usize,
}

impl PartialEq for FloodEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority.to_bits() == other.priority.to_bits() && self.index == other.index
    }
}

impl Eq for FloodEntry {}

impl PartialOrd for FloodEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FloodEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.index.cmp(&self.index))
    }
}

fn summary_checksum(
    seed: i64,
    basin_ids: &[u16],
    receiver_ids: &[u32],
    accumulation: &[u32],
    stream_order: &[u8],
    flags: &[u8],
    segments: &[McloneLandformPlanSegment],
    sinks: &[McloneLandformPlanSink],
) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    hash_u64(&mut hash, seed as u64);
    for index in 0..basin_ids.len() {
        hash_u64(&mut hash, u64::from(basin_ids[index]));
        hash_u64(&mut hash, u64::from(receiver_ids[index]));
        hash_u64(&mut hash, u64::from(accumulation[index]));
        hash_u64(&mut hash, u64::from(stream_order[index]));
        hash_u64(&mut hash, u64::from(flags[index]));
    }
    for segment in segments {
        hash_u64(&mut hash, segment.kind as u64);
        hash_u64(&mut hash, u64::from(segment.a_x.to_bits()));
        hash_u64(&mut hash, u64::from(segment.a_z.to_bits()));
        hash_u64(&mut hash, u64::from(segment.b_x.to_bits()));
        hash_u64(&mut hash, u64::from(segment.b_z.to_bits()));
        hash_u64(&mut hash, u64::from(segment.order));
        hash_u64(&mut hash, u64::from(segment.accumulation));
    }
    for sink in sinks {
        hash_u64(&mut hash, u64::from(sink.id));
        hash_u64(&mut hash, sink.kind as u64);
        hash_u64(&mut hash, sink.world_x as u64);
        hash_u64(&mut hash, sink.world_z as u64);
    }
    hash
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn quantize_unit(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn dequantize_unit(value: u8) -> f32 {
    f32::from(value) / 255.0
}

fn hash_unit(seed: i64, left: u64, right: u64) -> f64 {
    let hash = splitmix64(seed as u64 ^ left.rotate_left(17) ^ right.rotate_left(41));
    (hash >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_summary_is_complete_and_deterministic() {
        let first = McloneLandformPlanSummary::build_plane(12_345);
        let second = McloneLandformPlanSummary::build_plane(12_345);
        assert_eq!(first, second);
        assert_eq!(first.width_cells, 192);
        assert_eq!(first.depth_cells, 192);
        assert_eq!(first.basin_ids.len(), 192 * 192);
        assert!(first.metrics.channel_cells > 0);
        assert!(first.metrics.drainage_segments > 0);
        assert!(first.metrics.divide_segments > 0);
        assert_eq!(first.metrics.protected_sinks, MAX_PROTECTED_SINKS as u32);
        assert!(
            first
                .sinks
                .iter()
                .filter(|sink| { sink.kind == McloneLandformPlanSinkKind::ProtectedClosed })
                .all(|sink| sink
                    .spill_level
                    .is_some_and(|spill| spill >= sink.source_level))
        );
    }

    #[test]
    fn point_inspection_respects_the_bounded_study_domain() {
        let summary = McloneLandformPlanSummary::build_plane(-98_765);
        let center = summary.point(0.0, 0.0).expect("domain center");
        assert_eq!(center.grid_x, 96);
        assert_eq!(center.grid_z, 96);
        assert!(center.receiver_id < u32::MAX);
        assert!(summary.point(-3_073.0, 0.0).is_none());
        assert!(summary.point(0.0, 3_072.0).is_none());
    }
}
