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
    controllers: &[XrControllerSnapshot],
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
) -> EngineCameraInput {
    xr_locomotion_input_from_controllers_with_turn_policy(
        controllers,
        dt_seconds,
        movement_yaw_radians,
        XrTurnPolicy::default(),
    )
}

pub fn xr_locomotion_input_from_controllers_with_turn_policy(
    controllers: &[XrControllerSnapshot],
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
    turn_policy: XrTurnPolicy,
) -> EngineCameraInput {
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    let left_axis = xr_left_stick_axis(controllers);
    let right_axis = xr_right_stick_axis(controllers);
    let jump = xr_right_a_pressed(controllers);
    let descend = xr_right_b_pressed(controllers);
    let movement_impulse = (left_axis.length_squared() > f32::EPSILON)
        .then(|| xr_left_stick_movement_impulse(left_axis));
    let mouse_delta_x =
        if matches!(turn_policy, XrTurnPolicy::Smooth) && ENGINE_CAMERA_MOUSE_SENSITIVITY > 0.0 {
            let yaw_delta =
                -f64::from(right_axis.x) * XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND * dt_seconds;
            -yaw_delta / ENGINE_CAMERA_MOUSE_SENSITIVITY
        } else {
            0.0
        };

    EngineCameraInput {
        dt_seconds,
        mouse_delta_x,
        jump,
        descend,
        movement_impulse,
        movement_yaw_radians,
        ..EngineCameraInput::default()
    }
}

pub(crate) fn xr_left_stick_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO)
}

pub(crate) fn xr_left_stick_raw_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)
        .map(|controller| {
            if controller.thumbstick.is_finite() {
                controller.thumbstick
            } else {
                Vec2::ZERO
            }
        })
        .unwrap_or(Vec2::ZERO)
}

pub(crate) fn xr_left_stick_blink_engaged(controllers: &[XrControllerSnapshot]) -> bool {
    xr_left_stick_raw_axis(controllers).length() > XR_BLINK_TELEPORT_STICK_THRESHOLD
}

pub(crate) fn xr_right_stick_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Right)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO)
}

pub(crate) fn xr_right_a_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XrHand::Right && controller.a_pressed)
}

pub(crate) fn xr_right_b_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XrHand::Right && controller.b_pressed)
}

pub fn xr_hand_push_input_from_controllers(
    controllers: &[XrControllerSnapshot],
    views: &[xr::View],
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
    let left_pose = mclone_xr_host::view_pose(&views[0])?;
    let right_pose = mclone_xr_host::view_pose(&views[1])?;
    let head_stage_position = (left_pose.position + right_pose.position) * 0.5;

    Ok(Some(EngineHandPushInput::new(
        vec3d_from_glam(transform.transform_position(head_stage_position)),
        vec3d_from_glam(transform.transform_position(left_hand_position)),
        vec3d_from_glam(transform.transform_position(right_hand_position)),
    )))
}

pub(crate) fn xr_controller_hand_position(
    controllers: &[XrControllerSnapshot],
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
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
) -> EngineThrusterInput {
    EngineThrusterInput::new(
        xr_thruster_hand(controllers, XrHand::Left, transform),
        xr_thruster_hand(controllers, XrHand::Right, transform),
    )
}

fn xr_thruster_hand(
    controllers: &[XrControllerSnapshot],
    hand: XrHand,
    transform: XrStageToWorld,
) -> EngineThrusterHand {
    let Some(controller) = controllers.iter().find(|c| c.hand == hand) else {
        return EngineThrusterHand::NONE;
    };
    let Some(world_palm_normal) = xr_thruster_world_palm_normal(controller, transform) else {
        return EngineThrusterHand::NONE;
    };
    EngineThrusterHand::new(
        vec3d_from_glam(world_palm_normal),
        controller.trigger.clamp(0.0, 1.0),
    )
}

