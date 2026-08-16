#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use image::{Rgb, RgbImage};
use mclone_season::{
    MCLONE_AXIAL_TILT_DEGREES, MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z,
    MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS, OrbitalPhase, SolarCoordinatePolicy, SolarInput,
    SolarSample,
};
use mclone_worldgen::levelgen::{McloneOverworldSampler, McloneOverworldTerrainSample};
use serde::Serialize;

const MAP_PANEL_WIDTH: u32 = 480;
const MAP_PANEL_HEIGHT: u32 = 320;
const MAP_HALF_X_BLOCKS: f64 = 49_152.0;
const MAP_HALF_Z_BLOCKS: f64 = 49_152.0;
const CONNECTED_GRID: usize = 256;
const CONNECTED_STEP_BLOCKS: i32 = 128;
const CANDIDATE_WAVELENGTHS: [f64; 3] = [49_152.0, 98_304.0, 196_608.0];
const METRIC_SEEDS: [i64; 4] = [0, 12_345, 67_890, -98_765];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewReceipt {
    schema_version: u32,
    seed: i64,
    map_bounds_blocks: [f64; 4],
    map_panel_size: [u32; 2],
    panel_order: [&'static str; 4],
    accepted_wavelength_blocks: f64,
    phase_origin_z: f64,
    axial_tilt_degrees: f64,
    candidates: Vec<CandidateReceipt>,
    continent_metrics: Vec<ContinentMetric>,
    solar_cases: Vec<SolarCaseReceipt>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateReceipt {
    wavelength_blocks: f64,
    equator_to_pole_blocks: f64,
    status: &'static str,
    rationale: &'static str,
    image: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContinentMetric {
    seed: i64,
    sampled_span_blocks: i32,
    sample_step_blocks: i32,
    land_fraction: f64,
    largest_component_area_blocks_approx: u64,
    largest_component_width_blocks: u32,
    largest_component_depth_blocks: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SolarCaseReceipt {
    id: &'static str,
    world_x: f64,
    world_z: f64,
    latitude_degrees: f64,
    latitude_phase: f64,
    orbital_phase: f64,
    solar_time_hours: f64,
    axial_tilt_degrees: f64,
    direction: [f32; 3],
    elevation_degrees: f32,
    azimuth_degrees: f32,
    declination_degrees: f32,
    daylight_factor: f32,
    twilight_factor: f32,
    day_length_hours: f32,
    polar_state: &'static str,
}

fn main() -> Result<()> {
    let (output, seed) = parse_args()?;
    fs::create_dir_all(&output).with_context(|| {
        format!(
            "create seasonal solar review directory {}",
            output.display()
        )
    })?;

    let sampler = McloneOverworldSampler::new(seed);
    let samples = sample_map(sampler);
    let mut candidates = Vec::new();
    for wavelength in CANDIDATE_WAVELENGTHS {
        let filename = format!("latitude-climate-{wavelength:.0}.png");
        write_candidate_map(&output.join(&filename), &samples, wavelength)?;
        let (status, rationale) = if wavelength == MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS {
            (
                "accepted",
                "24,576 blocks from equator to pole keeps several major terrain regions inside a temperate shoulder",
            )
        } else if wavelength < MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS {
            (
                "rejected-too-frequent",
                "12,288 blocks from equator to pole makes latitude dominate continent-scale travel too often",
            )
        } else {
            (
                "rejected-too-diffuse",
                "49,152 blocks from equator to pole makes the loop difficult to read during ordinary fast travel",
            )
        };
        candidates.push(CandidateReceipt {
            wavelength_blocks: wavelength,
            equator_to_pole_blocks: wavelength / 4.0,
            status,
            rationale,
            image: filename,
        });
    }

    let receipt = ReviewReceipt {
        schema_version: 1,
        seed,
        map_bounds_blocks: [
            -MAP_HALF_X_BLOCKS,
            -MAP_HALF_Z_BLOCKS,
            MAP_HALF_X_BLOCKS,
            MAP_HALF_Z_BLOCKS,
        ],
        map_panel_size: [MAP_PANEL_WIDTH, MAP_PANEL_HEIGHT],
        panel_order: [
            "terrain/coast",
            "existing temperature/moisture",
            "candidate effective latitude",
            "review-only annual-temperature proxy",
        ],
        accepted_wavelength_blocks: MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS,
        phase_origin_z: MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z,
        axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
        candidates,
        continent_metrics: METRIC_SEEDS.into_iter().map(measure_continents).collect(),
        solar_cases: solar_cases()?,
    };
    let receipt_path = output.join("solar-review.json");
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("write {}", receipt_path.display()))?;
    println!("{}", receipt_path.display());
    Ok(())
}

fn parse_args() -> Result<(PathBuf, i64)> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut seed = 12_345;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--seed" => {
                let raw = args.next().context("--seed requires an i64")?;
                seed = raw
                    .parse::<i64>()
                    .with_context(|| format!("invalid --seed `{raw}`"))?;
            }
            "--help" | "-h" => {
                println!("mclone-season-lab --output DIR [--seed 12345]");
                std::process::exit(0);
            }
            _ => bail!("unknown argument `{arg}`"),
        }
    }
    Ok((
        output.context("mclone-season-lab requires --output DIR")?,
        seed,
    ))
}

