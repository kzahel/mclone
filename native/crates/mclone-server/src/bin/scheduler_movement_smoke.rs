use std::{env, thread, time::Duration, time::Instant};

use mclone_core::ChunkPos;
use mclone_protocol::ChunkInterest;
use mclone_server::{
    ChunkScheduler, ChunkSchedulerEvent, ChunkSchedulerMetrics, MAX_CHUNK_DISTANCE,
    PLAYER_TICKET_LEVEL,
};
use mclone_worldgen::levelgen::OverworldFeatureBatchTiming;

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_RADIUS_CHUNKS: u32 = 1;
const DEFAULT_STEPS: usize = 3;
const DEFAULT_MAX_POLLS: usize = 120_000;
const DEFAULT_POLL_SLEEP_MS: u64 = 1;
const DEFAULT_COMPLETION_WAIT_MS: u64 = 30_000;
const DEFAULT_POLL_MODE: PollMode = PollMode::Completion;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args().skip(1))?;
    let total_start = Instant::now();
    let mut scheduler = ChunkScheduler::new(config.seed);
    let mut steps = Vec::with_capacity(config.steps);

    for index in 0..config.steps {
        let previous_job_count = scheduler.job_count();
        let center = ChunkPos::new(i32::try_from(index).expect("step index exceeded i32"), 0);
        let step_start = Instant::now();
        let apply_start = Instant::now();
        let mut events = scheduler
            .apply_interest(ChunkInterest {
                center,
                radius_chunks: config.radius_chunks,
            })
            .map_err(|error| error.to_string())?;
        let apply_interest_ms = elapsed_ms(apply_start.elapsed());

        let mut polls = 0_usize;
        let mut empty_polls = 0_usize;
        let mut publish_polls = 0_usize;
        let mut poll_call_ms = 0.0_f64;
        let mut empty_poll_call_ms = 0.0_f64;
        let mut publish_poll_call_ms = 0.0_f64;
        let mut completion_wait_ms = 0.0_f64;
        let mut max_poll_call_ms = 0.0_f64;
        let mut completion_waits = 0_usize;
        let mut completion_timeouts = 0_usize;
        let worker_wait_start = Instant::now();
        while scheduler.pending_job_count() > 0 {
            if polls >= config.max_polls {
                return Err(format!(
                    "timed out waiting for step {index} center=({}, {}) after {polls} polls",
                    center.x, center.z
                ));
            }
            match config.poll_mode {
                PollMode::Completion => {
                    completion_waits += 1;
                    let completion_wait_start = Instant::now();
                    let completed = scheduler.wait_for_worldgen_completion(Duration::from_millis(
                        config.completion_wait_ms,
                    ));
                    completion_wait_ms += elapsed_ms(completion_wait_start.elapsed());
                    if !completed {
                        completion_timeouts += 1;
                    }
                }
                PollMode::Sleep => {
                    if config.poll_sleep_ms > 0 {
                        thread::sleep(Duration::from_millis(config.poll_sleep_ms));
                    }
                }
                PollMode::Spin => {}
            }
            let poll_start = Instant::now();
            let poll_events = scheduler.poll().map_err(|error| error.to_string())?;
            let poll_elapsed_ms = elapsed_ms(poll_start.elapsed());
            poll_call_ms += poll_elapsed_ms;
            max_poll_call_ms = max_poll_call_ms.max(poll_elapsed_ms);
            if poll_events.is_empty() {
                empty_polls += 1;
                empty_poll_call_ms += poll_elapsed_ms;
            } else {
                publish_polls += 1;
                publish_poll_call_ms += poll_elapsed_ms;
            }
            events.extend(poll_events);
            polls += 1;
        }
        let worker_wait_ms = elapsed_ms(worker_wait_start.elapsed());
        let non_poll_wait_ms = (worker_wait_ms - poll_call_ms).max(0.0);

        let snapshots = events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
            .count();
        let unloads = events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::Unloaded { .. }))
            .count();
        let status_events = events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::StatusChanged { .. }))
            .count();
        let (feature_jobs, feature_timing) =
            feature_timing_since_job_count(&scheduler, previous_job_count);

        steps.push(StepReport {
            index,
            center,
            elapsed_ms: elapsed_ms(step_start.elapsed()),
            apply_interest_ms,
            worker_wait_ms,
            poll_call_ms,
            empty_poll_call_ms,
            publish_poll_call_ms,
            completion_wait_ms,
            non_poll_wait_ms,
            max_poll_call_ms,
            polls,
            empty_polls,
            publish_polls,
            completion_waits,
            completion_timeouts,
            scheduler_events: events.len(),
            client_updates: snapshots + unloads,
            status_events,
            feature_jobs,
            feature_timing,
            snapshots,
            unloads,
            metrics: scheduler.metrics(),
        });
    }

    let total_elapsed_ms = elapsed_ms(total_start.elapsed());
    let report = SmokeReport {
        config,
        total_elapsed_ms,
        steps,
    };
    report.validate()?;
    report.print_json();
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Config {
    seed: i64,
    radius_chunks: u32,
    steps: usize,
    max_polls: usize,
    poll_mode: PollMode,
    poll_sleep_ms: u64,
    completion_wait_ms: u64,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            radius_chunks: DEFAULT_RADIUS_CHUNKS,
            steps: DEFAULT_STEPS,
            max_polls: DEFAULT_MAX_POLLS,
            poll_mode: DEFAULT_POLL_MODE,
            poll_sleep_ms: DEFAULT_POLL_SLEEP_MS,
            completion_wait_ms: DEFAULT_COMPLETION_WAIT_MS,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seed" => {
                    config.seed = parse_next(&mut args, "--seed")?;
                }
                "--radius" | "--radius-chunks" => {
                    config.radius_chunks = parse_next(&mut args, "--radius")?;
                }
                "--steps" => {
                    config.steps = parse_next(&mut args, "--steps")?;
                }
                "--max-polls" => {
                    config.max_polls = parse_next(&mut args, "--max-polls")?;
                }
                "--poll-mode" => {
                    config.poll_mode = parse_next(&mut args, "--poll-mode")?;
                }
                "--poll-sleep-ms" => {
                    config.poll_sleep_ms = parse_next(&mut args, "--poll-sleep-ms")?;
                }
                "--completion-wait-ms" => {
                    config.completion_wait_ms = parse_next(&mut args, "--completion-wait-ms")?;
                }
                "--help" | "-h" => {
                    return Err(usage());
                }
                _ => return Err(format!("unknown argument {arg}\n{}", usage())),
            }
        }

        if config.steps == 0 {
            return Err("--steps must be greater than zero".to_owned());
        }

        Ok(config)
    }
}

