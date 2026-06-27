use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::{
    FlatRenderResources, FullFrameGui, FullFrameRenderSummary, RenderStreamStats,
    record_render_section_update_stats, render_full_frame_for_view,
};
use mclone_client::{
    ActorInterpolationState, ClientInteractionController, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
};
use mclone_core::{HitResultType, Vec3d};
use mclone_mesh::quad_face_count_from_indices;
use mclone_protocol::{AcceptTeleportCommand, ClientCommand, MovePlayerCommand};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::entity::ActorDrawResources;
use mclone_render::gui::GuiRenderer;
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessChunkOptions, HeadlessFrameLoopOptions, HeadlessFrameOptions,
    run_headless_capture_loop, save_rgba_png, write_headless_frame_png,
    write_headless_textured_sections_png_with_ready_sections,
};
use mclone_render::screen_effect::{ScreenEffectsRenderer, UnderwaterOverlay};
use mclone_render::sky_render::SkyRenderer;
use mclone_render_session::actor_instances_from_presentations;
use mclone_ui::{GameUi, GuiDrawList, GuiScale};

use crate::app::game_ui_render_state;
use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessDualViewOptions, HeadlessScreenshotOptions, RendererRebuildSmokeOptions, SceneOptions,
};
use crate::frame_pacing::{FramePacingDebugStats, FramePacingUiState, FrameTimingStats};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};
use crate::ui::{DebugPaneStats, render_debug_pane};

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
    pub(crate) remote_player_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
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
    fn capture(runtime: &WindowSceneRuntime, camera: ChunkCamera, ui: &GameUi) -> Self {
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
    runtime: WindowSceneRuntime,
    resources: FlatRenderResources,
    asset_source: mclone_assets::AssetSourceChain,
    sections: Vec<mclone_mesh::TexturedRenderSectionMesh>,
    camera: ChunkCamera,
    ui: GameUi,
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

pub(crate) fn write_headless_dual_view(
    options: &HeadlessDualViewOptions,
) -> Result<Vec<HeadlessDualViewReport>> {
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "failed to create dual-view output directory {}",
            options.directory.display()
        )
    })?;

    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    if let Some(day_time) = options.scene.day_time_override {
        runtime.force_day_time(day_time);
    }

    let base_camera = ChunkCamera::overview_for_chunk_area(
        options.scene.chunk_x,
        options.scene.chunk_z,
        options.scene.render_distance,
    );
    runtime.sync_all_render_sections(Vec3::from_array(base_camera.eye))?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!(
            "headless dual-view seed={} center=({}, {}) render_distance={} produced no render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance
        );
    }

    let mut reports = Vec::new();
    for (view_name, camera) in dual_view_cameras(base_camera) {
        let path = options.directory.join(format!("{view_name}.png"));
        reports.push(write_headless_dual_view_frame(
            &runtime,
            &sections,
            path,
            options.width,
            options.height,
            view_name,
            camera,
            options.render_options,
        )?);
    }
    Ok(reports)
}

