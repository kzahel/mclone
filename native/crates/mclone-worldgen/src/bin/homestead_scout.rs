use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use mclone_core::{BlockPos, HorizontalTopology};
use mclone_worldgen::homestead_site::{
    FALLBACK_SEARCH_RADIUS, FlatGrassHomesteadSurveySource, HomesteadScoutError,
    HomesteadScoutReceipt, HomesteadScoutRequest, HomesteadSurveySource,
    McloneOverworldHomesteadSurveySource, scout_homestead_site,
};
use mclone_worldgen::levelgen::{
    McloneOverworldSamplingTopology, mclone_overworld_homestead_scout_origin_chunk_with_topology,
};
use serde::Serialize;

#[derive(Serialize)]
struct ScoutArtifact<'a> {
    profile: &'a str,
    elapsed_micros: u128,
    receipt: &'a HomesteadScoutReceipt,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut profile = "mclone".to_owned();
    let mut seed = 8_675_309_i64;
    let mut output = PathBuf::from("/tmp/mclone-homestead-scout");
    let mut write_map = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => profile = args.next().ok_or("--profile needs a value")?,
            "--seed" => seed = args.next().ok_or("--seed needs a value")?.parse()?,
            "--out" => output = PathBuf::from(args.next().ok_or("--out needs a value")?),
            "--no-map" => write_map = false,
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }

    let spawn = if profile == "mclone" {
        let chunk = mclone_overworld_homestead_scout_origin_chunk_with_topology(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
        );
        BlockPos::new(chunk.min_block_x() + 8, 0, chunk.min_block_z() + 8)
    } else {
        BlockPos::new(8, 4, 8)
    };
    let request = HomesteadScoutRequest {
        seed,
        provisional_spawn: spawn,
        topology: HorizontalTopology::UNBOUNDED,
    };

    match profile.as_str() {
        "mclone" => run(
            &profile,
            &output,
            request,
            write_map,
            McloneOverworldHomesteadSurveySource::new(
                seed,
                McloneOverworldSamplingTopology::Unbounded,
            ),
        )?,
        "flat" => run(
            &profile,
            &output,
            request,
            write_map,
            FlatGrassHomesteadSurveySource,
        )?,
        _ => return Err(format!("unsupported profile {profile}; use mclone or flat").into()),
    }
    Ok(())
}

fn run(
    profile: &str,
    output: &PathBuf,
    request: HomesteadScoutRequest,
    write_map: bool,
    mut source: impl HomesteadSurveySource,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let receipt = match scout_homestead_site(&mut source, request) {
        Ok(receipt) => receipt,
        Err(HomesteadScoutError::NoSafeSite(receipt)) => *receipt,
        Err(error) => return Err(error.into()),
    };
    let elapsed_micros = started.elapsed().as_micros();
    let json_path = output.with_extension("json");
    let svg_path = output.with_extension("svg");
    fs::write(
        &json_path,
        serde_json::to_vec_pretty(&ScoutArtifact {
            profile,
            elapsed_micros,
            receipt: &receipt,
        })?,
    )?;
    if write_map {
        fs::write(&svg_path, scout_svg(&mut source, &receipt)?)?;
    }
    println!(
        "profile={profile} seed={} elapsed_us={elapsed_micros} evaluations={} anchors={} selected={} json={} svg={}",
        request.seed,
        receipt.evaluation_count,
        receipt.unique_anchor_count,
        receipt
            .selected
            .as_ref()
            .map(|selected| format!(
                "{:?}@({}, {})/{:?}",
                selected.tier,
                selected.candidate.anchor_x,
                selected.candidate.anchor_z,
                selected.facing
            ))
            .unwrap_or_else(|| "none".to_owned()),
        json_path.display(),
        if write_map {
            svg_path.display().to_string()
        } else {
            "disabled".to_owned()
        },
    );
    Ok(())
}

