use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use glam::{Vec3, Vec4};
use mclone_app_runtime::frame_render::{
    FlatRenderResources, FullFrameGui, FullFrameRenderSummary, RenderStreamStats,
    record_render_section_update_stats,
};
use mclone_app_runtime::native_remote_session::NativeRemoteServerSession;
use mclone_app_runtime::native_service_assembly::NativeSceneServices;
use mclone_client::ActorInterpolationState;
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::entity::{ActorDrawResources, ActorInstance, ActorRenderStats};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_render::screen_effect::UnderwaterOverlay;
use mclone_render_session::actor_instances_from_presentations;
use mclone_ui::{GameUiHost, GuiDrawList, GuiScale};

use crate::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessActorReviewSheetOptions, HeadlessActorWalkReviewOptions, HeadlessDualViewOptions,
    HeadlessScreenshotOptions, RendererRebuildSmokeOptions,
};
use crate::offscreen_scene_host::OffscreenDriver;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{
    WindowSceneAssets, native_window_scene_runtime_with_mesh_assets, poll_window_runtime_until_idle,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) gui_command_count: usize,
    pub(crate) flat_hud_retained_rebuild_count: u64,
    pub(crate) flat_hud_retained_cache_hit_count: u64,
    pub(crate) remote_player_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
    pub(crate) far_lod_region_draw_count: usize,
    pub(crate) far_lod_uploaded_bytes: usize,
    pub(crate) underwater: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessDualViewReport {
    pub(crate) view_name: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) non_clear_rgb_pixel_count: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) gui_command_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessActorReviewSheetReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) view_count: usize,
    pub(crate) non_clear_rgb_pixel_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessActorWalkReviewReport {
    pub(crate) sheet_path: PathBuf,
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) frame_width: u32,
    pub(crate) frame_height: u32,
    pub(crate) sheet_width: u32,
    pub(crate) frame_count: usize,
    pub(crate) fps: u32,
    pub(crate) cycles: f32,
    pub(crate) sheet_byte_len: usize,
    pub(crate) video_byte_len: Option<u64>,
    pub(crate) non_clear_rgb_pixel_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RendererRebuildSmokeReport {
    pub(crate) before_path: PathBuf,
    pub(crate) after_path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) before_non_clear_rgb_pixel_count: usize,
    pub(crate) after_non_clear_rgb_pixel_count: usize,
    pub(crate) pixel_mismatch_count: usize,
    pub(crate) reuploaded_section_count: usize,
    pub(crate) state_preserved: bool,
    pub(crate) config_changed: bool,
    pub(crate) before_render_size: [u32; 2],
    pub(crate) after_render_size: [u32; 2],
}

#[derive(Clone, Debug, PartialEq)]
struct RendererRebuildSmokePreservedState {
    interest_center: mclone_core::ChunkPos,
    render_distance: u32,
    chunk_tracking_radius: u32,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    tracked_players: usize,
    player_visible_chunks: usize,
    camera: ChunkCamera,
    ui_active: bool,
    ui_covers_world: bool,
    gui_scale: [f32; 2],
}

impl RendererRebuildSmokePreservedState {
    fn capture(
        runtime: &NativeSceneServices<NativeRemoteServerSession>,
        camera: ChunkCamera,
        ui: &GameUiHost,
    ) -> Self {
        let runtime = runtime.stats();
        let scale = ui.scale();
        Self {
            interest_center: runtime.interest_center,
            render_distance: runtime.render_distance,
            chunk_tracking_radius: runtime.chunk_tracking_radius,
            loaded_chunks: runtime.loaded_chunks,
            client_visible_chunks: runtime.client_visible_chunks,
            tracked_players: runtime.tracked_players,
            player_visible_chunks: runtime.player_visible_chunks,
            camera,
            ui_active: ui.is_active(),
            ui_covers_world: ui.covers_world(),
            gui_scale: [scale.width, scale.height],
        }
    }
}

struct RendererRebuildSmokeState {
    runtime: NativeSceneServices<NativeRemoteServerSession>,
    actor_textures: ActorTextureAssets,
    resources: FlatRenderResources,
    asset_source: mclone_assets::AssetSourceChain,
    sections: Vec<mclone_mesh::TexturedRenderSectionMesh>,
    camera: ChunkCamera,
    ui: GameUiHost,
    actor_interpolation: ActorInterpolationState,
    render_stats: RenderStreamStats,
    render_options: TexturedSectionRenderOptions,
    next_render_config: Option<RenderConfig>,
    config_changed: bool,
    before_render_size: [u32; 2],
    after_render_size: [u32; 2],
    reuploaded_section_count: usize,
    state_preserved: bool,
    summary: Option<FullFrameRenderSummary>,
}

struct ActorReviewSheetState {
    actors: ActorDrawResources,
    stats: Vec<ActorRenderStats>,
    render_options: TexturedSectionRenderOptions,
}

