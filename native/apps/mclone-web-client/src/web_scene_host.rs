//! Thin production browser driver around the shared scene host.
//!
//! This module owns only canvas/surface resources, browser lifecycle facts,
//! typed asynchronous operation completion, and wasm-bindgen reports. Gameplay,
//! session policy, camera semantics, streaming admission, frame assembly, UI,
//! and asset epochs remain in `McloneSceneHost`.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use glam::{Vec2, Vec3};
use mclone_app_runtime::chunk_tracking_radius_for_render_distance;
use mclone_app_runtime::client_entry::{
    ClientEntryController, ClientEntryEffect, ClientEntryResolution, ClientHostAvailability,
};
use mclone_app_runtime::client_experience::web_client_experience_profile;
use mclone_app_runtime::frame_render::FlatSurfacePresentation;
use mclone_app_runtime::input_preferences::{
    ClientInputPreferences, PreferenceKeyValueStore, parse_touch_controls_mode,
    touch_controls_mode_label,
};
use mclone_app_runtime::platform_operation::{PlatformOperationCompletion, PlatformOperationToken};
use mclone_app_runtime::prepared_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, MINECRAFT_REFERENCE_PACK_ID, PreparedSceneAssets,
};
use mclone_app_runtime::scene_session_runtime::RuntimeRenderPriority;
#[cfg(all(test, target_arch = "wasm32"))]
use mclone_app_runtime::session::RemoteSessionEndpoint;
use mclone_app_runtime::session::{ActiveSessionDescriptor, GameSessionState, SessionStartRequest};
use mclone_app_runtime::world_catalog::{
    WorldCatalogError, WorldCatalogErrorKind, WorldCatalogResponse,
};
use mclone_assets::{
    MemoryAssetSource, PackedAssetSource, default_player_figure_path, load_prepared_figure,
};
use mclone_client::BlockInteractionTarget;
use mclone_core::{BlockPos, ChunkPos, Direction, Vec3d};
use mclone_input::{
    ControllerLayoutFamily, FlatInputAction, InputCapabilities, InputCapabilityState,
    InputDeviceKind, InputPreferences, KeyboardKey, MouseWheelDirection, PointerButton,
    TouchContactPhase, TouchControlsMode, TouchInputAdapter, TouchInputEvent, TouchInputSettings,
    TouchUiContactRoute, TouchUiContactTracker,
};
use mclone_render::actor_composition_fixture::ActorCompositionFixture;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, TexturedSectionRenderOptions,
};
use mclone_render::composition_fixture::ComplementaryHalfSpaceTerrainFixture;
use mclone_render::prepared_figure::{PreparedFigureDrawResources, clear_prepared_figure_target};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render::uniform::SINGLE_VIEW_SLOT;
use mclone_scene::{
    AssetReplacementStatus, ExternalSceneSessionStart, HostEffects, McloneSceneHost,
    McloneSceneHostOptions, MonoInputDisposition, MonoInteractiveInputRouter,
    MonoSceneFrameSummary, MonoUiContext, MonoUiPresentation, MonoWorldActionStatus,
};
use mclone_ui::{
    GameHelpParent, GameOptionsParent, GameScreen, GameTouchSettings, GameUiAction, GuiScale,
    Point, StatusOverlay, TouchJoystickOverlay, TouchOverlay, touch_control_at,
    touch_menu_button_rect,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::web_bootstrap::{InitialAssetPacks, WebBootstrapResources, WebHostCapabilities};
use crate::web_canvas::{
    WebCanvasContext, WebSceneRuntimeService, WebStartupConfig, WebWorldStorageStartupOptions,
    gui_key_from_label, prepare_web_scene_assets_from_pack,
    prepare_web_scene_assets_from_selection, ui_action_label, web_asset_pack_catalog,
};
use crate::web_catalog_execution::WebCatalogExecution;
use crate::web_gamepad::BrowserGamepadCollector;
use crate::web_render_worker::WebRenderWorkerCoordinator;
use crate::web_scene_protocol::{
    WebSceneFrameAdmission, WebSceneFrameDriverPolicy, WebSceneFrameState, WebScenePlatformServices,
};

use mclone_app_runtime::local_profile::WEB_ASSET_PACK_PREFERENCE_KEY;

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

#[derive(Clone, Copy, Debug, Default)]
struct WebPreferenceKeyValueStore;

impl PreferenceKeyValueStore for WebPreferenceKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        else {
            return Ok(None);
        };
        storage
            .get_item(key)
            .map_err(|error| anyhow::anyhow!("read browser localStorage: {error:?}"))
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        let storage = web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or_else(|| anyhow::anyhow!("browser localStorage is unavailable"))?;
        storage
            .set_item(key, value)
            .map_err(|error| anyhow::anyhow!("write browser localStorage: {error:?}"))
    }

    fn label(&self) -> &str {
        "browser localStorage input preferences"
    }
}
use crate::web_server_worker::WebIntegratedServerRunnerConfig;

const RESUME_OBSERVATION_FRAMES: u8 = 8;
const INITIAL_PRESENTATION_STABLE_FRAMES: u8 = 6;

