use anyhow::Result;
use mclone_app_runtime::client_experience::{
    ClientExperienceCapabilityProjection, ClientExperienceSettingEffect,
    ClientExperienceSettingsEffects,
};
use mclone_app_runtime::client_session_policy::ClientSessionHostAction;
use mclone_app_runtime::far_lod::FarLodDetailMode;
use mclone_input::TouchControlsMode;
use mclone_ui::{
    GameCollisionMode, GameLeafDetail, GameMovementMode, GamePlayerModel, GameSimulationCadence,
    GameTravelAssistMode, GameTurnMode, GameWorldRenderScaleMode, GameXrTurnMode, StatusOverlay,
};

/// Platform hooks emitted by shared client-experience policy.
///
/// Implementations may adapt these requests to winit, Android, OpenXR, or a
/// headless harness. Engine settings do not belong here; they are applied
/// through [`ClientExperienceSettingsHost`].
pub trait HostEffects {
    fn request_mouse_lock(&mut self, requested: bool) -> Result<()>;
    fn cycle_frame_pacing(&mut self) -> Result<()>;
    fn cycle_fps_cap(&mut self) -> Result<()>;
    fn set_world_render_scale_mode(&mut self, mode: GameWorldRenderScaleMode) -> Result<()>;
    fn set_touch_controls_mode(&mut self, mode: TouchControlsMode) -> Result<()>;
    fn quit_to_title(&mut self) -> Result<()>;
    fn exit(&mut self) -> Result<()>;
}

/// Shared settings contract used by the exhaustive client-experience effect
/// dispatcher.
pub trait ClientExperienceSettingsHost {
    fn set_section_occlusion_culling(&mut self, enabled: bool) -> Result<()>;
    fn set_leaf_detail(&mut self, detail: GameLeafDetail) -> Result<()>;
    fn set_fullbright(&mut self, enabled: bool) -> Result<()>;
    fn set_far_lod(&mut self, enabled: bool, extra_radius_chunks: u32) -> Result<()>;
    fn set_far_lod_detail_mode(&mut self, mode: FarLodDetailMode) -> Result<()>;
    fn clear_far_lod(&mut self) -> Result<()>;
    fn set_player_collision_box_visible(&mut self, visible: bool) -> Result<()>;
    fn set_first_person_player_visible(&mut self, visible: bool) -> Result<()>;
    fn set_crosshair_visible(&mut self, visible: bool) -> Result<()>;
    fn set_frame_pipeline_overlay_visible(&mut self, visible: bool) -> Result<()>;
    fn set_debug_diagnostics_visible(&mut self, visible: bool) -> Result<()>;
    fn set_player_model(&mut self, model: GamePlayerModel) -> Result<()>;
    fn sync_player_appearance(&mut self) -> Result<()>;
    fn set_movement_mode(&mut self, mode: GameMovementMode) -> Result<()>;
    fn set_collision_mode(&mut self, mode: GameCollisionMode) -> Result<()>;
    fn set_travel_assist_mode(&mut self, mode: GameTravelAssistMode) -> Result<()>;
    fn set_turn_mode(&mut self, mode: GameTurnMode) -> Result<()>;
    fn set_xr_turn_mode(&mut self, mode: GameXrTurnMode) -> Result<()>;
    fn set_render_distance(&mut self, render_distance: u32) -> Result<()>;
    fn set_fly_speed_multiplier(&mut self, multiplier: f32) -> Result<()>;
    fn set_movement_speed_multiplier(&mut self, multiplier: f32) -> Result<()>;
    fn set_touch_look_sensitivity(&mut self, sensitivity: f32) -> Result<()>;
    fn set_server_simulation_cadence(&mut self, cadence: GameSimulationCadence) -> Result<()>;
    fn set_status_overlay(&mut self, status: StatusOverlay);
}

