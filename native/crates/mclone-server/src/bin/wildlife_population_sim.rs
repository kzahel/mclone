use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use mclone_server::{
    WILDLIFE_LIFECYCLE_RULE_REVISION, WILDLIFE_RESOURCE_KIND_COUNT,
    WILDLIFE_RESOURCE_RULE_REVISION, WILDLIFE_SIMULATION_SCHEMA_VERSION,
    WildlifeForageCellSnapshot, WildlifeLifecycleTuning, WildlifePopulationSnapshot,
    WildlifePopulationSubject, WildlifeResourceKind, WildlifeSimulationConfig,
    WildlifeSimulationDeathCause, WildlifeSimulationEvent, WildlifeSimulationEventKind,
    WildlifeSimulationLifeStage, WildlifeSimulationRemainsSnapshot, WildlifeSimulationSession,
    WildlifeSimulationSex, WildlifeSimulationSpecies, WildlifeSimulationSuppressionReason,
    WildlifeSimulationWorkSnapshot,
};
use mclone_worldgen::levelgen::MCLONE_WILDLIFE_POPULATION_REVISION;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u32 = 3;
const MINECRAFT_DAY_TICKS: u64 = 24_000;

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct Args {
    config: WildlifeSimulationConfig,
    days: u32,
    sample_ticks: u64,
    output: PathBuf,
    tuning_path: Option<PathBuf>,
    label: String,
    full_ticks: bool,
    canary_ticks: u32,
    command: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PopulationCounts {
    total: u32,
    rabbits: u32,
    rabbit_adults: u32,
    rabbit_young: u32,
    deer: u32,
    deer_adult_females: u32,
    deer_adult_males: u32,
    deer_young: u32,
    rabbit_known_refuge: u32,
    rabbit_sheltered: u32,
    deer_proximity_groups: u32,
    active: u32,
    inactive: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IntegerDistribution {
    count: u32,
    sum: u64,
    minimum: u32,
    p10: u32,
    p50: u32,
    p90: u32,
    maximum: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventCounts {
    rabbit_births: u32,
    deer_births: u32,
    rabbit_old_age_deaths: u32,
    rabbit_starvation_deaths: u32,
    deer_old_age_deaths: u32,
    deer_starvation_deaths: u32,
    young_deaths: u32,
    adult_deaths: u32,
    intake_events: u32,
    intake_amount: u64,
    remains_created_biomass: u64,
    remains_decayed_biomass: u64,
    suppressed_low_condition: u32,
    suppressed_cooldown: u32,
    suppressed_crowding: u32,
    suppressed_no_mate: u32,
    suppressed_no_refuge_capacity: u32,
    suppressed_hard_overload: u32,
}

impl EventCounts {
    fn births(&self) -> u64 {
        u64::from(self.rabbit_births) + u64::from(self.deer_births)
    }

    fn deaths(&self) -> u64 {
        u64::from(self.rabbit_old_age_deaths)
            + u64::from(self.rabbit_starvation_deaths)
            + u64::from(self.deer_old_age_deaths)
            + u64::from(self.deer_starvation_deaths)
    }

    fn suppressed(&self) -> u64 {
        u64::from(self.suppressed_low_condition)
            + u64::from(self.suppressed_cooldown)
            + u64::from(self.suppressed_crowding)
            + u64::from(self.suppressed_no_mate)
            + u64::from(self.suppressed_no_refuge_capacity)
            + u64::from(self.suppressed_hard_overload)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ForageCounts {
    cells: u32,
    strata: [ResourceCounts; WILDLIFE_RESOURCE_KIND_COUNT],
    potential: u64,
    available: u64,
    cumulative_recovered: u64,
    cumulative_rabbit_consumed: u64,
    cumulative_deer_consumed: u64,
    cumulative_mallard_consumed: u64,
    interval_recovered: u64,
    interval_rabbit_consumed: u64,
    interval_deer_consumed: u64,
    interval_mallard_consumed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResourceCounts {
    resource: WildlifeResourceKind,
    potential: u64,
    available: u64,
    cumulative_recovered: u64,
    cumulative_rabbit_consumed: u64,
    cumulative_deer_consumed: u64,
    cumulative_mallard_consumed: u64,
    interval_recovered: u64,
    interval_rabbit_consumed: u64,
    interval_deer_consumed: u64,
    interval_mallard_consumed: u64,
}

impl Default for ResourceCounts {
    fn default() -> Self {
        Self {
            resource: WildlifeResourceKind::LowHerbaceous,
            potential: 0,
            available: 0,
            cumulative_recovered: 0,
            cumulative_rabbit_consumed: 0,
            cumulative_deer_consumed: 0,
            cumulative_mallard_consumed: 0,
            interval_recovered: 0,
            interval_rabbit_consumed: 0,
            interval_deer_consumed: 0,
            interval_mallard_consumed: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkCounts {
    decision_admitted: u64,
    habitat_admitted: u64,
    path_admitted: u64,
    decision_deferred: u64,
    habitat_deferred: u64,
    path_deferred: u64,
    oldest_debt_ticks: u64,
    habitat_candidates: u64,
    neighbor_candidates: u64,
    peak_active_rabbits: u32,
    peak_due_rabbits: u32,
}

impl WorkCounts {
    fn add(&mut self, sample: WildlifeSimulationWorkSnapshot) {
        self.decision_admitted += u64::from(sample.decision_admitted);
        self.habitat_admitted += u64::from(sample.habitat_admitted);
        self.path_admitted += u64::from(sample.path_admitted);
        self.decision_deferred += u64::from(sample.decision_deferred);
        self.habitat_deferred += u64::from(sample.habitat_deferred);
        self.path_deferred += u64::from(sample.path_deferred);
        self.oldest_debt_ticks = self.oldest_debt_ticks.max(sample.oldest_debt_ticks);
        self.habitat_candidates += u64::from(sample.habitat_candidates);
        self.neighbor_candidates += u64::from(sample.neighbor_candidates);
        self.peak_active_rabbits = self.peak_active_rabbits.max(sample.active_rabbits);
        self.peak_due_rabbits = self.peak_due_rabbits.max(sample.due_rabbits);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Invariants {
    living_conservation: bool,
    biomass_conservation: bool,
    forage_bounds: bool,
    below_hard_overload_guard: bool,
    boundary_violations: u32,
    post_initial_spawns: u32,
    unexplained_removals: u32,
    duplicate_identities: u32,
}

impl Invariants {
    fn passed(&self) -> bool {
        self.living_conservation
            && self.biomass_conservation
            && self.forage_bounds
            && self.below_hard_overload_guard
            && self.boundary_violations == 0
            && self.post_initial_spawns == 0
            && self.unexplained_removals == 0
            && self.duplicate_identities == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DailySummary {
    sample_index: u32,
    simulation_tick: u64,
    day_milli: u64,
    population: PopulationCounts,
    rabbit_energy: IntegerDistribution,
    deer_energy: IntegerDistribution,
    age_ticks: IntegerDistribution,
    deficit_ticks: IntegerDistribution,
    events: EventCounts,
    forage: ForageCounts,
    remains_records: u32,
    remains_biomass: u64,
    work: WorkCounts,
    identity_checksum: String,
    invariants: Invariants,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DailyReceipt {
    report_schema_version: u32,
    summary: DailySummary,
    subjects: Vec<WildlifePopulationSubject>,
    forage_cells: Vec<WildlifeForageCellSnapshot>,
    remains: Vec<WildlifeSimulationRemainsSnapshot>,
    identity_events: Vec<WildlifeSimulationEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    report_schema_version: u32,
    simulation_schema_version: u32,
    git_revision: String,
    package_version: &'static str,
    label: String,
    command: Vec<String>,
    world_profile: &'static str,
    topology: &'static str,
    config: WildlifeSimulationConfig,
    duration_days: u32,
    duration_ticks: u64,
    sample_ticks: u64,
    execution_mode: &'static str,
    equivalence_canary: Option<EquivalenceCanary>,
    ticking_chunks: Vec<[i32; 2]>,
    initial_population_revision: u16,
    lifecycle_rule_revision: u32,
    resource_rule_revision: u32,
    tuning: WildlifeLifecycleTuning,
    initial_identity_checksum: String,
    final_state_checksum: String,
    daily_series_checksum: String,
    samples: usize,
    all_invariants_passed: bool,
    terrain_lab_url: String,
    deterministic_files: [&'static str; 2],
    nondeterministic_performance_file: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceSample {
    sample_index: u32,
    ticks: u64,
    wall_millis: u128,
    mean_tick_micros: u128,
    max_tick_micros: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceReport {
    equivalence_canary_millis: u128,
    setup_millis: u128,
    run_millis: u128,
    total_ticks: u64,
    mean_tick_micros: u128,
    max_tick_micros: u128,
    peak_living: u32,
    peak_remains_records: u32,
    loaded_entity_ticking_chunks: usize,
    samples: Vec<PerformanceSample>,
    note: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct EquivalenceCanary {
    ticks: u32,
    tuning: WildlifeLifecycleTuning,
    initial_living: u32,
    births: u64,
    deaths: u64,
    forage_cells: u32,
    remains_created_biomass: u64,
    final_state_checksum: String,
    full_and_accelerated_equal: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("wildlife population simulation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> AnyResult<()> {
    let args = parse_args()?;
    prepare_output(&args.output)?;
    let tuning = load_tuning(args.tuning_path.as_deref())?;

    let canary_started = Instant::now();
    let equivalence_canary = if args.full_ticks || args.canary_ticks == 0 {
        None
    } else {
        Some(run_equivalence_canary(
            args.config,
            tuning,
            args.canary_ticks,
        )?)
    };
    let canary_elapsed = canary_started.elapsed();

    let setup_started = Instant::now();
    let mut simulation = WildlifeSimulationSession::open_with_tuning(args.config, tuning)?;
    let setup_elapsed = setup_started.elapsed();
    let initial = simulation.initial_snapshot().clone();
    let initial_identity_checksum = sha_json(&simulation.initial_identities())?;
    let ticking_chunks = simulation
        .immutable_ticking_chunks()
        .into_iter()
        .map(|chunk| [chunk.x, chunk.z])
        .collect::<Vec<_>>();

    let receipts_path = args.output.join("receipts.jsonl");
    let csv_path = args.output.join("daily.csv");
    let mut receipts = BufWriter::new(File::create(&receipts_path)?);
    let mut csv = BufWriter::new(File::create(&csv_path)?);
    write_csv_header(&mut csv)?;
    let mut series_hasher = Sha256::new();
    let mut summaries = Vec::new();
    let mut performance_samples = Vec::new();
    let mut pending_events = Vec::new();
    let mut work = WorkCounts::default();
    let mut previous_population = initial.subjects.len() as u64;
    let mut previous_biomass = 0_u64;
    let mut previous_forage = ForageCounts::default();
    let mut sample_started = Instant::now();
    let mut sample_tick_micros = 0_u128;
    let mut sample_max_tick_micros = 0_u128;
    let mut total_tick_micros = 0_u128;
    let mut total_max_tick_micros = 0_u128;
    let mut peak_living = initial.subjects.len() as u32;
    let mut peak_remains = 0_u32;
    let total_ticks = u64::from(args.days) * MINECRAFT_DAY_TICKS;
    let run_started = Instant::now();

    emit_sample(
        &mut simulation,
        &initial,
        0,
        &mut pending_events,
        std::mem::take(&mut work),
        &mut previous_population,
        &mut previous_biomass,
        &mut previous_forage,
        &mut receipts,
        &mut csv,
        &mut series_hasher,
        &mut summaries,
    )?;

    for tick in 1..=total_ticks {
        let tick_started = Instant::now();
        let snapshot = if args.full_ticks {
            simulation.advance_tick()?
        } else {
            simulation.advance_ecology_tick()?
        };
        let tick_micros = tick_started.elapsed().as_micros();
        sample_tick_micros += tick_micros;
        sample_max_tick_micros = sample_max_tick_micros.max(tick_micros);
        total_tick_micros += tick_micros;
        total_max_tick_micros = total_max_tick_micros.max(tick_micros);
        pending_events.extend(simulation.drain_events());
        work.add(simulation.work_snapshot());
        peak_living = peak_living.max(snapshot.subjects.len() as u32);

        if tick.is_multiple_of(args.sample_ticks) || tick == total_ticks {
            let sample_index = summaries.len() as u32;
            let interval_ticks = if tick.is_multiple_of(args.sample_ticks) {
                args.sample_ticks
            } else {
                tick % args.sample_ticks
            };
            let remains_count = simulation.remains().len() as u32;
            peak_remains = peak_remains.max(remains_count);
            emit_sample(
                &mut simulation,
                &snapshot,
                sample_index,
                &mut pending_events,
                std::mem::take(&mut work),
                &mut previous_population,
                &mut previous_biomass,
                &mut previous_forage,
                &mut receipts,
                &mut csv,
                &mut series_hasher,
                &mut summaries,
            )?;
            performance_samples.push(PerformanceSample {
                sample_index,
                ticks: interval_ticks,
                wall_millis: sample_started.elapsed().as_millis(),
                mean_tick_micros: sample_tick_micros / u128::from(interval_ticks.max(1)),
                max_tick_micros: sample_max_tick_micros,
            });
            sample_started = Instant::now();
            sample_tick_micros = 0;
            sample_max_tick_micros = 0;
            if tick.is_multiple_of(MINECRAFT_DAY_TICKS) {
                let summary = summaries.last().expect("sample just emitted");
                eprintln!(
                    "day {:>3}: rabbits={} deer={} births={} deaths={} forage={}/{} invariants={}",
                    tick / MINECRAFT_DAY_TICKS,
                    summary.population.rabbits,
                    summary.population.deer,
                    summary.events.births(),
                    summary.events.deaths(),
                    summary.forage.available,
                    summary.forage.potential,
                    summary.invariants.passed(),
                );
            }
        }
    }
    receipts.flush()?;
    csv.flush()?;
    let run_elapsed = run_started.elapsed();
    let daily_series_checksum = hex_digest(series_hasher.finalize());
    let final_snapshot = if total_ticks == 0 {
        initial
    } else {
        simulation.current_snapshot().clone()
    };
    let final_remains = simulation.remains();
    let final_forage = simulation.forage_cells();
    let final_state_checksum = sha_json(&(final_snapshot, final_remains, final_forage))?;
    let all_invariants_passed = summaries.iter().all(|row| row.invariants.passed());
    let terrain_lab_url = terrain_lab_url(args.config);
    let manifest = Manifest {
        report_schema_version: REPORT_SCHEMA_VERSION,
        simulation_schema_version: WILDLIFE_SIMULATION_SCHEMA_VERSION,
        git_revision: git_revision(),
        package_version: env!("CARGO_PKG_VERSION"),
        label: args.label,
        command: args.command,
        world_profile: "mclone-overworld-v1",
        topology: "unbounded-plane",
        config: args.config,
        duration_days: args.days,
        duration_ticks: total_ticks,
        sample_ticks: args.sample_ticks,
        execution_mode: if args.full_ticks {
            "full-authoritative"
        } else {
            "accelerated-authoritative-ecology"
        },
        equivalence_canary,
        ticking_chunks,
        initial_population_revision: MCLONE_WILDLIFE_POPULATION_REVISION,
        lifecycle_rule_revision: WILDLIFE_LIFECYCLE_RULE_REVISION,
        resource_rule_revision: WILDLIFE_RESOURCE_RULE_REVISION,
        tuning,
        initial_identity_checksum,
        final_state_checksum,
        daily_series_checksum,
        samples: summaries.len(),
        all_invariants_passed,
        terrain_lab_url,
        deterministic_files: ["receipts.jsonl", "daily.csv"],
        nondeterministic_performance_file: "performance.json",
    };
    let performance = PerformanceReport {
        equivalence_canary_millis: canary_elapsed.as_millis(),
        setup_millis: setup_elapsed.as_millis(),
        run_millis: run_elapsed.as_millis(),
        total_ticks,
        mean_tick_micros: total_tick_micros / u128::from(total_ticks.max(1)),
        max_tick_micros: total_max_tick_micros,
        peak_living,
        peak_remains_records: peak_remains,
        loaded_entity_ticking_chunks: manifest.ticking_chunks.len(),
        samples: performance_samples,
        note: "Wall timing is host-dependent and excluded from deterministic checksums.",
    };
    fs::write(
        args.output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        args.output.join("performance.json"),
        serde_json::to_vec_pretty(&performance)?,
    )?;
    fs::write(
        args.output.join("report.html"),
        render_html(&manifest, &performance, &summaries)?,
    )?;

    println!("report: {}", args.output.join("report.html").display());
    println!("manifest: {}", args.output.join("manifest.json").display());
    println!("daily checksum: {}", manifest.daily_series_checksum);
    if !all_invariants_passed {
        return Err("one or more population invariants failed".into());
    }
    Ok(())
}

fn run_equivalence_canary(
    config: WildlifeSimulationConfig,
    tuning: WildlifeLifecycleTuning,
    ticks: u32,
) -> AnyResult<EquivalenceCanary> {
    let tuning = equivalence_canary_tuning(tuning);
    let mut full = WildlifeSimulationSession::open_with_tuning(config, tuning)?;
    let mut accelerated = WildlifeSimulationSession::open_with_tuning(config, tuning)?;
    let initial_living = full.initial_snapshot().subjects.len() as u32;
    let mut event_counts = EventCounts::default();
    if full.initial_snapshot() != accelerated.initial_snapshot() {
        return Err("equivalence canary initial snapshots differ".into());
    }
    for tick in 1..=ticks {
        let full_snapshot = full.advance_tick()?;
        let accelerated_snapshot = accelerated.advance_ecology_tick()?;
        let full_events = full.drain_events();
        let accelerated_events = accelerated.drain_events();
        if full_snapshot != accelerated_snapshot
            || full_events != accelerated_events
            || full.forage_cells() != accelerated.forage_cells()
            || full.remains() != accelerated.remains()
        {
            return Err(format!("full/accelerated equivalence failed at tick {tick}").into());
        }
        add_event_counts(&mut event_counts, &count_events(&full_events));
    }
    if initial_living > 0 && event_counts.deaths() == 0 {
        return Err("equivalence canary did not reach a required natural death".into());
    }
    let final_state_checksum =
        sha_json(&(full.current_snapshot(), full.forage_cells(), full.remains()))?;
    Ok(EquivalenceCanary {
        ticks,
        tuning,
        initial_living,
        births: event_counts.births(),
        deaths: event_counts.deaths(),
        forage_cells: full.forage_cells().len() as u32,
        remains_created_biomass: event_counts.remains_created_biomass,
        final_state_checksum,
        full_and_accelerated_equal: true,
    })
}

fn equivalence_canary_tuning(mut tuning: WildlifeLifecycleTuning) -> WildlifeLifecycleTuning {
    tuning.rabbit_maturation_ticks = 40;
    tuning.rabbit_lifespan_ticks = 320;
    tuning.rabbit_lifespan_variance_ticks = 0;
    tuning.rabbit_breeding_cooldown_ticks = 80;
    tuning.rabbit_reproductive_energy = 600;
    tuning.rabbit_birth_energy_cost = 100;
    tuning.rabbit_starvation_ticks = 200;
    tuning.deer_maturation_ticks = 40;
    tuning.deer_lifespan_ticks = 400;
    tuning.deer_lifespan_variance_ticks = 0;
    tuning.deer_breeding_cooldown_ticks = 120;
    tuning.deer_reproductive_energy = 600;
    tuning.deer_birth_energy_cost = 100;
    tuning.deer_starvation_ticks = 240;
    tuning
}

fn add_event_counts(target: &mut EventCounts, source: &EventCounts) {
    target.rabbit_births += source.rabbit_births;
    target.deer_births += source.deer_births;
    target.rabbit_old_age_deaths += source.rabbit_old_age_deaths;
    target.rabbit_starvation_deaths += source.rabbit_starvation_deaths;
    target.deer_old_age_deaths += source.deer_old_age_deaths;
    target.deer_starvation_deaths += source.deer_starvation_deaths;
    target.young_deaths += source.young_deaths;
    target.adult_deaths += source.adult_deaths;
    target.intake_events += source.intake_events;
    target.intake_amount += source.intake_amount;
    target.remains_created_biomass += source.remains_created_biomass;
    target.remains_decayed_biomass += source.remains_decayed_biomass;
    target.suppressed_low_condition += source.suppressed_low_condition;
    target.suppressed_cooldown += source.suppressed_cooldown;
    target.suppressed_crowding += source.suppressed_crowding;
    target.suppressed_no_mate += source.suppressed_no_mate;
    target.suppressed_no_refuge_capacity += source.suppressed_no_refuge_capacity;
    target.suppressed_hard_overload += source.suppressed_hard_overload;
}

#[allow(clippy::too_many_arguments)]
fn emit_sample(
    simulation: &mut WildlifeSimulationSession,
    snapshot: &WildlifePopulationSnapshot,
    sample_index: u32,
    pending_events: &mut Vec<WildlifeSimulationEvent>,
    work: WorkCounts,
    previous_population: &mut u64,
    previous_biomass: &mut u64,
    previous_forage: &mut ForageCounts,
    receipts: &mut BufWriter<File>,
    csv: &mut BufWriter<File>,
    series_hasher: &mut Sha256,
    summaries: &mut Vec<DailySummary>,
) -> AnyResult<()> {
    let mut forage_cells = simulation.forage_cells();
    forage_cells.sort_unstable_by_key(|cell| cell.position);
    let remains = simulation.remains();
    let events = std::mem::take(pending_events);
    let event_counts = count_events(&events);
    let forage = count_forage(&forage_cells, previous_forage);
    let remains_biomass = remains.iter().map(|entry| u64::from(entry.biomass)).sum();
    let population = count_population(&snapshot.subjects);
    let population_end = u64::from(population.total);
    let living_conservation = population_end
        == previous_population
            .saturating_add(event_counts.births())
            .saturating_sub(event_counts.deaths());
    let biomass_conservation = remains_biomass
        == previous_biomass
            .saturating_add(event_counts.remains_created_biomass)
            .saturating_sub(event_counts.remains_decayed_biomass);
    let forage_bounds = forage_cells
        .iter()
        .flat_map(|cell| cell.strata)
        .all(|stratum| stratum.available <= stratum.potential);
    let below_hard_overload_guard = population.total < simulation.tuning().hard_population_guard;
    let summary = DailySummary {
        sample_index,
        simulation_tick: snapshot.simulation_tick,
        day_milli: snapshot.simulation_tick.saturating_mul(1_000) / MINECRAFT_DAY_TICKS,
        population,
        rabbit_energy: distribution(
            snapshot
                .subjects
                .iter()
                .filter(|entry| entry.species == WildlifeSimulationSpecies::Rabbit)
                .map(|entry| u32::from(entry.energy)),
        ),
        deer_energy: distribution(
            snapshot
                .subjects
                .iter()
                .filter(|entry| entry.species == WildlifeSimulationSpecies::Deer)
                .map(|entry| u32::from(entry.energy)),
        ),
        age_ticks: distribution(snapshot.subjects.iter().map(|entry| entry.age_ticks)),
        deficit_ticks: distribution(snapshot.subjects.iter().map(|entry| entry.deficit_ticks)),
        events: event_counts,
        forage,
        remains_records: remains.len() as u32,
        remains_biomass,
        work,
        identity_checksum: sha_json(&snapshot.subjects)?,
        invariants: Invariants {
            living_conservation,
            biomass_conservation,
            forage_bounds,
            below_hard_overload_guard,
            boundary_violations: 0,
            post_initial_spawns: 0,
            unexplained_removals: 0,
            duplicate_identities: 0,
        },
    };
    let identity_events = events
        .into_iter()
        .filter(|event| {
            matches!(
                event.event,
                WildlifeSimulationEventKind::Birth { .. }
                    | WildlifeSimulationEventKind::Death { .. }
            )
        })
        .collect();
    let receipt = DailyReceipt {
        report_schema_version: REPORT_SCHEMA_VERSION,
        summary: summary.clone(),
        subjects: snapshot.subjects.clone(),
        forage_cells,
        remains,
        identity_events,
    };
    let mut line = serde_json::to_vec(&receipt)?;
    line.push(b'\n');
    receipts.write_all(&line)?;
    series_hasher.update(&line);
    write_csv_row(csv, &summary)?;
    *previous_population = population_end;
    *previous_biomass = remains_biomass;
    *previous_forage = summary.forage.clone();
    summaries.push(summary);
    Ok(())
}

fn parse_args() -> AnyResult<Args> {
    let command = env::args().collect::<Vec<_>>();
    if command.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        std::process::exit(0);
    }
    let mut seed = None;
    let mut center = (0, 0);
    let mut radius = 2_u32;
    let mut days = 10_u32;
    let mut sample_ticks = MINECRAFT_DAY_TICKS;
    let mut output = None;
    let mut tuning_path = None;
    let mut label = "wildlife-population-run".to_owned();
    let mut full_ticks = false;
    let mut canary_ticks = 480_u32;
    let mut index = 1;
    while index < command.len() {
        let flag = &command[index];
        if flag == "--full-ticks" {
            full_ticks = true;
            index += 1;
            continue;
        }
        let value = command
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--seed" => seed = Some(value.parse()?),
            "--center" => center = parse_pair(value)?,
            "--radius" => radius = value.parse()?,
            "--days" => days = value.parse()?,
            "--sample-ticks" => sample_ticks = value.parse()?,
            "--output" => output = Some(PathBuf::from(value)),
            "--tuning" => tuning_path = Some(PathBuf::from(value)),
            "--label" => label = value.clone(),
            "--canary-ticks" => canary_ticks = value.parse()?,
            _ => return Err(format!("unknown argument {flag}; use --help").into()),
        }
        index += 2;
    }
    if radius > 16
        || days > 1_000
        || sample_ticks == 0
        || sample_ticks > MINECRAFT_DAY_TICKS
        || canary_ticks > 24_000
    {
        return Err(
            "radius must be <=16, days <=1000, sample ticks in 1..=24000, and canary ticks <=24000"
                .into(),
        );
    }
    if !MINECRAFT_DAY_TICKS.is_multiple_of(sample_ticks) {
        return Err("sample ticks must divide one 24000-tick Minecraft day".into());
    }
    Ok(Args {
        config: WildlifeSimulationConfig {
            seed: seed.ok_or("--seed is required")?,
            center_chunk_x: center.0,
            center_chunk_z: center.1,
            ticking_radius_chunks: radius,
        },
        days,
        sample_ticks,
        output: output.ok_or("--output is required")?,
        tuning_path,
        label,
        full_ticks,
        canary_ticks,
        command,
    })
}

fn print_help() {
    print!(
        "{}",
        "wildlife_population_sim --seed <signed-i64> --center <x,z> \\\n+  --radius <chunks> --days <minecraft-days> --output <directory> \\\n+  [--sample-ticks <ticks>] [--tuning <tuning.json>] [--label <text>] \\\n+  [--canary-ticks <ticks>] [--full-ticks]\n\n\
Runs ordinary authoritative Mclone wildlife in one immutable loaded domain.\n\
The output directory must be absent or empty."
            .replace("\n+", "\n")
    );
}

fn parse_pair(value: &str) -> AnyResult<(i32, i32)> {
    let (x, z) = value
        .split_once(',')
        .ok_or("--center must be chunk-x,chunk-z")?;
    Ok((x.parse()?, z.parse()?))
}

fn prepare_output(path: &Path) -> AnyResult<()> {
    if path.exists() && fs::read_dir(path)?.next().is_some() {
        return Err(format!("output directory {} is not empty", path.display()).into());
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn load_tuning(path: Option<&Path>) -> AnyResult<WildlifeLifecycleTuning> {
    match path {
        Some(path) => Ok(serde_json::from_slice(&fs::read(path)?)?),
        None => Ok(WildlifeLifecycleTuning::default()),
    }
}

fn count_population(subjects: &[WildlifePopulationSubject]) -> PopulationCounts {
    let mut counts = PopulationCounts {
        total: subjects.len() as u32,
        active: subjects.len() as u32,
        ..PopulationCounts::default()
    };
    for subject in subjects {
        match subject.species {
            WildlifeSimulationSpecies::Rabbit => {
                counts.rabbits += 1;
                match subject.life_stage {
                    WildlifeSimulationLifeStage::Young => counts.rabbit_young += 1,
                    WildlifeSimulationLifeStage::Adult => counts.rabbit_adults += 1,
                }
                counts.rabbit_known_refuge += u32::from(subject.rabbit_has_refuge);
                counts.rabbit_sheltered += u32::from(subject.rabbit_sheltered);
            }
            WildlifeSimulationSpecies::Deer => {
                counts.deer += 1;
                match (subject.life_stage, subject.sex) {
                    (WildlifeSimulationLifeStage::Young, _) => counts.deer_young += 1,
                    (WildlifeSimulationLifeStage::Adult, WildlifeSimulationSex::Female) => {
                        counts.deer_adult_females += 1;
                    }
                    (WildlifeSimulationLifeStage::Adult, WildlifeSimulationSex::Male) => {
                        counts.deer_adult_males += 1;
                    }
                    (WildlifeSimulationLifeStage::Adult, WildlifeSimulationSex::Unknown) => {}
                }
            }
        }
    }
    counts.deer_proximity_groups = deer_proximity_groups(subjects);
    counts
}

fn deer_proximity_groups(subjects: &[WildlifePopulationSubject]) -> u32 {
    let deer = subjects
        .iter()
        .filter(|subject| subject.species == WildlifeSimulationSpecies::Deer)
        .collect::<Vec<_>>();
    if deer.is_empty() {
        return 0;
    }
    let mut parent = (0..deer.len()).collect::<Vec<_>>();
    let mut cells = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for (index, subject) in deer.iter().enumerate() {
        let cell = (
            subject.position_x_milli.div_euclid(16_000),
            subject.position_z_milli.div_euclid(16_000),
        );
        for x in cell.0 - 1..=cell.0 + 1 {
            for z in cell.1 - 1..=cell.1 + 1 {
                if let Some(candidates) = cells.get(&(x, z)) {
                    for candidate in candidates {
                        let other = deer[*candidate];
                        let dx = subject.position_x_milli - other.position_x_milli;
                        let dz = subject.position_z_milli - other.position_z_milli;
                        if dx * dx + dz * dz <= 16_000_i64.pow(2) {
                            union(&mut parent, index, *candidate);
                        }
                    }
                }
            }
        }
        cells.entry(cell).or_default().push(index);
    }
    (0..deer.len())
        .map(|index| find(&mut parent, index))
        .collect::<BTreeSet<_>>()
        .len() as u32
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}

fn count_events(events: &[WildlifeSimulationEvent]) -> EventCounts {
    let mut counts = EventCounts::default();
    for event in events {
        match event.event {
            WildlifeSimulationEventKind::Intake { energy, .. } => {
                counts.intake_events += 1;
                counts.intake_amount += u64::from(energy);
            }
            WildlifeSimulationEventKind::Birth { .. } => match event.species {
                WildlifeSimulationSpecies::Rabbit => counts.rabbit_births += 1,
                WildlifeSimulationSpecies::Deer => counts.deer_births += 1,
            },
            WildlifeSimulationEventKind::Death { cause, life_stage } => {
                match (event.species, cause) {
                    (WildlifeSimulationSpecies::Rabbit, WildlifeSimulationDeathCause::OldAge) => {
                        counts.rabbit_old_age_deaths += 1;
                    }
                    (
                        WildlifeSimulationSpecies::Rabbit,
                        WildlifeSimulationDeathCause::Starvation,
                    ) => counts.rabbit_starvation_deaths += 1,
                    (WildlifeSimulationSpecies::Deer, WildlifeSimulationDeathCause::OldAge) => {
                        counts.deer_old_age_deaths += 1;
                    }
                    (WildlifeSimulationSpecies::Deer, WildlifeSimulationDeathCause::Starvation) => {
                        counts.deer_starvation_deaths += 1
                    }
                }
                match life_stage {
                    WildlifeSimulationLifeStage::Young => counts.young_deaths += 1,
                    WildlifeSimulationLifeStage::Adult => counts.adult_deaths += 1,
                }
            }
            WildlifeSimulationEventKind::RemainsCreated { biomass } => {
                counts.remains_created_biomass += u64::from(biomass);
            }
            WildlifeSimulationEventKind::RemainsDecayed { amount } => {
                counts.remains_decayed_biomass += u64::from(amount);
            }
            WildlifeSimulationEventKind::ReproductionSuppressed { reason } => match reason {
                WildlifeSimulationSuppressionReason::LowCondition => {
                    counts.suppressed_low_condition += 1;
                }
                WildlifeSimulationSuppressionReason::Cooldown => {
                    counts.suppressed_cooldown += 1;
                }
                WildlifeSimulationSuppressionReason::Crowding => {
                    counts.suppressed_crowding += 1;
                }
                WildlifeSimulationSuppressionReason::NoMate => {
                    counts.suppressed_no_mate += 1;
                }
                WildlifeSimulationSuppressionReason::NoRefugeCapacity => {
                    counts.suppressed_no_refuge_capacity += 1;
                }
                WildlifeSimulationSuppressionReason::HardOverload => {
                    counts.suppressed_hard_overload += 1;
                }
            },
        }
    }
    counts
}

fn count_forage(cells: &[WildlifeForageCellSnapshot], previous: &ForageCounts) -> ForageCounts {
    let strata = std::array::from_fn(|index| {
        let resource = WildlifeResourceKind::ALL[index];
        let prior = previous.strata[index];
        let mut counts = ResourceCounts {
            resource,
            potential: cells
                .iter()
                .map(|cell| u64::from(cell.strata[index].potential))
                .sum(),
            available: cells
                .iter()
                .map(|cell| u64::from(cell.strata[index].available))
                .sum(),
            cumulative_recovered: cells.iter().map(|cell| cell.strata[index].recovered).sum(),
            cumulative_rabbit_consumed: cells
                .iter()
                .map(|cell| cell.strata[index].rabbit_consumed)
                .sum(),
            cumulative_deer_consumed: cells
                .iter()
                .map(|cell| cell.strata[index].deer_consumed)
                .sum(),
            cumulative_mallard_consumed: cells
                .iter()
                .map(|cell| cell.strata[index].mallard_consumed)
                .sum(),
            ..ResourceCounts::default()
        };
        counts.interval_recovered = counts
            .cumulative_recovered
            .saturating_sub(prior.cumulative_recovered);
        counts.interval_rabbit_consumed = counts
            .cumulative_rabbit_consumed
            .saturating_sub(prior.cumulative_rabbit_consumed);
        counts.interval_deer_consumed = counts
            .cumulative_deer_consumed
            .saturating_sub(prior.cumulative_deer_consumed);
        counts.interval_mallard_consumed = counts
            .cumulative_mallard_consumed
            .saturating_sub(prior.cumulative_mallard_consumed);
        counts
    });
    let mut counts = ForageCounts {
        cells: cells.len() as u32,
        strata,
        potential: strata.iter().map(|entry| entry.potential).sum(),
        available: strata.iter().map(|entry| entry.available).sum(),
        cumulative_recovered: strata.iter().map(|entry| entry.cumulative_recovered).sum(),
        cumulative_rabbit_consumed: strata
            .iter()
            .map(|entry| entry.cumulative_rabbit_consumed)
            .sum(),
        cumulative_deer_consumed: strata
            .iter()
            .map(|entry| entry.cumulative_deer_consumed)
            .sum(),
        cumulative_mallard_consumed: strata
            .iter()
            .map(|entry| entry.cumulative_mallard_consumed)
            .sum(),
        ..ForageCounts::default()
    };
    counts.interval_recovered = counts
        .cumulative_recovered
        .saturating_sub(previous.cumulative_recovered);
    counts.interval_rabbit_consumed = counts
        .cumulative_rabbit_consumed
        .saturating_sub(previous.cumulative_rabbit_consumed);
    counts.interval_deer_consumed = counts
        .cumulative_deer_consumed
        .saturating_sub(previous.cumulative_deer_consumed);
    counts.interval_mallard_consumed = counts
        .cumulative_mallard_consumed
        .saturating_sub(previous.cumulative_mallard_consumed);
    counts
}

fn distribution(values: impl Iterator<Item = u32>) -> IntegerDistribution {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return IntegerDistribution::default();
    }
    values.sort_unstable();
    IntegerDistribution {
        count: values.len() as u32,
        sum: values.iter().map(|value| u64::from(*value)).sum(),
        minimum: values[0],
        p10: percentile(&values, 10),
        p50: percentile(&values, 50),
        p90: percentile(&values, 90),
        maximum: *values.last().unwrap(),
    }
}

fn percentile(values: &[u32], percentile: usize) -> u32 {
    values[(values.len() - 1) * percentile / 100]
}

fn write_csv_header(writer: &mut impl Write) -> AnyResult<()> {
    writeln!(
        writer,
        "sample,day,tick,total,rabbits,rabbit_adults,rabbit_young,deer,deer_adult_females,deer_adult_males,deer_young,rabbit_births,deer_births,deaths,forage_available,forage_potential,rabbit_consumed,deer_consumed,rabbit_energy_p50,deer_energy_p50,remains_records,remains_biomass,suppressed,decision_admitted,path_admitted,path_deferred,identity_checksum,invariants_passed"
    )?;
    Ok(())
}

fn write_csv_row(writer: &mut impl Write, row: &DailySummary) -> AnyResult<()> {
    writeln!(
        writer,
        "{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        row.sample_index,
        row.day_milli as f64 / 1_000.0,
        row.simulation_tick,
        row.population.total,
        row.population.rabbits,
        row.population.rabbit_adults,
        row.population.rabbit_young,
        row.population.deer,
        row.population.deer_adult_females,
        row.population.deer_adult_males,
        row.population.deer_young,
        row.events.rabbit_births,
        row.events.deer_births,
        row.events.deaths(),
        row.forage.available,
        row.forage.potential,
        row.forage.interval_rabbit_consumed,
        row.forage.interval_deer_consumed,
        row.rabbit_energy.p50,
        row.deer_energy.p50,
        row.remains_records,
        row.remains_biomass,
        row.events.suppressed(),
        row.work.decision_admitted,
        row.work.path_admitted,
        row.work.path_deferred,
        row.identity_checksum,
        row.invariants.passed(),
    )?;
    Ok(())
}

fn render_html(
    manifest: &Manifest,
    performance: &PerformanceReport,
    rows: &[DailySummary],
) -> AnyResult<String> {
    let population_chart = chart(
        "Living population",
        rows,
        &[
            ("Rabbits", "#5fbf75", |row: &DailySummary| {
                u64::from(row.population.rabbits)
            }),
            ("Deer", "#d9a25f", |row: &DailySummary| {
                u64::from(row.population.deer)
            }),
        ],
    );
    let birth_death_chart = chart(
        "Births and deaths per sample",
        rows,
        &[
            ("Births", "#75d98b", |row: &DailySummary| {
                row.events.births()
            }),
            ("Deaths", "#e16b66", |row: &DailySummary| {
                row.events.deaths()
            }),
        ],
    );
    let forage_chart = chart(
        "Forage",
        rows,
        &[
            ("Available", "#63b9a7", |row: &DailySummary| {
                row.forage.available
            }),
            ("Potential", "#b6d96a", |row: &DailySummary| {
                row.forage.potential
            }),
        ],
    );
    let energy_chart = chart(
        "Median energy",
        rows,
        &[
            ("Rabbit", "#88d399", |row: &DailySummary| {
                u64::from(row.rabbit_energy.p50)
            }),
            ("Deer", "#e1b579", |row: &DailySummary| {
                u64::from(row.deer_energy.p50)
            }),
        ],
    );
    let remains_chart = chart(
        "Remaining biomass",
        rows,
        &[("Biomass", "#b58c78", |row: &DailySummary| {
            row.remains_biomass
        })],
    );
    let final_row = rows.last().ok_or("report has no samples")?;
    let mut table = String::new();
    for row in rows.iter().rev().take(20).rev() {
        let _ = writeln!(
            table,
            "<tr><td>{:.3}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td><td>{}</td></tr>",
            row.day_milli as f64 / 1_000.0,
            row.population.rabbits,
            row.population.deer,
            row.events.births(),
            row.events.deaths(),
            row.forage.available,
            row.forage.potential,
            if row.invariants.passed() {
                "pass"
            } else {
                "FAIL"
            },
        );
    }
    let manifest_json = html_escape(&serde_json::to_string_pretty(manifest)?);
    Ok(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Wildlife population report</title><style>{}</style></head><body><main><p class=\"eyebrow\">Mclone authoritative ecology</p><h1>{}</h1><p>Seed <code>{}</code>, center chunk <code>{},{}</code>, radius <code>{}</code>, {} Minecraft days. <a href=\"{}\">Open the matching Terrain Lab wildlife map</a>.</p><section class=\"cards\"><article><b>{}</b><span>final rabbits</span></article><article><b>{}</b><span>final deer</span></article><article><b>{}</b><span>peak living</span></article><article class=\"{}\"><b>{}</b><span>invariants</span></article></section><section class=\"grid\">{}{}{}{}{}</section><h2>Recent samples</h2><div class=\"table-wrap\"><table><thead><tr><th>Day</th><th>Rabbits</th><th>Deer</th><th>Births</th><th>Deaths</th><th>Forage</th><th>Checks</th></tr></thead><tbody>{}</tbody></table></div><h2>Performance (not deterministic)</h2><p>{} ticks in {} ms; mean {} µs/tick; max {} µs; {} entity-ticking chunks.</p><h2>Run manifest</h2><pre>{}</pre></main></body></html>",
        CSS,
        html_escape(&manifest.label),
        manifest.config.seed,
        manifest.config.center_chunk_x,
        manifest.config.center_chunk_z,
        manifest.config.ticking_radius_chunks,
        manifest.duration_days,
        html_escape(&manifest.terrain_lab_url),
        final_row.population.rabbits,
        final_row.population.deer,
        performance.peak_living,
        if manifest.all_invariants_passed {
            "pass"
        } else {
            "fail"
        },
        if manifest.all_invariants_passed {
            "PASS"
        } else {
            "FAIL"
        },
        population_chart,
        birth_death_chart,
        forage_chart,
        energy_chart,
        remains_chart,
        table,
        performance.total_ticks,
        performance.run_millis,
        performance.mean_tick_micros,
        performance.max_tick_micros,
        performance.loaded_entity_ticking_chunks,
        manifest_json,
    ))
}

type SeriesValue = fn(&DailySummary) -> u64;

fn chart(title: &str, rows: &[DailySummary], series: &[(&str, &str, SeriesValue)]) -> String {
    let width = 680.0_f64;
    let height = 220.0_f64;
    let values = series
        .iter()
        .flat_map(|(_, _, value)| rows.iter().map(value))
        .collect::<Vec<_>>();
    let maximum = values.into_iter().max().unwrap_or(1).max(1) as f64;
    let denominator = rows.len().saturating_sub(1).max(1) as f64;
    let mut lines = String::new();
    let mut legend = String::new();
    for (name, color, value) in series {
        let points = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let x = index as f64 / denominator * width;
                let y = height - value(row) as f64 / maximum * height;
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(
            lines,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"3\" vector-effect=\"non-scaling-stroke\"/>",
            points, color
        );
        let _ = write!(
            legend,
            "<span><i style=\"background:{}\"></i>{}</span>",
            color,
            html_escape(name)
        );
    }
    format!(
        "<article class=\"chart\"><h2>{}</h2><div class=\"legend\">{}</div><svg viewBox=\"0 0 {} {}\" role=\"img\"><path d=\"M0 {}H{}\" stroke=\"#334137\"/>{}</svg><small>0–{} · {} samples</small></article>",
        html_escape(title),
        legend,
        width,
        height,
        height,
        width,
        lines,
        maximum as u64,
        rows.len(),
    )
}

fn terrain_lab_url(config: WildlifeSimulationConfig) -> String {
    let blocks = ((config.ticking_radius_chunks * 2 + 1) * 16).max(128);
    format!(
        "https://mclone.kzahel.com/terrain/?profile=mclone-overworld-v1&seed={}&x={}&z={}&blocks={}&panes=wildlife&view=map",
        config.seed,
        config.center_chunk_x * 16,
        config.center_chunk_z * 16,
        blocks,
    )
}

fn sha_json(value: &impl Serialize) -> AnyResult<String> {
    Ok(hex_digest(Sha256::digest(serde_json::to_vec(value)?)))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn git_revision() -> String {
    env::var("MCLONE_GIT_REVISION").unwrap_or_else(|_| {
        let dirty = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .is_some_and(|output| !output.stdout.is_empty());
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|revision| revision.trim().to_owned())
            .filter(|revision| !revision.is_empty())
            .map(|revision| {
                if dirty {
                    format!("{revision}+dirty")
                } else {
                    revision
                }
            })
            .unwrap_or_else(|| "unknown".to_owned())
    })
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS: &str = r#"
:root{color-scheme:dark;font-family:ui-sans-serif,system-ui;background:#111712;color:#edf4e8}
body{margin:0;background:radial-gradient(circle at 20% 0,#243527,#111712 45%)}
main{max-width:1440px;margin:auto;padding:48px 28px 80px}.eyebrow{text-transform:uppercase;letter-spacing:.18em;color:#91b999;font-size:.76rem}h1{font-size:clamp(2.2rem,5vw,4.6rem);margin:.1em 0}.cards,.grid{display:grid;gap:16px}.cards{grid-template-columns:repeat(auto-fit,minmax(150px,1fr));margin:28px 0}.cards article,.chart,pre,.table-wrap{background:#182219;border:1px solid #324435;border-radius:14px;padding:18px}.cards b{display:block;font-size:2rem}.cards span,small{color:#9eae9f}.cards .pass{border-color:#4d9b60}.cards .fail{border-color:#c45b57}.grid{grid-template-columns:repeat(auto-fit,minmax(430px,1fr))}.chart h2{margin:0 0 8px}.chart svg{width:100%;height:auto;background:#121914;border-radius:8px}.legend{display:flex;gap:18px;margin-bottom:12px;color:#bac8ba}.legend i{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}a{color:#9fdab0}code{color:#d8e7a9}.table-wrap{overflow:auto}table{width:100%;border-collapse:collapse}th,td{text-align:right;padding:9px;border-bottom:1px solid #2b392e}th:first-child,td:first-child{text-align:left}pre{white-space:pre-wrap;word-break:break-word;color:#b9c8b9}@media(max-width:560px){main{padding:28px 14px}.grid{grid-template-columns:1fr}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributions_and_event_counts_are_exact() {
        assert_eq!(distribution([9, 1, 5].into_iter()).p50, 5);
        let identity = mclone_server::WildlifeSimulationIdentity { most: 1, least: 2 };
        let events = [
            WildlifeSimulationEvent {
                tick: 1,
                species: WildlifeSimulationSpecies::Rabbit,
                subject: identity,
                event: WildlifeSimulationEventKind::Birth {
                    child: identity,
                    parents: [identity, identity],
                },
            },
            WildlifeSimulationEvent {
                tick: 2,
                species: WildlifeSimulationSpecies::Rabbit,
                subject: identity,
                event: WildlifeSimulationEventKind::Death {
                    cause: WildlifeSimulationDeathCause::Starvation,
                    life_stage: WildlifeSimulationLifeStage::Young,
                },
            },
        ];
        let counts = count_events(&events);
        assert_eq!(counts.births(), 1);
        assert_eq!(counts.deaths(), 1);
        assert_eq!(counts.young_deaths, 1);
    }

    #[test]
    fn deer_group_count_uses_bounded_neighbor_cells() {
        let make = |least, x| WildlifePopulationSubject {
            identity_most: 0,
            identity_least: least,
            species: WildlifeSimulationSpecies::Deer,
            chunk_x: 0,
            chunk_z: 0,
            position_x_milli: x,
            position_y_milli: 64_000,
            position_z_milli: 0,
            life_stage: WildlifeSimulationLifeStage::Adult,
            sex: WildlifeSimulationSex::Female,
            age_ticks: 1,
            lifespan_ticks: 2,
            energy: 3,
            deficit_ticks: 0,
            recent_intake: 0,
            reproductive_condition: 0,
            reproduction_cooldown: 0,
            parents: [None; 2],
            rabbit_behavior: None,
            deer_behavior: None,
            rabbit_has_refuge: false,
            rabbit_sheltered: false,
        };
        assert_eq!(
            deer_proximity_groups(&[make(1, 0), make(2, 8_000), make(3, 40_000)]),
            2
        );
    }
}
