use crate::client_catalog_policy::{
    ClientCatalogActionContext, ClientCatalogController, ClientCatalogEffects,
};
use crate::client_session_policy::{
    ClientSessionActionContext, ClientSessionEffects, client_session_effects_for_action,
};
use crate::far_lod::{
    MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS, MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
};
use mclone_input::TouchControlsMode;
use mclone_ui::{
    GameCollisionMode, GameFramePacingMode, GameMovementMode, GamePlayerModel,
    GameSimulationCadence, GameTouchSettings, GameTravelAssistMode, GameTurnMode, GameUiAction,
    GameUiRenderState, GameXrTurnMode,
};

use crate::asset_pack_ui::{ClientAssetPackController, ClientAssetPackEffect};

#[derive(Clone, Debug, PartialEq)]
pub struct ClientExperienceController {
    profile: ClientExperienceProfile,
    catalog: ClientCatalogController,
    asset_packs: ClientAssetPackController,
    settings: ClientExperienceSettingsController,
}

impl Default for ClientExperienceController {
    fn default() -> Self {
        Self::new(ClientExperienceProfile::default())
    }
}

impl ClientExperienceController {
    pub fn new(profile: ClientExperienceProfile) -> Self {
        Self {
            profile,
            catalog: ClientCatalogController::new(),
            asset_packs: ClientAssetPackController::default(),
            settings: ClientExperienceSettingsController::default(),
        }
    }

    pub fn profile(&self) -> ClientExperienceProfile {
        self.profile
    }

    pub fn set_profile(&mut self, profile: ClientExperienceProfile) {
        self.profile = profile;
    }

    pub fn catalog(&self) -> &ClientCatalogController {
        &self.catalog
    }

    pub fn catalog_mut(&mut self) -> &mut ClientCatalogController {
        &mut self.catalog
    }

    pub fn asset_packs(&self) -> &ClientAssetPackController {
        &self.asset_packs
    }

    pub fn asset_packs_mut(&mut self) -> &mut ClientAssetPackController {
        &mut self.asset_packs
    }

    pub fn settings(&self) -> &ClientExperienceSettingsController {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut ClientExperienceSettingsController {
        &mut self.settings
    }

    pub fn set_settings_state(&mut self, state: ClientExperienceSettingsState) {
        self.settings.set_state(state);
    }

    pub fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        context: ClientExperienceActionContext<'_>,
    ) -> ClientExperienceEffects {
        let mut effects = ClientExperienceEffects::default();
        match action {
            GameUiAction::OpenWorldList
            | GameUiAction::OpenWorldCreate
            | GameUiAction::SelectWorld(_)
            | GameUiAction::OpenWorld(_)
            | GameUiAction::CreateCatalogWorld
            | GameUiAction::ConfirmDeleteWorld(_)
            | GameUiAction::DeleteWorld(_)
            | GameUiAction::CancelDeleteWorld => {
                effects.catalog = self.catalog.apply_ui_action(
                    action,
                    ClientCatalogActionContext {
                        new_world_seed: context.new_world_seed,
                    },
                );
                if matches!(action, GameUiAction::OpenWorldCreate) {
                    effects.session.new_world_seed = Some(context.new_world_seed);
                }
                if matches!(
                    action,
                    GameUiAction::OpenWorldList
                        | GameUiAction::OpenWorldCreate
                        | GameUiAction::SelectWorld(_)
                        | GameUiAction::ConfirmDeleteWorld(_)
                        | GameUiAction::CancelDeleteWorld
                ) {
                    effects.session.clear_inactive_session_status = true;
                }
            }
            GameUiAction::OpenNewWorld
            | GameUiAction::OpenJoinRemote
            | GameUiAction::RerollSeed
            | GameUiAction::CreateWorld(_)
            | GameUiAction::JoinRemote
            | GameUiAction::BackToTitle
            | GameUiAction::QuitToTitle
            | GameUiAction::Quit => {
                effects.session = client_session_effects_for_action(
                    action,
                    ClientSessionActionContext {
                        next_new_world_seed: context.next_new_world_seed,
                        current_join_remote_addr: context.current_join_remote_addr,
                        fallback_remote_addr: context.fallback_remote_addr,
                    },
                );
            }
            GameUiAction::ToggleSectionOcclusion
            | GameUiAction::ToggleFullbright
            | GameUiAction::ToggleFarLod
            | GameUiAction::SetFarLodRange(_)
            | GameUiAction::TogglePlayerCollisionBox
            | GameUiAction::ToggleFirstPersonPlayer
            | GameUiAction::ToggleCrosshair
            | GameUiAction::ToggleFramePipelineOverlay
            | GameUiAction::ToggleDebugDiagnostics
            | GameUiAction::SetPlayerModel(_)
            | GameUiAction::SetMovementMode(_)
            | GameUiAction::SetCollisionMode(_)
            | GameUiAction::SetTravelAssistMode(_)
            | GameUiAction::SetTurnMode(_)
            | GameUiAction::SetXrTurnMode(_)
            | GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetRenderDistance(_)
            | GameUiAction::SetFlySpeed(_)
            | GameUiAction::SetMovementSpeed(_)
            | GameUiAction::SetTouchLookSensitivity(_)
            | GameUiAction::SetTouchControlsMode(_)
            | GameUiAction::SetServerSimulationCadence(_) => {
                effects.settings = self.settings.apply_ui_action(action, self.profile.settings);
            }
            GameUiAction::OpenAssetPacks(_)
            | GameUiAction::ToggleAssetPack(_)
            | GameUiAction::ApplyAssetPacks
            | GameUiAction::CancelAssetPacks => {
                if let Some(effect) =
                    self.asset_packs
                        .apply_ui_action(action)
                        .unwrap_or_else(|error| {
                            self.asset_packs.mark_failed(format!("{error:#}"));
                            None
                        })
                {
                    effects.asset_packs.push(effect);
                }
            }
            GameUiAction::AssignHotbarBlock { slot, block_state } => {
                effects
                    .gameplay
                    .push(ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state });
            }
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenBlockPalette
            | GameUiAction::OpenHelp(_)
            | GameUiAction::CloseHelp(_)
            | GameUiAction::OpenOptions(_)
            | GameUiAction::OpenOptionsCategory(_, _)
            | GameUiAction::OpenServerSettings(_)
            | GameUiAction::BackToPause => {
                effects
                    .projection
                    .push(ClientExperienceProjectionEffect::ApplyUiAction(action));
            }
        }
        effects
    }

    pub fn capability_projection(&self) -> ClientExperienceCapabilityProjection {
        self.settings.capability_projection(self.profile.settings)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientExperienceActionContext<'a> {
    pub new_world_seed: i64,
    pub next_new_world_seed: Option<i64>,
    pub current_join_remote_addr: &'a str,
    pub fallback_remote_addr: Option<&'a str>,
}

