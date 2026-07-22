use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use image::ImageReader;
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, chunk_section_index};
use mclone_input::{FlatInputAction, FlatInputFrame};
use mclone_render::headless::{
    HeadlessFrameLoopOptions, run_headless_capture_loop, run_headless_frame_loop, save_rgba_png,
};
use mclone_render_session::EngineCameraViewMode;
use mclone_scene::{
    EmbeddedWorldActivationPhase, EmbeddedWorldActivationReport, EmbeddedWorldPreviewMutationPhase,
    EmbeddedWorldPreviewMutationSnapshot, EmbeddedWorldPreviewPhase,
    EmbeddedWorldPreviewTranslucentOrderSnapshot, MonoWorldActionStatus, WarmWorldSwitchReport,
    WorldInstanceId,
};
use mclone_server::{
    AUTHORED_WORLD_FIXTURE_MARKER_FILE, AuthoredWorldFixtureManifest, SqliteWorldStore, WorldStore,
};
use serde::Serialize;

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, LiveDioramaSmokeOptions, StartupWaitPolicy,
    XrEmulationScreenshotOptions,
};
use crate::offscreen_flat_client::{
    OffscreenFlatClientFrameOptions, OffscreenFlatClientHost, OffscreenFlatClientScreenshotReport,
    run_offscreen_flat_client_screenshot,
};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{
    WindowSceneAssets, chunk_tracking_radius_for_render_distance, square_count,
};
use crate::xr_emulation::run_xr_emulation_screenshot;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaSmokeReport {
    #[serde(skip)]
    pub(crate) directory: PathBuf,
    pub(crate) report_path: PathBuf,
    pub(crate) active_only_path: PathBuf,
    pub(crate) front_path: PathBuf,
    pub(crate) side_path: PathBuf,
    pub(crate) behind_path: PathBuf,
    pub(crate) stereo_path: PathBuf,
    pub(crate) mutation_before_path: PathBuf,
    pub(crate) mutation_after_path: PathBuf,
    pub(crate) active_preview_pixel_difference_count: usize,
    pub(crate) preview_bounded_section_count: usize,
    pub(crate) preview_drawn_section_count: usize,
    pub(crate) preview_drawn_index_count: u32,
    pub(crate) stereo_eye_pixel_difference_count: usize,
    pub(crate) front_translucent_order: LiveDioramaTranslucentOrderReport,
    pub(crate) behind_translucent_order: LiveDioramaTranslucentOrderReport,
    pub(crate) stereo_translucent_order: LiveDioramaTranslucentOrderReport,
    pub(crate) activation: LiveDioramaActivationSmokeReport,
    pub(crate) mutation: LiveDioramaMutationSmokeReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) soak: Option<LiveDioramaSoakReport>,
    pub(crate) standby_cadence_hz: [u32; 3],
    pub(crate) standby_cadence_applied: bool,
    pub(crate) standby_loaded_chunk_count: usize,
    pub(crate) standby_configured_chunk_limit: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaActivationSmokeReport {
    pub(crate) schema_version: u32,
    pub(crate) initial_path: PathBuf,
    pub(crate) outbound_covered_path: PathBuf,
    pub(crate) outbound_first_uncovered_path: PathBuf,
    pub(crate) return_covered_path: PathBuf,
    pub(crate) return_first_uncovered_path: PathBuf,
    pub(crate) outbound: LiveDioramaActivationLegReport,
    pub(crate) return_leg: LiveDioramaActivationLegReport,
    pub(crate) synthetic_stereo: [LiveDioramaActivationLegReport; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaActivationLegReport {
    pub(crate) sequence: u64,
    pub(crate) source_world: u64,
    pub(crate) destination_world: u64,
    pub(crate) close_ms: f64,
    pub(crate) covered_ms: f64,
    pub(crate) open_ms: f64,
    pub(crate) switch_elapsed_ms: f64,
    pub(crate) covered_rendered_frames: u32,
    pub(crate) first_uncovered_activation_frame: u32,
    pub(crate) first_uncovered_drawn_section_count: usize,
    pub(crate) first_uncovered_eye_count: usize,
    pub(crate) first_uncovered_uploaded_section_count: usize,
    pub(crate) first_uncovered_submitted_compile_section_count: usize,
    pub(crate) first_uncovered_accepted_compile_result_count: usize,
    pub(crate) first_uncovered_queue_lifecycle_items: usize,
    pub(crate) first_uncovered_pending_compile_jobs: usize,
    pub(crate) accepted_entry: [f64; 3],
    pub(crate) post_swap_entry: [f64; 3],
    pub(crate) post_swap_on_ground: bool,
    pub(crate) post_swap_supported: bool,
    pub(crate) first_uncovered_entry: [f64; 3],
    pub(crate) first_uncovered_on_ground: bool,
    pub(crate) first_uncovered_supported: bool,
    pub(crate) stability_activation_frame: u32,
    pub(crate) stability_entry: [f64; 3],
    pub(crate) stability_on_ground: bool,
    pub(crate) stability_supported: bool,
    pub(crate) switch_uploaded_section_count: usize,
    pub(crate) switch_submitted_compile_section_count: usize,
    pub(crate) switch_accepted_compile_result_count: usize,
    pub(crate) switch_materialized_renderer: bool,
    pub(crate) source_cadence_changed: bool,
    pub(crate) destination_cadence_changed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaTranslucentOrderReport {
    pub(crate) section_count: usize,
    pub(crate) active_section_count: usize,
    pub(crate) preview_section_count: usize,
    pub(crate) source_switch_count: usize,
    pub(crate) first_world: Option<u64>,
    pub(crate) first_section: Option<[i32; 3]>,
    pub(crate) last_world: Option<u64>,
    pub(crate) last_section: Option<[i32; 3]>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaMutationSmokeReport {
    pub(crate) block: [i32; 3],
    pub(crate) before_after_pixel_difference_count: usize,
    pub(crate) command_changed: bool,
    pub(crate) command_update_count: usize,
    pub(crate) command_section_block_update_count: usize,
    pub(crate) completed_after_rendered_frame: u32,
    pub(crate) submitted_compile_section_count: usize,
    pub(crate) accepted_compile_result_count: usize,
    pub(crate) uploaded_section_count: usize,
    pub(crate) max_pending_compile_jobs: usize,
    pub(crate) max_queued_upload_lifecycle_items: usize,
    pub(crate) max_queued_upload_mesh_owned_bytes: usize,
    pub(crate) out_of_region_submission_count: usize,
    pub(crate) active_world_block_unchanged: bool,
    pub(crate) persisted_after_restart: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDioramaSoakReport {
    pub(crate) duration_seconds: u64,
    pub(crate) frame_count: usize,
    pub(crate) fixed_interest_center: [i32; 2],
    pub(crate) baseline_bounded_section_count: usize,
    pub(crate) final_bounded_section_count: usize,
    pub(crate) max_pending_compile_jobs: usize,
    pub(crate) max_queued_upload_lifecycle_items: usize,
    pub(crate) max_queued_upload_mesh_owned_bytes: usize,
    pub(crate) out_of_region_submission_count: usize,
    pub(crate) camera_orbit_changed_source_priority: bool,
}

struct LiveDioramaMutationState {
    host: OffscreenFlatClientHost,
    block: BlockPos,
    active_block_before: Option<BlockStateId>,
    active_block_after: Option<BlockStateId>,
    mutation: Option<EmbeddedWorldPreviewMutationSnapshot>,
    completed_frame_index: Option<usize>,
}

struct LiveDioramaActivationState {
    host: OffscreenFlatClientHost,
    source_world: WorldInstanceId,
    destination_world: Option<WorldInstanceId>,
    requested_leg_count: usize,
    reports: Vec<EmbeddedWorldActivationReport>,
    switches: Vec<WarmWorldSwitchReport>,
    covered_frame_indices: Vec<usize>,
    first_uncovered_frame_indices: Vec<usize>,
}

struct LiveDioramaSoakState {
    host: OffscreenFlatClientHost,
    target: Vec3,
    fixed_interest_center: mclone_core::ChunkPos,
    baseline_bounded_section_count: usize,
    final_bounded_section_count: usize,
    initial_source_priority: mclone_core::Vec3d,
    source_priority_changed: bool,
    max_pending_compile_jobs: usize,
    max_queued_upload_lifecycle_items: usize,
    max_queued_upload_mesh_owned_bytes: usize,
    out_of_region_submission_count: usize,
}

pub(crate) fn run_live_diorama_smoke(
    options: &LiveDioramaSmokeOptions,
) -> Result<LiveDioramaSmokeReport> {
    fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create live-diorama smoke directory `{}`",
            options.directory.display()
        )
    })?;
    let diorama = options
        .scene
        .live_diorama
        .as_ref()
        .context("live-diorama smoke has no configured preview")?;
    let anchor = diorama.placement.composition_anchor();
    let target = [anchor.x as f32, anchor.y as f32 + 0.25, anchor.z as f32];
    let front_eye = [
        anchor.x as f32,
        anchor.y as f32 + 2.0,
        anchor.z as f32 - 14.0,
    ];
    let side_eye = [
        anchor.x as f32 - 6.0,
        anchor.y as f32 + 2.0,
        anchor.z as f32,
    ];
    let behind_eye = [
        anchor.x as f32,
        anchor.y as f32 + 2.0,
        anchor.z as f32 + 14.0,
    ];

    let active_only_path = options.directory.join("a-alone.png");
    let front_path = options.directory.join("a-plus-b-front.png");
    let side_path = options.directory.join("a-plus-b-side.png");
    let behind_path = options.directory.join("a-plus-b-behind.png");
    let stereo_path = options.directory.join("a-plus-b-stereo.png");
    let mutation_before_path = options.directory.join("mutation-before.png");
    let mutation_after_path = options.directory.join("mutation-after.png");
    let activation_initial_path = options.directory.join("activation-a-before.png");
    let activation_outbound_covered_path = options.directory.join("activation-a-to-b-covered.png");
    let activation_outbound_first_uncovered_path = options
        .directory
        .join("activation-a-to-b-first-uncovered.png");
    let activation_return_covered_path = options.directory.join("activation-b-to-a-covered.png");
    let activation_return_first_uncovered_path = options
        .directory
        .join("activation-b-to-a-first-uncovered.png");
    let report_path = options.directory.join("report.json");

    let mut active_scene = options.scene.clone();
    active_scene.live_diorama = None;
    active_scene.warm_world_standby_cadence = None;
    let active_only = run_offscreen_flat_client_screenshot(&screenshot_options(
        options,
        active_scene,
        active_only_path.clone(),
        front_eye,
        target,
    ))?;
    if active_only.summary.drawn_section_count == 0 || active_only.embedded_preview.is_some() {
        bail!(
            "A-only control did not preserve the ordinary direct path: drawn={} preview={:?}",
            active_only.summary.drawn_section_count,
            active_only.embedded_preview,
        );
    }
    let front = run_offscreen_flat_client_screenshot(&screenshot_options(
        options,
        options.scene.clone(),
        front_path.clone(),
        front_eye,
        target,
    ))?;
    let side = run_offscreen_flat_client_screenshot(&screenshot_options(
        options,
        options.scene.clone(),
        side_path.clone(),
        side_eye,
        target,
    ))?;
    let behind = run_offscreen_flat_client_screenshot(&screenshot_options(
        options,
        options.scene.clone(),
        behind_path.clone(),
        behind_eye,
        target,
    ))?;

    validate_visible_preview("front", &front)?;
    validate_visible_preview("side", &side)?;
    validate_visible_preview("behind", &behind)?;
    let preview = front
        .embedded_preview
        .as_ref()
        .expect("validated front preview exists");
    let standby = front
        .warm_world_standby
        .as_ref()
        .context("front capture lost retained-world diagnostics")?;
    if let Some(expected) = options.scene.warm_world_standby_cadence {
        if standby.standby_cadence != expected || !standby.standby_cadence_applied {
            bail!(
                "preview drawing changed or failed to apply standby cadence: expected={expected:?} actual={:?} applied={}",
                standby.standby_cadence,
                standby.standby_cadence_applied,
            );
        }
    }

    let stereo = run_xr_emulation_screenshot(&XrEmulationScreenshotOptions {
        path: stereo_path.clone(),
        eye_width: options.width / 2,
        eye_height: options.height,
        scene: options.scene.clone(),
        render_options: options.render_options,
        held_keys: Vec::new(),
        input_frames: 1,
    })?;
    let stereo_preview = stereo
        .embedded_preview
        .as_ref()
        .context("synthetic-stereo capture lost embedded preview diagnostics")?;
    if stereo_preview.phase != EmbeddedWorldPreviewPhase::Visible
        || stereo_preview.last_drawn_section_count == 0
        || stereo_preview.last_drawn_index_count == 0
    {
        bail!("synthetic-stereo capture did not draw the embedded preview: {stereo_preview:?}");
    }
    let front_translucent_order = translucent_order_report(preview.render.last_translucent_order);
    let behind_preview = behind
        .embedded_preview
        .as_ref()
        .context("behind capture lost embedded preview diagnostics")?;
    let behind_translucent_order =
        translucent_order_report(behind_preview.render.last_translucent_order);
    let stereo_translucent_order =
        translucent_order_report(stereo_preview.render.last_translucent_order);
    validate_reversible_translucent_fixture_orders(
        preview.source_world.get(),
        &front_translucent_order,
        &behind_translucent_order,
        &stereo_translucent_order,
    )?;

    let active_preview_pixel_difference_count =
        differing_pixel_count(&active_only_path, &front_path)?;
    if active_preview_pixel_difference_count == 0 {
        bail!("A-only and A+B captures are pixel-identical");
    }

    let activation = run_live_diorama_activation_smoke(
        options,
        &activation_initial_path,
        &activation_outbound_covered_path,
        &activation_outbound_first_uncovered_path,
        &activation_return_covered_path,
        &activation_return_first_uncovered_path,
        &stereo.embedded_activation_reports,
        &stereo.embedded_activation_switch_reports,
    )?;
    let mutation = run_live_diorama_mutation_smoke(
        options,
        Vec3::from_array(front_eye),
        Vec3::from_array(target),
        &mutation_before_path,
        &mutation_after_path,
    )?;
    let soak = (options.soak_seconds > 0)
        .then(|| run_live_diorama_post_mutation_soak(options, Vec3::from_array(target)))
        .transpose()?;

    let cadence = standby.standby_cadence;
    let render_distance = u32::try_from(options.scene.render_distance)
        .context("live-diorama render distance must be non-negative")?;
    let tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
    let standby_configured_chunk_limit = square_count(
        i32::try_from(tracking_radius).context("standby tracking radius does not fit i32")?,
    )?;
    if standby.loaded_chunks > standby_configured_chunk_limit {
        bail!(
            "B subscription exceeded its configured view: loaded={} limit={}",
            standby.loaded_chunks,
            standby_configured_chunk_limit
        );
    }
    let report = LiveDioramaSmokeReport {
        directory: options.directory.clone(),
        report_path,
        active_only_path,
        front_path,
        side_path,
        behind_path,
        stereo_path,
        mutation_before_path,
        mutation_after_path,
        active_preview_pixel_difference_count,
        preview_bounded_section_count: preview.bounded_section_count,
        preview_drawn_section_count: preview.last_drawn_section_count,
        preview_drawn_index_count: preview.last_drawn_index_count,
        stereo_eye_pixel_difference_count: stereo.eye_pixel_difference_count,
        front_translucent_order,
        behind_translucent_order,
        stereo_translucent_order,
        activation,
        mutation,
        soak,
        standby_cadence_hz: [
            cadence.host_rate_hz,
            cadence.gameplay_rate_hz,
            cadence.physics_rate_hz,
        ],
        standby_cadence_applied: standby.standby_cadence_applied,
        standby_loaded_chunk_count: standby.loaded_chunks,
        standby_configured_chunk_limit,
    };
    let json = serde_json::to_vec_pretty(&report).context("encode live-diorama smoke report")?;
    fs::write(&report.report_path, json).with_context(|| {
        format!(
            "write live-diorama smoke report `{}`",
            report.report_path.display()
        )
    })?;
    Ok(report)
}

fn run_live_diorama_post_mutation_soak(
    options: &LiveDioramaSmokeOptions,
    target: Vec3,
) -> Result<LiveDioramaSoakReport> {
    const SOAK_RATE_HZ: u64 = 10;
    let frame_count = usize::try_from(
        options
            .soak_seconds
            .checked_mul(SOAK_RATE_HZ)
            .context("live-diorama soak frame count overflow")?,
    )
    .context("live-diorama soak frame count does not fit usize")?;
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let (loop_report, state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: 320,
            height: 200,
            frame_count,
            pace_frame_duration: Some(Duration::from_millis(1_000 / SOAK_RATE_HZ)),
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
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Playable)?;
            host.drive_until_embedded_preview_idle(device, queue)?;
            let preview = host
                .scene_host()
                .embedded_world_preview_snapshot()
                .context("post-mutation soak has no embedded preview")?;
            Ok(LiveDioramaSoakState {
                host,
                target,
                fixed_interest_center: preview.fixed_interest_center,
                baseline_bounded_section_count: preview.bounded_section_count,
                final_bounded_section_count: preview.bounded_section_count,
                initial_source_priority: preview.preparation.source_priority_position,
                source_priority_changed: false,
                max_pending_compile_jobs: 0,
                max_queued_upload_lifecycle_items: 0,
                max_queued_upload_mesh_owned_bytes: 0,
                out_of_region_submission_count: 0,
            })
        },
        |frame_index, frame, state| {
            // One orbit per minute exercises B's inverse/clamped source
            // priority while its server interest remains fixed.
            let orbit_period_frames = SOAK_RATE_HZ as usize * 60;
            let phase = (frame_index % orbit_period_frames) as f32 / orbit_period_frames as f32
                * std::f32::consts::TAU;
            let eye = Vec3::new(
                state.target.x + phase.sin() * 6.0,
                state.target.y + 1.75,
                state.target.z - phase.cos() * 6.0,
            );
            state.host.set_camera_look_at(eye, state.target);
            state.host.commit_camera()?;
            state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions { hud: false })?;
            let preview = state
                .host
                .scene_host()
                .embedded_world_preview_snapshot()
                .context("post-mutation soak lost its embedded preview")?;
            if preview.fixed_interest_center != state.fixed_interest_center {
                bail!(
                    "post-mutation soak moved B interest from {:?} to {:?}",
                    state.fixed_interest_center,
                    preview.fixed_interest_center
                );
            }
            state.final_bounded_section_count = preview.bounded_section_count;
            state.max_pending_compile_jobs = state
                .max_pending_compile_jobs
                .max(preview.preparation.pending_compile_jobs);
            state.max_queued_upload_lifecycle_items = state
                .max_queued_upload_lifecycle_items
                .max(preview.preparation.queued_upload_lifecycle_items);
            state.max_queued_upload_mesh_owned_bytes = state
                .max_queued_upload_mesh_owned_bytes
                .max(preview.preparation.queued_upload_mesh_owned_bytes);
            state.out_of_region_submission_count = preview.render.out_of_region_submission_count;
            state.source_priority_changed |=
                preview.preparation.source_priority_position != state.initial_source_priority;
            if state.out_of_region_submission_count != 0 {
                bail!("post-mutation soak submitted preview geometry outside B's region");
            }
            Ok(())
        },
    )?;

    if state.final_bounded_section_count != state.baseline_bounded_section_count {
        bail!(
            "post-mutation soak changed B resident sections from {} to {}",
            state.baseline_bounded_section_count,
            state.final_bounded_section_count
        );
    }
    if state.max_pending_compile_jobs > 4
        || state.max_queued_upload_lifecycle_items > 1
        || !state.source_priority_changed
    {
        bail!(
            "post-mutation soak violated bounded-work/orbit contract: pending={} upload_queue={} priority_changed={}",
            state.max_pending_compile_jobs,
            state.max_queued_upload_lifecycle_items,
            state.source_priority_changed
        );
    }
    Ok(LiveDioramaSoakReport {
        duration_seconds: options.soak_seconds,
        frame_count: loop_report.frame_count,
        fixed_interest_center: [state.fixed_interest_center.x, state.fixed_interest_center.z],
        baseline_bounded_section_count: state.baseline_bounded_section_count,
        final_bounded_section_count: state.final_bounded_section_count,
        max_pending_compile_jobs: state.max_pending_compile_jobs,
        max_queued_upload_lifecycle_items: state.max_queued_upload_lifecycle_items,
        max_queued_upload_mesh_owned_bytes: state.max_queued_upload_mesh_owned_bytes,
        out_of_region_submission_count: state.out_of_region_submission_count,
        camera_orbit_changed_source_priority: state.source_priority_changed,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_live_diorama_activation_smoke(
    options: &LiveDioramaSmokeOptions,
    initial_path: &Path,
    outbound_covered_path: &Path,
    outbound_first_uncovered_path: &Path,
    return_covered_path: &Path,
    return_first_uncovered_path: &Path,
    stereo_reports: &[EmbeddedWorldActivationReport],
    stereo_switches: &[WarmWorldSwitchReport],
) -> Result<LiveDioramaActivationSmokeReport> {
    const FRAME_COUNT: usize = 64;
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let (_, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: FRAME_COUNT,
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
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Playable)?;
            host.drive_until_embedded_preview_idle(device, queue)?;
            aim_flat_host_at_embedded_preview(&mut host)?;
            let source_world = host.scene_host().active_world_instance_id();
            Ok(LiveDioramaActivationState {
                host,
                source_world,
                destination_world: None,
                requested_leg_count: 0,
                reports: Vec::with_capacity(2),
                switches: Vec::with_capacity(2),
                covered_frame_indices: Vec::with_capacity(2),
                first_uncovered_frame_indices: Vec::with_capacity(2),
            })
        },
        |frame_index, frame, state| {
            if frame_index > 0 {
                state.host.apply_input_frame(FlatInputFrame::default())?;
            }
            state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions { hud: false })?;

            let snapshot = state.host.scene_host().embedded_world_activation_snapshot();
            if snapshot.phase == EmbeddedWorldActivationPhase::Failed {
                bail!("flat embedded-world activation failed: {snapshot:?}");
            }
            if snapshot.alpha >= 1.0
                && state.covered_frame_indices.len() < state.requested_leg_count
            {
                state.covered_frame_indices.push(frame_index);
            }
            if snapshot
                .last_report
                .as_ref()
                .is_some_and(|report| report.first_uncovered_activation_frame.is_some())
                && state.first_uncovered_frame_indices.len() < state.requested_leg_count
            {
                state.first_uncovered_frame_indices.push(frame_index);
            }

            if snapshot.phase == EmbeddedWorldActivationPhase::Idle
                && snapshot.last_report.as_ref().is_some_and(|report| {
                    state.reports.last().map(|saved| saved.sequence) != Some(report.sequence)
                })
            {
                let report = snapshot
                    .last_report
                    .context("flat embedded-world activation completed without a report")?;
                state.switches.push(
                    state
                        .host
                        .scene_host()
                        .last_warm_world_switch_report()
                        .context("flat embedded-world activation completed without switch facts")?,
                );
                state.reports.push(report);
                if state.reports.len() == 1 {
                    state.destination_world =
                        Some(state.host.scene_host().active_world_instance_id());
                    aim_flat_host_at_embedded_preview(&mut state.host)?;
                    request_flat_embedded_world_activation(&mut state.host)?;
                    state.requested_leg_count = 2;
                }
            }

            if frame_index == 0 {
                request_flat_embedded_world_activation(&mut state.host)?;
                state.requested_leg_count = 1;
            }
            Ok(())
        },
    )?;

    if state.reports.len() != 2
        || state.switches.len() != 2
        || state.covered_frame_indices.len() != 2
        || state.first_uncovered_frame_indices.len() != 2
    {
        bail!(
            "flat embedded-world activation did not produce two complete covered transitions: reports={} switches={} covered={} uncovered={}",
            state.reports.len(),
            state.switches.len(),
            state.covered_frame_indices.len(),
            state.first_uncovered_frame_indices.len(),
        );
    }
    let destination_world = state
        .destination_world
        .context("flat embedded-world activation lost destination identity")?;
    if state.host.scene_host().active_world_instance_id() != state.source_world
        || state.reports[0].source_world != state.source_world
        || state.reports[0].destination_world != destination_world
        || state.reports[1].source_world != destination_world
        || state.reports[1].destination_world != state.source_world
    {
        bail!("flat embedded-world activation lost A-to-B-to-A identity");
    }
    for (report, switch) in state.reports.iter().zip(&state.switches) {
        validate_flat_embedded_world_activation(report, switch)?;
    }
    if stereo_reports.len() != 2 || stereo_switches.len() != 2 {
        bail!(
            "synthetic-stereo activation produced {} reports/{} switches, expected two each",
            stereo_reports.len(),
            stereo_switches.len(),
        );
    }

    let covered = [
        state.covered_frame_indices[0],
        state.covered_frame_indices[1],
    ];
    let uncovered = [
        state.first_uncovered_frame_indices[0],
        state.first_uncovered_frame_indices[1],
    ];
    save_rgba_png(
        initial_path,
        options.width,
        options.height,
        &frame_pixels[0],
    )?;
    save_rgba_png(
        outbound_covered_path,
        options.width,
        options.height,
        &frame_pixels[covered[0]],
    )?;
    save_rgba_png(
        outbound_first_uncovered_path,
        options.width,
        options.height,
        &frame_pixels[uncovered[0]],
    )?;
    save_rgba_png(
        return_covered_path,
        options.width,
        options.height,
        &frame_pixels[covered[1]],
    )?;
    save_rgba_png(
        return_first_uncovered_path,
        options.width,
        options.height,
        &frame_pixels[uncovered[1]],
    )?;
    for frame_index in covered {
        if frame_pixels[frame_index]
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [0, 0, 0])
        {
            bail!("covered activation frame {frame_index} was not fully black");
        }
    }
    for frame_index in uncovered {
        if !frame_pixels[frame_index]
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [0, 0, 0])
        {
            bail!("first uncovered activation frame {frame_index} was blank");
        }
    }

    Ok(LiveDioramaActivationSmokeReport {
        schema_version: 1,
        initial_path: initial_path.to_owned(),
        outbound_covered_path: outbound_covered_path.to_owned(),
        outbound_first_uncovered_path: outbound_first_uncovered_path.to_owned(),
        return_covered_path: return_covered_path.to_owned(),
        return_first_uncovered_path: return_first_uncovered_path.to_owned(),
        outbound: activation_leg_report(&state.reports[0], Some(&state.switches[0]))?,
        return_leg: activation_leg_report(&state.reports[1], Some(&state.switches[1]))?,
        synthetic_stereo: [
            activation_leg_report(&stereo_reports[0], Some(&stereo_switches[0]))?,
            activation_leg_report(&stereo_reports[1], Some(&stereo_switches[1]))?,
        ],
    })
}

