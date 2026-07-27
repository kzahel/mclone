//! Research-only hybrid macro-landform planner study.
//!
//! This binary deliberately owns every prototype type and algorithm. Nothing
//! in production world generation depends on it. Tactical 267 decides whether
//! any representation deserves promotion after Human Review B.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::fs;
use std::hint::black_box;
use std::mem::size_of;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use image::{Rgba, RgbaImage};
use mclone_worldgen::levelgen::{
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler,
    McloneOverworldSamplingTopology, McloneOverworldTerrainSample,
};
use mclone_worldgen::noise::{GradientNoise2d, SeedDomain, ValueNoise2d};
use serde::Serialize;

const DEFAULT_OUTPUT_DIR: &str = "/tmp/mclone-hybrid-landform-study";
const STUDY_BLOCKS: i32 = MCLONE_OVERWORLD_PERIOD_BLOCKS;
const CELL_BLOCKS: i32 = 32;
const CELLS: usize = (STUDY_BLOCKS / CELL_BLOCKS) as usize;
const DEFAULT_SEEDS: [i64; 3] = [12_345, 8_675_309, -98_765];
const MAX_PROTECTED_SINKS: usize = 4;
const MIN_SINK_COAST_DISTANCE_CELLS: f64 = 8.0;
const MIN_SINK_SEPARATION_CELLS: f64 = 22.0;
const CHANNEL_ACCUMULATION_THRESHOLD: u32 = 28;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;
const MAP_SCALE: u32 = 4;
const MAP_PIXELS: u32 = CELLS as u32 * MAP_SCALE;
const FAR_SUMMARY_STRIDE_CELLS: usize = 4;
const QUERY_WARMUP_ITERATIONS: usize = 1;
const QUERY_MEASURED_ITERATIONS: usize = 5;

const UPLIFT_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7570_6c31);
const UPLIFT_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7570_6c32);
const QUIET_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7175_6965);
const BROAD_LOW_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_6c6f_7731);
const SINK_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6c70_7369_6e6b);

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = Config::parse(std::env::args().skip(1))?;
    fs::create_dir_all(&config.output_dir).with_context(|| {
        format!(
            "create landform study output directory {}",
            config.output_dir.display()
        )
    })?;

    let mut cases = Vec::new();
    for &seed in &config.seeds {
        for &topology in &config.topologies {
            let plan = HybridPlan::build(seed, topology)?;
            let graph_only = plan.evaluate_grid(false);
            let detailed = plan.evaluate_grid(true);
            let prefix = format!("seed-{seed}-{}", topology.label());
            let evidence =
                write_evidence(&config.output_dir, &prefix, &plan, &graph_only, &detailed)?;
            let receipt_path = config.output_dir.join(format!("{prefix}-plan.json"));
            let receipt = plan.core_receipt(&graph_only, &detailed, evidence);
            let json = serde_json::to_string_pretty(&receipt)?;
            fs::write(&receipt_path, format!("{json}\n"))
                .with_context(|| format!("write {}", receipt_path.display()))?;
            println!(
                "{}: cells={} channels={} drainage_segments={} divide_segments={} \
                 sinks={} closed_basins={} build={:.3} ms",
                prefix,
                plan.cells.len(),
                plan.metrics.channel_cells,
                plan.metrics.drainage_segments,
                plan.metrics.divide_segments,
                plan.sinks.len(),
                plan.metrics.closed_basins,
                plan.timings.total_ms,
            );
            cases.push(CorpusCase {
                seed,
                topology: topology.label(),
                receipt: receipt_path.display().to_string(),
                checksum: receipt.checksum.clone(),
                atlas: receipt.artifacts.paths["atlas"].clone(),
                oblique: receipt.artifacts.paths["oblique"].clone(),
                journey: receipt.artifacts.paths["journey"].clone(),
            });
        }
    }

    let corpus = CorpusReceipt {
        schema: "mclone-hybrid-landform-study-corpus-v1",
        research_only: true,
        production_field_revision_unchanged: true,
        study_blocks: STUDY_BLOCKS,
        cell_blocks: CELL_BLOCKS,
        cases,
    };
    let corpus_path = config.output_dir.join("corpus.json");
    fs::write(
        &corpus_path,
        format!("{}\n", serde_json::to_string_pretty(&corpus)?),
    )
    .with_context(|| format!("write {}", corpus_path.display()))?;
    println!("wrote {}", corpus_path.display());
    Ok(())
}

