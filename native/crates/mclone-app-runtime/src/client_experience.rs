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
    GameFramePacingMode, GameMovementMode, GamePlayerModel, GameSimulationCadence,
    GameTouchSettings, GameUiAction, GameUiRenderState, GameXrTurnMode,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ClientExperienceController {
    profile: ClientExperienceProfile,
    catalog: ClientCatalogController,
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
            | GameUiAction::SetPlayerModel(_)
            | GameUiAction::SetMovementMode(_)
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
    pub player_model: ClientExperienceCapabilityStatus,
    pub movement_mode: ClientExperienceCapabilityStatus,
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
            player_model: ClientExperienceCapabilityStatus::Supported,
            movement_mode: ClientExperienceCapabilityStatus::Supported,
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
            ClientExperienceActionKind::SetPlayerModel => self.player_model,
            ClientExperienceActionKind::SetMovementMode => self.movement_mode,
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
}

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
                self.state.movement_mode = mode;
                effects
                    .setting_effects
                    .push(ClientExperienceSettingEffect::SetMovementMode(mode));
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
                ClientExperienceActionKind::SetPlayerModel,
                profile.player_model,
            ),
            (
                ClientExperienceActionKind::SetMovementMode,
                profile.movement_mode,
            ),
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
    pub player_model: GamePlayerModel,
    pub movement_mode: GameMovementMode,
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
            player_model: state.player_model,
            movement_mode: state.movement_mode,
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
        state.player_model = self.player_model;
        state.movement_mode = self.movement_mode;
        state.xr_turn_mode = self.xr_turn_mode;
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
    SetPlayerModel(GamePlayerModel),
    SyncPlayerAppearance,
    SetMovementMode(GameMovementMode),
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
    OpenServerSettings,
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
    SetPlayerModel,
    SetMovementMode,
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
        GameUiAction::OpenServerSettings(_) => ClientExperienceActionKind::OpenServerSettings,
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
        GameUiAction::SetPlayerModel(_) => ClientExperienceActionKind::SetPlayerModel,
        GameUiAction::SetMovementMode(_) => ClientExperienceActionKind::SetMovementMode,
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
        | ClientExperienceActionKind::SetFlySpeed
        | ClientExperienceActionKind::SetMovementSpeed => {
            ClientExperienceActionClassification::CoreAction
        }
        ClientExperienceActionKind::ToggleFarLod
        | ClientExperienceActionKind::SetFarLodRange
        | ClientExperienceActionKind::ToggleCrosshair
        | ClientExperienceActionKind::ToggleFramePipelineOverlay
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
    use mclone_ui::{GameHelpParent, GameOptionsParent, WorldCatalogUiWorldId};

    fn context() -> ClientExperienceActionContext<'static> {
        ClientExperienceActionContext {
            new_world_seed: 1234,
            next_new_world_seed: Some(5678),
            current_join_remote_addr: "127.0.0.1:25565",
            fallback_remote_addr: Some("10.0.0.2:25565"),
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
            GameUiAction::OpenServerSettings(GameOptionsParent::Title),
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
            GameUiAction::SetPlayerModel(GamePlayerModel::UprightBear),
            GameUiAction::SetMovementMode(GameMovementMode::Fly),
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

        assert_eq!(samples.len(), 44);
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
