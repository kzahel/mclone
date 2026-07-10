//! Smoke-only browser driver for Tactical 170 Slice 4.
//!
//! This owner is intentionally not used by the production app. It proves that
//! the browser resource rim can construct and step the shared scene host before
//! Slice 5 atomically replaces the old production orchestrator.

use anyhow::Result;
use glam::Vec3;
use mclone_app_runtime::chunk_tracking_radius_for_render_distance;
use mclone_app_runtime::client_experience::web_client_experience_profile;
use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_input::{FlatInputAction, FlatInputFrame, LookDelta, TouchControlsMode};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_scene::{
    HostEffects, McloneSceneHost, McloneSceneHostOptions, MonoSceneFrameSummary, MonoUiContext,
    MonoUiPresentation, MonoWorldActionStatus,
};
use mclone_ui::{GameScreen, GameUiAction};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::web_canvas::{
    WebCanvasContext, WebSceneRuntimeService, prepare_web_scene_assets_from_pack,
};
use crate::web_scene_protocol::{
    WebSceneFrameAdmission, WebSceneFrameDriverPolicy, WebSceneFrameState,
    WebScenePlatformServices, WebSceneSessionCompletionDisposition, WebSceneSessionOperation,
    WebSceneSessionOperationResult,
};
use crate::web_server_worker::WebIntegratedServerRunnerConfig;

const PROOF_SEED: i64 = 12_345;
const PROOF_RENDER_DISTANCE: u32 = 2;
const PROOF_DAY_TIME: u64 = 6_000;
const PROOF_RESUME_OBSERVATION_FRAMES: u8 = 8;

#[derive(Debug, Default)]
struct ProofHostEffects {
    mouse_lock_requested: Option<bool>,
    quit_to_title: bool,
    exit: bool,
}

impl HostEffects for ProofHostEffects {
    fn request_mouse_lock(&mut self, requested: bool) -> Result<()> {
        self.mouse_lock_requested = Some(requested);
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, _mode: TouchControlsMode) -> Result<()> {
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        self.quit_to_title = true;
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        self.exit = true;
        Ok(())
    }
}

/// Isolated browser proof owner. The production app never constructs this
/// type; Slice 5 folds its browser rim around the shared host and deletes it.
#[wasm_bindgen]
pub struct WebSceneHostProof {
    context: WebCanvasContext,
    depth: ChunkDepthTarget,
    host: Option<McloneSceneHost>,
    platform: WebScenePlatformServices,
    frame_policy: WebSceneFrameDriverPolicy,
    frame_count: u64,
    rendered_frame_count: u64,
    last_visible_frame_millis: Option<f64>,
    max_frame_gap_millis: f64,
    movement_input_applied: bool,
    dom_input_frame_count: u64,
    interaction_sent: bool,
    interaction_changed: bool,
    settings_effect_applied: bool,
    pause_ui_rendered: bool,
    resized: bool,
    background_save_count: u64,
    resume_frames_remaining: u8,
    max_resume_poll_updates: usize,
    max_resume_drop_backlog: usize,
    shutdown_complete: bool,
}