pub(crate) fn write_actor_review_sheet(
    options: &HeadlessActorReviewSheetOptions,
) -> Result<HeadlessActorReviewSheetReport> {
    let view_count = actor_review_views().len();
    let panel_width = (options.width.max(view_count as u32) / view_count as u32).max(1);
    let panel_height = options.height.max(1);
    let sheet_width = panel_width * view_count as u32;
    let mut render_options = options.render_options;
    render_options.force_fullbright = true;
    let actor_assets = load_actor_texture_assets()
        .context("failed to load actor assets for actor review sheet")?;

    let (loop_report, panel_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: panel_width,
            height: panel_height,
            frame_count: view_count,
            pace_frame_duration: None,
        },
        move |device, queue, format, _size| {
            Ok(ActorReviewSheetState {
                actors: ActorDrawResources::new(
                    device,
                    queue,
                    format,
                    actor_assets.atlas.as_upload(),
                    Some(&actor_assets.figures),
                )?,
                stats: Vec::with_capacity(view_count),
                render_options,
            })
        },
        |index, frame, state| render_actor_review_panel(index, frame, state),
    )?;
    if panel_pixels.len() != view_count {
        bail!(
            "actor review rendered {} panels but expected {view_count}",
            panel_pixels.len()
        );
    }

    let sheet_pixels = stitch_actor_review_panels(&panel_pixels, panel_width, panel_height);
    save_rgba_png(&options.path, sheet_width, panel_height, &sheet_pixels)?;
    Ok(HeadlessActorReviewSheetReport {
        path: options.path.clone(),
        width: sheet_width,
        height: panel_height,
        byte_len: sheet_pixels.len(),
        view_count: loop_report.frame_count,
        non_clear_rgb_pixel_count: non_clear_rgb_pixel_count(&sheet_pixels),
        actor_count: state
            .stats
            .iter()
            .map(|stats| stats.submitted_actor_count)
            .sum(),
        drawn_actor_count: state
            .stats
            .iter()
            .map(|stats| stats.drawn_actor_count)
            .sum(),
    })
}

pub(crate) fn write_actor_walk_review(
    options: &HeadlessActorWalkReviewOptions,
) -> Result<HeadlessActorWalkReviewReport> {
    let frame_width = options.width.max(1);
    let frame_height = options.height.max(1);
    let frame_count = options.frames.max(1);
    let sheet_width = frame_width
        .checked_mul(frame_count as u32)
        .context("actor walk review sheet width overflowed")?;
    let mut render_options = options.render_options;
    render_options.force_fullbright = true;
    let actor_assets =
        load_actor_texture_assets().context("failed to load actor assets for walk review")?;
    let asset_source =
        load_asset_source().context("failed to load asset source for walk review")?;
    let specs = actor_walk_review_specs(&asset_source)?;

    let (loop_report, mut frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: frame_width,
            height: frame_height,
            frame_count,
            pace_frame_duration: None,
        },
        move |device, queue, format, _size| {
            Ok(ActorReviewSheetState {
                actors: ActorDrawResources::new(
                    device,
                    queue,
                    format,
                    actor_assets.atlas.as_upload(),
                    Some(&actor_assets.figures),
                )?,
                stats: Vec::with_capacity(frame_count),
                render_options,
            })
        },
        |index, frame, state| {
            render_actor_walk_review_frame(index, frame, state, &specs, frame_count, options.cycles)
        },
    )?;
    if frame_pixels.len() != frame_count {
        bail!(
            "actor walk review rendered {} frames but expected {frame_count}",
            frame_pixels.len()
        );
    }

    for pixels in &mut frame_pixels {
        draw_actor_walk_review_overlay(
            pixels,
            frame_width,
            frame_height,
            actor_review_side_camera(),
        );
    }

    let sheet_pixels = stitch_actor_review_panels(&frame_pixels, frame_width, frame_height);
    save_rgba_png(
        &options.sheet_path,
        sheet_width,
        frame_height,
        &sheet_pixels,
    )?;
    let video_byte_len = options
        .video_path
        .as_deref()
        .map(|path| {
            write_actor_walk_review_video(
                path,
                &frame_pixels,
                frame_width,
                frame_height,
                options.fps,
            )
        })
        .transpose()?;

    Ok(HeadlessActorWalkReviewReport {
        sheet_path: options.sheet_path.clone(),
        video_path: options.video_path.clone(),
        frame_width,
        frame_height,
        sheet_width,
        frame_count: loop_report.frame_count,
        fps: options.fps,
        cycles: options.cycles,
        sheet_byte_len: sheet_pixels.len(),
        video_byte_len,
        non_clear_rgb_pixel_count: non_clear_rgb_pixel_count(&sheet_pixels),
        actor_count: state
            .stats
            .iter()
            .map(|stats| stats.submitted_actor_count)
            .sum(),
        drawn_actor_count: state
            .stats
            .iter()
            .map(|stats| stats.drawn_actor_count)
            .sum(),
    })
}

