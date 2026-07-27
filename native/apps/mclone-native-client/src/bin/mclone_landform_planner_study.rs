//! Research-only hybrid macro-landform planner study.
//!
//! This binary deliberately owns every prototype type and algorithm. Nothing
//! in production world generation depends on it. Tactical 267 decides whether
//! any representation deserves promotion after Human Review B.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
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
            let prefix = format!("seed-{seed}-{}", topology.label());
            let receipt_path = config.output_dir.join(format!("{prefix}-plan.json"));
            let receipt = plan.core_receipt();
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
                plan.timings.total_ms
            );
            cases.push(CorpusCase {
                seed,
                topology: topology.label(),
                receipt: receipt_path.display().to_string(),
                checksum: receipt.checksum.clone(),
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
    control: McloneOverworldTerrainSample,
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
struct HybridPlan {
    seed: i64,
    topology: StudyTopology,
    grid: StudyGrid,
    fields: RegionalFields,
    cells: Vec<PlanCell>,
    sinks: Vec<BasinSink>,
    segments: Vec<PlanSegment>,
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
                control,
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
            total_ms,
        };

        Ok(Self {
            seed,
            topology,
            grid,
            fields,
            cells,
            sinks,
            segments,
            visit_order,
            metrics,
            timings,
        })
    }

    fn core_receipt(&self) -> CoreReceipt {
        let checksum = self.checksum();
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
    closed_basins: usize,
    valid_spills: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct BuildTimings {
    sample_ms: f64,
    sink_selection_ms: f64,
    drainage_ms: f64,
    extraction_ms: f64,
    total_ms: f64,
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

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
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
    }
}