fn write_headless_dual_view_frame(
    runtime: &WindowSceneRuntime,
    sections: &[mclone_mesh::TexturedRenderSectionMesh],
    path: PathBuf,
    width: u32,
    height: u32,
    view_name: &'static str,
    camera: ChunkCamera,
    render_options: TexturedSectionRenderOptions,
) -> Result<HeadlessDualViewReport> {
    let sky_clear_color = runtime.sky_clear_color();
    let time_of_day = runtime.time_of_day();
    let sun_angle = runtime.sun_angle();
    let (frame_report, summary) = write_headless_frame_png(
        HeadlessFrameOptions {
            path,
            width,
            height,
        },
        |frame| {
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            let mut draw = TexturedSectionDrawResources::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                sections,
                runtime.mesh_assets().atlas.as_upload(),
            )?;
            let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(render_view.camera_position),
            );
            let sky = SkyRenderer::new_with_color_profile(
                frame.device,
                HEADLESS_FORMAT,
                render_options.color_profile,
            );
            let mut render_stats = RenderStreamStats {
                section_count: draw.section_count(),
                index_count: draw.index_count(),
                face_count: quad_face_count_from_indices(draw.index_count()),
                ..RenderStreamStats::default()
            };
            render_full_frame_for_view(
                frame,
                &depth,
                &sky,
                &mut draw,
                None,
                None,
                None,
                render_view,
                &[],
                None,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FullFrameGui::new(false, false, [1.0, 1.0]),
                |_| GuiDrawList::new(),
                &mut render_stats,
            )
        },
    )?;
    if frame_report.non_clear_rgb_pixel_count == 0 {
        bail!(
            "headless dual-view {view_name} capture had no world pixels over the clear background: {}",
            frame_report.path.display()
        );
    }

    Ok(HeadlessDualViewReport {
        view_name,
        path: frame_report.path,
        width: frame_report.width,
        height: frame_report.height,
        byte_len: frame_report.byte_len,
        non_clear_rgb_pixel_count: frame_report.non_clear_rgb_pixel_count,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        index_count: summary.index_count,
        drawn_index_count: summary.drawn_index_count,
    })
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

    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
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
    let sections = runtime.cached_sections();
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
                runtime.actor_textures.atlas.as_upload(),
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

            let mut ui = GameUi::new_ingame();
            ui.set_scale(GuiScale::from_pixels(size[0], size[1]));
            let before_render_size = resources.render_size();

            Ok(RendererRebuildSmokeState {
                runtime,
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
        state.runtime.actor_textures.atlas.as_upload(),
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
    let ui_render_state = game_ui_render_state(
        state.runtime.render_distance() as i32,
        state.render_options,
        FramePacingUiState::default(),
        false,
        1.0,
        1.0,
    );
    let gui_scale = state.ui.scale();
    let gui_state = FullFrameGui::new(
        state.ui.is_active(),
        state.ui.covers_world(),
        [gui_scale.width, gui_scale.height],
    );
    let ui_draw = state.ui.render_draw_list(ui_render_state);
    state.resources.render_full_frame(
        frame,
        state.camera,
        &actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        state.render_options,
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

pub(crate) fn write_headless_chunk_scenarios(
    directory: &Path,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    render_options: TexturedSectionRenderOptions,
) -> Result<Vec<mclone_render::headless::HeadlessChunkReport>> {
    let mut runtime = WindowSceneRuntime::new(scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    let mut reports = Vec::new();
    for (name, camera) in chunk_capture_scenarios(scene) {
        reports.push(write_headless_runtime_chunk_with_camera(
            &mut runtime,
            directory.join(format!("{name}.png")),
            width,
            height,
            camera,
            render_options,
        )?);
    }
    Ok(reports)
}

pub(crate) fn write_headless_runtime_chunk(
    path: PathBuf,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    render_options: TexturedSectionRenderOptions,
) -> Result<mclone_render::headless::HeadlessChunkReport> {
    let mut runtime = WindowSceneRuntime::new(scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    write_headless_runtime_chunk_with_camera(
        &mut runtime,
        path,
        width,
        height,
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.render_distance),
        render_options,
    )
}

fn write_headless_runtime_chunk_with_camera(
    runtime: &mut WindowSceneRuntime,
    path: PathBuf,
    width: u32,
    height: u32,
    camera: ChunkCamera,
    render_options: TexturedSectionRenderOptions,
) -> Result<mclone_render::headless::HeadlessChunkReport> {
    let camera_position = Vec3::from_array(camera.eye);
    runtime.sync_all_render_sections(camera_position)?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!("headless runtime chunk capture produced no render sections");
    }
    let ready_sections = runtime.traversal_ready_render_section_keys(camera_position);
    write_headless_textured_sections_png_with_ready_sections(
        HeadlessChunkOptions {
            path,
            width,
            height,
            color: mclone_render::default_clear_color(),
            camera,
        },
        &sections,
        runtime.mesh_assets().atlas.as_upload(),
        render_options,
        Some(&ready_sections),
    )
}

pub(crate) fn run_headless_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<HeadlessScreenshotReport> {
    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    let initial_player_position = ack_pending_player_position_updates(&mut runtime)?;
    if let Some(day_time) = options.scene.day_time_override {
        runtime.force_day_time(day_time);
    }
    settle_remote_screenshot_session(&mut runtime, options.remote_settle_ms)?;
    let mut spectator = SpectatorCamera::spawn_for_scene(&options.scene);
    let (world_x, world_z) = spectator.block_column();
    if let Some(surface_y) = runtime.highest_non_air_block_y_at_world(world_x, world_z) {
        spectator.place_above_surface(surface_y);
    }
    if options.scripted_interaction {
        apply_scripted_interaction(&mut runtime, &mut spectator, initial_player_position)?;
    }
    frame_first_actor_for_screenshot(&runtime, &mut spectator);
    if let Some(eye) = options.eye {
        spectator.position = Vec3::from_array(eye);
    }
    let section_update = runtime.sync_all_render_sections(spectator.position)?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!(
            "headless screenshot seed={} center=({}, {}) render_distance={} produced no render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance
        );
    }

    let mut ui = GameUi::new();
    if options.ui == crate::cli::HeadlessScreenshotUi::NewWorld {
        ui.set_new_world_seed(options.scene.seed);
    }
    ui.set_screen(options.ui.game_screen());
    ui.set_scale(GuiScale::from_pixels(options.width, options.height));

    let mut render_stats = RenderStreamStats::default();
    let debug_pane = options.debug_pane;
    let render_options = options.render_options;
    let camera = spectator.camera(runtime.render_distance());
    let underwater_overlay = runtime
        .camera_inside_water(spectator.position)
        .then(|| UnderwaterOverlay::vanilla_from_native_camera(spectator.yaw, spectator.pitch));
    let underwater = underwater_overlay.is_some();
    let sky_clear_color = runtime.sky_clear_color();
    let time_of_day = runtime.time_of_day();
    let sun_angle = runtime.sun_angle();
    let runtime_stats = runtime.stats();
    let actor_interpolation =
        ActorInterpolationState::from_authoritative(runtime.client().actor_presentations());
    let actor_instances =
        actor_instances_from_presentations(&actor_interpolation.presentations(), runtime.client());
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: section_update.rebuilt_section_count(),
        removed_section_count: section_update.removed_section_count(),
        uploaded_vertex_count: section_update.rebuilt_vertex_count,
        uploaded_index_count: section_update.rebuilt_index_count,
    };

    let (frame_report, summary) = write_headless_frame_png(
        HeadlessFrameOptions {
            path: options.path.clone(),
            width: options.width,
            height: options.height,
        },
        |frame| {
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            let mut draw = TexturedSectionDrawResources::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                &sections,
                runtime.mesh_assets().atlas.as_upload(),
            )?;
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(spectator.position),
            );
            let sky = SkyRenderer::new_with_color_profile(
                frame.device,
                HEADLESS_FORMAT,
                render_options.color_profile,
            );
            let mut actors = ActorDrawResources::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                runtime.actor_textures.atlas.as_upload(),
            )?;
            let asset_source = load_asset_source()?;
            let mut screen_effects = ScreenEffectsRenderer::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                &asset_source,
            )?;
            let mut gui = GuiRenderer::new(frame.device, HEADLESS_FORMAT);

            render_stats.section_count = draw.section_count();
            render_stats.index_count = draw.index_count();
            render_stats.face_count = quad_face_count_from_indices(render_stats.index_count);
            record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);

            let debug_stats = debug_pane.then_some(DebugPaneStats {
                position: spectator.position,
                speed: spectator.speed,
                movement_mode: "NOCLIP",
                on_ground: false,
                runtime: runtime_stats,
                render: render_stats,
                frame: FrameTimingStats::default(),
                pacing: FramePacingDebugStats::default(),
                section_occlusion: render_options.section_occlusion_culling,
                force_fullbright: render_options.force_fullbright,
                color_profile: render_options.color_profile.label(),
            });
            let ui_active = ui.is_active();
            let ui_covers_world = ui.covers_world();
            let debug_stats = (!ui_active).then_some(debug_stats).flatten();
            let gui_active = ui_active || debug_stats.is_some();
            let gui_scale = ui.scale();
            let base_ui_draw = ui.render_draw_list(game_ui_render_state(
                runtime.render_distance() as i32,
                render_options,
                FramePacingUiState::default(),
                false,
                1.0,
                1.0,
            ));
            let gui_state = FullFrameGui::new(
                gui_active,
                ui_covers_world,
                [gui_scale.width, gui_scale.height],
            );

            let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
            render_full_frame_for_view(
                frame,
                &depth,
                &sky,
                &mut draw,
                Some(&mut actors),
                Some(&mut screen_effects),
                Some(&mut gui),
                render_view,
                &actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                gui_state,
                |stats| {
                    let mut ui_draw = base_ui_draw;
                    if let Some(mut debug_stats) = debug_stats {
                        debug_stats.render = *stats;
                        render_debug_pane(gui_scale, &mut ui_draw, &debug_stats);
                    }
                    ui_draw
                },
                &mut render_stats,
            )
        },
    )?;

    Ok(HeadlessScreenshotReport {
        path: frame_report.path,
        width: frame_report.width,
        height: frame_report.height,
        byte_len: frame_report.byte_len,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        index_count: summary.index_count,
        drawn_index_count: summary.drawn_index_count,
        gui_command_count: summary.gui_command_count,
        remote_player_count: runtime.client().remote_player_count(),
        entity_count: runtime.client().entity_count(),
        actor_count: summary.actor_count,
        drawn_actor_count: summary.drawn_actor_count,
        underwater,
    })
}