fn render_actor_walk_review_frame(
    index: usize,
    frame: mclone_render::target::RenderFrameContext<'_>,
    state: &mut ActorReviewSheetState,
    specs: &[ActorWalkReviewFigureSpec],
    frame_count: usize,
    cycles: f32,
) -> Result<()> {
    let depth = ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
    clear_actor_review_frame(frame.encoder, frame.target, &depth);
    let phase = actor_walk_review_phase(index, frame_count, cycles);
    let actors = actor_walk_review_actors(specs, phase);
    let stats = state.actors.render(
        frame.device,
        frame.queue,
        frame.encoder,
        frame.target.with_depth(&depth.view),
        actor_review_side_camera().render_view(frame.target.size[0], frame.target.size[1]),
        state.render_options,
        &actors,
    )?;
    state.stats.push(stats);
    Ok(())
}

fn render_actor_review_panel(
    index: usize,
    frame: mclone_render::target::RenderFrameContext<'_>,
    state: &mut ActorReviewSheetState,
) -> Result<()> {
    let depth = ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
    clear_actor_review_frame(frame.encoder, frame.target, &depth);
    let views = actor_review_views();
    let view = views
        .get(index)
        .context("actor review panel index out of range")?;
    let actors = actor_review_actors(view.name);
    let stats = state.actors.render(
        frame.device,
        frame.queue,
        frame.encoder,
        frame.target.with_depth(&depth.view),
        view.camera
            .render_view(frame.target.size[0], frame.target.size[1]),
        state.render_options,
        &actors,
    )?;
    state.stats.push(stats);
    Ok(())
}

