//! Thin desktop surface/cadence driver over the shared scene host's Mono
//! topology (tactical 168 Slice 7c).

use anyhow::Result;
use mclone_app_runtime::frame_pipeline_accounting::FramePipelineAccountant;
use mclone_app_runtime::frame_render::{FlatScalePresentation, scaled_frame_size};
use mclone_audio::AudioEngine;
use mclone_diagnostics::FrameHostKind;
use mclone_input::{FlatInputAction, FlatInputFrame, TouchControlsMode};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::color_profile::RenderConfig;
use mclone_render::target::RenderFrameContext;
use mclone_render_session::{EngineCameraMovementMode, EngineCameraViewMode};
use mclone_scene::{
    HostEffects, MonoBlinkCommitStatus, MonoSceneFrameSummary, MonoUiActionOutcome, MonoUiContext,
    MonoUiPresentation, MonoWorldActionStatus, record_mono_frame_pipeline,
    xr_frame_pipeline_accounting_config,
};
use mclone_ui::{GameUiAction, GameUiHost, GuiKey, GuiScale, Point, UiDebugSnapshot};

use crate::cli::{SceneOptions, WindowStartIntent};
use crate::desktop_scene_host::{DesktopMonoSceneHost, create_desktop_mono_scene_host};
use crate::scene_runtime::WindowSceneAssets;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WinitHostEffectOutcome {
    pub(crate) mouse_lock_requested: Option<bool>,
    pub(crate) cycle_frame_pacing: bool,
    pub(crate) cycle_fps_cap: bool,
    pub(crate) touch_controls_mode: Option<TouchControlsMode>,
    pub(crate) quit_to_title: bool,
    pub(crate) exit: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WinitUiActionOutcome {
    pub(crate) scene: MonoUiActionOutcome,
    pub(crate) host: WinitHostEffectOutcome,
}

#[derive(Default)]
struct WinitHostEffects {
    outcome: WinitHostEffectOutcome,
}

impl HostEffects for WinitHostEffects {
    fn request_mouse_lock(&mut self, requested: bool) -> Result<()> {
        self.outcome.mouse_lock_requested = Some(requested);
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        self.outcome.cycle_frame_pacing = true;
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        self.outcome.cycle_fps_cap = true;
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, mode: TouchControlsMode) -> Result<()> {
        self.outcome.touch_controls_mode = Some(mode);
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        self.outcome.quit_to_title = true;
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        self.outcome.exit = true;
        Ok(())
    }
}

pub(crate) struct WinitFrameDriver {
    host: DesktopMonoSceneHost,
    depth: ChunkDepthTarget,
    target_size: [u32; 2],
    adaptive_render_admission_budget: bool,
    frame_pipeline_accounting: FramePipelineAccountant,
    render_config: RenderConfig,
    scale_presentation: Option<FlatScalePresentation>,
    arm_mouse_lock_after_start: bool,
    deferred_mouse_lock_request: Option<bool>,
}

impl WinitFrameDriver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        target_size: [u32; 2],
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
        start_intent: WindowStartIntent,
        ui_v2_debug_overlay: bool,
    ) -> Result<Self> {
        let mut initial_scene = scene.clone();
        if start_intent == WindowStartIntent::Menu {
            initial_scene.remote_addr = None;
        }
        let mut host = create_desktop_mono_scene_host(
            device,
            queue,
            render_config.color_format,
            &initial_scene,
            render_options,
            assets,
            asset_source,
            None,
        )?;

        if start_intent == WindowStartIntent::Menu {
            host.enter_mono_title(device, queue)?;
        }
        let mut ui = match start_intent {
            WindowStartIntent::InWorld => GameUiHost::new_ingame(),
            WindowStartIntent::Menu => GameUiHost::new(),
        };
        ui.set_join_remote_addr(
            scene
                .remote_addr
                .clone()
                .unwrap_or_else(|| mclone_ui::DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
        );
        ui.set_v2_debug_overlay(ui_v2_debug_overlay);
        host.configure_mono_ui(ui, MonoUiContext::default());
        host.set_frame_host_kind(FrameHostKind::DesktopFlatWinit);
        host.set_display_refresh_hz(None);

        let target_size = [target_size[0].max(1), target_size[1].max(1)];
        Ok(Self {
            host,
            depth: ChunkDepthTarget::new(device, target_size[0], target_size[1]),
            target_size,
            adaptive_render_admission_budget: scene.adaptive_render_admission_budget,
            frame_pipeline_accounting: FramePipelineAccountant::new(
                xr_frame_pipeline_accounting_config(None),
            ),
            scale_presentation: FlatScalePresentation::new(
                device,
                target_size,
                render_config.color_format,
                render_config.render_scale,
            ),
            render_config,
            arm_mouse_lock_after_start: false,
            deferred_mouse_lock_request: None,
        })
    }

    pub(crate) fn set_audio_engine(&mut self, audio: Option<AudioEngine>) {
        self.host.set_audio_engine(audio);
    }

    pub(crate) fn drive_until_idle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_winit_startup_idle_target"),
            size: wgpu::Extent3d {
                width: self.target_size[0],
                height: self.target_size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.render_config.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let start = std::time::Instant::now();
        loop {
            let render_view = self.host.mono_render_view(self.target_size)?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_winit_startup_idle_encoder"),
            });
            self.host.render_mono_scene_frame(
                RenderFrameContext::new(
                    device,
                    queue,
                    &mut encoder,
                    mclone_render::target::RenderFrameTarget::color(&view, self.target_size),
                ),
                &self.depth,
                render_view,
                MonoUiPresentation::None,
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            device.poll(wgpu::PollType::Poll)?;
            if self.host.local_startup_complete()
                && self.host.pending_stream_work(render_view.camera_position) == 0
            {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                anyhow::bail!("desktop Mono scene did not reach idle startup before {timeout:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub(crate) fn rebuild_render_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
    ) -> Result<()> {
        self.host.rebuild_mono_render_resources(
            device,
            queue,
            assets.actor_textures.atlas.clone(),
            &assets.actor_textures.figures,
            asset_source,
        )
    }

    pub(crate) fn set_render_config(
        &mut self,
        device: &wgpu::Device,
        output_size: [u32; 2],
        render_config: RenderConfig,
    ) {
        self.render_config = render_config;
        self.scale_presentation = FlatScalePresentation::new(
            device,
            output_size,
            render_config.color_format,
            render_config.render_scale,
        );
    }

    pub(crate) fn resize(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        self.resize_depth(device, size);
        self.host
            .set_mono_ui_scale(GuiScale::from_pixels(size[0], size[1]));
    }

    fn resize_depth(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        if self.target_size != size {
            self.target_size = size;
            self.depth = ChunkDepthTarget::new(device, size[0], size[1]);
        }
    }

    pub(crate) fn render(
        &mut self,
        frame: RenderFrameContext<'_>,
        context: MonoUiContext,
    ) -> Result<MonoSceneFrameSummary> {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target: output_target,
        } = frame;
        if let Some(scale) = &mut self.scale_presentation {
            scale.resize(device, output_target.size, self.render_config.render_scale);
        }
        let (world_size, ui_size) = winit_frame_sizes(
            output_target.size,
            self.scale_presentation.is_some(),
            self.render_config.render_scale,
        );
        self.resize_depth(device, world_size);
        self.host
            .set_mono_ui_scale(GuiScale::from_pixels(ui_size[0], ui_size[1]));
        let world_target = self
            .scale_presentation
            .as_ref()
            .map_or(output_target, |scale| scale.render_target(output_target));
        self.host.set_display_refresh_hz(
            self.adaptive_render_admission_budget
                .then_some(context.pacing_debug.target_frame_ms)
                .flatten()
                .map(|period_ms| (1_000.0 / period_ms) as f32),
        );
        self.host.set_mono_ui_context(context);
        let view = self.host.mono_render_view(world_target.size)?;
        let split_native_ui = self.scale_presentation.is_some();
        let mut summary = self.host.render_mono_scene_frame(
            RenderFrameContext::new(device, queue, encoder, world_target),
            &self.depth,
            view,
            if split_native_ui {
                MonoUiPresentation::None
            } else {
                MonoUiPresentation::ScreenSpaceHud
            },
        )?;
        if let Some(scale) = &self.scale_presentation {
            scale.present(encoder, output_target);
            let (gui_command_count, retained_cache) =
                self.host
                    .render_mono_screen_space_ui(RenderFrameContext::new(
                        device,
                        queue,
                        encoder,
                        output_target,
                    ))?;
            summary.render.gui_command_count = gui_command_count;
            summary.render.flat_hud_retained_cache = retained_cache;
        }
        if self.arm_mouse_lock_after_start
            && self.host.has_runtime()
            && self.host.local_startup_complete()
        {
            self.arm_mouse_lock_after_start = false;
            self.deferred_mouse_lock_request = Some(true);
        }
        Ok(summary)
    }

    pub(crate) fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        from_pointer_click: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitUiActionOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.host.apply_mono_ui_action(
            action,
            from_pointer_click,
            device,
            queue,
            &mut effects,
        )?;
        if scene.session_start_requested {
            if self.host.has_runtime() && self.host.local_startup_complete() {
                effects.outcome.mouse_lock_requested = Some(true);
            } else {
                self.arm_mouse_lock_after_start = true;
            }
        }
        Ok(WinitUiActionOutcome {
            scene,
            host: effects.outcome,
        })
    }

    pub(crate) fn take_deferred_mouse_lock_request(&mut self) -> Option<bool> {
        self.deferred_mouse_lock_request.take()
    }

    pub(crate) fn record_frame_pipeline(
        &mut self,
        frame_wall_ms: f64,
        rendered: bool,
        summary: Option<MonoSceneFrameSummary>,
    ) {
        let budget = self.host.latest_budget_decision_panel();
        let (report, revision) = record_mono_frame_pipeline(
            &mut self.frame_pipeline_accounting,
            frame_wall_ms,
            rendered,
            summary,
            budget,
        );
        self.host.set_frame_pipeline_report(report, revision);
    }

    pub(crate) fn host(&self) -> &DesktopMonoSceneHost {
        &self.host
    }

    pub(crate) fn host_mut(&mut self) -> &mut DesktopMonoSceneHost {
        &mut self.host
    }

    pub(crate) fn has_runtime(&self) -> bool {
        self.host.has_runtime()
    }

    pub(crate) fn ui_is_active(&self) -> bool {
        self.host.mono_ui_is_active()
    }

    pub(crate) fn ui_key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        self.host.mono_ui_key_pressed(key)
    }

    pub(crate) fn ui_pointer_down(&mut self, point: Point) -> bool {
        self.host.mono_ui_pointer_down(point)
    }

    pub(crate) fn ui_pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.host.mono_ui_pointer_up(point)
    }

    pub(crate) fn ui_pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.host.mono_ui_pointer_move(point)
    }

    pub(crate) fn clear_ui_input(&mut self) {
        self.host.clear_mono_ui_input();
    }

    pub(crate) fn ui_v2_is_active(&self) -> bool {
        self.host.mono_ui_v2_is_active()
    }

    pub(crate) fn ui_v2_debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.host.mono_ui_debug_snapshot()
    }

    pub(crate) fn open_pause_menu(&mut self) {
        self.host.open_mono_pause_menu();
    }

    pub(crate) fn open_block_palette(&mut self) {
        self.host.open_mono_block_palette();
    }

    pub(crate) fn apply_movement_frame(&mut self, frame: FlatInputFrame, dt_seconds: f64) -> bool {
        self.host.apply_mono_movement_frame(frame, dt_seconds)
    }

    pub(crate) fn apply_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        self.host.apply_mono_look_frame(frame)
    }

    pub(crate) fn clear_camera_input(&mut self) {
        self.host.clear_mono_camera_input();
    }

    pub(crate) fn begin_blink_debug(&mut self) -> bool {
        self.host.begin_mono_blink_debug()
    }

    pub(crate) fn update_blink_debug(&mut self) -> bool {
        self.host.update_mono_blink_debug()
    }

    pub(crate) fn clear_blink_debug(&mut self) {
        self.host.clear_mono_blink_debug();
    }

    pub(crate) fn commit_blink_debug(&mut self) -> Result<MonoBlinkCommitStatus> {
        self.host.commit_mono_blink_debug()
    }

    pub(crate) fn commit_player_pose(&mut self) -> Result<bool> {
        self.host.commit_mono_player_pose()
    }

    pub(crate) fn toggle_camera_view(&mut self) -> EngineCameraViewMode {
        self.host.toggle_mono_camera_view()
    }

    pub(crate) fn toggle_movement_mode(&mut self) -> EngineCameraMovementMode {
        self.host.toggle_mono_movement_mode()
    }

    pub(crate) fn adjust_camera_speed(&mut self, amount: f64) {
        self.host.adjust_mono_camera_speed(amount);
    }

    pub(crate) fn camera_speed_blocks_per_second(&self) -> f64 {
        self.host.mono_camera_speed_blocks_per_second()
    }

    pub(crate) fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.host.select_mono_hotbar_slot(slot)
    }

    pub(crate) fn handle_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> Result<MonoWorldActionStatus> {
        self.host.handle_mono_world_action(action)
    }

    pub(crate) fn shoot_debug_physics_cube(&mut self) -> Result<bool> {
        self.host.shoot_mono_debug_physics_cube()
    }
}

fn winit_frame_sizes(
    output_size: [u32; 2],
    scaled_world: bool,
    render_scale: f32,
) -> ([u32; 2], [u32; 2]) {
    let world_size = if scaled_world {
        scaled_frame_size(output_size, render_scale)
    } else {
        output_size
    };
    (world_size, output_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_world_keeps_screen_space_ui_at_output_resolution() {
        let (world, ui) = winit_frame_sizes([1920, 1080], true, 0.5);

        assert_eq!(world, [960, 540]);
        assert_eq!(ui, [1920, 1080]);
    }

    #[test]
    fn unscaled_world_and_ui_share_output_resolution() {
        let (world, ui) = winit_frame_sizes([1280, 720], false, 1.0);

        assert_eq!(world, [1280, 720]);
        assert_eq!(ui, [1280, 720]);
    }
}
