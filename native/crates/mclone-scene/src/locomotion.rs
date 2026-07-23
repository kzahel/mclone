use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrLocomotionMode {
    #[default]
    HeadsetYaw,
    PlayerYaw,
}

impl XrLocomotionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::HeadsetYaw => "headset-yaw",
            Self::PlayerYaw => "player-yaw",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XrTurnPolicy {
    Snap { degrees: f32 },
    Smooth,
}

impl Default for XrTurnPolicy {
    fn default() -> Self {
        Self::Snap {
            degrees: XR_DEFAULT_SNAP_TURN_DEGREES,
        }
    }
}

impl XrTurnPolicy {
    pub const fn snap(degrees: f32) -> Self {
        Self::Snap { degrees }
    }

    pub fn game_mode(self) -> GameXrTurnMode {
        match self {
            Self::Snap { degrees } if (degrees - 30.0).abs() < f32::EPSILON => {
                GameXrTurnMode::Snap30
            }
            Self::Snap { degrees } if (degrees - 45.0).abs() < f32::EPSILON => {
                GameXrTurnMode::Snap45
            }
            Self::Snap { .. } => GameXrTurnMode::Snap15,
            Self::Smooth => GameXrTurnMode::Smooth,
        }
    }

    pub fn from_game_mode(mode: GameXrTurnMode) -> Self {
        match mode {
            GameXrTurnMode::Snap15 => Self::Snap { degrees: 15.0 },
            GameXrTurnMode::Snap30 => Self::Snap { degrees: 30.0 },
            GameXrTurnMode::Snap45 => Self::Snap { degrees: 45.0 },
            GameXrTurnMode::Smooth => Self::Smooth,
        }
    }

    fn snap_radians(self) -> Option<f64> {
        match self {
            Self::Snap { degrees } if degrees.is_finite() && degrees > 0.0 => {
                Some(f64::from(degrees).to_radians())
            }
            Self::Snap { .. } => Some(f64::from(XR_DEFAULT_SNAP_TURN_DEGREES).to_radians()),
            Self::Smooth => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrSnapTurnState {
    waiting_for_recenter: bool,
}

impl XrSnapTurnState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, axis_x: f32, policy: XrTurnPolicy) -> Option<f64> {
        let axis_x = if axis_x.is_finite() { axis_x } else { 0.0 };
        let magnitude = axis_x.abs();
        if self.waiting_for_recenter {
            if magnitude <= XR_SNAP_TURN_RECENTER_THRESHOLD {
                self.waiting_for_recenter = false;
            }
            return None;
        }
        if magnitude < XR_SNAP_TURN_ENGAGE_THRESHOLD {
            return None;
        }
        let snap_radians = policy.snap_radians()?;
        self.waiting_for_recenter = true;
        Some(-f64::from(axis_x.signum()) * snap_radians)
    }
}

pub fn xr_locomotion_input_from_controllers(
    input: &XrInputFrame,
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
) -> EngineCameraInput {
    xr_locomotion_input_from_controllers_with_turn_policy(
        input,
        dt_seconds,
        movement_yaw_radians,
        XrTurnPolicy::default(),
    )
}

pub fn xr_locomotion_input_from_controllers_with_turn_policy(
    input: &XrInputFrame,
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
    turn_policy: XrTurnPolicy,
) -> EngineCameraInput {
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    let actions = &input.actions;
    let jump = actions.held.contains(&PlayerAction::Jump);
    let descend = actions.held.contains(&PlayerAction::Descend);
    let sprint = actions.held.contains(&PlayerAction::Sprint);
    let shift = actions.held.contains(&PlayerAction::Sneak);
    let movement_impulse = (actions.movement.left.abs() > f32::EPSILON
        || actions.movement.forward.abs() > f32::EPSILON)
        .then(|| EngineCameraMovementImpulse::new(actions.movement.left, actions.movement.forward));
    let mouse_delta_x = if matches!(turn_policy, XrTurnPolicy::Smooth) {
        actions.look_rate.x as f64 * dt_seconds
    } else {
        0.0
    };

    // Spell every field explicitly: this is a gameplay-semantic constructor and a
    // bare `..default()` here is exactly how sprint/sneak were silently dropped
    // on both XR surfaces (tactical 168 Slice 0). New fields must fail to compile
    // rather than default to "off" on XR.
    EngineCameraInput {
        dt_seconds,
        mouse_delta_x,
        mouse_delta_y: 0.0,
        forward: false,
        backward: false,
        left: false,
        right: false,
        jump,
        descend,
        shift,
        sprint,
        movement_impulse,
        movement_yaw_radians,
        hand_push: None,
        hand_push_emulation: false,
        thruster: None,
        thruster_emulation: false,
    }
}

pub(crate) fn xr_left_stick_raw_axis(input: &XrInputFrame) -> Vec2 {
    input
        .xr_specific
        .controller(XrHand::Left)
        .map(|controller| {
            if controller.locomotion_axis.is_finite() {
                controller.locomotion_axis
            } else {
                Vec2::ZERO
            }
        })
        .unwrap_or(Vec2::ZERO)
}

pub(crate) fn xr_left_stick_blink_engaged(input: &XrInputFrame) -> bool {
    xr_left_stick_raw_axis(input).length() > XR_BLINK_TELEPORT_STICK_THRESHOLD
}

pub fn xr_hand_push_input_from_controllers(
    controllers: &[TrackedControllerState],
    views: &[XrView],
    transform: XrStageToWorld,
) -> Result<Option<EngineHandPushInput>> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let Some(left_hand_position) = xr_controller_hand_position(controllers, XrHand::Left) else {
        return Ok(None);
    };
    let Some(right_hand_position) = xr_controller_hand_position(controllers, XrHand::Right) else {
        return Ok(None);
    };
    let left_pose = views[0].pose;
    let right_pose = views[1].pose;
    let head_stage_position = (left_pose.position + right_pose.position) * 0.5;

