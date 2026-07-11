use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use glam::{Vec3, Vec4};
use mclone_app_runtime::far_lod::{
    FarTerrainLodConfig, MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
    MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS, far_lod_sample_spacing_for_level,
};
use mclone_app_runtime::lod_coverage::LodReplacementCounters;
use mclone_core::{ChunkPos, LodTileKey, chunk_middle_block_coord, chunk_min_block_coord};
use mclone_render::chunk::ChunkRenderView;
use mclone_render::headless::{
    HeadlessFrameLoopOptions, read_depth32, run_headless_aux_loop,
    run_headless_capture_loop_with_aux, save_rgba_png,
};
use mclone_scene::{FarLodChunkLedgerRow, FarLodSettleSnapshot};
use mclone_worldgen::block::is_air_like;
use mclone_worldgen::levelgen::{GeneratedChunk, generate_overworld_surface_chunk};
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
const SCRIPT_SCHEMA: u32 = 4;
const MAX_WAYPOINTS: usize = 64;
const MAX_CAPTURE_SETTLE_PASSES: usize = 4;
const MAX_CLEAR_DEPTH_SAMPLE_DETAILS: usize = 64;
const SMOOTH_MOVEMENT_TARGET_HZ: f64 = 60.0;
const SMOOTH_MOVEMENT_SPEED_BLOCKS_PER_SECOND: f32 = 48.0;
const SMOOTH_MOVEMENT_TAIL_FRAMES: usize = 120;
const SMOOTH_MOVEMENT_STABLE_FRAMES: usize = 6;
const SMOOTH_MOVEMENT_MAX_MISSING_FRAMES: usize = 0;
const FLY_UP_EYE_OFFSET_X: f32 = 0.25;
const FLY_UP_EYE_Y: f32 = 500.0;
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

#[derive(Clone, Copy)]
struct SmoothPose {
    eye: Vec3,
    target: Vec3,
}

struct SmoothMovementPlan {
    start: SmoothPose,
    poses: Vec<SmoothPose>,
    moving_frames: usize,
}

struct SmoothProbeHost {
    host: OffscreenFlatClientHost,
    plan: SmoothMovementPlan,
    current: Option<SmoothFrameObservation>,
    surface_sampler: SurfaceSampler,
}

struct SmoothFrameObservation {
    moving: bool,
    center: ChunkPos,
    snapshot: FarLodSettleSnapshot,
    render_view: ChunkRenderView,
    pending_stream_work: usize,
    far_lod_stats: mclone_app_runtime::far_lod::FarTerrainLodProducerStats,
}

struct SmoothFrameEvidence {
    index: usize,
    moving: bool,
    center: ChunkPos,
    pending_stream_work: usize,
    far_lod_stats: mclone_app_runtime::far_lod::FarTerrainLodProducerStats,
    coverage: CoverageProbeResult,
    clear_ledger: Vec<serde_json::Value>,
}