fn clear_actor_review_frame(
    encoder: &mut wgpu::CommandEncoder,
    target: mclone_render::target::RenderFrameTarget<'_>,
    depth: &ChunkDepthTarget,
) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mclone_actor_review_clear_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target.color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.62,
                    g: 0.66,
                    b: 0.70,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth.view,
            depth_ops: Some(wgpu::Operations {
                // Reversed-Z: far plane is 0.0 (see tactical 158).
                load: wgpu::LoadOp::Clear(mclone_render::chunk::REVERSED_Z_DEPTH_CLEAR),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        ..Default::default()
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorReviewView {
    name: &'static str,
    camera: ChunkCamera,
}

fn actor_review_views() -> [ActorReviewView; 3] {
    [
        ActorReviewView {
            name: "front",
            camera: actor_review_camera(Vec3::new(0.0, 1.0, 5.2)),
        },
        ActorReviewView {
            name: "side",
            camera: actor_review_side_camera(),
        },
        ActorReviewView {
            name: "three_quarter",
            camera: actor_review_camera(Vec3::new(4.0, 1.1, 4.0)),
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorWalkReviewFigureSpec {
    figure: mclone_assets::ActorFigureId,
    cycle_distance: f32,
}

fn actor_walk_review_specs(
    source: &impl mclone_assets::AssetSource,
) -> Result<Vec<ActorWalkReviewFigureSpec>> {
    mclone_assets::FIRST_PARTY_ACTOR_FIGURE_IDS
        .iter()
        .map(|figure| {
            let path = mclone_assets::actor_figure_path(*figure)
                .with_context(|| format!("unknown first-party actor figure {}", figure.as_str()))?;
            let asset = mclone_assets::load_figure_asset(source, &path)
                .with_context(|| format!("failed to load actor figure {}", figure.as_str()))?;
            let walk = asset
                .clips
                .get("walk")
                .with_context(|| format!("actor figure {} has no walk clip", figure.as_str()))?;
            let locomotion = walk.locomotion.as_ref().with_context(|| {
                format!(
                    "actor figure {} walk clip has no locomotion metadata",
                    figure.as_str()
                )
            })?;
            if !locomotion.cycle_distance.is_finite() || locomotion.cycle_distance <= 0.0 {
                bail!(
                    "actor figure {} walk cycleDistance must be positive",
                    figure.as_str()
                );
            }
            Ok(ActorWalkReviewFigureSpec {
                figure: *figure,
                cycle_distance: locomotion.cycle_distance,
            })
        })
        .collect()
}

fn actor_walk_review_phase(index: usize, frame_count: usize, cycles: f32) -> f32 {
    (index as f32 / frame_count.max(1) as f32) * cycles.max(0.0)
}

fn actor_walk_review_actors(
    specs: &[ActorWalkReviewFigureSpec],
    phase_cycles: f32,
) -> Vec<ActorInstance> {
    let spacing = 0.9;
    let center = (specs.len().saturating_sub(1)) as f32 * spacing * 0.5;
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let offset = index as f32 * spacing - center;
            let mut actor = ActorInstance::local_player_with_figure(
                Vec3::new(0.0, 0.0, offset),
                0.0,
                spec.figure,
            )
            .with_walk_animation_distance(phase_cycles * spec.cycle_distance);
            if spec.figure == mclone_assets::chicken_figure_id() {
                actor = actor.with_chicken_wing_flap_radians(Some(0.45));
            }
            actor
        })
        .collect()
}

fn actor_review_actors(view_name: &str) -> Vec<ActorInstance> {
    let figure_count = mclone_assets::FIRST_PARTY_ACTOR_FIGURE_IDS.len();
    let count = figure_count + 1;
    let spacing = 0.75;
    let center = (count.saturating_sub(1)) as f32 * spacing * 0.5;
    (0..count)
        .map(|index| {
            let offset = index as f32 * spacing - center;
            let position = match view_name {
                "side" => Vec3::new(0.0, 0.0, offset),
                _ => Vec3::new(offset, 0.0, 0.0),
            };
            if index < figure_count {
                ActorInstance::local_player_with_figure(
                    position,
                    0.0,
                    mclone_assets::FIRST_PARTY_ACTOR_FIGURE_IDS[index],
                )
            } else {
                ActorInstance::item_egg(position, 0.0, 0.25, 0.25)
            }
        })
        .collect()
}

fn actor_review_side_camera() -> ChunkCamera {
    actor_review_camera(Vec3::new(5.2, 1.0, 0.0))
}

fn actor_review_camera(eye: Vec3) -> ChunkCamera {
    ChunkCamera {
        eye: eye.to_array(),
        target: [0.0, 0.92, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 28.0_f32.to_radians(),
        z_near: 0.05,
        z_far: 16.0,
    }
}

fn stitch_actor_review_panels(panel_pixels: &[Vec<u8>], panel_width: u32, height: u32) -> Vec<u8> {
    let view_count = panel_pixels.len();
    let sheet_width = panel_width * view_count as u32;
    let mut sheet = vec![0; (sheet_width * height * 4) as usize];
    let panel_row_bytes = (panel_width * 4) as usize;
    let sheet_row_bytes = (sheet_width * 4) as usize;
    for (panel_index, panel) in panel_pixels.iter().enumerate() {
        for row in 0..height as usize {
            let source_start = row * panel_row_bytes;
            let source_end = source_start + panel_row_bytes;
            let dest_start = row * sheet_row_bytes + panel_index * panel_row_bytes;
            let dest_end = dest_start + panel_row_bytes;
            sheet[dest_start..dest_end].copy_from_slice(&panel[source_start..source_end]);
        }
    }
    sheet
}

fn draw_actor_walk_review_overlay(pixels: &mut [u8], width: u32, height: u32, camera: ChunkCamera) {
    let guide_color = [59, 64, 69, 255];
    let tick_color = [92, 98, 104, 255];
    for z in [
        -1.25, -1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0, 1.25,
    ] {
        draw_projected_line(
            pixels,
            width,
            height,
            camera,
            Vec3::new(0.0, 0.0, z),
            Vec3::new(0.0, 0.08, z),
            tick_color,
        );
    }
    draw_projected_line(
        pixels,
        width,
        height,
        camera,
        Vec3::new(0.0, 0.0, -1.35),
        Vec3::new(0.0, 0.0, 1.35),
        guide_color,
    );
}

fn draw_projected_line(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    camera: ChunkCamera,
    start: Vec3,
    end: Vec3,
    color: [u8; 4],
) {
    let view = camera.render_view(width, height);
    let Some(start) = project_world_to_pixel(view.view_projection, width, height, start) else {
        return;
    };
    let Some(end) = project_world_to_pixel(view.view_projection, width, height, end) else {
        return;
    };
    draw_line_2d(pixels, width, height, start, end, color);
}

fn project_world_to_pixel(
    view_projection: glam::Mat4,
    width: u32,
    height: u32,
    point: Vec3,
) -> Option<[i32; 2]> {
    let clip = view_projection * Vec4::new(point.x, point.y, point.z, 1.0);
    if clip.w.abs() <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() {
        return None;
    }
    let x = ((ndc.x * 0.5 + 0.5) * width as f32).round() as i32;
    let y = ((1.0 - (ndc.y * 0.5 + 0.5)) * height as f32).round() as i32;
    Some([x, y])
}

fn draw_line_2d(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    start: [i32; 2],
    end: [i32; 2],
    color: [u8; 4],
) {
    let mut x0 = start[0];
    let mut y0 = start[1];
    let x1 = end[0];
    let y1 = end[1];
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        put_pixel(pixels, width, height, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let err2 = err * 2;
        if err2 >= dy {
            err += dy;
            x0 += sx;
        }
        if err2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn put_pixel(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let start = ((y as u32 * width + x as u32) * 4) as usize;
    let Some(pixel) = pixels.get_mut(start..start + 4) else {
        return;
    };
    pixel.copy_from_slice(&color);
}

fn write_actor_walk_review_video(
    video_path: &Path,
    frame_pixels: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
) -> Result<u64> {
    ensure_parent_dir(video_path)?;
    let frame_dir = temporary_actor_walk_review_frame_dir();
    std::fs::create_dir_all(&frame_dir)
        .with_context(|| format!("failed to create `{}`", frame_dir.display()))?;
    for (index, pixels) in frame_pixels.iter().enumerate() {
        let path = frame_dir.join(format!("frame-{index:04}.png"));
        save_rgba_png(&path, width, height, pixels)?;
    }

    let input_pattern = frame_dir.join("frame-%04d.png");
    let fps_arg = fps.to_string();
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-framerate")
        .arg(&fps_arg)
        .arg("-i")
        .arg(&input_pattern)
        .arg("-vf")
        .arg("pad=ceil(iw/2)*2:ceil(ih/2)*2")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg(video_path)
        .output()
        .with_context(|| {
            format!(
                "failed to run ffmpeg for actor walk review video `{}`",
                video_path.display()
            )
        })?;
    let _ = std::fs::remove_dir_all(&frame_dir);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg failed for actor walk review video: {stderr}");
    }
    Ok(std::fs::metadata(video_path)
        .with_context(|| format!("failed to stat `{}`", video_path.display()))?
        .len())
}

fn temporary_actor_walk_review_frame_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "mclone-actor-walk-review-{}-{nanos}",
        std::process::id()
    ))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }
    Ok(())
}

fn non_clear_rgb_pixel_count(pixels: &[u8]) -> usize {
    let Some(clear) = pixels.get(0..3) else {
        return 0;
    };
    pixels
        .chunks_exact(4)
        .filter(|pixel| &pixel[0..3] != clear)
        .count()
}

pub(crate) fn write_headless_dual_view(
    options: &HeadlessDualViewOptions,
) -> Result<Vec<HeadlessDualViewReport>> {
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "failed to create dual-view output directory {}",
            options.directory.display()
        )
    })?;

    let base_camera = ChunkCamera::overview_for_chunk_area(
        options.scene.chunk_x,
        options.scene.chunk_z,
        options.scene.render_distance,
    );
    let cameras = dual_view_cameras(base_camera);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let assets = WindowSceneAssets::load()?;
    let asset_source = load_asset_source()?;
    let ui = if options.hud {
        mclone_scene::MonoUiPresentation::ScreenSpaceHud
    } else {
        mclone_scene::MonoUiPresentation::None
    };

    // Drive the shared scene host's mono (flat) view topology (tactical 168
    // Slice 3): the host owns the session runtime, budgeted section streaming,
    // and sky/time/sun/far-LOD frame-input assembly. Two capture frames render
    // the offset stereo-preview cameras with the runtime frozen after warmup.
    let (loop_report, frame_pixels, host) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: cameras.len(),
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenDriver::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                None,
            )?;
            host.drive_until_streamed(device, queue)?;
            Ok(host)
        },
        |index, frame, host| {
            host.render_chunk_camera_frozen(frame, cameras[index].1, ui)?;
            Ok(())
        },
    )?;

    let summaries = host.captured();
    let mut reports = Vec::new();
    for (index, (view_name, _camera)) in cameras.iter().enumerate() {
        let pixels = frame_pixels
            .get(index)
            .with_context(|| format!("dual-view {view_name} capture produced no frame"))?;
        let non_clear_rgb_pixel_count = non_clear_rgb_pixel_count(pixels);
        if non_clear_rgb_pixel_count == 0 {
            bail!(
                "headless dual-view {view_name} capture had no world pixels over the clear background"
            );
        }
        let path = options.directory.join(format!("{view_name}.png"));
        save_rgba_png(&path, loop_report.width, loop_report.height, pixels)?;
        let summary = summaries
            .get(index)
            .with_context(|| format!("dual-view {view_name} capture recorded no summary"))?
            .render;
        if options.hud && summary.gui_command_count == 0 {
            bail!("headless dual-view {view_name} HUD capture emitted no GUI commands");
        }
        reports.push(HeadlessDualViewReport {
            view_name,
            path,
            width: loop_report.width,
            height: loop_report.height,
            byte_len: pixels.len(),
            non_clear_rgb_pixel_count,
            section_count: summary.section_count,
            drawn_section_count: summary.drawn_section_count,
            index_count: summary.index_count,
            drawn_index_count: summary.drawn_index_count,
            gui_command_count: summary.gui_command_count,
        });
    }
    Ok(reports)
}

