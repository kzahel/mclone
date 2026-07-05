use std::{
    env,
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use mclone_core::ChunkPos;
use mclone_protocol::ChunkView;
use mclone_server::{ChunkScheduler, ChunkSchedulerEvent, ChunkSchedulerMetrics};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_RENDER_DISTANCE: u32 = 10;
const DEFAULT_MAX_SECONDS: u64 = 120;
const DEFAULT_POLL_MODE: PollMode = PollMode::Completion;
const COMPLETION_WAIT_MS: u64 = 30_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args().skip(1))?;
    let center = ChunkPos::new(config.chunk_x, config.chunk_z);
    let chunk_tracking_radius = config
        .chunk_tracking_radius
        .unwrap_or_else(|| chunk_tracking_radius_for_render_distance(config.render_distance));
    let target_chunks = square_count(chunk_tracking_radius)?;
    let view = ChunkView {
        center,
        render_distance: config.render_distance,
        chunk_tracking_radius,
    };

    let mut scheduler = ChunkScheduler::new(config.seed);
    scheduler.set_lighting_enabled(config.lighting_enabled);

    let total_start = Instant::now();
    let apply_start = Instant::now();
    let initial_events = scheduler
        .apply_interest(view.clone())
        .map_err(|error| error.to_string())?;
    let apply_interest_ms = elapsed_ms(apply_start.elapsed());

    let mut counters = LoopCounters::default();
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
            let total_elapsed_ms = elapsed_ms(total_start.elapsed());
            let report = Report {
                config,
                center,
                render_distance: view.render_distance,
                chunk_tracking_radius,
                target_chunks,
                apply_interest_ms,
                first_view_ready_ms,
                total_elapsed_ms,
                counters,
                final_metrics: metrics,
            };
            report.print_json();
            return Ok(());
        }
        if total_start.elapsed() > max_duration {
            return Err(format!(
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
            ));
        }

        match config.poll_mode {
            PollMode::Completion => wait_for_next_completion(&mut scheduler, &mut counters),
            PollMode::Sleep => thread::sleep(Duration::from_millis(1)),
            PollMode::Spin => {}
        }

        let poll_start = Instant::now();
        let events = scheduler.poll().map_err(|error| error.to_string())?;
        let poll_elapsed_ms = elapsed_ms(poll_start.elapsed());
        counters.polls = counters.polls.saturating_add(1);
        counters.poll_call_ms += poll_elapsed_ms;
        counters.max_poll_call_ms = counters.max_poll_call_ms.max(poll_elapsed_ms);
        if events.is_empty() {
            counters.empty_polls = counters.empty_polls.saturating_add(1);
        } else {
            counters.publish_polls = counters.publish_polls.saturating_add(1);
        }
        counters.observe_events(&events);

        let unload_start = Instant::now();
        let unloaded = scheduler
            .process_pending_unloads(usize::MAX)
            .map_err(|error| error.to_string())?;
        counters.pending_unloads_processed =
            counters.pending_unloads_processed.saturating_add(unloaded);
        counters.pending_unload_ms += elapsed_ms(unload_start.elapsed());
    }
}

#[derive(Clone, Copy, Debug)]
struct Config {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    render_distance: u32,
    chunk_tracking_radius: Option<u32>,
    lighting_enabled: bool,
    poll_mode: PollMode,
    max_seconds: u64,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            render_distance: DEFAULT_RENDER_DISTANCE,
            chunk_tracking_radius: None,
            lighting_enabled: true,
            poll_mode: DEFAULT_POLL_MODE,
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
                "--poll-mode" => config.poll_mode = parse_next(&mut args, "--poll-mode")?,
                "--max-seconds" => config.max_seconds = parse_next(&mut args, "--max-seconds")?,
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument {arg}\n{}", usage())),
            }
        }

        if config.render_distance > 33 {
            return Err("--render-distance must be <= 33".to_owned());
        }
        if matches!(config.chunk_tracking_radius, Some(radius) if radius > 33) {
            return Err("--chunk-tracking-radius must be <= 33".to_owned());
        }
        if config.max_seconds == 0 {
            return Err("--max-seconds must be greater than zero".to_owned());
        }

        Ok(config)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PollMode {
    Completion,
    Sleep,
    Spin,
}

impl PollMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Completion => "completion",
            Self::Sleep => "sleep",
            Self::Spin => "spin",
        }
    }
}

impl std::str::FromStr for PollMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "completion" => Ok(Self::Completion),
            "sleep" => Ok(Self::Sleep),
            "spin" => Ok(Self::Spin),
            _ => Err(format!(
                "invalid poll mode `{value}`; expected completion, sleep, or spin"
            )),
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
    scheduler_events: usize,
    snapshot_ready_events: usize,
    status_changed_events: usize,
    unload_events: usize,
}

impl LoopCounters {
    fn observe_events(&mut self, events: &[ChunkSchedulerEvent]) {
        self.scheduler_events = self.scheduler_events.saturating_add(events.len());
        for event in events {
            match event {
                ChunkSchedulerEvent::SnapshotReady(_) => {
                    self.snapshot_ready_events = self.snapshot_ready_events.saturating_add(1);
                }
                ChunkSchedulerEvent::StatusChanged { .. } => {
                    self.status_changed_events = self.status_changed_events.saturating_add(1);
                }
                ChunkSchedulerEvent::Unloaded { .. } => {
                    self.unload_events = self.unload_events.saturating_add(1);
                }
                ChunkSchedulerEvent::HolderUnloaded { .. }
                | ChunkSchedulerEvent::EntityChunkLoaded { .. }
                | ChunkSchedulerEvent::SectionBlockUpdates { .. }
                | ChunkSchedulerEvent::BlockTickScheduled { .. }
                | ChunkSchedulerEvent::FluidTickScheduled { .. } => {}
            }
        }
    }
}

