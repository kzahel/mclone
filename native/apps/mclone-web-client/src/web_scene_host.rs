//! Thin production browser driver around the shared scene host.
//!
//! This module owns only canvas/surface resources, browser lifecycle facts,
//! typed asynchronous operation completion, and wasm-bindgen reports. Gameplay,
//! session policy, camera semantics, streaming admission, frame assembly, UI,
//! and asset epochs remain in `McloneSceneHost`.

use std::collections::HashMap;

use anyhow::Result;
use glam::Vec3;
use mclone_app_runtime::chunk_tracking_radius_for_render_distance;
use mclone_app_runtime::client_experience::web_client_experience_profile;
use mclone_app_runtime::platform_operation::{PlatformOperationCompletion, PlatformOperationToken};
use mclone_app_runtime::prepared_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, MINECRAFT_REFERENCE_PACK_ID,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionState, RemoteSessionEndpoint, SessionRuntimeKind,
    SessionStartRequest,
};
use mclone_app_runtime::world_catalog::{
    LocalWorldCreateOptions, LocalWorldId, WorldCatalogError, WorldCatalogErrorKind,
    WorldCatalogRequest,
};
use mclone_assets::PackedAssetSource;
use mclone_client::BlockInteractionTarget;
use mclone_core::{BlockPos, ChunkPos, Direction, Vec3d};
use mclone_input::{
    FlatInputAction, FlatInputFrame, LookDelta, MovementImpulse, TouchControlsMode,
};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_scene::{
    AssetReplacementStatus, ExternalSceneSessionStart, HostEffects, McloneSceneHost,
    McloneSceneHostOptions, MonoSceneFrameSummary, MonoUiContext, MonoUiPresentation,
    MonoWorldActionStatus,
};
use mclone_ui::{
    GameHelpParent, GameOptionsParent, GameScreen, GameTouchSettings, GameUiAction, GuiScale,
    Point, StatusOverlay, TouchJoystickOverlay, TouchOverlay,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::web_canvas::{
    WebCanvasContext, WebSceneRuntimeService, decode_world_catalog_response, gui_key_from_label,
    prepare_web_scene_assets_from_pack, prepare_web_scene_assets_from_selection,
    startup_render_options, ui_action_label, web_asset_pack_catalog,
};
use crate::web_scene_protocol::{
    WebSceneFrameAdmission, WebSceneFrameDriverPolicy, WebSceneFrameState,
    WebScenePlatformServices, WebSceneSessionCompletionDisposition, WebSceneSessionOperation,
    WebSceneSessionOperationResult,
};

const WEB_ASSET_PACK_PREFERENCE_KEY: &str = "mclone.assetPacks.v1";

struct WebAssetPackPreferenceStorage;

impl mclone_app_runtime::asset_pack_preferences::AssetPackPreferenceStorage
    for WebAssetPackPreferenceStorage
{
    fn load(
        &self,
    ) -> anyhow::Result<Option<mclone_app_runtime::asset_pack_preferences::AssetPackPreference>>
    {
        let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        else {
            return Ok(None);
        };
        storage
            .get_item(WEB_ASSET_PACK_PREFERENCE_KEY)
            .map_err(|error| anyhow::anyhow!("read browser localStorage: {error:?}"))?
            .map(|json| {
                mclone_app_runtime::asset_pack_preferences::AssetPackPreference::from_json(&json)
            })
            .transpose()
    }

    fn store(
        &self,
        preference: &mclone_app_runtime::asset_pack_preferences::AssetPackPreference,
    ) -> anyhow::Result<()> {
        let storage = web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or_else(|| anyhow::anyhow!("browser localStorage is unavailable"))?;
        storage
            .set_item(WEB_ASSET_PACK_PREFERENCE_KEY, &preference.to_json()?)
            .map_err(|error| anyhow::anyhow!("write browser localStorage: {error:?}"))
    }

    fn label(&self) -> &str {
        "browser localStorage mclone.assetPacks.v1"
    }
}
use crate::web_server_worker::WebIntegratedServerRunnerConfig;

const DEFAULT_SEED: i64 = 12_345;
const RESUME_OBSERVATION_FRAMES: u8 = 8;
const TOUCH_LOOK_SENSITIVITY_MIN: f32 = 0.1;
const TOUCH_LOOK_SENSITIVITY_MAX: f32 = 6.0;

#[derive(Debug, Default)]
struct WebHostEffects {
    mouse_lock_requested: Option<bool>,
    touch_controls_mode: Option<TouchControlsMode>,
    quit_to_title: bool,
    exit: bool,
}

impl HostEffects for WebHostEffects {
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

    fn set_touch_controls_mode(&mut self, mode: TouchControlsMode) -> Result<()> {
        self.touch_controls_mode = Some(mode);
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

#[derive(Clone, Copy, Debug, Default)]
struct LastFrameStats {
    section_count: usize,
    drawn_section_count: usize,
    frustum_section_count: usize,
    index_count: u32,
    drawn_index_count: u32,
    actor_count: usize,
    drawn_actor_count: usize,
    gui_command_count: usize,
    flat_hud_rebuilds: usize,
    flat_hud_cache_hits: usize,
    deferred_drop_backlog: usize,
    far_lod_region_draw_count: usize,
    far_lod_uploaded_bytes: usize,
}

/// Browser resource/presentation rim around the one shared scene-policy owner.
#[wasm_bindgen]
pub struct WebSceneHost {
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
    compiler_wake: js_sys::Function,
    asset_pack_file_count: usize,
    pending_asset_pack_file_count: Option<(u64, usize)>,
    status_overlay: StatusOverlay,
    touch_look_sensitivity: f32,
    touch_settings_available: bool,
    touch_controls_mode: TouchControlsMode,
    touch_overlay: TouchOverlay,
    last_action: Option<GameUiAction>,
    last_frame: LastFrameStats,
    command_count: usize,
    update_count: usize,
    interaction_count: usize,
    mesh_build_count: usize,
    catalog_tokens: HashMap<String, PlatformOperationToken>,
    render_color_profile: String,
    last_runner_kind: String,
    startup_camera_reconciled: bool,
}

#[wasm_bindgen]
impl WebSceneHost {
    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(
        &mut self,
        now_millis: f64,
        mouse_delta_x: f32,
        mouse_delta_y: f32,
        keyboard_turn: f32,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        jump: bool,
        descend: bool,
        sneak: bool,
        sprint: bool,
        analog_active: bool,
        analog_left: f32,
        analog_forward: f32,
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
        self.dom_input_frame_count = self.dom_input_frame_count.saturating_add(1);

        let Some(host) = self.host.as_mut() else {
            self.frame_policy.shutdown();
            return self
                .report(None, false, delta_seconds, first_after_resume)
                .map_err(JsValue::from);
        };
        let runtime_ready = host
            .runtime_stats()
            .is_some_and(|stats| stats.loaded_chunks > 0);
        if runtime_ready && !self.startup_camera_reconciled {
            if let Some(target) = find_interaction_surface(host) {
                aim_player_host_at_block(host, target);
                self.startup_camera_reconciled = true;
            }
        }
        let input = FlatInputFrame {
            forward,
            backward,
            left,
            right,
            movement: MovementImpulse {
                left: analog_left,
                forward: analog_forward,
            },
            analog_movement: analog_active.then_some(MovementImpulse {
                left: analog_left,
                forward: analog_forward,
            }),
            keyboard_turn,
            look_delta: LookDelta {
                x: mouse_delta_x,
                y: mouse_delta_y,
            },
            jump,
            descend,
            sneak,
            sprint,
            ..FlatInputFrame::default()
        };
        self.movement_input_applied |= host.apply_mono_look_frame(input);
        if runtime_ready {
            self.movement_input_applied |= host.apply_mono_movement_frame(input, delta_seconds);
        }
        if forward || backward || left || right || analog_active || jump || descend || sneak {
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
                    label: Some("mclone_web_scene_host_encoder"),
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
        self.mesh_build_count = self
            .mesh_build_count
            .saturating_add(summary.upload.accepted_compile_result_count);
        self.last_frame = LastFrameStats {
            section_count: summary.render.section_count,
            drawn_section_count: summary.render.drawn_section_count,
            frustum_section_count: summary.render.frustum_section_count,
            index_count: summary.render.index_count,
            drawn_index_count: summary.render.drawn_index_count,
            actor_count: summary.render.actor_count,
            drawn_actor_count: summary.render.drawn_actor_count,
            gui_command_count: summary.render.gui_command_count,
            flat_hud_rebuilds: summary.render.flat_hud_retained_cache.rebuild_count as usize,
            flat_hud_cache_hits: summary.render.flat_hud_retained_cache.cache_hit_count as usize,
            deferred_drop_backlog: summary.upload.poll_client_deferred_chunk_drop_backlog_items,
            far_lod_region_draw_count: summary.render.far_lod_region_draw_count,
            far_lod_uploaded_bytes: summary.render.far_lod_uploaded_bytes,
        };
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
        if let Some((epoch, file_count)) = self.pending_asset_pack_file_count {
            match host.asset_replacement_status() {
                AssetReplacementStatus::Active {
                    epoch: active_epoch,
                } if *active_epoch == epoch => {
                    self.asset_pack_file_count = file_count;
                    self.pending_asset_pack_file_count = None;
                }
                AssetReplacementStatus::Failed { .. } => {
                    self.pending_asset_pack_file_count = None;
                }
                _ => {}
            }
        }
        self.report(Some(&summary), true, delta_seconds, first_after_resume)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = pendingChunkRenderCompileJobCount)]
    pub fn pending_chunk_render_compile_job_count(&self) -> usize {
        self.host
            .as_ref()
            .and_then(McloneSceneHost::runtime_stats)
            .map_or(0, |stats| stats.pending_render_compile_jobs)
    }

    #[wasm_bindgen(js_name = renderCompilerSharedSupported)]
    pub fn render_compiler_shared_supported(&self) -> bool {
        let global = js_sys::global();
        js_sys::Reflect::has(&global, &JsValue::from_str("SharedArrayBuffer")).unwrap_or(false)
            && js_sys::Reflect::has(&global, &JsValue::from_str("Atomics")).unwrap_or(false)
    }

    #[wasm_bindgen(js_name = cameraFrameState)]
    pub fn camera_frame_state(&self) -> Result<JsValue, JsValue> {
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = adjustCameraSpeed)]
    pub fn adjust_camera_speed(&mut self, amount: f64) -> Result<JsValue, JsValue> {
        self.host_mut()?.adjust_mono_camera_speed(amount);
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = toggleMovementMode)]
    pub fn toggle_movement_mode(&mut self) -> Result<JsValue, JsValue> {
        let _ = self.host_mut()?.toggle_mono_movement_mode();
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = selectHotbarSlot)]
    pub fn select_hotbar_slot(&mut self, slot: u8) -> Result<JsValue, JsValue> {
        let _ = self.host_mut()?.select_mono_hotbar_slot(slot);
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = previewBlockTarget)]
    pub fn preview_block_target(&self) -> Result<JsValue, JsValue> {
        self.block_target_report().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = interactBlock)]
    pub fn interact_block(&mut self, action: &str) -> Result<JsValue, JsValue> {
        let action = match action {
            "break" => FlatInputAction::Attack,
            "place" => FlatInputAction::Use,
            other => {
                return Err(JsValue::from_str(&format!(
                    "unknown block interaction {other:?}"
                )));
            }
        };
        let before_target = self.host_ref()?.mono_block_target();
        let before_state = before_target.as_ref().and_then(|target| {
            self.host_ref()
                .ok()?
                .mono_client()?
                .block_state_at_block_pos(target.hit.block_pos)
        });
        let status = self
            .host_mut()?
            .handle_mono_world_action(action)
            .map_err(js_error)?;
        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
        report_set_string(
            &object,
            "action",
            if action == FlatInputAction::Attack {
                "break"
            } else {
                "place"
            },
        )
        .map_err(JsValue::from)?;
        match status {
            MonoWorldActionStatus::Sent { target, changed } => {
                self.interaction_count = self.interaction_count.saturating_add(1);
                self.command_count = self.command_count.saturating_add(1);
                self.update_count = self.update_count.saturating_add(usize::from(changed));
                self.interaction_sent = true;
                self.interaction_changed |= changed;
                write_block_target(&object, &target).map_err(JsValue::from)?;
                let result_state = if action == FlatInputAction::Use {
                    self.host_ref()?.selected_mono_hotbar_block_state()
                } else {
                    self.host_ref()?
                        .mono_client()
                        .and_then(|client| client.block_state_at_block_pos(target.hit.block_pos))
                };
                report_set_number(
                    &object,
                    "hitBlockStateId",
                    before_state.map_or(-1.0, |state| f64::from(state.0)),
                )
                .map_err(JsValue::from)?;
                report_set_number(
                    &object,
                    "resultBlockStateId",
                    result_state.map_or(-1.0, |state| f64::from(state.0)),
                )
                .map_err(JsValue::from)?;
                report_set_number(
                    &object,
                    "selectedHotbarSlot",
                    f64::from(self.host_ref()?.selected_mono_hotbar_slot()),
                )
                .map_err(JsValue::from)?;
                report_set_bool(&object, "carriedItemSynced", action == FlatInputAction::Use)
                    .map_err(JsValue::from)?;
                report_set_number(
                    &object,
                    "interactionUpdateCount",
                    if changed { 1.0 } else { 0.0 },
                )
                .map_err(JsValue::from)?;
                report_set_bool(&object, "commandSent", true).map_err(JsValue::from)?;
                report_set_bool(&object, "changed", changed).map_err(JsValue::from)?;
                report_set_number(&object, "commandCountDelta", 1.0).map_err(JsValue::from)?;
                report_set_number(&object, "updateCountDelta", if changed { 1.0 } else { 0.0 })
                    .map_err(JsValue::from)?;
            }
            _ => {
                report_set_bool(&object, "hit", false).map_err(JsValue::from)?;
                report_set_bool(&object, "commandSent", false).map_err(JsValue::from)?;
                report_set_bool(&object, "changed", false).map_err(JsValue::from)?;
            }
        }
        self.write_common_counts(&object).map_err(JsValue::from)?;
        Ok(object.into())
    }

    #[wasm_bindgen(js_name = blockStateAt)]
    pub fn block_state_at(&self, x: i32, y: i32, z: i32) -> Result<JsValue, JsValue> {
        let object = js_sys::Object::new();
        let state = self
            .host_ref()?
            .mono_client()
            .and_then(|client| client.block_state_at_block_pos(BlockPos::new(x, y, z)));
        report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
        report_set_bool(&object, "loaded", state.is_some()).map_err(JsValue::from)?;
        report_set_number(&object, "blockX", f64::from(x)).map_err(JsValue::from)?;
        report_set_number(&object, "blockY", f64::from(y)).map_err(JsValue::from)?;
        report_set_number(&object, "blockZ", f64::from(z)).map_err(JsValue::from)?;
        report_set_number(
            &object,
            "blockStateId",
            state.map_or(-1.0, |state| f64::from(state.0)),
        )
        .map_err(JsValue::from)?;
        Ok(object.into())
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
            self.resume_frames_remaining = RESUME_OBSERVATION_FRAMES;
        }
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = resizeCanvas)]
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

    #[wasm_bindgen(js_name = uiStatus)]
    pub fn ui_status(&mut self) -> Result<JsValue, JsValue> {
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setDebugOverlayVisible)]
    pub fn set_debug_overlay_visible(&mut self, visible: bool) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_debug_diagnostics_visible(visible);
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setStatusOverlay)]
    pub fn set_status_overlay(
        &mut self,
        message: &str,
        ok: bool,
        visible: bool,
    ) -> Result<JsValue, JsValue> {
        self.status_overlay = if visible {
            StatusOverlay::new(message, ok)
        } else {
            StatusOverlay::hidden()
        };
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchLookSensitivity)]
    pub fn set_touch_look_sensitivity(
        &mut self,
        sensitivity: f32,
        available: bool,
    ) -> Result<JsValue, JsValue> {
        self.touch_look_sensitivity = if sensitivity.is_finite() {
            sensitivity.clamp(TOUCH_LOOK_SENSITIVITY_MIN, TOUCH_LOOK_SENSITIVITY_MAX)
        } else {
            1.0
        };
        self.touch_settings_available = available;
        self.refresh_mono_ui_context()?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchControlsMode)]
    pub fn set_touch_controls_mode(&mut self, mode: &str) -> Result<JsValue, JsValue> {
        self.touch_controls_mode = match mode {
            "auto" => TouchControlsMode::Auto,
            "on" => TouchControlsMode::On,
            "off" => TouchControlsMode::Off,
            other => return Err(JsValue::from_str(&format!("invalid touch mode {other:?}"))),
        };
        self.refresh_mono_ui_context()?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchControlsOverlay)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_touch_controls_overlay(
        &mut self,
        visible: bool,
        movement_active: bool,
        base_x: f64,
        base_y: f64,
        thumb_x: f64,
        thumb_y: f64,
        jump_pressed: bool,
        sprint_pressed: bool,
        descend_pressed: bool,
        menu_pressed: bool,
    ) -> Result<JsValue, JsValue> {
        let point = |x: f64, y: f64| Point {
            x: (x as f32).clamp(0.0, self.context.width as f32),
            y: (y as f32).clamp(0.0, self.context.height as f32),
        };
        self.touch_overlay = TouchOverlay {
            visible,
            menu_pressed,
            movement: TouchJoystickOverlay {
                active: movement_active,
                base: point(base_x, base_y),
                thumb: point(thumb_x, thumb_y),
            },
            jump_pressed,
            sprint_pressed,
            sneak_pressed: false,
            descend_pressed,
            interaction_visible: false,
            attack_pressed: false,
            use_pressed: false,
            hotbar_visible: false,
            selected_hotbar_slot: self.host_ref()?.selected_mono_hotbar_slot(),
            hotbar_pressed_slot: None,
            hotbar_icons: mclone_ui::EMPTY_HOTBAR_ICONS,
        };
        self.refresh_mono_ui_context()?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openTitleUi)]
    pub fn open_title_ui(&mut self) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_ui_screen(Some(GameScreen::Title));
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openPauseUi)]
    pub fn open_pause_ui(&mut self) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_ui_screen(Some(GameScreen::Pause));
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openHelpUi)]
    pub fn open_help_ui(&mut self) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_ui_screen(Some(GameScreen::Help {
            parent: GameHelpParent::Game,
        }));
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = closeUi)]
    pub fn close_ui(&mut self) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_ui_screen(None);
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiKey)]
    pub fn handle_ui_key(&mut self, key: &str) -> Result<JsValue, JsValue> {
        let key = gui_key_from_label(key)
            .ok_or_else(|| JsValue::from_str(&format!("unknown UI key {key:?}")))?;
        let (handled, action) = self.host_mut()?.mono_ui_key_pressed(key);
        self.apply_ui_event(handled, action, false)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerMove)]
    pub fn handle_ui_pointer_move(
        &mut self,
        x: f64,
        y: f64,
        _radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point(x, y);
        let (handled, action) = self.host_mut()?.mono_ui_pointer_move(point);
        self.apply_ui_event(handled, action, false)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerDown)]
    pub fn handle_ui_pointer_down(
        &mut self,
        x: f64,
        y: f64,
        _radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point(x, y);
        let handled = self.host_mut()?.mono_ui_pointer_down(point);
        self.ui_report(handled, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerUp)]
    pub fn handle_ui_pointer_up(
        &mut self,
        x: f64,
        y: f64,
        _radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point(x, y);
        let (handled, action) = self.host_mut()?.mono_ui_pointer_up(point);
        self.apply_ui_event(handled, action, true)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = syncOverviewRenderFrame)]
    pub fn sync_overview_render_frame(
        &mut self,
        center_x: i32,
        center_z: i32,
        _radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let eye = Vec3d::new(
            f64::from(center_x) * 16.0 + 8.0,
            112.0,
            f64::from(center_z) * 16.0 + 8.0,
        );
        self.host_mut()?
            .set_mono_capture_camera(eye, 0.55, -0.45, 4.3);
        self.render_frame(
            browser_now_millis(),
            0.0,
            0.0,
            0.0,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            0.0,
            0.0,
        )
    }

    #[wasm_bindgen(js_name = startLocalWorld)]
    pub async fn start_local_world(
        &mut self,
        seed: i64,
        worker_url: String,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<JsValue, JsValue> {
        if !self.has_pending_session_start() {
            self.host_mut()?
                .request_external_session_start(SessionStartRequest::new_seed_local_world(seed))
                .map_err(js_error)?;
        }
        let pending = self.take_pending_session_start()?;
        self.start_worker_runtime(
            pending,
            WebIntegratedServerRunnerConfig::new(
                seed,
                worker_url,
                job_worker_url,
                bindgen_js_url,
                bindgen_wasm_url,
            ),
        )
        .await
    }

    #[wasm_bindgen(js_name = startIndexedDbLocalWorld)]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_indexed_db_local_world(
        &mut self,
        seed: i64,
        world_id: String,
        display_name: String,
        request_kind: String,
        worker_url: String,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<JsValue, JsValue> {
        let world_id_value =
            LocalWorldId::new(world_id.clone()).map_err(|error| JsValue::from(error.message))?;
        if !self.has_pending_session_start() {
            let request = if request_kind == "createLocalWorld" {
                SessionStartRequest::create_local_world(
                    LocalWorldCreateOptions::new(display_name, seed)
                        .map_err(|error| JsValue::from(error.message))?
                        .with_requested_id(world_id_value),
                )
            } else {
                SessionStartRequest::open_local_world(world_id_value)
            };
            self.host_mut()?
                .request_external_session_start(request)
                .map_err(js_error)?;
        }
        let pending = self.take_pending_session_start()?;
        self.start_worker_runtime(
            pending,
            WebIntegratedServerRunnerConfig::new(
                seed,
                worker_url,
                job_worker_url,
                bindgen_js_url,
                bindgen_wasm_url,
            )
            .with_indexed_db_world(world_id, false),
        )
        .await
    }

    #[wasm_bindgen(js_name = joinRemoteWebSocket)]
    pub async fn join_remote_websocket(&mut self, url: String) -> Result<JsValue, JsValue> {
        if !self.has_pending_session_start() {
            self.host_mut()?
                .request_external_session_start(SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new(url.clone()),
                })
                .map_err(js_error)?;
        }
        let pending = self.take_pending_session_start()?;
        let center = pending.scene.center();
        let render_distance = pending.scene.render_distance;
        let mut runtime = crate::WebRuntime::websocket_remote_at(url, center)
            .await
            .map_err(JsValue::from)?;
        runtime
            .request_chunk_view_deferred(
                center,
                render_distance,
                chunk_tracking_radius_for_render_distance(render_distance),
            )
            .map_err(JsValue::from)?;
        self.complete_started_runtime(pending, runtime)
    }

    #[wasm_bindgen(js_name = applyWorldCatalogResponse)]
    pub fn apply_world_catalog_response(
        &mut self,
        request_id: String,
        operation: String,
        payload: JsValue,
    ) -> Result<JsValue, JsValue> {
        let token = self
            .catalog_tokens
            .remove(&request_id)
            .ok_or_else(|| JsValue::from_str("unknown catalog request completion"))?;
        let response =
            decode_world_catalog_response(&operation, &payload).map_err(JsValue::from)?;
        self.platform
            .complete_catalog_operation(PlatformOperationCompletion {
                token,
                result: Ok(response),
            });
        let (device, queue) = (&self.context.device, &self.context.queue);
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
            .poll_external_catalog_operations(device, queue)
            .map_err(js_error)?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = applyWorldCatalogError)]
    pub fn apply_world_catalog_error(
        &mut self,
        request_id: String,
        message: String,
    ) -> Result<JsValue, JsValue> {
        let token = self
            .catalog_tokens
            .remove(&request_id)
            .ok_or_else(|| JsValue::from_str("unknown catalog request failure"))?;
        self.platform
            .complete_catalog_operation(PlatformOperationCompletion {
                token,
                result: Err(WorldCatalogError::new(
                    WorldCatalogErrorKind::StorageFailure,
                    message,
                )),
            });
        let (device, queue) = (&self.context.device, &self.context.queue);
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
            .poll_external_catalog_operations(device, queue)
            .map_err(js_error)?;
        self.ui_report(false, None).map_err(JsValue::from)
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
        let mut effects = WebHostEffects::default();
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
            .require_restart("simulated WebGPU surface loss; recreate scene host");
        self.report(None, false, 0.0, false).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = shutdown)]
    pub fn shutdown(&mut self) -> Result<JsValue, JsValue> {
        if let Some(kind) = self
            .host
            .as_ref()
            .and_then(McloneSceneHost::runtime_stats)
            .and_then(|stats| stats.server_runner_kind)
        {
            self.last_runner_kind = kind.label().to_owned();
        }
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

    #[wasm_bindgen(js_name = shutdownAsync)]
    pub async fn shutdown_async(&mut self) -> Result<JsValue, JsValue> {
        self.shutdown()
    }

    #[wasm_bindgen(js_name = completeAssetPackSelection)]
    pub async fn complete_asset_pack_selection(
        &mut self,
        authored_pack_bytes: js_sys::Uint8Array,
        reference_pack_bytes: js_sys::Uint8Array,
        fallback_pack_bytes: js_sys::Uint8Array,
    ) -> Result<JsValue, JsValue> {
        let pending = self
            .host_ref()?
            .pending_external_asset_pack_selection()
            .cloned()
            .ok_or_else(|| JsValue::from_str("no browser asset-pack selection is pending"))?;
        let authored_enabled = pending
            .selection
            .is_enabled(&mclone_assets::AssetPackId::new(
                AUTHORED_FIRST_PARTY_PACK_ID,
            ));
        let reference_enabled = pending
            .selection
            .is_enabled(&mclone_assets::AssetPackId::new(
                MINECRAFT_REFERENCE_PACK_ID,
            ));
        let authored = authored_pack_bytes.to_vec();
        let reference = reference_pack_bytes.to_vec();
        let fallback = fallback_pack_bytes.to_vec();
        let selected_file_count = [
            authored_enabled.then_some(authored.as_slice()),
            reference_enabled.then_some(reference.as_slice()),
            Some(fallback.as_slice()),
        ]
        .into_iter()
        .flatten()
        .map(|bytes| PackedAssetSource::from_bytes(bytes.to_vec()).map(|pack| pack.file_count()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| JsValue::from_str(&format!("failed to inspect selected packs: {error}")))?
        .into_iter()
        .sum();
        let assets = match prepare_web_scene_assets_from_selection(
            pending.epoch,
            authored,
            reference,
            fallback,
            authored_enabled,
            reference_enabled,
        ) {
            Ok(assets) => assets,
            Err(error) => {
                self.host_mut()?
                    .fail_external_asset_pack_preparation(error.clone());
                return Err(JsValue::from_str(&error));
            }
        };
        if let Err(error) = self
            .host_mut()?
            .complete_external_asset_pack_preparation(assets)
        {
            let message = format!("failed to complete browser asset replacement: {error:#}");
            self.host_mut()?
                .fail_external_asset_pack_preparation(message.clone());
            return Err(JsValue::from_str(&message));
        }
        self.pending_asset_pack_file_count = Some((pending.epoch, selected_file_count));
        self.ui_report(false, None).map_err(JsValue::from)
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub async fn mclone_web_create_worker_scene_host_with_startup(
    canvas: HtmlCanvasElement,
    reference_pack_bytes: js_sys::Uint8Array,
    authored_pack_bytes: js_sys::Uint8Array,
    fallback_pack_bytes: js_sys::Uint8Array,
    seed: i64,
    initial_center_x: i32,
    initial_center_z: i32,
    render_distance: u32,
    movement_speed_multiplier: f32,
    light_status_batch_size: usize,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    far_lod_enabled: bool,
    render_color_profile: String,
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    world_storage: String,
    world_id: String,
    clear_world_storage: bool,
    compiler_wake: js_sys::Function,
) -> Result<WebSceneHost, JsValue> {
    let center = ChunkPos::new(initial_center_x, initial_center_z);
    let mut config = WebIntegratedServerRunnerConfig::new(
        seed,
        server_worker_url,
        server_job_worker_url,
        bindgen_js_url,
        bindgen_wasm_url,
    )
    .with_light_status_batch_size(light_status_batch_size);
    if world_storage == "indexeddb" {
        config = config.with_indexed_db_world(world_id, clear_world_storage);
    } else if world_storage != "transient" {
        return Err(JsValue::from_str("unsupported browser world storage"));
    }
    let mut runtime = crate::WebRuntime::web_worker_integrated_at(config, center)
        .await
        .map_err(JsValue::from)?;
    runtime
        .request_chunk_view_deferred(
            center,
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
        .map_err(JsValue::from)?;
    let descriptor = ActiveSessionDescriptor::LocalWorld {
        seed,
        id: None,
        display_name: None,
    };
    let scene = McloneSceneHostOptions {
        seed,
        chunk_x: center.x,
        chunk_z: center.z,
        render_distance,
        movement_speed_multiplier,
        use_initial_spawn_center: false,
        far_lod: if far_lod_enabled {
            mclone_app_runtime::far_lod::FarTerrainLodConfig::enabled()
        } else {
            Default::default()
        },
        startup_lod_prewarm: false,
        ..McloneSceneHostOptions::default()
    };
    create_scene_host(
        canvas,
        reference_pack_bytes.to_vec(),
        authored_pack_bytes.to_vec(),
        fallback_pack_bytes.to_vec(),
        runtime,
        descriptor,
        scene,
        startup_render_options(
            section_occlusion_culling,
            force_fullbright,
            &render_color_profile,
        )
        .map_err(JsValue::from)?,
        compiler_wake,
    )
    .await
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub async fn mclone_web_create_remote_scene_host_with_startup(
    canvas: HtmlCanvasElement,
    reference_pack_bytes: js_sys::Uint8Array,
    authored_pack_bytes: js_sys::Uint8Array,
    fallback_pack_bytes: js_sys::Uint8Array,
    websocket_url: String,
    initial_center_x: i32,
    initial_center_z: i32,
    render_distance: u32,
    movement_speed_multiplier: f32,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    far_lod_enabled: bool,
    render_color_profile: String,
    compiler_wake: js_sys::Function,
) -> Result<WebSceneHost, JsValue> {
    let center = ChunkPos::new(initial_center_x, initial_center_z);
    let mut runtime = crate::WebRuntime::websocket_remote_at(websocket_url.clone(), center)
        .await
        .map_err(JsValue::from)?;
    runtime
        .request_chunk_view_deferred(
            center,
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
        .map_err(JsValue::from)?;
    let descriptor = ActiveSessionDescriptor::Remote {
        endpoint: RemoteSessionEndpoint::new(websocket_url.clone()),
    };
    let scene = McloneSceneHostOptions {
        seed: DEFAULT_SEED,
        chunk_x: center.x,
        chunk_z: center.z,
        render_distance,
        movement_speed_multiplier,
        use_initial_spawn_center: false,
        far_lod: if far_lod_enabled {
            mclone_app_runtime::far_lod::FarTerrainLodConfig::enabled()
        } else {
            Default::default()
        },
        startup_lod_prewarm: false,
        ..McloneSceneHostOptions::default()
    };
    create_scene_host(
        canvas,
        reference_pack_bytes.to_vec(),
        authored_pack_bytes.to_vec(),
        fallback_pack_bytes.to_vec(),
        runtime,
        descriptor,
        scene,
        startup_render_options(
            section_occlusion_culling,
            force_fullbright,
            &render_color_profile,
        )
        .map_err(JsValue::from)?,
        compiler_wake,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn create_scene_host(
    canvas: HtmlCanvasElement,
    reference_pack_bytes: Vec<u8>,
    authored_pack_bytes: Vec<u8>,
    fallback_pack_bytes: Vec<u8>,
    runtime: crate::WebRuntime,
    descriptor: ActiveSessionDescriptor,
    scene: McloneSceneHostOptions,
    render_options: TexturedSectionRenderOptions,
    compiler_wake: js_sys::Function,
) -> Result<WebSceneHost, JsValue> {
    let render_color_profile = render_options.color_profile.as_str().to_owned();
    let initial_center = scene.center();
    let initial_speed = f64::from(scene.movement_speed_multiplier);
    let asset_pack_file_count = PackedAssetSource::from_bytes(reference_pack_bytes.clone())
        .map_err(|error| JsValue::from_str(&format!("invalid browser asset pack: {error}")))?
        .file_count();
    let catalog = web_asset_pack_catalog(
        authored_pack_bytes,
        reference_pack_bytes.clone(),
        fallback_pack_bytes,
    )
    .map_err(JsValue::from)?;
    let active_assets =
        prepare_web_scene_assets_from_pack(reference_pack_bytes).map_err(JsValue::from)?;
    let context = WebCanvasContext::new_with_color_profile(canvas, render_options.color_profile)
        .await
        .map_err(JsValue::from)?;
    let (mut platform, clock, catalog_operations) = WebScenePlatformServices::new();
    let runtime = WebSceneRuntimeService::new(
        runtime,
        active_assets.mesh.clone(),
        compiler_wake.clone(),
        clock.clone(),
    )
    .into_scene_session_runtime(descriptor.clone());
    let operation = match &descriptor {
        ActiveSessionDescriptor::LocalWorld { seed, id, .. } => {
            WebSceneSessionOperation::StartLocal {
                seed: *seed,
                world_id: id.as_ref().map(|id| id.as_str().to_owned()),
            }
        }
        ActiveSessionDescriptor::Remote { endpoint } => WebSceneSessionOperation::ConnectRemote {
            url: endpoint.address.clone(),
        },
    };
    let start = platform.lifecycle_mut().begin_start(operation);
    let start_disposition = platform
        .lifecycle_mut()
        .complete(PlatformOperationCompletion {
            token: start.token,
            result: Ok(WebSceneSessionOperationResult::Started(descriptor)),
        });
    if start_disposition != WebSceneSessionCompletionDisposition::Applied {
        return Err(JsValue::from_str(
            "scene host lifecycle start was not applied",
        ));
    }
    let mut host = McloneSceneHost::with_scene_runtime(
        &context.device,
        &context.queue,
        context.format,
        clock,
        scene,
        runtime,
        render_options,
        active_assets,
        web_client_experience_profile(),
        Some(catalog_operations),
        None,
    )
    .map_err(js_error)?;
    host.set_mono_ui_context(MonoUiContext::default());
    host.set_mono_ui_screen(None);
    host.configure_external_asset_pack_catalog(
        catalog,
        mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
    )
    .map_err(js_error)?;
    host.configure_asset_pack_preference_storage(Box::new(WebAssetPackPreferenceStorage))
        .map_err(js_error)?;
    host.set_mono_player_camera(
        Vec3d::new(
            f64::from(initial_center.x) * 16.0 + 8.0,
            112.0,
            f64::from(initial_center.z) * 16.0 + 8.0,
        ),
        0.0,
        -0.35,
        initial_speed,
    );
    let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
    Ok(WebSceneHost {
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
        compiler_wake,
        asset_pack_file_count,
        pending_asset_pack_file_count: None,
        status_overlay: StatusOverlay::hidden(),
        touch_look_sensitivity: 1.0,
        touch_settings_available: false,
        touch_controls_mode: TouchControlsMode::Auto,
        touch_overlay: TouchOverlay::hidden(),
        last_action: None,
        last_frame: LastFrameStats::default(),
        command_count: 0,
        update_count: 0,
        interaction_count: 0,
        mesh_build_count: 0,
        catalog_tokens: HashMap::new(),
        render_color_profile,
        last_runner_kind: "none".to_owned(),
        startup_camera_reconciled: false,
    })
}

impl WebSceneHost {
    fn host_ref(&self) -> Result<&McloneSceneHost, JsValue> {
        self.host
            .as_ref()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))
    }

    fn host_mut(&mut self) -> Result<&mut McloneSceneHost, JsValue> {
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))
    }

    fn has_pending_session_start(&self) -> bool {
        matches!(
            self.host.as_ref().map(McloneSceneHost::session_state),
            Some(mclone_app_runtime::session::GameSessionState::Starting { .. })
        )
    }

    fn take_pending_session_start(&mut self) -> Result<ExternalSceneSessionStart, JsValue> {
        self.host_mut()?
            .take_external_session_start()
            .ok_or_else(|| JsValue::from_str("scene host has no pending session start"))
    }

    async fn start_worker_runtime(
        &mut self,
        pending: ExternalSceneSessionStart,
        config: WebIntegratedServerRunnerConfig,
    ) -> Result<JsValue, JsValue> {
        let center = pending.scene.center();
        let render_distance = pending.scene.render_distance;
        match crate::WebRuntime::web_worker_integrated_at(config, center).await {
            Ok(mut runtime) => {
                runtime
                    .request_chunk_view_deferred(
                        center,
                        render_distance,
                        chunk_tracking_radius_for_render_distance(render_distance),
                    )
                    .map_err(JsValue::from)?;
                self.complete_started_runtime(pending, runtime)
            }
            Err(error) => {
                self.host_mut()?.fail_external_session_start(pending, error);
                self.ui_report(false, None).map_err(JsValue::from)
            }
        }
    }

    fn complete_started_runtime(
        &mut self,
        pending: ExternalSceneSessionStart,
        runtime: crate::WebRuntime,
    ) -> Result<JsValue, JsValue> {
        let descriptor = pending.descriptor.clone();
        let operation = match pending.runtime_kind {
            SessionRuntimeKind::Local => WebSceneSessionOperation::StartLocal {
                seed: descriptor.local_seed().unwrap_or(DEFAULT_SEED),
                world_id: descriptor.local_world_id().map(|id| id.as_str().to_owned()),
            },
            SessionRuntimeKind::Remote => WebSceneSessionOperation::ConnectRemote {
                url: match &descriptor {
                    ActiveSessionDescriptor::Remote { endpoint } => endpoint.address.clone(),
                    ActiveSessionDescriptor::LocalWorld { .. } => String::new(),
                },
            },
        };
        let lifecycle = self.platform.lifecycle_mut().begin_start(operation);
        let active_assets = self
            .host_ref()?
            .active_asset_snapshot_for_epoch(self.host_ref()?.active_asset_epoch());
        let runtime = WebSceneRuntimeService::new(
            runtime,
            active_assets.mesh.clone(),
            self.compiler_wake.clone(),
            self.platform.clock_handle(),
        )
        .into_scene_session_runtime(descriptor.clone());
        let (device, queue) = (&self.context.device, &self.context.queue);
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
            .complete_external_session_start(device, queue, pending, runtime)
            .map_err(js_error)?;
        let disposition = self
            .platform
            .lifecycle_mut()
            .complete(PlatformOperationCompletion {
                token: lifecycle.token,
                result: Ok(WebSceneSessionOperationResult::Started(descriptor)),
            });
        if disposition != WebSceneSessionCompletionDisposition::Applied {
            return Err(JsValue::from_str(
                "scene lifecycle rejected external session completion",
            ));
        }
        self.last_frame = LastFrameStats::default();
        self.startup_camera_reconciled = false;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    fn ui_point(&self, x: f64, y: f64) -> Point {
        GuiScale::from_pixels(self.context.width, self.context.height).client_to_gui(x, y)
    }

    fn refresh_mono_ui_context(&mut self) -> Result<(), JsValue> {
        let mut context = MonoUiContext::default();
        context.touch_overlay = self.touch_overlay.clone();
        context.touch_controls_mode = Some(self.touch_controls_mode);
        context.touch_settings = self
            .touch_settings_available
            .then_some(GameTouchSettings::new(
                self.touch_look_sensitivity,
                TOUCH_LOOK_SENSITIVITY_MIN,
                TOUCH_LOOK_SENSITIVITY_MAX,
            ));
        self.host_mut()?.set_mono_ui_context(context);
        Ok(())
    }

    fn apply_ui_event(
        &mut self,
        handled: bool,
        action: Option<GameUiAction>,
        from_pointer: bool,
    ) -> Result<JsValue, String> {
        if let Some(action) = action {
            let mut effects = WebHostEffects::default();
            self.host
                .as_mut()
                .ok_or_else(|| "scene host is shut down".to_owned())?
                .apply_mono_ui_action(
                    action,
                    from_pointer,
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(|error| format!("apply shared web UI action: {error:#}"))?;
            if let Some(mode) = effects.mouse_lock_requested {
                let _ = mode;
            }
            if let Some(mode) = effects.touch_controls_mode {
                self.touch_controls_mode = mode;
                self.refresh_mono_ui_context()
                    .map_err(|error| format!("refresh web UI context: {error:?}"))?;
            }
            if let GameUiAction::SetTouchLookSensitivity(value) = action {
                self.touch_look_sensitivity = value;
                self.refresh_mono_ui_context()
                    .map_err(|error| format!("refresh web UI context: {error:?}"))?;
            }
            self.last_action = Some(action);
        }
        self.ui_report(handled, action)
    }

    fn ui_report(
        &mut self,
        handled: bool,
        action: Option<GameUiAction>,
    ) -> Result<JsValue, String> {
        let value = self.report(None, false, 0.0, false)?;
        let object: js_sys::Object = value.unchecked_into();
        report_set_bool(&object, "handled", handled)?;
        if let Some(action) = action {
            report_set_string(&object, "action", ui_action_label(action))?;
            match action {
                GameUiAction::CreateWorld(seed) => {
                    report_set_number(&object, "worldSeed", seed as f64)?;
                    report_set_string(&object, "worldSeedText", &seed.to_string())?;
                }
                GameUiAction::JoinRemote => {
                    if let Some(ActiveSessionDescriptor::Remote { endpoint }) = self
                        .host
                        .as_ref()
                        .and_then(|host| match host.session_state() {
                            mclone_app_runtime::session::GameSessionState::Active { session } => {
                                Some(session.clone())
                            }
                            _ => None,
                        })
                    {
                        report_set_string(&object, "remoteEndpoint", &endpoint.address)?;
                    }
                }
                GameUiAction::SetRenderDistance(distance) => {
                    report_set_number(&object, "renderDistance", f64::from(distance))?;
                }
                GameUiAction::SetTouchLookSensitivity(value) => {
                    report_set_number(&object, "touchLookSensitivity", f64::from(value))?;
                }
                GameUiAction::SetTouchControlsMode(mode) => {
                    report_set_string(&object, "touchControlsMode", touch_mode_label(mode))?;
                }
                GameUiAction::SelectWorld(id) | GameUiAction::OpenWorld(id) => {
                    report_set_number(&object, "catalogWorldUiId", id.0 as f64)?;
                    if let Some(world_id) = self
                        .host
                        .as_ref()
                        .and_then(|host| host.mono_local_world_id_for_ui_id(id))
                    {
                        report_set_string(&object, "catalogWorldId", world_id.as_str())?;
                    }
                }
                _ => {}
            }
        }
        if let Some(operation) = self.platform.take_catalog_operation() {
            let request = &operation.kind.request;
            let request_id = request.id.0.to_string();
            self.catalog_tokens
                .insert(request_id.clone(), operation.token);
            report_set_bool(&object, "catalogRequest", true)?;
            report_set_string(&object, "catalogRequestId", &request_id)?;
            if let Some(active) = operation.kind.active_world {
                report_set_string(&object, "activeWorldId", active.as_str())?;
            }
            write_catalog_request(&object, &request.request)?;
        }
        Ok(object.into())
    }

    fn block_target_report(&self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true)?;
        if let Some(target) = self
            .host
            .as_ref()
            .and_then(McloneSceneHost::mono_block_target)
        {
            write_block_target(&object, &target)?;
            let hit_state = self
                .host
                .as_ref()
                .and_then(McloneSceneHost::mono_client)
                .and_then(|client| client.block_state_at_block_pos(target.hit.block_pos));
            report_set_number(
                &object,
                "hitBlockStateId",
                hit_state.map_or(-1.0, |state| f64::from(state.0)),
            )?;
            if let Some(host) = self.host.as_ref() {
                report_set_number(
                    &object,
                    "selectedHotbarSlot",
                    f64::from(host.selected_mono_hotbar_slot()),
                )?;
            }
        } else {
            report_set_bool(&object, "hit", false)?;
            report_set_string(&object, "hitType", "miss")?;
            report_set_number(&object, "hitBlockStateId", -1.0)?;
        }
        self.write_common_counts(&object)?;
        Ok(object.into())
    }

    fn write_common_counts(&self, object: &js_sys::Object) -> Result<(), String> {
        let stats = self.host.as_ref().and_then(McloneSceneHost::runtime_stats);
        report_set_number(
            object,
            "commandCount",
            stats.map_or(self.command_count, |stats| stats.command_count) as f64,
        )?;
        report_set_number(
            object,
            "updateCount",
            stats.map_or(self.update_count, |stats| stats.update_count) as f64,
        )?;
        report_set_number(
            object,
            "pendingCompileJobCount",
            self.pending_chunk_render_compile_job_count() as f64,
        )
    }

    fn report(
        &self,
        summary: Option<&MonoSceneFrameSummary>,
        rendered: bool,
        delta_seconds: f64,
        first_after_resume: bool,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true)?;
        report_set_string(&object, "owner", "McloneSceneHost")?;
        report_set_string(&object, "state", self.frame_policy.state().label())?;
        if let WebSceneFrameState::RestartRequired { reason } = self.frame_policy.state() {
            report_set_string(&object, "restartReason", reason)?;
        }
        report_set_bool(&object, "rendered", rendered)?;
        report_set_bool(&object, "hostOwnedFrameAssembly", summary.is_some())?;
        report_set_bool(&object, "audioCapabilityAbsent", true)?;
        report_set_bool(&object, "teleportCapabilityAbsent", true)?;
        report_set_number(&object, "frameCount", self.frame_count as f64)?;
        report_set_number(
            &object,
            "renderedFrameCount",
            self.rendered_frame_count as f64,
        )?;
        report_set_number(&object, "deltaSeconds", delta_seconds)?;
        report_set_bool(&object, "firstAfterResume", first_after_resume)?;
        report_set_number(&object, "maxFrameGapMs", self.max_frame_gap_millis)?;
        report_set_number(
            &object,
            "hiddenFrameSkips",
            self.frame_policy.hidden_frame_skips() as f64,
        )?;
        report_set_number(
            &object,
            "resumeCount",
            self.frame_policy.resume_count() as f64,
        )?;
        report_set_number(
            &object,
            "resumeFramesRemaining",
            f64::from(self.resume_frames_remaining),
        )?;
        report_set_number(
            &object,
            "maxResumePollUpdates",
            self.max_resume_poll_updates as f64,
        )?;
        report_set_number(
            &object,
            "maxResumeDeferredDropBacklogItems",
            self.max_resume_drop_backlog as f64,
        )?;
        report_set_number(
            &object,
            "backgroundSaveCount",
            self.background_save_count as f64,
        )?;
        report_set_bool(&object, "movementInputApplied", self.movement_input_applied)?;
        report_set_number(
            &object,
            "domInputFrameCount",
            self.dom_input_frame_count as f64,
        )?;
        report_set_bool(&object, "interactionSent", self.interaction_sent)?;
        report_set_bool(&object, "interactionChanged", self.interaction_changed)?;
        report_set_bool(
            &object,
            "settingsEffectApplied",
            self.settings_effect_applied,
        )?;
        report_set_bool(&object, "pauseUiRendered", self.pause_ui_rendered)?;
        report_set_bool(&object, "resized", self.resized)?;
        report_set_number(&object, "width", self.context.width as f64)?;
        report_set_number(&object, "height", self.context.height as f64)?;
        report_set_bool(&object, "shutdownComplete", self.shutdown_complete)?;
        report_set_bool(&object, "configured", true)?;
        report_set_bool(&object, "assetPackLoaded", self.asset_pack_file_count > 0)?;
        report_set_bool(&object, "textured", self.asset_pack_file_count > 0)?;
        report_set_bool(&object, "skyRendered", rendered)?;
        report_set_number(
            &object,
            "assetPackFileCount",
            self.asset_pack_file_count as f64,
        )?;
        report_set_number(&object, "assetPackParseCount", 1.0)?;
        report_set_number(&object, "terrainAssetLoadCount", 1.0)?;
        report_set_number(&object, "atlasUploadCount", 1.0)?;
        report_set_number(&object, "renderCount", self.rendered_frame_count as f64)?;
        report_set_number(&object, "interactionCount", self.interaction_count as f64)?;
        self.write_common_counts(&object)?;

        if let Some(host) = self.host.as_ref() {
            let camera = host.camera_frame_state();
            report_set_number(&object, "cameraX", camera.camera.eye.x)?;
            report_set_number(&object, "cameraY", camera.camera.eye.y)?;
            report_set_number(&object, "cameraZ", camera.camera.eye.z)?;
            report_set_number(&object, "cameraYawRadians", camera.camera.yaw_radians)?;
            report_set_number(&object, "cameraPitchRadians", camera.camera.pitch_radians)?;
            report_set_string(&object, "movementMode", camera.movement_mode_label())?;
            report_set_string(&object, "collisionMode", camera.collision_mode_label())?;
            report_set_number(
                &object,
                "cameraSpeedBlocksPerSecond",
                camera.camera.speed_blocks_per_second,
            )?;
            report_set_bool(&object, "onGround", camera.on_ground)?;
            report_set_bool(&object, "horizontalCollision", camera.horizontal_collision)?;
            report_set_bool(&object, "verticalCollision", camera.vertical_collision)?;
            report_set_number(
                &object,
                "selectedHotbarSlot",
                f64::from(host.selected_mono_hotbar_slot()),
            )?;
            let center = camera.camera.chunk_pos;
            report_set_number(&object, "centerX", f64::from(center.x))?;
            report_set_number(&object, "centerZ", f64::from(center.z))?;
            report_set_number(
                &object,
                "radiusChunks",
                f64::from(host.current_render_distance()),
            )?;
            report_set_number(&object, "timeOfDay", f64::from(host.mono_time_of_day()))?;
            report_set_number(
                &object,
                "dayTime",
                f64::from(host.mono_time_of_day()) * 24_000.0,
            )?;
            report_set_number(&object, "sunAngle", f64::from(host.mono_sun_angle()))?;
            report_set_number(
                &object,
                "activeAssetEpoch",
                host.active_asset_epoch() as f64,
            )?;
            let active_selection = host.active_asset_pack_selection();
            report_set_bool(
                &object,
                "assetPackActiveAuthored",
                active_selection.is_enabled(&mclone_assets::AssetPackId::new(
                    AUTHORED_FIRST_PARTY_PACK_ID,
                )),
            )?;
            report_set_bool(
                &object,
                "assetPackActiveReference",
                active_selection.is_enabled(&mclone_assets::AssetPackId::new(
                    MINECRAFT_REFERENCE_PACK_ID,
                )),
            )?;
            let asset_diagnostics = host.asset_pack_runtime_diagnostics();
            report_set_string(
                &object,
                "assetPackPreferredIds",
                &asset_diagnostics.preferred_ids.join(","),
            )?;
            report_set_number(
                &object,
                "assetProvenanceFirstParty",
                asset_diagnostics.provenance.first_party as f64,
            )?;
            report_set_number(
                &object,
                "assetProvenanceGenerated",
                asset_diagnostics.provenance.generated as f64,
            )?;
            report_set_number(
                &object,
                "assetProvenanceMinecraftReference",
                asset_diagnostics.provenance.minecraft_reference as f64,
            )?;
            report_set_number(
                &object,
                "assetProvenanceUnknown",
                asset_diagnostics.provenance.unknown as f64,
            )?;
            report_set_bool(
                &object,
                "assetProprietaryFree",
                asset_diagnostics.proprietary_free,
            )?;
            if let Some(commit) = asset_diagnostics.last_commit {
                report_set_number(&object, "assetReloadPreparationMs", commit.preparation_ms)?;
                report_set_number(&object, "assetReloadCompileMs", commit.compile_ms)?;
                report_set_number(&object, "assetReloadUploadMs", commit.upload_ms)?;
                report_set_number(&object, "assetReloadTotalMs", commit.total_ms)?;
                report_set_number(
                    &object,
                    "assetReloadPeakRetainedCpuBytes",
                    commit.peak_retained_cpu_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "assetReloadPeakRetainedGpuBytes",
                    commit.peak_retained_gpu_bytes as f64,
                )?;
            }
            if let Some(error) = host.asset_pack_preference_error() {
                report_set_string(&object, "assetPackPreferenceError", error)?;
            }
            match host.asset_replacement_status() {
                AssetReplacementStatus::Active { .. } => {
                    report_set_string(&object, "assetReplacementState", "active")?;
                }
                AssetReplacementStatus::PreparingAssets { .. } => {
                    report_set_string(&object, "assetReplacementState", "preparing-assets")?;
                }
                AssetReplacementStatus::PreparingMeshes { .. } => {
                    report_set_string(&object, "assetReplacementState", "preparing-meshes")?;
                }
                AssetReplacementStatus::Failed { message, .. } => {
                    report_set_string(&object, "assetReplacementState", "failed")?;
                    report_set_string(&object, "assetReplacementError", message)?;
                }
            }
            if let Some(pending) = host.pending_external_asset_pack_selection() {
                report_set_bool(&object, "assetPackRequest", true)?;
                report_set_number(&object, "assetPackRequestEpoch", pending.epoch as f64)?;
                report_set_bool(
                    &object,
                    "assetPackAuthoredEnabled",
                    pending
                        .selection
                        .is_enabled(&mclone_assets::AssetPackId::new(
                            AUTHORED_FIRST_PARTY_PACK_ID,
                        )),
                )?;
                report_set_bool(
                    &object,
                    "assetPackReferenceEnabled",
                    pending
                        .selection
                        .is_enabled(&mclone_assets::AssetPackId::new(
                            MINECRAFT_REFERENCE_PACK_ID,
                        )),
                )?;
            }
            let ui_state = host.mono_ui_render_state();
            let world_catalog = ui_state.world_catalog;
            report_set_bool(&object, "uiActive", host.mono_ui_is_active())?;
            report_set_bool(&object, "uiCoversWorld", host.mono_ui_is_active())?;
            report_set_string(&object, "uiScreen", screen_label(host.mono_ui_screen()))?;
            if let Some(parent) = screen_options_parent(host.mono_ui_screen()) {
                report_set_string(&object, "uiOptionsParent", parent)?;
                report_set_string(&object, "optionsParent", parent)?;
            }
            report_set_bool(
                &object,
                "sectionOcclusionCulling",
                ui_state.section_occlusion_culling,
            )?;
            report_set_bool(&object, "forceFullbright", ui_state.force_fullbright)?;
            report_set_bool(&object, "farLodEnabled", ui_state.far_lod_enabled)?;
            report_set_number(
                &object,
                "farLodRangeChunks",
                f64::from(ui_state.far_lod_range_chunks),
            )?;
            let far_lod = host.far_lod_stats();
            report_set_number(&object, "farLodDesiredTiles", far_lod.desired_tiles as f64)?;
            report_set_number(
                &object,
                "farLodResidentTiles",
                far_lod.resident_tiles as f64,
            )?;
            report_set_number(&object, "farLodVisibleTiles", far_lod.visible_tiles as f64)?;
            report_set_number(
                &object,
                "farLodPendingBuilds",
                far_lod.pending_builds as f64,
            )?;
            report_set_number(
                &object,
                "farLodInflightBuilds",
                far_lod.inflight_builds as f64,
            )?;
            report_set_number(
                &object,
                "farLodQueuedUploads",
                far_lod.queued_uploads as f64,
            )?;
            report_set_number(
                &object,
                "farLodTotalUploadBytes",
                far_lod.total_upload_bytes as f64,
            )?;
            report_set_bool(&object, "worldCatalogPersistent", world_catalog.persistent)?;
            report_set_bool(&object, "worldCatalogLoading", world_catalog.loading)?;
            report_set_number(
                &object,
                "worldCatalogEntryCount",
                world_catalog.entries.iter().flatten().count() as f64,
            )?;
            report_set_bool(
                &object,
                "worldCatalogStatusVisible",
                world_catalog.status.visible,
            )?;
            report_set_bool(&object, "worldCatalogStatusOk", world_catalog.status.ok)?;
            report_set_string(
                &object,
                "worldCatalogStatusMessage",
                world_catalog.status.message.as_str(),
            )?;
            report_set_string(&object, "renderColorProfile", &self.render_color_profile)?;
            report_set_string(
                &object,
                "touchControlsMode",
                touch_mode_label(self.touch_controls_mode),
            )?;
            report_set_number(
                &object,
                "touchLookSensitivity",
                f64::from(self.touch_look_sensitivity),
            )?;
            report_set_bool(
                &object,
                "touchLookSensitivityAvailable",
                self.touch_settings_available,
            )?;
            write_session_state(&object, host.session_state())?;
            if let Some(pending) = host.external_session_start_snapshot() {
                report_set_bool(&object, "sessionStartPending", true)?;
                write_external_session_start(&object, &pending)?;
            }
            let effective_status = if self.status_overlay.visible {
                self.status_overlay.clone()
            } else {
                host.mono_status_overlay()
            };
            report_set_bool(&object, "statusOverlayVisible", effective_status.visible)?;
            report_set_bool(&object, "statusOverlayOk", effective_status.ok)?;
            report_set_string(&object, "statusOverlayMessage", &effective_status.message)?;
            let camera_position = Vec3::new(
                camera.camera.eye.x as f32,
                camera.camera.eye.y as f32,
                camera.camera.eye.z as f32,
            );
            report_set_number(
                &object,
                "pendingStreamWork",
                host.pending_stream_work(camera_position) as f64,
            )?;
            if let Some(stats) = host.runtime_stats() {
                report_set_string(&object, "hostMode", stats.host_mode.label())?;
                report_set_string(
                    &object,
                    "runnerKind",
                    stats.server_runner_kind.map_or("none", |kind| kind.label()),
                )?;
                let visible_chunk_capacity =
                    usize::try_from((u64::from(stats.render_distance) * 2 + 1).pow(2))
                        .unwrap_or(usize::MAX);
                report_set_number(
                    &object,
                    "loadedChunkCount",
                    stats.loaded_chunks.min(visible_chunk_capacity) as f64,
                )?;
                report_set_number(&object, "commandCount", stats.command_count as f64)?;
                report_set_number(&object, "updateCount", stats.update_count as f64)?;
                report_set_number(
                    &object,
                    "snapshotUpdateCount",
                    stats.snapshot_update_count as f64,
                )?;
                report_set_number(
                    &object,
                    "sectionBlockUpdateCount",
                    stats.section_block_update_count as f64,
                )?;
                report_set_number(
                    &object,
                    "unloadUpdateCount",
                    stats.unload_update_count as f64,
                )?;
                report_set_number(
                    &object,
                    "runnerCommandQueueDepth",
                    stats.server_command_queue_depth as f64,
                )?;
                report_set_number(
                    &object,
                    "runnerUpdateQueueDepth",
                    stats.server_update_queue_depth as f64,
                )?;
                report_set_number(&object, "runnerPendingJobs", stats.pending_jobs as f64)?;
                report_set_number(
                    &object,
                    "runnerPendingPublications",
                    stats.pending_publications as f64,
                )?;
                report_set_number(
                    &object,
                    "runnerPendingPersistenceLoads",
                    stats.pending_persistence_loads as f64,
                )?;
                report_set_number(
                    &object,
                    "runnerPendingPersistenceSaves",
                    stats.pending_persistence_saves as f64,
                )?;
                report_set_number(
                    &object,
                    "pendingRenderChunks",
                    stats.pending_render_chunks as f64,
                )?;
                report_set_number(
                    &object,
                    "pendingCompileJobCount",
                    stats.pending_render_compile_jobs as f64,
                )?;
                report_set_bool(&object, "chunkLoaded", stats.loaded_chunks > 0)?;
                report_set_bool(&object, "meshBuilt", self.last_frame.section_count > 0)?;
                report_set_number(
                    &object,
                    "residentSectionCount",
                    self.last_frame.section_count as f64,
                )?;
                let settled = stats.server_command_queue_depth == 0
                    && stats.server_update_queue_depth == 0
                    && stats.pending_jobs == 0
                    && stats.pending_publications == 0
                    && stats.pending_persistence_loads == 0
                    && stats.pending_persistence_saves == 0
                    && stats.pending_render_compile_jobs == 0
                    && host.pending_stream_work(camera_position) == 0;
                report_set_bool(&object, "streamingIdle", settled)?;
                report_set_bool(&object, "renderPendingWork", !settled)?;
                report_set_number(
                    &object,
                    "renderDirtyChunkCount",
                    stats.pending_render_chunks as f64,
                )?;
                report_set_number(
                    &object,
                    "renderInflightSectionCount",
                    stats.inflight_render_sections as f64,
                )?;
            }
            if let Some(diagnostics) = host.runtime_poll_diagnostics() {
                report_set_string(
                    &object,
                    "runnerTransportKind",
                    diagnostics.runner_frame_metrics.transport_kind.label(),
                )?;
                report_set_string(
                    &object,
                    "worldgenTransportKind",
                    diagnostics
                        .worldgen_job_frame_metrics
                        .transport_kind
                        .label(),
                )?;
                report_set_string(
                    &object,
                    "lightTransportKind",
                    diagnostics
                        .light_status_job_frame_metrics
                        .transport_kind
                        .label(),
                )?;
                report_set_bool(
                    &object,
                    "updatePumpStalled",
                    diagnostics.update_pump_stalled,
                )?;
                report_set_number(
                    &object,
                    "clientDeferredChunkDropBacklogItems",
                    diagnostics.client_deferred_chunk_drop_backlog_items as f64,
                )?;
                crate::web_canvas::set_worker_frame_metrics(
                    &object,
                    "runnerFrameMetrics",
                    diagnostics.runner_frame_metrics,
                )?;
                crate::web_canvas::set_worker_frame_metrics(
                    &object,
                    "worldgenJobFrameMetrics",
                    diagnostics.worldgen_job_frame_metrics,
                )?;
                crate::web_canvas::set_worker_frame_metrics(
                    &object,
                    "lightStatusJobFrameMetrics",
                    diagnostics.light_status_job_frame_metrics,
                )?;
                report_set_string(&object, "worldgenMailboxKind", "web-worker")?;
                report_set_string(&object, "lightStatusMailboxKind", "web-worker")?;
                report_set_number(
                    &object,
                    "worldgenMailboxPendingJobs",
                    diagnostics.scheduler_worldgen_mailbox_pending_jobs as f64,
                )?;
                report_set_number(
                    &object,
                    "lightStatusMailboxPendingStatuses",
                    diagnostics.scheduler_light_mailbox_pending_statuses as f64,
                )?;
            }
        } else {
            report_set_string(&object, "runnerKind", &self.last_runner_kind)?;
        }
        if let Some(summary) = summary {
            report_set_number(&object, "sectionCount", summary.render.section_count as f64)?;
            report_set_number(
                &object,
                "acceptedCompileSectionCount",
                summary.upload.completed_compile_section_count as f64,
            )?;
            report_set_number(
                &object,
                "drawnSectionCount",
                summary.render.drawn_section_count as f64,
            )?;
            report_set_number(
                &object,
                "drawnIndexCount",
                summary.render.drawn_index_count as f64,
            )?;
            report_set_number(&object, "actorCount", summary.render.actor_count as f64)?;
            report_set_number(
                &object,
                "drawnActorCount",
                summary.render.drawn_actor_count as f64,
            )?;
            report_set_number(
                &object,
                "guiCommandCount",
                summary.render.gui_command_count as f64,
            )?;
            report_set_number(&object, "pollUpdates", summary.upload.poll_updates as f64)?;
            report_set_number(
                &object,
                "deferredDropBacklogItems",
                summary.upload.poll_client_deferred_chunk_drop_backlog_items as f64,
            )?;
            report_set_bool(&object, "playable", summary.render.drawn_section_count > 0)?;
        }
        report_set_number(
            &object,
            "sectionCount",
            self.last_frame.section_count as f64,
        )?;
        report_set_number(
            &object,
            "drawnSectionCount",
            self.last_frame.drawn_section_count as f64,
        )?;
        report_set_number(
            &object,
            "frustumSectionCount",
            self.last_frame.frustum_section_count as f64,
        )?;
        report_set_number(&object, "indexCount", self.last_frame.index_count as f64)?;
        report_set_number(
            &object,
            "drawnIndexCount",
            self.last_frame.drawn_index_count as f64,
        )?;
        report_set_number(&object, "actorCount", self.last_frame.actor_count as f64)?;
        report_set_number(
            &object,
            "drawnActorCount",
            self.last_frame.drawn_actor_count as f64,
        )?;
        report_set_number(
            &object,
            "guiCommandCount",
            self.last_frame.gui_command_count as f64,
        )?;
        report_set_number(
            &object,
            "flatHudRetainedRebuilds",
            self.last_frame.flat_hud_rebuilds as f64,
        )?;
        report_set_number(
            &object,
            "flatHudRetainedCacheHits",
            self.last_frame.flat_hud_cache_hits as f64,
        )?;
        report_set_number(
            &object,
            "deferredDropBacklogItems",
            self.last_frame.deferred_drop_backlog as f64,
        )?;
        report_set_number(
            &object,
            "farLodRegionDrawCount",
            self.last_frame.far_lod_region_draw_count as f64,
        )?;
        report_set_number(
            &object,
            "farLodUploadedBytes",
            self.last_frame.far_lod_uploaded_bytes as f64,
        )?;
        report_set_number(&object, "meshBuildCount", self.mesh_build_count as f64)?;
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

fn aim_player_host_at_block(host: &mut McloneSceneHost, block: BlockPos) {
    let target = Vec3::new(
        block.x as f32 + 0.5,
        block.y as f32 + 0.5,
        block.z as f32 + 0.5,
    );
    let eye = target + Vec3::new(0.0, 1.8, -1.3);
    let direction = (target - eye).normalize_or_zero();
    let yaw = direction.x.atan2(direction.z);
    let pitch = direction.y.clamp(-1.0, 1.0).asin();
    host.set_mono_player_camera(
        Vec3d::new(f64::from(eye.x), f64::from(eye.y), f64::from(eye.z)),
        f64::from(yaw),
        f64::from(pitch),
        4.3,
    );
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

fn browser_now_millis() -> f64 {
    js_sys::Date::now()
}

fn touch_mode_label(mode: TouchControlsMode) -> &'static str {
    match mode {
        TouchControlsMode::Auto => "auto",
        TouchControlsMode::On => "on",
        TouchControlsMode::Off => "off",
    }
}

fn screen_label(screen: Option<GameScreen>) -> &'static str {
    match screen {
        None => "none",
        Some(GameScreen::Title) => "title",
        Some(GameScreen::WorldList) => "worldList",
        Some(GameScreen::WorldCreate) => "worldCreate",
        Some(GameScreen::WorldDeleteConfirm { .. }) => "worldDeleteConfirm",
        Some(GameScreen::NewWorld) => "newWorld",
        Some(GameScreen::JoinRemote) => "joinRemote",
        Some(GameScreen::Pause) => "pause",
        Some(GameScreen::Help { .. }) => "help",
        Some(GameScreen::BlockPalette) => "blockPalette",
        Some(GameScreen::Options { .. }) => "options",
        Some(GameScreen::OptionsCategory { .. }) => "optionsCategory",
        Some(GameScreen::ServerSettings { .. }) => "serverSettings",
        Some(GameScreen::AssetPacks { .. }) => "assetPacks",
    }
}

fn screen_options_parent(screen: Option<GameScreen>) -> Option<&'static str> {
    match screen {
        Some(
            GameScreen::Options { parent }
            | GameScreen::OptionsCategory { parent, .. }
            | GameScreen::ServerSettings { parent }
            | GameScreen::AssetPacks { parent },
        ) => Some(match parent {
            GameOptionsParent::Title => "title",
            GameOptionsParent::Pause => "pause",
        }),
        _ => None,
    }
}

fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

fn write_block_target(
    object: &js_sys::Object,
    target: &BlockInteractionTarget,
) -> Result<(), String> {
    let hit = target.hit;
    report_set_bool(object, "hit", !hit.miss)?;
    report_set_string(object, "hitType", if hit.miss { "miss" } else { "block" })?;
    report_set_bool(object, "inside", hit.inside)?;
    report_set_string(object, "direction", direction_label(hit.direction))?;
    report_set_number(object, "blockX", f64::from(hit.block_pos.x))?;
    report_set_number(object, "blockY", f64::from(hit.block_pos.y))?;
    report_set_number(object, "blockZ", f64::from(hit.block_pos.z))?;
    report_set_number(object, "hitX", hit.location.x)?;
    report_set_number(object, "hitY", hit.location.y)?;
    report_set_number(object, "hitZ", hit.location.z)
}

fn write_catalog_request(
    object: &js_sys::Object,
    request: &WorldCatalogRequest,
) -> Result<(), String> {
    match request {
        WorldCatalogRequest::ListWorlds => {
            report_set_string(object, "catalogOperation", "listWorlds")
        }
        WorldCatalogRequest::CreateWorld { options } => {
            report_set_string(object, "catalogOperation", "createWorld")?;
            report_set_string(object, "catalogDisplayName", &options.display_name)?;
            report_set_number(object, "catalogWorldSeed", options.seed as f64)?;
            report_set_string(object, "catalogWorldSeedText", &options.seed.to_string())?;
            if let Some(id) = &options.requested_id {
                report_set_string(object, "catalogRequestedId", id.as_str())?;
            }
            Ok(())
        }
        WorldCatalogRequest::OpenWorld { id } => {
            report_set_string(object, "catalogOperation", "openWorld")?;
            report_set_string(object, "catalogWorldId", id.as_str())
        }
        WorldCatalogRequest::DeleteWorld { id } => {
            report_set_string(object, "catalogOperation", "deleteWorld")?;
            report_set_string(object, "catalogWorldId", id.as_str())
        }
    }
}

fn write_external_session_start(
    object: &js_sys::Object,
    pending: &ExternalSceneSessionStart,
) -> Result<(), String> {
    match &pending.descriptor {
        ActiveSessionDescriptor::LocalWorld {
            seed,
            id,
            display_name,
        } => {
            report_set_string(object, "sessionOperationKind", "localWorld")?;
            report_set_number(object, "sessionSeed", *seed as f64)?;
            report_set_string(object, "sessionSeedText", &seed.to_string())?;
            if let Some(id) = id {
                report_set_string(object, "sessionWorldId", id.as_str())?;
                report_set_bool(object, "catalogSessionStart", true)?;
                report_set_string(object, "catalogWorldId", id.as_str())?;
                report_set_string(
                    object,
                    "catalogWorldDisplayName",
                    display_name.as_deref().unwrap_or(id.as_str()),
                )?;
                report_set_number(object, "catalogWorldSeed", *seed as f64)?;
                report_set_string(object, "catalogWorldSeedText", &seed.to_string())?;
                report_set_string(
                    object,
                    "catalogSessionRequest",
                    match pending.request {
                        SessionStartRequest::CreateLocalWorld { .. } => "createLocalWorld",
                        _ => "openLocalWorld",
                    },
                )?;
            }
        }
        ActiveSessionDescriptor::Remote { endpoint } => {
            report_set_string(object, "sessionOperationKind", "remote")?;
            report_set_string(object, "sessionRemoteEndpoint", &endpoint.address)?;
            report_set_string(object, "remoteEndpoint", &endpoint.address)?;
        }
    }
    Ok(())
}

fn write_session_state(object: &js_sys::Object, state: &GameSessionState) -> Result<(), String> {
    match state {
        GameSessionState::NoSession => report_set_string(object, "sessionState", "none"),
        GameSessionState::Starting { request } => {
            report_set_string(object, "sessionState", "starting")?;
            match request {
                SessionStartRequest::CreateLocalWorld { options } => {
                    report_set_string(object, "sessionKind", "localWorld")?;
                    report_set_number(object, "sessionSeed", options.seed as f64)?;
                    report_set_string(object, "sessionSeedText", &options.seed.to_string())
                }
                SessionStartRequest::OpenLocalWorld { id } => {
                    report_set_string(object, "sessionKind", "localWorld")?;
                    report_set_string(object, "sessionWorldId", id.as_str())
                }
                SessionStartRequest::JoinRemote { endpoint } => {
                    report_set_string(object, "sessionKind", "remote")?;
                    report_set_string(object, "sessionRemoteEndpoint", &endpoint.address)
                }
                SessionStartRequest::Unknown => report_set_string(object, "sessionKind", "unknown"),
            }
        }
        GameSessionState::Active { session } => {
            report_set_string(object, "sessionState", "active")?;
            match session {
                ActiveSessionDescriptor::LocalWorld { seed, id, .. } => {
                    report_set_string(object, "sessionKind", "localWorld")?;
                    report_set_number(object, "sessionSeed", *seed as f64)?;
                    report_set_string(object, "sessionSeedText", &seed.to_string())?;
                    if let Some(id) = id {
                        report_set_string(object, "sessionWorldId", id.as_str())?;
                    }
                    Ok(())
                }
                ActiveSessionDescriptor::Remote { endpoint } => {
                    report_set_string(object, "sessionKind", "remote")?;
                    report_set_string(object, "sessionRemoteEndpoint", &endpoint.address)
                }
            }
        }
        GameSessionState::Failed { request, error } => {
            report_set_string(object, "sessionState", "failed")?;
            report_set_string(object, "sessionFailureMessage", &error.message)?;
            let _ = request;
            Ok(())
        }
    }
}

fn report_set_bool(object: &js_sys::Object, key: &str, value: bool) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_bool(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set report field {key}: {error:?}"))
}

fn report_set_number(object: &js_sys::Object, key: &str, value: f64) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_f64(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set report field {key}: {error:?}"))
}

fn report_set_string(object: &js_sys::Object, key: &str, value: &str) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_str(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set report field {key}: {error:?}"))
}

fn js_error(error: anyhow::Error) -> JsValue {
    JsValue::from_str(&format!("{error:#}"))
}