fn dual_view_cameras(base: ChunkCamera) -> [(&'static str, ChunkCamera); 2] {
    let eye = Vec3::from_array(base.eye);
    let target = Vec3::from_array(base.target);
    let up = Vec3::from_array(base.up);
    let forward = (target - eye).normalize_or_zero();
    let right = forward.cross(up).normalize_or_zero();
    let separation = 1.2_f32;
    [
        ("left", offset_camera(base, right * -separation * 0.5)),
        ("right", offset_camera(base, right * separation * 0.5)),
    ]
}

fn offset_camera(mut camera: ChunkCamera, offset: Vec3) -> ChunkCamera {
    camera.eye = (Vec3::from_array(camera.eye) + offset).to_array();
    camera.target = (Vec3::from_array(camera.target) + offset).to_array();
    camera
}

pub(crate) fn run_renderer_rebuild_smoke(
    options: &RendererRebuildSmokeOptions,
) -> Result<RendererRebuildSmokeReport> {
    if options.scene.remote_addr.is_some() {
        bail!("renderer rebuild smoke currently requires the local integrated server path");
    }
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "failed to create renderer rebuild smoke output directory {}",
            options.directory.display()
        )
    })?;
    let before_path = options.directory.join("before.png");
    let after_path = options.directory.join("after.png");

    let assets = WindowSceneAssets::load()?;
    let mut runtime =
        native_window_scene_runtime_with_mesh_assets(&options.scene, assets.mesh_assets.clone())?;
    poll_window_runtime_until_idle(&mut runtime)?;
    if let Some(day_time) = options.scene.day_time_override {
        runtime.force_day_time(day_time);
    }

    let mut spectator = SpectatorCamera::spawn_for_scene(&options.scene);
    let (world_x, world_z) = spectator.block_column();
    if let Some(surface_y) = runtime.highest_non_air_block_y_at_world(world_x, world_z) {
        spectator.place_above_surface(surface_y);
    }
    let camera = spectator.camera(runtime.render_distance());
    let section_update = runtime.sync_all_render_sections(spectator.position)?;
    // docs/tactical/163: first (cold) sync; keep an owned copy of the transient
    // full batch to re-upload during the render-resource rebuild step, since the
    // resident cache no longer retains CPU meshes.
    let sections = section_update.rebuilt_sections.clone();
    if sections.is_empty() {
        bail!(
            "renderer rebuild smoke seed={} center=({}, {}) render_distance={} produced no render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance
        );
    }
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: section_update.rebuilt_section_count(),
        removed_section_count: section_update.removed_section_count(),
        uploaded_vertex_count: section_update.rebuilt_vertex_count,
        uploaded_index_count: section_update.rebuilt_index_count,
    };
    let asset_source = load_asset_source()?;
    let render_options = options.render_options;
    let rebuild_render_scale = options.rebuild_render_scale;

    let (loop_report, frame_pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: 2,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let render_config =
                RenderConfig::for_color_target(render_options.color_profile, format);
            let next_render_config = rebuild_render_scale
                .map(|render_scale| render_config.with_render_scale(render_scale));
            let config_changed = next_render_config
                .map(|next_render_config| next_render_config != render_config)
                .unwrap_or(false);
            let mut resources = FlatRenderResources::new(
                device,
                queue,
                size,
                render_config,
                runtime.mesh_assets().atlas.as_upload(),
                assets.actor_textures.atlas.as_upload(),
                Some(&assets.actor_textures.figures),
                &asset_source,
            )?;
            resources
                .draw_mut()
                .update_sections(device, &sections)
                .context("failed to upload initial renderer rebuild smoke sections")?;
            resources.draw_mut().set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(spectator.position),
            );

            let mut render_stats = RenderStreamStats {
                section_count: resources.section_count(),
                index_count: resources.index_count(),
                face_count: quad_face_count_from_indices(resources.index_count()),
                ..RenderStreamStats::default()
            };
            record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);

            let mut ui = GameUiHost::new_ingame();
            ui.set_scale(GuiScale::from_pixels(size[0], size[1]));
            let before_render_size = resources.render_size();

            Ok(RendererRebuildSmokeState {
                runtime,
                actor_textures: assets.actor_textures,
                resources,
                asset_source,
                sections,
                camera,
                ui,
                actor_interpolation: ActorInterpolationState::new(),
                render_stats,
                render_options,
                next_render_config,
                config_changed,
                before_render_size,
                after_render_size: before_render_size,
                reuploaded_section_count: 0,
                state_preserved: false,
                summary: None,
            })
        },
        |index, frame, state| {
            if index == 1 {
                rebuild_renderer_rebuild_smoke_resources(&frame, state)?;
            }
            let summary = render_renderer_rebuild_smoke_frame(frame, state)?;
            state.summary = Some(summary);
            Ok(())
        },
    )?;

    if frame_pixels.len() != 2 {
        bail!(
            "renderer rebuild smoke expected 2 captured frames, got {}",
            frame_pixels.len()
        );
    }
    let before_pixels = &frame_pixels[0];
    let after_pixels = &frame_pixels[1];
    save_rgba_png(
        &before_path,
        loop_report.width,
        loop_report.height,
        before_pixels,
    )?;
    save_rgba_png(
        &after_path,
        loop_report.width,
        loop_report.height,
        after_pixels,
    )?;

    let pixel_mismatch_count = count_pixel_mismatches(before_pixels, after_pixels);
    if !state.config_changed && pixel_mismatch_count > 0 {
        bail!("renderer rebuild smoke before/after pixels differ in {pixel_mismatch_count} pixels");
    }
    if state.config_changed && state.before_render_size == state.after_render_size {
        bail!(
            "renderer rebuild smoke expected render size to change for config-changing run, got {:?}",
            state.after_render_size
        );
    }
    let before_non_clear_rgb_pixel_count = count_non_clear_rgb_pixels(before_pixels);
    let after_non_clear_rgb_pixel_count = count_non_clear_rgb_pixels(after_pixels);
    if before_non_clear_rgb_pixel_count == 0 || after_non_clear_rgb_pixel_count == 0 {
        bail!("renderer rebuild smoke captured no world pixels");
    }

    let summary = state
        .summary
        .context("renderer rebuild smoke rendered no summary")?;
    if !state.state_preserved {
        bail!("renderer rebuild smoke did not perform the rebuild preservation check");
    }

    Ok(RendererRebuildSmokeReport {
        before_path,
        after_path,
        width: loop_report.width,
        height: loop_report.height,
        byte_len: before_pixels.len(),
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        index_count: summary.index_count,
        drawn_index_count: summary.drawn_index_count,
        before_non_clear_rgb_pixel_count,
        after_non_clear_rgb_pixel_count,
        pixel_mismatch_count,
        reuploaded_section_count: state.reuploaded_section_count,
        state_preserved: state.state_preserved,
        config_changed: state.config_changed,
        before_render_size: state.before_render_size,
        after_render_size: state.after_render_size,
    })
}