fn sample_map(sampler: McloneOverworldSampler) -> Vec<McloneOverworldTerrainSample> {
    let mut samples = Vec::with_capacity((MAP_PANEL_WIDTH * MAP_PANEL_HEIGHT) as usize);
    for py in 0..MAP_PANEL_HEIGHT {
        let z = map_z(py).round() as i32;
        for px in 0..MAP_PANEL_WIDTH {
            samples.push(sampler.sample(map_x(px).round() as i32, z));
        }
    }
    samples
}

fn write_candidate_map(
    path: &Path,
    samples: &[McloneOverworldTerrainSample],
    wavelength: f64,
) -> Result<()> {
    let mut image = RgbImage::new(MAP_PANEL_WIDTH * 2, MAP_PANEL_HEIGHT * 2);
    let policy = SolarCoordinatePolicy::McloneCyclicPlane {
        wavelength_blocks: wavelength,
        phase_origin_z: MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z,
    };
    for py in 0..MAP_PANEL_HEIGHT {
        let world_z = map_z(py);
        let latitude = policy.latitude_at(0.0, world_z)?.degrees;
        for px in 0..MAP_PANEL_WIDTH {
            let sample = samples[(py * MAP_PANEL_WIDTH + px) as usize];
            image.put_pixel(px, py, terrain_color(sample));
            image.put_pixel(px + MAP_PANEL_WIDTH, py, climate_color(sample));
            image.put_pixel(px, py + MAP_PANEL_HEIGHT, latitude_color(latitude));
            image.put_pixel(
                px + MAP_PANEL_WIDTH,
                py + MAP_PANEL_HEIGHT,
                annual_proxy_color(sample, latitude),
            );
        }
    }
    draw_latitude_guides(&mut image, policy)?;
    image
        .save(path)
        .with_context(|| format!("save {}", path.display()))
}

fn map_x(pixel: u32) -> f64 {
    -MAP_HALF_X_BLOCKS + f64::from(pixel) / f64::from(MAP_PANEL_WIDTH - 1) * MAP_HALF_X_BLOCKS * 2.0
}

fn map_z(pixel: u32) -> f64 {
    MAP_HALF_Z_BLOCKS - f64::from(pixel) / f64::from(MAP_PANEL_HEIGHT - 1) * MAP_HALF_Z_BLOCKS * 2.0
}

fn terrain_color(sample: McloneOverworldTerrainSample) -> Rgb<u8> {
    if sample.continentalness <= 0.0 {
        let depth = ((63 - sample.surface_y).max(0) as f64 / 32.0).clamp(0.0, 1.0);
        Rgb([
            20,
            (92.0 - depth * 35.0) as u8,
            (155.0 - depth * 45.0) as u8,
        ])
    } else {
        let altitude = ((sample.surface_y - 63) as f64 / 70.0).clamp(0.0, 1.0);
        Rgb([
            (65.0 + altitude * 115.0) as u8,
            (128.0 + altitude * 70.0) as u8,
            (62.0 + altitude * 90.0) as u8,
        ])
    }
}

fn climate_color(sample: McloneOverworldTerrainSample) -> Rgb<u8> {
    let temperature = unit(sample.climate.temperature);
    let moisture = unit(sample.climate.moisture);
    Rgb([
        (temperature * 255.0) as u8,
        (moisture * 255.0) as u8,
        ((1.0 - (temperature - moisture).abs()) * 110.0) as u8,
    ])
}

fn latitude_color(latitude: f64) -> Rgb<u8> {
    signed_heat(latitude / 90.0)
}

fn annual_proxy_color(sample: McloneOverworldTerrainSample, latitude: f64) -> Rgb<u8> {
    let annual = (sample
        .climate
        .altitude_adjusted_temperature(sample.surface_y)
        - latitude.abs() / 90.0 * 0.65)
        .clamp(-1.0, 1.0);
    signed_heat(annual)
}

fn signed_heat(value: f64) -> Rgb<u8> {
    let value = value.clamp(-1.0, 1.0);
    if value >= 0.0 {
        Rgb([
            255,
            (255.0 * (1.0 - value)) as u8,
            (255.0 * (1.0 - value)) as u8,
        ])
    } else {
        let cold = -value;
        Rgb([
            (255.0 * (1.0 - cold)) as u8,
            (255.0 * (1.0 - cold)) as u8,
            255,
        ])
    }
}