pub(crate) fn aim_flat_host_at_embedded_preview(host: &mut OffscreenFlatClientHost) -> Result<()> {
    let preview = host
        .scene_host()
        .embedded_world_preview_snapshot()
        .context("flat activation has no embedded preview placement")?;
    if preview.phase != EmbeddedWorldPreviewPhase::Visible {
        bail!("flat activation preview is not visible: {preview:?}");
    }
    let anchor = preview.placement.composition_anchor();
    let target = Vec3::new(anchor.x as f32, anchor.y as f32 + 0.25, anchor.z as f32);
    let eye = Vec3::new(
        anchor.x as f32,
        anchor.y as f32 + 2.0,
        anchor.z as f32 - 6.0,
    );
    host.set_camera_look_at(eye, target);
    host.commit_camera()?;
    Ok(())
}

pub(crate) fn request_flat_embedded_world_activation(
    host: &mut OffscreenFlatClientHost,
) -> Result<()> {
    let statuses = host.apply_input_frame(FlatInputFrame {
        use_item: true,
        ..FlatInputFrame::default()
    })?;
    match statuses.as_slice() {
        [(FlatInputAction::Use, MonoWorldActionStatus::EmbeddedWorldActivationRequested)] => Ok(()),
        _ => bail!("flat diorama Use did not request scene activation: {statuses:?}"),
    }
}