#[derive(Clone, Copy, Debug, Default)]
struct WebHostEffects {
    mouse_lock_requested: Option<bool>,
    touch_controls_mode: Option<TouchControlsMode>,
    quit_to_title: bool,
    exit: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct WebFrameInputEffects {
    clear_transient_input: bool,
    pointer_capture_desired: Option<bool>,
    exit_requested: bool,
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

    fn set_world_render_scale_mode(
        &mut self,
        _mode: mclone_ui::GameWorldRenderScaleMode,
    ) -> Result<()> {
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

struct WebGraphicsPreferenceStorage;

impl mclone_app_runtime::graphics_preferences::ClientGraphicsPreferenceStorage
    for WebGraphicsPreferenceStorage
{
    fn load(
        &self,
    ) -> Result<Option<mclone_app_runtime::graphics_preferences::ClientGraphicsPreferences>> {
        let store = WebPreferenceKeyValueStore;
        let Some(json) =
            store.get(mclone_app_runtime::graphics_preferences::GRAPHICS_PREFERENCE_STORAGE_KEY)?
        else {
            return Ok(None);
        };
        mclone_app_runtime::graphics_preferences::ClientGraphicsPreferences::from_json(&json)
            .map(Some)
    }

    fn store(
        &self,
        preferences: &mclone_app_runtime::graphics_preferences::ClientGraphicsPreferences,
    ) -> Result<()> {
        preferences.store(&WebPreferenceKeyValueStore)
    }

    fn label(&self) -> &str {
        "browser localStorage mclone.graphics.preferences.v1"
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LastFrameStats {
    section_count: usize,
    drawn_section_count: usize,
    frustum_section_count: usize,
    index_count: u32,
    drawn_index_count: u32,
    grass_resident_patch_count: u32,
    grass_drawn_patch_count: u32,
    grass_estimated_blade_count: u32,
    grass_draw_calls: usize,
    grass_resident_bytes: u64,
    grass_interaction_field_count: u32,
    grass_interaction_active_cell_count: u32,
    grass_interaction_stamp_count: u32,
    grass_interaction_recenter_count: u32,
    grass_interaction_reset_count: u32,
    grass_interaction_uploaded_bytes: u64,
    actor_count: usize,
    drawn_actor_count: usize,
    gui_command_count: usize,
    flat_hud_rebuilds: usize,
    flat_hud_cache_hits: usize,
    deferred_drop_backlog: usize,
    far_lod_region_draw_count: usize,
    far_lod_uploaded_bytes: usize,
    accepted_compile_section_count: usize,
    poll_updates: usize,
    render_views_ms: f64,
}

enum WebRuntimeStartEffect {
    Integrated(WebIntegratedServerRunnerConfig),
    Remote(String),
    #[cfg(test)]
    TestOnlyRemote(String),
}
#[cfg(test)]
const _: fn(String) -> WebRuntimeStartEffect = WebRuntimeStartEffect::TestOnlyRemote;

enum WebSceneOperationEffect {
    Runtime {
        pending: ExternalSceneSessionStart,
        start: WebRuntimeStartEffect,
        center: ChunkPos,
        render_distance: u32,
    },
    IndexedDb(WebCatalogExecution),
    Assets {
        token: PlatformOperationToken,
        content_generation: u64,
        selected_file_count: usize,
        effect: WebAssetPackPreparationEffect,
    },
}

struct WebAssetPackPreparationEffect {
    content_generation: u64,
    authored: Vec<u8>,
    reference: Vec<u8>,
    fallback: Vec<u8>,
    diagnostic: Vec<u8>,
    selection: mclone_assets::AssetPackSelection,
    presentation: mclone_assets::TexturePresentation,
    render_worker: WebRenderWorkerCoordinator,
}

type WebRuntimeResources = (String, String, String, String);

enum WebSceneOperationCompletion {
    Runtime {
        pending: ExternalSceneSessionStart,
        outcome: Result<crate::WebRuntime, String>,
    },
    IndexedDb {
        token: PlatformOperationToken,
        outcome: Result<WorldCatalogResponse, String>,
    },
    Assets {
        token: PlatformOperationToken,
        content_generation: u64,
        selected_file_count: usize,
        outcome: Result<PreparedSceneAssets, String>,
    },
}

/// Opaque Rust-admitted Promise or IndexedDB effect; meaning and identity stay private.
#[wasm_bindgen]
pub struct WebSceneOperation {
    effect: Option<WebSceneOperationEffect>,
    completion: Rc<RefCell<Option<WebSceneOperationCompletion>>>,
    indexed_db_token: Option<PlatformOperationToken>,
}

impl WebSceneOperation {
    fn new(effect: WebSceneOperationEffect) -> Self {
        Self {
            effect: Some(effect),
            completion: Default::default(),
            indexed_db_token: None,
        }
    }
}

/// The operation-type-blind part of the browser drain. The exported host
/// selects admitted work and folds the returned completion into its shared
/// owner; this core owns the unchanged take/complete vocabulary between them.
struct WebSceneOperationDrain;

impl WebSceneOperationDrain {
    fn take_scene_operation(effect: WebSceneOperationEffect) -> WebSceneOperation {
        WebSceneOperation::new(effect)
    }

    fn complete_scene_operation(
        operation: &mut WebSceneOperation,
    ) -> Result<WebSceneOperationCompletion, JsValue> {
        operation
            .completion
            .borrow_mut()
            .take()
            .ok_or_else(|| JsValue::from_str("scene operation was consumed"))
    }
}

#[wasm_bindgen]
impl WebSceneOperation {
    /// Start Worker/render preparation work without retaining a host borrow.
    #[wasm_bindgen(js_name = start)]
    pub fn start(&mut self) -> Result<js_sys::Promise, JsValue> {
        if matches!(self.effect, Some(WebSceneOperationEffect::IndexedDb(..))) {
            return Err(JsValue::from_str(
                "IndexedDB scene operation must use its action plan",
            ));
        }
        let effect = self
            .effect
            .take()
            .ok_or_else(|| JsValue::from_str("scene operation was already started"))?;
        let completion = Rc::clone(&self.completion);
        Ok(wasm_bindgen_futures::future_to_promise(async move {
            let completed = match effect {
                WebSceneOperationEffect::Runtime {
                    pending,
                    start,
                    center,
                    render_distance,
                } => WebSceneOperationCompletion::Runtime {
                    pending,
                    outcome: execute_web_runtime_start(start, center, render_distance).await,
                },
                WebSceneOperationEffect::Assets {
                    token,
                    content_generation,
                    selected_file_count,
                    effect,
                } => WebSceneOperationCompletion::Assets {
                    token,
                    content_generation,
                    selected_file_count,
                    outcome: execute_web_asset_pack_preparation(effect).await,
                },
                WebSceneOperationEffect::IndexedDb(..) => unreachable!(),
            };
            *completion.borrow_mut() = Some(completed);
            Ok(JsValue::UNDEFINED)
        }))
    }

    /// Take the optional mechanical IndexedDB plan. `undefined` means the
    /// operation uses the Promise executor returned by `start()`.
    #[wasm_bindgen(js_name = takeIndexedDbExecution)]
    pub fn take_indexed_db_execution(&mut self) -> Result<Option<WebCatalogExecution>, JsValue> {
        if !matches!(self.effect, Some(WebSceneOperationEffect::IndexedDb(..))) {
            return Ok(None);
        }
        let Some(WebSceneOperationEffect::IndexedDb(execution)) = self.effect.take() else {
            unreachable!()
        };
        self.indexed_db_token = Some(execution.token());
        Ok(Some(execution))
    }

    /// Return a completed IndexedDB plan to the opaque operation.
    #[wasm_bindgen(js_name = completeIndexedDbExecution)]
    pub fn complete_indexed_db_execution(
        &mut self,
        execution: &WebCatalogExecution,
        error: Option<String>,
    ) -> Result<(), JsValue> {
        let token = self
            .indexed_db_token
            .take()
            .ok_or_else(|| JsValue::from_str("scene operation does not use IndexedDB"))?;
        if execution.token() != token {
            return Err(JsValue::from_str(
                "IndexedDB execution does not match the scene operation",
            ));
        }
        *self.completion.borrow_mut() = Some(WebSceneOperationCompletion::IndexedDb {
            token,
            outcome: error.map_or_else(|| execution.response(), Err),
        });
        Ok(())
    }
}

async fn execute_web_runtime_start(
    start: WebRuntimeStartEffect,
    center: ChunkPos,
    render_distance: u32,
) -> Result<crate::WebRuntime, String> {
    let mut runtime = match start {
        WebRuntimeStartEffect::Integrated(config) => {
            crate::WebRuntime::web_worker_integrated_at(config, center).await?
        }
        WebRuntimeStartEffect::Remote(url) => {
            crate::WebRuntime::websocket_remote_at(url, center).await?
        }
        #[cfg(test)]
        WebRuntimeStartEffect::TestOnlyRemote(url) => {
            return Err(format!("test-only remote completion: {url}"));
        }
    };
    runtime.request_chunk_view_deferred(
        center,
        render_distance,
        chunk_tracking_radius_for_render_distance(render_distance),
    )?;
    Ok(runtime)
}

async fn execute_web_asset_pack_preparation(
    effect: WebAssetPackPreparationEffect,
) -> Result<PreparedSceneAssets, String> {
    let WebAssetPackPreparationEffect {
        content_generation,
        authored,
        reference,
        fallback,
        diagnostic,
        selection,
        presentation,
        render_worker,
    } = effect;
    let profile = mclone_assets::TextureVisualProfile::from_selection(&selection)
        .ok_or_else(|| "browser asset request is not a named visual profile".to_owned())?;
    render_worker
        .prepare_asset_candidate(
            content_generation,
            authored.clone(),
            reference.clone(),
            fallback.clone(),
            diagnostic.clone(),
            profile,
            presentation,
        )
        .await?;
    prepare_web_scene_assets_from_selection(
        content_generation,
        authored,
        reference,
        fallback,
        diagnostic,
        selection,
        presentation,
    )
    .inspect_err(|_| render_worker.settle_asset_epoch(content_generation, false))
}

/// Browser resource/presentation rim around the one shared scene-policy owner.
#[wasm_bindgen]
pub struct WebSceneHost {
    context: WebCanvasContext,
    depth: ChunkDepthTarget,
    split_presentation: Option<FlatSurfacePresentation>,
    host: Option<McloneSceneHost>,
    platform: WebScenePlatformServices,
    frame_policy: WebSceneFrameDriverPolicy,
    frame_count: u64,
    rendered_frame_count: u64,
    initial_presentation_stable_frames: u8,
    startup_status_visible: bool,
    last_visible_frame_millis: Option<f64>,
    max_frame_gap_millis: f64,
    movement_input_applied: bool,
    dom_input_frame_count: u64,
    interaction_sent: bool,
    settings_effect_applied: bool,
    pause_ui_rendered: bool,
    resized: bool,
    background_save_count: u64,
    resume_frames_remaining: u8,
    max_resume_poll_updates: usize,
    max_resume_drop_backlog: usize,
    shutdown_complete: bool,
    render_worker: WebRenderWorkerCoordinator,
    startup_runtime_storage: Option<WebWorldStorageStartupOptions>,
    initial_asset_packs: InitialAssetPacks,
    asset_pack_file_count: usize,
    pending_asset_pack_file_count: Option<(u64, usize)>,
    status_overlay: StatusOverlay,
    touch_look_sensitivity: f32,
    touch_settings_available: bool,
    touch_controls_mode: TouchControlsMode,
    input_preferences: ClientInputPreferences,
    input_preference_error: Option<String>,
    input_capability_state: InputCapabilityState,
    touch_overlay: TouchOverlay,
    interactive_input: MonoInteractiveInputRouter,
    gamepad_collector: BrowserGamepadCollector,
    frame_input_effects: WebFrameInputEffects,
    touch_input: TouchInputAdapter,
    ui_touch: TouchUiContactTracker,
    last_action: Option<GameUiAction>,
    last_frame: LastFrameStats,
    command_count: usize,
    update_count: usize,
    interaction_count: usize,
    mesh_build_count: usize,
    render_color_profile: String,
    last_runner_kind: String,
}

impl WebSceneHost {
    /// Deterministic device/resource-generation recovery hook. Production
    /// device recreation can call the same prepared-asset entry after replacing
    /// `WebCanvasContext`; the smoke uses the current device to verify retained
    /// world cancellation and stale-start rejection without fabricating a loss.
    fn rebuild_render_resources_for_smoke(&mut self) -> Result<JsValue, JsValue> {
        let assets = self
            .host_ref()?
            .active_asset_snapshot_for_epoch(self.host_ref()?.active_asset_epoch());
        let (device, queue) = (&self.context.device, &self.context.queue);
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
            .rebuild_mono_render_resources_with_assets(
                device,
                queue,
                assets.actors.atlas.clone(),
                &assets.actors.figures,
                &assets.screen_effects,
            )
            .map_err(js_error)?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    /// Present the shared renderer-owned complementary-half-space fixture.
    /// The browser adapter supplies only its WebGPU target and presentation.
    fn render_half_space_terrain_proof(&mut self) -> Result<JsValue, JsValue> {
        let fixture = ComplementaryHalfSpaceTerrainFixture::new(
            &self.context.device,
            &self.context.queue,
            self.context.format,
        )
        .map_err(js_error)?;
        let surface_texture = self
            .context
            .surface
            .get_current_texture()
            .map_err(|error| {
                JsValue::from_str(&format!("acquire half-space proof surface: {error}"))
            })?;
        let view = surface_texture.texture.create_view(&Default::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_web_half_space_terrain_proof_encoder"),
                });
        let size = [self.context.width, self.context.height];
        let report = fixture
            .render_mono(
                &self.context.device,
                &self.context.queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &view,
                    &self.depth.view,
                    size,
                    ComplementaryHalfSpaceTerrainFixture::clear_color(),
                ),
                ComplementaryHalfSpaceTerrainFixture::render_view(size, 0.0),
                SINGLE_VIEW_SLOT,
            )
            .map_err(js_error)?;
        self.context.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
        report_set_string(&object, "backend", "browser-webgpu").map_err(JsValue::from)?;
        report_set_bool(
            &object,
            "sharedImmutableResources",
            fixture.shares_immutable_resources(),
        )
        .map_err(JsValue::from)?;
        report_set_bool(
            &object,
            "clippedRendererMaterialized",
            fixture.clipped_renderer_materialized(),
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "leftDrawnSections",
            report.left.drawn_section_count as f64,
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "rightDrawnSections",
            report.right.drawn_section_count as f64,
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "leftDrawnIndices",
            f64::from(report.left.drawn_index_count),
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "rightDrawnIndices",
            f64::from(report.right.drawn_index_count),
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "leftTranslucentSections",
            report.left_translucent_section_count as f64,
        )
        .map_err(JsValue::from)?;
        report_set_number(
            &object,
            "rightTranslucentSections",
            report.right_translucent_section_count as f64,
        )
        .map_err(JsValue::from)?;
        Ok(object.into())
    }

    /// Present the canonical player through the shared startup-prepared path.
    /// The browser adapter embeds canonical semantic JSON and supplies only
    /// its WebGPU target, review camera, and presentation.
    fn render_prepared_figure_proof(&mut self) -> Result<JsValue, JsValue> {
        let figure_path = default_player_figure_path();
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            figure_path.clone(),
            include_str!("../../../../assets/mclone/figures/player.figure.json"),
        );
        let figure =
            load_prepared_figure(&source, &figure_path).map_err(|error| js_error(error.into()))?;
        let mut draw = PreparedFigureDrawResources::new(
            &self.context.device,
            &self.context.queue,
            self.context.format,
            &figure,
        )
        .map_err(js_error)?;

        let surface_texture = self
            .context
            .surface
            .get_current_texture()
            .map_err(|error| {
                JsValue::from_str(&format!("acquire prepared figure proof surface: {error}"))
            })?;
        let view = surface_texture.texture.create_view(&Default::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_web_prepared_figure_proof_encoder"),
                });
        let size = [self.context.width, self.context.height];
        let target = RenderFrameTarget::color(&view, size).with_depth(&self.depth.view);
        clear_prepared_figure_target(
            &mut encoder,
            target,
            &self.depth.view,
            wgpu::Color {
                r: 0xed as f64 / 255.0,
                g: 0xf1 as f64 / 255.0,
                b: 0xf4 as f64 / 255.0,
                a: 1.0,
            },
        );

        let min = Vec3::from_array(figure.bounds.min);
        let max = Vec3::from_array(figure.bounds.max);
        let center = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.75);
        let fov_y_radians = 35.0_f32.to_radians();
        let distance = 2.2_f32.max((radius / (fov_y_radians * 0.5).sin()) * 1.12);
        let eye = center + Vec3::new(0.0, 0.2, 1.0).normalize() * distance;
        let stats = draw
            .render(
                &self.context.queue,
                &mut encoder,
                target,
                ChunkCamera {
                    eye: eye.to_array(),
                    target: center.to_array(),
                    up: [0.0, 1.0, 0.0],
                    fov_y_radians,
                    z_near: 0.01,
                    z_far: 100.0,
                }
                .render_view(size[0], size[1]),
            )
            .map_err(js_error)?;
        let gpu = draw.snapshot();
        self.context.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
        report_set_string(&object, "backend", "browser-webgpu").map_err(JsValue::from)?;
        report_set_string(&object, "compilerId", figure.diagnostics.compiler_id)
            .map_err(JsValue::from)?;
        if let Some(crc32) = figure.diagnostics.semantic_crc32 {
            report_set_string(&object, "semanticCrc32", &format!("{crc32:08x}"))
                .map_err(JsValue::from)?;
        }
        for (name, value) in [
            ("partCount", figure.parts.len() as u64),
            ("vertexCount", u64::from(stats.vertex_count)),
            ("indexCount", u64::from(stats.index_count)),
            ("drawRangeCount", figure.draw_ranges.len() as u64),
            ("atlasWidth", u64::from(figure.atlas.width)),
            ("atlasHeight", u64::from(figure.atlas.height)),
            ("drawCount", u64::from(stats.draw_count)),
            ("pipelineCount", gpu.pipeline_count),
            ("immutableUploadCount", gpu.immutable_upload_count),
            ("viewUniformWriteCount", gpu.view_uniform_write_count),
            ("multiviewPipelineCount", gpu.multiview_pipeline_count),
        ] {
            report_set_number(&object, name, value as f64).map_err(JsValue::from)?;
        }
        Ok(object.into())
    }

    /// Present the shared renderer-owned placed/clipped actor fixture.
    /// The browser adapter supplies only its WebGPU target and presentation.
    fn render_actor_composition_proof(&mut self) -> Result<JsValue, JsValue> {
        let mut fixture = ActorCompositionFixture::new(
            &self.context.device,
            &self.context.queue,
            self.context.format,
        )
        .map_err(js_error)?;
        let surface_texture = self
            .context
            .surface
            .get_current_texture()
            .map_err(|error| {
                JsValue::from_str(&format!("acquire actor composition proof surface: {error}"))
            })?;
        let view = surface_texture.texture.create_view(&Default::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_web_actor_composition_proof_encoder"),
                });
        let size = [self.context.width, self.context.height];
        let report = fixture
            .render_mono(
                &self.context.device,
                &self.context.queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &view,
                    &self.depth.view,
                    size,
                    ActorCompositionFixture::clear_color(),
                ),
                ActorCompositionFixture::render_view(size, 0.0),
                SINGLE_VIEW_SLOT,
            )
            .map_err(js_error)?;
        self.context.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
        report_set_string(&object, "backend", "browser-webgpu").map_err(JsValue::from)?;
        report_set_bool(
            &object,
            "sharedImmutableResources",
            fixture.shares_immutable_resources(),
        )
        .map_err(JsValue::from)?;
        for (name, value) in [
            (
                "leftTerrainSections",
                report.terrain.left.drawn_section_count,
            ),
            (
                "rightTerrainSections",
                report.terrain.right.drawn_section_count,
            ),
            (
                "unboundedSubmittedActors",
                report.unbounded.submitted_actor_count,
            ),
            ("unboundedDrawnActors", report.unbounded.drawn_actor_count),
            ("leftSubmittedActors", report.left.submitted_actor_count),
            ("leftDrawnActors", report.left.drawn_actor_count),
            (
                "leftSourceRejectedActors",
                report.left.source_rejected_actor_count,
            ),
            (
                "leftClipRejectedActors",
                report.left.clip_rejected_actor_count,
            ),
            (
                "leftFrustumRejectedActors",
                report.left.frustum_rejected_actor_count,
            ),
            ("rightSubmittedActors", report.right.submitted_actor_count),
            ("rightDrawnActors", report.right.drawn_actor_count),
            (
                "leftMeshRebuilds",
                report.left_resources.mesh.rebuild_count as usize,
            ),
            (
                "leftMeshUploads",
                report.left_resources.mesh.upload_count as usize,
            ),
            (
                "rightMeshRebuilds",
                report.right_resources.mesh.rebuild_count as usize,
            ),
            (
                "rightMeshUploads",
                report.right_resources.mesh.upload_count as usize,
            ),
            (
                "preparedFigureCount",
                report.right_resources.prepared_shared.figure_count,
            ),
            (
                "preparedImmutableUploads",
                report
                    .right_resources
                    .prepared_shared
                    .immutable_upload_count as usize,
            ),
            (
                "rightPreparedRecords",
                report.right_resources.prepared_world.actor_record_count,
            ),
            (
                "rightPreparedActors",
                report.right_resources.prepared_world.prepared_actor_count,
            ),
            (
                "rightLegacyActors",
                report.right_resources.prepared_world.legacy_actor_count,
            ),
            (
                "rightPreparedPoseEvaluations",
                report.right_resources.prepared_world.pose_evaluation_count as usize,
            ),
            (
                "rightPreparedPaletteWrites",
                report.right_resources.prepared_world.palette_write_count as usize,
            ),
            (
                "rightPreparedDraws",
                report.right_resources.prepared_world.draw_count as usize,
            ),
            (
                "placedPipelines",
                report.left_resources.placed_pipeline_count,
            ),
            (
                "clippedPlacedPipelines",
                report.left_resources.clipped_placed_pipeline_count,
            ),
        ] {
            report_set_number(&object, name, value as f64).map_err(JsValue::from)?;
        }
        Ok(object.into())
    }
}

/// Explicit test-client surface, constructed only by browser smoke pages.
#[wasm_bindgen]
pub struct WebSceneSmokeHarness {
    render_resource_generation: u64,
}

#[wasm_bindgen]
impl WebSceneSmokeHarness {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            render_resource_generation: 0,
        }
    }

    #[wasm_bindgen(js_name = rebuildRenderResourcesForSmoke)]
    pub fn rebuild_render_resources_for_smoke(
        &mut self,
        host: &mut WebSceneHost,
    ) -> Result<JsValue, JsValue> {
        let report = host.rebuild_render_resources_for_smoke()?;
        self.render_resource_generation = self.render_resource_generation.saturating_add(1);
        let object: js_sys::Object = report.unchecked_into();
        report_set_number(
            &object,
            "renderResourceGeneration",
            self.render_resource_generation as f64,
        )
        .map_err(JsValue::from)?;
        Ok(object.into())
    }

    #[wasm_bindgen(js_name = renderHalfSpaceTerrainProof)]
    pub fn render_half_space_terrain_proof(
        &mut self,
        host: &mut WebSceneHost,
    ) -> Result<JsValue, JsValue> {
        host.render_half_space_terrain_proof()
    }

    #[wasm_bindgen(js_name = renderPreparedFigureProof)]
    pub fn render_prepared_figure_proof(
        &mut self,
        host: &mut WebSceneHost,
    ) -> Result<JsValue, JsValue> {
        host.render_prepared_figure_proof()
    }

    #[wasm_bindgen(js_name = renderActorCompositionProof)]
    pub fn render_actor_composition_proof(
        &mut self,
        host: &mut WebSceneHost,
    ) -> Result<JsValue, JsValue> {
        host.render_actor_composition_proof()
    }

    #[wasm_bindgen(js_name = beginLobbySmokeWithChunkSpan)]
    pub fn begin_lobby_smoke_with_chunk_span(
        &mut self,
        host: &mut WebSceneHost,
        chunk_span: u32,
    ) -> Result<JsValue, JsValue> {
        host.begin_lobby_smoke_with_chunk_span(chunk_span)
    }
}