struct SmoothMovementResult {
    value: serde_json::Value,
    expectations_met: bool,
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
    lod_assertions: Vec<LodTileAssertion>,
    mutation: Option<LodSettleMutation>,
    expected_far_lod_state: Option<FarLodStateAssertion>,
    actual_far_lod_config: FarTerrainLodConfig,
    render_distance: u32,
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
    #[serde(default)]
    lod_assertions: Vec<LodTileAssertion>,
    #[serde(default)]
    mutation: Option<LodSettleMutation>,
    #[serde(default)]
    expected_far_lod_state: Option<FarLodStateAssertion>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum LodSettleMutation {
    ToggleFarLod,
    SetFarLodRange {
        #[serde(rename = "extraRadiusChunks")]
        extra_radius_chunks: u32,
    },
}

impl LodSettleMutation {
    const fn report_name(self) -> &'static str {
        match self {
            Self::ToggleFarLod => "toggle-far-lod",
            Self::SetFarLodRange { .. } => "set-far-lod-range",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FarLodStateAssertion {
    enabled: bool,
    extra_radius_chunks: u32,
    desired_tiles: usize,
    suppressed_chunks: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LodTileAssertion {
    chunk: [i32; 2],
    expected: LodDesiredLevelExpectation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum LodDesiredLevelExpectation {
    NotDesired,
    #[serde(rename = "level-1")]
    Level1,
    #[serde(rename = "level-2")]
    Level2,
    #[serde(rename = "level-3")]
    Level3,
}

impl LodDesiredLevelExpectation {
    const fn level(self) -> Option<u8> {
        match self {
            Self::NotDesired => None,
            Self::Level1 => Some(1),
            Self::Level2 => Some(2),
            Self::Level3 => Some(3),
        }
    }

    const fn report_name(self) -> &'static str {
        match self {
            Self::NotDesired => "not-desired",
            Self::Level1 => "level-1",
            Self::Level2 => "level-2",
            Self::Level3 => "level-3",
        }
    }
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
                    lod_assertions: Vec::new(),
                    mutation: None,
                    expected_far_lod_state: None,
                },
                LodSettleWaypoint {
                    name: FLY_UP_FIXTURE.to_owned(),
                    camera: LodSettleCamera::LookAt {
                        eye: [
                            center_x + FLY_UP_EYE_OFFSET_X,
                            FLY_UP_EYE_Y,
                            center_z + FLY_UP_EYE_OFFSET_Z,
                        ],
                        target: [
                            center_x + FLY_UP_EYE_OFFSET_X,
                            FLY_UP_TARGET_Y,
                            center_z + FLY_UP_EYE_OFFSET_Z,
                        ],
                    },
                    expected: LodSettleExpectation::Pass,
                    coverage_probe: Some(LodCoverageProbe { plane_y: 64.0 }),
                    matches_pixels: None,
                    lod_assertions: Vec::new(),
                    mutation: None,
                    expected_far_lod_state: None,
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
            if self.schema < 3 && !waypoint.lod_assertions.is_empty() {
                bail!(
                    "waypoint `{}` lodAssertions requires script schema 3",
                    waypoint.name
                );
            }
            if self.schema < 4
                && (waypoint.mutation.is_some() || waypoint.expected_far_lod_state.is_some())
            {
                bail!(
                    "waypoint `{}` mutation/expectedFarLodState requires script schema 4",
                    waypoint.name
                );
            }
            if waypoint.mutation.is_some() && waypoint.expected_far_lod_state.is_none() {
                bail!(
                    "waypoint `{}` mutation requires expectedFarLodState",
                    waypoint.name
                );
            }
            if let Some(LodSettleMutation::SetFarLodRange {
                extra_radius_chunks,
            }) = waypoint.mutation
                && !(MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
                    ..=MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS)
                    .contains(&extra_radius_chunks)
            {
                bail!(
                    "waypoint `{}` far LOD range must be {}..={}, got {extra_radius_chunks}",
                    waypoint.name,
                    MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                    MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
                );
            }
            if let Some(expected) = waypoint.expected_far_lod_state {
                if !(MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
                    ..=MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS)
                    .contains(&expected.extra_radius_chunks)
                {
                    bail!(
                        "waypoint `{}` expected far LOD range must be {}..={}",
                        waypoint.name,
                        MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                        MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
                    );
                }
                if !expected.enabled
                    && (expected.desired_tiles != 0 || expected.suppressed_chunks != 0)
                {
                    bail!(
                        "waypoint `{}` disabled far LOD state must expect zero desired/suppressed chunks",
                        waypoint.name
                    );
                }
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
            let mut asserted_chunks = BTreeSet::new();
            for assertion in &waypoint.lod_assertions {
                let pos = ChunkPos::new(assertion.chunk[0], assertion.chunk[1]);
                if !asserted_chunks.insert(pos) {
                    bail!(
                        "waypoint `{}` has duplicate LOD assertion for chunk {},{}",
                        waypoint.name,
                        pos.x,
                        pos.z
                    );
                }
            }
            prior_waypoints.insert(waypoint.name.as_str(), waypoint);
        }
        Ok(())
    }
}

impl SmoothMovementPlan {
    fn from_script(script: &LodSettleScript) -> Option<Self> {
        let endpoints = script
            .waypoints
            .iter()
            .map(|waypoint| match waypoint.camera {
                LodSettleCamera::LookAt { eye, target } if waypoint.coverage_probe.is_some() => {
                    Some(SmoothPose {
                        eye: Vec3::from_array(eye),
                        target: Vec3::from_array(target),
                    })
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let start = *endpoints.first()?;
        if endpoints.len() < 2 {
            return None;
        }
        let blocks_per_frame =
            SMOOTH_MOVEMENT_SPEED_BLOCKS_PER_SECOND / SMOOTH_MOVEMENT_TARGET_HZ as f32;
        let mut poses = Vec::new();
        for segment in endpoints.windows(2) {
            let from = segment[0];
            let to = segment[1];
            let distance = from.eye.distance(to.eye);
            let frame_count = (distance / blocks_per_frame).ceil().max(1.0) as usize;
            for frame in 1..=frame_count {
                let amount = frame as f32 / frame_count as f32;
                poses.push(SmoothPose {
                    eye: from.eye.lerp(to.eye, amount),
                    target: from.target.lerp(to.target, amount),
                });
            }
        }
        if poses.is_empty() {
            return None;
        }
        let moving_frames = poses.len();
        let last = *poses.last()?;
        poses.extend(std::iter::repeat_n(last, SMOOTH_MOVEMENT_TAIL_FRAMES));
        Some(Self {
            start,
            poses,
            moving_frames,
        })
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
    let smooth_plan = SmoothMovementPlan::from_script(&script);
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
    let (loop_report, frame_pixels, frame_depths, state) = run_headless_capture_loop_with_aux(
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
            if let Some(mutation) = waypoint.mutation {
                let action = match mutation {
                    LodSettleMutation::ToggleFarLod => mclone_ui::GameUiAction::ToggleFarLod,
                    LodSettleMutation::SetFarLodRange {
                        extra_radius_chunks,
                    } => mclone_ui::GameUiAction::SetFarLodRange(
                        i32::try_from(extra_radius_chunks)
                            .expect("validated far LOD range fits i32"),
                    ),
                };
                state
                    .host
                    .apply_ui_action(action, frame.device, frame.queue)?;
            }
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
                lod_assertions: waypoint.lod_assertions,
                mutation: waypoint.mutation,
                expected_far_lod_state: waypoint.expected_far_lod_state,
                actual_far_lod_config: state.host.far_lod_config(),
                render_distance: state.host.render_distance(),
            });
            Ok(())
        },
        |_, device, queue, state| read_depth32(device, queue, state.host.depth_target()),
    )?;

    let fixture_count = state.script.waypoints.len();
    if state.observations.len() != fixture_count
        || frame_pixels.len() != fixture_count
        || frame_depths.len() != fixture_count
    {
        bail!(
            "far LOD settle probe expected {fixture_count} observations/color/depth captures, got {}/{}/{}",
            state.observations.len(),
            frame_pixels.len(),
            frame_depths.len()
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
    let mut surface_sampler = SurfaceSampler::default();
    let pixel_analyses = state
        .observations
        .iter()
        .enumerate()
        .map(|(index, observation)| {
            fixture_pixel_analysis(
                observation,
                &frame_pixels[index],
                &frame_depths[index],
                &frame_pixels,
                &observation_indexes,
                [loop_report.width, loop_report.height],
                options.scene.seed,
                &mut surface_sampler,
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
    let smooth_movement = smooth_plan
        .map(|plan| run_smooth_movement_probe(options, plan))
        .transpose()?;
    let expectations_met = fixtures
        .iter()
        .all(|fixture| fixture["matchedExpectation"].as_bool() == Some(true))
        && smooth_movement
            .as_ref()
            .is_none_or(|movement| movement.expectations_met);
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
            "farLodDetail": format!("{:?}", options.scene.far_lod.detail_mode),
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
        "smoothMovement": smooth_movement.map(|movement| movement.value),
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

fn run_smooth_movement_probe(
    options: &LodSettleProbeOptions,
    plan: SmoothMovementPlan,
) -> Result<SmoothMovementResult> {
    let frame_count = plan.poses.len();
    let moving_frames = plan.moving_frames;
    let start = plan.start;
    let end = *plan
        .poses
        .last()
        .context("smooth movement plan requires a final pose")?;
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let seed = options.scene.seed;
    let frame_duration = std::time::Duration::from_secs_f64(1.0 / SMOOTH_MOVEMENT_TARGET_HZ);

    let (headless, frames, state) = run_headless_aux_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count,
            pace_frame_duration: Some(frame_duration),
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
            host.start_scene_with_wait_policy_report(device, queue, StartupWaitPolicy::Idle)?;
            host.set_camera_look_at(start.eye, start.target);
            host.commit_camera()?;
            drive_until_streamed_with_diagnostics(&mut host, device, queue, "smooth-start")?;
            let target_frame_ms = 1_000.0 / SMOOTH_MOVEMENT_TARGET_HZ;
            host.set_frame_clock(target_frame_ms, Some(target_frame_ms));
            Ok(SmoothProbeHost {
                host,
                plan,
                current: None,
                surface_sampler: SurfaceSampler::default(),
            })
        },
        |index, frame, state| {
            let pose = state.plan.poses[index];
            state.host.set_camera_look_at(pose.eye, pose.target);
            state.host.commit_camera()?;
            state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions { hud: false })?;
            let render_view = state.host.render_view([options.width, options.height])?;
            state.current = Some(SmoothFrameObservation {
                moving: index < state.plan.moving_frames,
                center: ChunkPos::from_block_coords(
                    pose.eye.x.floor() as i32,
                    pose.eye.z.floor() as i32,
                ),
                snapshot: state.host.far_lod_settle_snapshot()?,
                render_view,
                pending_stream_work: state.host.pending_stream_work(),
                far_lod_stats: state.host.far_lod_stats(),
            });
            Ok(())
        },
        |index, device, queue, state| {
            let depths = read_depth32(device, queue, state.host.depth_target())?;
            let observation = state
                .current
                .take()
                .context("smooth depth capture is missing its frame observation")?;
            let coverage = coverage_probe_result(
                &observation.snapshot,
                observation.render_view,
                &depths,
                [options.width, options.height],
                LodCoverageProbe { plane_y: 64.0 },
                state.host.far_lod_config(),
                seed,
                state.host.render_distance(),
                &mut state.surface_sampler,
            );
            let clear_ledger = coverage
                .clear_depth_columns
                .iter()
                .take(MAX_CLEAR_DEPTH_SAMPLE_DETAILS)
                .map(|pos| {
                    observation.snapshot.chunks.get(pos).map_or_else(
                        || ledger_row_json(*pos, &FarLodChunkLedgerRow::default()),
                        |row| ledger_row_json(*pos, row),
                    )
                })
                .collect();
            Ok(SmoothFrameEvidence {
                index,
                moving: observation.moving,
                center: observation.center,
                pending_stream_work: observation.pending_stream_work,
                far_lod_stats: observation.far_lod_stats,
                coverage,
                clear_ledger,
            })
        },
    )?;

    let mut active_missing = BTreeMap::<ChunkPos, usize>::new();
    let mut max_chunk_missing_frames = 0usize;
    let mut longest_missing_chunk = None;
    let mut current_any_gap_run = 0usize;
    let mut max_any_gap_run_frames = 0usize;
    let mut frames_with_clear_depth = 0usize;
    let mut max_clear_depth_columns = 0usize;
    for frame in &frames {
        let clear = &frame.coverage.clear_depth_columns;
        if clear.is_empty() {
            current_any_gap_run = 0;
        } else {
            frames_with_clear_depth += 1;
            current_any_gap_run += 1;
            max_any_gap_run_frames = max_any_gap_run_frames.max(current_any_gap_run);
            max_clear_depth_columns = max_clear_depth_columns.max(clear.len());
        }
        active_missing.retain(|pos, _| clear.contains(pos));
        for pos in clear {
            let age = active_missing.entry(*pos).or_default();
            *age += 1;
            if *age > max_chunk_missing_frames {
                max_chunk_missing_frames = *age;
                longest_missing_chunk = Some(*pos);
            }
        }
    }

    let first_failure = frames
        .iter()
        .find(|frame| !frame.coverage.clear_depth_columns.is_empty());
    let final_stable = frames
        .iter()
        .rev()
        .take(SMOOTH_MOVEMENT_STABLE_FRAMES)
        .count()
        == SMOOTH_MOVEMENT_STABLE_FRAMES
        && frames
            .iter()
            .rev()
            .take(SMOOTH_MOVEMENT_STABLE_FRAMES)
            .all(|frame| frame.coverage.clear_depth_columns.is_empty());
    let expectations_met = final_stable
        && max_chunk_missing_frames <= SMOOTH_MOVEMENT_MAX_MISSING_FRAMES
        && max_any_gap_run_frames <= SMOOTH_MOVEMENT_MAX_MISSING_FRAMES;
    let value = serde_json::json!({
        "status": if expectations_met { "pass" } else { "fail" },
        "depthIsCoverageEvidence": true,
        "targetHz": SMOOTH_MOVEMENT_TARGET_HZ,
        "speedBlocksPerSecond": SMOOTH_MOVEMENT_SPEED_BLOCKS_PER_SECOND,
        "movingFrames": moving_frames,
        "tailFrames": SMOOTH_MOVEMENT_TAIL_FRAMES,
        "totalFrames": frames.len(),
        "startEye": start.eye.to_array(),
        "endEye": end.eye.to_array(),
        "framesWithClearDepth": frames_with_clear_depth,
        "maxClearDepthColumns": max_clear_depth_columns,
        "maxConsecutiveClearFrames": max_any_gap_run_frames,
        "maxChunkMissingFrames": max_chunk_missing_frames,
        "longestMissingChunk": longest_missing_chunk.map(|pos| serde_json::json!({ "x": pos.x, "z": pos.z })),
        "allowedMissingFrames": SMOOTH_MOVEMENT_MAX_MISSING_FRAMES,
        "finalStableFrames": SMOOTH_MOVEMENT_STABLE_FRAMES,
        "finalStable": final_stable,
        "headless": {
            "width": headless.width,
            "height": headless.height,
            "setupMs": headless.setup_ms,
            "totalFrameMs": headless.total_frame_ms,
            "averageFrameMs": headless.average_frame_ms,
            "maxFrameMs": headless.max_frame_ms,
        },
        "firstFailure": first_failure.map(smooth_failure_json),
        "finalFailure": frames.last().filter(|frame| !frame.coverage.clear_depth_columns.is_empty()).map(smooth_failure_json),
        "frames": frames.iter().map(|frame| serde_json::json!({
            "index": frame.index,
            "moving": frame.moving,
            "center": { "x": frame.center.x, "z": frame.center.z },
            "pendingStreamWork": frame.pending_stream_work,
            "projectedColumns": frame.coverage.projected_columns,
            "clearDepthSamples": frame.coverage.clear_depth_sample_count,
            "clearDepthColumns": frame.coverage.clear_depth_columns.len(),
            "farLod": smooth_far_lod_stats_json(frame.far_lod_stats),
        })).collect::<Vec<_>>(),
        "finalBudgetPanel": state.host.latest_budget_decision_panel(),
    });
    Ok(SmoothMovementResult {
        value,
        expectations_met,
    })
}

fn smooth_failure_json(frame: &SmoothFrameEvidence) -> serde_json::Value {
    serde_json::json!({
        "index": frame.index,
        "moving": frame.moving,
        "center": { "x": frame.center.x, "z": frame.center.z },
        "pendingStreamWork": frame.pending_stream_work,
        "clearDepthSampleCount": frame.coverage.clear_depth_sample_count,
        "clearDepthColumns": chunk_positions_json(&frame.coverage.clear_depth_columns),
        "clearDepthSamples": frame.coverage.clear_depth_samples.iter().map(|sample| serde_json::json!({
            "chunk": { "x": sample.chunk.x, "z": sample.chunk.z },
            "source": sample.source,
            "local": { "x": sample.local_x, "z": sample.local_z },
            "surfaceY": sample.surface_y,
            "pixel": { "x": sample.pixel[0], "y": sample.pixel[1] },
            "clearPixelsIn3x3": sample.clear_pixels_in_3x3,
        })).collect::<Vec<_>>(),
        "ledger": frame.clear_ledger,
        "farLod": smooth_far_lod_stats_json(frame.far_lod_stats),
    })
}

fn smooth_far_lod_stats_json(
    stats: mclone_app_runtime::far_lod::FarTerrainLodProducerStats,
) -> serde_json::Value {
    serde_json::json!({
        "desiredTiles": stats.desired_tiles,
        "prefetchTiles": stats.prefetch_tiles,
        "residentTiles": stats.resident_tiles,
        "visibleTiles": stats.visible_tiles,
        "pendingBuilds": stats.pending_builds,
        "inflightBuilds": stats.inflight_builds,
        "queuedUploads": stats.queued_uploads,
        "submittedBuilds": stats.submitted_builds,
        "completedBuilds": stats.completed_builds,
        "staleBuilds": stats.stale_builds,
        "oldestBuildAgeMs": stats.oldest_build_age_ms,
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
    projected_columns: usize,
    sampled_depth_points: usize,
    clear_depth_sample_count: usize,
    clear_depth_columns: BTreeSet<ChunkPos>,
    clear_depth_samples: Vec<CoverageClearDepthSample>,
}

#[derive(Default)]
struct SurfaceSampler {
    chunks: BTreeMap<ChunkPos, GeneratedChunk>,
}

#[derive(Clone, Copy, Debug)]
struct CoverageClearDepthSample {
    chunk: ChunkPos,
    source: &'static str,
    local_x: f32,
    local_z: f32,
    surface_y: f32,
    pixel: [u32; 2],
    clear_pixels_in_3x3: usize,
}

#[derive(Debug)]
struct PixelDeterminismResult {
    reference: String,
    mismatch_count: usize,
}

fn fixture_pixel_analysis(
    observation: &FixtureObservation,
    pixels: &[u8],
    depths: &[f32],
    all_pixels: &[Vec<u8>],
    observation_indexes: &BTreeMap<&str, usize>,
    size: [u32; 2],
    seed: i64,
    surface_sampler: &mut SurfaceSampler,
) -> Result<FixturePixelAnalysis> {
    let coverage = observation.coverage_probe.map(|probe| {
        coverage_probe_result(
            &observation.snapshot,
            observation.render_view,
            depths,
            size,
            probe,
            observation.actual_far_lod_config,
            seed,
            observation.render_distance,
            surface_sampler,
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
    depths: &[f32],
    size: [u32; 2],
    _probe: LodCoverageProbe,
    config: FarTerrainLodConfig,
    seed: i64,
    render_distance: u32,
    surface_sampler: &mut SurfaceSampler,
) -> CoverageProbeResult {
    let mut projected_columns = 0usize;
    let mut sampled_depth_points = 0usize;
    let mut clear_depth_sample_count = 0usize;
    let mut clear_depth_columns = BTreeSet::new();
    let mut clear_depth_samples = Vec::new();
    let center = ChunkPos::from_block_coords(
        render_view.camera_position.x.floor() as i32,
        render_view.camera_position.z.floor() as i32,
    );
    let outer_radius = render_distance.saturating_add(config.extra_radius_chunks) as i32;
    for dz in -outer_radius..=outer_radius {
        for dx in -outer_radius..=outer_radius {
            let pos = ChunkPos::new(center.x + dx, center.z + dz);
            let row = snapshot.chunks.get(&pos);
            let suppressed = row.is_some_and(|row| row.suppressed);
            let visible_level = row.and_then(|row| row.lod_visible_levels.iter().next().copied());
            let desired_level = row.and_then(|row| row.lod_desired_level);
            let (source, samples) = if suppressed {
                ("real", real_surface_test_points(surface_sampler, seed, pos))
            } else {
                let level = visible_level.or(desired_level).unwrap_or(3);
                let spacing = far_lod_sample_spacing_for_level(config.sample_spacing_blocks, level);
                (
                    if visible_level.is_some() {
                        "far-lod"
                    } else if desired_level.is_some() {
                        "missing-lod"
                    } else {
                        "undesired"
                    },
                    lod_surface_test_points(surface_sampler, seed, pos, spacing),
                )
            };
            let mut column_projected = false;
            for sample in samples {
                let Some([x, y]) = project_world_to_pixel(
                    render_view,
                    Vec4::new(sample.world_x, sample.surface_y, sample.world_z, 1.0),
                    size,
                ) else {
                    continue;
                };
                column_projected = true;
                sampled_depth_points += 1;
                let index = (y * size[0] + x) as usize;
                if depths
                    .get(index)
                    .is_none_or(|depth| *depth <= 0.0 || !depth.is_finite())
                {
                    clear_depth_sample_count += 1;
                    clear_depth_columns.insert(pos);
                    if clear_depth_samples.len() < MAX_CLEAR_DEPTH_SAMPLE_DETAILS {
                        clear_depth_samples.push(CoverageClearDepthSample {
                            chunk: pos,
                            source,
                            local_x: sample.world_x - chunk_min_block_coord(pos.x) as f32,
                            local_z: sample.world_z - chunk_min_block_coord(pos.z) as f32,
                            surface_y: sample.surface_y,
                            pixel: [x, y],
                            clear_pixels_in_3x3: clear_depth_neighborhood_count(
                                depths,
                                size,
                                [x, y],
                            ),
                        });
                    }
                }
            }
            projected_columns += usize::from(column_projected);
        }
    }
    CoverageProbeResult {
        projected_columns,
        sampled_depth_points,
        clear_depth_sample_count,
        clear_depth_columns,
        clear_depth_samples,
    }
}

#[derive(Clone, Copy)]
struct SurfaceTestPoint {
    world_x: f32,
    world_z: f32,
    surface_y: f32,
}

fn real_surface_test_points(
    sampler: &mut SurfaceSampler,
    seed: i64,
    pos: ChunkPos,
) -> Vec<SurfaceTestPoint> {
    const OFFSETS: [i32; 3] = [4, 8, 12];
    OFFSETS
        .into_iter()
        .flat_map(|z| OFFSETS.into_iter().map(move |x| (x, z)))
        .filter_map(|(x, z)| {
            let world_x = chunk_min_block_coord(pos.x) + x;
            let world_z = chunk_min_block_coord(pos.z) + z;
            surface_y_at(sampler, seed, world_x, world_z).map(|surface_y| SurfaceTestPoint {
                world_x: world_x as f32 + 0.5,
                world_z: world_z as f32 + 0.5,
                surface_y,
            })
        })
        .collect()
}

fn lod_surface_test_points(
    sampler: &mut SurfaceSampler,
    seed: i64,
    pos: ChunkPos,
    spacing: u32,
) -> Vec<SurfaceTestPoint> {
    let spacing = spacing.clamp(1, 16) as i32;
    let min_x = chunk_min_block_coord(pos.x);
    let min_z = chunk_min_block_coord(pos.z);
    (0..16)
        .step_by(spacing as usize)
        .flat_map(|z0| (0..16).step_by(spacing as usize).map(move |x0| (x0, z0)))
        .filter_map(|(x0, z0)| {
            let x1 = (x0 + spacing).min(16);
            let z1 = (z0 + spacing).min(16);
            let sample_x = min_x + x0 + (x1 - x0) / 2;
            let sample_z = min_z + z0 + (z1 - z0) / 2;
            surface_y_at(sampler, seed, sample_x, sample_z).map(|surface_y| SurfaceTestPoint {
                // Stay off the quad diagonal and cell boundaries so rasterization
                // tests the owned top face rather than a seam pixel.
                world_x: min_x as f32 + x0 as f32 + (x1 - x0) as f32 * 0.35,
                world_z: min_z as f32 + z0 as f32 + (z1 - z0) as f32 * 0.65,
                surface_y,
            })
        })
        .collect()
}

fn surface_y_at(
    sampler: &mut SurfaceSampler,
    seed: i64,
    world_x: i32,
    world_z: i32,
) -> Option<f32> {
    let pos = ChunkPos::from_block_coords(world_x, world_z);
    let chunk = sampler
        .chunks
        .entry(pos)
        .or_insert_with(|| generate_overworld_surface_chunk(seed, pos.x, pos.z));
    let local_x = world_x - chunk_min_block_coord(pos.x);
    let local_z = world_z - chunk_min_block_coord(pos.z);
    (chunk.min_y..chunk.min_y + chunk.height)
        .rev()
        .find(|y| !is_air_like(chunk.block_at_y(local_x, *y, local_z).raw()))
        .map(|y| (y + 1) as f32)
}

fn clear_depth_neighborhood_count(depths: &[f32], size: [u32; 2], pixel: [u32; 2]) -> usize {
    let min_x = pixel[0].saturating_sub(1);
    let max_x = pixel[0].saturating_add(1).min(size[0].saturating_sub(1));
    let min_y = pixel[1].saturating_sub(1);
    let max_y = pixel[1].saturating_add(1).min(size[1].saturating_sub(1));
    (min_y..=max_y)
        .flat_map(|y| (min_x..=max_x).map(move |x| (y * size[0] + x) as usize))
        .filter(|index| {
            depths
                .get(*index)
                .is_none_or(|depth| *depth <= 0.0 || !depth.is_finite())
        })
        .count()
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

fn count_pixel_mismatches(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len()).div_ceil(4)
}

fn lod_assertion_results(
    observation: &FixtureObservation,
) -> (Vec<serde_json::Value>, Vec<String>) {
    let desired = &observation.snapshot.runtime.producer.desired_tiles;
    let mut results = Vec::with_capacity(observation.lod_assertions.len());
    let mut failures = Vec::new();
    for assertion in &observation.lod_assertions {
        let pos = ChunkPos::new(assertion.chunk[0], assertion.chunk[1]);
        let actual = desired.get(&pos).copied();
        let matched = actual == assertion.expected.level();
        if !matched {
            failures.push(format!(
                "C1: chunk {},{} expected {} but desired level was {}",
                pos.x,
                pos.z,
                assertion.expected.report_name(),
                actual.map_or_else(
                    || "not-desired".to_owned(),
                    |level| format!("level-{level}")
                )
            ));
        }
        results.push(serde_json::json!({
            "chunk": { "x": pos.x, "z": pos.z },
            "expected": assertion.expected.report_name(),
            "actual": actual.map(|level| format!("level-{level}")).unwrap_or_else(|| "not-desired".to_owned()),
            "matched": matched,
        }));
    }
    (results, failures)
}

fn far_lod_state_result(observation: &FixtureObservation) -> (serde_json::Value, Vec<String>) {
    let Some(expected) = observation.expected_far_lod_state else {
        return (serde_json::Value::Null, Vec::new());
    };
    let producer = &observation.snapshot.runtime.producer;
    let actual_desired = producer.desired_tiles.len();
    let actual_suppressed = observation.snapshot.runtime.suppressed_chunks.len();
    let actual = observation.actual_far_lod_config;
    let mut failures = Vec::new();
    if actual.enabled != expected.enabled {
        failures.push(format!(
            "far LOD enabled expected {} but was {}",
            expected.enabled, actual.enabled
        ));
    }
    if actual.extra_radius_chunks != expected.extra_radius_chunks {
        failures.push(format!(
            "far LOD range expected {} but was {}",
            expected.extra_radius_chunks, actual.extra_radius_chunks
        ));
    }
    if actual_desired != expected.desired_tiles {
        failures.push(format!(
            "far LOD desired tiles expected {} but was {actual_desired}",
            expected.desired_tiles
        ));
    }
    if actual_suppressed != expected.suppressed_chunks {
        failures.push(format!(
            "far LOD suppressed chunks expected {} but was {actual_suppressed}",
            expected.suppressed_chunks
        ));
    }
    if !expected.enabled {
        let retained = producer.resident_tiles.len()
            + producer.uploaded_tiles.len()
            + producer.published_tiles_by_chunk.len()
            + producer.visible_tiles.len();
        if retained != 0 {
            failures.push(format!(
                "disabled far LOD retained {retained} resident/uploaded/published/visible entries"
            ));
        }
    }
    let matched = failures.is_empty();
    (
        serde_json::json!({
            "expected": {
                "enabled": expected.enabled,
                "extraRadiusChunks": expected.extra_radius_chunks,
                "desiredTiles": expected.desired_tiles,
                "suppressedChunks": expected.suppressed_chunks,
            },
            "actual": {
                "enabled": actual.enabled,
                "extraRadiusChunks": actual.extra_radius_chunks,
                "desiredTiles": actual_desired,
                "suppressedChunks": actual_suppressed,
                "residentTiles": producer.resident_tiles.len(),
                "uploadedTiles": producer.uploaded_tiles.len(),
                "publishedTiles": producer.published_tiles_by_chunk.len(),
                "visibleTiles": producer.visible_tiles.len(),
                "frameSummaryFarLodRegionDraws": observation.summary.far_lod_region_draw_count,
            },
            "matched": matched,
        }),
        failures,
    )
}

fn fixture_json(
    observation: &FixtureObservation,
    capture_path: &Path,
    pixel_analysis: &FixturePixelAnalysis,
) -> serde_json::Value {
    let coherence_failures = settle_coherence_failures(observation);
    let culled_but_suppressed = observation.snapshot.culled_but_suppressed_chunks();
    // A wholly graph-culled in-frustum column is a coverage defect only in the
    // explicit unoccluded/top-down probe. An angled gameplay view can
    // legitimately occlude a paintable column behind nearer real terrain.
    let observed_d1_d2 = observation.coverage_probe.is_some() && !culled_but_suppressed.is_empty();
    let (lod_assertions, lod_assertion_failures) = lod_assertion_results(observation);
    let lod_assertions_matched = lod_assertion_failures.is_empty();
    let (far_lod_state, far_lod_state_failures) = far_lod_state_result(observation);
    let far_lod_state_matched = far_lod_state_failures.is_empty();
    let coverage_sampled = pixel_analysis
        .coverage
        .as_ref()
        .is_none_or(|coverage| coverage.projected_columns > 0 && coverage.sampled_depth_points > 0);
    let observed_clear_depth = pixel_analysis
        .coverage
        .as_ref()
        .is_some_and(|coverage| !coverage.clear_depth_columns.is_empty());
    let coverage_matched = pixel_analysis.coverage.as_ref().is_none_or(|_| {
        coverage_sampled
            && match observation.expected {
                LodSettleExpectation::Pass => !observed_d1_d2 && !observed_clear_depth,
                LodSettleExpectation::FailD1D2 => observed_d1_d2,
            }
    });
    let determinism_matched = pixel_analysis
        .determinism
        .as_ref()
        .is_none_or(|determinism| determinism.mismatch_count == 0);
    let observed = if !coherence_failures.is_empty() {
        "fail:set-coherence"
    } else if !far_lod_state_matched {
        "fail:far-lod-state"
    } else if !lod_assertions_matched {
        "fail:lod-assertion"
    } else if !determinism_matched {
        "fail:pixel-determinism"
    } else if observed_d1_d2 {
        "fail:d1-d2"
    } else if observed_clear_depth {
        "fail:c2-clear-depth"
    } else {
        "pass"
    };
    let matched_expectation = coherence_failures.is_empty()
        && far_lod_state_matched
        && lod_assertions_matched
        && coverage_matched
        && determinism_matched
        && match observation.expected {
            // Slice 1B defined the spawn fixture as the set-coherence canary;
            // pixel/coverage completeness becomes a pass condition in 1C2.
            LodSettleExpectation::Pass => !observed_d1_d2 && !observed_clear_depth,
            LodSettleExpectation::FailD1D2 => observed_d1_d2,
        };
    let mut assertion_failures = coherence_failures.clone();
    assertion_failures.extend(far_lod_state_failures);
    assertion_failures.extend(lod_assertion_failures);
    if !coverage_sampled {
        assertion_failures.push("C2: coverage probe projected no columns".to_owned());
    }
    if let Some(coverage) = pixel_analysis
        .coverage
        .as_ref()
        .filter(|coverage| !coverage.clear_depth_columns.is_empty())
    {
        assertion_failures.push(format!(
            "C2: {} inset samples retained clear depth across {} in-coverage columns",
            coverage.clear_depth_sample_count,
            coverage.clear_depth_columns.len(),
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
    if observed_d1_d2 {
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
            "projectedColumns": coverage.projected_columns,
            "sampledDepthPoints": coverage.sampled_depth_points,
            "clearDepthSampleCount": coverage.clear_depth_sample_count,
            "clearDepthColumns": chunk_positions_json(&coverage.clear_depth_columns),
            "clearDepthSamples": coverage.clear_depth_samples.iter().map(|sample| serde_json::json!({
                "chunk": { "x": sample.chunk.x, "z": sample.chunk.z },
                "source": sample.source,
                "local": { "x": sample.local_x, "z": sample.local_z },
                "surfaceY": sample.surface_y,
                "pixel": { "x": sample.pixel[0], "y": sample.pixel[1] },
                "clearPixelsIn3x3": sample.clear_pixels_in_3x3,
            })).collect::<Vec<_>>(),
            "depthIsCoverageEvidence": true,
        })),
        "pixelDeterminism": pixel_analysis.determinism.as_ref().map(|determinism| serde_json::json!({
            "reference": determinism.reference,
            "mismatchCount": determinism.mismatch_count,
        })),
        "lodAssertions": lod_assertions,
        "mutation": observation.mutation.map(|mutation| serde_json::json!({
            "kind": mutation.report_name(),
            "extraRadiusChunks": match mutation {
                LodSettleMutation::SetFarLodRange { extra_radius_chunks } => Some(extra_radius_chunks),
                LodSettleMutation::ToggleFarLod => None,
            },
        })),
        "farLodState": far_lod_state,
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
        "ledger": if observed_clear_depth || observed_d1_d2 || !matched_expectation { ledger_json(&observation.snapshot) } else { serde_json::Value::Null },
    })
}

fn settle_coherence_failures(observation: &FixtureObservation) -> Vec<String> {
    let producer = &observation.snapshot.runtime.producer;
    let expects_enabled = observation
        .expected_far_lod_state
        .is_none_or(|state| state.enabled);
    let desired = producer
        .desired_tiles
        .iter()
        .map(|(pos, level)| LodTileKey::new(*pos, *level))
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    // The aggregate also includes normal-section work that a capture render
    // can discover after the drive-to-idle pass. Keep it in the report, but
    // gate the exact far-LOD lifecycle below and painted depth separately.
    if expects_enabled && desired.is_empty() {
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
    ] {
        let missing = desired.difference(actual).count();
        if missing != 0 {
            failures.push(format!("{missing} desired tiles are not {label}"));
        }
    }
    let expected_visible = desired
        .iter()
        .copied()
        .filter(|tile| {
            !observation
                .snapshot
                .runtime
                .suppressed_chunks
                .contains(&tile.chunk)
        })
        .collect::<BTreeSet<_>>();
    if producer.visible_tiles != expected_visible {
        failures.push(format!(
            "visible tile set differs from unsuppressed desired: visible={} expected={}",
            producer.visible_tiles.len(),
            expected_visible.len()
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
        "lodPrefetchLevel": row.lod_prefetch_level,
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
            lod_assertions: Vec::new(),
            mutation: None,
            expected_far_lod_state: None,
            actual_far_lod_config: FarTerrainLodConfig::enabled(),
            render_distance: 4,
        };

        assert!(settle_coherence_failures(&observation).is_empty());
    }

    #[test]
    fn coherence_allows_desired_prebuild_hidden_by_real_terrain() {
        let pos = ChunkPos::new(1, 2);
        let tile = LodTileKey::new(pos, 1);
        let mut producer = FarTerrainLodSettleSnapshot::default();
        producer.desired_tiles.insert(pos, 1);
        producer.resident_tiles.insert(tile);
        producer.uploaded_tiles.insert(tile);
        let snapshot = FarLodSettleSnapshot {
            runtime: FarLodRuntimeSettleSnapshot {
                producer,
                suppressed_chunks: BTreeSet::from([pos]),
                ..FarLodRuntimeSettleSnapshot::default()
            },
            ..FarLodSettleSnapshot::default()
        };
        let observation = FixtureObservation {
            name: SPAWN_FIXTURE.to_owned(),
            expected: LodSettleExpectation::Pass,
            warmup: OffscreenWarmupReport::default(),
            summary: mclone_app_runtime::frame_render::FullFrameRenderSummary {
                drawn_section_count: 1,
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
            lod_assertions: Vec::new(),
            mutation: None,
            expected_far_lod_state: None,
            actual_far_lod_config: FarTerrainLodConfig::enabled(),
            render_distance: 4,
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
        assert_eq!(script.schema, 2);
        assert_eq!(script.waypoints.len(), 4);
        assert_eq!(script.waypoints[1].expected, LodSettleExpectation::Pass);
        assert_eq!(script.waypoints[1].coverage_probe.unwrap().plane_y, 64.0);
        assert_eq!(
            script.waypoints[3].matches_pixels.as_deref(),
            Some(FLY_UP_FIXTURE)
        );
    }

    #[test]
    fn checked_in_movement_script_is_valid() {
        let script: LodSettleScript = serde_json::from_str(include_str!(
            "../../../../test/fixtures/far-lod/settle-movement.json"
        ))
        .unwrap();

        script.validate().unwrap();
        assert_eq!(script.schema, 3);
        assert_eq!(script.waypoints.len(), 5);
        assert_eq!(
            script.waypoints[3].lod_assertions[0].expected,
            LodDesiredLevelExpectation::Level1
        );
        assert_eq!(
            script.waypoints[4].lod_assertions[0].expected,
            LodDesiredLevelExpectation::NotDesired
        );
        let smooth = SmoothMovementPlan::from_script(&script).unwrap();
        assert_eq!(smooth.moving_frames, 220);
        assert_eq!(smooth.poses.len(), 220 + SMOOTH_MOVEMENT_TAIL_FRAMES);
        assert_eq!(smooth.start.eye.to_array(), [8.25, 500.0, 8.25]);
        assert_eq!(
            smooth.poses.last().unwrap().eye.to_array(),
            [184.25, 500.0, 8.25]
        );
    }

    #[test]
    fn checked_in_mutation_script_is_valid() {
        let script: LodSettleScript = serde_json::from_str(include_str!(
            "../../../../test/fixtures/far-lod/settle-mutations.json"
        ))
        .unwrap();

        script.validate().unwrap();
        assert_eq!(script.schema, SCRIPT_SCHEMA);
        assert_eq!(script.waypoints.len(), 5);
        assert_eq!(
            script.waypoints[1].mutation,
            Some(LodSettleMutation::ToggleFarLod)
        );
        assert_eq!(
            script.waypoints[4].mutation,
            Some(LodSettleMutation::SetFarLodRange {
                extra_radius_chunks: 8
            })
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
        script.waypoints[1].coverage_probe = None;
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
    fn lod_assertions_require_schema_three_and_unique_chunks() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        let assertion = LodTileAssertion {
            chunk: [9, 0],
            expected: LodDesiredLevelExpectation::Level3,
        };
        script.waypoints[0].lod_assertions.push(assertion);
        script.schema = 2;
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("requires script schema 3")
        );

        script.schema = 3;
        script.waypoints[0].lod_assertions.push(assertion);
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("duplicate LOD assertion")
        );
    }

    #[test]
    fn mutations_require_schema_four_and_expected_state() {
        let mut script = LodSettleScript::default_for_scene(&crate::cli::SceneOptions::default());
        script.waypoints[0].mutation = Some(LodSettleMutation::ToggleFarLod);
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("requires expectedFarLodState")
        );

        script.schema = 3;
        script.waypoints[0].expected_far_lod_state = Some(FarLodStateAssertion {
            enabled: false,
            extra_radius_chunks: 6,
            desired_tiles: 0,
            suppressed_chunks: 0,
        });
        assert!(
            script
                .validate()
                .unwrap_err()
                .to_string()
                .contains("requires script schema 4")
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
    fn coverage_probe_maps_clear_depth_back_into_coverage() {
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
                suppressed: true,
                ..FarLodChunkLedgerRow::default()
            },
        );
        let depths = vec![0.0; (size[0] * size[1]) as usize];

        let mut surface_sampler = SurfaceSampler::default();
        let result = coverage_probe_result(
            &snapshot,
            render_view,
            &depths,
            size,
            LodCoverageProbe { plane_y: 64.0 },
            FarTerrainLodConfig::enabled(),
            12345,
            0,
            &mut surface_sampler,
        );

        assert!(result.projected_columns > 0);
        assert!(result.sampled_depth_points > 0);
        assert_eq!(result.clear_depth_sample_count, result.sampled_depth_points);
        assert!(result.clear_depth_columns.contains(&ChunkPos::new(0, 0)));
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