    Ok(Some(EngineHandPushInput::new(
        vec3d_from_glam(transform.transform_position(head_stage_position)),
        vec3d_from_glam(transform.transform_position(left_hand_position)),
        vec3d_from_glam(transform.transform_position(right_hand_position)),
    )))
}

pub(crate) fn xr_controller_hand_position(
    controllers: &[TrackedControllerState],
    hand: XrHand,
) -> Option<Vec3> {
    controllers
        .iter()
        .find(|controller| controller.hand == hand)
        .and_then(|controller| controller.grip_position.or(controller.aim_position))
}

/// Per-hand thrust intent for the Iron Man / repulsor flight mode (tactical
/// 157). Derives each hand's world-space palm normal from the OpenXR grip pose
/// and reads the analog trigger as throttle. Missing hands / poses fall back to
/// `EngineThrusterHand::NONE`. The Slice 2 integrator consumes this; in Slice 1
/// the camera only retains it.
pub fn xr_thruster_input_from_controllers(
    input: &XrInputFrame,
    transform: XrStageToWorld,
) -> EngineThrusterInput {
    EngineThrusterInput::new(
        xr_thruster_hand(input, XrHand::Left, transform),
        xr_thruster_hand(input, XrHand::Right, transform),
    )
}

fn xr_thruster_hand(
    input: &XrInputFrame,
    hand: XrHand,
    transform: XrStageToWorld,
) -> EngineThrusterHand {
    let Some(controller) = input
        .tracked
        .iter()
        .find(|controller| controller.hand == hand)
    else {
        return EngineThrusterHand::NONE;
    };
    let throttle = input
        .xr_specific
        .controller(hand)
        .map_or(0.0, |controller| controller.pointer_select_value);
    let Some(world_palm_normal) = xr_thruster_world_palm_normal(controller, transform) else {
        return EngineThrusterHand::NONE;
    };
    EngineThrusterHand::new(vec3d_from_glam(world_palm_normal), throttle.clamp(0.0, 1.0))
}