fn settle_remote_screenshot_session(
    runtime: &mut WindowSceneRuntime,
    remote_settle_ms: u64,
) -> Result<()> {
    if remote_settle_ms == 0
        || runtime.client().host() != mclone_client::ClientHost::RemoteDedicated
    {
        return Ok(());
    }

    std::thread::sleep(Duration::from_millis(remote_settle_ms));
    runtime
        .send_gameplay_command(ClientCommand::MovePlayer(MovePlayerCommand::StatusOnly {
            on_ground: false,
        }))
        .context("failed to poll remote screenshot session after settle delay")?;
    Ok(())
}

fn ack_pending_player_position_updates(runtime: &mut WindowSceneRuntime) -> Result<Option<Vec3d>> {
    let mut last_position = None;
    for update in runtime.drain_player_position_updates() {
        last_position = Some(update.position);
        runtime
            .send_gameplay_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: update.teleport_id,
            }))
            .context("failed to acknowledge initial player position")?;
    }
    Ok(last_position)
}

fn apply_scripted_interaction(
    runtime: &mut WindowSceneRuntime,
    spectator: &mut SpectatorCamera,
    initial_player_position: Option<Vec3d>,
) -> Result<()> {
    let (base_x, base_z) = spectator.block_column();
    let target_x = base_x;
    let target_z = base_z + 4;
    let surface_y = runtime
        .highest_non_air_block_y_at_world(target_x, target_z)
        .with_context(|| {
            format!("no loaded surface for scripted interaction at ({target_x}, {target_z})")
        })?;
    let interaction = ClientInteractionController::new();
    let eye_position = Vec3d::new(
        target_x as f64 + 0.5,
        surface_y as f64 + 3.0,
        target_z as f64 + 0.5,
    );
    let player_feet_position =
        eye_position.add(Vec3d::new(0.0, -LOCAL_PLAYER_STANDING_EYE_HEIGHT, 0.0));
    sync_scripted_player_position(
        runtime,
        initial_player_position.unwrap_or(player_feet_position),
        player_feet_position,
    )?;
    let hit = interaction.pick_block(runtime.client(), eye_position, Vec3d::new(0.0, -1.0, 0.0));
    if hit.hit_type() != HitResultType::Block {
        bail!("scripted interaction ray missed target column");
    }
    let break_command = interaction
        .debug_instant_break_command(hit)
        .context("scripted interaction did not produce break command")?;
    let break_changed = runtime.send_gameplay_command(break_command)?;
    let place_command = interaction
        .use_item_on_command(hit)
        .context("scripted interaction did not produce place command")?;
    let place_changed = runtime.send_gameplay_command(place_command)?;
    if !break_changed || !place_changed {
        bail!(
            "scripted interaction did not mutate both blocks: break_changed={break_changed} place_changed={place_changed}"
        );
    }

    spectator.position = glam::Vec3::new(
        target_x as f32 + 0.5,
        surface_y as f32 + 5.0,
        target_z as f32 - 6.0,
    );
    spectator.yaw = 0.0;
    spectator.pitch = -0.7;
    Ok(())
}

