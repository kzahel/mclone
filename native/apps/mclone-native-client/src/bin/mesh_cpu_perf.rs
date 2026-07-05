use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use mclone_app_runtime::render_assets::load_asset_source;
use mclone_core::{
    ChunkPos, ChunkSnapshot, SECTION_HEIGHT, block_to_section_coord, obfuscate_biome_zoom_seed,
};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionBuildReport,
    load_textured_terrain_assets,
};
use mclone_protocol::ChunkView;
use mclone_render::{
    chunk::{ChunkTextureAtlas, TexturedSectionDrawResources, TexturedSectionUploadTiming},
    headless::{HEADLESS_FORMAT, create_headless_device},
};
use mclone_render_session::build_render_sections_from_snapshots_with_biome_zoom_seed;
use mclone_server::{ChunkScheduler, ChunkSchedulerEvent, ChunkSchedulerMetrics, SqliteWorldStore};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_RENDER_DISTANCE: u32 = 10;
const DEFAULT_MAX_SECONDS: u64 = 180;
const COMPLETION_WAIT_MS: u64 = 30_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = Config::parse(env::args().skip(1))?;
    let center = ChunkPos::new(config.chunk_x, config.chunk_z);
    let chunk_tracking_radius = config
        .chunk_tracking_radius
        .unwrap_or_else(|| chunk_tracking_radius_for_render_distance(config.render_distance));
    let target_chunks = square_count(chunk_tracking_radius)?;
    let target_chunk_positions = square_chunk_positions(center, chunk_tracking_radius);
    let view = ChunkView {
        center,
        render_distance: config.render_distance,
        chunk_tracking_radius,
    };

    let asset_start = Instant::now();
    let asset_source = load_asset_source().context("failed to load asset source")?;
    let terrain_assets = load_textured_terrain_assets(&asset_source)
        .context("failed to load terrain mesh assets")?;
    let asset_load_ms = elapsed_ms(asset_start.elapsed());

    let snapshot_source = load_snapshots(&config, &view, target_chunks)?;
    let target_sections =
        target_sections_for_loaded_chunks(&snapshot_source.snapshots, &target_chunk_positions)?;
    let target_section_count = target_sections.len();
    let mesh_start = Instant::now();
    let mesh_report = build_render_sections_from_snapshots_with_biome_zoom_seed(
        &snapshot_source.snapshots,
        &terrain_assets.catalog,
        &target_sections,
        Some(obfuscate_biome_zoom_seed(config.seed)),
    )
    .context("failed to build CPU render-section meshes")?;
    let mesh_build_ms = elapsed_ms(mesh_start.elapsed());
    let mesh_stats = MeshBuildStats::from_report(&mesh_report, mesh_build_ms);
    if mesh_stats.built_sections != target_section_count {
        bail!(
            "mesh build returned {} sections for {} target sections",
            mesh_stats.built_sections,
            target_section_count
        );
    }
    if mesh_stats.non_empty_sections == 0 {
        bail!("mesh build produced no non-empty sections");
    }
    let gpu_upload = if config.gpu_upload {
        Some(run_gpu_upload(&terrain_assets.atlas, &mesh_report)?)
    } else {
        None
    };

    let report = Report {
        config,
        center,
        chunk_tracking_radius,
        target_chunks,
        target_sections: target_section_count,
        asset_load_ms,
        snapshot_source,
        mesh_stats,
        gpu_upload,
    };
    report.print_json();
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Config {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    render_distance: u32,
    chunk_tracking_radius: Option<u32>,
    lighting_enabled: bool,
    snapshot_source: SnapshotSource,
    gpu_upload: bool,
    max_seconds: u64,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            render_distance: DEFAULT_RENDER_DISTANCE,
            chunk_tracking_radius: None,
            lighting_enabled: true,
            snapshot_source: SnapshotSource::PersistedSqliteReload,
            gpu_upload: false,
            max_seconds: DEFAULT_MAX_SECONDS,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seed" => config.seed = parse_next(&mut args, "--seed")?,
                "--chunk-x" => config.chunk_x = parse_next(&mut args, "--chunk-x")?,
                "--chunk-z" => config.chunk_z = parse_next(&mut args, "--chunk-z")?,
                "--render-distance" => {
                    config.render_distance = parse_next(&mut args, "--render-distance")?
                }
                "--chunk-tracking-radius" | "--radius" => {
                    config.chunk_tracking_radius =
                        Some(parse_next(&mut args, "--chunk-tracking-radius")?)
                }
                "--lighting" => config.lighting_enabled = parse_bool_next(&mut args, "--lighting")?,
                "--disable-lighting" => config.lighting_enabled = false,
                "--enable-lighting" => config.lighting_enabled = true,
                "--snapshot-source" => {
                    config.snapshot_source = parse_next(&mut args, "--snapshot-source")?
                }
                "--fresh-snapshots" => config.snapshot_source = SnapshotSource::Fresh,
                "--persisted-sqlite-reload" => {
                    config.snapshot_source = SnapshotSource::PersistedSqliteReload
                }
                "--gpu-upload" => config.gpu_upload = true,
                "--no-gpu-upload" => config.gpu_upload = false,
                "--max-seconds" => config.max_seconds = parse_next(&mut args, "--max-seconds")?,
                "--help" | "-h" => bail!("{}", usage()),
                _ => bail!("unknown argument {arg}\n{}", usage()),
            }
        }

        if config.render_distance > 33 {
            bail!("--render-distance must be <= 33");
        }
        if matches!(config.chunk_tracking_radius, Some(radius) if radius > 33) {
            bail!("--chunk-tracking-radius must be <= 33");
        }
        if config.max_seconds == 0 {
            bail!("--max-seconds must be greater than zero");
        }

        Ok(config)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotSource {
    Fresh,
    PersistedSqliteReload,
}

