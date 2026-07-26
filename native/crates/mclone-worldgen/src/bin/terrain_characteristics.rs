use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use mclone_worldgen::biome::OverworldBiomeSource;
use mclone_worldgen::block::{has_fluid, is_air_like};
use mclone_worldgen::levelgen::{
    AlphaGenerationStage, BetaGenerationStage, GeneratedChunk, MutableChunkBlockBuffer,
    NoiseBasedChunkGenerator, NoiseGeneratorSettings, generate_alpha_stage_chunk,
    generate_beta_stage_chunk, generate_mclone_overworld_surface_chunk,
};
use mclone_worldgen::terrain_analysis::{
    DEFAULT_TERRAIN_ANALYSIS_LAGS, DEFAULT_TERRAIN_PLANE_RADII, TerrainCharacteristics,
    TerrainHeightRaster, analyze_terrain_height_raster,
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-terrain-characteristics.json";
const DEFAULT_RADIUS_CHUNKS: i32 = 8;
const DEFAULT_LAND_MIN_Y: i32 = 67;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Profile {
    AlphaV1,
    BetaV1,
    Overworld,
    McloneOverworldV1,
}

impl Profile {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "alpha-v1" => Ok(Self::AlphaV1),
            "beta-v1" => Ok(Self::BetaV1),
            "overworld" => Ok(Self::Overworld),
            "mclone-overworld-v1" => Ok(Self::McloneOverworldV1),
            _ => Err(format!(
                "unknown terrain-characteristics profile `{value}`; expected alpha-v1, beta-v1, overworld, or mclone-overworld-v1"
            )),
        }
    }

    fn reference(self) -> &'static str {
        match self {
            Self::AlphaV1 => "Minecraft Java Alpha v1.1.2_01",
            Self::BetaV1 => "Minecraft Java Beta 1.7.3",
            Self::Overworld => "Minecraft Java 1.17.1",
            Self::McloneOverworldV1 => "Mclone original overworld",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteSpec {
    label: String,
    group: String,
    profile: Profile,
    seed: i64,
    center_chunk: [i32; 2],
}

#[derive(Clone, Debug)]
struct Config {
    output: PathBuf,
    radius_chunks: i32,
    land_min_y: i32,
    sites: Vec<SiteSpec>,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut output = PathBuf::from(DEFAULT_OUTPUT);
        let mut radius_chunks = DEFAULT_RADIUS_CHUNKS;
        let mut land_min_y = DEFAULT_LAND_MIN_Y;
        let mut sites = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output" => output = parse_next(&mut args, &arg)?,
                "--radius-chunks" => radius_chunks = parse_next(&mut args, &arg)?,
                "--land-min-y" => land_min_y = parse_next(&mut args, &arg)?,
                "--site" => {
                    let value: String = parse_next(&mut args, &arg)?;
                    sites.push(parse_site(&value)?);
                }
                "--help" | "-h" => return Err(usage().to_owned()),
                _ => return Err(format!("unknown argument `{arg}`\n{}", usage())),
            }
        }
        if !(2..=32).contains(&radius_chunks) {
            return Err("--radius-chunks must be between 2 and 32".to_owned());
        }
        if sites.is_empty() {
            sites = default_sites();
        }
        Ok(Self {
            output,
            radius_chunks,
            land_min_y,
            sites,
        })
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args().skip(1))?;
    let mut site_reports = Vec::with_capacity(config.sites.len());
    for site in &config.sites {
        eprintln!(
            "sampling {} ({:?}, seed {}, chunk {}, {})",
            site.label, site.profile, site.seed, site.center_chunk[0], site.center_chunk[1]
        );
        let started = Instant::now();
        let sampled = generate_height_raster(site, config.radius_chunks, config.land_min_y)?;
        let generation_elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let analysis_started = Instant::now();
        let characteristics = if sampled.included_columns == 0 {
            None
        } else {
            Some(analyze_terrain_height_raster(
                &sampled.raster,
                &DEFAULT_TERRAIN_ANALYSIS_LAGS,
                &DEFAULT_TERRAIN_PLANE_RADII,
            )?)
        };
        let analysis_elapsed_ms = analysis_started.elapsed().as_secs_f64() * 1_000.0;
        site_reports.push(SiteReport {
            site: site.clone(),
            generation_elapsed_ms,
            analysis_elapsed_ms,
            land_coverage: sampled.included_columns as f64 / sampled.total_columns as f64,
            characteristics,
        });
    }

    let groups = summarize_groups(&site_reports);
    let comparisons = [
        compare_groups(
            "mountains",
            "vanilla-mountains",
            "mclone-mountains",
            &groups,
        ),
        compare_groups("lowlands", "vanilla-lowlands", "mclone-lowlands", &groups),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let report = Report {
        schema: 2,
        profile_references: config
            .sites
            .iter()
            .map(|site| (site.profile, site.profile.reference()))
            .collect(),
        stage: "top solid terrain before carvers and decoration; one-block shared grid",
        radius_chunks: config.radius_chunks,
        land_min_y: config.land_min_y,
        lags_blocks: DEFAULT_TERRAIN_ANALYSIS_LAGS,
        plane_fit_radii_blocks: DEFAULT_TERRAIN_PLANE_RADII,
        sites: site_reports,
        groups,
        comparisons,
    };
    if let Some(parent) = config
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(
        &config.output,
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write {}: {error}", config.output.display()))?;

    print_comparisons(&report.comparisons);
    println!("receipt: {}", config.output.display());
    Ok(())
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse()
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn parse_site(value: &str) -> Result<SiteSpec, String> {
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 6 {
        return Err(format!(
            "invalid --site `{value}`; expected LABEL,GROUP,PROFILE,SEED,CHUNK_X,CHUNK_Z"
        ));
    }
    Ok(SiteSpec {
        label: parts[0].to_owned(),
        group: parts[1].to_owned(),
        profile: Profile::parse(parts[2])?,
        seed: parts[3]
            .parse()
            .map_err(|error| format!("invalid site seed `{}`: {error}", parts[3]))?,
        center_chunk: [
            parts[4]
                .parse()
                .map_err(|error| format!("invalid site chunk x `{}`: {error}", parts[4]))?,
            parts[5]
                .parse()
                .map_err(|error| format!("invalid site chunk z `{}`: {error}", parts[5]))?,
        ],
    })
}

fn usage() -> &'static str {
    "usage: terrain_characteristics [--output PATH] [--radius-chunks 2..32] \
     [--land-min-y Y] [--site LABEL,GROUP,PROFILE,SEED,CHUNK_X,CHUNK_Z]..."
}

fn default_sites() -> Vec<SiteSpec> {
    [
        (
            "vanilla-mountains",
            "vanilla-mountains",
            Profile::Overworld,
            33,
            -12,
            9,
        ),
        (
            "vanilla-gravelly-mountains",
            "vanilla-mountains",
            Profile::Overworld,
            250,
            -3,
            13,
        ),
        (
            "vanilla-snowy-mountains",
            "vanilla-mountains",
            Profile::Overworld,
            326,
            4,
            -21,
        ),
        (
            "mclone-range-negative",
            "mclone-mountains",
            Profile::McloneOverworldV1,
            -98_765,
            -186,
            25,
        ),
        (
            "mclone-range-positive",
            "mclone-mountains",
            Profile::McloneOverworldV1,
            12_345,
            -133,
            -66,
        ),
        (
            "mclone-mountain-valley",
            "mclone-mountains",
            Profile::McloneOverworldV1,
            -98_765,
            -204,
            22,
        ),
        (
            "vanilla-baseline-lowland",
            "vanilla-lowlands",
            Profile::Overworld,
            12_345,
            0,
            0,
        ),
        (
            "mclone-lowland-control",
            "mclone-lowlands",
            Profile::McloneOverworldV1,
            8_675_309,
            122,
            -96,
        ),
    ]
    .map(|(label, group, profile, seed, chunk_x, chunk_z)| SiteSpec {
        label: label.to_owned(),
        group: group.to_owned(),
        profile,
        seed,
        center_chunk: [chunk_x, chunk_z],
    })
    .to_vec()
}

fn generate_height_raster(
    site: &SiteSpec,
    radius_chunks: i32,
    land_min_y: i32,
) -> Result<SampledTerrain, String> {
    let chunk_diameter = radius_chunks * 2 + 1;
    let width = usize::try_from(chunk_diameter * 16)
        .map_err(|_| "terrain raster width does not fit usize")?;
    let mut heights = vec![0; width * width];
    let mut included = vec![false; width * width];
    let min_chunk_x = site.center_chunk[0] - radius_chunks;
    let min_chunk_z = site.center_chunk[1] - radius_chunks;

    match site.profile {
        Profile::AlphaV1 => {
            for chunk_offset_z in 0..chunk_diameter {
                for chunk_offset_x in 0..chunk_diameter {
                    let chunk = generate_alpha_stage_chunk(
                        site.seed,
                        min_chunk_x + chunk_offset_x,
                        min_chunk_z + chunk_offset_z,
                        false,
                        AlphaGenerationStage::Surface,
                    );
                    write_generated_heights(
                        &chunk,
                        chunk_offset_x,
                        chunk_offset_z,
                        width,
                        land_min_y,
                        &mut heights,
                        &mut included,
                    );
                }
            }
        }
        Profile::BetaV1 => {
            for chunk_offset_z in 0..chunk_diameter {
                for chunk_offset_x in 0..chunk_diameter {
                    let chunk = generate_beta_stage_chunk(
                        site.seed,
                        min_chunk_x + chunk_offset_x,
                        min_chunk_z + chunk_offset_z,
                        BetaGenerationStage::Surface,
                    );
                    write_generated_heights(
                        &chunk,
                        chunk_offset_x,
                        chunk_offset_z,
                        width,
                        land_min_y,
                        &mut heights,
                        &mut included,
                    );
                }
            }
        }
        Profile::Overworld => {
            let generator = NoiseBasedChunkGenerator::new(
                OverworldBiomeSource::new(site.seed, false, false),
                site.seed,
                NoiseGeneratorSettings::overworld(),
            );
            for chunk_offset_z in 0..chunk_diameter {
                for chunk_offset_x in 0..chunk_diameter {
                    let chunk = generator.fill_from_noise(
                        min_chunk_x + chunk_offset_x,
                        min_chunk_z + chunk_offset_z,
                    );
                    write_buffer_heights(
                        &chunk,
                        chunk_offset_x,
                        chunk_offset_z,
                        width,
                        land_min_y,
                        &mut heights,
                        &mut included,
                    );
                }
            }
        }
        Profile::McloneOverworldV1 => {
            for chunk_offset_z in 0..chunk_diameter {
                for chunk_offset_x in 0..chunk_diameter {
                    let chunk = generate_mclone_overworld_surface_chunk(
                        site.seed,
                        min_chunk_x + chunk_offset_x,
                        min_chunk_z + chunk_offset_z,
                    );
                    write_generated_heights(
                        &chunk,
                        chunk_offset_x,
                        chunk_offset_z,
                        width,
                        land_min_y,
                        &mut heights,
                        &mut included,
                    );
                }
            }
        }
    }
    let included_columns = included.iter().filter(|included| **included).count();
    let total_columns = included.len();
    Ok(SampledTerrain {
        raster: TerrainHeightRaster::new(width, width, heights, included)?,
        included_columns,
        total_columns,
    })
}

#[derive(Clone, Debug)]
struct SampledTerrain {
    raster: TerrainHeightRaster,
    included_columns: usize,
    total_columns: usize,
}

#[allow(clippy::too_many_arguments)]
fn write_buffer_heights(
    chunk: &MutableChunkBlockBuffer,
    chunk_offset_x: i32,
    chunk_offset_z: i32,
    width: usize,
    land_min_y: i32,
    heights: &mut [i32],
    included: &mut [bool],
) {
    for local_z in 0..16 {
        for local_x in 0..16 {
            let top = top_solid_y(chunk.min_y, chunk.height, |y| {
                chunk.get_block_at_y(local_x, y, local_z)
            });
            write_height(
                chunk_offset_x,
                chunk_offset_z,
                local_x,
                local_z,
                width,
                land_min_y,
                top,
                heights,
                included,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_generated_heights(
    chunk: &GeneratedChunk,
    chunk_offset_x: i32,
    chunk_offset_z: i32,
    width: usize,
    land_min_y: i32,
    heights: &mut [i32],
    included: &mut [bool],
) {
    for local_z in 0..16 {
        for local_x in 0..16 {
            let top = top_solid_y(chunk.min_y, chunk.height, |y| {
                chunk.block_at_y(local_x, y, local_z).raw()
            });
            write_height(
                chunk_offset_x,
                chunk_offset_z,
                local_x,
                local_z,
                width,
                land_min_y,
                top,
                heights,
                included,
            );
        }
    }
}

fn top_solid_y(min_y: i32, height: i32, block_at: impl Fn(i32) -> u8) -> i32 {
    for y in (min_y..min_y + height).rev() {
        let block = block_at(y);
        if !is_air_like(block) && !has_fluid(block) {
            return y;
        }
    }
    min_y
}

#[allow(clippy::too_many_arguments)]
fn write_height(
    chunk_offset_x: i32,
    chunk_offset_z: i32,
    local_x: i32,
    local_z: i32,
    width: usize,
    land_min_y: i32,
    top: i32,
    heights: &mut [i32],
    included: &mut [bool],
) {
    let x = usize::try_from(chunk_offset_x * 16 + local_x).expect("nonnegative raster x");
    let z = usize::try_from(chunk_offset_z * 16 + local_z).expect("nonnegative raster z");
    let index = z * width + x;
    heights[index] = top;
    included[index] = top >= land_min_y;
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteReport {
    #[serde(flatten)]
    site: SiteSpec,
    generation_elapsed_ms: f64,
    analysis_elapsed_ms: f64,
    land_coverage: f64,
    characteristics: Option<TerrainCharacteristics>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureVector {
    included_share: f64,
    lag1_rms_delta: f64,
    lag4_rms_delta: f64,
    lag16_rms_delta: f64,
    lag64_rms_delta: f64,
    curvature_rms: f64,
    plane_r4_rmse: f64,
    plane_r8_rmse: f64,
    plane_r16_rmse: f64,
    plane_r32_rmse: f64,
    orientation_r4_coherence: f64,
    orientation_r4_diagonal: f64,
    orientation_r8_coherence: f64,
    orientation_r8_diagonal: f64,
    roughness_exponent: f64,
    fine_detail_share_r4_of_r32: f64,
}

impl FeatureVector {
    fn from_characteristics(value: &TerrainCharacteristics) -> Self {
        Self {
            included_share: value.included_share,
            lag1_rms_delta: lag_rms(value, 1),
            lag4_rms_delta: lag_rms(value, 4),
            lag16_rms_delta: lag_rms(value, 16),
            lag64_rms_delta: lag_rms(value, 64),
            curvature_rms: value.curvature.rms,
            plane_r4_rmse: plane_rmse(value, 4),
            plane_r8_rmse: plane_rmse(value, 8),
            plane_r16_rmse: plane_rmse(value, 16),
            plane_r32_rmse: plane_rmse(value, 32),
            orientation_r4_coherence: orientation_coherence(value, 4),
            orientation_r4_diagonal: orientation_diagonal(value, 4),
            orientation_r8_coherence: orientation_coherence(value, 8),
            orientation_r8_diagonal: orientation_diagonal(value, 8),
            roughness_exponent: value.roughness_exponent.unwrap_or(0.0),
            fine_detail_share_r4_of_r32: value.fine_detail_share_r4_of_r32.unwrap_or(0.0),
        }
    }

    fn rows(self) -> [(&'static str, f64); 16] {
        [
            ("included land share", self.included_share),
            ("lag 1 RMS delta", self.lag1_rms_delta),
            ("lag 4 RMS delta", self.lag4_rms_delta),
            ("lag 16 RMS delta", self.lag16_rms_delta),
            ("lag 64 RMS delta", self.lag64_rms_delta),
            ("curvature RMS", self.curvature_rms),
            ("plane r4 RMSE", self.plane_r4_rmse),
            ("plane r8 RMSE", self.plane_r8_rmse),
            ("plane r16 RMSE", self.plane_r16_rmse),
            ("plane r32 RMSE", self.plane_r32_rmse),
            ("orientation r4 coherence", self.orientation_r4_coherence),
            ("orientation r4 diagonal", self.orientation_r4_diagonal),
            ("orientation r8 coherence", self.orientation_r8_coherence),
            ("orientation r8 diagonal", self.orientation_r8_diagonal),
            ("roughness exponent", self.roughness_exponent),
            ("fine detail share r4/r32", self.fine_detail_share_r4_of_r32),
        ]
    }
}

fn lag_rms(value: &TerrainCharacteristics, lag: usize) -> f64 {
    value
        .lag_curve
        .iter()
        .find(|band| band.lag_blocks == lag)
        .map_or(0.0, |band| band.rms_height_delta)
}

fn plane_rmse(value: &TerrainCharacteristics, radius: usize) -> f64 {
    value
        .plane_fit_curve
        .iter()
        .find(|scale| scale.radius_blocks == radius)
        .map_or(0.0, |scale| scale.pooled_rmse)
}

fn orientation_coherence(value: &TerrainCharacteristics, radius: usize) -> f64 {
    value
        .orientation_curve
        .iter()
        .find(|scale| scale.radius_blocks == radius)
        .map_or(0.0, |scale| scale.mean_coherence)
}

fn orientation_diagonal(value: &TerrainCharacteristics, radius: usize) -> f64 {
    value
        .orientation_curve
        .iter()
        .find(|scale| scale.radius_blocks == radius)
        .map_or(0.0, |scale| scale.mean_diagonal_coherence)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupSummary {
    window_count: usize,
    analyzed_window_count: usize,
    land_bearing_window_share: f64,
    median_land_coverage: f64,
    median: Option<FeatureVector>,
}

fn summarize_groups(reports: &[SiteReport]) -> BTreeMap<String, GroupSummary> {
    let mut grouped = BTreeMap::<String, Vec<&SiteReport>>::new();
    for report in reports {
        grouped
            .entry(report.site.group.clone())
            .or_default()
            .push(report);
    }
    grouped
        .into_iter()
        .map(|(group, reports)| {
            let values = reports
                .iter()
                .filter_map(|report| report.characteristics.as_ref())
                .map(FeatureVector::from_characteristics)
                .collect::<Vec<_>>();
            let mut land_coverages = reports
                .iter()
                .map(|report| report.land_coverage)
                .collect::<Vec<_>>();
            land_coverages.sort_by(f64::total_cmp);
            let window_count = reports.len();
            let analyzed_window_count = values.len();
            (
                group,
                GroupSummary {
                    window_count,
                    analyzed_window_count,
                    land_bearing_window_share: analyzed_window_count as f64 / window_count as f64,
                    median_land_coverage: land_coverages[(land_coverages.len() - 1) / 2],
                    median: (!values.is_empty()).then(|| median_feature_vector(&values)),
                },
            )
        })
        .collect()
}

fn median_feature_vector(values: &[FeatureVector]) -> FeatureVector {
    let rows = values.iter().map(|value| value.rows()).collect::<Vec<_>>();
    let mut medians = [0.0; 16];
    for index in 0..medians.len() {
        let mut column = rows.iter().map(|row| row[index].1).collect::<Vec<_>>();
        column.sort_by(f64::total_cmp);
        medians[index] = column[(column.len() - 1) / 2];
    }
    FeatureVector {
        included_share: medians[0],
        lag1_rms_delta: medians[1],
        lag4_rms_delta: medians[2],
        lag16_rms_delta: medians[3],
        lag64_rms_delta: medians[4],
        curvature_rms: medians[5],
        plane_r4_rmse: medians[6],
        plane_r8_rmse: medians[7],
        plane_r16_rmse: medians[8],
        plane_r32_rmse: medians[9],
        orientation_r4_coherence: medians[10],
        orientation_r4_diagonal: medians[11],
        orientation_r8_coherence: medians[12],
        orientation_r8_diagonal: medians[13],
        roughness_exponent: medians[14],
        fine_detail_share_r4_of_r32: medians[15],
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Comparison {
    label: &'static str,
    reference_group: &'static str,
    candidate_group: &'static str,
    rows: Vec<ComparisonRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonRow {
    feature: &'static str,
    reference: f64,
    candidate: f64,
    ratio: Option<f64>,
    log2_ratio_magnitude: Option<f64>,
}

fn compare_groups(
    label: &'static str,
    reference_group: &'static str,
    candidate_group: &'static str,
    groups: &BTreeMap<String, GroupSummary>,
) -> Option<Comparison> {
    let reference = groups.get(reference_group)?.median?;
    let candidate = groups.get(candidate_group)?.median?;
    let reference_rows = reference.rows();
    let candidate_rows = candidate.rows();
    let mut rows = reference_rows
        .into_iter()
        .zip(candidate_rows)
        .map(|((feature, reference), (_, candidate))| {
            let ratio = (reference.abs() > 1e-12).then_some(candidate / reference);
            ComparisonRow {
                feature,
                reference,
                candidate,
                ratio,
                log2_ratio_magnitude: ratio
                    .filter(|ratio| *ratio > 0.0)
                    .map(|ratio| ratio.log2().abs()),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .log2_ratio_magnitude
            .unwrap_or(0.0)
            .total_cmp(&left.log2_ratio_magnitude.unwrap_or(0.0))
    });
    Some(Comparison {
        label,
        reference_group,
        candidate_group,
        rows,
    })
}

fn print_comparisons(comparisons: &[Comparison]) {
    for comparison in comparisons {
        println!(
            "{}: {} vs {}",
            comparison.label, comparison.candidate_group, comparison.reference_group
        );
        println!(
            "  {:<28} {:>10} {:>10} {:>8}",
            "feature", "vanilla", "mclone", "ratio"
        );
        for row in &comparison.rows {
            println!(
                "  {:<28} {:>10.4} {:>10.4} {:>8}",
                row.feature,
                row.reference,
                row.candidate,
                row.ratio
                    .map(|ratio| format!("{ratio:.3}x"))
                    .unwrap_or_else(|| "n/a".to_owned())
            );
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema: u32,
    profile_references: BTreeMap<Profile, &'static str>,
    stage: &'static str,
    radius_chunks: i32,
    land_min_y: i32,
    lags_blocks: [usize; 8],
    plane_fit_radii_blocks: [usize; 5],
    sites: Vec<SiteReport>,
    groups: BTreeMap<String, GroupSummary>,
    comparisons: Vec<Comparison>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_parser_accepts_signed_seed_and_chunks() {
        let site = parse_site("test,group,mclone-overworld-v1,-9,-4,7").unwrap();
        assert_eq!(site.seed, -9);
        assert_eq!(site.center_chunk, [-4, 7]);
        assert_eq!(site.profile, Profile::McloneOverworldV1);
    }

    #[test]
    fn site_parser_accepts_all_reference_profiles() {
        assert_eq!(
            parse_site("alpha,era,alpha-v1,1,2,3").unwrap().profile,
            Profile::AlphaV1
        );
        assert_eq!(
            parse_site("beta,era,beta-v1,1,2,3").unwrap().profile,
            Profile::BetaV1
        );
        assert_eq!(
            parse_site("release,era,overworld,1,2,3").unwrap().profile,
            Profile::Overworld
        );
    }

    #[test]
    fn default_suite_has_paired_mountain_and_lowland_groups() {
        let sites = default_sites();
        let groups = sites
            .iter()
            .map(|site| site.group.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            groups,
            std::collections::BTreeSet::from([
                "mclone-lowlands",
                "mclone-mountains",
                "vanilla-lowlands",
                "vanilla-mountains",
            ])
        );
    }

    #[test]
    fn group_summary_retains_an_all_water_window() {
        let reports = [SiteReport {
            site: SiteSpec {
                label: "ocean".to_owned(),
                group: "survey".to_owned(),
                profile: Profile::AlphaV1,
                seed: 1,
                center_chunk: [0, 0],
            },
            generation_elapsed_ms: 1.0,
            analysis_elapsed_ms: 0.0,
            land_coverage: 0.0,
            characteristics: None,
        }];
        let summary = summarize_groups(&reports);
        let survey = &summary["survey"];
        assert_eq!(survey.window_count, 1);
        assert_eq!(survey.analyzed_window_count, 0);
        assert_eq!(survey.land_bearing_window_share, 0.0);
        assert_eq!(survey.median_land_coverage, 0.0);
        assert!(survey.median.is_none());
    }
}