#[wasm_bindgen]
impl WebSceneHostProof {
    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(
        &mut self,
        now_millis: f64,
        forward: bool,
        look_x: f32,
        look_y: f32,
        dom_input: bool,
    ) -> Result<JsValue, JsValue> {
        self.platform.observe_frame_time_millis(now_millis);
        let admission = self.frame_policy.admit_frame(now_millis);
        let WebSceneFrameAdmission::Render {
            delta_seconds,
            first_after_resume,
        } = admission
        else {
            return self.report(None, false, 0.0, false).map_err(JsValue::from);
        };

        if let Some(previous) = self.last_visible_frame_millis {
            self.max_frame_gap_millis = self
                .max_frame_gap_millis
                .max((now_millis - previous).max(0.0));
        }
        self.last_visible_frame_millis = Some(now_millis);
        self.frame_count = self.frame_count.saturating_add(1);
        if dom_input {
            self.dom_input_frame_count = self.dom_input_frame_count.saturating_add(1);
        }

        let Some(host) = self.host.as_mut() else {
            self.frame_policy.shutdown();
            return self
                .report(None, false, delta_seconds, first_after_resume)
                .map_err(JsValue::from);
        };
        let input = FlatInputFrame {
            forward,
            look_delta: LookDelta {
                x: look_x,
                y: look_y,
            },
            ..FlatInputFrame::default()
        };
        self.movement_input_applied |= host.apply_mono_look_frame(input);
        self.movement_input_applied |= host.apply_mono_movement_frame(input, delta_seconds);
        if forward {
            let _ = host.commit_mono_player_pose().map_err(js_error)?;
        }

        let surface_texture = match self.context.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(error) => {
                self.frame_policy
                    .require_restart(format!("WebGPU surface acquisition failed: {error}"));
                return self
                    .report(None, false, delta_seconds, first_after_resume)
                    .map_err(JsValue::from);
            }
        };
        let view = surface_texture.texture.create_view(&Default::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_web_scene_host_proof_encoder"),
                });
        let render_view = host
            .mono_render_view([self.context.width, self.context.height])
            .map_err(js_error)?;
        let summary = host
            .render_mono_scene_frame(
                RenderFrameContext::new(
                    &self.context.device,
                    &self.context.queue,
                    &mut encoder,
                    RenderFrameTarget::color(&view, [self.context.width, self.context.height]),
                ),
                &self.depth,
                render_view,
                MonoUiPresentation::ScreenSpaceHud,
            )
            .map_err(js_error)?;
        self.context.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        self.rendered_frame_count = self.rendered_frame_count.saturating_add(1);
        if host.mono_ui_screen() == Some(GameScreen::Pause) && summary.render.gui_command_count > 0
        {
            self.pause_ui_rendered = true;
        }
        if self.resume_frames_remaining > 0 {
            self.max_resume_poll_updates = self
                .max_resume_poll_updates
                .max(summary.upload.poll_updates);
            self.max_resume_drop_backlog = self
                .max_resume_drop_backlog
                .max(summary.upload.poll_client_deferred_chunk_drop_backlog_items);
            self.resume_frames_remaining -= 1;
        }
        self.report(Some(&summary), true, delta_seconds, first_after_resume)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setHidden)]
    pub fn set_hidden(&mut self, hidden: bool) -> Result<JsValue, JsValue> {
        if hidden && self.frame_policy.set_hidden(true) {
            self.last_visible_frame_millis = None;
            if let Some(host) = self.host.as_mut() {
                let _ = host.on_background().map_err(js_error)?;
                self.background_save_count = self.background_save_count.saturating_add(1);
            }
        } else if !hidden && self.frame_policy.set_hidden(false) {
            self.last_visible_frame_millis = None;
            self.resume_frames_remaining = PROOF_RESUME_OBSERVATION_FRAMES;
        }
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = resize)]
    pub fn resize(&mut self, width: u32, height: u32) -> Result<JsValue, JsValue> {
        if self.context.resize(width, height) {
            self.depth = ChunkDepthTarget::new(
                &self.context.device,
                self.context.width,
                self.context.height,
            );
            self.resized = true;
        }
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setPauseMenu)]
    pub fn set_pause_menu(&mut self, visible: bool) -> Result<JsValue, JsValue> {
        let host = self
            .host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?;
        host.set_mono_ui_screen(visible.then_some(GameScreen::Pause));
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = exerciseSettingsEffect)]
    pub fn exercise_settings_effect(&mut self) -> Result<JsValue, JsValue> {
        let host = self
            .host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?;
        let mut effects = ProofHostEffects::default();
        let outcome = host
            .apply_mono_ui_action(
                GameUiAction::ToggleCrosshair,
                false,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        self.settings_effect_applied = !outcome.scene_replaced
            && !outcome.session_start_requested
            && outcome.clear_gameplay_input;
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = exerciseBlockInteraction)]
    pub fn exercise_block_interaction(&mut self) -> Result<JsValue, JsValue> {
        let host = self
            .host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?;
        let target = find_interaction_surface(host)
            .ok_or_else(|| JsValue::from_str("no loaded surface for scene-host interaction"))?;
        aim_host_at_block(host, target);
        match host
            .handle_mono_world_action(FlatInputAction::Attack)
            .map_err(js_error)?
        {
            MonoWorldActionStatus::Sent { changed, .. } => {
                self.interaction_sent = true;
                self.interaction_changed |= changed;
            }
            status => {
                let camera = host.camera_frame_state().camera;
                return Err(JsValue::from_str(&format!(
                    "scene-host interaction did not issue a command: {status:?}; target={target:?}; camera={camera:?}"
                )));
            }
        }
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = frameFirstActor)]
    pub fn frame_first_actor(&mut self) -> Result<JsValue, JsValue> {
        let host = self
            .host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?;
        if let Some(actor) = host
            .mono_client()
            .and_then(|client| client.actor_presentations().first().copied())
        {
            let target = Vec3::new(
                actor.feet_position.x as f32,
                actor.feet_position.y as f32 + 1.0,
                actor.feet_position.z as f32,
            );
            let eye = target + Vec3::new(-2.2, 1.4, -4.8);
            set_host_camera_look_at(host, eye, target);
        }
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = simulateSurfaceLoss)]
    pub fn simulate_surface_loss(&mut self) -> Result<JsValue, JsValue> {
        self.frame_policy
            .require_restart("simulated WebGPU surface loss; recreate scene-host proof");
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = shutdown)]
    pub fn shutdown(&mut self) -> Result<JsValue, JsValue> {
        if let Some(mut host) = self.host.take() {
            let operation = self.platform.lifecycle_mut().begin_shutdown();
            host.enter_mono_title(&self.context.device, &self.context.queue)
                .map_err(js_error)?;
            let disposition = self.platform.lifecycle_mut().complete(
                mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                    token: operation.token,
                    result: Ok(WebSceneSessionOperationResult::Shutdown),
                },
            );
            self.shutdown_complete = disposition == WebSceneSessionCompletionDisposition::Applied;
        }
        self.frame_policy.shutdown();
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub async fn mclone_web_create_scene_host_proof(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    compiler_wake: js_sys::Function,
) -> Result<WebSceneHostProof, JsValue> {
    let active_assets =
        prepare_web_scene_assets_from_pack(asset_pack_bytes.to_vec()).map_err(JsValue::from)?;
    let context = WebCanvasContext::new(canvas).await.map_err(JsValue::from)?;
    let center = ChunkPos::new(0, 0);
    let mut runtime = crate::WebRuntime::web_worker_integrated_at(
        WebIntegratedServerRunnerConfig::new(
            PROOF_SEED,
            server_worker_url,
            server_job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        ),
        center,
    )
    .await
    .map_err(JsValue::from)?;
    runtime
        .request_chunk_view_deferred(
            center,
            PROOF_RENDER_DISTANCE,
            chunk_tracking_radius_for_render_distance(PROOF_RENDER_DISTANCE),
        )
        .map_err(JsValue::from)?;
    let runtime = WebSceneRuntimeService::new(runtime, active_assets.mesh.clone(), compiler_wake)
        .into_scene_session_runtime(
            mclone_app_runtime::session::ActiveSessionDescriptor::LocalWorld {
                seed: PROOF_SEED,
                id: None,
                display_name: None,
            },
        );
    let (mut platform, clock, catalog_operations) = WebScenePlatformServices::new();
    let start = platform
        .lifecycle_mut()
        .begin_start(WebSceneSessionOperation::StartLocal {
            seed: PROOF_SEED,
            world_id: None,
        });
    let start_disposition = platform.lifecycle_mut().complete(
        mclone_app_runtime::platform_operation::PlatformOperationCompletion {
            token: start.token,
            result: Ok(WebSceneSessionOperationResult::Started(
                mclone_app_runtime::session::ActiveSessionDescriptor::LocalWorld {
                    seed: PROOF_SEED,
                    id: None,
                    display_name: None,
                },
            )),
        },
    );
    if start_disposition != WebSceneSessionCompletionDisposition::Applied {
        return Err(JsValue::from_str(
            "scene-host proof lifecycle start was not applied",
        ));
    }
    let scene = McloneSceneHostOptions {
        seed: PROOF_SEED,
        chunk_x: center.x,
        chunk_z: center.z,
        render_distance: PROOF_RENDER_DISTANCE,
        day_time_override: Some(PROOF_DAY_TIME),
        freeze_time: true,
        use_initial_spawn_center: false,
        far_lod: Default::default(),
        startup_lod_prewarm: false,
        ..McloneSceneHostOptions::default()
    };
    let mut host = McloneSceneHost::with_scene_runtime(
        &context.device,
        &context.queue,
        context.format,
        clock,
        scene,
        runtime,
        TexturedSectionRenderOptions::default(),
        active_assets,
        web_client_experience_profile(),
        Some(catalog_operations),
        None,
    )
    .map_err(js_error)?;
    host.set_mono_ui_context(MonoUiContext::default());
    host.set_mono_ui_screen(None);
    host.force_mono_day_time(PROOF_DAY_TIME);
    host.set_mono_capture_camera(Vec3d::new(8.0, 112.0, 8.0), 0.55, -0.45, 4.3);
    let _ = host.toggle_mono_movement_mode();
    let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
    Ok(WebSceneHostProof {
        context,
        depth,
        host: Some(host),
        platform,
        frame_policy: WebSceneFrameDriverPolicy::default(),
        frame_count: 0,
        rendered_frame_count: 0,
        last_visible_frame_millis: None,
        max_frame_gap_millis: 0.0,
        movement_input_applied: false,
        dom_input_frame_count: 0,
        interaction_sent: false,
        interaction_changed: false,
        settings_effect_applied: false,
        pause_ui_rendered: false,
        resized: false,
        background_save_count: 0,
        resume_frames_remaining: 0,
        max_resume_poll_updates: 0,
        max_resume_drop_backlog: 0,
        shutdown_complete: false,
    })
}

