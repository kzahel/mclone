use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::{
    FlatRenderResources, FullFrameGui, FullFrameRenderSummary, RenderStreamStats,
    record_render_section_update_stats, render_full_frame_for_view,
};
use mclone_client::ActorInterpolationState;
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessChunkOptions, HeadlessFrameLoopOptions, HeadlessFrameOptions,
    run_headless_capture_loop, save_rgba_png, write_headless_frame_png,
    write_headless_textured_sections_png_with_ready_sections,
};
use mclone_render::screen_effect::UnderwaterOverlay;
use mclone_render::sky_render::SkyRenderer;
use mclone_render_session::actor_instances_from_presentations;
use mclone_ui::{GameUi, GuiDrawList, GuiScale};

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessDualViewOptions, HeadlessScreenshotOptions, RendererRebuildSmokeOptions, SceneOptions,
};
use crate::flat_client_driver::{FlatClientUiRenderOptions, game_ui_render_state};
use crate::frame_pacing::FramePacingUiState;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};

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
    let ui_render_state = game_ui_render_state(FlatClientUiRenderOptions {
        render_distance: state.runtime.render_distance() as i32,
        render_options: state.render_options,
        frame_pacing: FramePacingUiState::default(),
        fly_enabled: false,
        fly_speed_multiplier: 1.0,
        movement_speed_multiplier: 1.0,
    });
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
        remote_player_count: report.remote_player_count,
        entity_count: report.entity_count,
        actor_count: report.summary.actor_count,
        drawn_actor_count: report.summary.drawn_actor_count,
        underwater: report.underwater,
    })
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
