use std::collections::BTreeSet;

use anyhow::Context;
use mclone_app_runtime::frame_render::{
    FlatRenderResources, FullFrameGui, FullFrameRenderSummary, RenderStreamStats,
    record_render_section_update_stats,
};
use mclone_assets::AssetSource;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, BlockInteractionTarget,
    ClientInteractionController,
};
use mclone_core::Vec3d;
use mclone_input::{FlatInputAction, FlatInputFrame};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_render::chunk::{
    ChunkCamera, ChunkTextureAtlas, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::entity::ActorInstance;
use mclone_render::entity::ActorTextureAtlas;
use mclone_render::screen_effect::UnderwaterOverlay;
use mclone_render::selection_outline::SelectionOutline;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    RenderSectionCacheUpdate, actor_instances_from_presentations, render_camera_from_snapshot,
};
use mclone_ui::GuiDrawList;

use crate::camera::{SpectatorCamera, chunk_camera_from_engine};
use crate::cli::SceneOptions;
use crate::frame_pacing::FrameTimingStats;
use crate::scene_runtime::WindowSceneRuntime;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlatClientCameraView {
    pub(crate) snapshot: mclone_render_session::EngineCameraSnapshot,
    pub(crate) eye: glam::Vec3,
}

impl FlatClientCameraView {
    pub(crate) fn from_snapshot(snapshot: mclone_render_session::EngineCameraSnapshot) -> Self {
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
        }
    }

    pub(crate) fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    pub(crate) fn chunk_camera(self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot(self.snapshot, render_distance))
    }
}

pub(crate) struct FlatClientDriver {
    pub(crate) runtime: Option<WindowSceneRuntime>,
    pub(crate) spectator: SpectatorCamera,
    pub(crate) camera: EngineCameraController,
    pub(crate) actor_interpolation: ActorInterpolationState,
    pub(crate) interaction: ClientInteractionController,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) render_resources: Option<FlatRenderResources>,
    pub(crate) render_stats: RenderStreamStats,
    pub(crate) frame_timing: FrameTimingStats,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlatClientWorldActionStatus {
    NoRuntime,
    NoTarget,
    NoCommand,
    Sent {
        target: BlockInteractionTarget,
        changed: bool,
    },
}

pub(crate) struct FlatClientRuntimePoll {
    pub(crate) needs_section_upload: bool,
}

pub(crate) struct FlatClientSectionSync {
    pub(crate) section_update: RenderSectionCacheUpdate,
    pub(crate) remesh_ms: f64,
    pub(crate) loaded_chunk_count: usize,
}

pub(crate) struct FlatClientFullSectionSync {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) section_update: RenderSectionCacheUpdate,
    pub(crate) sections: Vec<TexturedRenderSectionMesh>,
    pub(crate) remesh_ms: f64,
}

pub(crate) struct FlatClientCachedSections {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) sections: Vec<TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientSectionUploadSummary {
    pub(crate) section_count: usize,
    pub(crate) face_count: u32,
    pub(crate) index_count: u32,
    pub(crate) loaded_chunk_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientFrameInputs {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) camera: ChunkCamera,
    pub(crate) sky_clear_color: wgpu::Color,
    pub(crate) time_of_day: f32,
    pub(crate) sun_angle: f32,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) underwater_overlay: Option<UnderwaterOverlay>,
    pub(crate) actor_instances: Vec<ActorInstance>,
    pub(crate) selection_outline: Option<SelectionOutline>,
    pub(crate) traversal_ready_sections: BTreeSet<RenderSectionKey>,
    pub(crate) render_stats: RenderStreamStats,
}

impl FlatClientDriver {
    pub(crate) fn new(scene: &SceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        let spectator = SpectatorCamera::spawn_for_scene(scene);
        let camera =
            engine_camera_controller_from_spectator(&spectator, scene.movement_speed_multiplier);
        Self {
            runtime: None,
            spectator,
            camera,
            actor_interpolation: ActorInterpolationState::new(),
            interaction: ClientInteractionController::new(),
            render_options,
            render_resources: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
        }
    }