impl WebSceneHostProof {
    fn report(
        &self,
        summary: Option<&MonoSceneFrameSummary>,
        rendered: bool,
        delta_seconds: f64,
        first_after_resume: bool,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        proof_set_bool(&object, "ok", true)?;
        proof_set_string(&object, "owner", "McloneSceneHost")?;
        proof_set_string(&object, "state", self.frame_policy.state().label())?;
        if let WebSceneFrameState::RestartRequired { reason } = self.frame_policy.state() {
            proof_set_string(&object, "restartReason", reason)?;
        }
        proof_set_bool(&object, "rendered", rendered)?;
        proof_set_bool(&object, "hostOwnedFrameAssembly", summary.is_some())?;
        proof_set_bool(&object, "audioCapabilityAbsent", true)?;
        proof_set_bool(&object, "teleportCapabilityAbsent", true)?;
        proof_set_number(&object, "frameCount", self.frame_count as f64)?;
        proof_set_number(
            &object,
            "renderedFrameCount",
            self.rendered_frame_count as f64,
        )?;
        proof_set_number(&object, "deltaSeconds", delta_seconds)?;
        proof_set_bool(&object, "firstAfterResume", first_after_resume)?;
        proof_set_number(&object, "maxFrameGapMs", self.max_frame_gap_millis)?;
        proof_set_number(
            &object,
            "hiddenFrameSkips",
            self.frame_policy.hidden_frame_skips() as f64,
        )?;
        proof_set_number(
            &object,
            "resumeCount",
            self.frame_policy.resume_count() as f64,
        )?;
        proof_set_number(
            &object,
            "resumeFramesRemaining",
            f64::from(self.resume_frames_remaining),
        )?;
        proof_set_number(
            &object,
            "maxResumePollUpdates",
            self.max_resume_poll_updates as f64,
        )?;
        proof_set_number(
            &object,
            "maxResumeDeferredDropBacklogItems",
            self.max_resume_drop_backlog as f64,
        )?;
        proof_set_number(
            &object,
            "backgroundSaveCount",
            self.background_save_count as f64,
        )?;
        proof_set_bool(&object, "movementInputApplied", self.movement_input_applied)?;
        proof_set_number(
            &object,
            "domInputFrameCount",
            self.dom_input_frame_count as f64,
        )?;
        proof_set_bool(&object, "interactionSent", self.interaction_sent)?;
        proof_set_bool(&object, "interactionChanged", self.interaction_changed)?;
        proof_set_bool(
            &object,
            "settingsEffectApplied",
            self.settings_effect_applied,
        )?;
        proof_set_bool(&object, "pauseUiRendered", self.pause_ui_rendered)?;
        proof_set_bool(&object, "resized", self.resized)?;
        proof_set_number(&object, "width", self.context.width as f64)?;
        proof_set_number(&object, "height", self.context.height as f64)?;
        proof_set_bool(&object, "shutdownComplete", self.shutdown_complete)?;

        if let Some(host) = self.host.as_ref() {
            let camera = host.camera_frame_state();
            proof_set_number(&object, "cameraX", camera.camera.eye.x)?;
            proof_set_number(&object, "cameraY", camera.camera.eye.y)?;
            proof_set_number(&object, "cameraZ", camera.camera.eye.z)?;
            proof_set_string(&object, "movementMode", camera.movement_mode_label())?;
            proof_set_string(&object, "collisionMode", camera.collision_mode_label())?;
            let camera_position = Vec3::new(
                camera.camera.eye.x as f32,
                camera.camera.eye.y as f32,
                camera.camera.eye.z as f32,
            );
            proof_set_number(
                &object,
                "pendingStreamWork",
                host.pending_stream_work(camera_position) as f64,
            )?;
            if let Some(stats) = host.runtime_stats() {
                proof_set_string(&object, "hostMode", stats.host_mode.label())?;
                proof_set_string(
                    &object,
                    "runnerKind",
                    stats.server_runner_kind.map_or("none", |kind| kind.label()),
                )?;
                proof_set_number(&object, "loadedChunkCount", stats.loaded_chunks as f64)?;
                proof_set_number(
                    &object,
                    "serverCommandQueueDepth",
                    stats.server_command_queue_depth as f64,
                )?;
                proof_set_number(
                    &object,
                    "serverUpdateQueueDepth",
                    stats.server_update_queue_depth as f64,
                )?;
                proof_set_number(&object, "serverPendingJobs", stats.pending_jobs as f64)?;
                proof_set_number(
                    &object,
                    "serverPendingPublications",
                    stats.pending_publications as f64,
                )?;
                proof_set_number(
                    &object,
                    "pendingRenderChunks",
                    stats.pending_render_chunks as f64,
                )?;
                proof_set_number(
                    &object,
                    "pendingCompileJobCount",
                    stats.pending_render_compile_jobs as f64,
                )?;
            }
            if let Some(diagnostics) = host.runtime_poll_diagnostics() {
                proof_set_string(
                    &object,
                    "runnerTransportKind",
                    diagnostics.runner_frame_metrics.transport_kind.label(),
                )?;
                proof_set_string(
                    &object,
                    "worldgenTransportKind",
                    diagnostics
                        .worldgen_job_frame_metrics
                        .transport_kind
                        .label(),
                )?;
                proof_set_string(
                    &object,
                    "lightTransportKind",
                    diagnostics
                        .light_status_job_frame_metrics
                        .transport_kind
                        .label(),
                )?;
                proof_set_bool(
                    &object,
                    "updatePumpStalled",
                    diagnostics.update_pump_stalled,
                )?;
            }
        }
        if let Some(summary) = summary {
            proof_set_number(&object, "sectionCount", summary.render.section_count as f64)?;
            proof_set_number(
                &object,
                "drawnSectionCount",
                summary.render.drawn_section_count as f64,
            )?;
            proof_set_number(
                &object,
                "drawnIndexCount",
                summary.render.drawn_index_count as f64,
            )?;
            proof_set_number(&object, "actorCount", summary.render.actor_count as f64)?;
            proof_set_number(
                &object,
                "drawnActorCount",
                summary.render.drawn_actor_count as f64,
            )?;
            proof_set_number(
                &object,
                "guiCommandCount",
                summary.render.gui_command_count as f64,
            )?;
            proof_set_number(&object, "pollUpdates", summary.upload.poll_updates as f64)?;
            proof_set_number(
                &object,
                "deferredDropBacklogItems",
                summary.upload.poll_client_deferred_chunk_drop_backlog_items as f64,
            )?;
            proof_set_bool(&object, "playable", summary.render.drawn_section_count > 0)?;
        }
        Ok(object.into())
    }
}

