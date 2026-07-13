use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use image::ImageReader;
use mclone_render_session::EngineCameraViewMode;
use mclone_scene::EmbeddedWorldPreviewPhase;
use serde::Serialize;

use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, LiveDioramaSmokeOptions, StartupWaitPolicy,
    XrEmulationScreenshotOptions,
};
use crate::offscreen_flat_client::{
    OffscreenFlatClientScreenshotReport, run_offscreen_flat_client_screenshot,
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
    pub(crate) active_preview_pixel_difference_count: usize,
    pub(crate) preview_bounded_section_count: usize,
    pub(crate) preview_drawn_section_count: usize,
    pub(crate) preview_drawn_index_count: u32,
    pub(crate) stereo_eye_pixel_difference_count: usize,
    pub(crate) standby_cadence_hz: [u32; 3],
    pub(crate) standby_cadence_applied: bool,
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
        anchor.z as f32 - 6.0,
    ];
    let side_eye = [
        anchor.x as f32 - 6.0,
        anchor.y as f32 + 2.0,
        anchor.z as f32,
    ];
    let behind_eye = [
        anchor.x as f32,
        anchor.y as f32 + 2.0,
        anchor.z as f32 + 6.0,
    ];

    let active_only_path = options.directory.join("a-alone.png");
    let front_path = options.directory.join("a-plus-b-front.png");
    let side_path = options.directory.join("a-plus-b-side.png");
    let behind_path = options.directory.join("a-plus-b-behind.png");
    let stereo_path = options.directory.join("a-plus-b-stereo.png");
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

    let active_preview_pixel_difference_count =
        differing_pixel_count(&active_only_path, &front_path)?;
    if active_preview_pixel_difference_count == 0 {
        bail!("A-only and A+B captures are pixel-identical");
    }

    let cadence = standby.standby_cadence;
    let report = LiveDioramaSmokeReport {
        directory: options.directory.clone(),
        report_path,
        active_only_path,
        front_path,
        side_path,
        behind_path,
        stereo_path,
        active_preview_pixel_difference_count,
        preview_bounded_section_count: preview.bounded_section_count,
        preview_drawn_section_count: preview.last_drawn_section_count,
        preview_drawn_index_count: preview.last_drawn_index_count,
        stereo_eye_pixel_difference_count: stereo.eye_pixel_difference_count,
        standby_cadence_hz: [
            cadence.host_rate_hz,
            cadence.gameplay_rate_hz,
            cadence.physics_rate_hz,
        ],
        standby_cadence_applied: standby.standby_cadence_applied,
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