/// World-space unit palm normal for one controller's grip pose (the direction
/// the palm repulsor fires), or `None` when the grip orientation is unavailable
/// this frame. Maps the profile's grip-local palm axis through the grip
/// orientation into stage space, then into world space. Shared by the thruster
/// input builder and the on-device calibration aid.
pub fn xr_thruster_world_palm_normal(
    controller: &TrackedControllerState,
    transform: XrStageToWorld,
) -> Option<Vec3> {
    let grip_orientation = controller.grip_orientation?;
    // grip-local palm axis -> stage space (via grip orientation) -> world space.
    let stage_palm_normal = grip_orientation * touch_palm_axis(controller.hand);
    Some(
        transform
            .transform_direction(stage_palm_normal)
            .normalize_or_zero(),
    )
}

/// Grip-local palm normal for the `oculus/touch_controller` interaction profile
/// (Quest 3 Touch Plus): the unit vector in grip space pointing OUT of the palm,
/// i.e. the direction a palm repulsor fires. The body accelerates along
/// `-palm_normal` (tactical 157).
///
/// Calibrated from the OpenXR / Windows-MR grip-pose convention: the grip
/// orientation's **Right (+X) axis** is "the ray normal to the palm — forward
/// from the LEFT palm, backward from the RIGHT palm" (OpenXR grip-pose spec / MS
/// motion-controllers doc). So the outward palm normal is grip-local **+X for the
/// left hand** and **-X for the right hand**.
///
/// Confirm on real hardware with the calibration aid: launch with
/// `MCLONE_THRUSTER_PALM_CAL=1`, enter Thruster mode, and hold the controller
/// palm-flat-down — the logged `world_palm_normal` should read ~ `(0, -1, 0)`. A
/// future Index/Vive/WMR profile adds its own constant here rather than reusing
/// this one.
const TOUCH_PALM_AXIS_LEFT: Vec3 = Vec3::X;
const TOUCH_PALM_AXIS_RIGHT: Vec3 = Vec3::NEG_X;

fn touch_palm_axis(hand: XrHand) -> Vec3 {
    match hand {
        XrHand::Left => TOUCH_PALM_AXIS_LEFT,
        XrHand::Right => TOUCH_PALM_AXIS_RIGHT,
    }
}

/// How often (in frames) the on-device palm-calibration aid emits a log line per
/// hand while enabled, to keep a manual calibration session readable.
const XR_THRUSTER_PALM_CAL_LOG_EVERY_FRAMES: u32 = 30;

/// True when on-device thruster palm-calibration logging is enabled via the
/// `MCLONE_THRUSTER_PALM_CAL` env var (checked once). See [`touch_palm_axis`].
fn thruster_palm_calibration_logging_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("MCLONE_THRUSTER_PALM_CAL").is_some())
}

/// Emit a throttled calibration line per hand (grip quaternion -> world palm
/// normal + throttle) so the grip-local palm axis can be confirmed on real
/// hardware per the [`touch_palm_axis`] procedure.
fn log_thruster_palm_calibration(input: &XrInputFrame, transform: XrStageToWorld) {
    static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if frame % XR_THRUSTER_PALM_CAL_LOG_EVERY_FRAMES != 0 {
        return;
    }
    for controller in &input.tracked {
        let Some(grip) = controller.grip_orientation else {
            continue;
        };
        let Some(normal) = xr_thruster_world_palm_normal(controller, transform) else {
            continue;
        };
        log::info!(
            "MCLONE_THRUSTER_PALM_CAL hand={:?} grip_quat=[{:.3},{:.3},{:.3},{:.3}] world_palm_normal=[{:.3},{:.3},{:.3}] throttle={:.3}",
            controller.hand,
            grip.x,
            grip.y,
            grip.z,
            grip.w,
            normal.x,
            normal.y,
            normal.z,
            input
                .xr_specific
                .controller(controller.hand)
                .map_or(0.0, |specific| specific.pointer_select_value)
                .clamp(0.0, 1.0),
        );
    }
}

pub fn xr_automated_flight_input(
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
) -> EngineCameraInput {
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    EngineCameraInput {
        dt_seconds,
        movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
        movement_yaw_radians,
        ..EngineCameraInput::default()
    }
}