fn rebuild_renderer_rebuild_smoke_resources(
    frame: &mclone_render::target::RenderFrameContext<'_>,
    state: &mut RendererRebuildSmokeState,
) -> Result<()> {
    let preserved_before =
        RendererRebuildSmokePreservedState::capture(&state.runtime, state.camera, &state.ui);
    let render_config = state
        .next_render_config
        .take()
        .unwrap_or_else(|| state.resources.render_config());
    let mut resources = FlatRenderResources::new(
        frame.device,
        frame.queue,
        frame.target.size,
        render_config,
        state.runtime.mesh_assets().atlas.as_upload(),
        state.actor_textures.atlas.as_upload(),
        Some(&state.actor_textures.figures),
        &state.asset_source,
    )?;
    let upload_report = resources
        .draw_mut()
        .update_sections(frame.device, &state.sections)
        .context("failed to reupload renderer rebuild smoke sections")?;
    resources.draw_mut().set_traversal_ready_sections(
        &state
            .runtime
            .traversal_ready_render_section_keys(Vec3::from_array(state.camera.eye)),
    );
    state.render_stats.section_count = resources.section_count();
    state.render_stats.index_count = resources.index_count();
    state.render_stats.face_count = quad_face_count_from_indices(resources.index_count());
    state.render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
    state.render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
    state.render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
    state.render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
    state.render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
    state.reuploaded_section_count = upload_report.uploaded_section_count;
    state.after_render_size = resources.render_size();
    state.resources = resources;

    let preserved_after =
        RendererRebuildSmokePreservedState::capture(&state.runtime, state.camera, &state.ui);
    if preserved_before != preserved_after {
        bail!(
            "renderer rebuild smoke changed preserved state: before={preserved_before:?} after={preserved_after:?}"
        );
    }
    state.state_preserved = true;
    Ok(())
}