#[derive(Clone, Copy, Debug)]
struct StepReport {
    index: usize,
    center: ChunkPos,
    elapsed_ms: f64,
    apply_interest_ms: f64,
    worker_wait_ms: f64,
    poll_call_ms: f64,
    empty_poll_call_ms: f64,
    publish_poll_call_ms: f64,
    completion_wait_ms: f64,
    non_poll_wait_ms: f64,
    max_poll_call_ms: f64,
    polls: usize,
    empty_polls: usize,
    publish_polls: usize,
    completion_waits: usize,
    completion_timeouts: usize,
    scheduler_events: usize,
    client_updates: usize,
    status_events: usize,
    feature_jobs: usize,
    feature_timing: OverworldFeatureBatchTiming,
    snapshots: usize,
    unloads: usize,
    metrics: ChunkSchedulerMetrics,
}

#[derive(Debug)]
struct SmokeReport {
    config: Config,
    total_elapsed_ms: f64,
    steps: Vec<StepReport>,
}

impl SmokeReport {
    fn validate(&self) -> Result<(), String> {
        let interest_chunks = square_count(self.config.radius_chunks)?;
        let movement_strip_chunks =
            usize::try_from(self.config.radius_chunks).expect("radius fits usize") * 2 + 1;
        let active_chunks = active_square_count(self.config.radius_chunks)?;

        for step in &self.steps {
            if step.metrics.direct_ticket_chunks != interest_chunks {
                return Err(format!(
                    "step {} direct_ticket_chunks={} expected {interest_chunks}",
                    step.index, step.metrics.direct_ticket_chunks
                ));
            }
            if step.metrics.active_ticket_chunks != active_chunks {
                return Err(format!(
                    "step {} active_ticket_chunks={} expected {active_chunks}",
                    step.index, step.metrics.active_ticket_chunks
                ));
            }
            if step.metrics.holder_chunks != active_chunks {
                return Err(format!(
                    "step {} holder_chunks={} expected {active_chunks}",
                    step.index, step.metrics.holder_chunks
                ));
            }
            if step.metrics.client_visible_chunks != interest_chunks {
                return Err(format!(
                    "step {} client_visible_chunks={} expected {interest_chunks}",
                    step.index, step.metrics.client_visible_chunks
                ));
            }
            if step.metrics.pending_jobs != 0 {
                return Err(format!(
                    "step {} still has {} pending jobs",
                    step.index, step.metrics.pending_jobs
                ));
            }

            let expected_snapshots = if step.index == 0 {
                interest_chunks
            } else {
                movement_strip_chunks
            };
            let expected_unloads = if step.index == 0 {
                0
            } else {
                movement_strip_chunks
            };
            if step.snapshots != expected_snapshots {
                return Err(format!(
                    "step {} snapshots={} expected {expected_snapshots}",
                    step.index, step.snapshots
                ));
            }
            if step.unloads != expected_unloads {
                return Err(format!(
                    "step {} unloads={} expected {expected_unloads}",
                    step.index, step.unloads
                ));
            }
        }

        let final_metrics = self
            .steps
            .last()
            .expect("validated config guarantees at least one step")
            .metrics;
        if self.config.steps > 1 && final_metrics.total_seeded_dependency_chunks == 0 {
            return Err("movement did not seed any scheduler-owned dependency chunks".to_owned());
        }
        if final_metrics.total_seeded_dependency_chunks != final_metrics.total_dependency_cache_hits
        {
            return Err(format!(
                "seeded dependency chunks {} did not match worker cache hits {}",
                final_metrics.total_seeded_dependency_chunks,
                final_metrics.total_dependency_cache_hits
            ));
        }

        Ok(())
    }