#[wasm_bindgen]
impl WebSceneHost {
    #[wasm_bindgen(js_name = pollControllerInput)]
    pub fn poll_controller_input_only(&mut self, now_millis: f64) -> Result<JsValue, JsValue> {
        let handled = self.poll_controller_input(now_millis)?;
        let value = self
            .finish_frame_operational_report(false, 0.0, false)
            .map_err(JsValue::from)?;
        let object: js_sys::Object = value.unchecked_into();
        report_set_bool(&object, "handled", handled).map_err(JsValue::from)?;
        Ok(object.into())
    }

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&mut self, now_millis: f64) -> Result<JsValue, JsValue> {
        self.render_worker
            .poll(
                now_millis,
                self.frame_count,
                self.rendered_frame_count,
                self.max_frame_gap_millis,
            )
            .map_err(|error| JsValue::from_str(&error))?;
        self.platform.observe_frame_time_millis(now_millis);
        let admission = self.frame_policy.admit_frame(now_millis);
        let WebSceneFrameAdmission::Render {
            delta_seconds,
            first_after_resume,
        } = admission
        else {
            return self
                .finish_frame_operational_report(false, 0.0, false)
                .map_err(JsValue::from);
        };

        if let Some(previous) = self.last_visible_frame_millis {
            self.max_frame_gap_millis = self
                .max_frame_gap_millis
                .max((now_millis - previous).max(0.0));
        }
        self.last_visible_frame_millis = Some(now_millis);
        self.frame_count = self.frame_count.saturating_add(1);
        self.dom_input_frame_count = self.dom_input_frame_count.saturating_add(1);
        let _ = self.poll_controller_input(now_millis)?;

        let supplemental = self
            .touch_controls_visible()
            .then(|| self.touch_input.held_frame())
            .flatten();
        let Some(host) = self.host.as_mut() else {
            self.frame_policy.shutdown();
            return self
                .finish_frame_operational_report(false, delta_seconds, first_after_resume)
                .map_err(JsValue::from);
        };
        self.movement_input_applied |= self
            .interactive_input
            .advance_held_frame(host, supplemental, delta_seconds)
            .map_err(js_error)?
            .camera_changed;

        let surface_texture = match self.context.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(error) => {
                let reason = format!("WebGPU surface acquisition failed: {error}");
                self.frame_policy.require_restart(reason.clone());
                self.startup_status_visible = false;
                self.status_overlay = StatusOverlay::new(reason, false);
                return self
                    .finish_frame_operational_report(false, delta_seconds, first_after_resume)
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
        let output_target =
            RenderFrameTarget::color(&view, [self.context.width, self.context.height]);
        let split_layout = host
            .auxiliary_split_layout(output_target.size)
            .map_err(js_error)?;
        let summary = if let Some(layout) = split_layout {
            let primary = layout.panes()[0];
            host.set_mono_ui_scale(GuiScale::from_pixels(primary.width, primary.height));
            match &mut self.split_presentation {
                Some(presentation) => presentation.resize(&self.context.device, layout),
                slot @ None => {
                    *slot = Some(FlatSurfacePresentation::new(
                        &self.context.device,
                        self.context.format,
                        layout,
                    ));
                }
            }
            let presentation = self
                .split_presentation
                .as_ref()
                .expect("split presentation initialized from active layout");
            let summary = host
                .render_auxiliary_split_surface_frame(
                    &self.context.device,
                    &self.context.queue,
                    &mut encoder,
                    presentation,
                )
                .map_err(js_error)?;
            presentation.present(&mut encoder, output_target);
            summary
        } else {
            if !host.flat_split_selected() {
                self.split_presentation = None;
            }
            host.set_mono_ui_scale(GuiScale::from_pixels(
                output_target.size[0],
                output_target.size[1],
            ));
            let render_view = host
                .mono_render_view(output_target.size)
                .map_err(js_error)?;
            host.render_mono_scene_frame(
                RenderFrameContext::new(
                    &self.context.device,
                    &self.context.queue,
                    &mut encoder,
                    output_target,
                ),
                &self.depth,
                render_view,
                MonoUiPresentation::ScreenSpaceHud,
            )
            .map_err(js_error)?
        };
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
            grass_resident_patch_count: summary.render.grass_resident_patch_count,
            grass_drawn_patch_count: summary.render.grass_drawn_patch_count,
            grass_estimated_blade_count: summary.render.grass_estimated_blade_count,
            grass_draw_calls: summary.render.grass_draw_calls,
            grass_resident_bytes: summary.render.grass_resident_bytes,
            grass_interaction_field_count: summary.render.grass_interaction_field_count,
            grass_interaction_active_cell_count: summary.render.grass_interaction_active_cell_count,
            grass_interaction_stamp_count: summary.render.grass_interaction_stamp_count,
            grass_interaction_recenter_count: summary.render.grass_interaction_recenter_count,
            grass_interaction_reset_count: summary.render.grass_interaction_reset_count,
            grass_interaction_uploaded_bytes: summary.render.grass_interaction_uploaded_bytes,
            actor_count: summary.render.actor_count,
            drawn_actor_count: summary.render.drawn_actor_count,
            gui_command_count: summary.render.gui_command_count,
            flat_hud_rebuilds: summary.render.flat_hud_retained_cache.rebuild_count as usize,
            flat_hud_cache_hits: summary.render.flat_hud_retained_cache.cache_hit_count as usize,
            deferred_drop_backlog: summary.upload.poll_client_deferred_chunk_drop_backlog_items,
            far_lod_region_draw_count: summary.render.far_lod_region_draw_count,
            far_lod_uploaded_bytes: summary.render.far_lod_uploaded_bytes,
            accepted_compile_section_count: summary.upload.completed_compile_section_count,
            poll_updates: summary.upload.poll_updates,
            render_views_ms: summary.timing.render_views_ms,
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
                    self.render_worker.settle_asset_epoch(epoch, true);
                }
                AssetReplacementStatus::Failed { .. } => {
                    self.pending_asset_pack_file_count = None;
                    self.render_worker.settle_asset_epoch(epoch, false);
                }
                _ => {}
            }
        }
        self.observe_initial_presentation_frame();
        if self.startup_status_visible
            && self.initial_presentation_stable_frames >= INITIAL_PRESENTATION_STABLE_FRAMES
        {
            self.startup_status_visible = false;
            self.status_overlay = StatusOverlay::hidden();
        }
        self.finish_frame_operational_report(true, delta_seconds, first_after_resume)
            .map_err(JsValue::from)
    }

    /// Normalize one raw browser keyboard event and route it through the same
    /// shared binding/context owner as native flat clients.
    #[wasm_bindgen(js_name = handleRawKey)]
    pub fn handle_raw_key(
        &mut self,
        code: &str,
        legacy_key: &str,
        pressed: bool,
        repeat: bool,
    ) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Keyboard);
        let Some(key) = browser_keyboard_key(code, legacy_key) else {
            return self
                .input_disposition_report(
                    MonoInputDisposition::default(),
                    WebHostEffects::default(),
                )
                .map_err(JsValue::from);
        };

        if pressed && !repeat && !self.host_ref()?.mono_ui_is_active() {
            match key {
                KeyboardKey::KeyN => {
                    let _ = self.host_mut()?.toggle_mono_walk_fly_movement_mode();
                    return self
                        .input_disposition_report(
                            MonoInputDisposition {
                                handled: true,
                                scene_changed: true,
                                ..MonoInputDisposition::default()
                            },
                            WebHostEffects::default(),
                        )
                        .map_err(JsValue::from);
                }
                KeyboardKey::Backquote => {
                    return self
                        .apply_raw_runtime_ui_action(GameUiAction::ToggleDebugDiagnostics)
                        .map_err(JsValue::from);
                }
                _ => {}
            }
        }

        let mut effects = WebHostEffects::default();
        let disposition = self
            .interactive_input
            .route_key(
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                key,
                pressed,
                repeat,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    /// Route a raw browser button phase. `click` is the platform's neutral
    /// click-versus-drag classification; it does not name the bound action.
    #[wasm_bindgen(js_name = handleRawPointerButton)]
    pub fn handle_raw_pointer_button(
        &mut self,
        button: i16,
        pressed: bool,
        click: bool,
        x: f64,
        y: f64,
    ) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Mouse);
        let Some(button) = browser_pointer_button(button) else {
            return self
                .input_disposition_report(
                    MonoInputDisposition::default(),
                    WebHostEffects::default(),
                )
                .map_err(JsValue::from);
        };
        let ui_active = self.host_ref()?.mono_ui_is_active();
        let point = self.ui_point(x, y);
        let mut effects = WebHostEffects::default();
        let disposition = if ui_active {
            self.interactive_input
                .route_pointer_button(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    button,
                    pressed,
                    Some(point),
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(js_error)?
        } else if !pressed && click {
            let mut disposition = self
                .interactive_input
                .route_pointer_button(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    button,
                    true,
                    None,
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(js_error)?;
            let released = self
                .interactive_input
                .route_pointer_button(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    button,
                    false,
                    None,
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(js_error)?;
            disposition.handled |= released.handled;
            disposition.scene_changed |= released.scene_changed;
            disposition.clear_transient_input |= released.clear_transient_input;
            disposition.request_pointer_capture_when_ready |=
                released.request_pointer_capture_when_ready;
            disposition
        } else {
            MonoInputDisposition {
                handled: true,
                ..MonoInputDisposition::default()
            }
        };
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleRawPointerMove)]
    pub fn handle_raw_pointer_move(&mut self, x: f64, y: f64) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Mouse);
        let point = self.ui_point(x, y);
        let mut effects = WebHostEffects::default();
        let disposition = self
            .interactive_input
            .route_pointer_move(
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                point,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleRawMouseMotion)]
    pub fn handle_raw_mouse_motion(
        &mut self,
        delta_x: f32,
        delta_y: f32,
    ) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Mouse);
        let mut effects = WebHostEffects::default();
        let disposition = self
            .interactive_input
            .route_mouse_motion(
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                delta_x,
                delta_y,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleRawWheel)]
    pub fn handle_raw_wheel(&mut self, delta_y: f64, _delta_mode: u32) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Mouse);
        let direction = if delta_y < 0.0 {
            MouseWheelDirection::Up
        } else {
            MouseWheelDirection::Down
        };
        let mut effects = WebHostEffects::default();
        let disposition = self
            .interactive_input
            .route_wheel(
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                direction,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleRawTouch)]
    pub fn handle_raw_touch(
        &mut self,
        phase: &str,
        id: u32,
        x: f64,
        y: f64,
    ) -> Result<JsValue, JsValue> {
        self.input_capability_state
            .note_activity(InputDeviceKind::Touch);
        self.touch_settings_available = true;
        let phase = web_touch_contact_phase(phase)?;
        let id = u64::from(id);
        let scale = GuiScale::from_pixels(self.context.width, self.context.height);
        let point = scale.client_to_gui(x, y);
        let position = Vec2::new(point.x, point.y);
        self.touch_input
            .set_viewport_size(Vec2::new(scale.width, scale.height));

        let ui_active = self.host_ref()?.mono_ui_is_active();
        let route = self.ui_touch.route(id, phase, ui_active);
        let mut effects = WebHostEffects::default();
        let disposition = match route {
            TouchUiContactRoute::PointerDown | TouchUiContactRoute::PointerUp => self
                .interactive_input
                .route_touch_pointer_button(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    route == TouchUiContactRoute::PointerDown,
                    point,
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(js_error)?,
            TouchUiContactRoute::PointerMove => self
                .interactive_input
                .route_pointer_move(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    point,
                    &self.context.device,
                    &self.context.queue,
                    &mut effects,
                )
                .map_err(js_error)?,
            TouchUiContactRoute::Cancel => {
                self.host_mut()?.clear_mono_ui_input();
                MonoInputDisposition {
                    handled: true,
                    scene_changed: true,
                    ..MonoInputDisposition::default()
                }
            }
            TouchUiContactRoute::Ignore => MonoInputDisposition {
                handled: true,
                ..MonoInputDisposition::default()
            },
            TouchUiContactRoute::Gameplay => {
                if !self.touch_controls_visible() {
                    self.touch_input.end_contact(id, true);
                    return self
                        .input_disposition_report(
                            MonoInputDisposition {
                                handled: true,
                                ..MonoInputDisposition::default()
                            },
                            effects,
                        )
                        .map_err(JsValue::from);
                }
                let event = match phase {
                    TouchContactPhase::Started => {
                        let control = touch_control_at(scale, point);
                        self.touch_input.begin_contact(id, control, position)
                    }
                    TouchContactPhase::Moved => {
                        let menu_active = touch_menu_button_rect().contains(point);
                        self.touch_input.move_contact(id, position, menu_active)
                    }
                    TouchContactPhase::Ended | TouchContactPhase::Cancelled => self
                        .touch_input
                        .end_contact(id, phase == TouchContactPhase::Cancelled),
                };
                self.route_touch_input_event(event, &mut effects)?
            }
        };
        self.input_disposition_report(disposition, effects)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = clearRawInput)]
    pub fn clear_raw_input(&mut self) -> Result<JsValue, JsValue> {
        self.clear_interactive_input();
        self.input_disposition_report(
            MonoInputDisposition {
                handled: true,
                clear_transient_input: true,
                ..MonoInputDisposition::default()
            },
            WebHostEffects::default(),
        )
        .map_err(JsValue::from)
    }

    fn pending_chunk_render_compile_job_count(&self) -> usize {
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
        self.diagnostic_report(None, false, 0.0, false)
            .map_err(JsValue::from)
    }

    /// Rich semantic state for the explicit smoke/support observer.
    ///
    /// Ordinary browser cadence and input paths consume only the compact
    /// operational results returned by their own calls.
    #[wasm_bindgen(js_name = diagnosticSnapshot)]
    pub fn diagnostic_snapshot(&self) -> Result<JsValue, JsValue> {
        self.diagnostic_report(None, self.rendered_frame_count > 0, 0.0, false)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = adjustCameraSpeed)]
    pub fn adjust_camera_speed(&mut self, amount: f64) -> Result<JsValue, JsValue> {
        self.host_mut()?.adjust_mono_camera_speed(amount);
        self.diagnostic_report(None, false, 0.0, false)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = previewBlockTarget)]
    pub fn preview_block_target(&self) -> Result<JsValue, JsValue> {
        self.block_target_report().map_err(JsValue::from)
    }

    /// Smoke/receipt camera helper: frame the shared preview placement without
    /// exporting fixture coordinates to the platform adapter.
    #[wasm_bindgen(js_name = frameEmbeddedPreview)]
    pub fn frame_embedded_preview(&mut self) -> Result<JsValue, JsValue> {
        let host = self.host_mut()?;
        let Some(preview) = host.embedded_world_preview_snapshot() else {
            let object = js_sys::Object::new();
            report_set_bool(&object, "ok", true).map_err(JsValue::from)?;
            report_set_bool(&object, "available", false).map_err(JsValue::from)?;
            return Ok(object.into());
        };
        let anchor = preview.placement.composition_anchor();
        let target = Vec3::new(anchor.x as f32, anchor.y as f32 + 0.25, anchor.z as f32);
        set_host_camera_look_at(host, target + Vec3::new(-4.0, 2.25, -6.0), target);
        self.diagnostic_report(None, false, 0.0, false)
            .map_err(JsValue::from)
    }

    /// Aim at a loaded nearby surface for browser interaction receipts without
    /// exporting authored-world coordinates to the platform adapter.
    #[wasm_bindgen(js_name = frameInteractionSurface)]
    pub fn frame_interaction_surface(&mut self) -> Result<JsValue, JsValue> {
        let target = find_interaction_surface(self.host_ref()?);
        if let Some(target) = target {
            aim_player_host_at_block(self.host_mut()?, target);
        }
        self.diagnostic_report(None, false, 0.0, false)
            .map_err(JsValue::from)
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
            MonoWorldActionStatus::Submitted { target } => {
                self.interaction_count = self.interaction_count.saturating_add(1);
                self.command_count = self.command_count.saturating_add(1);
                self.interaction_sent = true;
                write_block_target(&object, &target).map_err(JsValue::from)?;
                report_set_number(
                    &object,
                    "hitBlockStateId",
                    before_state.map_or(-1.0, |state| f64::from(state.0)),
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
                report_set_bool(&object, "commandSent", true).map_err(JsValue::from)?;
                report_set_number(&object, "commandCountDelta", 1.0).map_err(JsValue::from)?;
            }
            MonoWorldActionStatus::DeniedByWorldBehavior => {
                report_set_bool(&object, "hit", before_target.is_some()).map_err(JsValue::from)?;
                report_set_bool(&object, "deniedByWorldBehavior", true).map_err(JsValue::from)?;
                report_set_bool(&object, "commandSent", false).map_err(JsValue::from)?;
            }
            MonoWorldActionStatus::EmbeddedWorldActivationRequested => {
                report_set_bool(&object, "hit", true).map_err(JsValue::from)?;
                report_set_bool(&object, "embeddedActivationRequested", true)
                    .map_err(JsValue::from)?;
                report_set_bool(&object, "deniedByWorldBehavior", false).map_err(JsValue::from)?;
                report_set_bool(&object, "commandSent", false).map_err(JsValue::from)?;
            }
            _ => {
                report_set_bool(&object, "hit", false).map_err(JsValue::from)?;
                report_set_bool(&object, "deniedByWorldBehavior", false).map_err(JsValue::from)?;
                report_set_bool(&object, "commandSent", false).map_err(JsValue::from)?;
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
            self.clear_interactive_input();
            if let Some(host) = self.host.as_mut() {
                let _ = host.on_background().map_err(js_error)?;
                self.background_save_count = self.background_save_count.saturating_add(1);
            }
        } else if !hidden && self.frame_policy.set_hidden(false) {
            self.last_visible_frame_millis = None;
            self.resume_frames_remaining = RESUME_OBSERVATION_FRAMES;
        }
        self.operational_report(false, 0.0, false)
            .map_err(JsValue::from)
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
        self.operational_report(false, 0.0, false)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setDebugOverlayVisible)]
    pub fn set_debug_overlay_visible(&mut self, visible: bool) -> Result<JsValue, JsValue> {
        self.host_mut()?.set_mono_debug_diagnostics_visible(visible);
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = reportHostFailure)]
    pub fn report_host_failure(&mut self, message: &str) -> Result<JsValue, JsValue> {
        self.startup_status_visible = false;
        self.status_overlay = StatusOverlay::new(format!("Browser host failure: {message}"), false);
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchLookSensitivity)]
    pub fn set_touch_look_sensitivity(
        &mut self,
        sensitivity: f32,
        available: bool,
    ) -> Result<JsValue, JsValue> {
        self.touch_look_sensitivity = TouchInputSettings {
            look_sensitivity: sensitivity,
            ..TouchInputSettings::default()
        }
        .normalized()
        .look_sensitivity;
        self.touch_input
            .set_look_sensitivity(self.touch_look_sensitivity);
        self.touch_settings_available = available;
        self.input_capability_state
            .set_present(InputDeviceKind::Touch, available);
        self.refresh_touch_overlay_from_input()?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchControlsMode)]
    pub fn set_touch_controls_mode(&mut self, mode: &str) -> Result<JsValue, JsValue> {
        self.touch_controls_mode = parse_touch_controls_mode(mode)
            .ok_or_else(|| JsValue::from_str(&format!("invalid touch mode {mode:?}")))?;
        self.refresh_touch_overlay_from_input()?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchInputAvailable)]
    pub fn set_touch_input_available(&mut self, available: bool) -> Result<JsValue, JsValue> {
        self.touch_settings_available = available;
        self.input_capability_state
            .set_present(InputDeviceKind::Touch, available);
        self.refresh_touch_overlay_from_input()?;
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
        self.render_frame(browser_now_millis())?;
        self.diagnostic_snapshot()
    }

    /// Take the next Rust-admitted coarse operation through one opaque
    /// browser boundary. URL arguments are mechanical Worker resources; all
    /// target, identity, ordering, and acceptance state remains in Rust.
    #[wasm_bindgen(js_name = takeSceneOperation)]
    pub fn take_scene_operation(
        &mut self,
        worker_url: String,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<Option<WebSceneOperation>, JsValue> {
        let resources = (worker_url, job_worker_url, bindgen_js_url, bindgen_wasm_url);
        if let Some(pending) = self.host_mut()?.take_external_session_start() {
            let startup_storage = self.startup_runtime_storage.take();
            return Ok(Some(WebSceneOperationDrain::take_scene_operation(
                lower_runtime_start(pending, resources.clone(), startup_storage.as_ref()),
            )));
        }
        if self
            .host_ref()?
            .pending_external_asset_pack_preparation_count()
            != 0
            && let Some(effect) = take_asset_pack_preparation(self)?
        {
            return Ok(Some(WebSceneOperationDrain::take_scene_operation(effect)));
        }
        if let Some(pending) = self.host_mut()?.take_external_runtime_start() {
            return Ok(Some(WebSceneOperationDrain::take_scene_operation(
                lower_runtime_start(pending, resources, None),
            )));
        }
        if self.host_ref()?.pending_external_catalog_operation_count() != 0
            && let Some(pending) = self.platform.take_catalog_operation()
        {
            let token = pending.token;
            let execution = WebCatalogExecution::new(
                token,
                pending.kind.request.request,
                pending.kind.active_world,
            )
            .map_err(JsValue::from)?;
            return Ok(Some(WebSceneOperationDrain::take_scene_operation(
                WebSceneOperationEffect::IndexedDb(execution),
            )));
        }
        Ok(None)
    }

    /// Resolve one opaque operation through its specialized Rust owner.
    #[wasm_bindgen(js_name = completeSceneOperation)]
    pub fn complete_scene_operation(
        &mut self,
        operation: &mut WebSceneOperation,
    ) -> Result<JsValue, JsValue> {
        let completion = WebSceneOperationDrain::complete_scene_operation(operation)?;
        let (report, receipt, source) = match completion {
            WebSceneOperationCompletion::Runtime { pending, outcome } => {
                let receipt = match &pending.target {
                    mclone_scene::ExternalSceneStartTarget::Lobby { .. } => "lobby-runtime",
                    mclone_scene::ExternalSceneStartTarget::ActiveSession { .. } => {
                        "active-runtime"
                    }
                };
                let source = pending
                    .storage_source
                    .as_ref()
                    .map(|source| (source.world_id().to_owned(), source.kind_label().to_owned()));
                let report = match outcome {
                    Ok(runtime) => self.complete_started_runtime(pending, runtime)?,
                    Err(error) => {
                        self.host_mut()?.fail_external_session_start(pending, error);
                        self.ui_report(false, None).map_err(JsValue::from)?
                    }
                };
                (report, receipt, source)
            }
            WebSceneOperationCompletion::Assets {
                token,
                content_generation,
                selected_file_count,
                outcome,
            } => (
                apply_asset_pack_preparation(
                    self,
                    token,
                    content_generation,
                    selected_file_count,
                    outcome,
                )?,
                "assets",
                None,
            ),
            WebSceneOperationCompletion::IndexedDb { token, outcome } => {
                self.platform
                    .complete_catalog_operation(PlatformOperationCompletion {
                        token,
                        result: outcome.map_err(|message| {
                            WorldCatalogError::new(WorldCatalogErrorKind::StorageFailure, message)
                        }),
                    });
                let (device, queue) = (&self.context.device, &self.context.queue);
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
                    .poll_external_catalog_operations(device, queue)
                    .map_err(js_error)?;
                (
                    self.ui_report(false, None).map_err(JsValue::from)?,
                    "catalog",
                    None,
                )
            }
        };
        let object: js_sys::Object = report.unchecked_into();
        report_set_string(&object, "sceneOperationReceipt", receipt).map_err(JsValue::from)?;
        if let Some((world_id, source_kind)) = source {
            report_set_string(&object, "worldId", &world_id).map_err(JsValue::from)?;
            report_set_string(&object, "storageSourceKind", &source_kind).map_err(JsValue::from)?;
        }
        Ok(object.into())
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
            host.enter_mono_title(&self.context.device, &self.context.queue)
                .map_err(js_error)?;
            self.shutdown_complete = true;
        }
        self.render_worker.terminate();
        self.frame_policy.shutdown();
        self.diagnostic_report(None, false, 0.0, false)
            .map_err(JsValue::from)
    }
}

