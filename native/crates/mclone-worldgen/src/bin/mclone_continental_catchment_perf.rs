use std::{
    env, fs,
    hint::black_box,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use mclone_worldgen::{
    continental_catchment_review::compile_continental_catchment_review,
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_hydrography::{ContinentalHydrographyPlan, ContinentalHydrographyWork},
    continental_surface::{
        ContinentalSurfaceConstructionCounts, ContinentalSurfacePlan,
        ContinentalSurfaceWindowRequest,
    },
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-catchment-review/performance.json";
const GRAPH_ITERATIONS: u32 = 10_000;
const POINT_ITERATIONS: u32 = 250;
const WINDOW_ITERATIONS: u32 = 8;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingReceipt<W> {
    iterations: u32,
    elapsed_ns: u128,
    nanoseconds_per_iteration: u128,
    work_per_iteration: W,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceReceipt {
    schema_revision: &'static str,
    source_commit: String,
    git_dirty: bool,
    catalog_selection_ns: u128,
    selected_owner_x: i32,
    selected_owner_z: i32,
    graph_construction: TimingReceipt<ContinentalHydrographyWork>,
    direct_surface_point: TimingReceipt<ContinentalSurfaceConstructionCounts>,
    cached_surface_window: TimingReceipt<ContinentalSurfaceConstructionCounts>,
    cache_reset: CacheResetReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheResetReceipt {
    first_elapsed_ns: u128,
    second_elapsed_ns: u128,
    samples_equal: bool,
    work_equal: bool,
    semantic_sha256_equal: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("continental catchment performance review failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let output = parse_output()?;
    let descriptor = ContinentalEcoregionDescriptor::plane(12_345);

    let catalog_started = Instant::now();
    let catalog = compile_continental_catchment_review(descriptor)?;
    let catalog_selection_ns = catalog_started.elapsed().as_nanos();
    let review_center = catalog
        .sites
        .iter()
        .find(|site| site.kind.label() == "tributary-confluence")
        .ok_or_else(|| "catchment review catalog is missing its confluence".to_owned())?;

    let hydrography = ContinentalHydrographyPlan::new(descriptor);
    let graph_started = Instant::now();
    let mut graph_hash = 0_u64;
    for iteration in 0..GRAPH_ITERATIONS {
        let owner_x = catalog.selected_owner_x + (iteration % 3) as i32 - 1;
        let owner_z = catalog.selected_owner_z + ((iteration / 3) % 3) as i32 - 1;
        let catchment = hydrography.catchment_for_owner(owner_x, owner_z);
        graph_hash ^= catchment.id.hash;
        black_box(&catchment);
    }
    black_box(graph_hash);
    let graph_elapsed = graph_started.elapsed();

    let surface = ContinentalSurfacePlan::new(descriptor).map_err(|error| error.to_string())?;
    let point_started = Instant::now();
    let mut last_point = None;
    for iteration in 0..POINT_ITERATIONS {
        let site = &catalog.sites[iteration as usize % catalog.sites.len()];
        last_point = Some(surface.query_point(site.center_x, site.center_z));
    }
    let point_elapsed = point_started.elapsed();
    let point_work = last_point
        .ok_or_else(|| "direct point benchmark executed no iterations".to_owned())?
        .work;

    let request = ContinentalSurfaceWindowRequest::new(
        review_center.center_x - 320,
        review_center.center_z - 320,
        21,
        21,
        32,
    );
    let window_started = Instant::now();
    let mut last_window = None;
    for _ in 0..WINDOW_ITERATIONS {
        last_window = Some(
            surface
                .query_window(request)
                .map_err(|error| error.to_string())?,
        );
    }
    let window_elapsed = window_started.elapsed();
    let window_work = last_window
        .ok_or_else(|| "window benchmark executed no iterations".to_owned())?
        .work;

    let first_started = Instant::now();
    let first = surface
        .query_window(request)
        .map_err(|error| error.to_string())?;
    let first_elapsed = first_started.elapsed();
    let second_started = Instant::now();
    let second = surface
        .query_window(request)
        .map_err(|error| error.to_string())?;
    let second_elapsed = second_started.elapsed();
    let cache_reset = CacheResetReceipt {
        first_elapsed_ns: first_elapsed.as_nanos(),
        second_elapsed_ns: second_elapsed.as_nanos(),
        samples_equal: first.samples == second.samples,
        work_equal: first.work == second.work,
        semantic_sha256_equal: first.semantic_sha256 == second.semantic_sha256,
    };
    if !cache_reset.samples_equal || !cache_reset.work_equal || !cache_reset.semantic_sha256_equal {
        return Err("cache-reset window repeat changed output or bounded work".to_owned());
    }

    let receipt = PerformanceReceipt {
        schema_revision: "mclone-continental-catchment-performance-v1",
        source_commit: command_text("git", &["rev-parse", "HEAD"]),
        git_dirty: !command_text("git", &["status", "--porcelain"]).is_empty(),
        catalog_selection_ns,
        selected_owner_x: catalog.selected_owner_x,
        selected_owner_z: catalog.selected_owner_z,
        graph_construction: timing(
            GRAPH_ITERATIONS,
            graph_elapsed,
            ContinentalHydrographyWork {
                owner_evaluations: 1,
                graph_constructions: 1,
                reach_evaluations: 0,
                exact_chunks: 0,
                raster_cells: 0,
            },
        ),
        direct_surface_point: timing(POINT_ITERATIONS, point_elapsed, point_work),
        cached_surface_window: timing(WINDOW_ITERATIONS, window_elapsed, window_work),
        cache_reset,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut json = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize performance receipt: {error}"))?;
    json.push(b'\n');
    fs::write(&output, json).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!(
        "graph: {} ns/construction; point: {} ns/sample; window: {} ns/441 samples",
        receipt.graph_construction.nanoseconds_per_iteration,
        receipt.direct_surface_point.nanoseconds_per_iteration,
        receipt.cached_surface_window.nanoseconds_per_iteration,
    );
    Ok(())
}

fn timing<W>(iterations: u32, elapsed: Duration, work_per_iteration: W) -> TimingReceipt<W> {
    TimingReceipt {
        iterations,
        elapsed_ns: elapsed.as_nanos(),
        nanoseconds_per_iteration: elapsed.as_nanos() / u128::from(iterations),
        work_per_iteration,
    }
}

fn parse_output() -> Result<PathBuf, String> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            output = PathBuf::from(
                arguments
                    .next()
                    .ok_or_else(|| "--output requires a path".to_owned())?,
            );
        } else {
            return Err(format!("unknown argument {argument:?}"));
        }
    }
    Ok(output)
}

fn command_text(program: &str, arguments: &[&str]) -> String {
    std::process::Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