    fn print_json(&self) {
        let interest_chunks = square_count(self.config.radius_chunks).expect("validated radius");
        let active_chunks =
            active_square_count(self.config.radius_chunks).expect("validated radius");
        println!("{{");
        println!("  \"seed\": {},", self.config.seed);
        println!("  \"radius_chunks\": {},", self.config.radius_chunks);
        println!("  \"steps\": {},", self.config.steps);
        println!("  \"poll_mode\": \"{}\",", self.config.poll_mode.as_str());
        println!("  \"poll_sleep_ms\": {},", self.config.poll_sleep_ms);
        println!(
            "  \"completion_wait_ms\": {},",
            self.config.completion_wait_ms
        );
        println!("  \"total_elapsed_ms\": {:.3},", self.total_elapsed_ms);
        println!("  \"expected_interest_chunks\": {interest_chunks},");
        println!("  \"expected_active_chunks\": {active_chunks},");
        println!("  \"step_reports\": [");
        for (index, step) in self.steps.iter().enumerate() {
            let suffix = if index + 1 == self.steps.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"index\": {},", step.index);
            println!(
                "      \"center\": {{ \"x\": {}, \"z\": {} }},",
                step.center.x, step.center.z
            );
            println!("      \"elapsed_ms\": {:.3},", step.elapsed_ms);
            println!(
                "      \"apply_interest_ms\": {:.3},",
                step.apply_interest_ms
            );
            println!(
                "      \"main_thread_scheduler_ms\": {:.3},",
                step.apply_interest_ms + step.poll_call_ms
            );
            println!(
                "      \"main_thread_publish_path_ms\": {:.3},",
                step.apply_interest_ms + step.publish_poll_call_ms
            );
            println!(
                "      \"main_thread_blocked_ms\": {:.3},",
                step.apply_interest_ms + step.completion_wait_ms + step.poll_call_ms
            );
            println!(
                "      \"max_main_thread_call_ms\": {:.3},",
                step.apply_interest_ms.max(step.max_poll_call_ms)
            );
            println!("      \"worker_wait_ms\": {:.3},", step.worker_wait_ms);
            println!("      \"poll_call_ms\": {:.3},", step.poll_call_ms);
            println!(
                "      \"empty_poll_call_ms\": {:.3},",
                step.empty_poll_call_ms
            );
            println!(
                "      \"publish_poll_call_ms\": {:.3},",
                step.publish_poll_call_ms
            );
            println!(
                "      \"completion_wait_ms\": {:.3},",
                step.completion_wait_ms
            );
            println!("      \"non_poll_wait_ms\": {:.3},", step.non_poll_wait_ms);
            println!("      \"max_poll_call_ms\": {:.3},", step.max_poll_call_ms);
            println!("      \"polls\": {},", step.polls);
            println!("      \"empty_polls\": {},", step.empty_polls);
            println!("      \"publish_polls\": {},", step.publish_polls);
            println!("      \"completion_waits\": {},", step.completion_waits);
            println!(
                "      \"completion_timeouts\": {},",
                step.completion_timeouts
            );
            println!("      \"scheduler_events\": {},", step.scheduler_events);
            println!("      \"client_updates\": {},", step.client_updates);
            println!("      \"status_events\": {},", step.status_events);
            println!("      \"feature_jobs\": {},", step.feature_jobs);
            print_timing_json("      ", step.feature_timing, true);
            println!("      \"snapshots\": {},", step.snapshots);
            println!("      \"unloads\": {},", step.unloads);
            print_metrics_json("      ", step.metrics, false);
            println!("    }}{suffix}");
        }
        println!("  ],");
        let final_metrics = self
            .steps
            .last()
            .expect("validated config guarantees at least one step")
            .metrics;
        print_metrics_json("  ", final_metrics, false);
        println!("}}");
    }
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
        "{indent}  \"total_retained_dependency_chunks\": {}",
        metrics.total_retained_dependency_chunks
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_timing_json(indent: &str, timing: OverworldFeatureBatchTiming, trailing_comma: bool) {
    println!("{indent}\"feature_timing\": {{");
    println!(
        "{indent}  \"total_ms\": {:.3},",
        micros_to_ms(timing.total_us())
    );
    println!(
        "{indent}  \"seed_dependency_insert_ms\": {:.3},",
        micros_to_ms(timing.seed_dependency_insert_us)
    );
    println!(
        "{indent}  \"plan_ms\": {:.3},",
        micros_to_ms(timing.plan_us)
    );
    println!(
        "{indent}  \"dependency_cache_hit_clone_ms\": {:.3},",
        micros_to_ms(timing.dependency_cache_hit_clone_us)
    );
    println!(
        "{indent}  \"dependency_generate_ms\": {:.3},",
        micros_to_ms(timing.dependency_generate_us)
    );
    println!(
        "{indent}  \"dependency_insert_clone_ms\": {:.3},",
        micros_to_ms(timing.dependency_insert_clone_us)
    );
    println!(
        "{indent}  \"dependency_retain_ms\": {:.3},",
        micros_to_ms(timing.dependency_retain_us)
    );
    println!(
        "{indent}  \"retained_dependency_clone_ms\": {:.3},",
        micros_to_ms(timing.retained_dependency_clone_us)
    );
    println!(
        "{indent}  \"feature_region_init_ms\": {:.3},",
        micros_to_ms(timing.feature_region_init_us)
    );
    println!(
        "{indent}  \"feature_decoration_ms\": {:.3},",
        micros_to_ms(timing.feature_decoration_us)
    );
    println!(
        "{indent}  \"target_extract_ms\": {:.3}",
        micros_to_ms(timing.target_extract_us)
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn feature_timing_since_job_count(
    scheduler: &ChunkScheduler,
    previous_job_count: usize,
) -> (usize, OverworldFeatureBatchTiming) {
    let mut timing = OverworldFeatureBatchTiming::default();
    let mut jobs = 0;
    for job in scheduler.jobs().skip(previous_job_count) {
        if let Some(job_timing) = scheduler.job_timing(job.id) {
            timing.add_assign(job_timing);
            jobs += 1;
        }
    }
    (jobs, timing)
}

fn square_count(radius_chunks: u32) -> Result<usize, String> {
    let width = usize::try_from(radius_chunks)
        .map_err(|_| "radius did not fit in usize".to_owned())?
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "radius width overflowed usize".to_owned())?;
    width
        .checked_mul(width)
        .ok_or_else(|| "radius square overflowed usize".to_owned())
}

fn active_square_count(radius_chunks: u32) -> Result<usize, String> {
    let ticket_radius = u32::try_from(MAX_CHUNK_DISTANCE - PLAYER_TICKET_LEVEL)
        .expect("ticket radius must fit in u32");
    square_count(
        radius_chunks
            .checked_add(ticket_radius)
            .ok_or_else(|| "active radius overflowed u32".to_owned())?,
    )
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1_000.0
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{name} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|_| format!("invalid value for {name}: {value}"))
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
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "completion" | "wait" => Ok(Self::Completion),
            "sleep" => Ok(Self::Sleep),
            "spin" => Ok(Self::Spin),
            _ => Err(()),
        }
    }
}

fn usage() -> String {
    "usage: scheduler_movement_smoke [--seed N] [--radius N] [--steps N] [--max-polls N] [--poll-mode completion|sleep|spin] [--poll-sleep-ms N] [--completion-wait-ms N]".to_owned()
}