impl Default for ClientExperienceActionContext<'static> {
    fn default() -> Self {
        Self {
            new_world_seed: 0,
            next_new_world_seed: None,
            current_join_remote_addr: "",
            fallback_remote_addr: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientExperienceEffects {
    pub catalog: ClientCatalogEffects,
    pub session: ClientSessionEffects,
    pub settings: ClientExperienceSettingsEffects,
    pub asset_packs: Vec<ClientAssetPackEffect>,
    pub gameplay: Vec<ClientExperienceGameplayEffect>,
    pub projection: Vec<ClientExperienceProjectionEffect>,
}

impl ClientExperienceEffects {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientExperienceGameplayEffect {
    AssignHotbarBlock { slot: u8, block_state: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientExperienceProjectionEffect {
    ApplyUiAction(GameUiAction),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientExperienceProfile {
    pub settings: ClientExperienceSettingsProfile,
}

impl ClientExperienceProfile {
    pub const fn new(settings: ClientExperienceSettingsProfile) -> Self {
        Self { settings }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientExperienceSettingsProfile {
    pub section_occlusion: ClientExperienceCapabilityStatus,
    pub fullbright: ClientExperienceCapabilityStatus,
    pub far_lod: ClientExperienceCapabilityStatus,
    pub player_collision_box: ClientExperienceCapabilityStatus,
    pub first_person_player: ClientExperienceCapabilityStatus,
    pub crosshair: ClientExperienceCapabilityStatus,
    pub frame_pipeline_overlay: ClientExperienceCapabilityStatus,
    pub debug_diagnostics: ClientExperienceCapabilityStatus,
    pub player_model: ClientExperienceCapabilityStatus,
    pub movement_mode: ClientExperienceCapabilityStatus,
    pub collision_mode: ClientExperienceCapabilityStatus,
    pub travel_assist: ClientExperienceCapabilityStatus,
    pub turn_mode: ClientExperienceCapabilityStatus,
    pub xr_turn: ClientExperienceCapabilityStatus,
    pub frame_pacing: ClientExperienceCapabilityStatus,
    pub fps_cap: ClientExperienceCapabilityStatus,
    pub render_distance: ClientExperienceCapabilityStatus,
    pub fly_speed: ClientExperienceCapabilityStatus,
    pub movement_speed: ClientExperienceCapabilityStatus,
    pub touch_look: ClientExperienceCapabilityStatus,
    pub touch_controls: ClientExperienceCapabilityStatus,
    pub server_simulation_cadence: ClientExperienceCapabilityStatus,
}

impl Default for ClientExperienceSettingsProfile {
    fn default() -> Self {
        Self::all_supported()
    }
}

impl ClientExperienceSettingsProfile {
    pub const fn all_supported() -> Self {
        Self {
            section_occlusion: ClientExperienceCapabilityStatus::Supported,
            fullbright: ClientExperienceCapabilityStatus::Supported,
            far_lod: ClientExperienceCapabilityStatus::Supported,
            player_collision_box: ClientExperienceCapabilityStatus::Supported,
            first_person_player: ClientExperienceCapabilityStatus::Supported,
            crosshair: ClientExperienceCapabilityStatus::Supported,
            frame_pipeline_overlay: ClientExperienceCapabilityStatus::Supported,
            debug_diagnostics: ClientExperienceCapabilityStatus::Supported,
            player_model: ClientExperienceCapabilityStatus::Supported,
            movement_mode: ClientExperienceCapabilityStatus::Supported,
            collision_mode: ClientExperienceCapabilityStatus::Supported,
            travel_assist: ClientExperienceCapabilityStatus::Supported,
            turn_mode: ClientExperienceCapabilityStatus::Supported,
            xr_turn: ClientExperienceCapabilityStatus::Supported,
            frame_pacing: ClientExperienceCapabilityStatus::Supported,
            fps_cap: ClientExperienceCapabilityStatus::Supported,
            render_distance: ClientExperienceCapabilityStatus::Supported,
            fly_speed: ClientExperienceCapabilityStatus::Supported,
            movement_speed: ClientExperienceCapabilityStatus::Supported,
            touch_look: ClientExperienceCapabilityStatus::Supported,
            touch_controls: ClientExperienceCapabilityStatus::Supported,
            server_simulation_cadence: ClientExperienceCapabilityStatus::Supported,
        }
    }

    pub fn capability_for_action(
        self,
        kind: ClientExperienceActionKind,
    ) -> ClientExperienceCapabilityStatus {
        match kind {
            ClientExperienceActionKind::ToggleSectionOcclusion => self.section_occlusion,
            ClientExperienceActionKind::ToggleFullbright => self.fullbright,
            ClientExperienceActionKind::ToggleFarLod
            | ClientExperienceActionKind::SetFarLodRange => self.far_lod,
            ClientExperienceActionKind::TogglePlayerCollisionBox => self.player_collision_box,
            ClientExperienceActionKind::ToggleFirstPersonPlayer => self.first_person_player,
            ClientExperienceActionKind::ToggleCrosshair => self.crosshair,
            ClientExperienceActionKind::ToggleFramePipelineOverlay => self.frame_pipeline_overlay,
            ClientExperienceActionKind::ToggleDebugDiagnostics => self.debug_diagnostics,
            ClientExperienceActionKind::SetPlayerModel => self.player_model,
            ClientExperienceActionKind::SetMovementMode => self.movement_mode,
            ClientExperienceActionKind::SetCollisionMode => self.collision_mode,
            ClientExperienceActionKind::SetTravelAssistMode => self.travel_assist,
            ClientExperienceActionKind::SetTurnMode => self.turn_mode,
            ClientExperienceActionKind::SetXrTurnMode => self.xr_turn,
            ClientExperienceActionKind::CycleFramePacing => self.frame_pacing,
            ClientExperienceActionKind::CycleFpsCap => self.fps_cap,
            ClientExperienceActionKind::SetRenderDistance => self.render_distance,
            ClientExperienceActionKind::SetFlySpeed => self.fly_speed,
            ClientExperienceActionKind::SetMovementSpeed => self.movement_speed,
            ClientExperienceActionKind::SetTouchLookSensitivity => self.touch_look,
            ClientExperienceActionKind::SetTouchControlsMode => self.touch_controls,
            ClientExperienceActionKind::SetServerSimulationCadence => {
                self.server_simulation_cadence
            }
            _ => ClientExperienceCapabilityStatus::Supported,
        }
    }

    /// The feature axis: engine features whose *availability* must be uniform
    /// across every native target (desktop flat, XR, Android flat, Android XR).
    /// A native target may only diverge here with an explicit, reason-bearing
    /// [`ClientExperienceCapabilityStatus::Unsupported`] that is tracked in
    /// [`NATIVE_FEATURE_PARITY_EXCEPTIONS`]; a bare
    /// [`ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED`] (silent drift)
    /// is forbidden. Input/surface-shaped capabilities (turn, touch, crosshair,
    /// frame pacing, fps cap) are intentionally excluded — those legitimately
    /// differ by hardware and may vary per platform.
    pub fn feature_axis(
        self,
    ) -> [(
        ClientExperienceFeatureCapability,
        ClientExperienceCapabilityStatus,
    ); 15] {
        use ClientExperienceFeatureCapability as F;
        [
            (F::SectionOcclusion, self.section_occlusion),
            (F::Fullbright, self.fullbright),
            (F::FarLod, self.far_lod),
            (F::PlayerCollisionBox, self.player_collision_box),
            (F::FirstPersonPlayer, self.first_person_player),
            (F::FramePipelineOverlay, self.frame_pipeline_overlay),
            (F::DebugDiagnostics, self.debug_diagnostics),
            (F::PlayerModel, self.player_model),
            (F::MovementMode, self.movement_mode),
            (F::CollisionMode, self.collision_mode),
            (F::TravelAssist, self.travel_assist),
            (F::RenderDistance, self.render_distance),
            (F::FlySpeed, self.fly_speed),
            (F::MovementSpeed, self.movement_speed),
            (F::ServerSimulationCadence, self.server_simulation_cadence),
        ]
    }
}

/// Engine features that must share the same availability across all native
/// targets. See [`ClientExperienceSettingsProfile::feature_axis`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientExperienceFeatureCapability {
    SectionOcclusion,
    Fullbright,
    FarLod,
    PlayerCollisionBox,
    FirstPersonPlayer,
    FramePipelineOverlay,
    DebugDiagnostics,
    PlayerModel,
    MovementMode,
    CollisionMode,
    TravelAssist,
    RenderDistance,
    FlySpeed,
    MovementSpeed,
    ServerSimulationCadence,
}

/// Native client-experience targets. A single XR profile serves both desktop XR
/// and Android XR, so three profiles cover the four native configurations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlatform {
    DesktopFlat,
    Xr,
    AndroidFlat,
}

/// Canonical native feature baseline: every engine feature available. Native
/// profiles derive from this and may only subtract input/surface capabilities,
/// or record a temporary, reason-bearing feature exception (tracked in
/// [`NATIVE_FEATURE_PARITY_EXCEPTIONS`]). This is the single owner that keeps a
/// feature from being silently `Unsupported` on one native target.
pub fn native_client_experience_baseline() -> ClientExperienceSettingsProfile {
    ClientExperienceSettingsProfile::all_supported()
}

/// Desktop flat profile: full native baseline minus touch/turn input, which are
/// hardware-shaped (input/surface axis, free to differ).
pub fn desktop_native_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = native_client_experience_baseline();
    settings.turn_mode = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.xr_turn = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_look = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_controls = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    ClientExperienceProfile::new(settings)
}

/// XR profile (desktop XR and Android XR): full native baseline minus
/// surface/input capabilities that XR owns differently (world-space reticle,
/// compositor-owned pacing, controller turn).
pub fn xr_native_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = native_client_experience_baseline();
    settings.crosshair = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.frame_pacing = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.fps_cap = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_look = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_controls = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    ClientExperienceProfile::new(settings)
}

/// Android flat profile: full native baseline minus surface capabilities that
/// remain fixed by the Android presenter.
pub fn android_flat_native_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = native_client_experience_baseline();
    settings.turn_mode =
        ClientExperienceCapabilityStatus::Unsupported("Turn mode is unavailable on flat Android");
    settings.xr_turn = ClientExperienceCapabilityStatus::Unsupported(
        "XR turn mode is unavailable on flat Android",
    );
    settings.frame_pacing = ClientExperienceCapabilityStatus::Unsupported(
        "Frame pacing is fixed by Android surface presentation",
    );
    settings.fps_cap =
        ClientExperienceCapabilityStatus::Unsupported("FPS cap is fixed on flat Android");
    ClientExperienceProfile::new(settings)
}

/// Web profile: the one non-native target, whose threading/runtime model
/// legitimately forces feature divergence. Web is exempt from native feature
/// parity, but every divergence must still be reason-bearing (never a silent
/// `PROFILE_UNSUPPORTED` on the feature axis), enforced by
/// `web_feature_divergences_match_audited_ledger`.
pub fn web_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = native_client_experience_baseline();
    settings.far_lod = ClientExperienceCapabilityStatus::Unsupported(WEB_FAR_LOD_REASON);
    settings.travel_assist =
        ClientExperienceCapabilityStatus::Unsupported(WEB_TRAVEL_ASSIST_REASON);
    settings.turn_mode = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.xr_turn = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.frame_pacing = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.fps_cap = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.frame_pipeline_overlay =
        ClientExperienceCapabilityStatus::Unsupported(WEB_FRAME_PIPELINE_OVERLAY_REASON);
    settings.debug_diagnostics =
        ClientExperienceCapabilityStatus::Unsupported(WEB_DEBUG_DIAGNOSTICS_REASON);
    settings.server_simulation_cadence =
        ClientExperienceCapabilityStatus::Unsupported(WEB_SERVER_SIMULATION_CADENCE_REASON);
    ClientExperienceProfile::new(settings)
}

const WEB_FAR_LOD_REASON: &str =
    "Far LOD needs the shared resident-tile substrate plus browser performance and rendering proof";
const WEB_TRAVEL_ASSIST_REASON: &str =
    "Travel assist needs browser input, UI, and movement smoke coverage";
const WEB_FRAME_PIPELINE_OVERLAY_REASON: &str =
    "Frame pipeline overlay needs browser frame-accounting reports and UI proof";
const WEB_DEBUG_DIAGNOSTICS_REASON: &str =
    "Debug diagnostics needs browser presenter wiring and panel proof";
const WEB_SERVER_SIMULATION_CADENCE_REASON: &str =
    "Server simulation cadence needs browser runtime control and diagnostic proof";

/// Audited browser feature gaps. Each entry names a concrete follow-up and must
/// exactly match a reason-bearing capability in [`web_client_experience_profile`].
pub const WEB_FEATURE_PARITY_EXCEPTIONS: &[(ClientExperienceFeatureCapability, &'static str)] = &[
    (
        ClientExperienceFeatureCapability::FarLod,
        WEB_FAR_LOD_REASON,
    ),
    (
        ClientExperienceFeatureCapability::TravelAssist,
        WEB_TRAVEL_ASSIST_REASON,
    ),
    (
        ClientExperienceFeatureCapability::FramePipelineOverlay,
        WEB_FRAME_PIPELINE_OVERLAY_REASON,
    ),
    (
        ClientExperienceFeatureCapability::DebugDiagnostics,
        WEB_DEBUG_DIAGNOSTICS_REASON,
    ),
    (
        ClientExperienceFeatureCapability::ServerSimulationCadence,
        WEB_SERVER_SIMULATION_CADENCE_REASON,
    ),
];

/// Temporary, explicitly-tracked feature-axis divergences on native targets.
/// Every entry is a genuine runtime plumbing gap to burn down toward the native
/// baseline — NOT a permanent capability difference. Adding an entry is a
/// deliberate, reviewed act; a native feature divergence that is NOT listed here
/// (or any silent `PROFILE_UNSUPPORTED` on the feature axis) fails
/// `native_targets_share_feature_capability_availability`. Remove the entry when
/// the feature is wired and becomes `Supported`.
pub const NATIVE_FEATURE_PARITY_EXCEPTIONS: &[(
    NativePlatform,
    ClientExperienceFeatureCapability,
)] = &[];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientExperienceCapabilityStatus {
    Supported,
    Unsupported(&'static str),
    Pending(&'static str),
    Disabled(&'static str),
}

impl Default for ClientExperienceCapabilityStatus {
    fn default() -> Self {
        Self::Supported
    }
}

impl ClientExperienceCapabilityStatus {
    pub const PROFILE_UNSUPPORTED: Self =
        Self::Unsupported("This action is unavailable for this profile");

    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    pub const fn message(self) -> Option<&'static str> {
        match self {
            Self::Supported => None,
            Self::Unsupported(message) | Self::Pending(message) | Self::Disabled(message) => {
                Some(message)
            }
        }
    }

    pub const fn ok(self) -> bool {
        matches!(self, Self::Supported)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientExperienceCapabilityProjection {
    pub actions: Vec<ClientExperienceActionAvailability>,
}

impl ClientExperienceCapabilityProjection {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn first_unavailable(&self) -> Option<ClientExperienceActionAvailability> {
        self.actions.first().copied()
    }

    fn push(&mut self, kind: ClientExperienceActionKind, status: ClientExperienceCapabilityStatus) {
        if !status.is_supported() {
            self.actions
                .push(ClientExperienceActionAvailability { kind, status });
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientExperienceActionAvailability {
    pub kind: ClientExperienceActionKind,
    pub status: ClientExperienceCapabilityStatus,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientExperienceSettingsController {
    state: ClientExperienceSettingsState,
}

impl ClientExperienceSettingsController {
    pub fn new(state: ClientExperienceSettingsState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> ClientExperienceSettingsState {
        self.state
    }

    pub fn set_state(&mut self, state: ClientExperienceSettingsState) {
        self.state = state;
    }

    fn apply_movement_experience_change(
        &mut self,
        change: ClientExperienceMovementSettingChange,
        effects: &mut ClientExperienceSettingsEffects,
    ) {
        let before = self.state;
        self.state.apply_movement_experience_change(change);
        effects.push_movement_experience_diff(before, self.state);
    }

    pub fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        profile: ClientExperienceSettingsProfile,
    ) -> ClientExperienceSettingsEffects {
        let kind = client_experience_action_kind(action);
        let mut effects = ClientExperienceSettingsEffects::default();
        if !self.require_capability(kind, profile, &mut effects) {
            return effects;
        }

        match action {
            GameUiAction::ToggleSectionOcclusion => {
                self.state.section_occlusion_culling = !self.state.section_occlusion_culling;
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetSectionOcclusionCulling(
                        self.state.section_occlusion_culling,
                    ),
                );
            }
            GameUiAction::ToggleFullbright => {
                self.state.force_fullbright = !self.state.force_fullbright;
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetFullbright(
                        self.state.force_fullbright,
                    ));
            }
            GameUiAction::ToggleFarLod => {
                self.state.far_lod_enabled = !self.state.far_lod_enabled;
                effects.push_far_lod(self.state.far_lod_enabled, self.state.far_lod_range_chunks);
            }
            GameUiAction::SetFarLodRange(range_chunks) => {
                self.state.far_lod_range_chunks = self.state.clamp_far_lod_range(range_chunks);
                effects.push_far_lod(self.state.far_lod_enabled, self.state.far_lod_range_chunks);
            }
            GameUiAction::TogglePlayerCollisionBox => {
                self.state.player_collision_box_visible = !self.state.player_collision_box_visible;
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetPlayerCollisionBoxVisible(
                        self.state.player_collision_box_visible,
                    ),
                );
            }
            GameUiAction::ToggleFirstPersonPlayer => {
                self.state.first_person_player_visible = !self.state.first_person_player_visible;
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetFirstPersonPlayerVisible(
                        self.state.first_person_player_visible,
                    ),
                );
            }
            GameUiAction::ToggleCrosshair => {
                let Some(visible) = self.state.crosshair_visible.as_mut() else {
                    effects.capability_projection.push(
                        kind,
                        ClientExperienceCapabilityStatus::Unsupported(
                            "Crosshair visibility is unavailable for this profile",
                        ),
                    );
                    return effects;
                };
                *visible = !*visible;
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetCrosshairVisible(*visible));
            }
            GameUiAction::ToggleFramePipelineOverlay => {
                self.state.frame_pipeline_overlay_visible =
                    !self.state.frame_pipeline_overlay_visible;
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetFramePipelineOverlayVisible(
                        self.state.frame_pipeline_overlay_visible,
                    ),
                );
            }
            GameUiAction::ToggleDebugDiagnostics => {
                self.state.debug_diagnostics_visible = !self.state.debug_diagnostics_visible;
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetDebugDiagnosticsVisible(
                        self.state.debug_diagnostics_visible,
                    ),
                );
            }
            GameUiAction::SetPlayerModel(model) => {
                self.state.player_model = model;
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetPlayerModel(model));
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SyncPlayerAppearance);
            }
            GameUiAction::SetMovementMode(mode) => {
                self.apply_movement_experience_change(
                    ClientExperienceMovementSettingChange::MovementMode(mode),
                    &mut effects,
                );
            }
            GameUiAction::SetCollisionMode(mode) => {
                self.apply_movement_experience_change(
                    ClientExperienceMovementSettingChange::CollisionMode(mode),
                    &mut effects,
                );
            }
            GameUiAction::SetTravelAssistMode(mode) => {
                self.apply_movement_experience_change(
                    ClientExperienceMovementSettingChange::TravelAssistMode(mode),
                    &mut effects,
                );
            }
            GameUiAction::SetTurnMode(mode) => {
                if self.state.turn_mode.is_none() {
                    effects.capability_projection.push(
                        kind,
                        ClientExperienceCapabilityStatus::Unsupported(
                            "Turn mode is unavailable for this profile",
                        ),
                    );
                    return effects;
                }
                self.apply_movement_experience_change(
                    ClientExperienceMovementSettingChange::TurnMode(Some(mode)),
                    &mut effects,
                );
            }
            GameUiAction::SetXrTurnMode(mode) => {
                let Some(turn_mode) = self.state.xr_turn_mode.as_mut() else {
                    effects.capability_projection.push(
                        kind,
                        ClientExperienceCapabilityStatus::Unsupported(
                            "XR turn mode is unavailable for this profile",
                        ),
                    );
                    return effects;
                };
                *turn_mode = mode;
                self.state.turn_mode = Some(GameTurnMode::from(mode));
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetXrTurnMode(mode));
            }
            GameUiAction::CycleFramePacing => {
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::CycleFramePacing);
            }
            GameUiAction::CycleFpsCap => {
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::CycleFpsCap);
            }
            GameUiAction::SetRenderDistance(render_distance) => {
                self.state.render_distance = self.state.clamp_render_distance(render_distance);
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetRenderDistance(
                        self.state.render_distance.max(0) as u32,
                    ));
            }
            GameUiAction::SetFlySpeed(multiplier) => {
                self.state.fly_speed_multiplier = self.state.clamp_fly_speed(multiplier);
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetFlySpeedMultiplier(
                        self.state.fly_speed_multiplier,
                    ));
            }
            GameUiAction::SetMovementSpeed(multiplier) => {
                self.state.movement_speed_multiplier = self.state.clamp_movement_speed(multiplier);
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetMovementSpeedMultiplier(
                        self.state.movement_speed_multiplier,
                    ),
                );
            }
            GameUiAction::SetTouchLookSensitivity(look_sensitivity) => {
                let Some(mut settings) = self.state.touch_settings else {
                    effects.capability_projection.push(
                        kind,
                        ClientExperienceCapabilityStatus::Unsupported(
                            "Touch look sensitivity is unavailable for this profile",
                        ),
                    );
                    return effects;
                };
                settings.look_sensitivity = clamp_f32(
                    look_sensitivity,
                    settings.look_sensitivity_limits().0,
                    settings.look_sensitivity_limits().1,
                );
                self.state.touch_settings = Some(settings);
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetTouchLookSensitivity(
                        settings.look_sensitivity,
                    ),
                );
            }
            GameUiAction::SetTouchControlsMode(mode) => {
                if self.state.touch_controls_mode.is_none() {
                    effects.capability_projection.push(
                        kind,
                        ClientExperienceCapabilityStatus::Unsupported(
                            "Touch controls are unavailable for this profile",
                        ),
                    );
                    return effects;
                }
                self.state.touch_controls_mode = Some(mode);
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetTouchControlsMode(mode));
            }
            GameUiAction::SetServerSimulationCadence(cadence) => {
                if !cadence.is_valid() {
                    effects
                        .rejections
                        .push(ClientExperienceActionRejection::invalid(
                            kind,
                            "Server simulation cadence is invalid",
                        ));
                    return effects;
                }
                self.state.server_cadence = Some(cadence);
                effects.setting_effects.push(
                    ClientExperienceSettingEffect::SetServerSimulationCadence(cadence),
                );
            }
            _ => {}
        }

        effects
    }

    pub fn capability_projection(
        &self,
        profile: ClientExperienceSettingsProfile,
    ) -> ClientExperienceCapabilityProjection {
        let mut projection = ClientExperienceCapabilityProjection::default();
        for (kind, status) in [
            (
                ClientExperienceActionKind::ToggleSectionOcclusion,
                profile.section_occlusion,
            ),
            (
                ClientExperienceActionKind::ToggleFullbright,
                profile.fullbright,
            ),
            (ClientExperienceActionKind::ToggleFarLod, profile.far_lod),
            (ClientExperienceActionKind::SetFarLodRange, profile.far_lod),
            (
                ClientExperienceActionKind::TogglePlayerCollisionBox,
                profile.player_collision_box,
            ),
            (
                ClientExperienceActionKind::ToggleFirstPersonPlayer,
                profile.first_person_player,
            ),
            (
                ClientExperienceActionKind::ToggleCrosshair,
                profile.crosshair,
            ),
            (
                ClientExperienceActionKind::ToggleFramePipelineOverlay,
                profile.frame_pipeline_overlay,
            ),
            (
                ClientExperienceActionKind::ToggleDebugDiagnostics,
                profile.debug_diagnostics,
            ),
            (
                ClientExperienceActionKind::SetPlayerModel,
                profile.player_model,
            ),
            (
                ClientExperienceActionKind::SetMovementMode,
                profile.movement_mode,
            ),
            (
                ClientExperienceActionKind::SetCollisionMode,
                profile.collision_mode,
            ),
            (
                ClientExperienceActionKind::SetTravelAssistMode,
                profile.travel_assist,
            ),
            (ClientExperienceActionKind::SetTurnMode, profile.turn_mode),
            (ClientExperienceActionKind::SetXrTurnMode, profile.xr_turn),
            (
                ClientExperienceActionKind::CycleFramePacing,
                profile.frame_pacing,
            ),
            (ClientExperienceActionKind::CycleFpsCap, profile.fps_cap),
            (
                ClientExperienceActionKind::SetRenderDistance,
                profile.render_distance,
            ),
            (ClientExperienceActionKind::SetFlySpeed, profile.fly_speed),
            (
                ClientExperienceActionKind::SetMovementSpeed,
                profile.movement_speed,
            ),
            (
                ClientExperienceActionKind::SetTouchLookSensitivity,
                profile.touch_look,
            ),
            (
                ClientExperienceActionKind::SetTouchControlsMode,
                profile.touch_controls,
            ),
            (
                ClientExperienceActionKind::SetServerSimulationCadence,
                profile.server_simulation_cadence,
            ),
        ] {
            projection.push(kind, status);
        }

        if profile.crosshair.is_supported() && self.state.crosshair_visible.is_none() {
            projection.push(
                ClientExperienceActionKind::ToggleCrosshair,
                ClientExperienceCapabilityStatus::Unsupported(
                    "Crosshair visibility is unavailable for this profile",
                ),
            );
        }
        if profile.xr_turn.is_supported() && self.state.xr_turn_mode.is_none() {
            projection.push(
                ClientExperienceActionKind::SetXrTurnMode,
                ClientExperienceCapabilityStatus::Unsupported(
                    "XR turn mode is unavailable for this profile",
                ),
            );
        }
        if profile.turn_mode.is_supported() && self.state.turn_mode.is_none() {
            projection.push(
                ClientExperienceActionKind::SetTurnMode,
                ClientExperienceCapabilityStatus::Unsupported(
                    "Turn mode is unavailable for this profile",
                ),
            );
        }
        if profile.touch_look.is_supported() && self.state.touch_settings.is_none() {
            projection.push(
                ClientExperienceActionKind::SetTouchLookSensitivity,
                ClientExperienceCapabilityStatus::Unsupported(
                    "Touch look sensitivity is unavailable for this profile",
                ),
            );
        }
        if profile.touch_controls.is_supported() && self.state.touch_controls_mode.is_none() {
            projection.push(
                ClientExperienceActionKind::SetTouchControlsMode,
                ClientExperienceCapabilityStatus::Unsupported(
                    "Touch controls are unavailable for this profile",
                ),
            );
        }
        if profile.server_simulation_cadence.is_supported() && self.state.server_cadence.is_none() {
            projection.push(
                ClientExperienceActionKind::SetServerSimulationCadence,
                ClientExperienceCapabilityStatus::Unsupported(
                    "Server simulation cadence is unavailable for this profile",
                ),
            );
        }

        projection
    }

    fn require_capability(
        &self,
        kind: ClientExperienceActionKind,
        profile: ClientExperienceSettingsProfile,
        effects: &mut ClientExperienceSettingsEffects,
    ) -> bool {
        let status = profile.capability_for_action(kind);
        if status.is_supported() {
            true
        } else {
            effects.capability_projection.push(kind, status);
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientExperienceSettingsState {
    pub render_distance: i32,
    pub min_render_distance: i32,
    pub max_render_distance: i32,
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub far_lod_enabled: bool,
    pub far_lod_range_chunks: i32,
    pub min_far_lod_range_chunks: i32,
    pub max_far_lod_range_chunks: i32,
    pub player_collision_box_visible: bool,
    pub first_person_player_visible: bool,
    pub crosshair_visible: Option<bool>,
    pub frame_pipeline_overlay_visible: bool,
    pub debug_diagnostics_visible: bool,
    pub player_model: GamePlayerModel,
    pub movement_mode: GameMovementMode,
    pub collision_mode: GameCollisionMode,
    pub travel_assist_mode: GameTravelAssistMode,
    pub turn_mode: Option<GameTurnMode>,
    pub xr_turn_mode: Option<GameXrTurnMode>,
    pub fly_speed_multiplier: f32,
    pub min_fly_speed_multiplier: f32,
    pub max_fly_speed_multiplier: f32,
    pub movement_speed_multiplier: f32,
    pub min_movement_speed_multiplier: f32,
    pub max_movement_speed_multiplier: f32,
    pub frame_pacing_mode: GameFramePacingMode,
    pub fps_cap: u32,
    pub server_cadence: Option<GameSimulationCadence>,
    pub touch_controls_mode: Option<TouchControlsMode>,
    pub touch_settings: Option<GameTouchSettings>,
}

impl Default for ClientExperienceSettingsState {
    fn default() -> Self {
        Self::from(GameUiRenderState::default())
    }
}

impl From<GameUiRenderState> for ClientExperienceSettingsState {
    fn from(state: GameUiRenderState) -> Self {
        Self {
            render_distance: state.render_distance,
            min_render_distance: state.min_render_distance,
            max_render_distance: state.max_render_distance,
            section_occlusion_culling: state.section_occlusion_culling,
            force_fullbright: state.force_fullbright,
            far_lod_enabled: state.far_lod_enabled,
            far_lod_range_chunks: state.far_lod_range_chunks,
            min_far_lod_range_chunks: state.min_far_lod_range_chunks,
            max_far_lod_range_chunks: state.max_far_lod_range_chunks,
            player_collision_box_visible: state.player_collision_box_visible,
            first_person_player_visible: state.first_person_player_visible,
            crosshair_visible: state.crosshair_visible,
            frame_pipeline_overlay_visible: state.frame_pipeline_overlay_visible,
            debug_diagnostics_visible: state.debug_diagnostics_visible,
            player_model: state.player_model,
            movement_mode: state.movement_mode,
            collision_mode: state
                .collision_mode
                .unwrap_or_else(|| legacy_collision_mode_for_movement(state.movement_mode)),
            travel_assist_mode: state
                .travel_assist_mode
                .unwrap_or(GameTravelAssistMode::Off),
            turn_mode: state
                .turn_mode
                .or_else(|| state.xr_turn_mode.map(GameTurnMode::from)),
            xr_turn_mode: state.xr_turn_mode,
            fly_speed_multiplier: state.fly_speed_multiplier,
            min_fly_speed_multiplier: state.min_fly_speed_multiplier,
            max_fly_speed_multiplier: state.max_fly_speed_multiplier,
            movement_speed_multiplier: state.movement_speed_multiplier,
            min_movement_speed_multiplier: state.min_movement_speed_multiplier,
            max_movement_speed_multiplier: state.max_movement_speed_multiplier,
            frame_pacing_mode: state.frame_pacing_mode,
            fps_cap: state.fps_cap,
            server_cadence: state.server_cadence,
            touch_controls_mode: state.touch_controls_mode,
            touch_settings: state.touch_settings,
        }
    }
}

impl ClientExperienceSettingsState {
    pub fn apply_to_render_state(self, state: &mut GameUiRenderState) {
        state.render_distance = self.render_distance;
        state.min_render_distance = self.min_render_distance;
        state.max_render_distance = self.max_render_distance;
        state.section_occlusion_culling = self.section_occlusion_culling;
        state.force_fullbright = self.force_fullbright;
        state.far_lod_enabled = self.far_lod_enabled;
        state.far_lod_range_chunks = self.far_lod_range_chunks;
        state.min_far_lod_range_chunks = self.min_far_lod_range_chunks;
        state.max_far_lod_range_chunks = self.max_far_lod_range_chunks;
        state.player_collision_box_visible = self.player_collision_box_visible;
        state.first_person_player_visible = self.first_person_player_visible;
        state.crosshair_visible = self.crosshair_visible;
        state.frame_pipeline_overlay_visible = self.frame_pipeline_overlay_visible;
        state.debug_diagnostics_visible = self.debug_diagnostics_visible;
        state.player_model = self.player_model;
        state.movement_mode = self.movement_mode;
        state.collision_mode = Some(self.collision_mode);
        state.travel_assist_mode = Some(self.travel_assist_mode);
        state.turn_mode = self.turn_mode;
        // The current render state still exposes the legacy XR-specific turn
        // slot. Keep it synchronized until the UI row is renamed.
        state.xr_turn_mode = self
            .turn_mode
            .map(GameXrTurnMode::from)
            .or(self.xr_turn_mode);
        state.fly_speed_multiplier = self.fly_speed_multiplier;
        state.min_fly_speed_multiplier = self.min_fly_speed_multiplier;
        state.max_fly_speed_multiplier = self.max_fly_speed_multiplier;
        state.movement_speed_multiplier = self.movement_speed_multiplier;
        state.min_movement_speed_multiplier = self.min_movement_speed_multiplier;
        state.max_movement_speed_multiplier = self.max_movement_speed_multiplier;
        state.frame_pacing_mode = self.frame_pacing_mode;
        state.fps_cap = self.fps_cap;
        state.server_cadence = self.server_cadence;
        state.touch_controls_mode = self.touch_controls_mode;
        state.touch_settings = self.touch_settings;
    }

    fn clamp_render_distance(self, render_distance: i32) -> i32 {
        let min = self.min_render_distance.min(self.max_render_distance);
        let max = self.min_render_distance.max(self.max_render_distance);
        render_distance.clamp(min, max)
    }

    fn clamp_far_lod_range(self, range_chunks: i32) -> i32 {
        let min = self
            .min_far_lod_range_chunks
            .min(self.max_far_lod_range_chunks);
        let max = self
            .min_far_lod_range_chunks
            .max(self.max_far_lod_range_chunks);
        range_chunks.clamp(min, max)
    }

    fn clamp_fly_speed(self, multiplier: f32) -> f32 {
        clamp_f32(
            multiplier,
            self.min_fly_speed_multiplier,
            self.max_fly_speed_multiplier,
        )
    }

    fn clamp_movement_speed(self, multiplier: f32) -> f32 {
        clamp_f32(
            multiplier,
            self.min_movement_speed_multiplier,
            self.max_movement_speed_multiplier,
        )
    }

    pub fn apply_movement_experience_change(
        &mut self,
        change: ClientExperienceMovementSettingChange,
    ) -> bool {
        let before = self.movement_experience_tuple();
        match change {
            ClientExperienceMovementSettingChange::MovementMode(mode) => {
                self.movement_mode = mode;
                // Thruster (tactical 157) is a flight mode like Fly (no travel
                // assist) and defaults to Normal collision on entry like HandPush
                // — but, unlike HandPush, it is NOT force-normalized below, so
                // NoClip stays selectable afterward.
                if matches!(mode, GameMovementMode::Fly | GameMovementMode::Thruster) {
                    self.travel_assist_mode = GameTravelAssistMode::Off;
                }
                if matches!(
                    mode,
                    GameMovementMode::HandPush | GameMovementMode::Thruster
                ) {
                    self.collision_mode = GameCollisionMode::Normal;
                }
            }
            ClientExperienceMovementSettingChange::CollisionMode(mode) => {
                self.collision_mode = mode;
                if mode == GameCollisionMode::NoClip {
                    self.travel_assist_mode = GameTravelAssistMode::Off;
                }
            }
            ClientExperienceMovementSettingChange::TravelAssistMode(mode) => {
                if self.movement_mode == GameMovementMode::Fly {
                    self.travel_assist_mode = GameTravelAssistMode::Off;
                } else {
                    self.travel_assist_mode = mode;
                    if mode.is_enabled() {
                        self.collision_mode = GameCollisionMode::Normal;
                    }
                }
            }
            ClientExperienceMovementSettingChange::TurnMode(mode) => {
                self.turn_mode = mode;
                self.xr_turn_mode = mode.map(GameXrTurnMode::from);
            }
        }
        self.normalize_movement_experience();
        self.movement_experience_tuple() != before
    }

    pub fn normalize_movement_experience(&mut self) -> bool {
        let before = self.movement_experience_tuple();
        if matches!(
            self.movement_mode,
            GameMovementMode::Fly | GameMovementMode::Thruster
        ) {
            self.travel_assist_mode = GameTravelAssistMode::Off;
        }
        // HandPush is always collision-backed; Thruster is intentionally NOT
        // forced here so its NoClip default-on-entry can be overridden.
        if self.movement_mode == GameMovementMode::HandPush {
            self.collision_mode = GameCollisionMode::Normal;
        }
        if self.collision_mode == GameCollisionMode::NoClip {
            self.travel_assist_mode = GameTravelAssistMode::Off;
        }
        if self.travel_assist_mode.is_enabled() {
            self.collision_mode = GameCollisionMode::Normal;
        }
        self.movement_experience_tuple() != before
    }

    fn movement_experience_tuple(
        self,
    ) -> (
        GameMovementMode,
        GameCollisionMode,
        GameTravelAssistMode,
        Option<GameTurnMode>,
    ) {
        (
            self.movement_mode,
            self.collision_mode,
            self.travel_assist_mode,
            self.turn_mode,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientExperienceMovementSettingChange {
    MovementMode(GameMovementMode),
    CollisionMode(GameCollisionMode),
    TravelAssistMode(GameTravelAssistMode),
    TurnMode(Option<GameTurnMode>),
}

const fn legacy_collision_mode_for_movement(mode: GameMovementMode) -> GameCollisionMode {
    match mode {
        GameMovementMode::Fly => GameCollisionMode::NoClip,
        GameMovementMode::Walk | GameMovementMode::HandPush | GameMovementMode::Thruster => {
            GameCollisionMode::Normal
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientExperienceSettingsEffects {
    pub setting_effects: Vec<ClientExperienceSettingEffect>,
    pub capability_projection: ClientExperienceCapabilityProjection,
    pub rejections: Vec<ClientExperienceActionRejection>,
}

impl ClientExperienceSettingsEffects {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn push_movement_experience_diff(
        &mut self,
        before: ClientExperienceSettingsState,
        after: ClientExperienceSettingsState,
    ) {
        if before.movement_mode != after.movement_mode {
            self.setting_effects
                .push(ClientExperienceSettingEffect::SetMovementMode(
                    after.movement_mode,
                ));
        }
        if before.collision_mode != after.collision_mode {
            self.setting_effects
                .push(ClientExperienceSettingEffect::SetCollisionMode(
                    after.collision_mode,
                ));
        }
        if before.travel_assist_mode != after.travel_assist_mode {
            self.setting_effects
                .push(ClientExperienceSettingEffect::SetTravelAssistMode(
                    after.travel_assist_mode,
                ));
        }
        if before.turn_mode != after.turn_mode {
            if let Some(turn_mode) = after.turn_mode {
                self.setting_effects
                    .push(ClientExperienceSettingEffect::SetTurnMode(turn_mode));
            }
        }
    }

    fn push_far_lod(&mut self, enabled: bool, range_chunks: i32) {
        let extra_radius_chunks = u32::try_from(range_chunks)
            .unwrap_or(MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS)
            .clamp(
                MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            );
        self.setting_effects
            .push(ClientExperienceSettingEffect::SetFarLod {
                enabled,
                extra_radius_chunks,
            });
        self.setting_effects
            .push(ClientExperienceSettingEffect::ClearFarLod);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientExperienceSettingEffect {
    SetSectionOcclusionCulling(bool),
    SetFullbright(bool),
    SetFarLod {
        enabled: bool,
        extra_radius_chunks: u32,
    },
    ClearFarLod,
    SetPlayerCollisionBoxVisible(bool),
    SetFirstPersonPlayerVisible(bool),
    SetCrosshairVisible(bool),
    SetFramePipelineOverlayVisible(bool),
    SetDebugDiagnosticsVisible(bool),
    SetPlayerModel(GamePlayerModel),
    SyncPlayerAppearance,
    SetMovementMode(GameMovementMode),
    SetCollisionMode(GameCollisionMode),
    SetTravelAssistMode(GameTravelAssistMode),
    SetTurnMode(GameTurnMode),
    SetXrTurnMode(GameXrTurnMode),
    CycleFramePacing,
    CycleFpsCap,
    SetRenderDistance(u32),
    SetFlySpeedMultiplier(f32),
    SetMovementSpeedMultiplier(f32),
    SetTouchLookSensitivity(f32),
    SetTouchControlsMode(TouchControlsMode),
    SetServerSimulationCadence(GameSimulationCadence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientExperienceActionRejection {
    pub kind: ClientExperienceActionKind,
    pub message: &'static str,
}

impl ClientExperienceActionRejection {
    pub const fn invalid(kind: ClientExperienceActionKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientExperienceActionClassification {
    CoreAction,
    HostEffectAction,
    CapabilityGated,
    ProjectionSpecific,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientExperienceActionKind {
    StartWorld,
    OpenWorldList,
    OpenWorldCreate,
    SelectWorld,
    OpenWorld,
    CreateCatalogWorld,
    ConfirmDeleteWorld,
    DeleteWorld,
    CancelDeleteWorld,
    OpenNewWorld,
    OpenJoinRemote,
    RerollSeed,
    CreateWorld,
    JoinRemote,
    Resume,
    OpenBlockPalette,
    OpenHelp,
    CloseHelp,
    AssignHotbarBlock,
    OpenOptions,
    OpenOptionsCategory,
    OpenServerSettings,
    OpenAssetPacks,
    ToggleAssetPack,
    ApplyAssetPacks,
    CancelAssetPacks,
    BackToTitle,
    BackToPause,
    QuitToTitle,
    ToggleSectionOcclusion,
    ToggleFullbright,
    ToggleFarLod,
    SetFarLodRange,
    TogglePlayerCollisionBox,
    ToggleFirstPersonPlayer,
    ToggleCrosshair,
    ToggleFramePipelineOverlay,
    ToggleDebugDiagnostics,
    SetPlayerModel,
    SetMovementMode,
    SetCollisionMode,
    SetTravelAssistMode,
    SetTurnMode,
    SetXrTurnMode,
    CycleFramePacing,
    CycleFpsCap,
    SetRenderDistance,
    SetFlySpeed,
    SetMovementSpeed,
    SetTouchLookSensitivity,
    SetTouchControlsMode,
    SetServerSimulationCadence,
    Quit,
}

pub fn client_experience_action_kind(action: GameUiAction) -> ClientExperienceActionKind {
    match action {
        GameUiAction::StartWorld => ClientExperienceActionKind::StartWorld,
        GameUiAction::OpenWorldList => ClientExperienceActionKind::OpenWorldList,
        GameUiAction::OpenWorldCreate => ClientExperienceActionKind::OpenWorldCreate,
        GameUiAction::SelectWorld(_) => ClientExperienceActionKind::SelectWorld,
        GameUiAction::OpenWorld(_) => ClientExperienceActionKind::OpenWorld,
        GameUiAction::CreateCatalogWorld => ClientExperienceActionKind::CreateCatalogWorld,
        GameUiAction::ConfirmDeleteWorld(_) => ClientExperienceActionKind::ConfirmDeleteWorld,
        GameUiAction::DeleteWorld(_) => ClientExperienceActionKind::DeleteWorld,
        GameUiAction::CancelDeleteWorld => ClientExperienceActionKind::CancelDeleteWorld,
        GameUiAction::OpenNewWorld => ClientExperienceActionKind::OpenNewWorld,
        GameUiAction::OpenJoinRemote => ClientExperienceActionKind::OpenJoinRemote,
        GameUiAction::RerollSeed => ClientExperienceActionKind::RerollSeed,
        GameUiAction::CreateWorld(_) => ClientExperienceActionKind::CreateWorld,
        GameUiAction::JoinRemote => ClientExperienceActionKind::JoinRemote,
        GameUiAction::Resume => ClientExperienceActionKind::Resume,
        GameUiAction::OpenBlockPalette => ClientExperienceActionKind::OpenBlockPalette,
        GameUiAction::OpenHelp(_) => ClientExperienceActionKind::OpenHelp,
        GameUiAction::CloseHelp(_) => ClientExperienceActionKind::CloseHelp,
        GameUiAction::AssignHotbarBlock { .. } => ClientExperienceActionKind::AssignHotbarBlock,
        GameUiAction::OpenOptions(_) => ClientExperienceActionKind::OpenOptions,
        GameUiAction::OpenOptionsCategory(_, _) => ClientExperienceActionKind::OpenOptionsCategory,
        GameUiAction::OpenServerSettings(_) => ClientExperienceActionKind::OpenServerSettings,
        GameUiAction::OpenAssetPacks(_) => ClientExperienceActionKind::OpenAssetPacks,
        GameUiAction::ToggleAssetPack(_) => ClientExperienceActionKind::ToggleAssetPack,
        GameUiAction::ApplyAssetPacks => ClientExperienceActionKind::ApplyAssetPacks,
        GameUiAction::CancelAssetPacks => ClientExperienceActionKind::CancelAssetPacks,
        GameUiAction::BackToTitle => ClientExperienceActionKind::BackToTitle,
        GameUiAction::BackToPause => ClientExperienceActionKind::BackToPause,
        GameUiAction::QuitToTitle => ClientExperienceActionKind::QuitToTitle,
        GameUiAction::ToggleSectionOcclusion => ClientExperienceActionKind::ToggleSectionOcclusion,
        GameUiAction::ToggleFullbright => ClientExperienceActionKind::ToggleFullbright,
        GameUiAction::ToggleFarLod => ClientExperienceActionKind::ToggleFarLod,
        GameUiAction::SetFarLodRange(_) => ClientExperienceActionKind::SetFarLodRange,
        GameUiAction::TogglePlayerCollisionBox => {
            ClientExperienceActionKind::TogglePlayerCollisionBox
        }
        GameUiAction::ToggleFirstPersonPlayer => {
            ClientExperienceActionKind::ToggleFirstPersonPlayer
        }
        GameUiAction::ToggleCrosshair => ClientExperienceActionKind::ToggleCrosshair,
        GameUiAction::ToggleFramePipelineOverlay => {
            ClientExperienceActionKind::ToggleFramePipelineOverlay
        }
        GameUiAction::ToggleDebugDiagnostics => ClientExperienceActionKind::ToggleDebugDiagnostics,
        GameUiAction::SetPlayerModel(_) => ClientExperienceActionKind::SetPlayerModel,
        GameUiAction::SetMovementMode(_) => ClientExperienceActionKind::SetMovementMode,
        GameUiAction::SetCollisionMode(_) => ClientExperienceActionKind::SetCollisionMode,
        GameUiAction::SetTravelAssistMode(_) => ClientExperienceActionKind::SetTravelAssistMode,
        GameUiAction::SetTurnMode(_) => ClientExperienceActionKind::SetTurnMode,
        GameUiAction::SetXrTurnMode(_) => ClientExperienceActionKind::SetXrTurnMode,
        GameUiAction::CycleFramePacing => ClientExperienceActionKind::CycleFramePacing,
        GameUiAction::CycleFpsCap => ClientExperienceActionKind::CycleFpsCap,
        GameUiAction::SetRenderDistance(_) => ClientExperienceActionKind::SetRenderDistance,
        GameUiAction::SetFlySpeed(_) => ClientExperienceActionKind::SetFlySpeed,
        GameUiAction::SetMovementSpeed(_) => ClientExperienceActionKind::SetMovementSpeed,
        GameUiAction::SetTouchLookSensitivity(_) => {
            ClientExperienceActionKind::SetTouchLookSensitivity
        }
        GameUiAction::SetTouchControlsMode(_) => ClientExperienceActionKind::SetTouchControlsMode,
        GameUiAction::SetServerSimulationCadence(_) => {
            ClientExperienceActionKind::SetServerSimulationCadence
        }
        GameUiAction::Quit => ClientExperienceActionKind::Quit,
    }
}

pub const fn classify_client_experience_action_kind(
    kind: ClientExperienceActionKind,
) -> ClientExperienceActionClassification {
    match kind {
        ClientExperienceActionKind::OpenWorldList
        | ClientExperienceActionKind::OpenWorldCreate
        | ClientExperienceActionKind::SelectWorld
        | ClientExperienceActionKind::OpenWorld
        | ClientExperienceActionKind::CreateCatalogWorld
        | ClientExperienceActionKind::ConfirmDeleteWorld
        | ClientExperienceActionKind::DeleteWorld
        | ClientExperienceActionKind::CancelDeleteWorld
        | ClientExperienceActionKind::OpenAssetPacks
        | ClientExperienceActionKind::ToggleAssetPack
        | ClientExperienceActionKind::ApplyAssetPacks
        | ClientExperienceActionKind::CancelAssetPacks
        | ClientExperienceActionKind::OpenNewWorld
        | ClientExperienceActionKind::OpenJoinRemote
        | ClientExperienceActionKind::RerollSeed
        | ClientExperienceActionKind::CreateWorld
        | ClientExperienceActionKind::JoinRemote
        | ClientExperienceActionKind::AssignHotbarBlock
        | ClientExperienceActionKind::BackToTitle
        | ClientExperienceActionKind::QuitToTitle
        | ClientExperienceActionKind::ToggleSectionOcclusion
        | ClientExperienceActionKind::ToggleFullbright
        | ClientExperienceActionKind::TogglePlayerCollisionBox
        | ClientExperienceActionKind::ToggleFirstPersonPlayer
        | ClientExperienceActionKind::SetPlayerModel
        | ClientExperienceActionKind::SetMovementMode
        | ClientExperienceActionKind::SetCollisionMode
        | ClientExperienceActionKind::SetTravelAssistMode
        | ClientExperienceActionKind::SetFlySpeed
        | ClientExperienceActionKind::SetMovementSpeed => {
            ClientExperienceActionClassification::CoreAction
        }
        ClientExperienceActionKind::ToggleFarLod
        | ClientExperienceActionKind::SetFarLodRange
        | ClientExperienceActionKind::ToggleCrosshair
        | ClientExperienceActionKind::ToggleFramePipelineOverlay
        | ClientExperienceActionKind::ToggleDebugDiagnostics
        | ClientExperienceActionKind::SetTurnMode
        | ClientExperienceActionKind::SetXrTurnMode
        | ClientExperienceActionKind::CycleFramePacing
        | ClientExperienceActionKind::CycleFpsCap
        | ClientExperienceActionKind::SetRenderDistance
        | ClientExperienceActionKind::SetTouchLookSensitivity
        | ClientExperienceActionKind::SetTouchControlsMode
        | ClientExperienceActionKind::SetServerSimulationCadence => {
            ClientExperienceActionClassification::CapabilityGated
        }
        ClientExperienceActionKind::Quit => ClientExperienceActionClassification::HostEffectAction,
        ClientExperienceActionKind::StartWorld
        | ClientExperienceActionKind::Resume
        | ClientExperienceActionKind::OpenBlockPalette
        | ClientExperienceActionKind::OpenHelp
        | ClientExperienceActionKind::CloseHelp
        | ClientExperienceActionKind::OpenOptions
        | ClientExperienceActionKind::OpenOptionsCategory
        | ClientExperienceActionKind::OpenServerSettings
        | ClientExperienceActionKind::BackToPause => {
            ClientExperienceActionClassification::ProjectionSpecific
        }
    }
}

pub fn classify_game_ui_action(action: GameUiAction) -> ClientExperienceActionClassification {
    classify_client_experience_action_kind(client_experience_action_kind(action))
}

pub const fn client_experience_should_apply_ui_projection(action: GameUiAction) -> bool {
    !matches!(
        action,
        GameUiAction::OpenWorld(_)
            | GameUiAction::CreateCatalogWorld
            | GameUiAction::CreateWorld(_)
            | GameUiAction::JoinRemote
            | GameUiAction::QuitToTitle
            | GameUiAction::Quit
    )
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    let min = finite_or(min, 0.0);
    let max = finite_or(max, min);
    finite_or(value, min).clamp(min.min(max), min.max(max))
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::far_lod::DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS;
    use crate::world_catalog::{WorldCatalogCapabilities, WorldCatalogRequest};
    use mclone_ui::{
        AssetPackUiId, GameHelpParent, GameOptionsCategory, GameOptionsParent,
        WorldCatalogUiWorldId,
    };

    fn context() -> ClientExperienceActionContext<'static> {
        ClientExperienceActionContext {
            new_world_seed: 1234,
            next_new_world_seed: Some(5678),
            current_join_remote_addr: "127.0.0.1:25565",
            fallback_remote_addr: Some("10.0.0.2:25565"),
        }
    }

    fn native_profiles() -> [(NativePlatform, ClientExperienceSettingsProfile); 3] {
        [
            (
                NativePlatform::DesktopFlat,
                desktop_native_client_experience_profile().settings,
            ),
            (
                NativePlatform::Xr,
                xr_native_client_experience_profile().settings,
            ),
            (
                NativePlatform::AndroidFlat,
                android_flat_native_client_experience_profile().settings,
            ),
        ]
    }

    /// The core native-parity invariant: a feature can never be *silently*
    /// unavailable on one native target. Feature-axis divergence is allowed only
    /// as a reason-bearing, explicitly-declared exception.
    #[test]
    fn native_targets_share_feature_capability_availability() {
        for (platform, profile) in native_profiles() {
            for (feature, status) in profile.feature_axis() {
                match status {
                    ClientExperienceCapabilityStatus::Supported => {}
                    ClientExperienceCapabilityStatus::Unsupported(message) => {
                        assert_ne!(
                            status,
                            ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED,
                            "{platform:?} silently drops native feature {feature:?}; \
                             feature-axis divergence must carry an explicit reason",
                        );
                        assert!(
                            !message.trim().is_empty(),
                            "{platform:?} feature {feature:?} exception has an empty reason",
                        );
                        assert!(
                            NATIVE_FEATURE_PARITY_EXCEPTIONS.contains(&(platform, feature)),
                            "{platform:?} diverges on native feature {feature:?} without a \
                             declared exception in NATIVE_FEATURE_PARITY_EXCEPTIONS",
                        );
                    }
                    other => panic!(
                        "{platform:?} feature {feature:?} has status {other:?}; the feature axis \
                         must be Supported or a reason-bearing Unsupported",
                    ),
                }
            }
        }
    }

    /// Keep the exception ledger honest: a declared exception that has become
    /// `Supported` is stale and must be removed so the ledger reflects real gaps.
    #[test]
    fn native_feature_parity_exceptions_are_live() {
        let profiles = native_profiles();
        for &(platform, feature) in NATIVE_FEATURE_PARITY_EXCEPTIONS {
            let profile = profiles
                .iter()
                .find(|(candidate, _)| *candidate == platform)
                .map(|(_, profile)| profile)
                .unwrap_or_else(|| panic!("no native profile for {platform:?}"));
            let status = profile
                .feature_axis()
                .into_iter()
                .find(|(candidate, _)| *candidate == feature)
                .map(|(_, status)| status)
                .expect("feature axis entry for declared exception");
            assert!(
                !status.is_supported(),
                "{platform:?} lists {feature:?} as a parity exception but it is now Supported; \
                 remove the stale NATIVE_FEATURE_PARITY_EXCEPTIONS entry",
            );
        }
    }

    /// Web may diverge on features (different runtime/threading model), but the
    /// audited ledger must exactly describe every reason-bearing gap.
    #[test]
    fn web_feature_divergences_match_audited_ledger() {
        let profile = web_client_experience_profile().settings;
        for (feature, status) in profile.feature_axis() {
            let ledger_reason = WEB_FEATURE_PARITY_EXCEPTIONS
                .iter()
                .find(|(candidate, _)| *candidate == feature)
                .map(|(_, reason)| *reason);
            match status {
                ClientExperienceCapabilityStatus::Supported => assert!(
                    ledger_reason.is_none(),
                    "web feature {feature:?} is supported but retains a stale exception",
                ),
                ClientExperienceCapabilityStatus::Unsupported(reason) => {
                    assert_ne!(
                        status,
                        ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED,
                        "web silently drops feature {feature:?}; record an explicit reason",
                    );
                    assert_eq!(
                        ledger_reason,
                        Some(reason),
                        "web feature {feature:?} must exactly match its audited exception",
                    );
                }
                other => {
                    panic!("web feature {feature:?} has unaudited capability status {other:?}",)
                }
            }
        }

        for &(feature, reason) in WEB_FEATURE_PARITY_EXCEPTIONS {
            assert!(
                !reason.trim().is_empty(),
                "web feature {feature:?} has an empty reason"
            );
            assert_eq!(
                WEB_FEATURE_PARITY_EXCEPTIONS
                    .iter()
                    .filter(|(candidate, _)| *candidate == feature)
                    .count(),
                1,
                "web feature {feature:?} has duplicate exception entries",
            );
        }
    }

    #[test]
    fn classification_covers_every_game_ui_action_variant() {
        let samples = [
            GameUiAction::StartWorld,
            GameUiAction::OpenWorldList,
            GameUiAction::OpenWorldCreate,
            GameUiAction::SelectWorld(WorldCatalogUiWorldId(1)),
            GameUiAction::OpenWorld(WorldCatalogUiWorldId(1)),
            GameUiAction::CreateCatalogWorld,
            GameUiAction::ConfirmDeleteWorld(WorldCatalogUiWorldId(1)),
            GameUiAction::DeleteWorld(WorldCatalogUiWorldId(1)),
            GameUiAction::CancelDeleteWorld,
            GameUiAction::OpenNewWorld,
            GameUiAction::OpenJoinRemote,
            GameUiAction::RerollSeed,
            GameUiAction::CreateWorld(1),
            GameUiAction::JoinRemote,
            GameUiAction::Resume,
            GameUiAction::OpenBlockPalette,
            GameUiAction::OpenHelp(GameHelpParent::Title),
            GameUiAction::CloseHelp(GameHelpParent::Title),
            GameUiAction::AssignHotbarBlock {
                slot: 0,
                block_state: 1,
            },
            GameUiAction::OpenOptions(GameOptionsParent::Title),
            GameUiAction::OpenOptionsCategory(
                GameOptionsParent::Title,
                GameOptionsCategory::Graphics,
            ),
            GameUiAction::OpenServerSettings(GameOptionsParent::Title),
            GameUiAction::OpenAssetPacks(GameOptionsParent::Title),
            GameUiAction::ToggleAssetPack(AssetPackUiId(1)),
            GameUiAction::ApplyAssetPacks,
            GameUiAction::CancelAssetPacks,
            GameUiAction::BackToTitle,
            GameUiAction::BackToPause,
            GameUiAction::QuitToTitle,
            GameUiAction::ToggleSectionOcclusion,
            GameUiAction::ToggleFullbright,
            GameUiAction::ToggleFarLod,
            GameUiAction::SetFarLodRange(8),
            GameUiAction::TogglePlayerCollisionBox,
            GameUiAction::ToggleFirstPersonPlayer,
            GameUiAction::ToggleCrosshair,
            GameUiAction::ToggleFramePipelineOverlay,
            GameUiAction::ToggleDebugDiagnostics,
            GameUiAction::SetPlayerModel(GamePlayerModel::UprightBear),
            GameUiAction::SetMovementMode(GameMovementMode::Fly),
            GameUiAction::SetCollisionMode(GameCollisionMode::NoClip),
            GameUiAction::SetTravelAssistMode(GameTravelAssistMode::Blink),
            GameUiAction::SetTurnMode(GameTurnMode::Snap30),
            GameUiAction::SetXrTurnMode(GameXrTurnMode::Snap30),
            GameUiAction::CycleFramePacing,
            GameUiAction::CycleFpsCap,
            GameUiAction::SetRenderDistance(8),
            GameUiAction::SetFlySpeed(2.0),
            GameUiAction::SetMovementSpeed(2.0),
            GameUiAction::SetTouchLookSensitivity(2.0),
            GameUiAction::SetTouchControlsMode(TouchControlsMode::On),
            GameUiAction::SetServerSimulationCadence(GameSimulationCadence::default()),
            GameUiAction::Quit,
        ];

        assert_eq!(samples.len(), 53);
        for sample in samples {
            let _ = classify_game_ui_action(sample);
        }
        assert_eq!(
            classify_game_ui_action(GameUiAction::ToggleFarLod),
            ClientExperienceActionClassification::CapabilityGated
        );
        assert_eq!(
            classify_game_ui_action(GameUiAction::ToggleFramePipelineOverlay),
            ClientExperienceActionClassification::CapabilityGated
        );
        assert_eq!(
            classify_game_ui_action(GameUiAction::ToggleDebugDiagnostics),
            ClientExperienceActionClassification::CapabilityGated
        );
        assert_eq!(
            classify_game_ui_action(GameUiAction::Quit),
            ClientExperienceActionClassification::HostEffectAction
        );
    }

    #[test]
    fn facade_routes_catalog_actions_to_catalog_controller() {
        let mut controller = ClientExperienceController::default();
        controller
            .catalog_mut()
            .set_capabilities(WorldCatalogCapabilities::persistent_local());

        let effects = controller.apply_ui_action(GameUiAction::OpenWorldList, context());

        assert_eq!(effects.catalog.catalog_requests.len(), 1);
        assert_eq!(
            effects.catalog.catalog_requests[0].request,
            WorldCatalogRequest::ListWorlds
        );
        assert!(effects.session.clear_inactive_session_status);
        assert!(effects.session.session_start.is_none());
        assert!(effects.session.host_action.is_none());
        assert!(effects.settings.is_empty());
    }

    #[test]
    fn facade_routes_session_actions_to_session_helper() {
        let mut controller = ClientExperienceController::default();
        let effects = controller.apply_ui_action(GameUiAction::OpenNewWorld, context());

        assert_eq!(effects.session.new_world_seed, Some(5678));
        assert!(effects.catalog.catalog_requests.is_empty());
        assert!(effects.settings.is_empty());
    }

    #[test]
    fn facade_routes_projection_and_gameplay_actions_without_platform_work() {
        let mut controller = ClientExperienceController::default();

        let effects = controller.apply_ui_action(GameUiAction::OpenBlockPalette, context());
        assert_eq!(
            effects.projection,
            vec![ClientExperienceProjectionEffect::ApplyUiAction(
                GameUiAction::OpenBlockPalette
            )]
        );

        let effects = controller.apply_ui_action(
            GameUiAction::AssignHotbarBlock {
                slot: 2,
                block_state: 9,
            },
            context(),
        );
        assert_eq!(
            effects.gameplay,
            vec![ClientExperienceGameplayEffect::AssignHotbarBlock {
                slot: 2,
                block_state: 9
            }]
        );
    }

    #[test]
    fn settings_toggles_round_trip_and_emit_effects() {
        let mut settings = ClientExperienceSettingsController::default();

        let effects = settings.apply_ui_action(
            GameUiAction::ToggleFullbright,
            ClientExperienceSettingsProfile::default(),
        );
        assert!(settings.state().force_fullbright);
        assert_eq!(
            effects.setting_effects,
            vec![ClientExperienceSettingEffect::SetFullbright(true)]
        );

        let effects = settings.apply_ui_action(
            GameUiAction::ToggleFullbright,
            ClientExperienceSettingsProfile::default(),
        );
        assert!(!settings.state().force_fullbright);
        assert_eq!(
            effects.setting_effects,
            vec![ClientExperienceSettingEffect::SetFullbright(false)]
        );

        let effects = settings.apply_ui_action(
            GameUiAction::ToggleFramePipelineOverlay,
            ClientExperienceSettingsProfile::default(),
        );
        assert!(settings.state().frame_pipeline_overlay_visible);
        assert_eq!(
            effects.setting_effects,
            vec![ClientExperienceSettingEffect::SetFramePipelineOverlayVisible(true)]
        );

        let effects = settings.apply_ui_action(
            GameUiAction::ToggleDebugDiagnostics,
            ClientExperienceSettingsProfile::default(),
        );
        assert!(settings.state().debug_diagnostics_visible);
        assert_eq!(
            effects.setting_effects,
            vec![ClientExperienceSettingEffect::SetDebugDiagnosticsVisible(
                true
            )]
        );
    }

    #[test]
    fn settings_clamp_render_and_far_lod_effects() {
        let mut settings = ClientExperienceSettingsController::new(ClientExperienceSettingsState {
            render_distance: 4,
            min_render_distance: 2,
            max_render_distance: 16,
            far_lod_range_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            ..ClientExperienceSettingsState::default()
        });

        let effects = settings.apply_ui_action(
            GameUiAction::SetRenderDistance(99),
            ClientExperienceSettingsProfile::default(),
        );
        assert_eq!(settings.state().render_distance, 16);
        assert_eq!(
            effects.setting_effects,
            vec![ClientExperienceSettingEffect::SetRenderDistance(16)]
        );

        let effects = settings.apply_ui_action(
            GameUiAction::SetFarLodRange(-5),
            ClientExperienceSettingsProfile::default(),
        );
        assert_eq!(
            settings.state().far_lod_range_chunks,
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32
        );
        assert_eq!(
            effects.setting_effects,
            vec![
                ClientExperienceSettingEffect::SetFarLod {
                    enabled: false,
                    extra_radius_chunks: MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                },
                ClientExperienceSettingEffect::ClearFarLod,
            ]
        );
    }

    #[test]
    fn capability_gated_actions_project_unsupported_without_mutating() {
        let profile = ClientExperienceSettingsProfile {
            far_lod: ClientExperienceCapabilityStatus::Unsupported(
                "Far LOD is unavailable for this profile",
            ),
            frame_pipeline_overlay: ClientExperienceCapabilityStatus::Unsupported(
                "Frame pipeline overlay is unavailable for this profile",
            ),
            debug_diagnostics: ClientExperienceCapabilityStatus::Unsupported(
                "Debug diagnostics are unavailable for this profile",
            ),
            ..ClientExperienceSettingsProfile::default()
        };
        let mut settings = ClientExperienceSettingsController::default();

        let effects = settings.apply_ui_action(GameUiAction::ToggleFarLod, profile);

        assert!(!settings.state().far_lod_enabled);
        assert!(effects.setting_effects.is_empty());
        assert_eq!(
            effects.capability_projection.actions,
            vec![ClientExperienceActionAvailability {
                kind: ClientExperienceActionKind::ToggleFarLod,
                status: ClientExperienceCapabilityStatus::Unsupported(
                    "Far LOD is unavailable for this profile"
                ),
            }]
        );

        let effects = settings.apply_ui_action(GameUiAction::ToggleFramePipelineOverlay, profile);
        assert!(!settings.state().frame_pipeline_overlay_visible);
        assert_eq!(
            effects.capability_projection.actions,
            vec![ClientExperienceActionAvailability {
                kind: ClientExperienceActionKind::ToggleFramePipelineOverlay,
                status: ClientExperienceCapabilityStatus::Unsupported(
                    "Frame pipeline overlay is unavailable for this profile"
                ),
            }]
        );

        let effects = settings.apply_ui_action(GameUiAction::ToggleDebugDiagnostics, profile);
        assert!(!settings.state().debug_diagnostics_visible);
        assert_eq!(
            effects.capability_projection.actions,
            vec![ClientExperienceActionAvailability {
                kind: ClientExperienceActionKind::ToggleDebugDiagnostics,
                status: ClientExperienceCapabilityStatus::Unsupported(
                    "Debug diagnostics are unavailable for this profile"
                ),
            }]
        );
    }

    #[test]
    fn capability_projection_surfaces_profile_and_state_unavailable_actions() {
        let profile = ClientExperienceSettingsProfile {
            xr_turn: ClientExperienceCapabilityStatus::Unsupported(
                "XR turn mode is unavailable outside XR",
            ),
            touch_look: ClientExperienceCapabilityStatus::Pending(
                "Touch look settings are pending input setup",
            ),
            ..ClientExperienceSettingsProfile::default()
        };
        let settings = ClientExperienceSettingsController::default();

        let projection = settings.capability_projection(profile);

        assert!(
            projection
                .actions
                .contains(&ClientExperienceActionAvailability {
                    kind: ClientExperienceActionKind::SetXrTurnMode,
                    status: ClientExperienceCapabilityStatus::Unsupported(
                        "XR turn mode is unavailable outside XR"
                    ),
                })
        );
        assert!(
            projection
                .actions
                .contains(&ClientExperienceActionAvailability {
                    kind: ClientExperienceActionKind::SetTouchLookSensitivity,
                    status: ClientExperienceCapabilityStatus::Pending(
                        "Touch look settings are pending input setup"
                    ),
                })
        );
        assert!(
            projection
                .actions
                .contains(&ClientExperienceActionAvailability {
                    kind: ClientExperienceActionKind::SetTouchControlsMode,
                    status: ClientExperienceCapabilityStatus::Unsupported(
                        "Touch controls are unavailable for this profile"
                    ),
                })
        );
    }

    #[test]
    fn movement_experience_reducer_disables_travel_assist_for_fly() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::Normal,
            travel_assist_mode: GameTravelAssistMode::Blink,
            ..ClientExperienceSettingsState::default()
        };

        assert!(state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::MovementMode(GameMovementMode::Fly)
        ));

        assert_eq!(state.movement_mode, GameMovementMode::Fly);
        assert_eq!(state.collision_mode, GameCollisionMode::Normal);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Off);
    }

    #[test]
    fn movement_experience_reducer_disables_travel_assist_for_no_clip() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::Normal,
            travel_assist_mode: GameTravelAssistMode::Blink,
            ..ClientExperienceSettingsState::default()
        };

        state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::CollisionMode(GameCollisionMode::NoClip),
        );

        assert_eq!(state.collision_mode, GameCollisionMode::NoClip);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Off);
    }

    #[test]
    fn movement_experience_reducer_selecting_blink_restores_normal_collision() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::NoClip,
            travel_assist_mode: GameTravelAssistMode::Off,
            ..ClientExperienceSettingsState::default()
        };

        state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::TravelAssistMode(GameTravelAssistMode::Blink),
        );

        assert_eq!(state.collision_mode, GameCollisionMode::Normal);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Blink);
    }

    #[test]
    fn movement_experience_reducer_selecting_warp_restores_normal_collision() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::NoClip,
            travel_assist_mode: GameTravelAssistMode::Off,
            ..ClientExperienceSettingsState::default()
        };

        state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::TravelAssistMode(GameTravelAssistMode::Warp),
        );

        assert_eq!(state.collision_mode, GameCollisionMode::Normal);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Warp);
    }

    #[test]
    fn movement_experience_reducer_rejects_travel_assist_for_fly() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Fly,
            collision_mode: GameCollisionMode::Normal,
            travel_assist_mode: GameTravelAssistMode::Off,
            ..ClientExperienceSettingsState::default()
        };

        let changed = state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::TravelAssistMode(GameTravelAssistMode::Blink),
        );

        assert!(!changed);
        assert_eq!(state.movement_mode, GameMovementMode::Fly);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Off);
    }

    #[test]
    fn movement_experience_reducer_keeps_gorilla_collision_normal() {
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Fly,
            collision_mode: GameCollisionMode::NoClip,
            travel_assist_mode: GameTravelAssistMode::Off,
            ..ClientExperienceSettingsState::default()
        };

        state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::MovementMode(GameMovementMode::HandPush),
        );

        assert_eq!(state.movement_mode, GameMovementMode::HandPush);
        assert_eq!(state.collision_mode, GameCollisionMode::Normal);
    }

    #[test]
    fn movement_experience_reducer_thruster_defaults_normal_and_clears_travel_assist() {
        // Tactical 157: entering Thruster defaults to Normal collision and turns
        // Travel Assist off (a flight mode, like Fly).
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::NoClip,
            travel_assist_mode: GameTravelAssistMode::Blink,
            ..ClientExperienceSettingsState::default()
        };

        state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::MovementMode(GameMovementMode::Thruster),
        );

        assert_eq!(state.movement_mode, GameMovementMode::Thruster);
        assert_eq!(state.collision_mode, GameCollisionMode::Normal);
        assert_eq!(state.travel_assist_mode, GameTravelAssistMode::Off);
    }

    #[test]
    fn movement_experience_reducer_thruster_keeps_noclip_selectable() {
        // Unlike HandPush, Thruster is not force-normalized, so NoClip stays
        // selectable after entry (tactical 157).
        let mut state = ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Thruster,
            collision_mode: GameCollisionMode::Normal,
            travel_assist_mode: GameTravelAssistMode::Off,
            ..ClientExperienceSettingsState::default()
        };

        let changed = state.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::CollisionMode(GameCollisionMode::NoClip),
        );

        assert!(changed);
        assert_eq!(state.movement_mode, GameMovementMode::Thruster);
        assert_eq!(state.collision_mode, GameCollisionMode::NoClip);
    }

    #[test]
    fn movement_experience_reducer_syncs_shared_turn_to_legacy_xr_slot() {
        let mut state = ClientExperienceSettingsState::default();

        state.apply_movement_experience_change(ClientExperienceMovementSettingChange::TurnMode(
            Some(GameTurnMode::Smooth),
        ));

        assert_eq!(state.turn_mode, Some(GameTurnMode::Smooth));
        assert_eq!(state.xr_turn_mode, Some(GameXrTurnMode::Smooth));
    }

    #[test]
    fn movement_experience_menu_actions_emit_shared_corrections() {
        let mut settings = ClientExperienceSettingsController::new(ClientExperienceSettingsState {
            movement_mode: GameMovementMode::Walk,
            collision_mode: GameCollisionMode::NoClip,
            travel_assist_mode: GameTravelAssistMode::Off,
            turn_mode: Some(GameTurnMode::Snap15),
            ..ClientExperienceSettingsState::default()
        });

        let effects = settings.apply_ui_action(
            GameUiAction::SetTravelAssistMode(GameTravelAssistMode::Blink),
            ClientExperienceSettingsProfile::default(),
        );

        assert_eq!(settings.state().collision_mode, GameCollisionMode::Normal);
        assert_eq!(
            effects.setting_effects,
            vec![
                ClientExperienceSettingEffect::SetCollisionMode(GameCollisionMode::Normal),
                ClientExperienceSettingEffect::SetTravelAssistMode(GameTravelAssistMode::Blink),
            ]
        );

        let effects = settings.apply_ui_action(
            GameUiAction::SetMovementMode(GameMovementMode::Fly),
            ClientExperienceSettingsProfile::default(),
        );

        assert_eq!(settings.state().movement_mode, GameMovementMode::Fly);
        assert_eq!(
            settings.state().travel_assist_mode,
            GameTravelAssistMode::Off
        );
        assert_eq!(
            effects.setting_effects,
            vec![
                ClientExperienceSettingEffect::SetMovementMode(GameMovementMode::Fly),
                ClientExperienceSettingEffect::SetTravelAssistMode(GameTravelAssistMode::Off),
            ]
        );
    }

    #[test]
    fn set_turn_mode_uses_shared_turn_capability() {
        let profile = ClientExperienceSettingsProfile {
            turn_mode: ClientExperienceCapabilityStatus::Unsupported(
                "Turn mode is unavailable for this profile",
            ),
            ..ClientExperienceSettingsProfile::default()
        };
        let mut settings = ClientExperienceSettingsController::new(ClientExperienceSettingsState {
            turn_mode: Some(GameTurnMode::Snap15),
            xr_turn_mode: Some(GameXrTurnMode::Snap15),
            ..ClientExperienceSettingsState::default()
        });

        let effects =
            settings.apply_ui_action(GameUiAction::SetTurnMode(GameTurnMode::Snap30), profile);

        assert_eq!(settings.state().turn_mode, Some(GameTurnMode::Snap15));
        assert_eq!(
            effects.capability_projection.actions,
            vec![ClientExperienceActionAvailability {
                kind: ClientExperienceActionKind::SetTurnMode,
                status: ClientExperienceCapabilityStatus::Unsupported(
                    "Turn mode is unavailable for this profile"
                ),
            }]
        );
    }

    #[test]
    fn invalid_server_cadence_is_rejected_in_shared_policy() {
        let mut settings = ClientExperienceSettingsController::default();
        let effects = settings.apply_ui_action(
            GameUiAction::SetServerSimulationCadence(GameSimulationCadence::new(20, 17, 60)),
            ClientExperienceSettingsProfile::default(),
        );

        assert!(effects.setting_effects.is_empty());
        assert_eq!(
            effects.rejections,
            vec![ClientExperienceActionRejection::invalid(
                ClientExperienceActionKind::SetServerSimulationCadence,
                "Server simulation cadence is invalid",
            )]
        );
    }
}