fn validate_flat_embedded_world_activation(
    report: &EmbeddedWorldActivationReport,
    switch: &WarmWorldSwitchReport,
) -> Result<()> {
    if report.switch_elapsed_ms.is_none()
        || report.covered_rendered_frames == 0
        || report.first_uncovered_world != Some(report.destination_world)
        || report.first_uncovered_drawn_section_count == 0
        || report.first_uncovered_uploaded_section_count != 0
        || report.first_uncovered_submitted_compile_section_count != 0
        || report.first_uncovered_accepted_compile_result_count != 0
        || report.first_uncovered_eye_count != 1
        || !report
            .post_swap_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .first_uncovered_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .stability_entry
            .is_some_and(|sample| sample.support.supported())
        || report.stability_activation_frame.is_none()
        || report.completed_activation_frame.is_none()
        || report.failure.is_some()
        || switch.source_instance_id != report.source_world
        || switch.destination_instance_id != report.destination_world
        || switch.switch_uploaded_section_count != 0
        || switch.switch_submitted_compile_section_count != 0
        || switch.switch_accepted_compile_result_count != 0
        || switch.switch_materialized_renderer
    {
        bail!(
            "flat embedded-world activation violated cover/conservation facts: report={report:?} switch={switch:?}"
        );
    }
    Ok(())
}