#[derive(Clone, Debug)]
struct Config {
    output_dir: PathBuf,
    seeds: Vec<i64>,
    topologies: Vec<StudyTopology>,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut output_dir = PathBuf::from(DEFAULT_OUTPUT_DIR);
        let mut seeds = Vec::new();
        let mut topologies = Vec::new();
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--output" => {
                    output_dir =
                        PathBuf::from(args.next().context("--output requires a directory")?);
                }
                "--seed" => {
                    let raw = args.next().context("--seed requires an integer")?;
                    seeds.push(
                        raw.parse()
                            .with_context(|| format!("invalid --seed value `{raw}`"))?,
                    );
                }
                "--topology" => {
                    let raw = args.next().context("--topology requires a value")?;
                    match raw.as_str() {
                        "plane" => topologies.push(StudyTopology::Plane),
                        "cylinder" | "cylinder-x:384" => topologies.push(StudyTopology::CylinderX),
                        "all" => {
                            topologies.push(StudyTopology::Plane);
                            topologies.push(StudyTopology::CylinderX);
                        }
                        _ => bail!("invalid --topology `{raw}`; expected plane, cylinder, or all"),
                    }
                }
                "--help" | "-h" => {
                    bail!(
                        "usage: mclone-landform-planner-study \
                         [--output DIR] [--seed N]... \
                         [--topology plane|cylinder|all]"
                    );
                }
                _ => bail!("unknown argument `{argument}`"),
            }
        }
        if seeds.is_empty() {
            seeds.extend(DEFAULT_SEEDS);
        }
        if topologies.is_empty() {
            topologies.extend([StudyTopology::Plane, StudyTopology::CylinderX]);
        }
        topologies.sort_unstable();
        topologies.dedup();
        Ok(Self {
            output_dir,
            seeds,
            topologies,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StudyTopology {
    Plane,
    CylinderX,
}

impl StudyTopology {
    const fn label(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::CylinderX => "cylinder-x-384",
        }
    }

    const fn sampling_topology(self) -> McloneOverworldSamplingTopology {
        match self {
            Self::Plane => McloneOverworldSamplingTopology::Unbounded,
            Self::CylinderX => McloneOverworldSamplingTopology::PeriodicX,
        }
    }

    const fn min_world_x(self) -> i32 {
        match self {
            Self::Plane => -STUDY_BLOCKS / 2,
            Self::CylinderX => 0,
        }
    }

    fn wrap_grid_x(self, grid_x: isize) -> Option<usize> {
        match self {
            Self::Plane => usize::try_from(grid_x).ok().filter(|&x| x < CELLS),
            Self::CylinderX => Some(grid_x.rem_euclid(CELLS as isize) as usize),
        }
    }

    fn canonical_world_x(self, world_x: f64) -> f64 {
        match self {
            Self::Plane => world_x,
            Self::CylinderX => world_x.rem_euclid(f64::from(STUDY_BLOCKS)),
        }
    }

    fn shortest_world_dx(self, dx: f64) -> f64 {
        match self {
            Self::Plane => dx,
            Self::CylinderX => {
                let period = f64::from(STUDY_BLOCKS);
                (dx + period * 0.5).rem_euclid(period) - period * 0.5
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct StudyGrid {
    topology: StudyTopology,
}

impl StudyGrid {
    const fn new(topology: StudyTopology) -> Self {
        Self { topology }
    }

    const fn len(self) -> usize {
        CELLS * CELLS
    }

    const fn index(self, grid_x: usize, grid_z: usize) -> usize {
        grid_z * CELLS + grid_x
    }

    const fn coords(self, index: usize) -> (usize, usize) {
        (index % CELLS, index / CELLS)
    }

    fn world(self, index: usize) -> (i32, i32) {
        let (grid_x, grid_z) = self.coords(index);
        (
            self.topology.min_world_x() + grid_x as i32 * CELL_BLOCKS + CELL_BLOCKS / 2,
            -STUDY_BLOCKS / 2 + grid_z as i32 * CELL_BLOCKS + CELL_BLOCKS / 2,
        )
    }

    fn neighbor(self, grid_x: isize, grid_z: isize) -> Option<usize> {
        let grid_x = self.topology.wrap_grid_x(grid_x)?;
        let grid_z = usize::try_from(grid_z).ok().filter(|&z| z < CELLS)?;
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
                    let distance = if delta_x == 0 || delta_z == 0 {
                        1.0
                    } else {
                        std::f64::consts::SQRT_2
                    };
                    result.push((neighbor, distance));
                }
            }
        }
        result
    }

    fn grid_distance(self, left: usize, right: usize) -> f64 {
        let (left_x, left_z) = self.coords(left);
        let (right_x, right_z) = self.coords(right);
        let mut delta_x = left_x.abs_diff(right_x) as f64;
        if self.topology == StudyTopology::CylinderX {
            delta_x = delta_x.min((CELLS as f64 - delta_x).abs());
        }
        let delta_z = left_z.abs_diff(right_z) as f64;
        delta_x.hypot(delta_z)
    }

    fn touches_crop_edge(self, index: usize) -> bool {
        let (grid_x, grid_z) = self.coords(index);
        grid_z == 0
            || grid_z + 1 == CELLS
            || (self.topology == StudyTopology::Plane && (grid_x == 0 || grid_x + 1 == CELLS))
    }

    fn index_for_world(self, world_x: f64, world_z: f64) -> Option<usize> {
        let world_x = self.topology.canonical_world_x(world_x);
        let local_x = world_x - f64::from(self.topology.min_world_x());
        let local_z = world_z + f64::from(STUDY_BLOCKS) * 0.5;
        if !(0.0..f64::from(STUDY_BLOCKS)).contains(&local_z) {
            return None;
        }
        if self.topology == StudyTopology::Plane
            && !(0.0..f64::from(STUDY_BLOCKS)).contains(&local_x)
        {
            return None;
        }
        let grid_x =
            (local_x.rem_euclid(f64::from(STUDY_BLOCKS)) / f64::from(CELL_BLOCKS)).floor() as usize;
        let grid_z = (local_z / f64::from(CELL_BLOCKS)).floor() as usize;
        Some(self.index(grid_x.min(CELLS - 1), grid_z.min(CELLS - 1)))
    }
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
    fn new(seed: i64, topology: StudyTopology) -> Self {
        let gradient = |domain, scale| match topology {
            StudyTopology::Plane => GradientNoise2d::new(seed, domain, scale),
            StudyTopology::CylinderX => {
                GradientNoise2d::new_periodic_x(seed, domain, scale, STUDY_BLOCKS)
            }
        };
        let value = |domain, scale| match topology {
            StudyTopology::Plane => ValueNoise2d::new(seed, domain, scale),
            StudyTopology::CylinderX => {
                ValueNoise2d::new_periodic_x(seed, domain, scale, STUDY_BLOCKS)
            }
        };
        Self {
            uplift_large: gradient(UPLIFT_LARGE_DOMAIN, 1_536),
            uplift_detail: gradient(UPLIFT_DETAIL_DOMAIN, 768),
            quiet: value(QUIET_DOMAIN, 2_048),
            broad_low: gradient(BROAD_LOW_DOMAIN, 1_024),
            sink: value(SINK_DOMAIN, 768),
        }
    }

    fn sample(
        self,
        world_x: i32,
        world_z: i32,
        control: McloneOverworldTerrainSample,
    ) -> EnvelopeSample {
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
        EnvelopeSample {
            uplift,
            quiet,
            broad_low,
            sink_permission,
            base_y,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct EnvelopeSample {
    uplift: f64,
    quiet: f64,
    broad_low: f64,
    sink_permission: f64,
    base_y: f64,
}

#[derive(Clone, Debug)]
struct PlanCell {
    control: ControlFacts,
    envelope: EnvelopeSample,
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

#[derive(Clone, Copy, Debug)]
struct ControlFacts {
    continentalness: f64,
    surface_y: i32,
    water: bool,
}

impl From<McloneOverworldTerrainSample> for ControlFacts {
    fn from(sample: McloneOverworldTerrainSample) -> Self {
        Self {
            continentalness: sample.continentalness,
            surface_y: sample.surface_y,
            water: sample.continentalness <= 0.0 || sample.watercourse.is_water(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SinkKind {
    Ocean,
    ProtectedClosed,
    FallbackOpen,
}

#[derive(Clone, Debug, Serialize)]
struct BasinSink {
    id: usize,
    kind: SinkKind,
    grid_x: usize,
    grid_z: usize,
    world_x: i32,
    world_z: i32,
    source_level: f64,
    contributing_cells: usize,
    spill_level: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SegmentKind {
    Drainage,
    Divide,
}

#[derive(Clone, Debug, Serialize)]
struct PlanSegment {
    id: usize,
    kind: SegmentKind,
    a_x: f64,
    a_z: f64,
    b_x: f64,
    b_z: f64,
    a_y: f64,
    b_y: f64,
    influence_blocks: f64,
    strength: f64,
    order: u8,
    accumulation: u32,
    left_owner: usize,
    right_owner: usize,
}

#[derive(Debug)]
struct SegmentIndex {
    bins: Vec<Vec<u32>>,
    stored_references: usize,
}

impl SegmentIndex {
    fn build(grid: StudyGrid, segments: &[PlanSegment]) -> Self {
        let mut bins = vec![Vec::new(); grid.len()];
        let mut stored_references = 0;
        for segment in segments {
            let midpoint_x = grid
                .topology
                .canonical_world_x((segment.a_x + segment.b_x) * 0.5);
            let midpoint_z = (segment.a_z + segment.b_z) * 0.5;
            let Some(midpoint) = grid.index_for_world(midpoint_x, midpoint_z) else {
                continue;
            };
            let (midpoint_grid_x, midpoint_grid_z) = grid.coords(midpoint);
            let radius_cells =
                (segment.influence_blocks / f64::from(CELL_BLOCKS)).ceil() as isize + 2;
            for delta_z in -radius_cells..=radius_cells {
                for delta_x in -radius_cells..=radius_cells {
                    let Some(index) = grid.neighbor(
                        midpoint_grid_x as isize + delta_x,
                        midpoint_grid_z as isize + delta_z,
                    ) else {
                        continue;
                    };
                    let (world_x, world_z) = grid.world(index);
                    let (distance, _) = distance_to_segment(
                        grid.topology,
                        f64::from(world_x),
                        f64::from(world_z),
                        segment,
                    );
                    if distance > segment.influence_blocks + f64::from(CELL_BLOCKS) {
                        continue;
                    }
                    bins[index].push(segment.id as u32);
                    stored_references += 1;
                }
            }
        }
        Self {
            bins,
            stored_references,
        }
    }

    fn candidates(&self, grid: StudyGrid, world_x: f64, world_z: f64) -> &[u32] {
        grid.index_for_world(world_x, world_z)
            .map_or(&[], |index| self.bins[index].as_slice())
    }

    fn approximate_bytes(&self) -> usize {
        size_of::<Self>()
            + self.bins.capacity() * size_of::<Vec<u32>>()
            + self
                .bins
                .iter()
                .map(|bin| bin.capacity() * size_of::<u32>())
                .sum::<usize>()
    }
}

#[derive(Debug)]
struct HybridPlan {
    seed: i64,
    topology: StudyTopology,
    grid: StudyGrid,
    sampler: McloneOverworldSampler,
    fields: RegionalFields,
    cells: Vec<PlanCell>,
    sinks: Vec<BasinSink>,
    segments: Vec<PlanSegment>,
    segment_index: SegmentIndex,
    visit_order: Vec<usize>,
    metrics: CoreMetrics,
    timings: BuildTimings,
}

impl HybridPlan {
    fn build(seed: i64, topology: StudyTopology) -> Result<Self> {
        let total_start = Instant::now();
        let grid = StudyGrid::new(topology);
        let fields = RegionalFields::new(seed, topology);
        let sampler = McloneOverworldSampler::new_with_topology(seed, topology.sampling_topology());

        let sample_start = Instant::now();
        let mut cells = Vec::with_capacity(grid.len());
        for index in 0..grid.len() {
            let (world_x, world_z) = grid.world(index);
            let control = sampler.sample(world_x, world_z);
            let envelope = fields.sample(world_x, world_z, control);
            cells.push(PlanCell {
                control: control.into(),
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
        let sample_ms = elapsed_ms(sample_start);

        let sink_start = Instant::now();
        let (mut sinks, source_cells) = select_sinks(seed, grid, &cells);
        let sink_selection_ms = elapsed_ms(sink_start);

        let drainage_start = Instant::now();
        let visit_order = build_drainage_forest(seed, grid, &mut cells, &source_cells)?;
        accumulate_drainage(&visit_order, &mut cells);
        classify_channels(&visit_order, &mut cells);
        assign_drain_levels(&visit_order, &sinks, &mut cells);
        classify_stream_order(&visit_order, &mut cells);
        let drainage_ms = elapsed_ms(drainage_start);

        let extraction_start = Instant::now();
        finish_basin_facts(grid, &cells, &mut sinks);
        let channel_mask = cells.iter().map(|cell| cell.channel).collect::<Vec<_>>();
        let channel_distance = distance_from_mask(grid, &channel_mask);
        for (cell, distance) in cells.iter_mut().zip(channel_distance) {
            cell.channel_distance = distance;
        }
        assign_receiver_ids(&visit_order, &mut cells);
        let segments = extract_segments(grid, &cells);
        let extraction_ms = elapsed_ms(extraction_start);

        let indexing_start = Instant::now();
        let segment_index = SegmentIndex::build(grid, &segments);
        let indexing_ms = elapsed_ms(indexing_start);

        let cycle_count = count_parent_cycles(&cells);
        let unreachable_cells = cells
            .iter()
            .filter(|cell| cell.basin_id == usize::MAX)
            .count();
        let channel_cells = cells.iter().filter(|cell| cell.channel).count();
        let confluences = cells
            .iter()
            .filter(|cell| cell.channel && cell.upstream_channels >= 2)
            .count();
        let drainage_segments = segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Drainage)
            .count();
        let divide_segments = segments.len() - drainage_segments;
        let water_bearing_segments = segments
            .iter()
            .filter(|segment| {
                segment.kind == SegmentKind::Drainage
                    && (segment.accumulation >= 96 || segment.order >= 2)
            })
            .count();
        let drainage_length_blocks = segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Drainage)
            .map(segment_length)
            .sum();
        let divide_length_blocks = segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Divide)
            .map(segment_length)
            .sum();
        let divide_catchment_pairs = segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Divide)
            .map(|segment| (segment.left_owner, segment.right_owner))
            .collect::<BTreeSet<_>>()
            .len();
        let mut channel_order_counts = [0_usize; 8];
        for cell in cells.iter().filter(|cell| cell.channel) {
            channel_order_counts[usize::from(cell.stream_order).min(7)] += 1;
        }
        let ocean_sinks = sinks
            .iter()
            .filter(|sink| sink.kind == SinkKind::Ocean)
            .count();
        let fallback_open_sinks = sinks
            .iter()
            .filter(|sink| sink.kind == SinkKind::FallbackOpen)
            .count();
        let closed_basins = sinks
            .iter()
            .filter(|sink| sink.kind == SinkKind::ProtectedClosed)
            .count();
        let valid_spills = sinks
            .iter()
            .filter(|sink| sink.kind == SinkKind::ProtectedClosed && sink.spill_level.is_some())
            .count();
        let metrics = CoreMetrics {
            cycle_count,
            unreachable_cells,
            channel_cells,
            confluences,
            drainage_segments,
            divide_segments,
            water_bearing_segments,
            drainage_length_blocks,
            divide_length_blocks,
            divide_catchment_pairs,
            channel_order_counts,
            terminal_sinks: sinks.len(),
            ocean_sinks,
            fallback_open_sinks,
            closed_basins,
            valid_spills,
        };
        if cycle_count != 0 || unreachable_cells != 0 {
            bail!("invalid drainage forest: cycles={cycle_count} unreachable={unreachable_cells}");
        }
        let total_ms = elapsed_ms(total_start);
        let timings = BuildTimings {
            sample_ms,
            sink_selection_ms,
            drainage_ms,
            extraction_ms,
            indexing_ms,
            total_ms,
        };

        Ok(Self {
            seed,
            topology,
            grid,
            sampler,
            fields,
            cells,
            sinks,
            segments,
            segment_index,
            visit_order,
            metrics,
            timings,
        })
    }

    fn core_receipt(
        &self,
        graph_only: &EvaluatedGrid,
        detailed: &EvaluatedGrid,
        evidence: EvidenceReceipt,
    ) -> CoreReceipt {
        let checksum = self.checksum();
        let seam = self.seam_receipt();
        let far_summary = self.far_summary_receipt();
        CoreReceipt {
            schema: "mclone-hybrid-landform-study-plan-v1",
            research_only: true,
            production_field_revision_unchanged: true,
            seed: self.seed,
            topology: self.topology,
            bounds: ReceiptBounds {
                min_x: self.topology.min_world_x(),
                min_z: -STUDY_BLOCKS / 2,
                width_blocks: STUDY_BLOCKS,
                depth_blocks: STUDY_BLOCKS,
                cell_blocks: CELL_BLOCKS,
                width_cells: CELLS,
                depth_cells: CELLS,
            },
            metrics: self.metrics,
            timings: self.timings,
            memory: self.memory_receipt(),
            far_summary,
            reconstruction: ReconstructionReceipt {
                graph_only: graph_only.query_receipt(),
                subordinate_detail: detailed.query_receipt(),
                cylinder_seam: seam,
            },
            analysis: evidence.analysis,
            journey: evidence.journey,
            artifacts: evidence.artifacts,
            sinks: self.sinks.clone(),
            checksum: format!("{checksum:016x}"),
        }
    }

    fn checksum(&self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        hash_i64(&mut hash, self.seed);
        hash_u64(&mut hash, self.topology as u64);
        for cell in &self.cells {
            hash_u64(&mut hash, cell.filled_y.to_bits());
            hash_u64(&mut hash, cell.parent.unwrap_or(usize::MAX) as u64);
            hash_u64(&mut hash, cell.basin_id as u64);
            hash_u64(&mut hash, u64::from(cell.accumulation));
            hash_u64(&mut hash, u64::from(cell.stream_order));
            hash_u64(&mut hash, cell.receiver_id as u64);
        }
        for segment in &self.segments {
            hash_u64(&mut hash, segment.kind as u64);
            hash_u64(&mut hash, segment.a_x.to_bits());
            hash_u64(&mut hash, segment.a_z.to_bits());
            hash_u64(&mut hash, segment.b_x.to_bits());
            hash_u64(&mut hash, segment.b_z.to_bits());
        }
        hash
    }

    fn evaluate_grid(&self, subordinate_detail: bool) -> EvaluatedGrid {
        let start = Instant::now();
        let mut heights = Vec::with_capacity(self.grid.len());
        let mut water = Vec::with_capacity(self.grid.len());
        let mut total_candidates = 0_u64;
        let mut maximum_candidates = 0_usize;
        let mut checksum = FNV_OFFSET_BASIS;
        for index in 0..self.grid.len() {
            let (world_x, world_z) = self.grid.world(index);
            let sample = self.sample_reconstruction(
                f64::from(world_x),
                f64::from(world_z),
                subordinate_detail,
            );
            heights.push(sample.height);
            water.push(sample.water);
            total_candidates += sample.candidate_count as u64;
            maximum_candidates = maximum_candidates.max(sample.candidate_count);
            hash_u64(&mut checksum, sample.height.to_bits());
            hash_u64(&mut checksum, u64::from(sample.water));
        }
        let materialization_ms = elapsed_ms(start);
        let query_iteration_ms = self.benchmark_grid_queries(subordinate_detail, checksum);
        EvaluatedGrid {
            heights,
            water,
            materialization_ms,
            query_iteration_ms,
            average_candidates: total_candidates as f64 / self.grid.len() as f64,
            maximum_candidates,
            checksum,
        }
    }

    fn benchmark_grid_queries(&self, subordinate_detail: bool, expected_checksum: u64) -> Vec<f64> {
        let mut iteration_ms = Vec::with_capacity(QUERY_MEASURED_ITERATIONS);
        for iteration in 0..QUERY_WARMUP_ITERATIONS + QUERY_MEASURED_ITERATIONS {
            let start = Instant::now();
            let mut checksum = FNV_OFFSET_BASIS;
            for index in 0..self.grid.len() {
                let (world_x, world_z) = self.grid.world(index);
                let sample = self.sample_reconstruction(
                    f64::from(world_x),
                    f64::from(world_z),
                    subordinate_detail,
                );
                hash_u64(&mut checksum, sample.height.to_bits());
                hash_u64(&mut checksum, u64::from(sample.water));
            }
            black_box(checksum);
            assert_eq!(
                checksum, expected_checksum,
                "warm point-query checksum must match materialized reconstruction"
            );
            if iteration >= QUERY_WARMUP_ITERATIONS {
                iteration_ms.push(elapsed_ms(start));
            }
        }
        iteration_ms
    }

    fn sample_reconstruction(
        &self,
        world_x: f64,
        world_z: f64,
        subordinate_detail: bool,
    ) -> ReconstructionSample {
        let world_x = self.topology.canonical_world_x(world_x);
        let sample_x = world_x.round() as i32;
        let sample_z = world_z.round() as i32;
        let control = self.sampler.sample(sample_x, sample_z);
        let envelope = self.fields.sample(sample_x, sample_z, control);
        if control.continentalness <= 0.0 {
            return ReconstructionSample {
                height: f64::from(control.surface_y),
                water: true,
                candidate_count: 0,
            };
        }

        let candidates = self.segment_index.candidates(self.grid, world_x, world_z);
        let mut ridge_lift = 0.0_f64;
        let mut drainage_profiles = Vec::new();
        let mut water = false;
        let mut maximum_valley_influence = 0.0_f64;
        for &segment_id in candidates {
            let segment = &self.segments[segment_id as usize];
            let (distance, along) = distance_to_segment(self.topology, world_x, world_z, segment);
            if distance >= segment.influence_blocks {
                continue;
            }
            let influence = compact_profile(distance / segment.influence_blocks);
            match segment.kind {
                SegmentKind::Drainage => {
                    let bed = segment.a_y + (segment.b_y - segment.a_y) * along;
                    let side_slope = (0.050 - f64::from(segment.order) * 0.004).max(0.026);
                    let target = bed + 1.0 + distance * side_slope;
                    drainage_profiles.push((target, influence));
                    maximum_valley_influence = maximum_valley_influence.max(influence);
                    let water_half_width = 3.0 + f64::from(segment.order) * 2.25;
                    let water_bearing = segment.accumulation >= 96 || segment.order >= 2;
                    water |= water_bearing && distance <= water_half_width;
                }
                SegmentKind::Divide => {
                    ridge_lift = ridge_lift.max(segment.strength * influence);
                }
            }
        }

        let mut height = envelope.base_y + ridge_lift;
        for (target, influence) in drainage_profiles {
            let carved = height + (target - height) * influence;
            height = height.min(carved);
        }

        if let Some(cell_index) = self.grid.index_for_world(world_x, world_z) {
            let basin_id = self.cells[cell_index].basin_id;
            if basin_id > 0 {
                let sink = &self.sinks[basin_id];
                if sink.kind == SinkKind::ProtectedClosed {
                    let delta_x = self
                        .topology
                        .shortest_world_dx(world_x - f64::from(sink.world_x));
                    let delta_z = world_z - f64::from(sink.world_z);
                    let distance = delta_x.hypot(delta_z);
                    let radius =
                        (72.0 + (sink.contributing_cells as f64).sqrt() * 4.0).clamp(96.0, 240.0);
                    if distance < radius {
                        let influence = compact_profile(distance / radius);
                        let target = sink.source_level + 0.5 + distance * 0.018;
                        height = height.min(height + (target - height) * influence);
                        water |= distance < radius * 0.42 && height <= sink.source_level + 1.4;
                    }
                }
            }
        }

        if subordinate_detail {
            height += control.mountain_detail
                * (1.35 + envelope.uplift * 2.35)
                * (1.0 - maximum_valley_influence * 0.82);
        }
        ReconstructionSample {
            height,
            water,
            candidate_count: candidates.len(),
        }
    }

    fn seam_receipt(&self) -> SeamReceipt {
        if self.topology == StudyTopology::Plane {
            return SeamReceipt {
                tested: false,
                sample_count: 0,
                graph_only_max_error: 0.0,
                subordinate_detail_max_error: 0.0,
                source_control_max_error: 0.0,
                source_surface_mismatches: 0,
                source_water_mismatches: 0,
                envelope_max_error: 0.0,
            };
        }
        let mut graph_only_max_error = 0.0_f64;
        let mut subordinate_detail_max_error = 0.0_f64;
        let mut source_control_max_error = 0.0_f64;
        let mut source_surface_mismatches = 0;
        let mut source_water_mismatches = 0;
        let mut envelope_max_error = 0.0_f64;
        let mut sample_count = 0;
        for grid_z in (0..CELLS).step_by(7) {
            let world_z = -STUDY_BLOCKS / 2 + grid_z as i32 * CELL_BLOCKS + CELL_BLOCKS / 2;
            let left = self.sample_reconstruction(0.0, f64::from(world_z), false);
            let right =
                self.sample_reconstruction(f64::from(STUDY_BLOCKS), f64::from(world_z), false);
            graph_only_max_error = graph_only_max_error.max((left.height - right.height).abs());
            let left_detail = self.sample_reconstruction(0.0, f64::from(world_z), true);
            let right_detail =
                self.sample_reconstruction(f64::from(STUDY_BLOCKS), f64::from(world_z), true);
            subordinate_detail_max_error =
                subordinate_detail_max_error.max((left_detail.height - right_detail.height).abs());
            let left_control = self.sampler.sample(0, world_z);
            let right_control = self.sampler.sample(STUDY_BLOCKS, world_z);
            source_control_max_error = source_control_max_error
                .max((left_control.continentalness - right_control.continentalness).abs())
                .max((left_control.mountain_detail - right_control.mountain_detail).abs());
            source_surface_mismatches +=
                usize::from(left_control.surface_y != right_control.surface_y);
            source_water_mismatches += usize::from(
                left_control.watercourse.is_water() != right_control.watercourse.is_water(),
            );
            let left_envelope = self.fields.sample(0, world_z, left_control);
            let right_envelope = self.fields.sample(STUDY_BLOCKS, world_z, right_control);
            envelope_max_error = envelope_max_error
                .max((left_envelope.uplift - right_envelope.uplift).abs())
                .max((left_envelope.quiet - right_envelope.quiet).abs())
                .max((left_envelope.broad_low - right_envelope.broad_low).abs())
                .max((left_envelope.sink_permission - right_envelope.sink_permission).abs())
                .max((left_envelope.base_y - right_envelope.base_y).abs());
            sample_count += 1;
        }
        SeamReceipt {
            tested: true,
            sample_count,
            graph_only_max_error,
            subordinate_detail_max_error,
            source_control_max_error,
            source_surface_mismatches,
            source_water_mismatches,
            envelope_max_error,
        }
    }

    fn memory_receipt(&self) -> MemoryReceipt {
        let cell_bytes = self.cells.capacity() * size_of::<PlanCell>();
        let sink_bytes = self.sinks.capacity() * size_of::<BasinSink>();
        let segment_bytes = self.segments.capacity() * size_of::<PlanSegment>();
        let visit_order_bytes = self.visit_order.capacity() * size_of::<usize>();
        let index_bytes = self.segment_index.approximate_bytes();
        let plan_bytes = cell_bytes + sink_bytes + segment_bytes + visit_order_bytes + index_bytes;
        let summary_bytes = self.sinks.len() * size_of::<BasinSink>()
            + self.segments.len() * size_of::<PlanSegment>()
            + self.cells.len() * size_of::<EnvelopeSample>();
        MemoryReceipt {
            cell_bytes,
            sink_bytes,
            segment_bytes,
            visit_order_bytes,
            index_bytes,
            stored_index_references: self.segment_index.stored_references,
            approximate_plan_bytes: plan_bytes,
            approximate_summary_bytes: summary_bytes,
        }
    }

    fn far_summary_receipt(&self) -> FarSummaryReceipt {
        let start = Instant::now();
        let width_cells = CELLS.div_ceil(FAR_SUMMARY_STRIDE_CELLS);
        let depth_cells = CELLS.div_ceil(FAR_SUMMARY_STRIDE_CELLS);
        let mut raster = Vec::with_capacity(width_cells * depth_cells);
        let mut checksum = FNV_OFFSET_BASIS;
        for grid_z in (0..CELLS).step_by(FAR_SUMMARY_STRIDE_CELLS) {
            for grid_x in (0..CELLS).step_by(FAR_SUMMARY_STRIDE_CELLS) {
                let cell = &self.cells[self.grid.index(grid_x, grid_z)];
                let summary = FarSummaryCell {
                    uplift: quantize_unit(cell.envelope.uplift),
                    quiet: quantize_unit(cell.envelope.quiet),
                    broad_low: quantize_unit(cell.envelope.broad_low),
                    basin_id: u16::try_from(cell.basin_id).unwrap_or(u16::MAX),
                    base_y: cell.envelope.base_y.round() as i16,
                };
                hash_u64(&mut checksum, u64::from(summary.uplift));
                hash_u64(&mut checksum, u64::from(summary.quiet));
                hash_u64(&mut checksum, u64::from(summary.broad_low));
                hash_u64(&mut checksum, u64::from(summary.basin_id));
                hash_i64(&mut checksum, i64::from(summary.base_y));
                raster.push(summary);
            }
        }
        for segment in &self.segments {
            hash_u64(&mut checksum, segment.kind as u64);
            hash_u64(&mut checksum, segment.a_x.to_bits());
            hash_u64(&mut checksum, segment.a_z.to_bits());
            hash_u64(&mut checksum, segment.b_x.to_bits());
            hash_u64(&mut checksum, segment.b_z.to_bits());
            hash_u64(&mut checksum, segment.influence_blocks.to_bits());
        }
        for sink in &self.sinks {
            hash_u64(&mut checksum, sink.id as u64);
            hash_u64(&mut checksum, sink.kind as u64);
            hash_i64(&mut checksum, i64::from(sink.world_x));
            hash_i64(&mut checksum, i64::from(sink.world_z));
        }
        let raster_bytes = raster.capacity() * size_of::<FarSummaryCell>();
        let skeleton_bytes = self.segments.len() * size_of::<PlanSegment>();
        let sink_bytes = self.sinks.len() * size_of::<BasinSink>();
        FarSummaryReceipt {
            cell_blocks: CELL_BLOCKS * FAR_SUMMARY_STRIDE_CELLS as i32,
            width_cells,
            depth_cells,
            raster_bytes,
            drainage_records: self.metrics.drainage_segments,
            divide_records: self.metrics.divide_segments,
            sink_records: self.sinks.len(),
            skeleton_bytes,
            sink_bytes,
            approximate_total_bytes: raster_bytes + skeleton_bytes + sink_bytes,
            build_elapsed_ms: elapsed_ms(start),
            checksum: format!("{checksum:016x}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ReconstructionSample {
    height: f64,
    water: bool,
    candidate_count: usize,
}

#[derive(Debug)]
struct EvaluatedGrid {
    heights: Vec<f64>,
    water: Vec<bool>,
    materialization_ms: f64,
    query_iteration_ms: Vec<f64>,
    average_candidates: f64,
    maximum_candidates: usize,
    checksum: u64,
}

impl EvaluatedGrid {
    fn query_receipt(&self) -> PointQueryReceipt {
        let mut sorted = self.query_iteration_ms.clone();
        sorted.sort_by(f64::total_cmp);
        let median_elapsed_ms = median_f64(&sorted);
        PointQueryReceipt {
            point_count: self.heights.len(),
            warmup_iterations: QUERY_WARMUP_ITERATIONS,
            measured_iterations: QUERY_MEASURED_ITERATIONS,
            materialization_ms: self.materialization_ms,
            iteration_elapsed_ms: self.query_iteration_ms.clone(),
            minimum_elapsed_ms: sorted.first().copied().unwrap_or(0.0),
            median_elapsed_ms,
            maximum_elapsed_ms: sorted.last().copied().unwrap_or(0.0),
            points_per_second: self.heights.len() as f64 / (median_elapsed_ms / 1_000.0),
            average_segment_candidates: self.average_candidates,
            maximum_segment_candidates: self.maximum_candidates,
            checksum: format!("{:016x}", self.checksum),
        }
    }
}

fn select_sinks(
    seed: i64,
    grid: StudyGrid,
    cells: &[PlanCell],
) -> (Vec<BasinSink>, Vec<(usize, usize, f64)>) {
    let ocean_mask = cells
        .iter()
        .map(|cell| cell.control.continentalness <= 0.0)
        .collect::<Vec<_>>();
    let coast_distance = distance_from_mask(grid, &ocean_mask);
    let ocean_indices = ocean_mask
        .iter()
        .enumerate()
        .filter_map(|(index, &is_ocean)| is_ocean.then_some(index))
        .collect::<Vec<_>>();
    let mut sinks = Vec::new();
    let mut source_cells = Vec::new();

    if ocean_indices.is_empty() {
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
            .expect("study grid is non-empty");
        let (grid_x, grid_z) = grid.coords(fallback);
        let (world_x, world_z) = grid.world(fallback);
        let level = cells[fallback].provisional_y;
        sinks.push(BasinSink {
            id: 0,
            kind: SinkKind::FallbackOpen,
            grid_x,
            grid_z,
            world_x,
            world_z,
            source_level: level,
            contributing_cells: 0,
            spill_level: None,
        });
        source_cells.push((fallback, 0, level));
    } else {
        let representative = ocean_indices[0];
        let (grid_x, grid_z) = grid.coords(representative);
        let (world_x, world_z) = grid.world(representative);
        sinks.push(BasinSink {
            id: 0,
            kind: SinkKind::Ocean,
            grid_x,
            grid_z,
            world_x,
            world_z,
            source_level: f64::from(MCLONE_OVERWORLD_SEA_LEVEL),
            contributing_cells: 0,
            spill_level: None,
        });
        source_cells.extend(
            ocean_indices
                .iter()
                .map(|&index| (index, 0, f64::from(MCLONE_OVERWORLD_SEA_LEVEL))),
        );
    }

    let mut candidates = cells
        .iter()
        .enumerate()
        .filter(|(index, cell)| {
            cell.control.continentalness > 0.10
                && coast_distance[*index] >= MIN_SINK_COAST_DISTANCE_CELLS
                && is_local_sink_maximum(grid, cells, *index)
        })
        .map(|(index, cell)| {
            let tie = hash_unit(seed, index as u64, 0x7369_6e6b);
            (index, cell.envelope.sink_permission + tie * 0.015)
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
        let (grid_x, grid_z) = grid.coords(index);
        let (world_x, world_z) = grid.world(index);
        let level =
            (cells[index].provisional_y - 4.0).max(f64::from(MCLONE_OVERWORLD_SEA_LEVEL) + 1.0);
        sinks.push(BasinSink {
            id,
            kind: SinkKind::ProtectedClosed,
            grid_x,
            grid_z,
            world_x,
            world_z,
            source_level: level,
            contributing_cells: 0,
            spill_level: None,
        });
        source_cells.push((index, id, level));
    }
    (sinks, source_cells)
}

fn is_local_sink_maximum(grid: StudyGrid, cells: &[PlanCell], index: usize) -> bool {
    let value = cells[index].envelope.sink_permission;
    grid.neighbors(index)
        .into_iter()
        .all(|(neighbor, _)| cells[neighbor].envelope.sink_permission <= value)
}

fn build_drainage_forest(
    seed: i64,
    grid: StudyGrid,
    cells: &mut [PlanCell],
    sources: &[(usize, usize, f64)],
) -> Result<Vec<usize>> {
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
    if heap.is_empty() {
        bail!("drainage forest has no receiving source");
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
            let minimum = entry.priority + distance * 0.008 + jitter;
            let priority = cells[neighbor].provisional_y.max(minimum);
            cells[neighbor].filled_y = priority;
            cells[neighbor].parent = Some(entry.index);
            cells[neighbor].basin_id = cells[entry.index].basin_id;
            heap.push(FloodEntry {
                priority,
                index: neighbor,
            });
        }
    }
    Ok(visit_order)
}

fn accumulate_drainage(visit_order: &[usize], cells: &mut [PlanCell]) {
    for &index in visit_order.iter().rev() {
        if let Some(parent) = cells[index].parent {
            cells[parent].accumulation = cells[parent]
                .accumulation
                .saturating_add(cells[index].accumulation);
        }
    }
}

fn classify_channels(visit_order: &[usize], cells: &mut [PlanCell]) {
    for &index in visit_order {
        cells[index].channel = cells[index].control.continentalness > 0.0
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

fn assign_drain_levels(visit_order: &[usize], sinks: &[BasinSink], cells: &mut [PlanCell]) {
    for &index in visit_order {
        let Some(parent) = cells[index].parent else {
            cells[index].drain_level = sinks[cells[index].basin_id].source_level;
            continue;
        };
        let parent_level = cells[parent].drain_level;
        let grade =
            0.08 + cells[index].envelope.uplift * 0.22 + (1.0 - cells[index].envelope.quiet) * 0.05;
        let preferred = parent_level + grade;
        let terrain_ceiling = cells[index].provisional_y - 1.5;
        cells[index].drain_level = if terrain_ceiling > parent_level + 0.04 {
            preferred.min(terrain_ceiling)
        } else {
            parent_level + 0.04
        };
    }
}

fn classify_stream_order(visit_order: &[usize], cells: &mut [PlanCell]) {
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

fn finish_basin_facts(grid: StudyGrid, cells: &[PlanCell], sinks: &mut [BasinSink]) {
    for sink in sinks.iter_mut() {
        sink.contributing_cells = cells
            .iter()
            .filter(|cell| cell.basin_id == sink.id && cell.control.continentalness > 0.0)
            .count();
    }
    for sink in sinks
        .iter_mut()
        .filter(|sink| sink.kind == SinkKind::ProtectedClosed)
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

fn assign_receiver_ids(visit_order: &[usize], cells: &mut [PlanCell]) {
    let cell_count = cells.len();
    for &index in visit_order {
        let Some(parent) = cells[index].parent else {
            cells[index].receiver_id = cell_count + cells[index].basin_id;
            continue;
        };
        let is_channel_node =
            cells[index].channel && (cells[index].upstream_channels != 1 || !cells[parent].channel);
        cells[index].receiver_id = if is_channel_node {
            index
        } else {
            cells[parent].receiver_id
        };
    }
}

fn extract_segments(grid: StudyGrid, cells: &[PlanCell]) -> Vec<PlanSegment> {
    let mut segments = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        let Some(parent) = cell.parent else {
            continue;
        };
        if !cell.channel {
            continue;
        }
        let (a_world_x, a_world_z) = grid.world(index);
        let (b_world_x, b_world_z) = grid.world(parent);
        let a_x = f64::from(a_world_x);
        let a_z = f64::from(a_world_z);
        let b_x = a_x
            + grid
                .topology
                .shortest_world_dx(f64::from(b_world_x - a_world_x));
        let order = cell.stream_order.max(1);
        let influence = 100.0 + f64::from(order) * 52.0 + f64::from(cell.accumulation).ln() * 16.0;
        segments.push(PlanSegment {
            id: segments.len(),
            kind: SegmentKind::Drainage,
            a_x,
            a_z,
            b_x,
            b_z: f64::from(b_world_z),
            a_y: cell.drain_level,
            b_y: cells[parent].drain_level,
            influence_blocks: influence.clamp(128.0, 520.0),
            strength: 1.0,
            order,
            accumulation: cell.accumulation,
            left_owner: cell.basin_id,
            right_owner: cell.basin_id,
        });
    }

    for grid_z in 0..CELLS {
        for grid_x in 0..CELLS {
            let index = grid.index(grid_x, grid_z);
            if cells[index].control.continentalness <= 0.0 || cells[index].channel_distance < 2.5 {
                continue;
            }
            if let Some(east) = grid.neighbor(grid_x as isize + 1, grid_z as isize) {
                if cells[east].control.continentalness > 0.0
                    && cells[east].channel_distance >= 2.5
                    && cells[index].receiver_id != cells[east].receiver_id
                    && receiver_owners_diverge(
                        cells[index].receiver_id,
                        cells[east].receiver_id,
                        cells,
                    )
                {
                    let (world_x, world_z) = grid.world(index);
                    push_divide_segment(
                        &mut segments,
                        f64::from(world_x + CELL_BLOCKS / 2),
                        f64::from(world_z - CELL_BLOCKS / 2),
                        f64::from(world_x + CELL_BLOCKS / 2),
                        f64::from(world_z + CELL_BLOCKS / 2),
                        cells,
                        index,
                        east,
                    );
                }
            }
            if grid_z + 1 < CELLS {
                let south = grid.index(grid_x, grid_z + 1);
                if cells[south].control.continentalness > 0.0
                    && cells[south].channel_distance >= 2.5
                    && cells[index].receiver_id != cells[south].receiver_id
                    && receiver_owners_diverge(
                        cells[index].receiver_id,
                        cells[south].receiver_id,
                        cells,
                    )
                {
                    let (world_x, world_z) = grid.world(index);
                    push_divide_segment(
                        &mut segments,
                        f64::from(world_x - CELL_BLOCKS / 2),
                        f64::from(world_z + CELL_BLOCKS / 2),
                        f64::from(world_x + CELL_BLOCKS / 2),
                        f64::from(world_z + CELL_BLOCKS / 2),
                        cells,
                        index,
                        south,
                    );
                }
            }
        }
    }
    segments
}

fn receiver_owners_diverge(left: usize, right: usize, cells: &[PlanCell]) -> bool {
    !receiver_is_ancestor(left, right, cells) && !receiver_is_ancestor(right, left, cells)
}

fn receiver_is_ancestor(ancestor: usize, descendant: usize, cells: &[PlanCell]) -> bool {
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

fn push_divide_segment(
    segments: &mut Vec<PlanSegment>,
    a_x: f64,
    a_z: f64,
    b_x: f64,
    b_z: f64,
    cells: &[PlanCell],
    left: usize,
    right: usize,
) {
    let uplift = (cells[left].envelope.uplift + cells[right].envelope.uplift) * 0.5;
    let mut owners = [cells[left].receiver_id, cells[right].receiver_id];
    owners.sort_unstable();
    segments.push(PlanSegment {
        id: segments.len(),
        kind: SegmentKind::Divide,
        a_x,
        a_z,
        b_x,
        b_z,
        a_y: (cells[left].provisional_y + cells[right].provisional_y) * 0.5,
        b_y: (cells[left].provisional_y + cells[right].provisional_y) * 0.5,
        influence_blocks: 96.0 + uplift * 120.0,
        strength: 3.0 + uplift * 11.0,
        order: 0,
        accumulation: 0,
        left_owner: owners[0],
        right_owner: owners[1],
    });
}

fn distance_to_segment(
    topology: StudyTopology,
    world_x: f64,
    world_z: f64,
    segment: &PlanSegment,
) -> (f64, f64) {
    let query_x = segment.a_x + topology.shortest_world_dx(world_x - segment.a_x);
    let segment_x = segment.b_x - segment.a_x;
    let segment_z = segment.b_z - segment.a_z;
    let length_squared = segment_x * segment_x + segment_z * segment_z;
    let along = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((query_x - segment.a_x) * segment_x + (world_z - segment.a_z) * segment_z)
            / length_squared)
            .clamp(0.0, 1.0)
    };
    let closest_x = segment.a_x + segment_x * along;
    let closest_z = segment.a_z + segment_z * along;
    ((query_x - closest_x).hypot(world_z - closest_z), along)
}

fn segment_length(segment: &PlanSegment) -> f64 {
    (segment.b_x - segment.a_x).hypot(segment.b_z - segment.a_z)
}

fn compact_profile(normalized_distance: f64) -> f64 {
    if normalized_distance >= 1.0 {
        0.0
    } else {
        let complement = 1.0 - normalized_distance * normalized_distance;
        complement * complement
    }
}

fn distance_from_mask(grid: StudyGrid, mask: &[bool]) -> Vec<f64> {
    let mut distance = vec![f64::INFINITY; grid.len()];
    let mut heap = BinaryHeap::new();
    for (index, &is_source) in mask.iter().enumerate() {
        if is_source {
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

fn count_parent_cycles(cells: &[PlanCell]) -> usize {
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

#[derive(Clone, Copy, Debug, Serialize)]
struct CoreMetrics {
    cycle_count: usize,
    unreachable_cells: usize,
    channel_cells: usize,
    confluences: usize,
    drainage_segments: usize,
    divide_segments: usize,
    water_bearing_segments: usize,
    drainage_length_blocks: f64,
    divide_length_blocks: f64,
    divide_catchment_pairs: usize,
    channel_order_counts: [usize; 8],
    terminal_sinks: usize,
    ocean_sinks: usize,
    fallback_open_sinks: usize,
    closed_basins: usize,
    valid_spills: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct BuildTimings {
    sample_ms: f64,
    sink_selection_ms: f64,
    drainage_ms: f64,
    extraction_ms: f64,
    indexing_ms: f64,
    total_ms: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct MemoryReceipt {
    cell_bytes: usize,
    sink_bytes: usize,
    segment_bytes: usize,
    visit_order_bytes: usize,
    index_bytes: usize,
    stored_index_references: usize,
    approximate_plan_bytes: usize,
    approximate_summary_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
struct FarSummaryCell {
    uplift: u8,
    quiet: u8,
    broad_low: u8,
    basin_id: u16,
    base_y: i16,
}

#[derive(Clone, Debug, Serialize)]
struct FarSummaryReceipt {
    cell_blocks: i32,
    width_cells: usize,
    depth_cells: usize,
    raster_bytes: usize,
    drainage_records: usize,
    divide_records: usize,
    sink_records: usize,
    skeleton_bytes: usize,
    sink_bytes: usize,
    approximate_total_bytes: usize,
    build_elapsed_ms: f64,
    checksum: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReconstructionReceipt {
    graph_only: PointQueryReceipt,
    subordinate_detail: PointQueryReceipt,
    cylinder_seam: SeamReceipt,
}

#[derive(Clone, Debug, Serialize)]
struct PointQueryReceipt {
    point_count: usize,
    warmup_iterations: usize,
    measured_iterations: usize,
    materialization_ms: f64,
    iteration_elapsed_ms: Vec<f64>,
    minimum_elapsed_ms: f64,
    median_elapsed_ms: f64,
    maximum_elapsed_ms: f64,
    points_per_second: f64,
    average_segment_candidates: f64,
    maximum_segment_candidates: usize,
    checksum: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct SeamReceipt {
    tested: bool,
    sample_count: usize,
    graph_only_max_error: f64,
    subordinate_detail_max_error: f64,
    source_control_max_error: f64,
    source_surface_mismatches: usize,
    source_water_mismatches: usize,
    envelope_max_error: f64,
}

#[derive(Debug, Serialize)]
struct CoreReceipt {
    schema: &'static str,
    research_only: bool,
    production_field_revision_unchanged: bool,
    seed: i64,
    topology: StudyTopology,
    bounds: ReceiptBounds,
    metrics: CoreMetrics,
    timings: BuildTimings,
    memory: MemoryReceipt,
    far_summary: FarSummaryReceipt,
    reconstruction: ReconstructionReceipt,
    analysis: AnalysisReceipt,
    journey: JourneyReceipt,
    artifacts: ArtifactReceipt,
    sinks: Vec<BasinSink>,
    checksum: String,
}

#[derive(Debug, Serialize)]
struct ReceiptBounds {
    min_x: i32,
    min_z: i32,
    width_blocks: i32,
    depth_blocks: i32,
    cell_blocks: i32,
    width_cells: usize,
    depth_cells: usize,
}

#[derive(Debug, Serialize)]
struct CorpusCase {
    seed: i64,
    topology: &'static str,
    receipt: String,
    checksum: String,
    atlas: String,
    oblique: String,
    journey: String,
}

#[derive(Debug, Serialize)]
struct CorpusReceipt {
    schema: &'static str,
    research_only: bool,
    production_field_revision_unchanged: bool,
    study_blocks: i32,
    cell_blocks: i32,
    cases: Vec<CorpusCase>,
}

#[derive(Debug)]
struct EvidenceReceipt {
    analysis: AnalysisReceipt,
    journey: JourneyReceipt,
    artifacts: ArtifactReceipt,
}

#[derive(Clone, Debug, Serialize)]
struct ArtifactReceipt {
    paths: BTreeMap<&'static str, String>,
    atlas_panel_order: [&'static str; 4],
    oblique_panel_order: [&'static str; 2],
}

#[derive(Clone, Debug, Serialize)]
struct AnalysisReceipt {
    contours: ContourComparison,
    quiet_space: QuietSpaceReceipt,
    coast_arrivals: CoastArrivalReceipt,
}

#[derive(Clone, Debug, Serialize)]
struct ContourComparison {
    control: ContourStats,
    graph_only: ContourStats,
    subordinate_detail: ContourStats,
}

#[derive(Clone, Debug, Serialize)]
struct ContourStats {
    levels: Vec<i32>,
    closed_components: usize,
    small_closed_components: usize,
    median_area_cells: f64,
    median_diameter_blocks: f64,
    repeated_size_ratio: f64,
}

#[derive(Clone, Debug, Serialize)]
struct QuietSpaceReceipt {
    quiet_land_cells: usize,
    quiet_land_ratio: f64,
    component_count: usize,
    largest_component_cells: usize,
    largest_component_span_blocks: [i32; 2],
}

#[derive(Clone, Copy, Debug, Serialize)]
struct CoastArrivalReceipt {
    coast_land_cells: usize,
    drainage: usize,
    divide: usize,
    quiet: usize,
    broad_low: usize,
}

#[derive(Clone, Debug, Serialize)]
struct JourneyReceipt {
    name: String,
    world_z: i32,
    transition_count: usize,
    distinct_fact_count: usize,
    fact_counts: BTreeMap<&'static str, usize>,
    samples: Vec<JourneySample>,
}

#[derive(Clone, Debug, Serialize)]
struct JourneySample {
    world_x: i32,
    control_y: f64,
    graph_only_y: f64,
    subordinate_detail_y: f64,
    fact: &'static str,
}

fn write_evidence(
    output_dir: &std::path::Path,
    prefix: &str,
    plan: &HybridPlan,
    graph_only: &EvaluatedGrid,
    detailed: &EvaluatedGrid,
) -> Result<EvidenceReceipt> {
    let control_heights = plan
        .cells
        .iter()
        .map(|cell| f64::from(cell.control.surface_y))
        .collect::<Vec<_>>();
    let control_water = plan
        .cells
        .iter()
        .map(|cell| cell.control.water)
        .collect::<Vec<_>>();
    let divide_mask = divide_cell_mask(plan);
    let journey = select_journey(
        plan,
        &control_heights,
        &graph_only.heights,
        &detailed.heights,
        &divide_mask,
    );
    let analysis = AnalysisReceipt {
        contours: ContourComparison {
            control: contour_stats(plan.grid, &control_heights, &control_water),
            graph_only: contour_stats(plan.grid, &graph_only.heights, &graph_only.water),
            subordinate_detail: contour_stats(plan.grid, &detailed.heights, &detailed.water),
        },
        quiet_space: quiet_space_receipt(plan),
        coast_arrivals: coast_arrival_receipt(plan, &divide_mask),
    };

    let control_map = render_height_map(plan.grid, &control_heights, &control_water);
    let envelope_map = render_envelope_map(plan);
    let plan_map = render_plan_map(plan);
    let graph_map = render_height_map(plan.grid, &graph_only.heights, &graph_only.water);
    let detailed_map = render_height_map(plan.grid, &detailed.heights, &detailed.water);
    let atlas = render_atlas([&control_map, &plan_map, &graph_map, &detailed_map]);
    let oblique = render_oblique_pair(
        plan.grid,
        &control_heights,
        &control_water,
        &detailed.heights,
        &detailed.water,
    );
    let journey_image = render_journey_profile(
        plan,
        &control_heights,
        &graph_only.heights,
        &detailed.heights,
        &divide_mask,
        &journey,
    );

    let mut paths = BTreeMap::new();
    save_artifact(output_dir, prefix, "control", &control_map, &mut paths)?;
    save_artifact(output_dir, prefix, "envelopes", &envelope_map, &mut paths)?;
    save_artifact(output_dir, prefix, "plan", &plan_map, &mut paths)?;
    save_artifact(output_dir, prefix, "graph-only", &graph_map, &mut paths)?;
    save_artifact(
        output_dir,
        prefix,
        "subordinate-detail",
        &detailed_map,
        &mut paths,
    )?;
    save_artifact(output_dir, prefix, "atlas", &atlas, &mut paths)?;
    save_artifact(output_dir, prefix, "oblique", &oblique, &mut paths)?;
    save_artifact(output_dir, prefix, "journey", &journey_image, &mut paths)?;

    Ok(EvidenceReceipt {
        analysis,
        journey,
        artifacts: ArtifactReceipt {
            paths,
            atlas_panel_order: [
                "revision-21 control",
                "typed hybrid plan",
                "graph-only reconstruction",
                "subordinate-detail reconstruction",
            ],
            oblique_panel_order: ["revision-21 control", "subordinate-detail reconstruction"],
        },
    })
}

fn save_artifact(
    output_dir: &std::path::Path,
    prefix: &str,
    name: &'static str,
    image: &RgbaImage,
    paths: &mut BTreeMap<&'static str, String>,
) -> Result<()> {
    let path = output_dir.join(format!("{prefix}-{name}.png"));
    image
        .save(&path)
        .with_context(|| format!("write {}", path.display()))?;
    paths.insert(name, path.display().to_string());
    Ok(())
}

fn render_height_map(grid: StudyGrid, heights: &[f64], water: &[bool]) -> RgbaImage {
    let mut image = RgbaImage::new(MAP_PIXELS, MAP_PIXELS);
    for index in 0..grid.len() {
        let (grid_x, grid_z) = grid.coords(index);
        let west = grid
            .neighbor(grid_x as isize - 1, grid_z as isize)
            .unwrap_or(index);
        let east = grid
            .neighbor(grid_x as isize + 1, grid_z as isize)
            .unwrap_or(index);
        let north = grid
            .neighbor(grid_x as isize, grid_z as isize - 1)
            .unwrap_or(index);
        let south = grid
            .neighbor(grid_x as isize, grid_z as isize + 1)
            .unwrap_or(index);
        let gradient_x = (heights[east] - heights[west]) * 0.5;
        let gradient_z = (heights[south] - heights[north]) * 0.5;
        let light =
            ((0.68 - gradient_x * 0.025 - gradient_z * 0.034).clamp(0.36, 1.08) * 255.0) as u8;
        let mut color = if water[index] {
            [44, 105, 159, 255]
        } else {
            hypsometric_color(heights[index])
        };
        for component in &mut color[..3] {
            *component = ((u16::from(*component) * u16::from(light)) / 255) as u8;
        }
        let contour = !water[index]
            && grid.neighbors(index).into_iter().any(|(neighbor, _)| {
                !water[neighbor]
                    && (heights[index] / 8.0).floor() != (heights[neighbor] / 8.0).floor()
            });
        if contour {
            for component in &mut color[..3] {
                *component = ((*component as f32) * 0.72) as u8;
            }
        }
        fill_map_cell(&mut image, grid_x, grid_z, Rgba(color));
    }
    image
}

fn render_envelope_map(plan: &HybridPlan) -> RgbaImage {
    let mut image = RgbaImage::new(MAP_PIXELS, MAP_PIXELS);
    for (index, cell) in plan.cells.iter().enumerate() {
        let (grid_x, grid_z) = plan.grid.coords(index);
        let color = if cell.control.continentalness <= 0.0 {
            Rgba([28, 75, 120, 255])
        } else {
            let uplift = cell.envelope.uplift;
            let quiet = cell.envelope.quiet;
            let low = cell.envelope.broad_low;
            Rgba([
                (50.0 + uplift * 175.0 + low * 20.0) as u8,
                (70.0 + quiet * 130.0 + low * 70.0) as u8,
                (52.0 + quiet * 95.0 + low * 110.0) as u8,
                255,
            ])
        };
        fill_map_cell(&mut image, grid_x, grid_z, color);
    }
    for sink in &plan.sinks {
        let color = match sink.kind {
            SinkKind::Ocean => Rgba([90, 210, 255, 255]),
            SinkKind::ProtectedClosed => Rgba([255, 90, 210, 255]),
            SinkKind::FallbackOpen => Rgba([255, 220, 70, 255]),
        };
        draw_circle(
            &mut image,
            sink.grid_x as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
            sink.grid_z as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
            5,
            color,
        );
    }
    image
}

fn render_plan_map(plan: &HybridPlan) -> RgbaImage {
    let mut image = RgbaImage::new(MAP_PIXELS, MAP_PIXELS);
    for (index, cell) in plan.cells.iter().enumerate() {
        let (grid_x, grid_z) = plan.grid.coords(index);
        let color = if cell.control.continentalness <= 0.0 {
            Rgba([22, 55, 94, 255])
        } else {
            let basin = basin_color(cell.basin_id);
            let quiet = cell.envelope.quiet;
            Rgba([
                ((basin[0] as f64) * (1.0 - quiet * 0.25)) as u8,
                ((basin[1] as f64) * (1.0 + quiet * 0.18).min(1.35)) as u8,
                ((basin[2] as f64) * (1.0 + quiet * 0.10).min(1.25)) as u8,
                255,
            ])
        };
        fill_map_cell(&mut image, grid_x, grid_z, color);
    }
    for segment in plan
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentKind::Divide)
    {
        draw_plan_segment(&mut image, plan, segment, Rgba([242, 195, 76, 255]), 1);
    }
    for segment in plan
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentKind::Drainage)
    {
        draw_plan_segment(
            &mut image,
            plan,
            segment,
            Rgba([52, 177, 247, 255]),
            i32::from(segment.order.clamp(1, 3)),
        );
    }
    for (index, cell) in plan.cells.iter().enumerate() {
        if cell.channel && cell.upstream_channels >= 2 {
            let (grid_x, grid_z) = plan.grid.coords(index);
            draw_circle(
                &mut image,
                grid_x as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
                grid_z as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
                2,
                Rgba([255, 248, 130, 255]),
            );
        }
    }
    for sink in &plan.sinks {
        draw_circle(
            &mut image,
            sink.grid_x as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
            sink.grid_z as i32 * MAP_SCALE as i32 + MAP_SCALE as i32 / 2,
            if sink.kind == SinkKind::ProtectedClosed {
                5
            } else {
                3
            },
            if sink.kind == SinkKind::ProtectedClosed {
                Rgba([255, 75, 202, 255])
            } else {
                Rgba([90, 220, 255, 255])
            },
        );
    }
    image
}

fn draw_plan_segment(
    image: &mut RgbaImage,
    plan: &HybridPlan,
    segment: &PlanSegment,
    color: Rgba<u8>,
    width: i32,
) {
    let min_x = f64::from(plan.topology.min_world_x());
    let scale = f64::from(MAP_SCALE) / f64::from(CELL_BLOCKS);
    let a_x = ((segment.a_x - min_x) * scale).round() as i32;
    let b_x = ((segment.b_x - min_x) * scale).round() as i32;
    let a_z = ((segment.a_z + f64::from(STUDY_BLOCKS) * 0.5) * scale).round() as i32;
    let b_z = ((segment.b_z + f64::from(STUDY_BLOCKS) * 0.5) * scale).round() as i32;
    if plan.topology == StudyTopology::CylinderX {
        for shift in [-(MAP_PIXELS as i32), 0, MAP_PIXELS as i32] {
            draw_line_width(image, a_x + shift, a_z, b_x + shift, b_z, width, color);
        }
    } else {
        draw_line_width(image, a_x, a_z, b_x, b_z, width, color);
    }
}

fn render_atlas(panels: [&RgbaImage; 4]) -> RgbaImage {
    let mut atlas = RgbaImage::new(MAP_PIXELS * 2, MAP_PIXELS * 2);
    for (panel_index, panel) in panels.into_iter().enumerate() {
        let offset_x = (panel_index % 2) as u32 * MAP_PIXELS;
        let offset_y = (panel_index / 2) as u32 * MAP_PIXELS;
        copy_image(panel, &mut atlas, offset_x, offset_y);
    }
    atlas
}

fn render_journey_profile(
    plan: &HybridPlan,
    control: &[f64],
    graph_only: &[f64],
    detailed: &[f64],
    divide_mask: &[bool],
    journey: &JourneyReceipt,
) -> RgbaImage {
    const WIDTH: u32 = 1_024;
    const HEIGHT: u32 = 420;
    let mut image = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([20, 25, 31, 255]));
    let grid_z = ((journey.world_z + STUDY_BLOCKS / 2 - CELL_BLOCKS / 2) / CELL_BLOCKS)
        .clamp(0, CELLS as i32 - 1) as usize;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for grid_x in 0..CELLS {
        let index = plan.grid.index(grid_x, grid_z);
        minimum = minimum
            .min(control[index])
            .min(graph_only[index])
            .min(detailed[index]);
        maximum = maximum
            .max(control[index])
            .max(graph_only[index])
            .max(detailed[index]);
    }
    minimum = minimum.min(f64::from(MCLONE_OVERWORLD_SEA_LEVEL) - 2.0);
    maximum = maximum.max(minimum + 24.0);
    let plot_y = |height: f64| {
        let normalized = (height - minimum) / (maximum - minimum);
        (HEIGHT as f64 - 55.0 - normalized * (HEIGHT as f64 - 90.0)).round() as i32
    };
    let sea_y = plot_y(f64::from(MCLONE_OVERWORLD_SEA_LEVEL));
    draw_line_width(
        &mut image,
        0,
        sea_y,
        WIDTH as i32 - 1,
        sea_y,
        1,
        Rgba([55, 105, 160, 255]),
    );
    for grid_x in 0..CELLS {
        let index = plan.grid.index(grid_x, grid_z);
        let fact = journey_fact(plan, index, divide_mask);
        let color = journey_fact_color(fact);
        let start_x = grid_x as u32 * WIDTH / CELLS as u32;
        let end_x = (grid_x as u32 + 1) * WIDTH / CELLS as u32;
        for y in HEIGHT - 28..HEIGHT {
            for x in start_x..end_x.max(start_x + 1) {
                if x < WIDTH {
                    image.put_pixel(x, y, color);
                }
            }
        }
    }
    for (values, color, width) in [
        (control, Rgba([210, 215, 220, 255]), 2),
        (graph_only, Rgba([246, 183, 70, 255]), 2),
        (detailed, Rgba([92, 232, 145, 255]), 2),
    ] {
        let mut previous = None;
        for grid_x in 0..CELLS {
            let index = plan.grid.index(grid_x, grid_z);
            let x = (grid_x as f64 / (CELLS - 1) as f64 * (WIDTH - 1) as f64).round() as i32;
            let y = plot_y(values[index]);
            if let Some((previous_x, previous_y)) = previous {
                draw_line_width(&mut image, previous_x, previous_y, x, y, width, color);
            }
            previous = Some((x, y));
        }
    }
    image
}

fn select_journey(
    plan: &HybridPlan,
    control: &[f64],
    graph_only: &[f64],
    detailed: &[f64],
    divide_mask: &[bool],
) -> JourneyReceipt {
    let mut best = (0_usize, 0_usize, 0_usize);
    for grid_z in 0..CELLS {
        let mut seen = [false; 6];
        let mut transitions = 0;
        let mut previous = None;
        for grid_x in 0..CELLS {
            let index = plan.grid.index(grid_x, grid_z);
            let fact = journey_fact_id(plan, index, divide_mask);
            seen[fact] = true;
            if previous.is_some_and(|value| value != fact) {
                transitions += 1;
            }
            previous = Some(fact);
        }
        let distinct = seen.into_iter().filter(|&value| value).count();
        let score = distinct * 8 + transitions.min(48);
        if score > best.0 {
            best = (score, grid_z, transitions);
        }
    }
    let grid_z = best.1;
    let world_z = plan.grid.world(plan.grid.index(0, grid_z)).1;
    let mut fact_counts = BTreeMap::new();
    let mut distinct = [false; 6];
    for grid_x in 0..CELLS {
        let index = plan.grid.index(grid_x, grid_z);
        let fact = journey_fact(plan, index, divide_mask);
        *fact_counts.entry(fact).or_insert(0) += 1;
        distinct[journey_fact_id(plan, index, divide_mask)] = true;
    }
    let samples = (0..CELLS)
        .step_by(4)
        .map(|grid_x| {
            let index = plan.grid.index(grid_x, grid_z);
            JourneySample {
                world_x: plan.grid.world(index).0,
                control_y: control[index],
                graph_only_y: graph_only[index],
                subordinate_detail_y: detailed[index],
                fact: journey_fact(plan, index, divide_mask),
            }
        })
        .collect();
    JourneyReceipt {
        name: format!("seed-{}-{}-east-west", plan.seed, plan.topology.label()),
        world_z,
        transition_count: best.2,
        distinct_fact_count: distinct.into_iter().filter(|&value| value).count(),
        fact_counts,
        samples,
    }
}

fn journey_fact_id(plan: &HybridPlan, index: usize, divide_mask: &[bool]) -> usize {
    let cell = &plan.cells[index];
    if cell.control.continentalness <= 0.0 {
        0
    } else if cell.basin_id > 0 && cell.channel_distance <= 2.0 {
        1
    } else if cell.channel {
        2
    } else if divide_mask[index] {
        3
    } else if cell.envelope.quiet >= 0.62 {
        4
    } else {
        5
    }
}

fn journey_fact(plan: &HybridPlan, index: usize, divide_mask: &[bool]) -> &'static str {
    match journey_fact_id(plan, index, divide_mask) {
        0 => "ocean",
        1 => "closed-basin low",
        2 => "drainage",
        3 => "divide",
        4 => "quiet",
        _ => "regional upland",
    }
}

fn journey_fact_color(fact: &str) -> Rgba<u8> {
    match fact {
        "ocean" => Rgba([43, 105, 164, 255]),
        "closed-basin low" => Rgba([180, 72, 173, 255]),
        "drainage" => Rgba([65, 190, 248, 255]),
        "divide" => Rgba([238, 187, 65, 255]),
        "quiet" => Rgba([93, 154, 94, 255]),
        _ => Rgba([120, 110, 82, 255]),
    }
}

fn divide_cell_mask(plan: &HybridPlan) -> Vec<bool> {
    (0..plan.grid.len())
        .map(|index| {
            let (world_x, world_z) = plan.grid.world(index);
            plan.segment_index
                .candidates(plan.grid, f64::from(world_x), f64::from(world_z))
                .iter()
                .any(|&segment_id| {
                    let segment = &plan.segments[segment_id as usize];
                    segment.kind == SegmentKind::Divide
                        && distance_to_segment(
                            plan.topology,
                            f64::from(world_x),
                            f64::from(world_z),
                            segment,
                        )
                        .0 <= 48.0
                })
        })
        .collect()
}

fn contour_stats(grid: StudyGrid, heights: &[f64], water: &[bool]) -> ContourStats {
    let levels = [72, 80, 88, 96, 104, 112, 120];
    let mut areas = Vec::new();
    for &level in &levels {
        let mask = heights
            .iter()
            .zip(water)
            .map(|(&height, &is_water)| !is_water && height >= f64::from(level))
            .collect::<Vec<_>>();
        let mut visited = vec![false; grid.len()];
        for start in 0..grid.len() {
            if visited[start] || !mask[start] {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            visited[start] = true;
            let mut area = 0;
            let mut touches_edge = false;
            while let Some(index) = queue.pop_front() {
                area += 1;
                touches_edge |= grid.touches_crop_edge(index);
                for (neighbor, distance) in grid.neighbors(index) {
                    if distance > 1.01 || visited[neighbor] || !mask[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
            if !touches_edge {
                areas.push(area);
            }
        }
    }
    areas.sort_unstable();
    let median = median_usize(&areas);
    let repeated = if areas.is_empty() || median <= 0.0 {
        0.0
    } else {
        areas
            .iter()
            .filter(|&&area| (area as f64) >= median * 0.5 && (area as f64) <= median * 2.0)
            .count() as f64
            / areas.len() as f64
    };
    ContourStats {
        levels: levels.to_vec(),
        closed_components: areas.len(),
        small_closed_components: areas.iter().filter(|&&area| area <= 64).count(),
        median_area_cells: median,
        median_diameter_blocks: 2.0
            * (median * f64::from(CELL_BLOCKS * CELL_BLOCKS) / std::f64::consts::PI).sqrt(),
        repeated_size_ratio: repeated,
    }
}

fn quiet_space_receipt(plan: &HybridPlan) -> QuietSpaceReceipt {
    let mask = plan
        .cells
        .iter()
        .map(|cell| cell.control.continentalness > 0.0 && cell.envelope.quiet >= 0.62)
        .collect::<Vec<_>>();
    let land_cells = plan
        .cells
        .iter()
        .filter(|cell| cell.control.continentalness > 0.0)
        .count();
    let quiet_land_cells = mask.iter().filter(|&&value| value).count();
    let mut visited = vec![false; plan.grid.len()];
    let mut components = Vec::new();
    for start in 0..plan.grid.len() {
        if visited[start] || !mask[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        let mut count = 0;
        let mut min_x = usize::MAX;
        let mut max_x = 0;
        let mut min_z = usize::MAX;
        let mut max_z = 0;
        while let Some(index) = queue.pop_front() {
            count += 1;
            let (grid_x, grid_z) = plan.grid.coords(index);
            min_x = min_x.min(grid_x);
            max_x = max_x.max(grid_x);
            min_z = min_z.min(grid_z);
            max_z = max_z.max(grid_z);
            for (neighbor, _) in plan.grid.neighbors(index) {
                if !visited[neighbor] && mask[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        components.push((count, max_x - min_x + 1, max_z - min_z + 1));
    }
    components.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    let largest = components.first().copied().unwrap_or((0, 0, 0));
    QuietSpaceReceipt {
        quiet_land_cells,
        quiet_land_ratio: quiet_land_cells as f64 / land_cells.max(1) as f64,
        component_count: components.len(),
        largest_component_cells: largest.0,
        largest_component_span_blocks: [
            largest.1 as i32 * CELL_BLOCKS,
            largest.2 as i32 * CELL_BLOCKS,
        ],
    }
}

fn coast_arrival_receipt(plan: &HybridPlan, divide_mask: &[bool]) -> CoastArrivalReceipt {
    let mut result = CoastArrivalReceipt {
        coast_land_cells: 0,
        drainage: 0,
        divide: 0,
        quiet: 0,
        broad_low: 0,
    };
    for (index, cell) in plan.cells.iter().enumerate() {
        if cell.control.continentalness <= 0.0
            || !plan
                .grid
                .neighbors(index)
                .into_iter()
                .any(|(neighbor, _)| plan.cells[neighbor].control.continentalness <= 0.0)
        {
            continue;
        }
        result.coast_land_cells += 1;
        result.drainage += usize::from(cell.channel || cell.channel_distance <= 1.5);
        result.divide += usize::from(divide_mask[index]);
        result.quiet += usize::from(cell.envelope.quiet >= 0.62);
        result.broad_low += usize::from(cell.envelope.broad_low >= 0.62);
    }
    result
}

fn render_oblique_pair(
    grid: StudyGrid,
    control: &[f64],
    control_water: &[bool],
    detailed: &[f64],
    detailed_water: &[bool],
) -> RgbaImage {
    const PANEL_WIDTH: u32 = 620;
    const HEIGHT: u32 = 560;
    let mut image = RgbaImage::from_pixel(PANEL_WIDTH * 2, HEIGHT, Rgba([18, 23, 29, 255]));
    render_oblique_panel(&mut image, 0, PANEL_WIDTH, grid, control, control_water);
    render_oblique_panel(
        &mut image,
        PANEL_WIDTH,
        PANEL_WIDTH,
        grid,
        detailed,
        detailed_water,
    );
    for y in 0..HEIGHT {
        image.put_pixel(PANEL_WIDTH, y, Rgba([230, 230, 230, 255]));
    }
    image
}

fn render_oblique_panel(
    image: &mut RgbaImage,
    offset_x: u32,
    panel_width: u32,
    grid: StudyGrid,
    heights: &[f64],
    water: &[bool],
) {
    let mut cells = (0..CELLS - 1)
        .flat_map(|grid_z| (0..CELLS - 1).map(move |grid_x| (grid_x + grid_z, grid_x, grid_z)))
        .collect::<Vec<_>>();
    cells.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    let project = |grid_x: usize, grid_z: usize, height: f64| -> (f32, f32) {
        let x = offset_x as f32 + panel_width as f32 * 0.5 + (grid_x as f32 - grid_z as f32) * 1.42;
        let y = 235.0 + (grid_x as f32 + grid_z as f32 - CELLS as f32) * 0.57
            - (height as f32 - 75.0) * 2.35;
        (x, y)
    };
    for (_, grid_x, grid_z) in cells {
        let indices = [
            grid.index(grid_x, grid_z),
            grid.index(grid_x + 1, grid_z),
            grid.index(grid_x + 1, grid_z + 1),
            grid.index(grid_x, grid_z + 1),
        ];
        let display_height = |index: usize| {
            if water[index] && heights[index] < f64::from(MCLONE_OVERWORLD_SEA_LEVEL) {
                f64::from(MCLONE_OVERWORLD_SEA_LEVEL)
            } else {
                heights[index]
            }
        };
        let points = [
            project(grid_x, grid_z, display_height(indices[0])),
            project(grid_x + 1, grid_z, display_height(indices[1])),
            project(grid_x + 1, grid_z + 1, display_height(indices[2])),
            project(grid_x, grid_z + 1, display_height(indices[3])),
        ];
        let average_height = indices
            .iter()
            .map(|&index| display_height(index))
            .sum::<f64>()
            / 4.0;
        let is_water = indices.iter().filter(|&&index| water[index]).count() >= 2;
        let mut color = if is_water {
            [43, 103, 157, 255]
        } else {
            hypsometric_color(average_height)
        };
        let gradient = display_height(indices[1]) - display_height(indices[3]);
        let light = (0.82 - gradient * 0.025).clamp(0.42, 1.08);
        for component in &mut color[..3] {
            *component = ((*component as f64) * light).clamp(0.0, 255.0) as u8;
        }
        fill_triangle(image, points[0], points[1], points[2], Rgba(color));
        fill_triangle(image, points[0], points[2], points[3], Rgba(color));
    }
}

fn hypsometric_color(height: f64) -> [u8; 4] {
    let color = if height < 66.0 {
        lerp_color(
            [181, 167, 108],
            [119, 154, 83],
            ((height - 58.0) / 8.0).clamp(0.0, 1.0),
        )
    } else if height < 82.0 {
        lerp_color([103, 148, 78], [76, 127, 68], (height - 66.0) / 16.0)
    } else if height < 104.0 {
        lerp_color([76, 127, 68], [131, 113, 76], (height - 82.0) / 22.0)
    } else if height < 132.0 {
        lerp_color([131, 113, 76], [166, 159, 143], (height - 104.0) / 28.0)
    } else {
        lerp_color(
            [166, 159, 143],
            [235, 237, 234],
            ((height - 132.0) / 42.0).clamp(0.0, 1.0),
        )
    };
    [color[0], color[1], color[2], 255]
}

fn lerp_color(left: [u8; 3], right: [u8; 3], amount: f64) -> [u8; 3] {
    [
        (f64::from(left[0]) + f64::from(right[0] - left[0]) * amount) as u8,
        (f64::from(left[1]) + f64::from(right[1] - left[1]) * amount) as u8,
        (f64::from(left[2]) + f64::from(right[2] - left[2]) * amount) as u8,
    ]
}

fn basin_color(id: usize) -> [u8; 3] {
    if id == 0 {
        return [91, 108, 83];
    }
    let hash = splitmix64(id as u64 * 0x9e37_79b9);
    [
        75 + ((hash >> 8) & 63) as u8,
        80 + ((hash >> 24) & 63) as u8,
        75 + ((hash >> 40) & 63) as u8,
    ]
}

fn fill_map_cell(image: &mut RgbaImage, grid_x: usize, grid_z: usize, color: Rgba<u8>) {
    let start_x = grid_x as u32 * MAP_SCALE;
    let start_z = grid_z as u32 * MAP_SCALE;
    for offset_z in 0..MAP_SCALE {
        for offset_x in 0..MAP_SCALE {
            image.put_pixel(start_x + offset_x, start_z + offset_z, color);
        }
    }
}

fn copy_image(source: &RgbaImage, target: &mut RgbaImage, offset_x: u32, offset_y: u32) {
    for (x, y, pixel) in source.enumerate_pixels() {
        target.put_pixel(offset_x + x, offset_y + y, *pixel);
    }
}

fn draw_circle(image: &mut RgbaImage, center_x: i32, center_y: i32, radius: i32, color: Rgba<u8>) {
    for y in center_y - radius..=center_y + radius {
        for x in center_x - radius..=center_x + radius {
            if (x - center_x).pow(2) + (y - center_y).pow(2) <= radius.pow(2) {
                put_pixel_checked(image, x, y, color);
            }
        }
    }
}

fn draw_line_width(
    image: &mut RgbaImage,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    width: i32,
    color: Rgba<u8>,
) {
    let delta_x = (x1 - x0).abs();
    let step_x = if x0 < x1 { 1 } else { -1 };
    let delta_y = -(y1 - y0).abs();
    let step_y = if y0 < y1 { 1 } else { -1 };
    let mut error = delta_x + delta_y;
    loop {
        draw_circle(image, x0, y0, width.max(1) - 1, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled = error * 2;
        if doubled >= delta_y {
            error += delta_y;
            x0 += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y0 += step_y;
        }
    }
}

fn fill_triangle(
    image: &mut RgbaImage,
    first: (f32, f32),
    second: (f32, f32),
    third: (f32, f32),
    color: Rgba<u8>,
) {
    let minimum_x = first.0.min(second.0).min(third.0).floor() as i32;
    let maximum_x = first.0.max(second.0).max(third.0).ceil() as i32;
    let minimum_y = first.1.min(second.1).min(third.1).floor() as i32;
    let maximum_y = first.1.max(second.1).max(third.1).ceil() as i32;
    let edge = |left: (f32, f32), right: (f32, f32), point: (f32, f32)| {
        (point.0 - left.0) * (right.1 - left.1) - (point.1 - left.1) * (right.0 - left.0)
    };
    let orientation = edge(first, second, third);
    for y in minimum_y..=maximum_y {
        for x in minimum_x..=maximum_x {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let a = edge(first, second, point);
            let b = edge(second, third, point);
            let c = edge(third, first, point);
            if (orientation >= 0.0 && a >= 0.0 && b >= 0.0 && c >= 0.0)
                || (orientation < 0.0 && a <= 0.0 && b <= 0.0 && c <= 0.0)
            {
                put_pixel_checked(image, x, y, color);
            }
        }
    }
}

fn put_pixel_checked(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, color);
    }
}

fn median_usize(values: &[usize]) -> f64 {
    match values.len() {
        0 => 0.0,
        length if length % 2 == 1 => values[length / 2] as f64,
        length => (values[length / 2 - 1] + values[length / 2]) as f64 * 0.5,
    }
}

fn median_f64(values: &[f64]) -> f64 {
    match values.len() {
        0 => 0.0,
        length if length % 2 == 1 => values[length / 2],
        length => (values[length / 2 - 1] + values[length / 2]) * 0.5,
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn quantize_unit(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hash_unit(seed: i64, left: u64, right: u64) -> f64 {
    let hash = splitmix64((seed as u64) ^ left.rotate_left(17) ^ right.rotate_left(41));
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

fn hash_i64(hash: &mut u64, value: i64) {
    hash_u64(hash, value as u64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_fixed_corpus() {
        let config = Config::parse(Vec::<String>::new()).expect("default config");
        assert_eq!(config.seeds, DEFAULT_SEEDS);
        assert_eq!(
            config.topologies,
            [StudyTopology::Plane, StudyTopology::CylinderX]
        );
        assert_eq!(config.output_dir, PathBuf::from(DEFAULT_OUTPUT_DIR));
    }

    #[test]
    fn cylinder_grid_wraps_without_making_z_periodic() {
        let grid = StudyGrid::new(StudyTopology::CylinderX);
        assert_eq!(grid.neighbor(-1, 0), Some(grid.index(CELLS - 1, 0)));
        assert_eq!(grid.neighbor(CELLS as isize, 0), Some(grid.index(0, 0)));
        assert_eq!(grid.neighbor(0, -1), None);
        assert_eq!(grid.neighbor(0, CELLS as isize), None);
    }

    #[test]
    fn one_plan_is_complete_deterministic_and_acyclic() {
        let first = HybridPlan::build(12_345, StudyTopology::Plane).expect("first plan");
        let second = HybridPlan::build(12_345, StudyTopology::Plane).expect("second plan");
        assert_eq!(first.checksum(), second.checksum());
        assert_eq!(first.metrics.cycle_count, 0);
        assert_eq!(first.metrics.unreachable_cells, 0);
        assert!(first.metrics.channel_cells > 0);
        assert!(first.metrics.drainage_segments > 0);
        assert!(first.metrics.divide_segments > 0);
        assert_eq!(first.metrics.closed_basins, MAX_PROTECTED_SINKS);
        assert_eq!(first.metrics.valid_spills, MAX_PROTECTED_SINKS);
        assert!(first.sinks.iter().all(|sink| sink.contributing_cells > 0));
        assert!(
            first
                .sinks
                .iter()
                .filter(|sink| sink.kind == SinkKind::ProtectedClosed)
                .all(|sink| sink
                    .spill_level
                    .is_some_and(|spill| spill >= sink.source_level))
        );
        assert!(
            first
                .segments
                .iter()
                .filter(|segment| segment.kind == SegmentKind::Divide)
                .all(|segment| receiver_owners_diverge(
                    segment.left_owner,
                    segment.right_owner,
                    &first.cells,
                ))
        );
    }

    #[test]
    fn cylinder_reconstruction_is_exactly_periodic() {
        let plan = HybridPlan::build(12_345, StudyTopology::CylinderX).expect("cylinder plan");
        let seam = plan.seam_receipt();
        assert!(seam.tested);
        assert_eq!(seam.graph_only_max_error, 0.0);
        assert_eq!(seam.subordinate_detail_max_error, 0.0);
        assert!(seam.source_control_max_error < 1.0e-12);
        assert_eq!(seam.source_surface_mismatches, 0);
        assert_eq!(seam.source_water_mismatches, 0);
        assert!(seam.envelope_max_error < 1.0e-12);
    }
}