pub fn apply_client_experience_settings_effects<T, H>(
    target: &mut T,
    host: &mut H,
    effects: ClientExperienceSettingsEffects,
) -> Result<bool>
where
    T: ClientExperienceSettingsHost,
    H: HostEffects,
{
    let has_setting_effects = !effects.setting_effects.is_empty();
    let has_unavailable =
        !effects.capability_projection.is_empty() || !effects.rejections.is_empty();

    for effect in effects.setting_effects {
        match effect {
            ClientExperienceSettingEffect::SetSectionOcclusionCulling(enabled) => {
                target.set_section_occlusion_culling(enabled)?;
            }
            ClientExperienceSettingEffect::SetLeafDetail(detail) => {
                target.set_leaf_detail(detail)?;
            }
            ClientExperienceSettingEffect::SetFullbright(enabled) => {
                target.set_fullbright(enabled)?;
            }
            ClientExperienceSettingEffect::SetFarLod {
                enabled,
                extra_radius_chunks,
            } => target.set_far_lod(enabled, extra_radius_chunks)?,
            ClientExperienceSettingEffect::SetFarLodDetailMode(mode) => {
                target.set_far_lod_detail_mode(mode)?;
            }
            ClientExperienceSettingEffect::ClearFarLod => target.clear_far_lod()?,
            ClientExperienceSettingEffect::SetPlayerCollisionBoxVisible(visible) => {
                target.set_player_collision_box_visible(visible)?;
            }
            ClientExperienceSettingEffect::SetFirstPersonPlayerVisible(visible) => {
                target.set_first_person_player_visible(visible)?;
            }
            ClientExperienceSettingEffect::SetCrosshairVisible(visible) => {
                target.set_crosshair_visible(visible)?;
            }
            ClientExperienceSettingEffect::SetFramePipelineOverlayVisible(visible) => {
                target.set_frame_pipeline_overlay_visible(visible)?;
            }
            ClientExperienceSettingEffect::SetDebugDiagnosticsVisible(visible) => {
                target.set_debug_diagnostics_visible(visible)?;
            }
            // Flat presentation topology is retained by `MonoUiContext` and
            // applied before this generic engine/platform dispatcher.
            ClientExperienceSettingEffect::SetAuxiliarySplitMode(_)
            | ClientExperienceSettingEffect::SetLocalPlayLayout(_)
            | ClientExperienceSettingEffect::SetLocalPlayGuestInput(_) => {}
            ClientExperienceSettingEffect::SetPlayerModel(model) => {
                target.set_player_model(model)?;
            }
            ClientExperienceSettingEffect::SyncPlayerAppearance => {
                target.sync_player_appearance()?;
            }
            ClientExperienceSettingEffect::SetMovementMode(mode) => {
                target.set_movement_mode(mode)?;
            }
            ClientExperienceSettingEffect::SetCollisionMode(mode) => {
                target.set_collision_mode(mode)?;
            }
            ClientExperienceSettingEffect::SetTravelAssistMode(mode) => {
                target.set_travel_assist_mode(mode)?;
            }
            ClientExperienceSettingEffect::SetTurnMode(mode) => target.set_turn_mode(mode)?,
            ClientExperienceSettingEffect::SetXrTurnMode(mode) => {
                target.set_xr_turn_mode(mode)?;
            }
            ClientExperienceSettingEffect::CycleFramePacing => host.cycle_frame_pacing()?,
            ClientExperienceSettingEffect::CycleFpsCap => host.cycle_fps_cap()?,
            ClientExperienceSettingEffect::SetWorldRenderScaleMode(mode) => {
                host.set_world_render_scale_mode(mode)?;
            }
            ClientExperienceSettingEffect::SetRenderDistance(render_distance) => {
                target.set_render_distance(render_distance)?;
            }
            ClientExperienceSettingEffect::SetFlySpeedMultiplier(multiplier) => {
                target.set_fly_speed_multiplier(multiplier)?;
            }
            ClientExperienceSettingEffect::SetMovementSpeedMultiplier(multiplier) => {
                target.set_movement_speed_multiplier(multiplier)?;
            }
            ClientExperienceSettingEffect::SetTouchLookSensitivity(sensitivity) => {
                target.set_touch_look_sensitivity(sensitivity)?;
            }
            ClientExperienceSettingEffect::SetTouchControlsMode(mode) => {
                host.set_touch_controls_mode(mode)?;
            }
            ClientExperienceSettingEffect::SetServerSimulationCadence(cadence) => {
                target.set_server_simulation_cadence(cadence)?;
            }
        }
    }

    apply_capability_projection(target, effects.capability_projection);
    let mut accepted = true;
    for rejection in effects.rejections {
        log::error!("{}", rejection.message);
        target.set_status_overlay(StatusOverlay::new(rejection.message, false));
        accepted = false;
    }
    if has_setting_effects && !has_unavailable {
        target.set_status_overlay(StatusOverlay::hidden());
    }
    Ok(accepted)
}

pub fn apply_client_session_host_action<H>(
    action: ClientSessionHostAction,
    host: &mut H,
) -> Result<()>
where
    H: HostEffects,
{
    match action {
        ClientSessionHostAction::QuitToTitle => host.quit_to_title(),
        ClientSessionHostAction::Quit => host.exit(),
    }
}