fn activation_leg_report(
    report: &EmbeddedWorldActivationReport,
    switch: Option<&WarmWorldSwitchReport>,
) -> Result<LiveDioramaActivationLegReport> {
    let accepted_entry = report
        .accepted_destination_entry_pose
        .context("activation report omitted accepted destination entry")?;
    let post_swap = report
        .post_swap_entry
        .context("activation report omitted post-swap support sample")?;
    let first_uncovered = report
        .first_uncovered_entry
        .context("activation report omitted first-uncovered support sample")?;
    let stability = report
        .stability_entry
        .context("activation report omitted delayed stability sample")?;
    Ok(LiveDioramaActivationLegReport {
        sequence: report.sequence,
        source_world: report.source_world.get(),
        destination_world: report.destination_world.get(),
        close_ms: report.close_seconds * 1_000.0,
        covered_ms: report.covered_seconds * 1_000.0,
        open_ms: report.open_seconds * 1_000.0,
        switch_elapsed_ms: report
            .switch_elapsed_ms
            .context("activation report omitted switch duration")?,
        covered_rendered_frames: report.covered_rendered_frames,
        first_uncovered_activation_frame: report
            .first_uncovered_activation_frame
            .context("activation report omitted first uncovered frame")?,
        first_uncovered_drawn_section_count: report.first_uncovered_drawn_section_count,
        first_uncovered_eye_count: report.first_uncovered_eye_count,
        first_uncovered_uploaded_section_count: report.first_uncovered_uploaded_section_count,
        first_uncovered_submitted_compile_section_count: report
            .first_uncovered_submitted_compile_section_count,
        first_uncovered_accepted_compile_result_count: report
            .first_uncovered_accepted_compile_result_count,
        first_uncovered_queue_lifecycle_items: report.first_uncovered_queue_lifecycle_items,
        first_uncovered_pending_compile_jobs: report.first_uncovered_pending_compile_jobs,
        accepted_entry: vec3d_array(accepted_entry.feet_position),
        post_swap_entry: vec3d_array(post_swap.pose.feet_position),
        post_swap_on_ground: post_swap.on_ground,
        post_swap_supported: post_swap.support.supported(),
        first_uncovered_entry: vec3d_array(first_uncovered.pose.feet_position),
        first_uncovered_on_ground: first_uncovered.on_ground,
        first_uncovered_supported: first_uncovered.support.supported(),
        stability_activation_frame: report
            .stability_activation_frame
            .context("activation report omitted delayed stability frame")?,
        stability_entry: vec3d_array(stability.pose.feet_position),
        stability_on_ground: stability.on_ground,
        stability_supported: stability.support.supported(),
        switch_uploaded_section_count: switch
            .map_or(0, |switch| switch.switch_uploaded_section_count),
        switch_submitted_compile_section_count: switch
            .map_or(0, |switch| switch.switch_submitted_compile_section_count),
        switch_accepted_compile_result_count: switch
            .map_or(0, |switch| switch.switch_accepted_compile_result_count),
        switch_materialized_renderer: switch
            .is_some_and(|switch| switch.switch_materialized_renderer),
        source_cadence_changed: switch.is_some_and(|switch| switch.source_cadence_changed),
        destination_cadence_changed: switch
            .is_some_and(|switch| switch.destination_cadence_changed),
    })
}