    pub(crate) fn camera_view(&self) -> FlatClientCameraView {
        FlatClientCameraView::from_snapshot(self.camera.snapshot())
    }

    pub(crate) fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    pub(crate) fn current_render_distance(&self, fallback_render_distance: i32) -> u32 {
        self.runtime.as_ref().map_or_else(
            || u32::try_from(fallback_render_distance).unwrap_or(0),
            WindowSceneRuntime::render_distance,
        )
    }

    pub(crate) fn current_render_scale(&self, fallback_render_scale: f32) -> f32 {
        self.render_resources
            .as_ref()
            .map_or(fallback_render_scale, |resources| {
                resources.render_config().render_scale
            })
    }

    pub(crate) fn tick_frame_timing(&mut self, frame_ms: f64, target_frame_ms: Option<f64>) {
        self.render_stats.last_frame_ms = frame_ms as f32;
        self.frame_timing.begin_frame(frame_ms, target_frame_ms);
    }

    pub(crate) fn apply_held_input_frame(
        &mut self,
        frame: FlatInputFrame,
        dt_seconds: f64,
    ) -> bool {
        let Some(runtime) = self.runtime.as_ref() else {
            return false;
        };
        let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
        let before = self.camera.snapshot();
        let after = self.camera.apply_movement_input(runtime.client(), input);
        after != before
    }

    pub(crate) fn clear_camera_input(&mut self) {
        self.camera.clear_keys();
    }

    pub(crate) fn render_config(&self) -> Option<RenderConfig> {
        self.render_resources
            .as_ref()
            .map(FlatRenderResources::render_config)
    }