/// World-space unit palm normal for one controller's grip pose (the direction
/// the palm repulsor fires), or `None` when the grip orientation is unavailable
/// this frame. Maps the profile's grip-local palm axis through the grip
/// orientation into stage space, then into world space. Shared by the thruster
/// input builder and the on-device calibration aid.
pub fn xr_thruster_world_palm_normal(
    controller: &XrControllerSnapshot,
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
fn log_thruster_palm_calibration(controllers: &[XrControllerSnapshot], transform: XrStageToWorld) {
    static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if frame % XR_THRUSTER_PALM_CAL_LOG_EVERY_FRAMES != 0 {
        return;
    }
    for controller in controllers {
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
            controller.trigger.clamp(0.0, 1.0),
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

pub(crate) fn xr_left_stick_movement_impulse(axis: Vec2) -> EngineCameraMovementImpulse {
    EngineCameraMovementImpulse::new(-axis.x, axis.y)
}

pub(crate) fn joypad_axis_after_dead_zone(axis: Vec2) -> Vec2 {
    if !axis.is_finite() {
        return Vec2::ZERO;
    }
    let length = axis.length();
    if length <= XR_JOYPAD_DEAD_ZONE {
        return Vec2::ZERO;
    }
    let normalized = axis / length;
    let adjusted = ((length.min(1.0) - XR_JOYPAD_DEAD_ZONE) / (1.0 - XR_JOYPAD_DEAD_ZONE)).max(0.0);
    normalized * adjusted
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn apply_locomotion_input(
        &mut self,
        controllers: &[XrControllerSnapshot],
        views: [xr::View; 2],
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        self.latest_controllers.extend_from_slice(controllers);
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        let ui_was_active = self.ui.is_active();
        self.apply_menu_toggle_input(controllers);
        self.apply_game_ui_toggle_input(controllers);
        let ui_active = self.ui.is_active();
        let gameplay_interaction_edges = self.update_gameplay_interaction_buttons(controllers);
        let suppress_gameplay_interaction = ui_was_active != ui_active;
        if self.runtime.is_none() {
            self.head_comfort.reset();
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        let mut transform = self
            .reconcile_room_scale_body_to_headset(&views)
            .context("reconcile XR room-scale body pose")?;
        self.update_head_comfort_state(transform, &views, dt_seconds)
            .context("update XR head comfort fade state")?;
        if self.ui.is_active() {
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(input_start.elapsed());
            let commit_start = Instant::now();
            let (_, commit_timing) = self
                .commit_engine_camera_player_pose_timed()
                .context("sync XR room-scale player pose")?;
            timing.commit_ms = elapsed_ms(commit_start.elapsed());
            timing.record_commit_timing(commit_timing);
            return Ok(timing);
        }
        if let Some(yaw_delta_radians) = self.snap_turn_delta_from_controllers(controllers) {
            transform = self
                .apply_snap_turn_preserving_headset(&views, transform, yaw_delta_radians)
                .context("apply XR snap turn")?;
        }
        let blink_frame = self
            .update_xr_blink_teleport(controllers, &views, transform)
            .context("update XR Blink teleport")?;
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians_with_transform(&views, transform)
            .context("resolve XR locomotion frame")?;
        let mut input = xr_locomotion_input_from_controllers_with_turn_policy(
            controllers,
            dt_seconds,
            movement_yaw_radians,
            self.turn_policy,
        );
        if blink_frame.suppress_left_stick_movement {
            input.movement_impulse = None;
        }
        input.hand_push = xr_hand_push_input_from_controllers(controllers, &views, transform)
            .context("resolve XR hand-push input")?;
        input.thruster = Some(xr_thruster_input_from_controllers(controllers, transform));
        if thruster_palm_calibration_logging_enabled()
            && self.camera.movement_mode() == EngineCameraMovementMode::Thruster
        {
            log_thruster_palm_calibration(controllers, transform);
        }
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        self.play_landing_events();
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR locomotion player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        if !suppress_gameplay_interaction {
            let interaction_start = Instant::now();
            self.apply_xr_gameplay_interaction_edges(gameplay_interaction_edges)?;
            timing.gameplay_interaction_ms = elapsed_ms(interaction_start.elapsed());
        }
        Ok(timing)
    }

    pub fn apply_automated_flight_input(
        &mut self,
        views: [xr::View; 2],
        speed_blocks_per_second: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.gameplay_interaction_buttons = XrGameplayInteractionButtons::default();
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.camera.set_movement_mode(EngineCameraMovementMode::Fly);
        self.camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR automated flight locomotion frame")?;
        let input = xr_automated_flight_input(dt_seconds, movement_yaw_radians);
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR automated flight player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        Ok(timing)
    }

    pub fn apply_automated_orbit_input(
        &mut self,
        speed_blocks_per_second: f64,
        elapsed_seconds: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.gameplay_interaction_buttons = XrGameplayInteractionButtons::default();
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.camera.set_movement_mode(EngineCameraMovementMode::Fly);
        self.camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let input = xr_automated_orbit_input(dt_seconds, speed_blocks_per_second, elapsed_seconds);
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR automated orbit player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        Ok(timing)
    }

    pub fn apply_automated_stationary_input(&mut self) -> XrLocomotionTiming {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return timing;
        }
        if self.scene.debug_ui_screen.is_none() {
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
        self.last_locomotion_update = Some(Instant::now());
        timing.input_ms = elapsed_ms(input_start.elapsed());
        timing
    }

    pub fn apply_automated_chunk_view_churn(
        &mut self,
        center_x: i32,
        center_z: i32,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = self.apply_automated_stationary_input();
        if self.local_startup.is_some() {
            return Ok(timing);
        }
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(timing);
        };
        let center = ChunkPos::new(center_x, center_z);
        let interest_start = Instant::now();
        let (changed, command_timing) = runtime
            .set_interest_center_with_update_policy_timed(
                center,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .context("set XR automated chunk-view churn interest center")?;
        timing.commit_interest_ms = elapsed_ms(interest_start.elapsed());
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

    pub(crate) fn snap_turn_delta_from_controllers(
        &mut self,
        controllers: &[XrControllerSnapshot],
    ) -> Option<f64> {
        let right_axis = xr_right_stick_axis(controllers);
        self.snap_turn_state.update(right_axis.x, self.turn_policy)
    }

    pub(crate) fn apply_snap_turn_preserving_headset(
        &mut self,
        views: &[xr::View],
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
        self.camera.turn_yaw_delta(yaw_delta_radians);
        let origin_after = origin_before.rebase_for_stage_position_world_position(
            headset_stage_position,
            headset_world_before,
            self.camera.snapshot(),
        )?;
        self.tracking_origin = Some(origin_after);
        XrStageToWorld::from_tracking_origin(origin_after, self.camera.snapshot())
    }

    pub(crate) fn locomotion_movement_yaw_radians(
        &mut self,
        views: &[xr::View],
    ) -> Result<Option<f64>> {
        let tracking_origin = self.tracking_origin_for_views(views)?;
        let transform =
            XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
        self.locomotion_movement_yaw_radians_with_transform(views, transform)
    }

    pub(crate) fn locomotion_movement_yaw_radians_with_transform(
        &self,
        views: &[xr::View],
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
    use std::f32::consts::FRAC_PI_2;

    fn identity_transform() -> XrStageToWorld {
        XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::ZERO,
            stage_to_world_rotation: Quat::IDENTITY,
        }
    }

    fn controller(
        hand: XrHand,
        grip_orientation: Option<Quat>,
        trigger: f32,
    ) -> XrControllerSnapshot {
        XrControllerSnapshot {
            hand,
            aim_position: None,
            aim_direction: None,
            grip_position: None,
            grip_orientation,
            trigger,
            squeeze: 0.0,
            select_pressed: false,
            a_pressed: false,
            b_pressed: false,
            y_pressed: false,
            thumbstick: Vec2::ZERO,
            thumbstick_pressed: false,
        }
    }

    #[test]
    fn palm_down_grip_yields_downward_world_palm_normal_both_hands() {
        // Calibrated `TOUCH_PALM_AXIS` is grip-local +X (left) / -X (right). A
        // grip orientation that rotates that axis to world (0,-1,0) — i.e. the
        // "palm flat, facing down" calibration pose — must produce a world palm
        // normal of (0,-1,0), so the body thrusts straight up along -palm_normal.
        let left_grip = Quat::from_rotation_z(-FRAC_PI_2); // +X -> (0,-1,0)
        let left = controller(XrHand::Left, Some(left_grip), 1.0);
        let left_normal =
            xr_thruster_world_palm_normal(&left, identity_transform()).expect("left palm normal");
        assert!(
            (left_normal - Vec3::NEG_Y).length() < 1.0e-5,
            "left world palm normal {left_normal:?} should be ~(0,-1,0)"
        );

        let right_grip = Quat::from_rotation_z(FRAC_PI_2); // -X -> (0,-1,0)
        let right = controller(XrHand::Right, Some(right_grip), 1.0);
        let right_normal =
            xr_thruster_world_palm_normal(&right, identity_transform()).expect("right palm normal");
        assert!(
            (right_normal - Vec3::NEG_Y).length() < 1.0e-5,
            "right world palm normal {right_normal:?} should be ~(0,-1,0)"
        );
    }

    #[test]
    fn missing_grip_orientation_yields_no_palm_normal() {
        let hand = controller(XrHand::Left, None, 1.0);
        assert!(xr_thruster_world_palm_normal(&hand, identity_transform()).is_none());
    }

    #[test]
    fn thruster_input_builder_carries_throttle_and_downward_thrust() {
        // Palm-down grip + full trigger -> the engine hand should thrust the body
        // upward (accel along -palm_normal, palm_normal ~ (0,-1,0)) at throttle 1.
        let controllers = [
            controller(XrHand::Left, Some(Quat::from_rotation_z(-FRAC_PI_2)), 0.75),
            controller(XrHand::Right, Some(Quat::from_rotation_z(FRAC_PI_2)), 0.25),
        ];
        let input = xr_thruster_input_from_controllers(&controllers, identity_transform());
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
}
