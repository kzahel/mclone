//! Thin desktop surface/cadence driver over the shared scene host's Mono
//! topology (tactical 168 Slice 7c).

use anyhow::Result;
use mclone_app_runtime::frame_pipeline_accounting::FramePipelineAccountant;
use mclone_app_runtime::frame_render::{
    FlatScalePresentation, FlatSurfacePresentation, scaled_frame_size,
};
use mclone_audio::AudioEngine;
use mclone_diagnostics::FrameHostKind;
use mclone_input::{
    ControllerInputPreferences, KeyboardKey, MouseWheelDirection, PointerButton, TouchControlsMode,
};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::color_profile::RenderConfig;
use mclone_render::target::RenderFrameContext;
use mclone_render_session::EngineCameraMovementMode;
use mclone_scene::{
    HostEffects, MonoBlinkCommitStatus, MonoInputDisposition, MonoInteractiveInputRouter,
    MonoSceneFrameSummary, MonoUiActionOutcome, MonoUiContext, MonoUiPresentation,
    record_mono_frame_pipeline, xr_frame_pipeline_accounting_config,
};
use mclone_ui::{
    GameUiAction, GameUiHost, GameWorldRenderScaleMode, GuiScale, Point, UiDebugSnapshot,
};

use crate::cli::{SceneOptions, WindowStartIntent};
use crate::desktop_scene_host::{DesktopSceneHost, create_desktop_scene_host};
use crate::scene_runtime::WindowSceneAssets;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WinitHostEffectOutcome {
    pub(crate) mouse_lock_requested: Option<bool>,
    pub(crate) cycle_frame_pacing: bool,
    pub(crate) cycle_fps_cap: bool,
    pub(crate) world_render_scale_mode: Option<GameWorldRenderScaleMode>,
    pub(crate) touch_controls_mode: Option<TouchControlsMode>,
    pub(crate) quit_to_title: bool,
    pub(crate) exit: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WinitUiActionOutcome {
    pub(crate) scene: MonoUiActionOutcome,
    pub(crate) host: WinitHostEffectOutcome,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WinitInputOutcome {
    pub(crate) scene: MonoInputDisposition,
    pub(crate) host: WinitHostEffectOutcome,
    pub(crate) controller_activity: bool,
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

    fn set_world_render_scale_mode(&mut self, mode: GameWorldRenderScaleMode) -> Result<()> {
        self.outcome.world_render_scale_mode = Some(mode);
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
    host: DesktopSceneHost,
    interactive_input: MonoInteractiveInputRouter,
    depth: ChunkDepthTarget,
    target_size: [u32; 2],
    adaptive_render_admission_budget: bool,
    frame_pipeline_accounting: FramePipelineAccountant,
    render_config: RenderConfig,
    scale_presentation: Option<FlatScalePresentation>,
    split_presentation: Option<FlatSurfacePresentation>,
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
        controller_preferences: &ControllerInputPreferences,
    ) -> Result<Self> {
        let mut initial_scene = scene.clone();
        if start_intent == WindowStartIntent::Menu {
            initial_scene.remote_addr = None;
        }
        let mut host = create_desktop_scene_host(
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
            interactive_input: MonoInteractiveInputRouter::with_controller_preferences(
                controller_preferences,
            ),
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
            split_presentation: None,
            render_config,
            arm_mouse_lock_after_start: false,
            deferred_mouse_lock_request: None,
        })
    }

    pub(crate) fn set_audio_engine(&mut self, audio: Option<AudioEngine>) {
        self.host.set_audio_output(audio.map_or_else(
            || mclone_audio::AudioOutputCapability::Unavailable,
            mclone_audio::AudioOutputCapability::available,
        ));
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
        self.split_presentation = None;
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
        mut context: MonoUiContext,
    ) -> Result<MonoSceneFrameSummary> {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target: output_target,
        } = frame;
        self.host.set_display_refresh_hz(
            self.adaptive_render_admission_budget
                .then_some(context.pacing_debug.target_frame_ms)
                .flatten()
                .map(|period_ms| (1_000.0 / period_ms) as f32),
        );
        context.controller_layout = self
            .interactive_input
            .latest_controller_actions()
            .active_controller_layout
            .unwrap_or(mclone_input::ControllerLayoutFamily::Unknown);
        self.host.set_mono_ui_context(context);
        let summary = if let Some(layout) = self.host.auxiliary_split_layout(output_target.size)? {
            let primary = layout.panes()[0];
            self.host
                .set_mono_ui_scale(GuiScale::from_pixels(primary.width, primary.height));
            match &mut self.split_presentation {
                Some(presentation) => presentation.resize(device, layout),
                slot @ None => {
                    *slot = Some(FlatSurfacePresentation::new(
                        device,
                        self.render_config.color_format,
                        layout,
                    ));
                }
            }
            let presentation = self
                .split_presentation
                .as_ref()
                .expect("split presentation initialized from active layout");
            let summary = self.host.render_auxiliary_split_surface_frame(
                device,
                queue,
                encoder,
                presentation,
            )?;
            presentation.present(encoder, output_target);
            summary
        } else {
            if !self.host.auxiliary_split_mode().is_split() {
                self.split_presentation = None;
            }
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
            summary
        };
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

    pub(crate) fn route_key(
        &mut self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_key(
            &mut self.host,
            key,
            pressed,
            repeat,
            device,
            queue,
            &mut effects,
        )?;
        Ok(self.finish_input_outcome(scene, effects))
    }

    pub(crate) fn route_pointer_button(
        &mut self,
        button: PointerButton,
        pressed: bool,
        point: Option<Point>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_pointer_button(
            &mut self.host,
            button,
            pressed,
            point,
            device,
            queue,
            &mut effects,
        )?;
        Ok(self.finish_input_outcome(scene, effects))
    }

    pub(crate) fn route_pointer_move(
        &mut self,
        point: Point,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_pointer_move(
            &mut self.host,
            point,
            device,
            queue,
            &mut effects,
        )?;
        Ok(self.finish_input_outcome(scene, effects))
    }

    pub(crate) fn route_mouse_motion(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_mouse_motion(
            &mut self.host,
            delta_x,
            delta_y,
            device,
            queue,
            &mut effects,
        )?;
        Ok(self.finish_input_outcome(scene, effects))
    }

    pub(crate) fn route_wheel(
        &mut self,
        direction: MouseWheelDirection,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_wheel(
            &mut self.host,
            direction,
            device,
            queue,
            &mut effects,
        )?;
        Ok(self.finish_input_outcome(scene, effects))
    }

    pub(crate) fn route_controller_poll(
        &mut self,
        poll: crate::desktop_gamepad::DesktopGamepadPoll,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WinitInputOutcome> {
        for (source_id, descriptor) in poll.connected {
            self.interactive_input
                .connect_controller_source(source_id, descriptor);
        }
        for source_id in poll.disconnected {
            self.interactive_input
                .disconnect_controller_source(source_id)?;
        }
        let mut effects = WinitHostEffects::default();
        let scene = self.interactive_input.route_controller_batch(
            &mut self.host,
            &poll.input,
            device,
            queue,
            &mut effects,
        )?;
        let controller_activity = scene.meaningful_controller_activity;
        let mut outcome = self.finish_input_outcome(scene, effects);
        outcome.controller_activity = controller_activity;
        Ok(outcome)
    }

    pub(crate) fn advance_held_input(&mut self, dt_seconds: f64) -> Result<bool> {
        Ok(self
            .interactive_input
            .advance_held_frame(&mut self.host, None, dt_seconds)?
            .changed())
    }

    pub(crate) fn clear_interactive_input(&mut self) {
        self.interactive_input.clear_transient_input();
        self.host.clear_mono_camera_input();
    }

    fn finish_input_outcome(
        &mut self,
        scene: MonoInputDisposition,
        mut effects: WinitHostEffects,
    ) -> WinitInputOutcome {
        if scene.request_pointer_capture_when_ready {
            if self.host.has_runtime() && self.host.local_startup_complete() {
                effects.outcome.mouse_lock_requested = Some(true);
            } else {
                self.arm_mouse_lock_after_start = true;
            }
        }
        WinitInputOutcome {
            scene,
            host: effects.outcome,
            controller_activity: false,
        }
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

    pub(crate) fn host(&self) -> &DesktopSceneHost {
        &self.host
    }

    pub(crate) fn host_mut(&mut self) -> &mut DesktopSceneHost {
        &mut self.host
    }

    pub(crate) fn has_runtime(&self) -> bool {
        self.host.has_runtime()
    }

    pub(crate) fn ui_is_active(&self) -> bool {
        self.host.mono_ui_is_active()
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

    pub(crate) fn toggle_walk_fly_movement_mode(&mut self) -> EngineCameraMovementMode {
        self.host.toggle_mono_walk_fly_movement_mode()
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