fn unit(value: f64) -> f64 {
    (value * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn draw_latitude_guides(image: &mut RgbImage, policy: SolarCoordinatePolicy) -> Result<()> {
    for py in 0..MAP_PANEL_HEIGHT {
        let latitude = policy.latitude_at(0.0, map_z(py))?.degrees;
        let guide = latitude.abs() < 0.8 || (latitude.abs() - 90.0).abs() < 0.8;
        if guide {
            for px in 0..MAP_PANEL_WIDTH {
                image.put_pixel(px, py + MAP_PANEL_HEIGHT, Rgb([20, 20, 20]));
            }
        }
    }
    Ok(())
}

fn measure_continents(seed: i64) -> ContinentMetric {
    let sampler = McloneOverworldSampler::new(seed);
    let mut land = vec![false; CONNECTED_GRID * CONNECTED_GRID];
    for z in 0..CONNECTED_GRID {
        for x in 0..CONNECTED_GRID {
            let world_x = (x as i32 - CONNECTED_GRID as i32 / 2) * CONNECTED_STEP_BLOCKS;
            let world_z = (z as i32 - CONNECTED_GRID as i32 / 2) * CONNECTED_STEP_BLOCKS;
            land[z * CONNECTED_GRID + x] = sampler.sample(world_x, world_z).continentalness > 0.0;
        }
    }
    let land_count = land.iter().filter(|cell| **cell).count();
    let mut visited = vec![false; land.len()];
    let mut best = (0usize, 0usize, 0usize);
    for index in 0..land.len() {
        if !land[index] || visited[index] {
            continue;
        }
        let mut queue = VecDeque::from([index]);
        visited[index] = true;
        let mut count = 0;
        let mut min_x = CONNECTED_GRID;
        let mut max_x = 0;
        let mut min_z = CONNECTED_GRID;
        let mut max_z = 0;
        while let Some(cell) = queue.pop_front() {
            count += 1;
            let x = cell % CONNECTED_GRID;
            let z = cell / CONNECTED_GRID;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
            for (nx, nz) in [
                (x.wrapping_sub(1), z),
                (x + 1, z),
                (x, z.wrapping_sub(1)),
                (x, z + 1),
            ] {
                if nx < CONNECTED_GRID && nz < CONNECTED_GRID {
                    let neighbor = nz * CONNECTED_GRID + nx;
                    if land[neighbor] && !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        if count > best.0 {
            best = (count, max_x - min_x + 1, max_z - min_z + 1);
        }
    }
    ContinentMetric {
        seed,
        sampled_span_blocks: CONNECTED_GRID as i32 * CONNECTED_STEP_BLOCKS,
        sample_step_blocks: CONNECTED_STEP_BLOCKS,
        land_fraction: land_count as f64 / land.len() as f64,
        largest_component_area_blocks_approx: best.0 as u64
            * CONNECTED_STEP_BLOCKS as u64
            * CONNECTED_STEP_BLOCKS as u64,
        largest_component_width_blocks: best.1 as u32 * CONNECTED_STEP_BLOCKS as u32,
        largest_component_depth_blocks: best.2 as u32 * CONNECTED_STEP_BLOCKS as u32,
    }
}

fn solar_cases() -> Result<Vec<SolarCaseReceipt>> {
    let north_75_z = MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS
        * ((75.0_f64 / 90.0).asin() / std::f64::consts::TAU);
    [
        ("equator-north-solstice-noon", 0.0, 0.25, 12.0),
        (
            "north45-north-solstice-09",
            MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS / 12.0,
            0.25,
            9.0,
        ),
        ("north75-polar-day-midnight", north_75_z, 0.25, 0.0),
        ("north75-polar-night-noon", north_75_z, 0.75, 12.0),
        (
            "north-pole-summer-midnight",
            MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS / 4.0,
            0.25,
            0.0,
        ),
        (
            "south45-south-solstice-15",
            -MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS / 12.0,
            0.75,
            15.0,
        ),
        (
            "north45-equinox-sunrise",
            MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS / 12.0,
            0.0,
            6.0,
        ),
    ]
    .into_iter()
    .map(|(id, world_z, phase, hour)| solar_case(id, world_z, phase, hour))
    .collect()
}

fn solar_case(
    id: &'static str,
    world_z: f64,
    orbital_phase: f64,
    solar_time_hours: f64,
) -> Result<SolarCaseReceipt> {
    let latitude = SolarCoordinatePolicy::MCLONE_PLANE.latitude_at(0.0, world_z)?;
    let phase = OrbitalPhase::from_turns_wrapped(orbital_phase)?;
    let sample = SolarSample::compute(SolarInput {
        orbital_phase: phase,
        effective_latitude_degrees: latitude.degrees,
        axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
        solar_time_fraction: solar_time_hours / 24.0,
    })?;
    Ok(SolarCaseReceipt {
        id,
        world_x: 0.0,
        world_z,
        latitude_degrees: latitude.degrees,
        latitude_phase: latitude.phase.unwrap_or(0.0),
        orbital_phase: phase.turns(),
        solar_time_hours,
        axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
        direction: sample.direction,
        elevation_degrees: sample.elevation_degrees,
        azimuth_degrees: sample.azimuth_degrees,
        declination_degrees: sample.declination_degrees,
        daylight_factor: sample.daylight_factor,
        twilight_factor: sample.twilight_factor,
        day_length_hours: sample.day_length_fraction * 24.0,
        polar_state: sample.polar_state.label(),
    })
}