    pub(crate) fn rebuild_render_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_size: [u32; 2],
        render_config: RenderConfig,
        chunk_atlas: ChunkTextureAtlas<'_>,
        actor_atlas: ActorTextureAtlas<'_>,
        asset_source: &impl AssetSource,
    ) -> anyhow::Result<(Option<RenderConfig>, RenderConfig)> {
        let previous_config = self.render_config();
        let resources = FlatRenderResources::new(
            device,
            queue,
            target_size,
            render_config,
            chunk_atlas,
            actor_atlas,
            asset_source,
        )?;
        let next_config = resources.render_config();
        self.render_resources = Some(resources);
        Ok((previous_config, next_config))
    }

    pub(crate) fn resize_frame_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if let Some(render_resources) = &mut self.render_resources {
            render_resources.resize_frame_targets(device, size);
        }
    }

    pub(crate) fn clear_draw_sections(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<TexturedSectionUploadReport> {
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(TexturedSectionUploadReport::default());
        };
        let report = render_resources
            .draw_mut()
            .update_sections(device, &[])
            .context("failed to clear world render sections")?;
        self.clear_render_stats();
        Ok(report)
    }

    pub(crate) fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.interaction.select_hotbar_slot(slot)
    }

    pub(crate) fn apply_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return false;
        }
        self.camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        self.sync_spectator_from_camera();
        true
    }

    pub(crate) fn adjust_camera_speed(&mut self, amount: f64) {
        self.camera.adjust_speed(amount);
        self.sync_spectator_from_camera();
    }

    pub(crate) fn commit_player_pose_change(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.commit_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_server_player_pose(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.sync_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn apply_pending_player_position_updates(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.apply_pending_engine_camera_position_updates(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_carried_item(&mut self) -> anyhow::Result<bool> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime.send_gameplay_command(command).map_err(Into::into)
    }

    pub(crate) fn current_block_interaction_target(&self) -> Option<BlockInteractionTarget> {
        let runtime = self.runtime.as_ref()?;
        self.camera
            .target_block(runtime.client(), &self.interaction)
    }

    pub(crate) fn current_selection_outline(&self, ui_active: bool) -> Option<SelectionOutline> {
        if ui_active {
            return None;
        }
        self.current_block_interaction_target()
            .map(|target| SelectionOutline::new(target.outline_boxes))
    }

    pub(crate) fn handle_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> anyhow::Result<FlatClientWorldActionStatus> {
        if self.runtime.is_none() {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        }
        self.sync_server_player_pose()?;
        self.sync_carried_item()?;
        let Some(target) = self.current_block_interaction_target() else {
            return Ok(FlatClientWorldActionStatus::NoTarget);
        };
        let command = match action {
            FlatInputAction::Attack => self.interaction.debug_instant_break_command(target.hit),
            FlatInputAction::Use => self.interaction.use_item_on_command(target.hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(FlatClientWorldActionStatus::NoCommand);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        };
        let changed = runtime.send_gameplay_command(command)?;
        Ok(FlatClientWorldActionStatus::Sent { target, changed })
    }

    pub(crate) fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime.as_ref().is_some_and(|runtime| {
                runtime.camera_inside_occluding_block(self.camera_view().eye)
            }),
        )
    }

    pub(crate) fn underwater_overlay(
        &self,
        camera_view: FlatClientCameraView,
    ) -> Option<UnderwaterOverlay> {
        self.runtime
            .as_ref()?
            .camera_inside_water(camera_view.eye)
            .then(|| {
                UnderwaterOverlay::vanilla_from_native_camera(
                    camera_view.snapshot.yaw_radians as f32,
                    camera_view.snapshot.pitch_radians as f32,
                )
            })
    }

    pub(crate) fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Vec::new();
        };
        self.actor_interpolation
            .reconcile_authoritative(runtime.client().actor_presentations());
        self.actor_interpolation.step(
            (self.render_stats.last_frame_ms * 0.001).min(0.1),
            ActorInterpolationConfig::default(),
        );
        actor_instances_from_presentations(
            &self.actor_interpolation.presentations(),
            runtime.client(),
        )
    }

    pub(crate) fn poll_runtime(&mut self) -> anyhow::Result<FlatClientRuntimePoll> {
        if self.runtime.is_none() {
            return Ok(FlatClientRuntimePoll {
                needs_section_upload: false,
            });
        }
        let camera_eye = self.camera_view().eye;
        let changed = {
            let runtime = self.runtime.as_mut().expect("runtime presence checked");
            runtime.poll()?
        };
        let changed = changed | self.apply_pending_player_position_updates()?;
        let needs_pending_upload = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.has_pending_render_work(camera_eye));
        Ok(FlatClientRuntimePoll {
            needs_section_upload: changed || needs_pending_upload,
        })
    }

    pub(crate) fn sync_runtime_sections(
        &mut self,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<FlatClientSectionSync>> {
        let camera_view = self.camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(None);
        };
        let remesh_start = std::time::Instant::now();
        let section_update = runtime.sync_render_sections(camera_view.eye)?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let loaded_chunk_count = runtime.client().loaded_chunk_count();
        Ok(Some(FlatClientSectionSync {
            section_update,
            remesh_ms,
            loaded_chunk_count,
        }))
    }

    pub(crate) fn sync_all_runtime_sections(
        &mut self,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<FlatClientFullSectionSync>> {
        let camera_view = self.camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(None);
        };
        let remesh_start = std::time::Instant::now();
        let section_update = runtime.sync_all_render_sections(camera_view.eye)?;
        let sections = runtime.cached_sections();
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        Ok(Some(FlatClientFullSectionSync {
            camera_view,
            section_update,
            sections,
            remesh_ms,
        }))
    }

    pub(crate) fn cached_runtime_sections(&self) -> Option<FlatClientCachedSections> {
        let camera_view = self.camera_view();
        let runtime = self.runtime.as_ref()?;
        Some(FlatClientCachedSections {
            camera_view,
            sections: runtime.cached_sections(),
        })
    }

    pub(crate) fn traversal_ready_section_keys(
        &self,
        camera_view: FlatClientCameraView,
    ) -> BTreeSet<RenderSectionKey> {
        self.runtime.as_ref().map_or_else(BTreeSet::new, |runtime| {
            runtime.traversal_ready_render_section_keys(camera_view.eye)
        })
    }

    pub(crate) fn record_section_upload(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
        section_count: usize,
        index_count: u32,
        remesh_ms: f64,
        upload_ms: f64,
        loaded_chunk_count: Option<usize>,
    ) -> FlatClientSectionUploadSummary {
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        FlatClientSectionUploadSummary {
            section_count,
            face_count,
            index_count,
            loaded_chunk_count,
        }
    }

    pub(crate) fn upload_runtime_sections(
        &mut self,
        device: &wgpu::Device,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<(FlatClientSectionSync, FlatClientSectionUploadSummary)>> {
        let Some(sync) = self.sync_runtime_sections(elapsed_ms)? else {
            return Ok(None);
        };
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .apply_section_updates(
                device,
                &sync.section_update.rebuilt_sections,
                &sync.section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_section_upload(
            &sync.section_update,
            upload_report,
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            sync.remesh_ms,
            upload_ms,
            Some(sync.loaded_chunk_count),
        );
        Ok(Some((sync, summary)))
    }

    pub(crate) fn upload_all_runtime_sections(
        &mut self,
        device: &wgpu::Device,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<(FlatClientFullSectionSync, FlatClientSectionUploadSummary)>> {
        let Some(sync) = self.sync_all_runtime_sections(elapsed_ms)? else {
            return Ok(None);
        };
        let traversal_ready_sections = self.traversal_ready_section_keys(sync.camera_view);
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .update_sections(device, &sync.sections)
            .context("failed to upload world chunk section resources")?;
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&traversal_ready_sections);
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_section_upload(
            &sync.section_update,
            upload_report,
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            sync.remesh_ms,
            upload_ms,
            None,
        );
        Ok(Some((sync, summary)))
    }

    pub(crate) fn upload_cached_runtime_sections(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<
        Option<(
            FlatClientCachedSections,
            FlatClientSectionUploadSummary,
            TexturedSectionUploadReport,
        )>,
    > {
        let Some(cached) = self.cached_runtime_sections() else {
            return Ok(None);
        };
        let traversal_ready_sections = self.traversal_ready_section_keys(cached.camera_view);
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .update_sections(device, &cached.sections)
            .context("failed to upload cached startup chunk section resources")?;
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&traversal_ready_sections);
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_cached_section_upload(
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            upload_ms,
        );
        Ok(Some((cached, summary, upload_report)))
    }

    pub(crate) fn record_cached_section_upload(
        &mut self,
        section_count: usize,
        index_count: u32,
        upload_ms: f64,
    ) -> FlatClientSectionUploadSummary {
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.render_stats.last_remesh_ms = 0.0;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(0.0, upload_ms);
        FlatClientSectionUploadSummary {
            section_count,
            face_count,
            index_count,
            loaded_chunk_count: None,
        }
    }

    pub(crate) fn clear_render_stats(&mut self) {
        self.render_stats.section_count = 0;
        self.render_stats.index_count = 0;
        self.render_stats.face_count = 0;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
    }

    pub(crate) fn prepare_frame_inputs(
        &mut self,
        fallback_render_distance: i32,
        ui_active: bool,
    ) -> FlatClientFrameInputs {
        let camera_view = self.camera_view();
        let camera =
            camera_view.chunk_camera(self.current_render_distance(fallback_render_distance));
        let sky_clear_color = self.runtime.as_ref().map_or_else(
            mclone_render::default_clear_color,
            WindowSceneRuntime::sky_clear_color,
        );
        let time_of_day = self
            .runtime
            .as_ref()
            .map_or(0.0, WindowSceneRuntime::time_of_day);
        let sun_angle = self
            .runtime
            .as_ref()
            .map_or(0.0, WindowSceneRuntime::sun_angle);
        let render_options = self.effective_render_options();
        let underwater_overlay = self.underwater_overlay(camera_view);
        let actor_instances = self.interpolated_actor_instances();
        let selection_outline = self.current_selection_outline(ui_active);
        let traversal_ready_sections = self.traversal_ready_section_keys(camera_view);
        let render_stats = self.render_stats;
        FlatClientFrameInputs {
            camera_view,
            camera,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            underwater_overlay,
            actor_instances,
            selection_outline,
            traversal_ready_sections,
            render_stats,
        }
    }

    pub(crate) fn render_full_frame<BuildGuiDraw>(
        &mut self,
        frame: mclone_render::target::RenderFrameContext<'_>,
        fallback_render_distance: i32,
        ui_active: bool,
        gui_state: FullFrameGui,
        build_gui_draw: BuildGuiDraw,
    ) -> anyhow::Result<FullFrameRenderSummary>
    where
        BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
    {
        let frame_inputs = self.prepare_frame_inputs(fallback_render_distance, ui_active);
        let Some(render_resources) = &mut self.render_resources else {
            anyhow::bail!("flat client render resources are not initialized");
        };
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&frame_inputs.traversal_ready_sections);
        let mut render_stats = frame_inputs.render_stats;
        let summary = render_resources.render_full_frame(
            frame,
            frame_inputs.camera,
            &frame_inputs.actor_instances,
            frame_inputs.underwater_overlay,
            frame_inputs.sky_clear_color,
            frame_inputs.time_of_day,
            frame_inputs.sun_angle,
            frame_inputs.render_options,
            frame_inputs.selection_outline.as_ref(),
            gui_state,
            build_gui_draw,
            &mut render_stats,
        )?;
        self.render_stats = render_stats;
        Ok(summary)
    }

    pub(crate) fn sync_spectator_from_camera(&mut self) {
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
    }

    pub(crate) fn set_spectator_camera(
        &mut self,
        spectator: SpectatorCamera,
        movement_speed_multiplier: f32,
    ) {
        self.spectator = spectator;
        self.camera =
            engine_camera_controller_from_spectator(&self.spectator, movement_speed_multiplier);
    }

    pub(crate) fn reset_world_state(&mut self, scene: &SceneOptions) {
        self.runtime = None;
        self.spectator = SpectatorCamera::spawn_for_scene(scene);
        self.camera = engine_camera_controller_from_spectator(
            &self.spectator,
            scene.movement_speed_multiplier,
        );
        self.actor_interpolation = ActorInterpolationState::new();
        self.interaction = ClientInteractionController::new();
        self.render_stats = RenderStreamStats::default();
        self.frame_timing = FrameTimingStats::default();
    }
}