fn vec3d_array(value: mclone_core::Vec3d) -> [f64; 3] {
    [value.x, value.y, value.z]
}

fn run_live_diorama_mutation_smoke(
    options: &LiveDioramaSmokeOptions,
    eye: Vec3,
    target: Vec3,
    before_path: &Path,
    after_path: &Path,
) -> Result<LiveDioramaMutationSmokeReport> {
    // B deliberately runs at 5 Hz in the canonical smoke. Leave enough real
    // time for one server tick plus the separately budgeted compile/accept/
    // upload frames without coupling the proof to the active frame rate.
    const MUTATION_FRAME_COUNT: usize = 160;
    const MAX_AFFECTED_SECTION_COMPILES: usize = 2;

    let diorama = options
        .scene
        .live_diorama
        .as_ref()
        .context("live-diorama mutation smoke has no configured preview")?;
    let source_world_dir = diorama.world_dir.clone();
    let marker_path = source_world_dir.join(AUTHORED_WORLD_FIXTURE_MARKER_FILE);
    let manifest: AuthoredWorldFixtureManifest =
        serde_json::from_slice(&fs::read(&marker_path).with_context(|| {
            format!("read authored fixture marker `{}`", marker_path.display())
        })?)
        .with_context(|| format!("decode authored fixture marker `{}`", marker_path.display()))?;
    let block = BlockPos::new(
        manifest.mutation_block[0],
        manifest.mutation_block[1],
        manifest.mutation_block[2],
    );

    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&scene);
    let (_, frame_pixels, mut state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: MUTATION_FRAME_COUNT,
            pace_frame_duration: Some(Duration::from_millis(5)),
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
            host.start_scene_with_wait_policy(device, queue, StartupWaitPolicy::Playable)?;
            host.set_camera_look_at(eye, target);
            host.commit_camera()?;
            host.drive_until_embedded_preview_idle(device, queue)?;
            let active_block_before = host
                .scene_host()
                .mono_client()
                .and_then(|client| client.block_state_at_block_pos(block));
            Ok(LiveDioramaMutationState {
                host,
                block,
                active_block_before,
                active_block_after: None,
                mutation: None,
                completed_frame_index: None,
            })
        },
        |frame_index, frame, state| {
            state
                .host
                .render_frame(frame, OffscreenFlatClientFrameOptions { hud: false })?;
            if frame_index == 0 {
                state.mutation = Some(
                    state
                        .host
                        .scene_host_mut()
                        .debug_break_embedded_world_preview_block(state.block)?,
                );
            }
            let preview = state
                .host
                .scene_host()
                .embedded_world_preview_snapshot()
                .context("mutation smoke lost its embedded preview")?;
            if preview.render.out_of_region_submission_count != 0 {
                bail!(
                    "mutation smoke submitted {} preview sections outside its region",
                    preview.render.out_of_region_submission_count
                );
            }
            if let Some(mutation) = preview.last_mutation {
                if mutation.phase == EmbeddedWorldPreviewMutationPhase::GpuApplied
                    && state.completed_frame_index.is_none()
                {
                    state.completed_frame_index = Some(frame_index);
                }
                state.mutation = Some(mutation);
            }
            state.active_block_after = state
                .host
                .scene_host()
                .mono_client()
                .and_then(|client| client.block_state_at_block_pos(state.block));
            Ok(())
        },
    )?;

    let mutation = state
        .mutation
        .clone()
        .context("mutation smoke produced no mutation diagnostics")?;
    state.completed_frame_index.with_context(|| {
        format!(
            "mutation smoke did not reach GPU-applied state: mutation={:?} preview={:?}",
            state.mutation,
            state.host.scene_host().embedded_world_preview_snapshot()
        )
    })?;
    if mutation.phase != EmbeddedWorldPreviewMutationPhase::GpuApplied
        || !mutation.command_changed
        || mutation.command_section_block_update_count != 1
        || mutation.submitted_compile_section_count == 0
        || mutation.submitted_compile_section_count > MAX_AFFECTED_SECTION_COMPILES
        || mutation.accepted_compile_result_count == 0
        || mutation.uploaded_section_count == 0
    {
        bail!("mutation smoke violated its bounded live-update contract: {mutation:?}");
    }
    if state.active_block_before.is_none() || state.active_block_before != state.active_block_after
    {
        bail!(
            "B mutation leaked into active A: before={:?} after={:?}",
            state.active_block_before,
            state.active_block_after
        );
    }

    let after_frame_index = MUTATION_FRAME_COUNT - 1;
    save_rgba_png(before_path, options.width, options.height, &frame_pixels[0])?;
    save_rgba_png(
        after_path,
        options.width,
        options.height,
        &frame_pixels[after_frame_index],
    )?;
    let before_after_pixel_difference_count = frame_pixels[0]
        .chunks_exact(4)
        .zip(frame_pixels[after_frame_index].chunks_exact(4))
        .filter(|(before, after)| before != after)
        .count();
    if before_after_pixel_difference_count == 0 {
        bail!("GPU-applied B mutation did not change any preview pixels");
    }

    let preview = state
        .host
        .scene_host()
        .embedded_world_preview_snapshot()
        .context("mutation smoke lost final preview diagnostics")?;
    state.host.scene_host_mut().flush_persistence()?;
    drop(state);

    let mut store = SqliteWorldStore::open_world_dir(&source_world_dir)?;
    let record = store
        .load_chunk(
            &mclone_protocol::DimensionKey::overworld(),
            block.chunk_pos(),
        )?
        .with_context(|| format!("persisted fixture lost chunk {:?}", block.chunk_pos()))?;
    let persisted_after_restart =
        snapshot_block_state(&record.snapshot, block) == AIR_BLOCK_STATE_ID;
    store.close()?;
    if !persisted_after_restart {
        bail!("B mutation was not durable after reopening its world store");
    }

    Ok(LiveDioramaMutationSmokeReport {
        block: [block.x, block.y, block.z],
        before_after_pixel_difference_count,
        command_changed: mutation.command_changed,
        command_update_count: mutation.command_update_count,
        command_section_block_update_count: mutation.command_section_block_update_count,
        completed_after_rendered_frame: mutation
            .completed_after_rendered_frame
            .context("GPU-applied mutation omitted its completion frame")?,
        submitted_compile_section_count: mutation.submitted_compile_section_count,
        accepted_compile_result_count: mutation.accepted_compile_result_count,
        uploaded_section_count: mutation.uploaded_section_count,
        max_pending_compile_jobs: preview.preparation.max_pending_compile_jobs,
        max_queued_upload_lifecycle_items: preview.preparation.max_queued_upload_lifecycle_items,
        max_queued_upload_mesh_owned_bytes: preview.preparation.max_queued_upload_mesh_owned_bytes,
        out_of_region_submission_count: preview.render.out_of_region_submission_count,
        active_world_block_unchanged: true,
        persisted_after_restart,
    })
}