fn find_interaction_surface(host: &McloneSceneHost) -> Option<BlockPos> {
    let camera = host.camera_frame_state().camera;
    let base_x = camera.eye.x.floor() as i32;
    let base_z = camera.eye.z.floor() as i32;
    for dz in -8..=8 {
        for dx in -8..=8 {
            let x = base_x + dx;
            let z = base_z + dz;
            let Some(y) = host.mono_highest_non_air_block_y_at_world(x, z) else {
                continue;
            };
            let pos = BlockPos::new(x, y, z);
            if host
                .mono_client()
                .and_then(|client| client.block_state_at_block_pos(pos))
                .is_some()
            {
                return Some(pos);
            }
        }
    }
    None
}

fn aim_host_at_block(host: &mut McloneSceneHost, block: BlockPos) {
    let target = Vec3::new(
        block.x as f32 + 0.5,
        block.y as f32 + 0.5,
        block.z as f32 + 0.5,
    );
    let eye = target + Vec3::new(0.0, 1.8, -1.3);
    set_host_camera_look_at(host, eye, target);
}

fn set_host_camera_look_at(host: &mut McloneSceneHost, eye: Vec3, target: Vec3) {
    let direction = (target - eye).normalize_or_zero();
    let yaw = direction.x.atan2(direction.z);
    let pitch = direction.y.clamp(-1.0, 1.0).asin();
    host.set_mono_capture_camera(
        Vec3d::new(f64::from(eye.x), f64::from(eye.y), f64::from(eye.z)),
        f64::from(yaw),
        f64::from(pitch),
        4.3,
    );
}

fn proof_set_bool(object: &js_sys::Object, key: &str, value: bool) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_bool(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set proof field {key}: {error:?}"))
}

fn proof_set_number(object: &js_sys::Object, key: &str, value: f64) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_f64(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set proof field {key}: {error:?}"))
}

fn proof_set_string(object: &js_sys::Object, key: &str, value: &str) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_str(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set proof field {key}: {error:?}"))
}

fn js_error(error: anyhow::Error) -> JsValue {
    JsValue::from_str(&format!("{error:#}"))
}