fn sync_scripted_player_position(
    runtime: &mut WindowSceneRuntime,
    mut current: Vec3d,
    target: Vec3d,
) -> Result<()> {
    for _ in 0..64 {
        let delta = target.subtract(current);
        if delta.length_sqr() <= 64.0 {
            send_scripted_player_move(runtime, target)?;
            return Ok(());
        }
        let length = delta.length_sqr().sqrt();
        current = current.add(delta.scale(8.0 / length));
        send_scripted_player_move(runtime, current)?;
        runtime.poll()?;
    }
    bail!("timed out moving scripted player to interaction target");
}

fn send_scripted_player_move(runtime: &mut WindowSceneRuntime, position: Vec3d) -> Result<()> {
    runtime
        .send_gameplay_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position,
            y_rot_degrees: 0.0,
            x_rot_degrees: 90.0,
            on_ground: false,
        }))
        .context("failed to sync scripted player position")?;
    Ok(())
}

fn frame_first_actor_for_screenshot(runtime: &WindowSceneRuntime, spectator: &mut SpectatorCamera) {
    let Some(actor) = runtime.client().actor_presentations().first().copied() else {
        return;
    };
    let target = glam::Vec3::new(
        actor.feet_position.x as f32,
        actor.feet_position.y as f32 + 1.0,
        actor.feet_position.z as f32,
    );
    let eye = target + glam::Vec3::new(-2.2, 1.4, -4.8);
    aim_spectator_at(spectator, eye, target);
}

fn aim_spectator_at(spectator: &mut SpectatorCamera, eye: glam::Vec3, target: glam::Vec3) {
    let direction = target - eye;
    let Some(direction) = direction.try_normalize() else {
        return;
    };
    spectator.position = eye;
    spectator.yaw = direction.x.atan2(direction.z);
    spectator.pitch = direction.y.clamp(-1.0, 1.0).asin();
}

fn chunk_capture_scenarios(scene: &SceneOptions) -> [(&'static str, ChunkCamera); 3] {
    let overview =
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.render_distance);
    let mut orbit = overview;
    orbit.orbit(0.7, -0.16);
    let mut close = overview;
    close.zoom(0.45);
    close.orbit(-0.32, 0.08);
    [
        ("overview", overview),
        ("orbit-east", orbit),
        ("close", close),
    ]
}