fn snapshot_block_state(snapshot: &mclone_core::ChunkSnapshot, block: BlockPos) -> BlockStateId {
    let section_y = block.y.div_euclid(16);
    snapshot
        .sections
        .iter()
        .find(|section| section.section_y == section_y)
        .map(|section| {
            section.unpack_block_state_ids()[chunk_section_index(
                block.x.rem_euclid(16),
                block.y.rem_euclid(16),
                block.z.rem_euclid(16),
            )]
        })
        .unwrap_or(AIR_BLOCK_STATE_ID)
}

fn screenshot_options(
    options: &LiveDioramaSmokeOptions,
    scene: crate::cli::SceneOptions,
    path: PathBuf,
    eye: [f32; 3],
    target: [f32; 3],
) -> HeadlessScreenshotOptions {
    HeadlessScreenshotOptions {
        path,
        width: options.width,
        height: options.height,
        scene,
        render_options: options.render_options,
        startup_wait: StartupWaitPolicy::Playable,
        camera_view: EngineCameraViewMode::FirstPerson,
        ui: HeadlessScreenshotUi::None,
        hud: false,
        frame_pipeline_overlay: false,
        debug_pane: false,
        player_collision_box: false,
        blink_debug: false,
        controller_focus: false,
        scripted_interaction: false,
        remote_settle_ms: 0,
        eye: Some(eye),
        target: Some(target),
    }
}