pub(crate) fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

pub(crate) fn vec3d_from_glam(value: glam::Vec3) -> Vec3d {
    Vec3d::new(value.x as f64, value.y as f64, value.z as f64)
}

pub(crate) fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub(crate) fn engine_camera_controller_from_spectator(
    spectator: &SpectatorCamera,
    movement_speed_multiplier: f32,
) -> EngineCameraController {
    let mut camera = EngineCameraController::from_eye_pose(
        vec3d_from_glam(spectator.position),
        f64::from(spectator.yaw),
        f64::from(spectator.pitch),
        f64::from(spectator.speed),
    );
    camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));
    camera
}

pub(crate) fn sync_spectator_from_camera(
    spectator: &mut SpectatorCamera,
    camera: &EngineCameraController,
) {
    let snapshot = camera.snapshot();
    spectator.position = glam_vec3_from_vec3d(snapshot.eye);
    spectator.yaw = snapshot.yaw_radians as f32;
    spectator.pitch = snapshot.pitch_radians as f32;
    spectator.speed = snapshot.speed_blocks_per_second as f32;
}

pub(crate) fn engine_camera_input_from_flat_frame(
    frame: FlatInputFrame,
    dt_seconds: f64,
) -> EngineCameraInput {
    EngineCameraInput {
        dt_seconds,
        mouse_delta_x: f64::from(frame.look_delta.x),
        mouse_delta_y: f64::from(frame.look_delta.y),
        forward: frame.forward,
        backward: frame.backward,
        left: frame.left,
        right: frame.right,
        jump: frame.jump,
        descend: frame.descend,
        shift: frame.sneak,
        sprint: frame.sprint,
        movement_impulse: frame
            .analog_movement
            .map(|movement| EngineCameraMovementImpulse::new(movement.left, movement.forward)),
        movement_yaw_radians: None,
    }
}