fn lower_runtime_start(
    pending: ExternalSceneSessionStart,
    resources: WebRuntimeResources,
    startup_storage: Option<&WebWorldStorageStartupOptions>,
) -> WebSceneOperationEffect {
    let (worker_url, job_worker_url, bindgen_js_url, bindgen_wasm_url) = resources;
    let center = pending.scene.center();
    let render_distance = pending.scene.render_distance;
    let effect = match pending.descriptor.clone() {
        ActiveSessionDescriptor::Remote { endpoint } => {
            WebRuntimeStartEffect::Remote(endpoint.address)
        }
        ActiveSessionDescriptor::LocalWorld { seed, id, .. } => {
            let observer_only = matches!(
                &pending.target,
                mclone_scene::ExternalSceneStartTarget::Lobby {
                    role: mclone_app_runtime::scenario_content::LobbyWorldRole::Destination,
                    ..
                }
            );
            let mut config = WebIntegratedServerRunnerConfig::new(
                seed,
                worker_url,
                job_worker_url,
                bindgen_js_url,
                bindgen_wasm_url,
            )
            .with_world_generation_profile(pending.scene.world_generation_profile)
            .with_world_topology(pending.scene.world_topology)
            .with_world_behavior_profile(pending.scene.world_behavior_profile)
            .with_freeze_scheduled_fluid_ticks(pending.scene.freeze_scheduled_fluid_ticks)
            .with_debug_passive_showcase(pending.scene.debug_passive_showcase)
            .with_debug_auxiliary_player_script(pending.scene.debug_auxiliary_player_script)
            .with_observer_only(observer_only);
            config = match pending.storage_source.as_ref() {
                Some(
                    mclone_app_runtime::scenario_content::LobbyWorldSource::TransientAuthored(
                        fixture,
                    ),
                ) => config.with_transient_authored_fixture(*fixture),
                Some(source) => config.with_indexed_db_world(source.world_id(), false),
                None if startup_storage
                    .is_some_and(|storage| storage.world_storage == "indexeddb") =>
                {
                    let storage = startup_storage.expect("startup storage presence checked");
                    config.with_indexed_db_world(
                        storage.world_id.clone(),
                        storage.clear_world_storage,
                    )
                }
                None if id.is_some() => {
                    config.with_indexed_db_world(id.as_ref().unwrap().as_str(), false)
                }
                None => config,
            };
            WebRuntimeStartEffect::Integrated(config)
        }
    };
    WebSceneOperationEffect::Runtime {
        pending,
        start: effect,
        center,
        render_distance,
    }
}

fn take_asset_pack_preparation(
    host: &mut WebSceneHost,
) -> Result<Option<WebSceneOperationEffect>, JsValue> {
    let Some(pending) = host.host_mut()?.take_external_asset_pack_selection() else {
        return Ok(None);
    };
    let enabled = |id| {
        pending
            .kind
            .selection
            .is_enabled(&mclone_assets::AssetPackId::new(id))
    };
    let authored = host.initial_asset_packs.authored.clone();
    let reference = host.initial_asset_packs.reference.clone();
    let fallback = host.initial_asset_packs.fallback.clone();
    let diagnostic = host.initial_asset_packs.diagnostic.clone();
    let selected_file_count = [
        enabled(AUTHORED_FIRST_PARTY_PACK_ID).then_some(authored.as_slice()),
        enabled(MINECRAFT_REFERENCE_PACK_ID).then_some(reference.as_slice()),
        enabled(mclone_assets::PROVISIONAL_FIRST_PARTY_PACK_ID).then_some(fallback.as_slice()),
        enabled(mclone_assets::DIAGNOSTIC_MISSING_PACK_ID).then_some(diagnostic.as_slice()),
    ]
    .into_iter()
    .flatten()
    .map(|bytes| PackedAssetSource::from_bytes(bytes.to_vec()).map(|pack| pack.file_count()))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| JsValue::from_str(&format!("failed to inspect selected packs: {error}")))?
    .into_iter()
    .sum();
    Ok(Some(WebSceneOperationEffect::Assets {
        token: pending.token,
        content_generation: pending.kind.content_generation,
        selected_file_count,
        effect: WebAssetPackPreparationEffect {
            content_generation: pending.kind.content_generation,
            authored,
            reference,
            fallback,
            diagnostic,
            selection: pending.kind.selection,
            presentation: pending.kind.presentation,
            render_worker: host.render_worker.clone(),
        },
    }))
}

fn apply_asset_pack_preparation(
    host: &mut WebSceneHost,
    token: PlatformOperationToken,
    content_generation: u64,
    selected_file_count: usize,
    outcome: Result<PreparedSceneAssets, String>,
) -> Result<JsValue, JsValue> {
    let succeeded = outcome.is_ok();
    let applied = host
        .host_mut()?
        .complete_external_asset_pack_preparation(token, outcome)
        .map_err(js_error)?;
    if !applied {
        host.render_worker
            .settle_asset_epoch(content_generation, false);
    }
    if !applied || !succeeded {
        return host.ui_report(false, None).map_err(JsValue::from);
    }
    host.pending_asset_pack_file_count = Some((content_generation, selected_file_count));
    host.ui_report(false, None).map_err(JsValue::from)
}