fn validate_visible_preview(
    label: &str,
    report: &OffscreenFlatClientScreenshotReport,
) -> Result<()> {
    let preview = report
        .embedded_preview
        .as_ref()
        .with_context(|| format!("{label} capture has no embedded preview"))?;
    if preview.phase != EmbeddedWorldPreviewPhase::Visible
        || !preview.renderer_topology_ready
        || !preview.source_anchor_gpu_resident
        || !preview.source_anchor_traversal_ready
        || preview.bounded_section_count == 0
        || preview.last_drawn_section_count == 0
        || preview.last_drawn_index_count == 0
    {
        bail!("{label} capture did not reach drawable preview readiness: {preview:?}");
    }
    Ok(())
}

fn translucent_order_report(
    snapshot: EmbeddedWorldPreviewTranslucentOrderSnapshot,
) -> LiveDioramaTranslucentOrderReport {
    let section = |submission: mclone_scene::EmbeddedWorldPreviewTranslucentSubmissionSnapshot| {
        [
            submission.section.chunk_x,
            submission.section.section_y,
            submission.section.chunk_z,
        ]
    };
    LiveDioramaTranslucentOrderReport {
        section_count: snapshot.section_count,
        active_section_count: snapshot.active_section_count,
        preview_section_count: snapshot.preview_section_count,
        source_switch_count: snapshot.source_switch_count,
        first_world: snapshot.first.map(|submission| submission.world.get()),
        first_section: snapshot.first.map(section),
        last_world: snapshot.last.map(|submission| submission.world.get()),
        last_section: snapshot.last.map(section),
    }
}

