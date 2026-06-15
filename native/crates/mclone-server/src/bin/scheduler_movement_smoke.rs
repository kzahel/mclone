use std::{env, thread, time::Duration, time::Instant};

use mclone_core::ChunkPos;
use mclone_protocol::{ChunkInterest, ClientCommand, ServerUpdate};
use mclone_server::{
    ChunkSchedulerMetrics, IntegratedServer, MAX_CHUNK_DISTANCE, PLAYER_TICKET_LEVEL,
};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_RADIUS_CHUNKS: u32 = 1;
const DEFAULT_STEPS: usize = 3;
const DEFAULT_MAX_POLLS: usize = 120_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args().skip(1))?;
    let total_start = Instant::now();
    let mut server = IntegratedServer::new(config.seed);
    let mut steps = Vec::with_capacity(config.steps);

    for index in 0..config.steps {
        let center = ChunkPos::new(i32::try_from(index).expect("step index exceeded i32"), 0);
        let step_start = Instant::now();
        let mut updates = server.handle_command(ClientCommand::SetChunkInterest(ChunkInterest {
            center,
            radius_chunks: config.radius_chunks,
        }));

        let mut polls = 0_usize;
        while server.pending_job_count() > 0 {
            if polls >= config.max_polls {
                return Err(format!(
                    "timed out waiting for step {index} center=({}, {}) after {polls} polls",
                    center.x, center.z
                ));
            }
            thread::sleep(Duration::from_millis(1));
            updates.extend(server.poll());
            polls += 1;
        }

        let snapshots = updates
            .iter()
            .filter(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
            .count();
        let unloads = updates
            .iter()
            .filter(|update| matches!(update, ServerUpdate::ChunkUnload { .. }))
            .count();

        steps.push(StepReport {
            index,
            center,
            elapsed_ms: elapsed_ms(step_start.elapsed()),
            polls,
            updates: updates.len(),
            snapshots,
            unloads,
            metrics: server.scheduler().metrics(),
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
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            radius_chunks: DEFAULT_RADIUS_CHUNKS,
            steps: DEFAULT_STEPS,
            max_polls: DEFAULT_MAX_POLLS,
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
    polls: usize,
    updates: usize,
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
            println!("      \"polls\": {},", step.polls);
            println!("      \"updates\": {},", step.updates);
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

fn usage() -> String {
    "usage: scheduler_movement_smoke [--seed N] [--radius N] [--steps N] [--max-polls N]".to_owned()
}
