use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Vec3, Vec4};
use mclone_app_runtime::lod_coverage::LodReplacementCounters;
use mclone_core::{ChunkPos, LodTileKey, chunk_middle_block_coord};
use mclone_render::chunk::ChunkRenderView;
use mclone_render::color_profile::{RenderConfig, color_transform_wgpu};
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png,
};
use mclone_scene::{FarLodChunkLedgerRow, FarLodSettleSnapshot};
use serde::Deserialize;

use crate::camera::SpectatorCamera;
use crate::cli::{LodSettleProbeOptions, StartupWaitPolicy};
use crate::offscreen_flat_client::{OffscreenFlatClientFrameOptions, OffscreenFlatClientHost};
use crate::offscreen_scene_host::OffscreenWarmupReport;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::WindowSceneAssets;

const SPAWN_FIXTURE: &str = "rd4-range6-spawn-settle";
const FLY_UP_FIXTURE: &str = "rd4-range6-fly-up-high";
const MIN_SCRIPT_SCHEMA: u32 = 1;
const SCRIPT_SCHEMA: u32 = 2;
const MAX_WAYPOINTS: usize = 64;
const MAX_CAPTURE_SETTLE_PASSES: usize = 4;
const FLY_UP_EYE_OFFSET_X: f32 = 0.25;
const FLY_UP_EYE_Y: f32 = 200.0;
const FLY_UP_EYE_OFFSET_Z: f32 = 0.25;
const FLY_UP_TARGET_Y: f32 = 64.0;

pub(crate) struct LodSettleProbeReport {
    value: serde_json::Value,
    expectations_met: bool,
}

impl LodSettleProbeReport {
    pub(crate) fn print_json(&self) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(&self.value)?);
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if !self.expectations_met {
            bail!("far LOD settle probe did not match its pinned fixture expectations");
        }
        Ok(())
    }
}

struct ProbeHost {
    host: OffscreenFlatClientHost,
    script: LodSettleScript,
    startup_warmup: Option<OffscreenWarmupReport>,
    observations: Vec<FixtureObservation>,
}