fn validate_reversible_translucent_fixture_orders(
    preview_world: u64,
    front: &LiveDioramaTranslucentOrderReport,
    behind: &LiveDioramaTranslucentOrderReport,
    stereo: &LiveDioramaTranslucentOrderReport,
) -> Result<()> {
    for (label, order) in [("front", front), ("behind", behind)] {
        if order.active_section_count < 2
            || order.preview_section_count == 0
            || order.source_switch_count < 2
            || order.first_world == Some(preview_world)
            || order.last_world == Some(preview_world)
        {
            bail!(
                "{label} translucent order does not bracket B with distinct A sections: {order:?}"
            );
        }
    }
    if front.first_section != behind.last_section
        || front.last_section != behind.first_section
        || front.first_section == front.last_section
    {
        bail!(
            "front/behind translucent orders did not reverse A section endpoints: front={front:?} behind={behind:?}"
        );
    }
    if stereo.active_section_count == 0
        || stereo.preview_section_count == 0
        || stereo.source_switch_count == 0
    {
        bail!("stereo midpoint translucent order is incomplete: {stereo:?}");
    }
    Ok(())
}

fn differing_pixel_count(left: &Path, right: &Path) -> Result<usize> {
    let left = ImageReader::open(left)
        .with_context(|| format!("open image `{}`", left.display()))?
        .decode()
        .context("decode active-only image")?
        .to_rgba8();
    let right = ImageReader::open(right)
        .with_context(|| format!("open image `{}`", right.display()))?
        .decode()
        .context("decode composed image")?
        .to_rgba8();
    if left.dimensions() != right.dimensions() {
        bail!(
            "live-diorama comparison dimensions differ: {:?} versus {:?}",
            left.dimensions(),
            right.dimensions()
        );
    }
    Ok(left
        .pixels()
        .zip(right.pixels())
        .filter(|(left, right)| left != right)
        .count())
}