fn render_renderer_rebuild_smoke_frame(
    frame: mclone_render::target::RenderFrameContext<'_>,
    state: &mut RendererRebuildSmokeState,
) -> Result<FullFrameRenderSummary> {
    let camera_position = Vec3::from_array(state.camera.eye);
    state.resources.draw_mut().set_traversal_ready_sections(
        &state
            .runtime
            .traversal_ready_render_section_keys(camera_position),
    );
    let underwater_overlay = state.runtime.camera_inside_water(camera_position).then(|| {
        let eye = Vec3::from_array(state.camera.eye);
        let target = Vec3::from_array(state.camera.target);
        let forward = (target - eye).normalize_or_zero();
        let forward = if forward.length_squared() > 0.0 {
            forward
        } else {
            Vec3::NEG_Z
        };
        let yaw = forward.x.atan2(forward.z);
        let pitch = forward.y.asin();
        UnderwaterOverlay::vanilla_from_native_camera(yaw, pitch)
    });
    let sky_clear_color = state.runtime.sky_clear_color();
    let time_of_day = state.runtime.time_of_day();
    let sun_angle = state.runtime.sun_angle();
    state
        .actor_interpolation
        .reconcile_authoritative(state.runtime.client().actor_presentations());
    state
        .actor_interpolation
        .step(1.0 / 60.0, Default::default());
    let actor_instances = actor_instances_from_presentations(
        &state.actor_interpolation.presentations(),
        state.runtime.client(),
    );
    let gui_scale = state.ui.scale();
    let gui_state = FullFrameGui::new(
        state.ui.is_active(),
        state.ui.covers_world(),
        [gui_scale.width, gui_scale.height],
    );
    let ui_draw = GuiDrawList::new();
    state.resources.render_full_frame(
        frame,
        state.camera,
        &actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        state.render_options,
        None,
        &[],
        None,
        gui_state,
        |_| ui_draw,
        &mut state.render_stats,
    )
}

fn count_pixel_mismatches(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len()).div_ceil(4)
}

fn count_non_clear_rgb_pixels(pixels: &[u8]) -> usize {
    let Some(clear) = pixels.get(0..3) else {
        return 0;
    };
    pixels
        .chunks_exact(4)
        .filter(|pixel| &pixel[0..3] != clear)
        .count()
}