#[wasm_bindgen]
pub async fn mclone_web_create_scene_host_with_startup(
    canvas: HtmlCanvasElement,
    resources: WebBootstrapResources,
    capabilities: WebHostCapabilities,
    startup: WebStartupConfig,
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    render_worker_transport_factory: js_sys::Function,
) -> Result<WebSceneHost, JsValue> {
    let initial_asset_packs = resources.into_initial_asset_packs()?;
    let (options, storage, entry) = startup.into_parts();
    let scene_startup = options.scene;
    let render_options = options.render_options;
    let _ = (server_worker_url, server_job_worker_url);
    let scene = McloneSceneHostOptions {
        startup: scene_startup,
        use_initial_spawn_center: false,
        startup_lod_prewarm: false,
        ..McloneSceneHostOptions::default()
    };
    create_scene_host(
        canvas,
        initial_asset_packs,
        capabilities,
        scene,
        render_options,
        entry,
        storage,
        bindgen_js_url,
        bindgen_wasm_url,
        render_worker_transport_factory,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn create_scene_host(
    canvas: HtmlCanvasElement,
    initial_asset_packs: InitialAssetPacks,
    capabilities: WebHostCapabilities,
    scene: McloneSceneHostOptions,
    render_options: TexturedSectionRenderOptions,
    entry: ClientEntryResolution,
    startup_storage: WebWorldStorageStartupOptions,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    render_worker_transport_factory: js_sys::Function,
) -> Result<WebSceneHost, JsValue> {
    let render_color_profile = render_options.color_profile.as_str().to_owned();
    let initial_center = scene.center();
    let input_preference_store = WebPreferenceKeyValueStore;
    let (input_preferences, input_preference_error) =
        match ClientInputPreferences::load(&input_preference_store) {
            Ok(preferences) => (preferences, None),
            Err(error) => {
                let message = format!("load {}: {error:#}", input_preference_store.label());
                web_sys::console::warn_1(&JsValue::from_str(&message));
                (ClientInputPreferences::default(), Some(message))
            }
        };
    let asset_pack_file_count =
        PackedAssetSource::from_bytes(initial_asset_packs.reference.clone())
            .map_err(|error| JsValue::from_str(&format!("invalid browser asset pack: {error}")))?
            .file_count();
    let render_worker = WebRenderWorkerCoordinator::new(
        render_worker_transport_factory,
        bindgen_js_url,
        bindgen_wasm_url,
        initial_asset_packs.reference.clone(),
    )
    .map_err(|error| JsValue::from_str(&error))?;
    let catalog = web_asset_pack_catalog(
        initial_asset_packs.authored.clone(),
        initial_asset_packs.reference.clone(),
        initial_asset_packs.fallback.clone(),
        initial_asset_packs.diagnostic.clone(),
    )
    .map_err(JsValue::from)?;
    let active_assets = prepare_web_scene_assets_from_pack(initial_asset_packs.reference.clone())
        .map_err(JsValue::from)?;
    let context = WebCanvasContext::new_with_color_profile(canvas, render_options.color_profile)
        .await
        .map_err(JsValue::from)?;
    let (platform, clock, catalog_operations) = WebScenePlatformServices::new();
    let mut host = McloneSceneHost::without_session(
        &context.device,
        &context.queue,
        context.format,
        clock,
        scene,
        render_options,
        active_assets,
        web_client_experience_profile(),
        Some(catalog_operations),
        None,
    )
    .map_err(js_error)?;
    host.set_mono_ui_context(MonoUiContext::default());
    host.set_mono_debug_diagnostics_visible(capabilities.initial_debug_overlay_visible());
    host.configure_external_asset_pack_catalog(
        catalog,
        mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
    )
    .map_err(js_error)?;
    host.configure_asset_pack_preference_storage(Box::new(WebAssetPackPreferenceStorage))
        .map_err(js_error)?;
    host.configure_graphics_preference_storage(Box::new(WebGraphicsPreferenceStorage))
        .map_err(js_error)?;
    // Reposition the browser camera without confusing the walking multiplier
    // with the camera's absolute fly speed. The shared host has already
    // applied both launch settings to their distinct camera fields.
    let initial_fly_speed = host.mono_camera_speed_blocks_per_second();
    host.set_mono_player_camera(
        Vec3d::new(
            f64::from(initial_center.x) * 16.0 + 8.0,
            112.0,
            f64::from(initial_center.z) * 16.0 + 8.0,
        ),
        0.0,
        -0.35,
        initial_fly_speed,
    );
    let startup_requests_session = matches!(
        &entry.intent,
        mclone_app_runtime::client_entry::ClientEntryIntent::StartSession(_)
    );
    let mut entry_controller = ClientEntryController::new(entry);
    let entry_effect = entry_controller
        .update_host(ClientHostAvailability {
            bootstrapped: true,
            foreground: true,
            presentation_available: true,
        })
        .expect("ready browser host dispatches its entry exactly once");
    web_sys::console::info_1(&JsValue::from_str(&format!(
        "Mclone browser client entry source={} intent={}",
        entry_controller.resolution().source.label(),
        entry_controller.resolution().intent.label(),
    )));
    let startup_status = match entry_effect {
        ClientEntryEffect::EnterTitle { status } => {
            host.set_client_entry_status(status);
            StatusOverlay::hidden()
        }
        ClientEntryEffect::StartSession(request) => {
            host.start_session_for_request(&context.device, &context.queue, request)
                .map_err(js_error)?;
            StatusOverlay::new("Generating world...", true)
        }
        ClientEntryEffect::LaunchScenario(intent) => {
            host.begin_lobby_launch(intent).map_err(js_error)?;
            StatusOverlay::new("Preparing lobby...", true)
        }
    };
    let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
    let mut input_capability_state = InputCapabilityState::new(InputCapabilities {
        keyboard: true,
        mouse: true,
        touch: capabilities.touch_input_available(),
        gamepad: false,
        xr_controller: false,
    });
    input_capability_state.note_activity(if capabilities.touch_input_available() {
        InputDeviceKind::Touch
    } else {
        InputDeviceKind::Keyboard
    });
    let mut web_host = WebSceneHost {
        context,
        depth,
        split_presentation: None,
        host: Some(host),
        platform,
        frame_policy: WebSceneFrameDriverPolicy::default(),
        frame_count: 0,
        rendered_frame_count: 0,
        initial_presentation_stable_frames: 0,
        startup_status_visible: true,
        last_visible_frame_millis: None,
        max_frame_gap_millis: 0.0,
        movement_input_applied: false,
        dom_input_frame_count: 0,
        interaction_sent: false,
        settings_effect_applied: false,
        pause_ui_rendered: false,
        resized: false,
        background_save_count: 0,
        resume_frames_remaining: 0,
        max_resume_poll_updates: 0,
        max_resume_drop_backlog: 0,
        shutdown_complete: false,
        render_worker,
        startup_runtime_storage: startup_requests_session.then_some(startup_storage),
        initial_asset_packs,
        asset_pack_file_count,
        pending_asset_pack_file_count: None,
        status_overlay: startup_status,
        touch_look_sensitivity: input_preferences.touch_look_sensitivity,
        touch_settings_available: capabilities.touch_input_available(),
        touch_controls_mode: input_preferences.touch_controls_mode,
        input_preferences: input_preferences.clone(),
        input_preference_error,
        input_capability_state,
        touch_overlay: TouchOverlay::hidden(),
        interactive_input: MonoInteractiveInputRouter::with_controller_preferences(
            &input_preferences.controller,
        ),
        gamepad_collector: BrowserGamepadCollector::new(),
        frame_input_effects: WebFrameInputEffects::default(),
        touch_input: TouchInputAdapter::with_settings(TouchInputSettings {
            look_sensitivity: input_preferences.touch_look_sensitivity,
            ..TouchInputSettings::default()
        }),
        ui_touch: TouchUiContactTracker::default(),
        last_action: None,
        last_frame: LastFrameStats::default(),
        command_count: 0,
        update_count: 0,
        interaction_count: 0,
        mesh_build_count: 0,
        render_color_profile,
        last_runner_kind: "none".to_owned(),
    };
    web_host.refresh_touch_overlay_from_input()?;
    Ok(web_host)
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

    fn begin_lobby_smoke_with_chunk_span(&mut self, chunk_span: u32) -> Result<JsValue, JsValue> {
        let bounds = mclone_app_runtime::scenario::ScenarioPreviewBounds::square(chunk_span)
            .map_err(js_error)?;
        self.host_mut()?
            .begin_lobby_launch(
                mclone_app_runtime::scenario::ScenarioLaunchIntent::lobby_preview()
                    .with_preview_bounds(bounds)
                    .map_err(js_error)?,
            )
            .map_err(js_error)?;
        self.ui_report(false, None).map_err(JsValue::from)
    }

    fn clear_interactive_input(&mut self) {
        self.interactive_input.clear_transient_input();
        self.touch_input.clear();
        self.ui_touch.clear();
        if let Some(host) = self.host.as_mut() {
            host.clear_mono_camera_input();
            host.clear_mono_ui_input();
        }
    }

    fn poll_controller_input(&mut self, now_millis: f64) -> Result<bool, JsValue> {
        if self.host.is_none() {
            return Ok(false);
        }
        let poll = match self.gamepad_collector.poll_browser(now_millis) {
            Ok(poll) => poll,
            Err(error) => {
                web_sys::console::warn_2(
                    &JsValue::from_str("browser Gamepad API poll failed"),
                    &error,
                );
                return Ok(false);
            }
        };
        let topology_changed = !poll.connected.is_empty() || !poll.disconnected.is_empty();
        self.input_capability_state
            .set_present(InputDeviceKind::Gamepad, poll.connected_count() > 0);
        for (source_id, descriptor) in poll.connected {
            self.interactive_input
                .connect_controller_source(source_id, descriptor);
        }
        for source_id in poll.disconnected {
            self.interactive_input
                .disconnect_controller_source(source_id)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }
        let mut effects = WebHostEffects::default();
        let disposition = self
            .interactive_input
            .route_controller_batch(
                self.host
                    .as_mut()
                    .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                &poll.input,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(js_error)?;
        if disposition.meaningful_controller_activity {
            self.input_capability_state
                .note_activity(InputDeviceKind::Gamepad);
        }
        let should_redraw = topology_changed
            || disposition.handled
            || disposition.scene_changed
            || disposition.meaningful_controller_activity
            || effects.mouse_lock_requested.is_some()
            || effects.touch_controls_mode.is_some()
            || effects.quit_to_title
            || effects.exit;
        self.frame_input_effects.clear_transient_input |= disposition.clear_transient_input;
        if effects.mouse_lock_requested.is_some() {
            self.frame_input_effects.pointer_capture_desired = effects.mouse_lock_requested;
        }
        self.frame_input_effects.exit_requested |= effects.exit;
        self.apply_input_effect_state(disposition, effects)
            .map_err(JsValue::from)?;
        Ok(should_redraw)
    }

    fn route_touch_input_event(
        &mut self,
        event: TouchInputEvent,
        effects: &mut WebHostEffects,
    ) -> Result<MonoInputDisposition, JsValue> {
        self.interactive_input.observe_supplemental_movement(
            self.host
                .as_mut()
                .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
            self.touch_input.held_frame(),
        );
        let mut disposition = MonoInputDisposition {
            handled: event.handled,
            ..MonoInputDisposition::default()
        };
        if let Some(delta) = event.look_delta {
            merge_input_disposition(
                &mut disposition,
                self.interactive_input.route_touch_look(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    delta,
                ),
            );
        }
        if let Some(frame) = event.frame {
            let routed = self
                .interactive_input
                .route_flat_frame(
                    self.host
                        .as_mut()
                        .ok_or_else(|| JsValue::from_str("scene host is shut down"))?,
                    frame,
                    &self.context.device,
                    &self.context.queue,
                    effects,
                )
                .map_err(js_error)?;
            merge_input_disposition(&mut disposition, routed);
        }
        Ok(disposition)
    }

    fn apply_raw_runtime_ui_action(&mut self, action: GameUiAction) -> Result<JsValue, String> {
        let mut effects = WebHostEffects::default();
        let outcome = self
            .host
            .as_mut()
            .ok_or_else(|| "scene host is shut down".to_owned())?
            .apply_mono_ui_action(
                action,
                false,
                &self.context.device,
                &self.context.queue,
                &mut effects,
            )
            .map_err(|error| format!("apply shared web runtime action: {error:#}"))?;
        self.last_action = Some(action);
        self.input_disposition_report(
            MonoInputDisposition {
                handled: true,
                scene_changed: true,
                clear_transient_input: outcome.clear_gameplay_input,
                request_pointer_capture_when_ready: outcome.session_start_requested,
                meaningful_controller_activity: false,
            },
            effects,
        )
    }

    fn input_disposition_report(
        &mut self,
        disposition: MonoInputDisposition,
        effects: WebHostEffects,
    ) -> Result<JsValue, String> {
        self.apply_input_effect_state(disposition, effects)?;

        let value = self.ui_report(disposition.handled, None)?;
        let object: js_sys::Object = value.unchecked_into();
        report_set_bool(&object, "sceneChanged", disposition.scene_changed)?;
        report_set_bool(
            &object,
            "clearTransientInput",
            disposition.clear_transient_input,
        )?;
        report_set_bool(
            &object,
            "requestPointerCaptureWhenReady",
            disposition.request_pointer_capture_when_ready,
        )?;
        report_set_bool(
            &object,
            "releasePointerCapture",
            self.host_ref()
                .map_err(|error| format!("{error:?}"))?
                .mono_ui_is_active(),
        )?;
        if let Some(requested) = effects.mouse_lock_requested {
            report_set_bool(&object, "pointerCaptureDesired", requested)?;
        }
        report_set_bool(&object, "exitRequested", effects.exit)?;
        Ok(object.into())
    }

    fn apply_input_effect_state(
        &mut self,
        disposition: MonoInputDisposition,
        effects: WebHostEffects,
    ) -> Result<(), String> {
        if let Some(mode) = effects.touch_controls_mode {
            self.touch_controls_mode = mode;
        }
        if let Some(settings) = self
            .host_ref()
            .map_err(|error| format!("{error:?}"))?
            .mono_ui_render_state()
            .touch_settings
        {
            self.touch_look_sensitivity = settings.clamped_look_sensitivity();
            self.touch_input
                .set_look_sensitivity(self.touch_look_sensitivity);
        }
        if disposition.clear_transient_input || effects.quit_to_title {
            self.clear_interactive_input();
        }
        self.refresh_touch_overlay_from_input()
            .map_err(|error| format!("refresh shared touch overlay: {error:?}"))?;
        self.persist_input_preferences_if_changed();
        Ok(())
    }

    fn refresh_touch_overlay_from_input(&mut self) -> Result<(), JsValue> {
        let visible = self.touch_controls_visible();
        if !visible {
            self.touch_input.clear();
        }
        let state = self.touch_input.overlay_state();
        self.touch_overlay = TouchOverlay {
            visible,
            menu_pressed: state.menu_pressed,
            movement: state
                .movement
                .map(|movement| TouchJoystickOverlay {
                    active: true,
                    base: point_from_vec2(movement.base),
                    thumb: point_from_vec2(movement.thumb),
                })
                .unwrap_or_default(),
            jump_pressed: state.jump_pressed,
            sprint_pressed: state.sprint_pressed,
            sneak_pressed: state.sneak_pressed,
            descend_pressed: state.descend_pressed,
            interaction_visible: true,
            attack_pressed: state.attack_pressed,
            use_pressed: state.use_pressed,
            hotbar_visible: true,
            selected_hotbar_slot: self.host_ref()?.selected_mono_hotbar_slot(),
            hotbar_pressed_slot: state.hotbar_pressed_slot,
            hotbar_icons: mclone_ui::EMPTY_HOTBAR_ICONS,
        };
        self.refresh_mono_ui_context()
    }

    fn touch_controls_visible(&self) -> bool {
        let mut preferences = InputPreferences::AUTO;
        preferences.preferred_scheme = self.input_preferences.controller.preferred_input;
        preferences.touch_controls = self.touch_controls_mode;
        self.touch_settings_available
            && self
                .input_capability_state
                .resolve(preferences)
                .touch_controls_visible
    }

    fn complete_started_runtime(
        &mut self,
        pending: ExternalSceneSessionStart,
        runtime: crate::WebRuntime,
    ) -> Result<JsValue, JsValue> {
        let descriptor = pending.descriptor.clone();
        let active_assets = self
            .host_ref()?
            .active_asset_snapshot_for_epoch(self.host_ref()?.active_asset_epoch());
        let runtime = WebSceneRuntimeService::new(
            runtime,
            active_assets.mesh.clone(),
            self.render_worker.clone(),
            self.platform.clock_handle(),
            pending.instance_id.get(),
            match &pending.target {
                mclone_scene::ExternalSceneStartTarget::ActiveSession { .. }
                | mclone_scene::ExternalSceneStartTarget::Lobby {
                    role: mclone_app_runtime::scenario_content::LobbyWorldRole::Primary,
                    ..
                } => RuntimeRenderPriority::Active,
                mclone_scene::ExternalSceneStartTarget::Lobby {
                    role: mclone_app_runtime::scenario_content::LobbyWorldRole::Destination,
                    ..
                } => RuntimeRenderPriority::Standby,
            },
        )
        .into_scene_session_runtime(descriptor.clone());
        let resets_active_presentation = matches!(
            pending.target,
            mclone_scene::ExternalSceneStartTarget::ActiveSession { .. }
                | mclone_scene::ExternalSceneStartTarget::Lobby {
                    role: mclone_app_runtime::scenario_content::LobbyWorldRole::Primary,
                    ..
                }
        );
        let (device, queue) = (&self.context.device, &self.context.queue);
        self.host
            .as_mut()
            .ok_or_else(|| JsValue::from_str("scene host is shut down"))?
            .complete_external_session_start(device, queue, pending, runtime)
            .map_err(js_error)?;
        self.last_frame = LastFrameStats::default();
        if resets_active_presentation {
            self.initial_presentation_stable_frames = 0;
        }
        self.ui_report(false, None).map_err(JsValue::from)
    }

    fn ui_point(&self, x: f64, y: f64) -> Point {
        GuiScale::from_pixels(self.context.width, self.context.height).client_to_gui(x, y)
    }

    fn refresh_mono_ui_context(&mut self) -> Result<(), JsValue> {
        let mut context = MonoUiContext::default();
        let mut preferences = InputPreferences::AUTO;
        preferences.preferred_scheme = self.input_preferences.controller.preferred_input;
        preferences.touch_controls = self.touch_controls_mode;
        context.resolved_input = self.input_capability_state.resolve(preferences);
        context.resolved_input.touch_controls_visible = self.touch_overlay.visible;
        context.controller_layout = self
            .interactive_input
            .latest_controller_actions()
            .active_controller_layout
            .unwrap_or(ControllerLayoutFamily::Unknown);
        context.touch_overlay = self.touch_overlay.clone();
        context.touch_controls_mode = Some(self.touch_controls_mode);
        context.touch_settings = self
            .touch_settings_available
            .then_some(GameTouchSettings::new(
                self.touch_look_sensitivity,
                TouchInputSettings::MIN_LOOK_SENSITIVITY,
                TouchInputSettings::MAX_LOOK_SENSITIVITY,
            ));
        self.host_mut()?.set_mono_ui_context(context);
        Ok(())
    }

    fn persist_input_preferences_if_changed(&mut self) {
        let mut current = self.input_preferences.clone();
        current.touch_look_sensitivity = self.touch_look_sensitivity;
        current.touch_controls_mode = self.touch_controls_mode;
        let current = current.normalized();
        if current == self.input_preferences {
            return;
        }
        let store = WebPreferenceKeyValueStore;
        match current.store(&store) {
            Ok(()) => self.input_preference_error = None,
            Err(error) => {
                let message = format!("store {}: {error:#}", store.label());
                web_sys::console::warn_1(&JsValue::from_str(&message));
                self.input_preference_error = Some(message);
            }
        }
        self.input_preferences = current;
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
            if matches!(action, GameUiAction::SetTouchLookSensitivity(_)) {
                if let Some(settings) = self
                    .host
                    .as_ref()
                    .and_then(|host| host.mono_ui_render_state().touch_settings)
                {
                    self.touch_look_sensitivity = settings.clamped_look_sensitivity();
                    self.touch_input
                        .set_look_sensitivity(self.touch_look_sensitivity);
                }
                self.refresh_mono_ui_context()
                    .map_err(|error| format!("refresh web UI context: {error:?}"))?;
            }
            self.persist_input_preferences_if_changed();
            self.last_action = Some(action);
        }
        self.ui_report(handled, action)
    }

    fn ui_report(
        &mut self,
        handled: bool,
        _action: Option<GameUiAction>,
    ) -> Result<JsValue, String> {
        let value = self.operational_report(false, 0.0, false)?;
        let object: js_sys::Object = value.unchecked_into();
        report_set_bool(&object, "handled", handled)?;
        Ok(object.into())
    }

    fn write_last_action_receipt(&self, object: &js_sys::Object) -> Result<(), String> {
        let Some(action) = self.last_action else {
            return Ok(());
        };
        report_set_string(object, "action", ui_action_label(action))?;
        match action {
            GameUiAction::CreateWorld(seed) => {
                report_set_number(object, "worldSeed", seed as f64)?;
                report_set_string(object, "worldSeedText", &seed.to_string())
            }
            GameUiAction::JoinRemote => {
                if let Some(ActiveSessionDescriptor::Remote { endpoint }) = self
                    .host
                    .as_ref()
                    .and_then(|host| match host.session_state() {
                        GameSessionState::Active { session } => Some(session.clone()),
                        _ => None,
                    })
                {
                    report_set_string(object, "remoteEndpoint", &endpoint.address)?;
                }
                Ok(())
            }
            GameUiAction::SetRenderDistance(distance) => {
                report_set_number(object, "renderDistance", f64::from(distance))
            }
            GameUiAction::SetTouchLookSensitivity(value) => {
                report_set_number(object, "touchLookSensitivity", f64::from(value))
            }
            GameUiAction::SetTouchControlsMode(mode) => {
                report_set_string(object, "touchControlsMode", touch_controls_mode_label(mode))
            }
            GameUiAction::SelectWorld(id) | GameUiAction::OpenWorld(id) => {
                report_set_number(object, "catalogWorldUiId", id.0 as f64)?;
                if let Some(world_id) = self
                    .host
                    .as_ref()
                    .and_then(|host| host.mono_local_world_id_for_ui_id(id))
                {
                    report_set_string(object, "catalogWorldId", world_id.as_str())?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
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

    fn streaming_idle(&self) -> bool {
        let Some(host) = self.host.as_ref() else {
            return false;
        };
        let Some(stats) = host.runtime_stats() else {
            return false;
        };
        let camera = host.camera_frame_state().camera;
        let camera_position = Vec3::new(
            camera.eye.x as f32,
            camera.eye.y as f32,
            camera.eye.z as f32,
        );
        stats.server_command_queue_depth == 0
            && stats.server_update_queue_depth == 0
            && stats.pending_jobs == 0
            && stats.pending_publications == 0
            && stats.pending_persistence_loads == 0
            && stats.pending_persistence_saves == 0
            && stats.pending_render_compile_jobs == 0
            && host.pending_stream_work(camera_position) == 0
            && self.render_worker.pending_request_count() == 0
    }

    fn observe_initial_presentation_frame(&mut self) {
        if self.initial_presentation_stable_frames >= INITIAL_PRESENTATION_STABLE_FRAMES {
            return;
        }
        if self.streaming_idle() && self.last_frame.section_count > 0 {
            self.initial_presentation_stable_frames = self
                .initial_presentation_stable_frames
                .saturating_add(1)
                .min(INITIAL_PRESENTATION_STABLE_FRAMES);
        } else {
            self.initial_presentation_stable_frames = 0;
        }
    }

    fn operational_report(
        &self,
        rendered: bool,
        delta_seconds: f64,
        first_after_resume: bool,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        report_set_bool(&object, "ok", true)?;
        report_set_string(&object, "state", self.frame_policy.state().label())?;
        if let WebSceneFrameState::RestartRequired { reason } = self.frame_policy.state() {
            report_set_string(&object, "restartReason", reason)?;
        }
        report_set_bool(&object, "rendered", rendered)?;
        report_set_number(&object, "deltaSeconds", delta_seconds)?;
        report_set_bool(&object, "firstAfterResume", first_after_resume)?;
        report_set_number(&object, "width", self.context.width as f64)?;
        report_set_number(&object, "height", self.context.height as f64)?;
        report_set_bool(
            &object,
            "initialPresentationReady",
            self.initial_presentation_stable_frames >= INITIAL_PRESENTATION_STABLE_FRAMES,
        )?;
        report_set_number(
            &object,
            "renderWorkerPendingRequestCount",
            self.render_worker.pending_request_count() as f64,
        )?;
        report_set_bool(&object, "shutdownComplete", self.shutdown_complete)?;
        report_set_bool(
            &object,
            "clearTransientInput",
            self.frame_input_effects.clear_transient_input,
        )?;
        if let Some(requested) = self.frame_input_effects.pointer_capture_desired {
            report_set_bool(&object, "pointerCaptureDesired", requested)?;
        }
        report_set_bool(
            &object,
            "exitRequested",
            self.frame_input_effects.exit_requested,
        )?;
        if let Some(host) = self.host.as_ref() {
            let ui_active = host.mono_ui_is_active();
            let movement_cadence = host.player_movement_cadence();
            let activity_demand = host.activity_demand();
            report_set_bool(&object, "active", ui_active)?;
            report_set_bool(&object, "uiActive", ui_active)?;
            report_set_string(&object, "activityDemand", activity_demand.label())?;
            if let mclone_app_runtime::client_entry::ClientActivityDemand::AnimatedUi {
                maximum_hz,
            } = activity_demand
            {
                report_set_number(&object, "activityMaximumHz", f64::from(maximum_hz))?;
            }
            report_set_number(
                &object,
                "playerMovementRateHz",
                f64::from(movement_cadence.rate_hz),
            )?;
            report_set_number(
                &object,
                "droppedPlayerMovementSteps",
                host.dropped_player_movement_steps() as f64,
            )?;
            report_set_bool(&object, "releasePointerCapture", ui_active)?;
            report_set_bool(
                &object,
                "sessionActive",
                matches!(host.session_state(), GameSessionState::Active { .. }),
            )?;
        }
        Ok(object.into())
    }

    fn finish_frame_operational_report(
        &mut self,
        rendered: bool,
        delta_seconds: f64,
        first_after_resume: bool,
    ) -> Result<JsValue, String> {
        let report = self.operational_report(rendered, delta_seconds, first_after_resume);
        self.frame_input_effects = WebFrameInputEffects::default();
        report
    }

    fn diagnostic_report(
        &self,
        summary: Option<&MonoSceneFrameSummary>,
        rendered: bool,
        delta_seconds: f64,
        first_after_resume: bool,
    ) -> Result<JsValue, String> {
        let value = self.operational_report(rendered, delta_seconds, first_after_resume)?;
        let object: js_sys::Object = value.unchecked_into();
        report_set_string(&object, "owner", "McloneSceneHost")?;
        report_set_number(&object, "frameCount", self.frame_count as f64)?;
        report_set_number(
            &object,
            "renderedFrameCount",
            self.rendered_frame_count as f64,
        )?;
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
        report_set_bool(
            &object,
            "settingsEffectApplied",
            self.settings_effect_applied,
        )?;
        report_set_bool(&object, "pauseUiRendered", self.pause_ui_rendered)?;
        report_set_bool(&object, "resized", self.resized)?;
        report_set_bool(&object, "configured", true)?;
        report_set_bool(&object, "assetPackLoaded", self.asset_pack_file_count > 0)?;
        report_set_bool(&object, "textured", self.asset_pack_file_count > 0)?;
        report_set_bool(&object, "skyRendered", rendered)?;
        report_set_number(
            &object,
            "assetPackFileCount",
            self.asset_pack_file_count as f64,
        )?;
        report_set_number(&object, "renderCount", self.rendered_frame_count as f64)?;
        report_set_number(&object, "interactionCount", self.interaction_count as f64)?;
        self.write_common_counts(&object)?;

        if let Some(host) = self.host.as_ref() {
            report_set_bool(&object, "lobbyLaunchActive", host.lobby_launch_active())?;
            report_set_string(
                &object,
                "lobbyDestinationFailure",
                host.lobby_destination_failure().unwrap_or(""),
            )?;
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
            if let Some(client) = host.mono_client() {
                let statistics = client.player_statistics();
                report_set_number(
                    &object,
                    "playerJumpStatistic",
                    f64::from(statistics.jump_count()),
                )?;
                report_set_number(
                    &object,
                    "playerSuccessfulBlockPlacementStatistic",
                    f64::from(statistics.successful_block_placement_count()),
                )?;
            }
            report_set_number(
                &object,
                "activeAssetEpoch",
                host.active_asset_epoch() as f64,
            )?;
            report_set_string(
                &object,
                "activeWorldInstanceId",
                &host.active_world_instance_id().get().to_string(),
            )?;
            report_set_string(
                &object,
                "activeWorldBehaviorProfile",
                host.active_world_behavior_profile().label(),
            )?;
            report_set_string(
                &object,
                "activeWorldSeedText",
                &host.active_world_seed().to_string(),
            )?;
            let active_persistent_actors = host.active_persistent_passive_actor_identity_summary();
            report_set_number(
                &object,
                "activePersistentPassiveActorCount",
                active_persistent_actors.count as f64,
            )?;
            report_set_string(
                &object,
                "activePersistentPassiveActorIdentityXor",
                &active_persistent_actors.identity_xor.to_string(),
            )?;
            report_set_string(
                &object,
                "activePersistentPassiveActorIdentitySum",
                &active_persistent_actors.identity_sum.to_string(),
            )?;
            report_set_string(
                &object,
                "activePersistentPassiveActorIds",
                &active_persistent_actors
                    .identities
                    .iter()
                    .flatten()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )?;
            if let Some(standby) = host.warm_world_standby_snapshot() {
                report_set_bool(&object, "standbyWorldPresent", true)?;
                report_set_string(
                    &object,
                    "standbyWorldInstanceId",
                    &standby.instance_id.get().to_string(),
                )?;
                report_set_string(&object, "standbyWorldPhase", standby.phase.label())?;
                report_set_number(
                    &object,
                    "standbyLoadedChunkCount",
                    standby.loaded_chunks as f64,
                )?;
                report_set_bool(
                    &object,
                    "standbyCadenceApplied",
                    standby.standby_cadence_applied,
                )?;
                report_set_bool(
                    &object,
                    "standbyCameraReconciled",
                    standby.camera_reconciled,
                )?;
                report_set_number(
                    &object,
                    "standbyQueuedUploadLifecycleItems",
                    standby.queued_upload_lifecycle_items as f64,
                )?;
                report_set_number(
                    &object,
                    "standbyEstimatedGpuTerrainBytes",
                    standby.estimated_gpu_terrain_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "standbyAtlasBaseBytes",
                    standby.atlas_base_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "standbyDuplicatedAtlasBaseBytes",
                    standby.duplicated_atlas_base_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "standbySharedTerrainResourceOwnerCount",
                    standby.shared_terrain_resource_owner_count as f64,
                )?;
                report_set_bool(
                    &object,
                    "standbyActorStateMaterialized",
                    standby.actor_state_materialized,
                )?;
                report_set_number(
                    &object,
                    "standbySharedActorResourceOwnerCount",
                    standby.shared_actor_resource_owner_count as f64,
                )?;
                report_set_number(
                    &object,
                    "standbySharedActorKnownRetainedBytes",
                    standby.shared_actor_known_retained_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "standbyActorStateAllocatedBytes",
                    standby.standby_actor_state_allocated_bytes as f64,
                )?;
                report_set_bool(&object, "standbySwitchable", standby.readiness.switchable)?;
                report_set_string(&object, "standbyWorldSeedText", &standby.seed.to_string())?;
            } else {
                report_set_bool(&object, "standbyWorldPresent", false)?;
            }
            if let Some(preview) = host.embedded_world_preview_snapshot() {
                report_set_string(
                    &object,
                    "embeddedPreviewWorldInstanceId",
                    &preview.source_world.get().to_string(),
                )?;
                report_set_string(
                    &object,
                    "embeddedPreviewPhase",
                    match preview.phase {
                        mclone_scene::EmbeddedWorldPreviewPhase::Warming => "warming",
                        mclone_scene::EmbeddedWorldPreviewPhase::Visible => "visible",
                        mclone_scene::EmbeddedWorldPreviewPhase::Failed => "failed",
                    },
                )?;
                let anchor = preview.placement.composition_anchor();
                report_set_number(&object, "embeddedPreviewAnchorX", anchor.x)?;
                report_set_number(&object, "embeddedPreviewAnchorY", anchor.y)?;
                report_set_number(&object, "embeddedPreviewAnchorZ", anchor.z)?;
                report_set_number(
                    &object,
                    "embeddedPreviewScale",
                    preview.placement.uniform_scale(),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMinChunkX",
                    f64::from(preview.region.min_chunk().x),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMinChunkZ",
                    f64::from(preview.region.min_chunk().z),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMaxChunkX",
                    f64::from(preview.region.max_chunk().x),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMaxChunkZ",
                    f64::from(preview.region.max_chunk().z),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewChunkWidth",
                    f64::from(preview.region.chunk_width()),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewChunkDepth",
                    f64::from(preview.region.chunk_depth()),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMinSectionY",
                    f64::from(preview.region.min_section_y()),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewMaxSectionY",
                    f64::from(preview.region.max_section_y()),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewBoundedSectionCount",
                    preview.bounded_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewDrawnSectionCount",
                    preview.last_drawn_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewDrawnIndexCount",
                    f64::from(preview.last_drawn_index_count),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewPendingCompileJobs",
                    preview.preparation.pending_compile_jobs as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewQueuedUploadLifecycleItems",
                    preview.preparation.queued_upload_lifecycle_items as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewOutOfRegionSubmissionCount",
                    preview.render.out_of_region_submission_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorEntityCount",
                    preview.render.last_actor_entity_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorRemotePlayerCount",
                    preview.render.last_actor_remote_player_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorSourceLocalPlayerCount",
                    preview.render.last_actor_source_local_player_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewSubmittedActorCount",
                    preview.render.last_submitted_actor_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewDrawnActorCount",
                    preview.render.last_drawn_actor_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewSourceRejectedActorCount",
                    preview.render.last_source_rejected_actor_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewClipRejectedActorCount",
                    preview.render.last_clip_rejected_actor_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewFrustumRejectedActorCount",
                    preview.render.last_frustum_rejected_actor_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorMeshRebuildCount",
                    preview.render.actor_mesh_rebuild_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorMeshUploadCount",
                    preview.render.actor_mesh_upload_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorGpuCapacityBytes",
                    preview.render.actor_gpu_capacity_bytes as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewPlacedActorPipelineCount",
                    preview.render.placed_actor_pipeline_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewPlacedActorMultiviewPipelineCount",
                    preview.render.placed_actor_multiview_pipeline_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorObservationCount",
                    preview.render.actor_observation_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewPersistentPassiveActorCount",
                    preview.render.persistent_passive_actor_count as f64,
                )?;
                report_set_string(
                    &object,
                    "embeddedPreviewPersistentPassiveActorIdentityXor",
                    &preview
                        .render
                        .persistent_passive_actor_identity_xor
                        .to_string(),
                )?;
                report_set_string(
                    &object,
                    "embeddedPreviewPersistentPassiveActorIdentitySum",
                    &preview
                        .render
                        .persistent_passive_actor_identity_sum
                        .to_string(),
                )?;
                report_set_string(
                    &object,
                    "embeddedPreviewPersistentPassiveActorIds",
                    &preview
                        .render
                        .persistent_passive_actor_identities
                        .iter()
                        .flatten()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewRemotePlayerObservationCount",
                    preview.render.remote_player_observation_count as f64,
                )?;
                for (prefix, observation) in [
                    ("First", preview.render.first_actor_observation),
                    ("Second", preview.render.second_actor_observation),
                ] {
                    let Some(observation) = observation else {
                        continue;
                    };
                    report_set_string(
                        &object,
                        &format!("embeddedPreview{prefix}ActorEntityId"),
                        &observation.entity_id.0.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        &format!("embeddedPreview{prefix}ActorPersistentId"),
                        &observation.persistent_id.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        &format!("embeddedPreview{prefix}ActorKind"),
                        match observation.kind {
                            mclone_protocol::EntityKind::Cow => "cow",
                            mclone_protocol::EntityKind::Chicken => "chicken",
                            mclone_protocol::EntityKind::Mannequin => "mannequin",
                            mclone_protocol::EntityKind::DebugCube => "debugCube",
                            mclone_protocol::EntityKind::Item => "item",
                        },
                    )?;
                    report_set_string(
                        &object,
                        &format!("embeddedPreview{prefix}ActorTickCount"),
                        &observation.tick_count.to_string(),
                    )?;
                    report_set_number(
                        &object,
                        &format!("embeddedPreview{prefix}ActorSourcePackedLight"),
                        observation.source_packed_light as f64,
                    )?;
                }
                if let Some(observation) = preview.render.first_remote_player_observation {
                    report_set_string(
                        &object,
                        "embeddedPreviewFirstRemotePlayerId",
                        &observation.player_id.0.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        "embeddedPreviewFirstRemotePlayerModel",
                        match observation.appearance.model {
                            mclone_protocol::PlayerModelKind::Player => "player",
                            mclone_protocol::PlayerModelKind::UprightBear => "uprightBear",
                        },
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewFirstRemotePlayerWalkDistance",
                        f64::from(observation.walk_animation_distance),
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewFirstRemotePlayerSourcePackedLight",
                        observation.source_packed_light as f64,
                    )?;
                    for (suffix, value) in [
                        ("SourceX", observation.source_feet_position.x),
                        ("SourceY", observation.source_feet_position.y),
                        ("SourceZ", observation.source_feet_position.z),
                        ("CompositionX", observation.composition_feet_position.x),
                        ("CompositionY", observation.composition_feet_position.y),
                        ("CompositionZ", observation.composition_feet_position.z),
                    ] {
                        report_set_number(
                            &object,
                            &format!("embeddedPreviewFirstRemotePlayer{suffix}"),
                            value,
                        )?;
                    }
                }
                report_set_number(
                    &object,
                    "embeddedPreviewActorMotionSequence",
                    preview.render.actor_motion_sequence as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorUpdateToVisibleMs",
                    preview.render.last_actor_update_to_visible_ms,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewActorUpdateToVisibleFrameCount",
                    preview.render.last_actor_update_to_visible_frame_count as f64,
                )?;
                if let (Some(from), Some(to)) = (
                    preview.render.last_actor_motion_from,
                    preview.render.last_actor_motion_to,
                ) {
                    report_set_string(
                        &object,
                        "embeddedPreviewActorMotionEntityId",
                        &to.entity_id.0.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        "embeddedPreviewActorMotionKind",
                        match to.kind {
                            mclone_protocol::EntityKind::Cow => "cow",
                            mclone_protocol::EntityKind::Chicken => "chicken",
                            mclone_protocol::EntityKind::Mannequin => "mannequin",
                            mclone_protocol::EntityKind::DebugCube => "debugCube",
                            mclone_protocol::EntityKind::Item => "item",
                        },
                    )?;
                    report_set_string(
                        &object,
                        "embeddedPreviewActorMotionFromTickCount",
                        &from.tick_count.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        "embeddedPreviewActorMotionToTickCount",
                        &to.tick_count.to_string(),
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewActorMotionSourcePackedLight",
                        to.source_packed_light as f64,
                    )?;
                    for (suffix, value) in [
                        ("FromSourceX", from.source_feet_position.x),
                        ("FromSourceY", from.source_feet_position.y),
                        ("FromSourceZ", from.source_feet_position.z),
                        ("ToSourceX", to.source_feet_position.x),
                        ("ToSourceY", to.source_feet_position.y),
                        ("ToSourceZ", to.source_feet_position.z),
                        ("FromCompositionX", from.composition_feet_position.x),
                        ("FromCompositionY", from.composition_feet_position.y),
                        ("FromCompositionZ", from.composition_feet_position.z),
                        ("ToCompositionX", to.composition_feet_position.x),
                        ("ToCompositionY", to.composition_feet_position.y),
                        ("ToCompositionZ", to.composition_feet_position.z),
                    ] {
                        report_set_number(
                            &object,
                            &format!("embeddedPreviewActorMotion{suffix}"),
                            value,
                        )?;
                    }
                }
                report_set_number(
                    &object,
                    "embeddedPreviewRemotePlayerMotionSequence",
                    preview.render.remote_player_motion_sequence as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewRemotePlayerUpdateToVisibleMs",
                    preview.render.last_remote_player_update_to_visible_ms,
                )?;
                report_set_number(
                    &object,
                    "embeddedPreviewRemotePlayerUpdateToVisibleFrameCount",
                    preview
                        .render
                        .last_remote_player_update_to_visible_frame_count
                        as f64,
                )?;
                if let (Some(from), Some(to)) = (
                    preview.render.last_remote_player_motion_from,
                    preview.render.last_remote_player_motion_to,
                ) {
                    report_set_string(
                        &object,
                        "embeddedPreviewRemotePlayerMotionId",
                        &to.player_id.0.to_string(),
                    )?;
                    report_set_string(
                        &object,
                        "embeddedPreviewRemotePlayerMotionModel",
                        match to.appearance.model {
                            mclone_protocol::PlayerModelKind::Player => "player",
                            mclone_protocol::PlayerModelKind::UprightBear => "uprightBear",
                        },
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewRemotePlayerMotionFromWalkDistance",
                        f64::from(from.walk_animation_distance),
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewRemotePlayerMotionToWalkDistance",
                        f64::from(to.walk_animation_distance),
                    )?;
                    report_set_number(
                        &object,
                        "embeddedPreviewRemotePlayerMotionSourcePackedLight",
                        to.source_packed_light as f64,
                    )?;
                    for (suffix, value) in [
                        ("FromSourceX", from.source_feet_position.x),
                        ("FromSourceY", from.source_feet_position.y),
                        ("FromSourceZ", from.source_feet_position.z),
                        ("ToSourceX", to.source_feet_position.x),
                        ("ToSourceY", to.source_feet_position.y),
                        ("ToSourceZ", to.source_feet_position.z),
                        ("FromCompositionX", from.composition_feet_position.x),
                        ("FromCompositionY", from.composition_feet_position.y),
                        ("FromCompositionZ", from.composition_feet_position.z),
                        ("ToCompositionX", to.composition_feet_position.x),
                        ("ToCompositionY", to.composition_feet_position.y),
                        ("ToCompositionZ", to.composition_feet_position.z),
                    ] {
                        report_set_number(
                            &object,
                            &format!("embeddedPreviewRemotePlayerMotion{suffix}"),
                            value,
                        )?;
                    }
                }
            }
            let activation = host.embedded_world_activation_snapshot();
            report_set_string(
                &object,
                "embeddedActivationPhase",
                match activation.phase {
                    mclone_scene::EmbeddedWorldActivationPhase::Idle => "idle",
                    mclone_scene::EmbeddedWorldActivationPhase::Closing => "closing",
                    mclone_scene::EmbeddedWorldActivationPhase::Covered => "covered",
                    mclone_scene::EmbeddedWorldActivationPhase::Opening => "opening",
                    mclone_scene::EmbeddedWorldActivationPhase::Failed => "failed",
                },
            )?;
            report_set_number(
                &object,
                "embeddedActivationAlpha",
                f64::from(activation.alpha),
            )?;
            report_set_bool(
                &object,
                "embeddedActivationReady",
                activation.activation_ready,
            )?;
            report_set_number(&object, "embeddedActivationSequence", 0.0)?;
            report_set_bool(&object, "embeddedActivationFailed", false)?;
            if let Some(report) = activation.last_report {
                report_set_number(
                    &object,
                    "embeddedActivationSequence",
                    report.sequence as f64,
                )?;
                report_set_string(
                    &object,
                    "embeddedActivationSourceWorldInstanceId",
                    &report.source_world.get().to_string(),
                )?;
                report_set_string(
                    &object,
                    "embeddedActivationDestinationWorldInstanceId",
                    &report.destination_world.get().to_string(),
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationSwitchElapsedMs",
                    report.switch_elapsed_ms.unwrap_or(0.0),
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationCoveredRenderedFrames",
                    f64::from(report.covered_rendered_frames),
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredFrame",
                    f64::from(report.first_uncovered_activation_frame.unwrap_or(0)),
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredDrawnSectionCount",
                    report.first_uncovered_drawn_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredUploadedSectionCount",
                    report.first_uncovered_uploaded_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredSubmittedCompileSectionCount",
                    report.first_uncovered_submitted_compile_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredAcceptedCompileResultCount",
                    report.first_uncovered_accepted_compile_result_count as f64,
                )?;
                report_set_number(
                    &object,
                    "embeddedActivationFirstUncoveredEyeCount",
                    report.first_uncovered_eye_count as f64,
                )?;
                if let Some(pose) = report.accepted_destination_entry_pose {
                    for (suffix, value) in [
                        ("X", pose.feet_position.x),
                        ("Y", pose.feet_position.y),
                        ("Z", pose.feet_position.z),
                    ] {
                        report_set_number(
                            &object,
                            &format!("embeddedActivationAcceptedEntry{suffix}"),
                            value,
                        )?;
                    }
                }
                for (prefix, sample) in [
                    ("PostSwap", report.post_swap_entry),
                    ("FirstUncovered", report.first_uncovered_entry),
                    ("Stability", report.stability_entry),
                ] {
                    if let Some(sample) = sample {
                        for (suffix, value) in [
                            ("X", sample.pose.feet_position.x),
                            ("Y", sample.pose.feet_position.y),
                            ("Z", sample.pose.feet_position.z),
                        ] {
                            report_set_number(
                                &object,
                                &format!("embeddedActivation{prefix}{suffix}"),
                                value,
                            )?;
                        }
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}OnGround"),
                            sample.on_ground,
                        )?;
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}BodyLoaded"),
                            sample.support.body_loaded,
                        )?;
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}BodyClear"),
                            sample.support.body_clear,
                        )?;
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}SupportLoaded"),
                            sample.support.support_loaded,
                        )?;
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}SolidSupport"),
                            sample.support.solid_support,
                        )?;
                        report_set_bool(
                            &object,
                            &format!("embeddedActivation{prefix}Supported"),
                            sample.support.supported(),
                        )?;
                    }
                }
                report_set_number(
                    &object,
                    "embeddedActivationStabilityFrame",
                    f64::from(report.stability_activation_frame.unwrap_or(0)),
                )?;
                report_set_bool(
                    &object,
                    "embeddedActivationFailed",
                    report.failure.is_some(),
                )?;
                if let Some(failure) = report.failure.as_deref() {
                    report_set_string(&object, "embeddedActivationFailure", failure)?;
                }
            }
            if let Some(switch) = host.last_warm_world_switch_report() {
                report_set_number(&object, "warmWorldSwitchSequence", switch.sequence as f64)?;
                report_set_number(
                    &object,
                    "warmWorldSwitchUploadedSectionCount",
                    switch.switch_uploaded_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "warmWorldSwitchSubmittedCompileSectionCount",
                    switch.switch_submitted_compile_section_count as f64,
                )?;
                report_set_number(
                    &object,
                    "warmWorldSwitchAcceptedCompileResultCount",
                    switch.switch_accepted_compile_result_count as f64,
                )?;
                report_set_bool(
                    &object,
                    "warmWorldSwitchMaterializedRenderer",
                    switch.switch_materialized_renderer,
                )?;
            }
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
            let ui_state = host.mono_ui_render_state();
            let world_catalog = ui_state.world_catalog;
            report_set_bool(&object, "uiActive", host.mono_ui_is_active())?;
            report_set_bool(&object, "uiCoversWorld", host.mono_ui_covers_world())?;
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
            report_set_string(
                &object,
                "farLodDetailMode",
                ui_state.far_lod_detail_mode.label(),
            )?;
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
            for (index, count) in far_lod.resident_tiles_by_level.into_iter().enumerate() {
                report_set_number(
                    &object,
                    &format!("farLodResidentLevel{}Tiles", index + 1),
                    count as f64,
                )?;
            }
            for (index, count) in far_lod.visible_tiles_by_level.into_iter().enumerate() {
                report_set_number(
                    &object,
                    &format!("farLodVisibleLevel{}Tiles", index + 1),
                    count as f64,
                )?;
            }
            report_set_number(
                &object,
                "farLodDoubleResidentTiles",
                far_lod.double_resident_tiles as f64,
            )?;
            report_set_number(
                &object,
                "farLodMaxDoubleResidentTiles",
                far_lod.max_double_resident_tiles as f64,
            )?;
            report_set_number(&object, "farLodLevelFlips", far_lod.level_flips as f64)?;
            report_set_number(
                &object,
                "farLodMaxLevelFlipsPerTile",
                far_lod.max_level_flips_per_tile as f64,
            )?;
            let lod_replacements = host.lod_coverage_counters();
            report_set_number(
                &object,
                "farLodSuppressedWithoutReplacement",
                lod_replacements.suppressed_without_replacement as f64,
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
            report_set_bool(
                &object,
                "debugOverlayVisible",
                host.mono_debug_diagnostics_visible(),
            )?;
            report_set_string(
                &object,
                "touchControlsMode",
                touch_controls_mode_label(self.touch_controls_mode),
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
            report_set_bool(&object, "touchControlsVisible", self.touch_overlay.visible)?;
            if let Some(error) = &self.input_preference_error {
                report_set_string(&object, "inputPreferenceError", error)?;
            }
            write_session_state(&object, host.session_state())?;
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
            report_set_bool(&object, "startupReady", host.gameplay_startup_complete())?;
            if let Some(progress) = host.mono_loading_progress_overlay() {
                report_set_bool(&object, "startupProgressVisible", true)?;
                report_set_number(
                    &object,
                    "startupProgressReadyChunks",
                    progress.target_ready_chunks as f64,
                )?;
                report_set_number(
                    &object,
                    "startupProgressChunkCount",
                    progress.target_chunk_count as f64,
                )?;
                report_set_number(
                    &object,
                    "startupProgressPercent",
                    f64::from(progress.percent()),
                )?;
            } else {
                report_set_bool(&object, "startupProgressVisible", false)?;
            }
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
                let settled = self.streaming_idle();
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
            report_set_number(
                &object,
                "grassResidentPatchCount",
                summary.render.grass_resident_patch_count as f64,
            )?;
            report_set_number(
                &object,
                "grassDrawnPatchCount",
                summary.render.grass_drawn_patch_count as f64,
            )?;
            report_set_number(
                &object,
                "grassEstimatedBladeCount",
                summary.render.grass_estimated_blade_count as f64,
            )?;
            report_set_number(
                &object,
                "grassDrawCalls",
                summary.render.grass_draw_calls as f64,
            )?;
            report_set_number(
                &object,
                "grassResidentBytes",
                summary.render.grass_resident_bytes as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionFieldCount",
                summary.render.grass_interaction_field_count as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionActiveCellCount",
                summary.render.grass_interaction_active_cell_count as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionStampCount",
                summary.render.grass_interaction_stamp_count as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionRecenterCount",
                summary.render.grass_interaction_recenter_count as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionResetCount",
                summary.render.grass_interaction_reset_count as f64,
            )?;
            report_set_number(
                &object,
                "grassInteractionUploadedBytes",
                summary.render.grass_interaction_uploaded_bytes as f64,
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
        if let Some(host) = self.host.as_ref() {
            report_set_string(
                &object,
                "grassDetail",
                match host.grass_detail() {
                    mclone_render::GrassQuality::Off => "off",
                    mclone_render::GrassQuality::Sparse => "sparse",
                    mclone_render::GrassQuality::Lush => "lush",
                    mclone_render::GrassQuality::Ultra => "ultra",
                },
            )?;
            report_set_bool(
                &object,
                "grassPatchesCompileEnabled",
                host.active_grass_patches_compile_enabled(),
            )?;
            report_set_string(
                &object,
                "graphicsPreferenceError",
                host.graphics_preference_error().unwrap_or_default(),
            )?;
        }
        report_set_number(
            &object,
            "grassResidentPatchCount",
            self.last_frame.grass_resident_patch_count as f64,
        )?;
        report_set_number(
            &object,
            "grassDrawnPatchCount",
            self.last_frame.grass_drawn_patch_count as f64,
        )?;
        report_set_number(
            &object,
            "grassEstimatedBladeCount",
            self.last_frame.grass_estimated_blade_count as f64,
        )?;
        report_set_number(
            &object,
            "grassDrawCalls",
            self.last_frame.grass_draw_calls as f64,
        )?;
        report_set_number(
            &object,
            "grassResidentBytes",
            self.last_frame.grass_resident_bytes as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionFieldCount",
            self.last_frame.grass_interaction_field_count as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionActiveCellCount",
            self.last_frame.grass_interaction_active_cell_count as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionStampCount",
            self.last_frame.grass_interaction_stamp_count as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionRecenterCount",
            self.last_frame.grass_interaction_recenter_count as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionResetCount",
            self.last_frame.grass_interaction_reset_count as f64,
        )?;
        report_set_number(
            &object,
            "grassInteractionUploadedBytes",
            self.last_frame.grass_interaction_uploaded_bytes as f64,
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
        let gui_scale = GuiScale::from_pixels(self.context.width, self.context.height);
        report_set_bool(
            &object,
            "touchJoystickGuiActive",
            self.touch_overlay.movement.active,
        )?;
        report_set_number(
            &object,
            "touchJoystickGuiBaseX",
            self.touch_overlay.movement.base.x as f64,
        )?;
        report_set_number(
            &object,
            "touchJoystickGuiBaseY",
            self.touch_overlay.movement.base.y as f64,
        )?;
        report_set_bool(
            &object,
            "touchJoystickGuiInBounds",
            self.touch_overlay.movement.base.x >= 0.0
                && self.touch_overlay.movement.base.x <= gui_scale.width
                && self.touch_overlay.movement.base.y >= 0.0
                && self.touch_overlay.movement.base.y <= gui_scale.height,
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
            "acceptedCompileSectionCount",
            self.last_frame.accepted_compile_section_count as f64,
        )?;
        report_set_number(&object, "pollUpdates", self.last_frame.poll_updates as f64)?;
        report_set_number(
            &object,
            "frameRenderViewsMs",
            self.last_frame.render_views_ms,
        )?;
        report_set_bool(&object, "playable", self.last_frame.drawn_section_count > 0)?;
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
        self.render_worker.write_report(
            &object,
            self.command_count,
            self.last_frame.accepted_compile_section_count,
            self.mesh_build_count,
            self.pending_chunk_render_compile_job_count(),
        )?;
        self.write_last_action_receipt(&object)?;
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
    let fly_speed = host.mono_camera_speed_blocks_per_second();
    host.set_mono_player_camera(
        Vec3d::new(f64::from(eye.x), f64::from(eye.y), f64::from(eye.z)),
        f64::from(yaw),
        f64::from(pitch),
        fly_speed,
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

fn browser_keyboard_key(code: &str, legacy_key: &str) -> Option<KeyboardKey> {
    KeyboardKey::from_code_name(code).or_else(|| match legacy_key {
        "w" | "W" => Some(KeyboardKey::KeyW),
        "a" | "A" => Some(KeyboardKey::KeyA),
        "s" | "S" => Some(KeyboardKey::KeyS),
        "d" | "D" => Some(KeyboardKey::KeyD),
        "x" | "X" => Some(KeyboardKey::KeyX),
        "n" | "N" => Some(KeyboardKey::KeyN),
        "`" => Some(KeyboardKey::Backquote),
        " " | "Spacebar" => Some(KeyboardKey::Space),
        "Shift" => Some(KeyboardKey::ShiftLeft),
        "Control" => Some(KeyboardKey::ControlLeft),
        "Escape" => Some(KeyboardKey::Escape),
        "F1" => Some(KeyboardKey::F1),
        "F5" => Some(KeyboardKey::F5),
        "ArrowUp" => Some(KeyboardKey::ArrowUp),
        "ArrowDown" => Some(KeyboardKey::ArrowDown),
        "ArrowLeft" => Some(KeyboardKey::ArrowLeft),
        "ArrowRight" => Some(KeyboardKey::ArrowRight),
        "1" => Some(KeyboardKey::Digit1),
        "2" => Some(KeyboardKey::Digit2),
        "3" => Some(KeyboardKey::Digit3),
        "4" => Some(KeyboardKey::Digit4),
        "5" => Some(KeyboardKey::Digit5),
        "6" => Some(KeyboardKey::Digit6),
        "7" => Some(KeyboardKey::Digit7),
        "8" => Some(KeyboardKey::Digit8),
        "9" => Some(KeyboardKey::Digit9),
        _ => None,
    })
}

fn browser_pointer_button(button: i16) -> Option<PointerButton> {
    match button {
        0 => Some(PointerButton::Primary),
        1 => Some(PointerButton::Middle),
        2 => Some(PointerButton::Secondary),
        _ => None,
    }
}

fn web_touch_contact_phase(phase: &str) -> Result<TouchContactPhase, JsValue> {
    match phase {
        "start" => Ok(TouchContactPhase::Started),
        "move" => Ok(TouchContactPhase::Moved),
        "end" => Ok(TouchContactPhase::Ended),
        "cancel" => Ok(TouchContactPhase::Cancelled),
        other => Err(JsValue::from_str(&format!(
            "unknown raw touch phase {other:?}"
        ))),
    }
}

fn merge_input_disposition(destination: &mut MonoInputDisposition, source: MonoInputDisposition) {
    destination.handled |= source.handled;
    destination.scene_changed |= source.scene_changed;
    destination.clear_transient_input |= source.clear_transient_input;
    destination.request_pointer_capture_when_ready |= source.request_pointer_capture_when_ready;
}

fn point_from_vec2(point: Vec2) -> Point {
    Point {
        x: point.x,
        y: point.y,
    }
}

fn screen_label(screen: Option<GameScreen>) -> &'static str {
    match screen {
        None => "none",
        Some(GameScreen::Title) => "title",
        Some(GameScreen::PreparingLobby) => "preparingLobby",
        Some(GameScreen::WorldList) => "worldList",
        Some(GameScreen::WorldCreate) => "worldCreate",
        Some(GameScreen::WorldDeleteConfirm { .. }) => "worldDeleteConfirm",
        Some(GameScreen::NewWorld) => "newWorld",
        Some(GameScreen::JoinRemote) => "joinRemote",
        Some(GameScreen::Pause) => "pause",
        Some(GameScreen::Death { .. }) => "death",
        Some(GameScreen::Help { .. }) => "help",
        Some(GameScreen::BlockPalette) => "blockPalette",
        Some(GameScreen::Options { .. }) => "options",
        Some(GameScreen::OptionsCategory { .. }) => "optionsCategory",
        Some(GameScreen::ServerSettings { .. }) => "serverSettings",
        Some(GameScreen::AssetPacks { .. }) => "assetPacks",
        Some(GameScreen::StorageConfirm { .. }) => "storageConfirm",
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

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use mclone_app_runtime::platform_operation::{
        PlatformOperationLedger, PlatformOperationResolution,
    };
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    async fn test_only_coarse_operation_runs_through_generic_drain() {
        let instance_id = mclone_scene::WorldInstanceId::new(41);
        let mut ledger = PlatformOperationLedger::new();
        let issued = ledger.issue(instance_id, ());
        let endpoint = RemoteSessionEndpoint::new("test-only.invalid:25565");
        let request = SessionStartRequest::JoinRemote {
            endpoint: endpoint.clone(),
        };
        let pending = ExternalSceneSessionStart {
            target: mclone_scene::ExternalSceneStartTarget::ActiveSession {
                token: issued.token,
            },
            instance_id,
            storage_source: None,
            request,
            runtime_kind: mclone_app_runtime::session::SessionRuntimeKind::Remote,
            scene: McloneSceneHostOptions::default(),
            descriptor: ActiveSessionDescriptor::Remote { endpoint },
            destination: None,
        };
        let effect = WebSceneOperationEffect::Runtime {
            pending,
            start: WebRuntimeStartEffect::TestOnlyRemote("new-operation-kind".to_owned()),
            center: ChunkPos::new(0, 0),
            render_distance: 1,
        };

        let mut operation = WebSceneOperationDrain::take_scene_operation(effect);
        JsFuture::from(operation.start().expect("generic Promise start"))
            .await
            .expect("test-only operation settles through the Promise capability");
        let completion = WebSceneOperationDrain::complete_scene_operation(&mut operation)
            .expect("generic completion drain");

        let WebSceneOperationCompletion::Runtime { pending, outcome } = completion else {
            panic!("test-only remote changed operation capability");
        };
        let error = outcome.expect_err("test-only remote returns its deterministic result");
        assert_eq!(error, "test-only remote completion: new-operation-kind");
        assert!(matches!(
            ledger.complete(PlatformOperationCompletion {
                token: match pending.target {
                    mclone_scene::ExternalSceneStartTarget::ActiveSession { token } => token,
                    mclone_scene::ExternalSceneStartTarget::Lobby { .. } => {
                        panic!("test-only remote changed completion target")
                    }
                },
                result: Err::<(), _>(error),
            }),
            PlatformOperationResolution::Failed { kind, .. } if kind == instance_id
        ));
        assert!(ledger.is_empty());
    }
}