pub fn xr_automated_orbit_input(
    dt_seconds: f64,
    speed_blocks_per_second: f64,
    elapsed_seconds: f64,
) -> EngineCameraInput {
    let angular_speed = if speed_blocks_per_second.is_finite()
        && speed_blocks_per_second > 0.0
        && XR_AUTOMATED_ORBIT_RADIUS_BLOCKS > 0.0
    {
        speed_blocks_per_second / XR_AUTOMATED_ORBIT_RADIUS_BLOCKS
    } else {
        0.0
    };
    let yaw = if elapsed_seconds.is_finite() {
        (elapsed_seconds.max(0.0) * angular_speed).rem_euclid(std::f64::consts::TAU)
    } else {
        0.0
    };
    xr_automated_flight_input(dt_seconds, Some(yaw))
}

pub(crate) fn game_movement_mode(mode: EngineCameraMovementMode) -> GameMovementMode {
    match mode {
        EngineCameraMovementMode::Walking => GameMovementMode::Walk,
        EngineCameraMovementMode::Fly => GameMovementMode::Fly,
        EngineCameraMovementMode::HandPush => GameMovementMode::HandPush,
        EngineCameraMovementMode::Thruster => GameMovementMode::Thruster,
    }
}

pub(crate) fn engine_movement_mode(mode: GameMovementMode) -> EngineCameraMovementMode {
    match mode {
        GameMovementMode::Walk => EngineCameraMovementMode::Walking,
        GameMovementMode::Fly => EngineCameraMovementMode::Fly,
        GameMovementMode::HandPush => EngineCameraMovementMode::HandPush,
        GameMovementMode::Thruster => EngineCameraMovementMode::Thruster,
    }
}

pub(crate) fn game_collision_mode(mode: EngineCameraCollisionMode) -> GameCollisionMode {
    match mode {
        EngineCameraCollisionMode::Normal => GameCollisionMode::Normal,
        EngineCameraCollisionMode::NoClip => GameCollisionMode::NoClip,
    }
}

pub(crate) fn engine_collision_mode(mode: GameCollisionMode) -> EngineCameraCollisionMode {
    match mode {
        GameCollisionMode::Normal => EngineCameraCollisionMode::Normal,
        GameCollisionMode::NoClip => EngineCameraCollisionMode::NoClip,
    }
}