impl SnapshotSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::PersistedSqliteReload => "persisted_sqlite_reload",
        }
    }
}

impl std::str::FromStr for SnapshotSource {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fresh" => Ok(Self::Fresh),
            "persisted-sqlite-reload" | "persisted_sqlite_reload" | "sqlite" => {
                Ok(Self::PersistedSqliteReload)
            }
            _ => bail!(
                "invalid snapshot source `{value}`; expected fresh or persisted-sqlite-reload"
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LoopCounters {
    polls: usize,
    empty_polls: usize,
    publish_polls: usize,
    worldgen_waits: usize,
    light_waits: usize,
    idle_waits: usize,
    wait_ms: f64,
    poll_call_ms: f64,
    max_poll_call_ms: f64,
    pending_unload_ms: f64,
    pending_unloads_processed: usize,
    snapshot_ready_events: usize,
    status_changed_events: usize,
}

impl LoopCounters {
    fn observe_events(&mut self, events: &[ChunkSchedulerEvent]) {
        for event in events {
            match event {
                ChunkSchedulerEvent::SnapshotReady(_) => {
                    self.snapshot_ready_events = self.snapshot_ready_events.saturating_add(1);
                }
                ChunkSchedulerEvent::StatusChanged { .. } => {
                    self.status_changed_events = self.status_changed_events.saturating_add(1);
                }
                ChunkSchedulerEvent::Unloaded { .. }
                | ChunkSchedulerEvent::HolderUnloaded { .. }
                | ChunkSchedulerEvent::EntityChunkLoaded { .. }
                | ChunkSchedulerEvent::SectionBlockUpdates { .. }
                | ChunkSchedulerEvent::BlockTickScheduled { .. }
                | ChunkSchedulerEvent::FluidTickScheduled { .. } => {}
            }
        }
    }
}

#[derive(Clone, Debug)]
struct SnapshotLoadReport {
    apply_interest_ms: f64,
    first_view_ready_ms: Option<f64>,
    total_elapsed_ms: f64,
    counters: LoopCounters,
    final_metrics: ChunkSchedulerMetrics,
}

#[derive(Clone, Debug)]
struct SnapshotSourceReport {
    mode: SnapshotSource,
    world_dir: Option<PathBuf>,
    sqlite_storage_bytes: Option<u64>,
    stored_chunk_records: Option<usize>,
    prewarm: Option<SnapshotLoadReport>,
    measured: SnapshotLoadReport,
    snapshots: Vec<ChunkSnapshot>,
}

#[derive(Clone, Copy, Debug)]
struct MeshBuildStats {
    build_ms: f64,
    built_sections: usize,
    non_empty_sections: usize,
    vertex_count: u64,
    index_count: u64,
    face_count: u64,
    visibility_graph_build_count: usize,
    visibility_graph_total_ms: f64,
    visibility_graph_worst_ms: f64,
}

impl MeshBuildStats {
    fn from_report(report: &TexturedRenderSectionBuildReport, build_ms: f64) -> Self {
        let mut non_empty_sections = 0_usize;
        let mut vertex_count = 0_u64;
        let mut index_count = 0_u64;
        let mut face_count = 0_u64;
        for section in &report.sections {
            if !section.is_empty() {
                non_empty_sections = non_empty_sections.saturating_add(1);
            }
            let stats = section.stats();
            vertex_count = vertex_count.saturating_add(u64::from(stats.vertex_count));
            index_count = index_count.saturating_add(u64::from(stats.index_count));
            face_count = face_count.saturating_add(u64::from(stats.face_count()));
        }
        Self {
            build_ms,
            built_sections: report.sections.len(),
            non_empty_sections,
            vertex_count,
            index_count,
            face_count,
            visibility_graph_build_count: report.visibility_graph.build_count,
            visibility_graph_total_ms: report.visibility_graph.total_ms,
            visibility_graph_worst_ms: report.visibility_graph.worst_ms,
        }
    }

    fn sections_per_second(self) -> f64 {
        per_second(self.built_sections, self.build_ms)
    }

    fn non_empty_sections_per_second(self) -> f64 {
        per_second(self.non_empty_sections, self.build_ms)
    }
}

#[derive(Clone, Copy, Debug)]
struct GpuUploadStats {
    device_create_ms: f64,
    draw_resource_setup_ms: f64,
    draw_resource_setup_poll_ms: f64,
    update_sections_ms: f64,
    device_poll_ms: f64,
    uploaded_sections: usize,
    resident_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
    uploaded_faces: u32,
    uploaded_bytes: u64,
    resident_indices: u32,
    timing: TexturedSectionUploadTiming,
}

#[derive(Debug)]
struct Report {
    config: Config,
    center: ChunkPos,
    chunk_tracking_radius: u32,
    target_chunks: usize,
    target_sections: usize,
    asset_load_ms: f64,
    snapshot_source: SnapshotSourceReport,
    mesh_stats: MeshBuildStats,
    gpu_upload: Option<GpuUploadStats>,
}

impl Report {
    fn print_json(&self) {
        println!("{{");
        let benchmark_name = if self.config.gpu_upload {
            "native_mesh_cpu_gpu_upload"
        } else {
            "native_mesh_cpu_only"
        };
        print_benchmark_metadata(benchmark_name, "  ", true);
        println!("  \"seed\": {},", self.config.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.center.x, self.center.z
        );
        println!("  \"render_distance\": {},", self.config.render_distance);
        println!(
            "  \"chunk_tracking_radius\": {},",
            self.chunk_tracking_radius
        );
        println!("  \"target_chunks\": {},", self.target_chunks);
        println!("  \"target_sections\": {},", self.target_sections);
        println!("  \"lighting_enabled\": {},", self.config.lighting_enabled);
        println!(
            "  \"snapshot_source_mode\": \"{}\",",
            self.snapshot_source.mode.as_str()
        );
        println!("  \"asset_load_ms\": {:.3},", self.asset_load_ms);
        print_snapshot_source_json("  ", &self.snapshot_source, true, self.target_chunks);
        print_mesh_stats_json("  ", self.mesh_stats, true);
        match self.gpu_upload {
            Some(stats) => print_gpu_upload_stats_json("  ", stats, false),
            None => println!("  \"gpu_upload\": null"),
        }
        println!("}}");
    }
}

fn run_gpu_upload(
    atlas: &mclone_mesh::TextureAtlasImage,
    mesh_report: &TexturedRenderSectionBuildReport,
) -> Result<GpuUploadStats> {
    let device_start = Instant::now();
    let (device, queue) =
        create_headless_device().context("failed to create headless GPU device")?;
    let device_create_ms = elapsed_ms(device_start.elapsed());

    let atlas_upload = ChunkTextureAtlas {
        width: atlas.width,
        height: atlas.height,
        rgba: atlas.rgba(),
    };
    let setup_start = Instant::now();
    let mut draw =
        TexturedSectionDrawResources::new(&device, &queue, HEADLESS_FORMAT, &[], atlas_upload)
            .context("failed to create empty textured section draw resources")?;
    let draw_resource_setup_ms = elapsed_ms(setup_start.elapsed());
    let setup_poll_start = Instant::now();
    device
        .poll(wgpu::PollType::Wait)
        .context("device poll after draw resource setup failed")?;
    let draw_resource_setup_poll_ms = elapsed_ms(setup_poll_start.elapsed());

    let upload_start = Instant::now();
    let (upload_report, timing) = draw
        .apply_section_updates_with_context_timed(
            &device,
            &mesh_report.sections,
            &BTreeSet::new(),
            false,
        )
        .context("failed to upload prebuilt render sections")?;
    let update_sections_ms = elapsed_ms(upload_start.elapsed());
    let poll_start = Instant::now();
    device
        .poll(wgpu::PollType::Wait)
        .context("device poll after section upload failed")?;
    let device_poll_ms = elapsed_ms(poll_start.elapsed());

    let vertex_bytes = u64::from(upload_report.uploaded_vertex_count)
        .saturating_mul(std::mem::size_of::<TexturedChunkVertex>() as u64);
    let index_bytes = u64::from(upload_report.uploaded_index_count)
        .saturating_mul(std::mem::size_of::<u32>() as u64);

    Ok(GpuUploadStats {
        device_create_ms,
        draw_resource_setup_ms,
        draw_resource_setup_poll_ms,
        update_sections_ms,
        device_poll_ms,
        uploaded_sections: upload_report.uploaded_section_count,
        resident_sections: draw.section_count(),
        uploaded_vertices: upload_report.uploaded_vertex_count,
        uploaded_indices: upload_report.uploaded_index_count,
        uploaded_faces: upload_report.uploaded_face_count(),
        uploaded_bytes: vertex_bytes.saturating_add(index_bytes),
        resident_indices: draw.index_count(),
        timing,
    })
}

fn load_snapshots(
    config: &Config,
    view: &ChunkView,
    target_chunks: usize,
) -> Result<SnapshotSourceReport> {
    match config.snapshot_source {
        SnapshotSource::Fresh => {
            let mut scheduler = ChunkScheduler::new(config.seed);
            scheduler.set_lighting_enabled(config.lighting_enabled);
            let (measured, snapshots) =
                run_scheduler_snapshot_load(config, view, target_chunks, scheduler)?;
            Ok(SnapshotSourceReport {
                mode: SnapshotSource::Fresh,
                world_dir: None,
                sqlite_storage_bytes: None,
                stored_chunk_records: None,
                prewarm: None,
                measured,
                snapshots,
            })
        }
        SnapshotSource::PersistedSqliteReload => {
            let world_dir = unique_sqlite_world_dir(config);
            let _ = fs::remove_dir_all(&world_dir);

            let prewarm_store =
                SqliteWorldStore::open_world_dir(&world_dir).with_context(|| {
                    format!(
                        "failed to open prewarm sqlite world at {}",
                        world_dir.display()
                    )
                })?;
            let mut prewarm_scheduler =
                ChunkScheduler::with_world_store(config.seed, Box::new(prewarm_store));
            prewarm_scheduler.set_lighting_enabled(config.lighting_enabled);
            let (prewarm, _) =
                run_scheduler_snapshot_load(config, view, target_chunks, prewarm_scheduler)?;
            let stored_chunk_records = Some(prewarm.final_metrics.loaded_snapshot_chunks);

            let reload_store = SqliteWorldStore::open_world_dir(&world_dir).with_context(|| {
                format!(
                    "failed to open reload sqlite world at {}",
                    world_dir.display()
                )
            })?;
            let mut reload_scheduler =
                ChunkScheduler::with_world_store(config.seed, Box::new(reload_store));
            reload_scheduler.set_lighting_enabled(config.lighting_enabled);
            let (measured, snapshots) =
                run_scheduler_snapshot_load(config, view, target_chunks, reload_scheduler)?;
            validate_persisted_reload(&measured)?;

            Ok(SnapshotSourceReport {
                mode: SnapshotSource::PersistedSqliteReload,
                world_dir: Some(world_dir.clone()),
                sqlite_storage_bytes: sqlite_storage_byte_count(&world_dir),
                stored_chunk_records,
                prewarm: Some(prewarm),
                measured,
                snapshots,
            })
        }
    }
}

fn run_scheduler_snapshot_load(
    config: &Config,
    view: &ChunkView,
    target_chunks: usize,
    mut scheduler: ChunkScheduler,
) -> Result<(SnapshotLoadReport, Vec<ChunkSnapshot>)> {
    let total_start = Instant::now();
    let apply_start = Instant::now();
    let initial_events = scheduler
        .apply_interest(view.clone())
        .context("failed to apply chunk interest")?;
    let apply_interest_ms = elapsed_ms(apply_start.elapsed());

    let mut counters = LoopCounters::default();
    let mut snapshots = BTreeMap::new();
    collect_snapshot_events(&mut snapshots, &initial_events);
    counters.observe_events(&initial_events);

    let mut first_view_ready_ms = None;
    let max_duration = Duration::from_secs(config.max_seconds);
    loop {
        let metrics = scheduler.metrics();
        let view_ready = metrics.client_visible_chunks == target_chunks;
        if view_ready && first_view_ready_ms.is_none() {
            first_view_ready_ms = Some(elapsed_ms(total_start.elapsed()));
        }
        if view_ready && server_idle(&scheduler) {
            let report = SnapshotLoadReport {
                apply_interest_ms,
                first_view_ready_ms,
                total_elapsed_ms: elapsed_ms(total_start.elapsed()),
                counters,
                final_metrics: metrics,
            };
            return Ok((report, snapshots.into_values().collect()));
        }
        if total_start.elapsed() > max_duration {
            bail!(
                "timed out after {:.3}s waiting for render_distance={} tracking_radius={} ready={}/{} pending_jobs={} pending_publications={} pending_loads={} pending_saves={}",
                elapsed_ms(total_start.elapsed()) / 1000.0,
                view.render_distance,
                view.chunk_tracking_radius,
                metrics.client_visible_chunks,
                target_chunks,
                scheduler.pending_job_count(),
                scheduler.pending_publication_count(),
                scheduler.pending_persistence_load_count(),
                scheduler.pending_persistence_save_count()
            );
        }

        wait_for_next_completion(&mut scheduler, &mut counters);

        let poll_start = Instant::now();
        let events = scheduler.poll().context("failed to poll scheduler")?;
        let poll_elapsed_ms = elapsed_ms(poll_start.elapsed());
        counters.polls = counters.polls.saturating_add(1);
        counters.poll_call_ms += poll_elapsed_ms;
        counters.max_poll_call_ms = counters.max_poll_call_ms.max(poll_elapsed_ms);
        if events.is_empty() {
            counters.empty_polls = counters.empty_polls.saturating_add(1);
        } else {
            counters.publish_polls = counters.publish_polls.saturating_add(1);
        }
        collect_snapshot_events(&mut snapshots, &events);
        counters.observe_events(&events);

        let unload_start = Instant::now();
        let unloaded = scheduler
            .process_pending_unloads(usize::MAX)
            .context("failed to process pending unloads")?;
        counters.pending_unloads_processed =
            counters.pending_unloads_processed.saturating_add(unloaded);
        counters.pending_unload_ms += elapsed_ms(unload_start.elapsed());
    }
}

fn collect_snapshot_events(
    snapshots: &mut BTreeMap<ChunkPos, ChunkSnapshot>,
    events: &[ChunkSchedulerEvent],
) {
    for event in events {
        if let ChunkSchedulerEvent::SnapshotReady(snapshot) = event {
            snapshots.insert(snapshot.pos, snapshot.clone());
        }
    }
}

fn wait_for_next_completion(scheduler: &mut ChunkScheduler, counters: &mut LoopCounters) {
    let wait_start = Instant::now();
    if scheduler.worldgen_mailbox_pending_count() > 0 {
        counters.worldgen_waits = counters.worldgen_waits.saturating_add(1);
        let _ = scheduler.wait_for_worldgen_completion(Duration::from_millis(COMPLETION_WAIT_MS));
    } else if scheduler.light_status_mailbox_pending_count() > 0 {
        counters.light_waits = counters.light_waits.saturating_add(1);
        let _ = scheduler.wait_for_light_completion(Duration::from_millis(COMPLETION_WAIT_MS));
    } else {
        counters.idle_waits = counters.idle_waits.saturating_add(1);
        thread::yield_now();
    }
    counters.wait_ms += elapsed_ms(wait_start.elapsed());
}

fn server_idle(scheduler: &ChunkScheduler) -> bool {
    scheduler.pending_job_count() == 0
        && scheduler.pending_publication_count() == 0
        && scheduler.pending_unload_count() == 0
        && scheduler.pending_persistence_load_count() == 0
        && scheduler.pending_persistence_save_count() == 0
        && scheduler.pending_external_persistence_request_count() == 0
        && scheduler.worldgen_mailbox_pending_count() == 0
        && scheduler.light_status_mailbox_pending_count() == 0
}

fn validate_persisted_reload(report: &SnapshotLoadReport) -> Result<()> {
    let metrics = report.final_metrics;
    if metrics.completed_jobs != 0 {
        bail!(
            "persisted reload unexpectedly completed {} worldgen jobs",
            metrics.completed_jobs
        );
    }
    if metrics.completed_light_statuses != 0 {
        bail!(
            "persisted reload unexpectedly completed {} light statuses",
            metrics.completed_light_statuses
        );
    }
    if metrics.total_light_status_compute_us != 0 {
        bail!(
            "persisted reload unexpectedly spent {:.3}ms computing light",
            micros_to_ms(metrics.total_light_status_compute_us)
        );
    }
    Ok(())
}

fn target_sections_for_loaded_chunks(
    snapshots: &[ChunkSnapshot],
    target_chunk_positions: &BTreeSet<ChunkPos>,
) -> Result<BTreeSet<RenderSectionKey>> {
    let snapshots_by_pos = snapshots
        .iter()
        .map(|snapshot| (snapshot.pos, snapshot))
        .collect::<BTreeMap<_, _>>();
    let mut sections = BTreeSet::new();
    for pos in target_chunk_positions {
        let snapshot = snapshots_by_pos
            .get(pos)
            .with_context(|| format!("missing loaded target snapshot {},{}", pos.x, pos.z))?;
        for local_y in (0..snapshot.height).step_by(SECTION_HEIGHT as usize) {
            let section_y = block_to_section_coord(snapshot.min_y + local_y);
            sections.insert(RenderSectionKey::new(pos.x, section_y, pos.z));
        }
    }
    Ok(sections)
}

fn square_chunk_positions(center: ChunkPos, radius: u32) -> BTreeSet<ChunkPos> {
    let radius = i32::try_from(radius).expect("validated render distance fits i32");
    let mut positions = BTreeSet::new();
    for z in center.z - radius..=center.z + radius {
        for x in center.x - radius..=center.x + radius {
            positions.insert(ChunkPos::new(x, z));
        }
    }
    positions
}

fn chunk_tracking_radius_for_render_distance(render_distance: u32) -> u32 {
    if render_distance < 2 {
        return render_distance;
    }
    render_distance.saturating_add(1).clamp(3, 33)
}

fn square_count(radius: u32) -> Result<usize> {
    let side = usize::try_from(radius)
        .context("radius does not fit usize")?
        .saturating_mul(2)
        .saturating_add(1);
    Ok(side * side)
}

fn unique_sqlite_world_dir(config: &Config) -> PathBuf {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    env::temp_dir().join(format!(
        "mclone-mesh-cpu-sqlite-rd{}-{}-{timestamp_ms}",
        config.render_distance,
        std::process::id()
    ))
}

fn sqlite_storage_byte_count(world_dir: &Path) -> Option<u64> {
    let database_path = SqliteWorldStore::database_path_for_world_dir(world_dir);
    let mut total = 0_u64;
    let mut found = false;
    for path in [
        database_path.clone(),
        sqlite_auxiliary_path(&database_path, "-wal"),
        sqlite_auxiliary_path(&database_path, "-shm"),
    ] {
        if let Ok(metadata) = fs::metadata(path) {
            total = total.saturating_add(metadata.len());
            found = true;
        }
    }
    found.then_some(total)
}

fn sqlite_auxiliary_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn print_snapshot_source_json(
    indent: &str,
    report: &SnapshotSourceReport,
    trailing_comma: bool,
    target_chunks: usize,
) {
    println!("{indent}\"snapshot_source\": {{");
    println!("{indent}  \"mode\": \"{}\",", report.mode.as_str());
    match &report.world_dir {
        Some(path) => println!(
            "{indent}  \"world_dir\": \"{}\",",
            json_escape(&path.display().to_string())
        ),
        None => println!("{indent}  \"world_dir\": null,"),
    }
    match report.sqlite_storage_bytes {
        Some(bytes) => println!("{indent}  \"sqlite_storage_bytes\": {bytes},"),
        None => println!("{indent}  \"sqlite_storage_bytes\": null,"),
    }
    match report.stored_chunk_records {
        Some(count) => println!("{indent}  \"stored_chunk_records\": {count},"),
        None => println!("{indent}  \"stored_chunk_records\": null,"),
    }
    match &report.prewarm {
        Some(prewarm) => print_snapshot_load_json(
            &format!("{indent}  "),
            "prewarm",
            prewarm,
            true,
            target_chunks,
        ),
        None => println!("{indent}  \"prewarm\": null,"),
    }
    print_snapshot_load_json(
        &format!("{indent}  "),
        "measured",
        &report.measured,
        true,
        target_chunks,
    );
    println!("{indent}  \"snapshot_count\": {}", report.snapshots.len());
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_snapshot_load_json(
    indent: &str,
    name: &str,
    report: &SnapshotLoadReport,
    trailing_comma: bool,
    target_chunks: usize,
) {
    println!("{indent}\"{name}\": {{");
    println!(
        "{indent}  \"apply_interest_ms\": {:.3},",
        report.apply_interest_ms
    );
    match report.first_view_ready_ms {
        Some(ms) => println!("{indent}  \"first_view_ready_ms\": {ms:.3},"),
        None => println!("{indent}  \"first_view_ready_ms\": null,"),
    }
    println!(
        "{indent}  \"total_elapsed_ms\": {:.3},",
        report.total_elapsed_ms
    );
    println!(
        "{indent}  \"view_ready_chunks_per_second\": {:.3},",
        report
            .first_view_ready_ms
            .map_or(0.0, |ms| per_second(target_chunks, ms))
    );
    println!(
        "{indent}  \"settled_chunks_per_second\": {:.3},",
        per_second(target_chunks, report.total_elapsed_ms)
    );
    print_loop_counters_json(&format!("{indent}  "), report.counters, true);
    print_metrics_json(&format!("{indent}  "), report.final_metrics, false);
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_loop_counters_json(indent: &str, counters: LoopCounters, trailing_comma: bool) {
    println!("{indent}\"loop_counters\": {{");
    println!("{indent}  \"polls\": {},", counters.polls);
    println!("{indent}  \"empty_polls\": {},", counters.empty_polls);
    println!("{indent}  \"publish_polls\": {},", counters.publish_polls);
    println!("{indent}  \"worldgen_waits\": {},", counters.worldgen_waits);
    println!("{indent}  \"light_waits\": {},", counters.light_waits);
    println!("{indent}  \"idle_waits\": {},", counters.idle_waits);
    println!("{indent}  \"wait_ms\": {:.3},", counters.wait_ms);
    println!("{indent}  \"poll_call_ms\": {:.3},", counters.poll_call_ms);
    println!(
        "{indent}  \"max_poll_call_ms\": {:.3},",
        counters.max_poll_call_ms
    );
    println!(
        "{indent}  \"pending_unload_ms\": {:.3},",
        counters.pending_unload_ms
    );
    println!(
        "{indent}  \"pending_unloads_processed\": {},",
        counters.pending_unloads_processed
    );
    println!(
        "{indent}  \"snapshot_ready_events\": {},",
        counters.snapshot_ready_events
    );
    println!(
        "{indent}  \"status_changed_events\": {}",
        counters.status_changed_events
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_metrics_json(indent: &str, metrics: ChunkSchedulerMetrics, trailing_comma: bool) {
    println!("{indent}\"metrics\": {{");
    println!(
        "{indent}  \"direct_ticket_chunks\": {},",
        metrics.direct_ticket_chunks
    );
    println!(
        "{indent}  \"active_ticket_chunks\": {},",
        metrics.active_ticket_chunks
    );
    println!("{indent}  \"holder_chunks\": {},", metrics.holder_chunks);
    println!(
        "{indent}  \"client_visible_chunks\": {},",
        metrics.client_visible_chunks
    );
    println!(
        "{indent}  \"loaded_snapshot_chunks\": {},",
        metrics.loaded_snapshot_chunks
    );
    println!("{indent}  \"pending_jobs\": {},", metrics.pending_jobs);
    println!("{indent}  \"completed_jobs\": {},", metrics.completed_jobs);
    println!(
        "{indent}  \"completed_light_statuses\": {},",
        metrics.completed_light_statuses
    );
    println!(
        "{indent}  \"completed_light_batches\": {},",
        metrics.completed_light_batches
    );
    println!(
        "{indent}  \"total_light_status_compute_ms\": {:.3}",
        micros_to_ms(metrics.total_light_status_compute_us)
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_mesh_stats_json(indent: &str, stats: MeshBuildStats, trailing_comma: bool) {
    println!("{indent}\"mesh_build\": {{");
    println!("{indent}  \"build_ms\": {:.3},", stats.build_ms);
    println!("{indent}  \"built_sections\": {},", stats.built_sections);
    println!(
        "{indent}  \"non_empty_sections\": {},",
        stats.non_empty_sections
    );
    println!(
        "{indent}  \"sections_per_second\": {:.3},",
        stats.sections_per_second()
    );
    println!(
        "{indent}  \"non_empty_sections_per_second\": {:.3},",
        stats.non_empty_sections_per_second()
    );
    println!("{indent}  \"vertex_count\": {},", stats.vertex_count);
    println!("{indent}  \"index_count\": {},", stats.index_count);
    println!("{indent}  \"face_count\": {},", stats.face_count);
    println!(
        "{indent}  \"visibility_graph_build_count\": {},",
        stats.visibility_graph_build_count
    );
    println!(
        "{indent}  \"visibility_graph_total_ms\": {:.3},",
        stats.visibility_graph_total_ms
    );
    println!(
        "{indent}  \"visibility_graph_worst_ms\": {:.3}",
        stats.visibility_graph_worst_ms
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_gpu_upload_stats_json(indent: &str, stats: GpuUploadStats, trailing_comma: bool) {
    println!("{indent}\"gpu_upload\": {{");
    println!(
        "{indent}  \"device_create_ms\": {:.3},",
        stats.device_create_ms
    );
    println!(
        "{indent}  \"draw_resource_setup_ms\": {:.3},",
        stats.draw_resource_setup_ms
    );
    println!(
        "{indent}  \"draw_resource_setup_poll_ms\": {:.3},",
        stats.draw_resource_setup_poll_ms
    );
    println!(
        "{indent}  \"update_sections_ms\": {:.3},",
        stats.update_sections_ms
    );
    println!("{indent}  \"device_poll_ms\": {:.3},", stats.device_poll_ms);
    println!(
        "{indent}  \"uploaded_sections\": {},",
        stats.uploaded_sections
    );
    println!(
        "{indent}  \"resident_sections\": {},",
        stats.resident_sections
    );
    println!(
        "{indent}  \"uploaded_vertices\": {},",
        stats.uploaded_vertices
    );
    println!(
        "{indent}  \"uploaded_indices\": {},",
        stats.uploaded_indices
    );
    println!("{indent}  \"uploaded_faces\": {},", stats.uploaded_faces);
    println!("{indent}  \"uploaded_bytes\": {},", stats.uploaded_bytes);
    println!(
        "{indent}  \"resident_indices\": {},",
        stats.resident_indices
    );
    println!("{indent}  \"timing\": {{");
    println!("{indent}    \"total_ms\": {:.3},", stats.timing.total_ms);
    println!(
        "{indent}    \"dirty_mark_ms\": {:.3},",
        stats.timing.dirty_mark_ms
    );
    println!("{indent}    \"remove_ms\": {:.3},", stats.timing.remove_ms);
    println!(
        "{indent}    \"section_state_ms\": {:.3},",
        stats.timing.section_state_ms
    );
    println!(
        "{indent}    \"vertex_bytes_ms\": {:.3},",
        stats.timing.vertex_bytes_ms
    );
    println!(
        "{indent}    \"vertex_buffer_ms\": {:.3},",
        stats.timing.vertex_buffer_ms
    );
    println!(
        "{indent}    \"index_bytes_ms\": {:.3},",
        stats.timing.index_bytes_ms
    );
    println!(
        "{indent}    \"index_buffer_ms\": {:.3},",
        stats.timing.index_buffer_ms
    );
    println!(
        "{indent}    \"mesh_insert_ms\": {:.3},",
        stats.timing.mesh_insert_ms
    );
    println!(
        "{indent}    \"mesh_upload_worst_ms\": {:.3}",
        stats.timing.mesh_upload_worst_ms
    );
    println!("{indent}  }}");
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let value = args
        .next()
        .with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|error| anyhow::anyhow!("{flag} received invalid value `{value}`: {error}"))
}

fn parse_bool_next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<bool> {
    let value = args
        .next()
        .with_context(|| format!("{flag} requires true or false"))?;
    match value.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("{flag} received invalid boolean `{value}`"),
    }
}

fn usage() -> String {
    "usage: mesh_cpu_perf [--seed N] [--chunk-x N] [--chunk-z N] [--render-distance N] [--chunk-tracking-radius N] [--lighting true|false] [--snapshot-source fresh|persisted-sqlite-reload] [--gpu-upload] [--max-seconds N]"
        .to_owned()
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn per_second(count: usize, elapsed_ms: f64) -> f64 {
    if elapsed_ms <= f64::EPSILON {
        0.0
    } else {
        count as f64 / (elapsed_ms / 1000.0)
    }
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

fn print_benchmark_metadata(name: &str, indent: &str, trailing_comma: bool) {
    println!("{indent}\"benchmark\": \"{}\",", json_escape(name));
    println!(
        "{indent}\"recorded_unix_seconds\": {},",
        current_unix_seconds()
    );
    println!(
        "{indent}\"git_commit\": \"{}\",",
        json_escape(&git_short_commit())
    );
    println!("{indent}\"git_dirty\": {},", git_dirty());
    let suffix = if trailing_comma { "," } else { "" };
    println!(
        "{indent}\"debug_assertions\": {}{suffix}",
        cfg!(debug_assertions)
    );
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_short_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
