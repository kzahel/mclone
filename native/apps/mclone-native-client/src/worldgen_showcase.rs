use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::prepared_assets::{
    AssetPackSourceRegistry, reference_asset_pack_selection,
};
use mclone_core::ChunkPos;
use mclone_render::chunk::ChunkCamera;
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_worldgen::levelgen::SMALL_ISLAND_SUPPORT_RADIUS;

use crate::camera::{SPECTATOR_BASE_SPEED, SpectatorCamera};
use crate::cli::{SceneOptions, WorldgenShowcaseOptions};
use crate::offscreen_flat_client::OffscreenFlatClientHost;
use crate::offscreen_scene_host::OffscreenWarmupReport;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, chunk_tracking_radius_for_render_distance};

const SHOWCASE_VIEW_COUNT: usize = 3;
const CARD_PADDING: u32 = 18;
const CARD_HEADER_HEIGHT: u32 = 82;
const CARD_LABEL_HEIGHT: u32 = 38;
const CARD_PANEL_GAP: u32 = 14;
const CARD_PANEL_BORDER: u32 = 2;

const CARD_BACKGROUND: [u8; 4] = [14, 20, 27, 255];
const CARD_PANEL_BACKGROUND: [u8; 4] = [27, 36, 46, 255];
const CARD_BORDER: [u8; 4] = [66, 82, 98, 255];
const CARD_ACCENT: [u8; 4] = [104, 196, 151, 255];
const CARD_TEXT: [u8; 4] = [236, 241, 245, 255];
const CARD_MUTED_TEXT: [u8; 4] = [174, 188, 200, 255];

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorldgenShowcaseView {
    slug: &'static str,
    label: &'static str,
    eye: Vec3,
    target: Vec3,
    fov_y_degrees: f32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldgenShowcasePanelReport {
    pub(crate) label: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) drawn_section_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldgenShowcaseReport {
    pub(crate) card_path: PathBuf,
    pub(crate) receipt_path: PathBuf,
    pub(crate) card_width: u32,
    pub(crate) card_height: u32,
    pub(crate) panel_width: u32,
    pub(crate) panel_height: u32,
    pub(crate) profile: &'static str,
    pub(crate) seed: i64,
    pub(crate) panels: Vec<WorldgenShowcasePanelReport>,
}

struct WorldgenShowcaseCaptureState {
    host: OffscreenFlatClientHost,
    views: [WorldgenShowcaseView; SHOWCASE_VIEW_COUNT],
    drawn_section_counts: Vec<usize>,
    readiness: WorldgenShowcaseReadiness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorldgenShowcaseReadiness {
    interest_center: ChunkPos,
    render_distance: u32,
    chunk_tracking_radius: u32,
    coverage_radius_blocks: u32,
    tracked_radius_blocks: u32,
    expected_visible_chunks: usize,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    target_ready_chunks: usize,
    target_chunk_count: usize,
    target_pending_stream_work: usize,
    target_pending_render_chunks: usize,
    target_ready_render_work_pending: bool,
    target_inflight_render_sections: usize,
    background_pending_generation_work: usize,
    background_pending_render_work: usize,
    resident_sections: usize,
    warmup_frames: usize,
    warmup_elapsed_ms: f64,
}

pub(crate) fn run_worldgen_showcase(
    options: &WorldgenShowcaseOptions,
) -> Result<WorldgenShowcaseReport> {
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create worldgen showcase directory {}",
            options.directory.display()
        )
    })?;

    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let initial_camera = SpectatorCamera::spawn_for_scene(&scene);
    let center_x = scene.chunk_x.saturating_mul(16) as f32;
    let center_z = scene.chunk_z.saturating_mul(16) as f32;

    let (loop_report, panel_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: SHOWCASE_VIEW_COUNT,
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
                initial_camera,
            )?;
            if let Some(registry) =
                AssetPackSourceRegistry::discover_native_with_reference(asset_source.clone())?
            {
                host.scene_host_mut()
                    .configure_asset_pack_sources(registry, reference_asset_pack_selection())?;
            }
            let warmup = host.drive_until_target_complete(device, queue)?;
            host.scene_host_mut().set_mono_ui_screen(None);
            if let Some(day_time) = scene.day_time_override {
                host.force_day_time(day_time);
            }
            let readiness = validate_showcase_readiness(&host, &scene, &warmup)?;
            let surface_y = host
                .scene_host()
                .mono_highest_non_air_block_y_at_world(center_x as i32, center_z as i32)
                .unwrap_or(64) as f32;
            Ok(WorldgenShowcaseCaptureState {
                host,
                views: showcase_views(center_x, surface_y, center_z),
                drawn_section_counts: Vec::with_capacity(SHOWCASE_VIEW_COUNT),
                readiness,
            })
        },
        |index, frame, state| {
            let view = state
                .views
                .get(index)
                .context("worldgen showcase requested an undeclared view")?;
            let camera = showcase_chunk_camera(view, state.readiness.render_distance)?;
            let summary = state.host.render_detached_chunk_camera(frame, camera)?;
            let current_center = state
                .host
                .scene_host()
                .runtime_stats()
                .context("worldgen showcase lost its runtime after warmup")?
                .interest_center;
            if current_center != state.readiness.interest_center {
                bail!(
                    "worldgen showcase detached camera moved interest from ({}, {}) to ({}, {})",
                    state.readiness.interest_center.x,
                    state.readiness.interest_center.z,
                    current_center.x,
                    current_center.z
                );
            }
            state.drawn_section_counts.push(summary.drawn_section_count);
            Ok(())
        },
    )?;

    if panel_pixels.len() != SHOWCASE_VIEW_COUNT
        || state.drawn_section_counts.len() != SHOWCASE_VIEW_COUNT
    {
        bail!(
            "worldgen showcase captured {} panels and {} summaries, expected {SHOWCASE_VIEW_COUNT}",
            panel_pixels.len(),
            state.drawn_section_counts.len()
        );
    }
    if let Some((_, view)) = state
        .views
        .iter()
        .enumerate()
        .find(|(index, _)| state.drawn_section_counts[*index] == 0)
    {
        bail!("worldgen showcase `{}` view drew no sections", view.label);
    }

    let profile = options.scene.world_generation_profile.label();
    let output_prefix = showcase_output_prefix(profile, options.scene.seed);
    let mut panels = Vec::with_capacity(SHOWCASE_VIEW_COUNT);
    for (((view, pixels), drawn_section_count), index) in state
        .views
        .iter()
        .zip(panel_pixels.iter())
        .zip(state.drawn_section_counts.iter().copied())
        .zip(0..)
    {
        let expected_len = (loop_report.width * loop_report.height * 4) as usize;
        if pixels.len() != expected_len {
            bail!(
                "worldgen showcase panel {index} has {} bytes, expected {expected_len}",
                pixels.len()
            );
        }
        let path = options
            .directory
            .join(format!("{output_prefix}-{}.png", view.slug));
        save_rgba_png(&path, loop_report.width, loop_report.height, pixels)?;
        panels.push(WorldgenShowcasePanelReport {
            label: view.label,
            path,
            drawn_section_count,
        });
    }

    let (card_width, card_height, card_pixels) = compose_showcase_card(
        &panel_pixels,
        loop_report.width,
        loop_report.height,
        profile,
        options.scene.seed,
        options.scene.render_distance,
        options.scene.day_time_override,
        &state.views,
    )?;
    let card_path = options.directory.join(format!("{output_prefix}-card.png"));
    save_rgba_png(&card_path, card_width, card_height, &card_pixels)?;

    let receipt_path = options.directory.join(format!("{output_prefix}-card.json"));
    let (commit, dirty) = git_state();
    let profile_coverage = if options.scene.world_generation_profile
        == mclone_server::WorldGenerationProfile::SmallIslandV1
    {
        serde_json::json!({
            "kind": "bounded-small-island",
            "supportRadiusBlocks": SMALL_ISLAND_SUPPORT_RADIUS,
            "waterMarginBlocks": f64::from(state.readiness.coverage_radius_blocks)
                - SMALL_ISLAND_SUPPORT_RADIUS,
        })
    } else {
        serde_json::json!({ "kind": "unbounded" })
    };
    let receipt = serde_json::json!({
        "schema": 4,
        "profile": profile,
        "commit": commit,
        "dirty": dirty,
        "seed": options.scene.seed,
        "centerChunk": [options.scene.chunk_x, options.scene.chunk_z],
        "renderDistance": options.scene.render_distance,
        "dayTime": options.scene.day_time_override,
        "panelDimensions": [loop_report.width, loop_report.height],
        "cardDimensions": [card_width, card_height],
        "cardPath": card_path,
        "interest": {
            "centerChunk": [state.readiness.interest_center.x, state.readiness.interest_center.z],
            "renderDistance": state.readiness.render_distance,
            "chunkTrackingRadius": state.readiness.chunk_tracking_radius,
            "coverageRadiusBlocks": state.readiness.coverage_radius_blocks,
            "trackedRadiusBlocks": state.readiness.tracked_radius_blocks,
            "expectedVisibleChunks": state.readiness.expected_visible_chunks,
            "loadedChunks": state.readiness.loaded_chunks,
            "clientVisibleChunks": state.readiness.client_visible_chunks,
            "targetReadyChunks": state.readiness.target_ready_chunks,
            "targetChunkCount": state.readiness.target_chunk_count,
            "targetPendingStreamWork": state.readiness.target_pending_stream_work,
            "targetPendingRenderChunks": state.readiness.target_pending_render_chunks,
            "targetReadyRenderWorkPending": state.readiness.target_ready_render_work_pending,
            "targetInflightRenderSections": state.readiness.target_inflight_render_sections,
            "residentSections": state.readiness.resident_sections,
            "warmupFrames": state.readiness.warmup_frames,
            "warmupElapsedMs": state.readiness.warmup_elapsed_ms,
            "profileCoverage": profile_coverage,
        },
        "backgroundWorkOutsideCaptureTarget": {
            "pendingGeneration": state.readiness.background_pending_generation_work,
            "pendingRender": state.readiness.background_pending_render_work,
        },
        "views": state.views.iter().zip(&panels).map(|(view, panel)| serde_json::json!({
            "label": view.label,
            "path": panel.path,
            "eye": view.eye.to_array(),
            "target": view.target.to_array(),
            "fovYDegrees": view.fov_y_degrees,
            "drawnSectionCount": panel.drawn_section_count,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("write worldgen showcase receipt {}", receipt_path.display()))?;

    Ok(WorldgenShowcaseReport {
        card_path,
        receipt_path,
        card_width,
        card_height,
        panel_width: loop_report.width,
        panel_height: loop_report.height,
        profile,
        seed: options.scene.seed,
        panels,
    })
}

fn showcase_views(center_x: f32, surface_y: f32, center_z: f32) -> [WorldgenShowcaseView; 3] {
    [
        WorldgenShowcaseView {
            slug: "top",
            label: "TOP DOWN",
            eye: Vec3::new(center_x + 0.35, surface_y + 315.0, center_z - 0.35),
            target: Vec3::new(center_x - 0.35, surface_y, center_z + 0.35),
            fov_y_degrees: 42.0,
        },
        WorldgenShowcaseView {
            slug: "coast",
            label: "LOW LANDSCAPE",
            eye: Vec3::new(center_x, surface_y + 8.0, center_z - 200.0),
            target: Vec3::new(center_x, surface_y - 6.0, center_z),
            fov_y_degrees: 48.0,
        },
        WorldgenShowcaseView {
            slug: "elevated",
            label: "ELEVATED",
            eye: Vec3::new(center_x + 196.0, surface_y + 100.0, center_z),
            target: Vec3::new(center_x, surface_y - 30.0, center_z),
            fov_y_degrees: 38.0,
        },
    ]
}

fn git_state() -> (Option<String>, Option<bool>) {
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    (commit, dirty)
}

fn showcase_chunk_camera(view: &WorldgenShowcaseView, render_distance: u32) -> Result<ChunkCamera> {
    let direction = (view.target - view.eye)
        .try_normalize()
        .context("worldgen showcase view has identical eye and target")?;
    let mut camera = SpectatorCamera {
        position: view.eye,
        yaw: direction.x.atan2(direction.z),
        pitch: direction.y.clamp(-1.0, 1.0).asin(),
        speed: SPECTATOR_BASE_SPEED,
    }
    .camera(render_distance);
    camera.fov_y_radians = view.fov_y_degrees.to_radians();
    Ok(camera)
}

fn validate_showcase_readiness(
    host: &OffscreenFlatClientHost,
    scene: &SceneOptions,
    warmup: &OffscreenWarmupReport,
) -> Result<WorldgenShowcaseReadiness> {
    let runtime = host
        .scene_host()
        .runtime_stats()
        .context("worldgen showcase reached idle without an active runtime")?;
    let expected_center = ChunkPos::new(scene.chunk_x, scene.chunk_z);
    if runtime.interest_center != expected_center {
        bail!(
            "worldgen showcase interest settled at ({}, {}), expected ({}, {})",
            runtime.interest_center.x,
            runtime.interest_center.z,
            expected_center.x,
            expected_center.z
        );
    }
    if runtime.render_distance != scene.render_distance {
        bail!(
            "worldgen showcase runtime render distance {} differs from requested {}",
            runtime.render_distance,
            scene.render_distance
        );
    }
    let expected_tracking_radius = chunk_tracking_radius_for_render_distance(scene.render_distance);
    if runtime.chunk_tracking_radius != expected_tracking_radius {
        bail!(
            "worldgen showcase runtime tracking radius {} differs from expected {}",
            runtime.chunk_tracking_radius,
            expected_tracking_radius
        );
    }
    let expected_visible_chunks = square_chunk_count(expected_tracking_radius)?;
    if runtime.client_visible_chunks != expected_visible_chunks
        || runtime.loaded_chunks < expected_visible_chunks
    {
        bail!(
            "worldgen showcase centered region is incomplete: visible={}/{} loaded={}",
            runtime.client_visible_chunks,
            expected_visible_chunks,
            runtime.loaded_chunks
        );
    }
    let view_progress = host
        .scene_host()
        .mono_view_readiness_overlay()
        .context("worldgen showcase full-view readiness is unavailable")?;
    if view_progress.target_chunk_count != expected_visible_chunks
        || view_progress.target_ready_chunks != expected_visible_chunks
    {
        bail!(
            "worldgen showcase full-view readiness is incomplete: ready={}/{} expected={}",
            view_progress.target_ready_chunks,
            view_progress.target_chunk_count,
            expected_visible_chunks
        );
    }
    let background_pending_generation_work = runtime.pending_jobs
        + runtime.pending_publications
        + runtime.scheduler_pending_worldgen_publication_chunks
        + runtime.scheduler_pending_light_publications
        + runtime.pending_persistence_loads
        + runtime.pending_persistence_saves;
    let background_pending_render_work = runtime.pending_render_chunks
        + runtime.pending_render_compile_jobs
        + runtime.inflight_render_sections;
    let target_pending_stream_work = host.pending_stream_work();
    let target_render = host.target_render_work_stats();
    if target_pending_stream_work != 0
        || target_render.pending_render_chunks != 0
        || target_render.ready_render_work_pending
        || target_render.inflight_render_sections != 0
    {
        bail!(
            "worldgen showcase target render is incomplete: stream={} chunks={} ready={} inflight={}",
            target_pending_stream_work,
            target_render.pending_render_chunks,
            target_render.ready_render_work_pending,
            target_render.inflight_render_sections
        );
    }
    let render = host.scene_host().render_stats();
    if render.section_count == 0 || render.last_pending_compile_jobs != 0 {
        bail!(
            "worldgen showcase centered region has incomplete render residency: sections={} pending_compiles={}",
            render.section_count,
            render.last_pending_compile_jobs
        );
    }
    let coverage_radius_blocks = scene
        .render_distance
        .checked_mul(16)
        .context("worldgen showcase coverage radius overflow")?;
    let tracked_radius_blocks = expected_tracking_radius
        .checked_mul(16)
        .context("worldgen showcase tracked radius overflow")?;

    Ok(WorldgenShowcaseReadiness {
        interest_center: expected_center,
        render_distance: runtime.render_distance,
        chunk_tracking_radius: runtime.chunk_tracking_radius,
        coverage_radius_blocks,
        tracked_radius_blocks,
        expected_visible_chunks,
        loaded_chunks: runtime.loaded_chunks,
        client_visible_chunks: runtime.client_visible_chunks,
        target_ready_chunks: view_progress.target_ready_chunks,
        target_chunk_count: view_progress.target_chunk_count,
        target_pending_stream_work,
        target_pending_render_chunks: target_render.pending_render_chunks,
        target_ready_render_work_pending: target_render.ready_render_work_pending,
        target_inflight_render_sections: target_render.inflight_render_sections,
        background_pending_generation_work,
        background_pending_render_work,
        resident_sections: render.section_count,
        warmup_frames: warmup.frame_count,
        warmup_elapsed_ms: warmup.elapsed_ms,
    })
}

fn square_chunk_count(radius: u32) -> Result<usize> {
    let diameter = u64::from(radius)
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .context("worldgen showcase chunk diameter overflow")?;
    usize::try_from(
        diameter
            .checked_mul(diameter)
            .context("worldgen showcase chunk count overflow")?,
    )
    .context("worldgen showcase chunk count does not fit usize")
}

#[allow(clippy::too_many_arguments)]
fn compose_showcase_card(
    panels: &[Vec<u8>],
    panel_width: u32,
    panel_height: u32,
    profile: &str,
    seed: i64,
    render_distance: u32,
    day_time: Option<u64>,
    views: &[WorldgenShowcaseView],
) -> Result<(u32, u32, Vec<u8>)> {
    if panels.len() != views.len() || panels.is_empty() {
        bail!(
            "worldgen showcase card requires matching non-empty panels and views, got {} and {}",
            panels.len(),
            views.len()
        );
    }
    let view_count = u32::try_from(views.len()).context("worldgen showcase view count overflow")?;
    let card_width = CARD_PADDING
        .checked_mul(2)
        .and_then(|value| value.checked_add(panel_width.checked_mul(view_count)?))
        .and_then(|value| {
            value.checked_add(CARD_PANEL_GAP.checked_mul(view_count.saturating_sub(1))?)
        })
        .context("worldgen showcase card width overflow")?;
    let card_height = CARD_PADDING
        .checked_mul(2)
        .and_then(|value| value.checked_add(CARD_HEADER_HEIGHT))
        .and_then(|value| value.checked_add(panel_height))
        .and_then(|value| value.checked_add(CARD_LABEL_HEIGHT))
        .context("worldgen showcase card height overflow")?;
    let mut card = vec![0; (card_width * card_height * 4) as usize];
    fill_rect(
        &mut card,
        card_width,
        card_height,
        0,
        0,
        card_width,
        card_height,
        CARD_BACKGROUND,
    );
    draw_text(
        &mut card,
        card_width,
        card_height,
        "WORLDGEN SHOWCASE",
        CARD_PADDING,
        CARD_PADDING,
        4,
        CARD_TEXT,
    );
    let detail = format!(
        "{}  SEED {}  DAY {}  RD {}",
        profile,
        seed,
        day_time
            .map(|value| value.to_string())
            .unwrap_or_else(|| "LIVE".to_owned()),
        render_distance
    );
    draw_text(
        &mut card,
        card_width,
        card_height,
        &detail,
        CARD_PADDING,
        CARD_PADDING + 40,
        2,
        CARD_MUTED_TEXT,
    );
    fill_rect(
        &mut card,
        card_width,
        card_height,
        CARD_PADDING,
        CARD_PADDING + CARD_HEADER_HEIGHT - 5,
        card_width - CARD_PADDING * 2,
        2,
        CARD_ACCENT,
    );

    let panel_y = CARD_PADDING + CARD_HEADER_HEIGHT;
    for (index, (panel, view)) in panels.iter().zip(views).enumerate() {
        let x = CARD_PADDING + index as u32 * (panel_width + CARD_PANEL_GAP);
        fill_rect(
            &mut card,
            card_width,
            card_height,
            x.saturating_sub(CARD_PANEL_BORDER),
            panel_y.saturating_sub(CARD_PANEL_BORDER),
            panel_width + CARD_PANEL_BORDER * 2,
            panel_height + CARD_LABEL_HEIGHT + CARD_PANEL_BORDER * 2,
            CARD_BORDER,
        );
        blit_rgba(
            &mut card,
            card_width,
            card_height,
            panel,
            panel_width,
            panel_height,
            x,
            panel_y,
        )?;
        fill_rect(
            &mut card,
            card_width,
            card_height,
            x,
            panel_y + panel_height,
            panel_width,
            CARD_LABEL_HEIGHT,
            CARD_PANEL_BACKGROUND,
        );
        draw_text(
            &mut card,
            card_width,
            card_height,
            view.label,
            x + 12,
            panel_y + panel_height + 8,
            3,
            CARD_TEXT,
        );
    }

    Ok((card_width, card_height, card))
}

#[allow(clippy::too_many_arguments)]
fn blit_rgba(
    destination: &mut [u8],
    destination_width: u32,
    destination_height: u32,
    source: &[u8],
    source_width: u32,
    source_height: u32,
    x: u32,
    y: u32,
) -> Result<()> {
    let expected_source_len = (source_width * source_height * 4) as usize;
    if source.len() != expected_source_len {
        bail!(
            "RGBA source has {} bytes, expected {expected_source_len}",
            source.len()
        );
    }
    if x + source_width > destination_width || y + source_height > destination_height {
        bail!("RGBA source does not fit in destination");
    }
    let source_row_bytes = (source_width * 4) as usize;
    let destination_row_bytes = (destination_width * 4) as usize;
    for row in 0..source_height as usize {
        let source_start = row * source_row_bytes;
        let destination_start = (y as usize + row) * destination_row_bytes + x as usize * 4;
        destination[destination_start..destination_start + source_row_bytes]
            .copy_from_slice(&source[source_start..source_start + source_row_bytes]);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    color: [u8; 4],
) {
    let max_x = x.saturating_add(rect_width).min(width);
    let max_y = y.saturating_add(rect_height).min(height);
    for pixel_y in y.min(height)..max_y {
        for pixel_x in x.min(width)..max_x {
            let offset = ((pixel_y * width + pixel_x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    text: &str,
    x: u32,
    y: u32,
    scale: u32,
    color: [u8; 4],
) {
    let mut cursor_x = x;
    for character in text.chars() {
        if character == ' ' {
            cursor_x = cursor_x.saturating_add(6 * scale);
            continue;
        }
        let rows = mclone_ui::Font::glyph_rows(character);
        for (row, bits) in rows.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        cursor_x + column * scale,
                        y + row as u32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cursor_x = cursor_x.saturating_add(6 * scale);
    }
}

fn showcase_output_prefix(profile: &str, seed: i64) -> String {
    let seed = if seed < 0 {
        format!("neg{}", seed.unsigned_abs())
    } else {
        seed.to_string()
    };
    format!("{profile}-seed-{seed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_prefix_is_stable_for_positive_and_negative_seeds() {
        assert_eq!(
            showcase_output_prefix("small-island-v1", 12345),
            "small-island-v1-seed-12345"
        );
        assert_eq!(
            showcase_output_prefix("small-island-v1", -42),
            "small-island-v1-seed-neg42"
        );
        assert_eq!(
            showcase_output_prefix("small-island-v1", i64::MIN),
            "small-island-v1-seed-neg9223372036854775808"
        );
    }

    #[test]
    fn card_composition_keeps_each_capture_in_its_panel() {
        let width = 4;
        let height = 3;
        let panels = [[200, 10, 10, 255], [10, 200, 10, 255], [10, 10, 200, 255]]
            .into_iter()
            .map(|color| color.repeat((width * height) as usize))
            .collect::<Vec<_>>();
        let views = showcase_views(0.0, 64.0, 0.0);

        let (card_width, card_height, card) = compose_showcase_card(
            &panels,
            width,
            height,
            "small-island-v1",
            7,
            6,
            Some(6000),
            &views,
        )
        .unwrap();

        assert_eq!(
            card_width,
            CARD_PADDING * 2 + width * 3 + CARD_PANEL_GAP * 2
        );
        assert_eq!(
            card_height,
            CARD_PADDING * 2 + CARD_HEADER_HEIGHT + height + CARD_LABEL_HEIGHT
        );
        let panel_y = CARD_PADDING + CARD_HEADER_HEIGHT;
        for (index, expected) in [[200, 10, 10, 255], [10, 200, 10, 255], [10, 10, 200, 255]]
            .into_iter()
            .enumerate()
        {
            let x = CARD_PADDING + index as u32 * (width + CARD_PANEL_GAP);
            let offset = ((panel_y * card_width + x) * 4) as usize;
            assert_eq!(&card[offset..offset + 4], &expected);
        }
    }

    #[test]
    fn showcase_views_include_near_vertical_and_two_landscape_angles() {
        let views = showcase_views(32.0, 70.0, -16.0);
        assert_eq!(views.map(|view| view.slug), ["top", "coast", "elevated"]);
        assert!(views[0].eye.y - views[0].target.y > 300.0);
        assert!(views[1].eye.y - views[1].target.y < 30.0);
        assert!(views[2].eye.y - views[2].target.y > 100.0);
        assert_eq!(views.map(|view| view.fov_y_degrees), [42.0, 48.0, 38.0]);
        for view in &views[1..] {
            let offset = view.eye - view.target;
            let horizontal_distance = offset.x.hypot(offset.z);
            assert!(horizontal_distance > SMALL_ISLAND_SUPPORT_RADIUS as f32);
        }
    }

    #[test]
    fn square_chunk_counts_cover_render_and_tracking_regions() {
        assert_eq!(square_chunk_count(15).unwrap(), 961);
        assert_eq!(square_chunk_count(16).unwrap(), 1_089);
        assert_eq!(square_chunk_count(17).unwrap(), 1_225);
    }
}
