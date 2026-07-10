use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct XrHeadComfortState {
    pub alpha: f32,
    pub target_alpha: f32,
    pub blocked_residual: bool,
    pub head_penetrating: bool,
}

impl XrHeadComfortState {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn update(&mut self, target: XrHeadComfortTarget, dt_seconds: f64) {
        self.target_alpha = target.alpha;
        self.blocked_residual = target.blocked_residual;
        self.head_penetrating = target.head_penetrating;

        let dt_seconds = if dt_seconds.is_finite() {
            dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS) as f32
        } else {
            0.0
        };
        let fade_seconds = if target.alpha > self.alpha {
            XR_HEAD_COMFORT_FADE_IN_SECONDS
        } else {
            XR_HEAD_COMFORT_FADE_OUT_SECONDS
        };
        let t = if fade_seconds > 0.0 {
            (dt_seconds / fade_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.alpha += (target.alpha - self.alpha) * t;
        if target.alpha == 0.0 && self.alpha < 0.005 {
            self.alpha = 0.0;
        }
    }

    pub(crate) fn overlay(self) -> Option<ScreenFadeOverlay> {
        (self.alpha > 0.005).then(|| ScreenFadeOverlay::black(self.alpha))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct XrHeadComfortTarget {
    pub(super) alpha: f32,
    pub(super) blocked_residual: bool,
    pub(super) head_penetrating: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct XrUnderwaterEffectStates {
    midpoint: UnderwaterEffectState,
    eyes: [UnderwaterEffectState; 2],
}

pub(crate) fn underwater_overlay_from_forward(
    forward: Vec3,
    water_vision: f32,
    effect_strength: f32,
) -> UnderwaterOverlay {
    let forward = if forward.is_finite() && forward.length_squared() > f32::EPSILON {
        forward.normalize()
    } else {
        Vec3::Z
    };
    let yaw = engine_movement_yaw_from_forward(forward).unwrap_or(0.0);
    let pitch = forward.y.clamp(-1.0, 1.0).asin();
    UnderwaterOverlay::vanilla_from_native_camera(yaw, pitch)
        .with_effect(water_vision, effect_strength)
}

pub(crate) fn xr_head_comfort_target(
    reconciliation: Option<EngineRoomScaleReconciliation>,
    headset_world_position: Vec3,
    client: &mclone_client::ClientRuntime,
    collision_mode: EngineCameraCollisionMode,
) -> XrHeadComfortTarget {
    if collision_mode == EngineCameraCollisionMode::NoClip {
        return XrHeadComfortTarget::default();
    }
    let horizontal_residual = reconciliation
        .map(|reconciliation| reconciliation.residual_horizontal_length_sqr().sqrt())
        .unwrap_or(0.0);
    let head_penetrating = sphere_intersects_solid_blocks(
        client,
        vec3d_from_glam(headset_world_position),
        HAND_PUSH_DEFAULT_HEAD_RADIUS,
    );
    xr_head_comfort_target_from_inputs(horizontal_residual, head_penetrating)
}

pub(crate) fn xr_head_comfort_target_from_inputs(
    horizontal_residual: f64,
    head_penetrating: bool,
) -> XrHeadComfortTarget {
    xr_head_comfort_target_from_inputs_for_collision_mode(
        horizontal_residual,
        head_penetrating,
        EngineCameraCollisionMode::Normal,
    )
}

pub(crate) fn xr_head_comfort_target_from_inputs_for_collision_mode(
    horizontal_residual: f64,
    head_penetrating: bool,
    collision_mode: EngineCameraCollisionMode,
) -> XrHeadComfortTarget {
    if collision_mode == EngineCameraCollisionMode::NoClip {
        return XrHeadComfortTarget::default();
    }
    let blocked_residual = horizontal_residual.is_finite()
        && horizontal_residual > XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS;
    let mut alpha = xr_head_comfort_residual_alpha(horizontal_residual);
    if head_penetrating {
        alpha = alpha.max(XR_HEAD_COMFORT_HEAD_PENETRATION_ALPHA);
    }
    XrHeadComfortTarget {
        alpha: alpha.min(XR_HEAD_COMFORT_MAX_ALPHA),
        blocked_residual,
        head_penetrating,
    }
}

pub(crate) fn xr_head_comfort_residual_alpha(horizontal_residual: f64) -> f32 {
    if !horizontal_residual.is_finite()
        || horizontal_residual <= XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS
    {
        return 0.0;
    }
    let span = XR_HEAD_COMFORT_RESIDUAL_FULL_BLOCKS - XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS;
    let t = ((horizontal_residual - XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS) / span)
        .clamp(0.0, 1.0) as f32;
    let smooth = t * t * (3.0 - 2.0 * t);
    smooth * XR_HEAD_COMFORT_MAX_ALPHA
}

pub(crate) fn xr_head_comfort_fade_overlays(
    state: XrHeadComfortState,
) -> [Option<ScreenFadeOverlay>; 2] {
    let overlay = state.overlay();
    [overlay, overlay]
}

impl<S> McloneSceneHost<S>
where
    S: RemoteDedicatedServerSession,
{
    pub(crate) fn update_head_comfort_state(
        &mut self,
        transform: XrStageToWorld,
        views: &[XrView],
        dt_seconds: f64,
    ) -> Result<()> {
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_position = transform.transform_position(headset_stage_position);
        let Some(runtime) = self.runtime.as_ref() else {
            self.head_comfort.reset();
            return Ok(());
        };
        let target = xr_head_comfort_target(
            self.camera.last_room_scale_reconciliation(),
            headset_world_position,
            runtime.client(),
            self.camera.collision_mode(),
        );
        self.head_comfort.update(target, dt_seconds);
        Ok(())
    }

    pub(crate) fn underwater_overlays(
        &mut self,
        render_views: [ChunkRenderView; 2],
    ) -> [Option<UnderwaterOverlay>; 2] {
        let dt_seconds = self.underwater_effect_dt_seconds();
        match self.scene.underwater_detection_mode {
            XrUnderwaterDetectionMode::Midpoint => {
                let center_position =
                    (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
                let underwater = self.camera_inside_water(center_position);
                let effect = self
                    .underwater_effects
                    .midpoint
                    .update(underwater, dt_seconds);
                let overlay = underwater.then(|| {
                    let forward = average_unit_direction(
                        render_views[0].camera_forward,
                        render_views[1].camera_forward,
                        Vec3::Z,
                    );
                    underwater_overlay_from_forward(
                        forward,
                        effect.water_vision,
                        effect.effect_strength,
                    )
                });
                [overlay, overlay]
            }
            XrUnderwaterDetectionMode::PerEye => {
                let left_underwater = self.camera_inside_water(render_views[0].camera_position);
                let right_underwater = self.camera_inside_water(render_views[1].camera_position);
                let left_effect =
                    self.underwater_effects.eyes[0].update(left_underwater, dt_seconds);
                let right_effect =
                    self.underwater_effects.eyes[1].update(right_underwater, dt_seconds);
                [
                    left_underwater.then(|| {
                        underwater_overlay_from_forward(
                            render_views[0].camera_forward,
                            left_effect.water_vision,
                            left_effect.effect_strength,
                        )
                    }),
                    right_underwater.then(|| {
                        underwater_overlay_from_forward(
                            render_views[1].camera_forward,
                            right_effect.water_vision,
                            right_effect.effect_strength,
                        )
                    }),
                ]
            }
        }
    }

    /// Single-view underwater overlay for the flat (mono) view topology
    /// (tactical 168 Slice 3). Mirrors the stereo midpoint case in
    /// [`Self::underwater_overlays`] for one camera, reusing the shared midpoint
    /// effect state so the flat and stereo submerged transitions match.
    pub(crate) fn mono_underwater_overlay(
        &mut self,
        render_view: ChunkRenderView,
    ) -> Option<UnderwaterOverlay> {
        let dt_seconds = self.underwater_effect_dt_seconds();
        let underwater = self.camera_inside_water(render_view.camera_position);
        let effect = self
            .underwater_effects
            .midpoint
            .update(underwater, dt_seconds);
        underwater.then(|| {
            underwater_overlay_from_forward(
                render_view.camera_forward,
                effect.water_vision,
                effect.effect_strength,
            )
        })
    }

    pub(crate) fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(runtime) = &self.runtime else {
            return false;
        };
        let Some(state_id) = runtime.block_state_at_position(position) else {
            return false;
        };
        runtime.mesh_assets().catalog.occludes(state_id)
    }

    pub(crate) fn camera_inside_water(&self, position: Vec3) -> bool {
        self.runtime
            .as_ref()
            .is_some_and(|runtime| runtime.camera_inside_water(position))
    }
}