struct FixtureObservation {
    name: String,
    expected: LodSettleExpectation,
    warmup: OffscreenWarmupReport,
    summary: mclone_app_runtime::frame_render::FullFrameRenderSummary,
    render_view: ChunkRenderView,
    snapshot: FarLodSettleSnapshot,
    pending_stream_work: usize,
    budget_panel: mclone_diagnostics::BudgetDecisionPanelReport,
    lod_counters: LodReplacementCounters,
    coverage_probe: Option<LodCoverageProbe>,
    matches_pixels: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LodSettleScript {
    schema: u32,
    name: String,
    waypoints: Vec<LodSettleWaypoint>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LodSettleWaypoint {
    name: String,
    camera: LodSettleCamera,
    expected: LodSettleExpectation,
    #[serde(default)]
    coverage_probe: Option<LodCoverageProbe>,
    #[serde(default)]
    matches_pixels: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LodCoverageProbe {
    plane_y: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum LodSettleCamera {
    SpawnSurface,
    LookAt { eye: [f32; 3], target: [f32; 3] },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum LodSettleExpectation {
    Pass,
    FailD1D2,
}

impl LodSettleExpectation {
    const fn report_name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::FailD1D2 => "fail:d1-d2",
        }
    }
}

impl LodSettleScript {
    fn load(path: Option<&Path>, scene: &crate::cli::SceneOptions) -> Result<Self> {
        let script = if let Some(path) = path {
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("read far LOD settle script {}", path.display()))?;
            serde_json::from_str(&source)
                .with_context(|| format!("parse far LOD settle script {}", path.display()))?
        } else {
            Self::default_for_scene(scene)
        };
        script.validate()?;
        Ok(script)
    }

    fn default_for_scene(scene: &crate::cli::SceneOptions) -> Self {
        let center_x = chunk_middle_block_coord(scene.chunk_x) as f32;
        let center_z = chunk_middle_block_coord(scene.chunk_z) as f32;
        Self {
            schema: SCRIPT_SCHEMA,
            name: "built-in-rd4-range6-smoke".to_owned(),
            waypoints: vec![
                LodSettleWaypoint {
                    name: SPAWN_FIXTURE.to_owned(),
                    camera: LodSettleCamera::SpawnSurface,
                    expected: LodSettleExpectation::Pass,
                    coverage_probe: None,
                    matches_pixels: None,
                },
                LodSettleWaypoint {
                    name: FLY_UP_FIXTURE.to_owned(),
                    camera: LodSettleCamera::LookAt {
                        eye: [
                            center_x + FLY_UP_EYE_OFFSET_X,
                            FLY_UP_EYE_Y,
                            center_z + FLY_UP_EYE_OFFSET_Z,
                        ],
                        target: [center_x, FLY_UP_TARGET_Y, center_z],
                    },
                    expected: LodSettleExpectation::FailD1D2,
                    coverage_probe: None,
                    matches_pixels: None,
                },
            ],
        }
    }

    fn validate(&self) -> Result<()> {
        if !(MIN_SCRIPT_SCHEMA..=SCRIPT_SCHEMA).contains(&self.schema) {
            bail!(
                "far LOD settle script schema must be {MIN_SCRIPT_SCHEMA}..={SCRIPT_SCHEMA}, got {}",
                self.schema
            );
        }
        validate_safe_name("script", &self.name)?;
        if self.waypoints.is_empty() || self.waypoints.len() > MAX_WAYPOINTS {
            bail!("far LOD settle script requires 1..={MAX_WAYPOINTS} waypoints");
        }
        let mut prior_waypoints = BTreeMap::<&str, &LodSettleWaypoint>::new();
        for waypoint in &self.waypoints {
            validate_safe_name("waypoint", &waypoint.name)?;
            if prior_waypoints.contains_key(waypoint.name.as_str()) {
                bail!("duplicate far LOD settle waypoint `{}`", waypoint.name);
            }
            if let LodSettleCamera::LookAt { eye, target } = waypoint.camera {
                if !eye.into_iter().chain(target).all(f32::is_finite) {
                    bail!(
                        "waypoint `{}` camera coordinates must be finite",
                        waypoint.name
                    );
                }
                if eye == target {
                    bail!(
                        "waypoint `{}` camera eye and target must differ",
                        waypoint.name
                    );
                }
            }
            if self.schema < 2
                && (waypoint.coverage_probe.is_some() || waypoint.matches_pixels.is_some())
            {
                bail!(
                    "waypoint `{}` coverageProbe/matchesPixels requires script schema 2",
                    waypoint.name
                );
            }
            if let Some(probe) = waypoint.coverage_probe {
                validate_coverage_probe(waypoint, probe)?;
            }
            if let Some(reference) = waypoint.matches_pixels.as_deref() {
                validate_safe_name("matchesPixels", reference)?;
                let Some(prior) = prior_waypoints.get(reference) else {
                    bail!(
                        "waypoint `{}` matchesPixels must name an earlier waypoint, got `{reference}`",
                        waypoint.name
                    );
                };
                if prior.camera != waypoint.camera {
                    bail!(
                        "waypoint `{}` matchesPixels camera differs from `{reference}`",
                        waypoint.name
                    );
                }
            }
            prior_waypoints.insert(waypoint.name.as_str(), waypoint);
        }
        Ok(())
    }
}

fn validate_coverage_probe(waypoint: &LodSettleWaypoint, probe: LodCoverageProbe) -> Result<()> {
    if !probe.plane_y.is_finite() {
        bail!(
            "waypoint `{}` coverage planeY must be finite",
            waypoint.name
        );
    }
    let LodSettleCamera::LookAt { eye, target } = waypoint.camera else {
        bail!(
            "waypoint `{}` coverageProbe requires an explicit top-down look-at camera",
            waypoint.name
        );
    };
    let eye = Vec3::from_array(eye);
    let target = Vec3::from_array(target);
    let direction = target - eye;
    let downward_fraction = -direction.y / direction.length();
    if eye.y <= probe.plane_y || downward_fraction < 0.95 {
        bail!(
            "waypoint `{}` coverageProbe camera must look down from above planeY",
            waypoint.name
        );
    }
    Ok(())
}

fn validate_safe_name(kind: &str, name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        bail!(
            "far LOD settle {kind} name `{name}` must use 1..=64 lowercase ASCII letters, digits, or hyphens"
        );
    }
    Ok(())
}

pub(crate) fn run_lod_settle_probe(
    options: &LodSettleProbeOptions,
) -> Result<LodSettleProbeReport> {
    let script = LodSettleScript::load(options.script.as_deref(), &options.scene)?;
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create far LOD settle probe directory {}",
            options.directory.display()
        )
    })?;
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let (loop_report, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: script.waypoints.len(),
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                startup_camera,
            )?;
            let startup_warmup =
                host.start_scene_with_wait_policy_report(device, queue, StartupWaitPolicy::Idle)?;
            Ok(ProbeHost {
                host,
                observations: Vec::with_capacity(script.waypoints.len()),
                script,
                startup_warmup: Some(startup_warmup),
            })
        },
        |index, frame, state| {
            let waypoint = state.script.waypoints[index].clone();
            match waypoint.camera {
                LodSettleCamera::SpawnSurface => state.host.place_camera_above_loaded_surface()?,
                LodSettleCamera::LookAt { eye, target } => {
                    state
                        .host
                        .set_camera_look_at(Vec3::from_array(eye), Vec3::from_array(target));
                    state.host.commit_camera()?;
                }
            }
            let mut warmup = drive_until_streamed_with_diagnostics(
                &mut state.host,
                frame.device,
                frame.queue,
                &waypoint.name,
            )?;
            if let Some(startup_warmup) = state.startup_warmup.take() {
                warmup = merge_warmups(startup_warmup, warmup);
            }
            let device = frame.device;
            let queue = frame.queue;
            let target = frame.target;
            let encoder = frame.encoder;
            let mut summary = state.host.render_frame(
                mclone_render::target::RenderFrameContext::new(
                    device,
                    queue,
                    &mut *encoder,
                    target,
                ),
                OffscreenFlatClientFrameOptions { hud: false },
            )?;
            for _ in 1..MAX_CAPTURE_SETTLE_PASSES {
                if state.host.pending_stream_work() == 0 {
                    break;
                }
                warmup = merge_warmups(
                    warmup,
                    drive_until_streamed_with_diagnostics(
                        &mut state.host,
                        device,
                        queue,
                        &waypoint.name,
                    )?,
                );
                summary = state.host.render_frame(
                    mclone_render::target::RenderFrameContext::new(
                        device,
                        queue,
                        &mut *encoder,
                        target,
                    ),
                    OffscreenFlatClientFrameOptions { hud: false },
                )?;
            }
            let snapshot = state.host.far_lod_settle_snapshot()?;
            let render_view = state.host.render_view(target.size)?;
            state.observations.push(FixtureObservation {
                name: waypoint.name,
                expected: waypoint.expected,
                warmup,
                summary,
                render_view,
                snapshot,
                pending_stream_work: state.host.pending_stream_work(),
                budget_panel: state.host.latest_budget_decision_panel(),
                lod_counters: state.host.lod_coverage_counters(),
                coverage_probe: waypoint.coverage_probe,
                matches_pixels: waypoint.matches_pixels,
            });
            Ok(())
        },
    )?;

    let fixture_count = state.script.waypoints.len();
    if state.observations.len() != fixture_count || frame_pixels.len() != fixture_count {
        bail!(
            "far LOD settle probe expected {fixture_count} observations/captures, got {}/{}",
            state.observations.len(),
            frame_pixels.len()
        );
    }

    let capture_paths = state
        .observations
        .iter()
        .map(|observation| options.directory.join(format!("{}.png", observation.name)))
        .collect::<Vec<_>>();
    for (path, pixels) in capture_paths.iter().zip(&frame_pixels) {
        save_rgba_png(path, loop_report.width, loop_report.height, pixels)?;
    }

    let observation_indexes = state
        .observations
        .iter()
        .enumerate()
        .map(|(index, observation)| (observation.name.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let sky_rgba = sky_clear_rgba(&options.scene, options.render_options);
    let pixel_analyses = state
        .observations
        .iter()
        .enumerate()
        .map(|(index, observation)| {
            fixture_pixel_analysis(
                observation,
                &frame_pixels[index],
                &frame_pixels,
                &observation_indexes,
                [loop_report.width, loop_report.height],
                sky_rgba,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let fixtures = state
        .observations
        .iter()
        .zip(&capture_paths)
        .zip(&pixel_analyses)
        .map(|((observation, path), analysis)| fixture_json(observation, path, analysis))
        .collect::<Vec<_>>();
    let expectations_met = fixtures
        .iter()
        .all(|fixture| fixture["matchedExpectation"].as_bool() == Some(true));
    let report_path = options.directory.join("lod-settle-report.json");
    let value = serde_json::json!({
        "schema": 1,
        "benchmark": "native_lod_settle_probe",
        "recordedUnixSeconds": crate::current_unix_seconds(),
        "gitCommit": crate::git_short_commit(),
        "gitDirty": crate::git_dirty(),
        "debugAssertions": cfg!(debug_assertions),
        "status": if expectations_met { "matched-expectations" } else { "unexpected" },
        "reportPath": report_path.display().to_string(),
        "config": {
            "seed": options.scene.seed,
            "centerChunk": { "x": options.scene.chunk_x, "z": options.scene.chunk_z },
            "renderDistance": options.scene.render_distance,
            "farLodRange": options.scene.far_lod.extra_radius_chunks,
            "sectionOcclusion": options.render_options.section_occlusion_culling,
            "width": loop_report.width,
            "height": loop_report.height,
        },
        "script": {
            "schema": state.script.schema,
            "name": state.script.name,
            "path": options.script.as_ref().map(|path| path.display().to_string()),
            "waypoints": state.script.waypoints.len(),
        },
        "fixtures": fixtures,
    });
    std::fs::write(&report_path, serde_json::to_string_pretty(&value)?).with_context(|| {
        format!(
            "write far LOD settle probe report {}",
            report_path.display()
        )
    })?;
    Ok(LodSettleProbeReport {
        value,
        expectations_met,
    })
}

fn drive_until_streamed_with_diagnostics(
    host: &mut OffscreenFlatClientHost,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixture: &str,
) -> Result<OffscreenWarmupReport> {
    match host.drive_until_streamed(device, queue) {
        Ok(report) => match host.drive_until_streamed_at_output_size(device, queue) {
            Ok(full_size) => Ok(merge_warmups(report, full_size)),
            Err(error) => settle_timeout_error(host, fixture, error),
        },
        Err(error) => settle_timeout_error(host, fixture, error),
    }
}

fn settle_timeout_error(
    host: &OffscreenFlatClientHost,
    fixture: &str,
    error: anyhow::Error,
) -> Result<OffscreenWarmupReport> {
    let snapshot = host.far_lod_settle_snapshot().ok();
    let diagnostics = serde_json::json!({
        "fixture": fixture,
        "pendingStreamWork": host.pending_stream_work(),
        "budgetPanel": serde_json::to_value(host.latest_budget_decision_panel())
            .unwrap_or(serde_json::Value::Null),
        "ledger": snapshot.as_ref().map(ledger_json),
    });
    bail!(
        "{fixture} settle timeout: {error:#}; diagnostics={}",
        serde_json::to_string(&diagnostics)?
    )
}

fn merge_warmups(
    mut first: OffscreenWarmupReport,
    second: OffscreenWarmupReport,
) -> OffscreenWarmupReport {
    first.frame_count += second.frame_count;
    first.elapsed_ms += second.elapsed_ms;
    first.poll_ms += second.poll_ms;
    first.sync_ms += second.sync_ms;
    first.upload_ms += second.upload_ms;
    first.visibility_graph.build_count += second.visibility_graph.build_count;
    first.visibility_graph.total_ms += second.visibility_graph.total_ms;
    first.visibility_graph.worst_ms = first
        .visibility_graph
        .worst_ms
        .max(second.visibility_graph.worst_ms);
    first.last_summary = second.last_summary.or(first.last_summary);
    first
}

#[derive(Debug)]
struct FixturePixelAnalysis {
    coverage: Option<CoverageProbeResult>,
    determinism: Option<PixelDeterminismResult>,
}

#[derive(Debug)]
struct CoverageProbeResult {
    plane_y: f32,
    projected_columns: usize,
    masked_pixels: usize,
    sky_pixel_count: usize,
    sky_columns: BTreeSet<ChunkPos>,
    sky_rgba: [u8; 4],
}

#[derive(Debug)]
struct PixelDeterminismResult {
    reference: String,
    mismatch_count: usize,
}

fn fixture_pixel_analysis(
    observation: &FixtureObservation,
    pixels: &[u8],
    all_pixels: &[Vec<u8>],
    observation_indexes: &BTreeMap<&str, usize>,
    size: [u32; 2],
    sky_rgba: [u8; 4],
) -> Result<FixturePixelAnalysis> {
    let coverage = observation.coverage_probe.map(|probe| {
        coverage_probe_result(
            &observation.snapshot,
            observation.render_view,
            pixels,
            size,
            probe,
            sky_rgba,
        )
    });
    let determinism = observation
        .matches_pixels
        .as_deref()
        .map(|reference| -> Result<PixelDeterminismResult> {
            let reference_index =
                observation_indexes
                    .get(reference)
                    .copied()
                    .with_context(|| {
                        format!(
                            "waypoint `{}` references missing capture `{reference}`",
                            observation.name
                        )
                    })?;
            Ok(PixelDeterminismResult {
                reference: reference.to_owned(),
                mismatch_count: count_pixel_mismatches(&all_pixels[reference_index], pixels),
            })
        })
        .transpose()?;
    Ok(FixturePixelAnalysis {
        coverage,
        determinism,
    })
}

fn coverage_probe_result(
    snapshot: &FarLodSettleSnapshot,
    render_view: ChunkRenderView,
    pixels: &[u8],
    size: [u32; 2],
    probe: LodCoverageProbe,
    sky_rgba: [u8; 4],
) -> CoverageProbeResult {
    let coverage_columns = snapshot
        .chunks
        .iter()
        .filter_map(|(pos, row)| (row.loaded || row.lod_desired_level.is_some()).then_some(*pos))
        .collect::<BTreeSet<_>>();
    let projected_columns = coverage_columns
        .iter()
        .filter(|pos| {
            project_world_to_pixel(
                render_view,
                Vec4::new(
                    chunk_middle_block_coord(pos.x) as f32 + 0.5,
                    probe.plane_y,
                    chunk_middle_block_coord(pos.z) as f32 + 0.5,
                    1.0,
                ),
                size,
            )
            .is_some()
        })
        .count();
    let mut masked_pixels = 0usize;
    let mut sky_pixel_count = 0usize;
    let mut sky_columns = BTreeSet::new();
    let inverse_view_projection = render_view.view_projection.inverse();
    for y in 0..size[1] {
        for x in 0..size[0] {
            let Some(world) =
                pixel_world_on_plane(inverse_view_projection, [x, y], size, probe.plane_y)
            else {
                continue;
            };
            let pos = ChunkPos::new(
                (world.x.floor() as i32).div_euclid(16),
                (world.z.floor() as i32).div_euclid(16),
            );
            if !coverage_columns.contains(&pos) {
                continue;
            }
            masked_pixels += 1;
            let index = ((y * size[0] + x) * 4) as usize;
            if pixels.get(index..index + 4) == Some(sky_rgba.as_slice()) {
                sky_pixel_count += 1;
                sky_columns.insert(pos);
            }
        }
    }
    CoverageProbeResult {
        plane_y: probe.plane_y,
        projected_columns,
        masked_pixels,
        sky_pixel_count,
        sky_columns,
        sky_rgba,
    }
}

fn pixel_world_on_plane(
    inverse_view_projection: Mat4,
    pixel: [u32; 2],
    size: [u32; 2],
    plane_y: f32,
) -> Option<Vec3> {
    let ndc_x = (pixel[0] as f32 + 0.5) / size[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel[1] as f32 + 0.5) / size[1] as f32 * 2.0;
    if !inverse_view_projection.is_finite() {
        return None;
    }
    let unproject = |depth| {
        let world = inverse_view_projection * Vec4::new(ndc_x, ndc_y, depth, 1.0);
        (world.w.abs() > f32::EPSILON).then_some(world.truncate() / world.w)
    };
    let near = unproject(1.0)?;
    let far = unproject(0.0)?;
    let direction = far - near;
    if direction.y.abs() <= f32::EPSILON {
        return None;
    }
    let distance = (plane_y - near.y) / direction.y;
    (distance >= 0.0).then_some(near + direction * distance)
}

fn project_world_to_pixel(
    render_view: ChunkRenderView,
    world: Vec4,
    size: [u32; 2],
) -> Option<[u32; 2]> {
    let clip = render_view.view_projection * world;
    if !clip.is_finite() || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(-1.0..=1.0).contains(&ndc.x)
        || !(-1.0..=1.0).contains(&ndc.y)
        || !(0.0..=1.0).contains(&ndc.z)
    {
        return None;
    }
    let x = ((ndc.x * 0.5 + 0.5) * size[0] as f32).floor() as u32;
    let y = ((1.0 - (ndc.y * 0.5 + 0.5)) * size[1] as f32).floor() as u32;
    (x < size[0] && y < size[1]).then_some([x, y])
}

fn sky_clear_rgba(
    scene: &crate::cli::SceneOptions,
    render_options: mclone_render::chunk::TexturedSectionRenderOptions,
) -> [u8; 4] {
    let time_of_day = mclone_core::time::time_of_day(scene.day_time_override.unwrap_or(6000));
    let clear = mclone_render::sky::overworld_clear_color(time_of_day);
    let transform = RenderConfig::for_color_target(render_options.color_profile, HEADLESS_FORMAT)
        .target_color_transform();
    let clear = color_transform_wgpu(clear, transform);
    [
        unorm8(clear.r),
        unorm8(clear.g),
        unorm8(clear.b),
        unorm8(clear.a),
    ]
}

fn unorm8(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn count_pixel_mismatches(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len()).div_ceil(4)
}

fn fixture_json(
    observation: &FixtureObservation,
    capture_path: &Path,
    pixel_analysis: &FixturePixelAnalysis,
) -> serde_json::Value {
    let coherence_failures = settle_coherence_failures(observation);
    let culled_but_suppressed = observation.snapshot.culled_but_suppressed_chunks();
    let observed_d1_d2 = !culled_but_suppressed.is_empty();
    let observes_d1_d2 = observation.expected == LodSettleExpectation::FailD1D2;
    let coverage_sampled = pixel_analysis
        .coverage
        .as_ref()
        .is_none_or(|coverage| coverage.projected_columns > 0 && coverage.masked_pixels > 0);
    let observed_sky = pixel_analysis
        .coverage
        .as_ref()
        .is_some_and(|coverage| !coverage.sky_columns.is_empty());
    let coverage_matched = pixel_analysis.coverage.as_ref().is_none_or(|_| {
        coverage_sampled
            && match observation.expected {
                LodSettleExpectation::Pass => !observed_sky,
                LodSettleExpectation::FailD1D2 => observed_sky,
            }
    });
    let determinism_matched = pixel_analysis
        .determinism
        .as_ref()
        .is_none_or(|determinism| determinism.mismatch_count == 0);
    let observed = if !coherence_failures.is_empty() {
        "fail:set-coherence"
    } else if !determinism_matched {
        "fail:pixel-determinism"
    } else if observation.expected == LodSettleExpectation::Pass && observed_sky {
        "fail:c2-sky"
    } else if observes_d1_d2 && observed_d1_d2 {
        "fail:d1-d2"
    } else {
        "pass"
    };
    let matched_expectation = coherence_failures.is_empty()
        && coverage_matched
        && determinism_matched
        && match observation.expected {
            // Slice 1B defined the spawn fixture as the set-coherence canary;
            // pixel/coverage completeness becomes a pass condition in 1C2.
            LodSettleExpectation::Pass => true,
            LodSettleExpectation::FailD1D2 => observed_d1_d2,
        };
    let mut assertion_failures = coherence_failures.clone();
    if !coverage_sampled {
        assertion_failures.push("C2: coverage probe projected no columns".to_owned());
    }
    if let Some(coverage) = pixel_analysis
        .coverage
        .as_ref()
        .filter(|coverage| !coverage.sky_columns.is_empty())
    {
        assertion_failures.push(format!(
            "C2: {} pixels inside the projected coverage footprint hit the sky clear color across {} columns",
            coverage.sky_pixel_count,
            coverage.sky_columns.len(),
        ));
    }
    if let Some(determinism) = pixel_analysis
        .determinism
        .as_ref()
        .filter(|determinism| determinism.mismatch_count != 0)
    {
        assertion_failures.push(format!(
            "C2 revisit: {} pixels differ from `{}`",
            determinism.mismatch_count, determinism.reference
        ));
    }
    if observes_d1_d2 && observed_d1_d2 {
        assertion_failures.push(format!(
            "D1/D2: {} traversal-ready suppressed columns were not painted",
            culled_but_suppressed.len()
        ));
    }
    serde_json::json!({
        "name": observation.name,
        "expected": observation.expected.report_name(),
        "observed": observed,
        "matchedExpectation": matched_expectation,
        "capturePath": capture_path.display().to_string(),
        "settle": {
            "frames": observation.warmup.frame_count,
            "elapsedMs": observation.warmup.elapsed_ms,
            "pollMs": observation.warmup.poll_ms,
            "syncMs": observation.warmup.sync_ms,
            "uploadMs": observation.warmup.upload_ms,
            "pendingStreamWork": observation.pending_stream_work,
        },
        "render": {
            "sections": observation.summary.section_count,
            "drawnSections": observation.summary.drawn_section_count,
            "frustumSections": observation.summary.frustum_section_count,
            "graphCullEnabled": observation.summary.graph_cull_enabled,
            "graphCulledSections": observation.summary.graph_culled_section_count,
            "farLodRegionDraws": observation.summary.far_lod_region_draw_count,
        },
        "coverageProbe": pixel_analysis.coverage.as_ref().map(|coverage| serde_json::json!({
            "planeY": coverage.plane_y,
            "projectedColumns": coverage.projected_columns,
            "maskedPixels": coverage.masked_pixels,
            "skyClearRgba": coverage.sky_rgba,
            "skyPixelCount": coverage.sky_pixel_count,
            "skyColumns": chunk_positions_json(&coverage.sky_columns),
        })),
        "pixelDeterminism": pixel_analysis.determinism.as_ref().map(|determinism| serde_json::json!({
            "reference": determinism.reference,
            "mismatchCount": determinism.mismatch_count,
        })),
        "sets": {
            "desiredTiles": observation.snapshot.runtime.producer.desired_tiles.len(),
            "residentTiles": observation.snapshot.runtime.producer.resident_tiles.len(),
            "uploadedTiles": observation.snapshot.runtime.producer.uploaded_tiles.len(),
            "visibleTiles": observation.snapshot.runtime.producer.visible_tiles.len(),
            "loadedChunks": observation.snapshot.runtime.loaded_chunks.len(),
            "suppressedChunks": observation.snapshot.runtime.suppressed_chunks.len(),
            "paintableFrustumChunks": observation.snapshot.paintable_frustum_chunks.len(),
            "paintedChunks": observation.snapshot.painted_chunks.len(),
            "culledButSuppressedChunks": chunk_positions_json(&culled_but_suppressed),
        },
        "lodReplacementCounters": lod_counters_json(observation.lod_counters),
        "coherenceFailures": coherence_failures,
        "assertionFailures": assertion_failures,
        "budgetPanel": observation.budget_panel,
        "ledger": if observes_d1_d2 || !matched_expectation { ledger_json(&observation.snapshot) } else { serde_json::Value::Null },
    })
}

fn settle_coherence_failures(observation: &FixtureObservation) -> Vec<String> {
    let producer = &observation.snapshot.runtime.producer;
    let desired = producer
        .desired_tiles
        .iter()
        .map(|(pos, level)| LodTileKey::new(*pos, *level))
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    if observation.pending_stream_work != 0 {
        failures.push(format!(
            "pending_stream_work={} after settle",
            observation.pending_stream_work
        ));
    }
    if desired.is_empty() {
        failures.push("desired LOD tile set is empty".to_owned());
    }
    if !producer.pending_builds.is_empty()
        || !producer.inflight_builds.is_empty()
        || !producer.queued_uploads.is_empty()
        || !producer.queued_removals.is_empty()
    {
        failures.push(format!(
            "LOD lifecycle work remains: pending={} inflight={} uploads={} removals={}",
            producer.pending_builds.len(),
            producer.inflight_builds.len(),
            producer.queued_uploads.len(),
            producer.queued_removals.len()
        ));
    }
    for (label, actual) in [
        ("resident", &producer.resident_tiles),
        ("uploaded", &producer.uploaded_tiles),
        ("visible", &producer.visible_tiles),
    ] {
        let missing = desired.difference(actual).count();
        if missing != 0 {
            failures.push(format!("{missing} desired tiles are not {label}"));
        }
    }
    if producer.visible_tiles != desired {
        failures.push(format!(
            "visible tile set differs from desired: visible={} desired={}",
            producer.visible_tiles.len(),
            desired.len()
        ));
    }
    if observation.summary.drawn_section_count == 0
        && observation.summary.far_lod_region_draw_count == 0
    {
        failures.push("capture drew neither real terrain nor far LOD".to_owned());
    }
    failures
}

fn ledger_json(snapshot: &FarLodSettleSnapshot) -> serde_json::Value {
    snapshot
        .chunks
        .iter()
        .map(|(pos, row)| ledger_row_json(*pos, row))
        .collect::<Vec<_>>()
        .into()
}

fn ledger_row_json(pos: ChunkPos, row: &FarLodChunkLedgerRow) -> serde_json::Value {
    serde_json::json!({
        "chunk": { "x": pos.x, "z": pos.z },
        "loaded": row.loaded,
        "traversalReady": row.traversal_ready,
        "paintableInFrustum": row.paintable_in_frustum,
        "painted": row.painted,
        "lodDesiredLevel": row.lod_desired_level,
        "lodResidentLevels": row.lod_resident_levels,
        "lodUploadedLevels": row.lod_uploaded_levels,
        "lodPublishedLevel": row.lod_published_level,
        "lodVisibleLevels": row.lod_visible_levels,
        "suppressed": row.suppressed,
        "lodPendingLevels": row.lod_pending_levels,
        "lodInflightLevels": row.lod_inflight_levels,
        "lodQueuedUploadLevels": row.lod_queued_upload_levels,
        "lodQueuedRemovalLevels": row.lod_queued_removal_levels,
    })
}

fn chunk_positions_json(positions: &BTreeSet<ChunkPos>) -> serde_json::Value {
    positions
        .iter()
        .map(|pos| serde_json::json!({ "x": pos.x, "z": pos.z }))
        .collect::<Vec<_>>()
        .into()
}

fn lod_counters_json(counters: LodReplacementCounters) -> serde_json::Value {
    serde_json::json!({
        "normalOverLod": counters.normal_over_lod,
        "reducedRealOverSynthetic": counters.reduced_real_over_synthetic,
        "lodRevealedOnUnload": counters.lod_revealed_on_unload,
        "lodFirstDrawn": counters.lod_first_drawn,
        "lodEvicted": counters.lod_evicted,
        "suppressedWithoutReplacement": counters.suppressed_without_replacement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::far_lod::FarTerrainLodSettleSnapshot;
    use mclone_app_runtime::scene_session_runtime::FarLodRuntimeSettleSnapshot;

    #[test]
    fn coherence_requires_every_desired_tile_to_be_visible_and_idle() {
        let pos = ChunkPos::new(1, 2);
        let tile = LodTileKey::new(pos, 1);
        let mut producer = FarTerrainLodSettleSnapshot::default();
        producer.desired_tiles.insert(pos, 1);
        producer.resident_tiles.insert(tile);
        producer.uploaded_tiles.insert(tile);
        producer.visible_tiles.insert(tile);
        let snapshot = FarLodSettleSnapshot {
            runtime: FarLodRuntimeSettleSnapshot {
                producer,
                ..FarLodRuntimeSettleSnapshot::default()
            },
            ..FarLodSettleSnapshot::default()
        };
        let observation = FixtureObservation {
            name: SPAWN_FIXTURE.to_owned(),
            expected: LodSettleExpectation::Pass,
            warmup: OffscreenWarmupReport::default(),
            summary: mclone_app_runtime::frame_render::FullFrameRenderSummary {
                far_lod_region_draw_count: 1,
                ..Default::default()
            },
            render_view: mclone_render::chunk::ChunkCamera::overview_for_chunk(0, 0)
                .render_view(960, 960),
            snapshot,
            pending_stream_work: 0,
            budget_panel: mclone_diagnostics::BudgetDecisionPanelReport::default(),
            lod_counters: LodReplacementCounters::default(),
            coverage_probe: None,
            matches_pixels: None,
        };

        assert!(settle_coherence_failures(&observation).is_empty());
    }

    #[test]
    fn d1_d2_classifier_excludes_off_frustum_suppression() {
        let pos = ChunkPos::new(0, 0);
        let mut snapshot = FarLodSettleSnapshot::default();
        snapshot.chunks.insert(
            pos,
            FarLodChunkLedgerRow {
                loaded: true,
                traversal_ready: true,
                suppressed: true,
                ..FarLodChunkLedgerRow::default()
            },
        );

        assert!(snapshot.culled_but_suppressed_chunks().is_empty());
        snapshot
            .chunks
            .get_mut(&pos)
            .expect("row inserted above")
            .paintable_in_frustum = true;

        assert_eq!(
            snapshot.culled_but_suppressed_chunks(),
            BTreeSet::from([pos])
        );
    }

    #[test]
    fn checked_in_smoke_script_is_valid() {
        let script: LodSettleScript = serde_json::from_str(include_str!(
            "../../../../test/fixtures/far-lod/settle-smoke.json"
        ))
        .unwrap();

        script.validate().unwrap();
        assert_eq!(script.schema, SCRIPT_SCHEMA);
        assert_eq!(script.waypoints.len(), 4);
        assert_eq!(script.waypoints[1].expected, LodSettleExpectation::FailD1D2);
        assert_eq!(script.waypoints[1].coverage_probe.unwrap().plane_y, 64.0);
        assert_eq!(
            script.waypoints[3].matches_pixels.as_deref(),
            Some(FLY_UP_FIXTURE)
        );
    }

    #[test]
    fn script_validation_rejects_duplicate_and_unsafe_waypoint_names() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        script.waypoints[1].name = script.waypoints[0].name.clone();
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );

        script.waypoints[1].name = "unsafe/name".to_owned();
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("lowercase ASCII")
        );
    }

    #[test]
    fn script_validation_rejects_invalid_camera_vectors() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        script.waypoints[1].camera = LodSettleCamera::LookAt {
            eye: [0.0, f32::NAN, 0.0],
            target: [0.0, 0.0, 0.0],
        };
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("must be finite")
        );

        script.waypoints[1].camera = LodSettleCamera::LookAt {
            eye: [1.0, 2.0, 3.0],
            target: [1.0, 2.0, 3.0],
        };
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("must differ")
        );
    }

    #[test]
    fn schema_one_remains_valid_but_rejects_schema_two_assertions() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        script.schema = 1;
        script.validate().unwrap();

        script.waypoints[1].coverage_probe = Some(LodCoverageProbe { plane_y: 64.0 });
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("requires script schema 2")
        );
    }

    #[test]
    fn pixel_match_requires_an_earlier_identical_camera() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        let mut revisit = script.waypoints[1].clone();
        revisit.name = "high-revisit".to_owned();
        revisit.matches_pixels = Some(FLY_UP_FIXTURE.to_owned());
        script.waypoints.push(revisit);
        script.validate().unwrap();

        script.waypoints[2].matches_pixels = Some("missing".to_owned());
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("must name an earlier waypoint")
        );

        script.waypoints[2].matches_pixels = Some(SPAWN_FIXTURE.to_owned());
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("camera differs")
        );
    }

    #[test]
    fn coverage_probe_maps_sky_pixels_back_into_coverage() {
        let size = [32, 32];
        let camera = mclone_render::chunk::ChunkCamera {
            eye: [8.25, 200.0, 8.25],
            target: [8.0, 64.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 58.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 600.0,
        };
        let render_view = camera.render_view(size[0], size[1]);
        let mut snapshot = FarLodSettleSnapshot::default();
        snapshot.chunks.insert(
            ChunkPos::new(0, 0),
            FarLodChunkLedgerRow {
                loaded: true,
                ..FarLodChunkLedgerRow::default()
            },
        );
        let sky_rgba = [120, 167, 255, 255];
        let pixels = sky_rgba.repeat((size[0] * size[1]) as usize);

        let result = coverage_probe_result(
            &snapshot,
            render_view,
            &pixels,
            size,
            LodCoverageProbe { plane_y: 64.0 },
            sky_rgba,
        );

        assert_eq!(result.projected_columns, 1);
        assert!(result.masked_pixels > 0);
        assert_eq!(result.sky_pixel_count, result.masked_pixels);
        assert_eq!(result.sky_columns, BTreeSet::from([ChunkPos::new(0, 0)]));
    }

    #[test]
    fn pixel_mismatch_count_is_rgba_pixel_granular() {
        assert_eq!(
            count_pixel_mismatches(&[1, 2, 3, 4, 5, 6, 7, 8], &[1, 2, 3, 4, 5, 6, 0, 8]),
            1
        );
        assert_eq!(count_pixel_mismatches(&[1, 2, 3, 4], &[]), 1);
    }
}
