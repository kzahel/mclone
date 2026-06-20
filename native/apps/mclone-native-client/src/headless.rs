use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
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
use mclone_render::entity::ActorDrawResources;
use mclone_render::gui::GuiRenderer;
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessChunkOptions, HeadlessFrameOptions, write_headless_frame_png,
    write_headless_textured_sections_png_with_options,
};
use mclone_render::sky_render::SkyRenderer;
use mclone_ui::GuiScale;

use crate::app::{
    RenderStreamStats, actor_instances_from_presentations, record_render_section_update_stats,
    render_full_frame,
};
use crate::camera::SpectatorCamera;
use crate::cli::{HeadlessScreenshotOptions, SceneOptions};
use crate::frame_pacing::{FramePacingDebugStats, FramePacingUiState, FrameTimingStats};
use crate::render_cache::SceneTexturedSections;
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};
use crate::ui::{DebugPaneStats, NativeUi};

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
}
pub(crate) fn write_headless_chunk_scenarios(
    directory: &Path,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    scene_mesh: &SceneTexturedSections,
    render_options: TexturedSectionRenderOptions,
) -> Result<Vec<mclone_render::headless::HeadlessChunkReport>> {
    let mut reports = Vec::new();
    for (name, camera) in chunk_capture_scenarios(scene) {
        let path = directory.join(format!("{name}.png"));
        reports.push(write_headless_textured_sections_png_with_options(
            HeadlessChunkOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
                camera,
            },
            &scene_mesh.sections,
            scene_mesh.atlas.as_upload(),
            render_options,
        )?);
    }
    Ok(reports)
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

    let mut ui = NativeUi::new(options.scene.render_distance);
    ui.set_screen(options.ui.native_screen());
    ui.set_scale(GuiScale::from_pixels(options.width, options.height));

    let mut render_stats = RenderStreamStats::default();
    let debug_pane = options.debug_pane;
    let render_options = options.render_options;
    let camera = spectator.camera(runtime.render_distance);
    let sky_clear_color = runtime.sky_clear_color();
    let time_of_day = runtime.time_of_day();
    let sun_angle = runtime.sun_angle();
    let runtime_stats = runtime.stats();
    let actor_interpolation =
        ActorInterpolationState::from_authoritative(runtime.client.actor_presentations());
    let actor_instances = actor_instances_from_presentations(&actor_interpolation.presentations());
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
                runtime.mesh_assets.atlas.as_upload(),
            )?;
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(spectator.position),
            );
            let sky = SkyRenderer::new(frame.device, HEADLESS_FORMAT);
            let mut actors = ActorDrawResources::new(frame.device, HEADLESS_FORMAT);
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
            });

            render_full_frame(
                frame,
                &depth,
                &sky,
                &mut draw,
                &mut actors,
                &mut gui,
                camera,
                &actor_instances,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FramePacingUiState::default(),
                &ui,
                debug_stats,
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
        remote_player_count: runtime.client.remote_player_count(),
        entity_count: runtime.client.entity_count(),
        actor_count: summary.actor_count,
        drawn_actor_count: summary.drawn_actor_count,
    })
}

fn settle_remote_screenshot_session(
    runtime: &mut WindowSceneRuntime,
    remote_settle_ms: u64,
) -> Result<()> {
    if remote_settle_ms == 0 || runtime.client.host() != mclone_client::ClientHost::RemoteDedicated
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
    let hit = interaction.pick_block(&runtime.client, eye_position, Vec3d::new(0.0, -1.0, 0.0));
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
    let Some(actor) = runtime.client.actor_presentations().first().copied() else {
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