fn scout_svg(
    source: &mut impl HomesteadSurveySource,
    receipt: &HomesteadScoutReceipt,
) -> Result<String, Box<dyn std::error::Error>> {
    const SIZE: i32 = 800;
    const STEP: i32 = 16;
    let origin_x = receipt.provisional_spawn[0];
    let origin_z = receipt.provisional_spawn[2];
    let radius = FALLBACK_SEARCH_RADIUS;
    let cells = radius * 2 / STEP + 1;
    let cell = f64::from(SIZE) / f64::from(cells);
    let mut sampled = Vec::new();
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for grid_z in 0..cells {
        for grid_x in 0..cells {
            let x = origin_x - radius + grid_x * STEP;
            let z = origin_z - radius + grid_z * STEP;
            let sample = source.sample_column(x, z)?;
            min_y = min_y.min(sample.surface_y);
            max_y = max_y.max(sample.surface_y);
            sampled.push((grid_x, grid_z, sample));
        }
    }

    let mut svg = String::new();
    writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}' viewBox='0 0 {SIZE} {SIZE}'>"
    )?;
    writeln!(
        svg,
        "<defs><marker id='arrival-arrow' markerWidth='8' markerHeight='8' refX='6' refY='3' orient='auto'><path d='M0,0 L0,6 L7,3 z' fill='#ffffff'/></marker></defs>"
    )?;
    writeln!(svg, "<rect width='{SIZE}' height='{SIZE}' fill='#101820'/>")?;
    let height_range = (max_y - min_y).max(1);
    for (grid_x, grid_z, sample) in sampled {
        let color = if sample.protected_content {
            "#d94dbb".to_owned()
        } else if sample.fluid {
            "#3478b8".to_owned()
        } else {
            let light = 28 + (sample.surface_y - min_y) * 42 / height_range;
            let green = 75 + i32::from(sample.meadow_milli) * 45 / 1_000;
            format!("hsl({green} 42% {light}%)")
        };
        writeln!(
            svg,
            "<rect x='{:.2}' y='{:.2}' width='{:.2}' height='{:.2}' fill='{color}'/>",
            f64::from(grid_x) * cell,
            f64::from(grid_z) * cell,
            cell + 0.25,
            cell + 0.25,
        )?;
    }
    for candidate in &receipt.retained_candidates {
        let x = map_coord(candidate.candidate.anchor_x, origin_x, radius, SIZE);
        let z = map_coord(candidate.candidate.anchor_z, origin_z, radius, SIZE);
        writeln!(
            svg,
            "<circle cx='{x:.2}' cy='{z:.2}' r='3' fill='#ffd166' stroke='#101820' stroke-width='1'/>",
        )?;
    }
    if let Some(selected) = &receipt.selected {
        let grade_cell = f64::from(STEP) * f64::from(SIZE) / f64::from(radius * 2);
        for sample_z in
            (selected.core_bounds.min_z..=selected.core_bounds.max_z).step_by(STEP as usize)
        {
            for sample_x in
                (selected.core_bounds.min_x..=selected.core_bounds.max_x).step_by(STEP as usize)
            {
                let sample = source.sample_column(sample_x, sample_z)?;
                let grade = sample.surface_y - selected.metrics.target_surface_y;
                if grade == 0 {
                    continue;
                }
                let color = if grade > 0 { "#ef4444" } else { "#38bdf8" };
                let opacity = (0.25 + f64::from(grade.abs().min(5)) * 0.1).min(0.75);
                let x = map_coord(sample_x, origin_x, radius, SIZE) - grade_cell * 0.5;
                let z = map_coord(sample_z, origin_z, radius, SIZE) - grade_cell * 0.5;
                writeln!(
                    svg,
                    "<rect x='{x:.2}' y='{z:.2}' width='{grade_cell:.2}' height='{grade_cell:.2}' fill='{color}' fill-opacity='{opacity:.2}'/>"
                )?;
            }
        }
        let x = map_coord(selected.reservation_bounds.min_x, origin_x, radius, SIZE);
        let y = map_coord(selected.reservation_bounds.min_z, origin_z, radius, SIZE);
        let max_x = map_coord(selected.reservation_bounds.max_x, origin_x, radius, SIZE);
        let max_y = map_coord(selected.reservation_bounds.max_z, origin_z, radius, SIZE);
        writeln!(
            svg,
            "<rect x='{x:.2}' y='{y:.2}' width='{:.2}' height='{:.2}' fill='none' stroke='#ff6b35' stroke-width='3'/>",
            max_x - x,
            max_y - y,
        )?;
        let core_x = map_coord(selected.core_bounds.min_x, origin_x, radius, SIZE);
        let core_y = map_coord(selected.core_bounds.min_z, origin_z, radius, SIZE);
        let core_max_x = map_coord(selected.core_bounds.max_x, origin_x, radius, SIZE);
        let core_max_y = map_coord(selected.core_bounds.max_z, origin_z, radius, SIZE);
        writeln!(
            svg,
            "<rect x='{core_x:.2}' y='{core_y:.2}' width='{:.2}' height='{:.2}' fill='none' stroke='#facc15' stroke-width='2' stroke-dasharray='5 3'/>",
            core_max_x - core_x,
            core_max_y - core_y,
        )?;
        let arrival_x = map_coord(selected.arrival[0], origin_x, radius, SIZE);
        let arrival_z = map_coord(selected.arrival[2], origin_z, radius, SIZE);
        let anchor_x = map_coord(selected.candidate.anchor_x, origin_x, radius, SIZE);
        let anchor_z = map_coord(selected.candidate.anchor_z, origin_z, radius, SIZE);
        writeln!(
            svg,
            "<line x1='{arrival_x:.2}' y1='{arrival_z:.2}' x2='{anchor_x:.2}' y2='{anchor_z:.2}' stroke='#ffffff' stroke-width='2' marker-end='url(#arrival-arrow)'/>"
        )?;
        writeln!(
            svg,
            "<circle cx='{arrival_x:.2}' cy='{arrival_z:.2}' r='6' fill='#ffffff' stroke='#111827' stroke-width='2'/>",
        )?;
    }
    let spawn_x = map_coord(origin_x, origin_x, radius, SIZE);
    let spawn_z = map_coord(origin_z, origin_z, radius, SIZE);
    writeln!(
        svg,
        "<path d='M {} {} l -7 14 h 14 z' fill='#ef476f' stroke='#fff' stroke-width='1'/>",
        spawn_x,
        spawn_z - 8.0,
    )?;
    writeln!(
        svg,
        "<rect x='10' y='10' width='540' height='72' rx='5' fill='#101820dd'/><text x='22' y='32' fill='white' font-family='monospace' font-size='15'>seed {} · y {}..{} · {} evals</text><text x='22' y='51' fill='#ffd166' font-family='monospace' font-size='13'>yellow finalists/core · orange reservation · red spawn</text><text x='22' y='69' fill='white' font-family='monospace' font-size='13'>grade red=cut cyan=fill · white arrival arrow=sightline</text>",
        receipt.seed, min_y, max_y, receipt.evaluation_count,
    )?;
    svg.push_str("</svg>\n");
    Ok(svg)
}

fn map_coord(value: i32, origin: i32, radius: i32, size: i32) -> f64 {
    f64::from(value - origin + radius) * f64::from(size) / f64::from(radius * 2)
}