pub(crate) fn run_headless_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<HeadlessScreenshotReport> {
    let report = crate::offscreen_flat_client::run_offscreen_flat_client_screenshot(options)?;

    Ok(HeadlessScreenshotReport {
        path: report.path,
        width: report.width,
        height: report.height,
        byte_len: report.byte_len,
        section_count: report.summary.section_count,
        drawn_section_count: report.summary.drawn_section_count,
        index_count: report.summary.index_count,
        drawn_index_count: report.summary.drawn_index_count,
        gui_command_count: report.summary.gui_command_count,
        flat_hud_retained_rebuild_count: report.summary.flat_hud_retained_cache.rebuild_count,
        flat_hud_retained_cache_hit_count: report.summary.flat_hud_retained_cache.cache_hit_count,
        remote_player_count: report.remote_player_count,
        entity_count: report.entity_count,
        actor_count: report.summary.actor_count,
        drawn_actor_count: report.summary.drawn_actor_count,
        far_lod_region_draw_count: report.summary.far_lod_region_draw_count,
        far_lod_uploaded_bytes: report.summary.far_lod_uploaded_bytes,
        underwater: report.underwater,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_review_views_include_front_side_and_three_quarter() {
        let views = actor_review_views();

        assert_eq!(
            views.map(|view| view.name),
            ["front", "side", "three_quarter"]
        );
        assert_eq!(views[0].camera.eye, [0.0, 1.0, 5.2]);
        assert_eq!(views[1].camera.eye, [5.2, 1.0, 0.0]);
        assert_eq!(views[2].camera.target, [0.0, 0.92, 0.0]);
    }

    #[test]
    fn actor_review_actors_include_first_party_figures() {
        let actors = actor_review_actors("front");

        assert_eq!(
            actors.len(),
            mclone_assets::FIRST_PARTY_ACTOR_FIGURE_IDS.len() + 1
        );
        assert_eq!(
            actors[0].shape,
            mclone_render::entity::ActorInstanceShape::Figure(
                mclone_assets::default_player_figure_id()
            )
        );
        assert_eq!(
            actors[1].shape,
            mclone_render::entity::ActorInstanceShape::Figure(
                mclone_assets::upright_bear_figure_id()
            )
        );
        assert_eq!(
            actors[2].shape,
            mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::chicken_figure_id())
        );
        assert_eq!(
            actors[3].shape,
            mclone_render::entity::ActorInstanceShape::ItemEgg
        );
        assert!(actors[0].feet_position.x < actors[1].feet_position.x);
        assert!(actors[1].feet_position.x < actors[2].feet_position.x);
        assert!(actors[2].feet_position.x < actors[3].feet_position.x);
    }

    #[test]
    fn actor_walk_review_actors_scale_distance_by_cycle() {
        let specs = [
            ActorWalkReviewFigureSpec {
                figure: mclone_assets::default_player_figure_id(),
                cycle_distance: 0.5,
            },
            ActorWalkReviewFigureSpec {
                figure: mclone_assets::upright_bear_figure_id(),
                cycle_distance: 1.25,
            },
            ActorWalkReviewFigureSpec {
                figure: mclone_assets::chicken_figure_id(),
                cycle_distance: 0.75,
            },
        ];

        let actors = actor_walk_review_actors(&specs, 1.5);

        assert_eq!(actors.len(), 3);
        assert_eq!(
            actors[0].shape,
            mclone_render::entity::ActorInstanceShape::Figure(
                mclone_assets::default_player_figure_id()
            )
        );
        assert_eq!(actors[0].animation.unwrap().distance, 0.75);
        assert_eq!(actors[1].animation.unwrap().distance, 1.875);
        assert_eq!(actors[2].animation.unwrap().distance, 1.125);
        assert_eq!(actors[2].chicken_wing_flap_radians, Some(0.45));
        assert!(actors[0].feet_position.z < actors[1].feet_position.z);
        assert!(actors[1].feet_position.z < actors[2].feet_position.z);
    }

    #[test]
    fn actor_walk_review_phase_spans_configured_cycles_without_duplicate_loop_frame() {
        assert_eq!(actor_walk_review_phase(0, 4, 2.0), 0.0);
        assert_eq!(actor_walk_review_phase(3, 4, 2.0), 1.5);
    }

    #[test]
    fn actor_walk_review_overlay_draws_floor_guide_pixels() {
        let width = 80;
        let height = 80;
        let mut pixels = vec![158, 168, 179, 255].repeat((width * height) as usize);

        draw_actor_walk_review_overlay(&mut pixels, width, height, actor_review_side_camera());

        assert!(non_clear_rgb_pixel_count(&pixels) > 0);
    }

    #[test]
    fn actor_review_sheet_stitches_panels_horizontally() {
        let red = vec![255, 0, 0, 255, 250, 0, 0, 255];
        let green = vec![0, 255, 0, 255, 0, 250, 0, 255];
        let blue = vec![0, 0, 255, 255, 0, 0, 250, 255];

        let sheet = stitch_actor_review_panels(&[red, green, blue], 2, 1);

        assert_eq!(
            sheet,
            vec![
                255, 0, 0, 255, 250, 0, 0, 255, 0, 255, 0, 255, 0, 250, 0, 255, 0, 0, 255, 255, 0,
                0, 250, 255,
            ]
        );
    }
}