pub(crate) fn engine_movement_yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some(horizontal.x.atan2(horizontal.z))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XrFrameLocomotionAutomation {
    Flight {
        speed_blocks_per_second: f64,
    },
    Orbit {
        speed_blocks_per_second: f64,
        elapsed_seconds: f64,
    },
    Stationary {
        frozen_render: bool,
    },
    ChunkViewChurn {
        center_x: i32,
        center_z: i32,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrFrameLocomotionOutcome {
    pub timing: XrLocomotionTiming,
    pub frozen_render: bool,
}

impl McloneSceneHost {
    pub fn apply_frame_locomotion(
        &mut self,
        input: &XrInputFrame,
        views: [XrView; 2],
        automation: Option<XrFrameLocomotionAutomation>,
    ) -> Result<XrFrameLocomotionOutcome> {
        let (mut timing, frozen_render) = match automation {
            Some(XrFrameLocomotionAutomation::Flight {
                speed_blocks_per_second,
            }) => (
                self.apply_automated_flight_input(views, speed_blocks_per_second)?,
                false,
            ),
            Some(XrFrameLocomotionAutomation::Orbit {
                speed_blocks_per_second,
                elapsed_seconds,
            }) => (
                self.apply_automated_orbit_input(speed_blocks_per_second, elapsed_seconds)?,
                false,
            ),
            Some(XrFrameLocomotionAutomation::Stationary { frozen_render }) => {
                (self.apply_automated_stationary_input(), frozen_render)
            }
            Some(XrFrameLocomotionAutomation::ChunkViewChurn { center_x, center_z }) => (
                self.apply_automated_chunk_view_churn(center_x, center_z)?,
                false,
            ),
            None => (self.apply_locomotion_input(input, views)?, false),
        };
        let commit_start = self.services.clock.now();
        if !self.embedded_world_activation.phase.active()
            && let Some((_, commit_timing)) = self
                .commit_engine_camera_player_pose_if_due_timed()
                .context("publish scheduled XR player pose")?
        {
            timing.commit_ms = elapsed_ms(self.services.clock.elapsed_since(commit_start));
            timing.record_commit_timing(commit_timing);
        }
        Ok(XrFrameLocomotionOutcome {
            timing,
            frozen_render,
        })
    }

    fn apply_locomotion_input(
        &mut self,
        input: &XrInputFrame,
        views: [XrView; 2],
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = self.services.clock.now();
        self.latest_xr_input = input.clone();
        self.latest_xr_head_gaze_stage = None;
        let now = self.services.clock.now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.saturating_duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        let ui_was_active = self.ui.is_active();
        self.apply_menu_toggle_input(&input.actions);
        self.apply_game_ui_toggle_input(&input.actions);
        let ui_active = self.ui.is_active();
        let gameplay_interaction_edges = XrGameplayInteractionEdges {
            attack: input.actions.pressed.contains(&PlayerAction::Attack),
            use_item: input.actions.pressed.contains(&PlayerAction::Use),
        };
        let suppress_gameplay_interaction = ui_was_active != ui_active;
        if self.active_world.runtime.is_none() {
            self.active_world.local_participant.reset_movement();
            self.head_comfort.reset();
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return Ok(timing);
        }
        let activation_was_active = self.embedded_world_activation.phase.active();
        self.advance_embedded_world_activation(dt_seconds);
        let mut transform = self
            .reconcile_room_scale_body_to_headset(&views)
            .context("reconcile XR room-scale body pose")?;
        let head_stage_position = xr_headset_stage_position_from_views(&views)?;
        let head_stage_forward = average_unit_direction(
            views[0].pose.orientation * Vec3::NEG_Z,
            views[1].pose.orientation * Vec3::NEG_Z,
            Vec3::NEG_Z,
        );
        if head_stage_forward.is_finite() && head_stage_forward.length_squared() > f32::EPSILON {
            self.latest_xr_head_gaze_stage = Some((head_stage_position, head_stage_forward));
        }
        self.update_head_comfort_state(transform, &views, dt_seconds)
            .context("update XR head comfort fade state")?;
        if self.ui.is_active() {
            self.active_world.local_participant.reset_movement();
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return Ok(timing);
        }
        if activation_was_active || self.embedded_world_activation.phase.active() {
            self.active_world.local_participant.reset_movement();
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            self.head_comfort.reset();
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return Ok(timing);
        }
        if let Some(yaw_delta_radians) = self.snap_turn_delta_from_controllers(input) {
            transform = self
                .apply_snap_turn_preserving_headset(&views, transform, yaw_delta_radians)
                .context("apply XR snap turn")?;
        }
        let blink_frame = self
            .update_xr_blink_teleport(input, &views, transform)
            .context("update XR Blink teleport")?;
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians_with_transform(&views, transform)
            .context("resolve XR locomotion frame")?;
        let mut input = xr_locomotion_input_from_controllers_with_turn_policy(
            input,
            dt_seconds,
            movement_yaw_radians,
            self.turn_policy,
        );
        if blink_frame.suppress_left_stick_movement {
            input.movement_impulse = None;
        }
        input.hand_push =
            xr_hand_push_input_from_controllers(&self.latest_xr_input.tracked, &views, transform)
                .context("resolve XR hand-push input")?;
        input.thruster = Some(xr_thruster_input_from_controllers(
            &self.latest_xr_input,
            transform,
        ));
        if thruster_palm_calibration_logging_enabled()
            && self.active_world.camera.movement_mode() == EngineCameraMovementMode::Thruster
        {
            log_thruster_palm_calibration(&self.latest_xr_input, transform);
        }
        timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
        let camera_apply_start = self.services.clock.now();
        let runtime = self
            .active_world
            .runtime
            .as_ref()
            .expect("runtime presence checked");
        let _ = self.active_world.local_participant.advance_movement(
            runtime.client(),
            input,
            dt_seconds,
        );
        timing.camera_apply_ms = elapsed_ms(self.services.clock.elapsed_since(camera_apply_start));
        self.play_landing_events();
        if !suppress_gameplay_interaction {
            if gameplay_interaction_edges.any() {
                let commit_start = self.services.clock.now();
                let (_, commit_timing) = self
                    .commit_engine_camera_player_pose_timed()
                    .context("publish XR player pose before interaction")?;
                timing.commit_ms = elapsed_ms(self.services.clock.elapsed_since(commit_start));
                timing.record_commit_timing(commit_timing);
            }
            let interaction_start = self.services.clock.now();
            self.apply_xr_gameplay_interaction_edges(gameplay_interaction_edges)?;
            timing.gameplay_interaction_ms =
                elapsed_ms(self.services.clock.elapsed_since(interaction_start));
        }
        Ok(timing)
    }

    fn apply_automated_flight_input(
        &mut self,
        views: [XrView; 2],
        speed_blocks_per_second: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = self.services.clock.now();
        self.latest_xr_input = XrInputFrame::default();
        self.latest_xr_head_gaze_stage = None;
        if self.active_world.local_startup.is_some() || self.active_world.runtime.is_none() {
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = self.services.clock.now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.saturating_duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.active_world
            .camera
            .set_movement_mode(EngineCameraMovementMode::Fly);
        self.active_world
            .camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.active_world
            .camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR automated flight locomotion frame")?;
        let input = xr_automated_flight_input(dt_seconds, movement_yaw_radians);
        timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
        let camera_apply_start = self.services.clock.now();
        let runtime = self
            .active_world
            .runtime
            .as_ref()
            .expect("runtime presence checked");
        let _ = self.active_world.local_participant.advance_movement(
            runtime.client(),
            input,
            dt_seconds,
        );
        timing.camera_apply_ms = elapsed_ms(self.services.clock.elapsed_since(camera_apply_start));
        Ok(timing)
    }

    fn apply_automated_orbit_input(
        &mut self,
        speed_blocks_per_second: f64,
        elapsed_seconds: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = self.services.clock.now();
        self.latest_xr_input = XrInputFrame::default();
        self.latest_xr_head_gaze_stage = None;
        if self.active_world.local_startup.is_some() || self.active_world.runtime.is_none() {
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = self.services.clock.now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.saturating_duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.active_world
            .camera
            .set_movement_mode(EngineCameraMovementMode::Fly);
        self.active_world
            .camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.active_world
            .camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let input = xr_automated_orbit_input(dt_seconds, speed_blocks_per_second, elapsed_seconds);
        timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
        let camera_apply_start = self.services.clock.now();
        let runtime = self
            .active_world
            .runtime
            .as_ref()
            .expect("runtime presence checked");
        let _ = self.active_world.local_participant.advance_movement(
            runtime.client(),
            input,
            dt_seconds,
        );
        timing.camera_apply_ms = elapsed_ms(self.services.clock.elapsed_since(camera_apply_start));
        Ok(timing)
    }

    fn apply_automated_stationary_input(&mut self) -> XrLocomotionTiming {
        let mut timing = XrLocomotionTiming::default();
        let input_start = self.services.clock.now();
        self.latest_xr_input = XrInputFrame::default();
        self.latest_xr_head_gaze_stage = None;
        if self.active_world.local_startup.is_some() || self.active_world.runtime.is_none() {
            timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
            return timing;
        }
        if self.active_world.scene.debug_ui_screen.is_none() {
            self.ui.close();
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = false;
        } else {
            self.apply_debug_ui_screen();
        }
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        self.last_locomotion_update = Some(self.services.clock.now());
        self.active_world.local_participant.reset_movement();
        timing.input_ms = elapsed_ms(self.services.clock.elapsed_since(input_start));
        timing
    }

    fn apply_automated_chunk_view_churn(
        &mut self,
        center_x: i32,
        center_z: i32,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = self.apply_automated_stationary_input();
        if self.active_world.local_startup.is_some() {
            return Ok(timing);
        }
        let Some(runtime) = self.active_world.runtime.as_mut() else {
            return Ok(timing);
        };
        let center = ChunkPos::new(center_x, center_z);
        let interest_start = self.services.clock.now();
        let (changed, command_timing) = runtime
            .set_interest_center_with_update_policy_timed(
                center,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .context("set XR automated chunk-view churn interest center")?;
        timing.commit_interest_ms = elapsed_ms(self.services.clock.elapsed_since(interest_start));
        timing.record_interest_command_timing(command_timing);
        if changed {
            log::info!(
                "MCLONE_ANDROID_XR_CHUNK_VIEW_CHURN center_x={} center_z={} changed=true",
                center.x,
                center.z
            );
        }
        Ok(timing)
    }

    pub(crate) fn snap_turn_delta_from_controllers(&mut self, input: &XrInputFrame) -> Option<f64> {
        let axis_x = input.actions.look_rate.x
            / mclone_input::ControllerSessionSettings::DEFAULT_LOOK_RATE_PER_SECOND;
        self.snap_turn_state.update(axis_x, self.turn_policy)
    }

    pub(crate) fn apply_snap_turn_preserving_headset(
        &mut self,
        views: &[XrView],
        before_transform: XrStageToWorld,
        yaw_delta_radians: f64,
    ) -> Result<XrStageToWorld> {
        if !yaw_delta_radians.is_finite() || yaw_delta_radians.abs() <= f64::EPSILON {
            return Ok(before_transform);
        }
        let Some(origin_before) = self.tracking_origin else {
            return Ok(before_transform);
        };
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_before = before_transform.transform_position(headset_stage_position);
        self.active_world.camera.turn_yaw_delta(yaw_delta_radians);
        let origin_after = origin_before.rebase_for_stage_position_world_position(
            headset_stage_position,
            headset_world_before,
            self.active_world.camera.snapshot(),
        )?;
        self.tracking_origin = Some(origin_after);
        XrStageToWorld::from_tracking_origin(origin_after, self.active_world.camera.snapshot())
    }

    pub(crate) fn locomotion_movement_yaw_radians(
        &mut self,
        views: &[XrView],
    ) -> Result<Option<f64>> {
        let tracking_origin = self.tracking_origin_for_views(views)?;
        let transform = XrStageToWorld::from_tracking_origin(
            tracking_origin,
            self.active_world.camera.snapshot(),
        )?;
        self.locomotion_movement_yaw_radians_with_transform(views, transform)
    }

    pub(crate) fn locomotion_movement_yaw_radians_with_transform(
        &self,
        views: &[XrView],
        transform: XrStageToWorld,
    ) -> Result<Option<f64>> {
        match self.locomotion_mode {
            XrLocomotionMode::PlayerYaw => Ok(None),
            XrLocomotionMode::HeadsetYaw => {
                xr_headset_world_yaw_from_views(views, transform).map(|yaw| Some(f64::from(yaw)))
            }
        }
    }
}

#[cfg(test)]
mod thruster_calibration_tests {
    use super::*;
    use mclone_input::XrSpecificInput;
    use std::f32::consts::FRAC_PI_2;

    fn identity_transform() -> XrStageToWorld {
        XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::ZERO,
            stage_to_world_rotation: Quat::IDENTITY,
        }
    }

    fn controller(hand: XrHand, grip_orientation: Option<Quat>) -> TrackedControllerState {
        TrackedControllerState {
            hand,
            aim_position: None,
            aim_direction: None,
            grip_position: None,
            grip_orientation,
        }
    }

    #[test]
    fn palm_down_grip_yields_downward_world_palm_normal_both_hands() {
        // Calibrated `TOUCH_PALM_AXIS` is grip-local +X (left) / -X (right). A
        // grip orientation that rotates that axis to world (0,-1,0) — i.e. the
        // "palm flat, facing down" calibration pose — must produce a world palm
        // normal of (0,-1,0), so the body thrusts straight up along -palm_normal.
        let left_grip = Quat::from_rotation_z(-FRAC_PI_2); // +X -> (0,-1,0)
        let left = controller(XrHand::Left, Some(left_grip));
        let left_normal =
            xr_thruster_world_palm_normal(&left, identity_transform()).expect("left palm normal");
        assert!(
            (left_normal - Vec3::NEG_Y).length() < 1.0e-5,
            "left world palm normal {left_normal:?} should be ~(0,-1,0)"
        );

        let right_grip = Quat::from_rotation_z(FRAC_PI_2); // -X -> (0,-1,0)
        let right = controller(XrHand::Right, Some(right_grip));
        let right_normal =
            xr_thruster_world_palm_normal(&right, identity_transform()).expect("right palm normal");
        assert!(
            (right_normal - Vec3::NEG_Y).length() < 1.0e-5,
            "right world palm normal {right_normal:?} should be ~(0,-1,0)"
        );
    }

    #[test]
    fn missing_grip_orientation_yields_no_palm_normal() {
        let hand = controller(XrHand::Left, None);
        assert!(xr_thruster_world_palm_normal(&hand, identity_transform()).is_none());
    }

    #[test]
    fn thruster_input_builder_carries_throttle_and_downward_thrust() {
        // Palm-down grip + full trigger -> the engine hand should thrust the body
        // upward (accel along -palm_normal, palm_normal ~ (0,-1,0)) at throttle 1.
        let frame = XrInputFrame {
            tracked: vec![
                controller(XrHand::Left, Some(Quat::from_rotation_z(-FRAC_PI_2))),
                controller(XrHand::Right, Some(Quat::from_rotation_z(FRAC_PI_2))),
            ],
            xr_specific: XrSpecificInput {
                controllers: vec![
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Left),
                        pointer_select_value: 0.75,
                        ..Default::default()
                    },
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Right),
                        pointer_select_value: 0.25,
                        ..Default::default()
                    },
                ],
            },
            ..Default::default()
        };
        let input = xr_thruster_input_from_controllers(&frame, identity_transform());
        assert!((input.left.throttle - 0.75).abs() < 1.0e-5);
        assert!((input.right.throttle - 0.25).abs() < 1.0e-5);
        assert!(
            input.left.palm_normal.y < -0.9,
            "left palm normal should point down, got {:?}",
            input.left.palm_normal
        );
        assert!(
            input.right.palm_normal.y < -0.9,
            "right palm normal should point down, got {:?}",
            input.right.palm_normal
        );
    }

    #[test]
    fn left_y_button_maps_to_sprint() {
        // Sprint is the left-hand Y button (the left thumbstick click is the
        // game-UI toggle). Tactical 168 Slice 0: sprint was previously dropped by
        // a `..default()` and unreachable on both XR surfaces.
        let mut frame = XrInputFrame::default();
        frame.actions.held.insert(PlayerAction::Sprint);
        let input = xr_locomotion_input_from_controllers(&frame, 0.016, None);
        assert!(input.sprint, "left Y button should engage sprint");
        assert!(!input.shift, "left Y button should not engage sneak");
    }

    #[test]
    fn right_thumbstick_click_maps_to_sneak() {
        // Sneak is the right thumbstick *click*, which does not collide with the
        // snap-turn axis on the same stick (tactical 168 Slice 0).
        let mut frame = XrInputFrame::default();
        frame.actions.held.insert(PlayerAction::Sneak);
        let input = xr_locomotion_input_from_controllers(&frame, 0.016, None);
        assert!(input.shift, "right thumbstick click should engage sneak");
        assert!(
            !input.sprint,
            "right thumbstick click should not engage sprint"
        );
    }

    #[test]
    fn left_thumbstick_click_is_not_sprint_or_sneak() {
        // Left thumbstick click is reserved for the game-UI toggle; it must not
        // leak into locomotion as sprint/sneak.
        let mut frame = XrInputFrame::default();
        frame.actions.held.insert(PlayerAction::OpenBlockPalette);
        let input = xr_locomotion_input_from_controllers(&frame, 0.016, None);
        assert!(!input.sprint);
        assert!(!input.shift);
    }
}