#[derive(Debug)]
struct Report {
    config: Config,
    center: ChunkPos,
    render_distance: u32,
    chunk_tracking_radius: u32,
    target_chunks: usize,
    apply_interest_ms: f64,
    first_view_ready_ms: Option<f64>,
    total_elapsed_ms: f64,
    counters: LoopCounters,
    final_metrics: ChunkSchedulerMetrics,
}

impl Report {
    fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_scheduler_loading_ceiling", "  ", true);
        println!("  \"seed\": {},", self.config.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.center.x, self.center.z
        );
        println!("  \"render_distance\": {},", self.render_distance);
        println!(
            "  \"chunk_tracking_radius\": {},",
            self.chunk_tracking_radius
        );
        println!("  \"target_chunks\": {},", self.target_chunks);
        println!("  \"lighting_enabled\": {},", self.config.lighting_enabled);
        println!("  \"poll_mode\": \"{}\",", self.config.poll_mode.as_str());
        println!("  \"apply_interest_ms\": {:.3},", self.apply_interest_ms);
        match self.first_view_ready_ms {
            Some(ms) => println!("  \"first_view_ready_ms\": {ms:.3},"),
            None => println!("  \"first_view_ready_ms\": null,"),
        }
        println!("  \"total_elapsed_ms\": {:.3},", self.total_elapsed_ms);
        println!(
            "  \"view_ready_chunks_per_second\": {:.3},",
            self.first_view_ready_ms
                .map_or(0.0, |ms| chunks_per_second(self.target_chunks, ms))
        );
        println!(
            "  \"settled_chunks_per_second\": {:.3},",
            chunks_per_second(self.target_chunks, self.total_elapsed_ms)
        );
        print_counters_json("  ", self.counters, true);
        print_metrics_json("  ", self.final_metrics, false);
        println!("}}");
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

fn chunk_tracking_radius_for_render_distance(render_distance: u32) -> u32 {
    if render_distance < 2 {
        return render_distance;
    }
    render_distance.saturating_add(1).clamp(3, 33)
}

fn square_count(radius: u32) -> Result<usize, String> {
    let side = usize::try_from(radius)
        .map_err(|_| "radius does not fit usize")?
        .saturating_mul(2)
        .saturating_add(1);
    Ok(side * side)
}

fn print_counters_json(indent: &str, counters: LoopCounters, trailing_comma: bool) {
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
        "{indent}  \"scheduler_events\": {},",
        counters.scheduler_events
    );
    println!(
        "{indent}  \"snapshot_ready_events\": {},",
        counters.snapshot_ready_events
    );
    println!(
        "{indent}  \"status_changed_events\": {},",
        counters.status_changed_events
    );
    println!("{indent}  \"unload_events\": {}", counters.unload_events);
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
    println!(
        "{indent}  \"dependency_holder_chunks\": {},",
        metrics.dependency_holder_chunks
    );
    println!(
        "{indent}  \"ready_dependency_chunks\": {},",
        metrics.ready_dependency_chunks
    );
    println!("{indent}  \"dirty_chunks\": {},", metrics.dirty_chunks);
    println!("{indent}  \"pending_jobs\": {},", metrics.pending_jobs);
    println!("{indent}  \"completed_jobs\": {},", metrics.completed_jobs);
    println!(
        "{indent}  \"total_seeded_dependency_chunks\": {},",
        metrics.total_seeded_dependency_chunks
    );
    println!(
        "{indent}  \"total_dependency_cache_hits\": {},",
        metrics.total_dependency_cache_hits
    );
    println!(
        "{indent}  \"total_dependency_cache_misses\": {},",
        metrics.total_dependency_cache_misses
    );
    println!(
        "{indent}  \"total_retained_dependency_chunks\": {},",
        metrics.total_retained_dependency_chunks
    );
    println!(
        "{indent}  \"max_feature_job_target_chunks\": {},",
        metrics.max_feature_job_target_chunks
    );
    println!(
        "{indent}  \"max_feature_job_feature_centers\": {},",
        metrics.max_feature_job_feature_centers
    );
    println!(
        "{indent}  \"max_feature_job_dependency_chunks\": {},",
        metrics.max_feature_job_dependency_chunks
    );
    println!(
        "{indent}  \"completed_light_statuses\": {},",
        metrics.completed_light_statuses
    );
    println!(
        "{indent}  \"completed_light_batches\": {},",
        metrics.completed_light_batches
    );
    println!(
        "{indent}  \"total_light_status_compute_ms\": {:.3},",
        micros_to_ms(metrics.total_light_status_compute_us)
    );
    println!(
        "{indent}  \"max_light_status_compute_ms\": {:.3},",
        micros_to_ms(metrics.max_light_status_compute_us)
    );
    println!(
        "{indent}  \"total_light_status_run_updates_ms\": {:.3}",
        micros_to_ms(metrics.total_light_status_run_updates_us)
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|_| format!("{flag} received invalid value `{value}`"))
}

fn parse_bool_next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<bool, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires true or false"))?;
    match value.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("{flag} received invalid boolean `{value}`")),
    }
}

fn usage() -> String {
    "usage: scheduler_loading_perf [--seed N] [--chunk-x N] [--chunk-z N] [--render-distance N] [--chunk-tracking-radius N] [--lighting true|false] [--poll-mode completion|sleep|spin] [--max-seconds N]"
        .to_owned()
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn chunks_per_second(chunks: usize, elapsed_ms: f64) -> f64 {
    if elapsed_ms <= f64::EPSILON {
        0.0
    } else {
        chunks as f64 / (elapsed_ms / 1000.0)
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