fn apply_capability_projection<T>(target: &mut T, projection: ClientExperienceCapabilityProjection)
where
    T: ClientExperienceSettingsHost,
{
    if let Some(unavailable) = projection.first_unavailable()
        && let Some(message) = unavailable.status.message()
    {
        log::warn!("{message}");
        target.set_status_overlay(StatusOverlay::new(message, unavailable.status.ok()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::client_experience::{
        ClientExperienceActionAvailability, ClientExperienceActionKind,
        ClientExperienceActionRejection, ClientExperienceCapabilityProjection,
        ClientExperienceCapabilityStatus, ClientExperienceSettingsEffects,
    };
    use mclone_ui::GameTravelAssistMode;

    #[derive(Default)]
    struct TestSettingsHost {
        calls: Vec<&'static str>,
        status: Option<StatusOverlay>,
    }

    macro_rules! record_method {
        ($name:ident($($arg:ident : $ty:ty),*)) => {
            fn $name(&mut self, $($arg: $ty),*) -> Result<()> {
                $(let _ = $arg;)*
                self.calls.push(stringify!($name));
                Ok(())
            }
        };
    }

    impl ClientExperienceSettingsHost for TestSettingsHost {
        record_method!(set_section_occlusion_culling(enabled: bool));
        record_method!(set_leaf_detail(detail: GameLeafDetail));
        record_method!(set_fullbright(enabled: bool));
        record_method!(set_far_lod(enabled: bool, extra_radius_chunks: u32));
        record_method!(set_far_lod_detail_mode(mode: FarLodDetailMode));
        record_method!(clear_far_lod());
        record_method!(set_player_collision_box_visible(visible: bool));
        record_method!(set_first_person_player_visible(visible: bool));
        record_method!(set_crosshair_visible(visible: bool));
        record_method!(set_frame_pipeline_overlay_visible(visible: bool));
        record_method!(set_debug_diagnostics_visible(visible: bool));
        record_method!(set_player_model(model: GamePlayerModel));
        record_method!(sync_player_appearance());
        record_method!(set_movement_mode(mode: GameMovementMode));
        record_method!(set_collision_mode(mode: GameCollisionMode));
        record_method!(set_travel_assist_mode(mode: GameTravelAssistMode));
        record_method!(set_turn_mode(mode: GameTurnMode));
        record_method!(set_xr_turn_mode(mode: GameXrTurnMode));
        record_method!(set_render_distance(render_distance: u32));
        record_method!(set_fly_speed_multiplier(multiplier: f32));
        record_method!(set_movement_speed_multiplier(multiplier: f32));
        record_method!(set_touch_look_sensitivity(sensitivity: f32));
        record_method!(set_server_simulation_cadence(cadence: GameSimulationCadence));

        fn set_status_overlay(&mut self, status: StatusOverlay) {
            self.status = Some(status);
        }
    }

    #[derive(Default)]
    struct TestHostEffects {
        calls: Vec<&'static str>,
    }

    impl HostEffects for TestHostEffects {
        record_method!(request_mouse_lock(requested: bool));
        record_method!(cycle_frame_pacing());
        record_method!(cycle_fps_cap());
        record_method!(set_world_render_scale_mode(mode: GameWorldRenderScaleMode));
        record_method!(set_touch_controls_mode(mode: TouchControlsMode));
        record_method!(quit_to_title());
        record_method!(exit());
    }

    #[test]
    fn shared_dispatch_routes_engine_and_platform_effects_once() {
        let mut target = TestSettingsHost::default();
        let mut host = TestHostEffects::default();
        let effects = ClientExperienceSettingsEffects {
            setting_effects: vec![
                ClientExperienceSettingEffect::SetLeafDetail(GameLeafDetail::Bushy),
                ClientExperienceSettingEffect::SetFullbright(true),
                ClientExperienceSettingEffect::SetTravelAssistMode(GameTravelAssistMode::Blink),
                ClientExperienceSettingEffect::CycleFramePacing,
                ClientExperienceSettingEffect::SetWorldRenderScaleMode(
                    GameWorldRenderScaleMode::ThreeQuarters,
                ),
                ClientExperienceSettingEffect::SetTouchControlsMode(TouchControlsMode::On),
            ],
            ..ClientExperienceSettingsEffects::default()
        };

        assert!(apply_client_experience_settings_effects(&mut target, &mut host, effects).unwrap());
        assert_eq!(
            target.calls,
            vec![
                "set_leaf_detail",
                "set_fullbright",
                "set_travel_assist_mode"
            ]
        );
        assert_eq!(
            host.calls,
            vec![
                "cycle_frame_pacing",
                "set_world_render_scale_mode",
                "set_touch_controls_mode",
            ]
        );
        assert!(target.status.as_ref().is_some_and(|status| !status.visible));
    }

    #[test]
    fn capability_and_rejection_status_remain_visible() {
        let mut target = TestSettingsHost::default();
        let mut host = TestHostEffects::default();
        let effects = ClientExperienceSettingsEffects {
            capability_projection: ClientExperienceCapabilityProjection {
                actions: vec![ClientExperienceActionAvailability {
                    kind: ClientExperienceActionKind::SetTravelAssistMode,
                    status: ClientExperienceCapabilityStatus::Unsupported("travel unavailable"),
                }],
            },
            rejections: vec![ClientExperienceActionRejection::invalid(
                ClientExperienceActionKind::SetTravelAssistMode,
                "travel rejected",
            )],
            ..ClientExperienceSettingsEffects::default()
        };

        assert!(
            !apply_client_experience_settings_effects(&mut target, &mut host, effects).unwrap()
        );
        let status = target.status.expect("visible rejection status");
        assert!(status.visible);
        assert!(!status.ok);
        assert_eq!(status.message, "travel rejected");
    }

    #[test]
    fn session_host_actions_route_through_platform_boundary() {
        let mut host = TestHostEffects::default();
        apply_client_session_host_action(ClientSessionHostAction::QuitToTitle, &mut host).unwrap();
        apply_client_session_host_action(ClientSessionHostAction::Quit, &mut host).unwrap();
        assert_eq!(host.calls, vec!["quit_to_title", "exit"]);
    }
}
