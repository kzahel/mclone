use super::*;

pub const ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND: f64 = 32.0;
pub const ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND: f64 = 2.0;
pub const ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND: f64 = 256.0;
/// Fly-speed range exposed to the in-game menu, expressed as a multiplier of the
/// base speed. The range is symmetric in log space so a 1.0x multiplier sits at
/// the slider's midpoint.
pub const ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER: f64 = 0.125;
pub const ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER: f64 = 8.0;
pub const ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER: f64 = 1.0;
pub const ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER: f64 = 0.125;
pub const ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER: f64 = 8.0;
pub const ENGINE_CAMERA_MOUSE_SENSITIVITY: f64 = 0.0035;
pub const ENGINE_CAMERA_SPAWN_Y: f64 = 104.0;
pub const ENGINE_CAMERA_SPAWN_YAW_RADIANS: f64 = 0.55;
pub const ENGINE_CAMERA_SPAWN_PITCH_RADIANS: f64 = -0.35;
const ENGINE_HAND_PUSH_EMULATION_CYCLE_HZ: f64 = 1.8;
const ENGINE_HAND_PUSH_EMULATION_HAND_SPACING: f64 = 0.34;
const ENGINE_HAND_PUSH_EMULATION_FORWARD_REACH: f64 = 0.32;
const ENGINE_HAND_PUSH_EMULATION_STROKE: f64 = 0.36;
const ENGINE_HAND_PUSH_EMULATION_LIFT: f64 = 0.16;
pub(crate) const ENGINE_DEBUG_HAND_SPHERE_SEGMENTS: usize = 24;
const ENGINE_DEBUG_PLAYER_BOX_COLOR: [f32; 4] = [0.1, 0.95, 0.65, 0.95];
const ENGINE_DEBUG_LEFT_HAND_COLOR: [f32; 4] = [0.2, 0.55, 1.0, 0.95];
const ENGINE_DEBUG_RIGHT_HAND_COLOR: [f32; 4] = [1.0, 0.62, 0.18, 0.95];
const ENGINE_DEBUG_HEADSET_RESIDUAL_COLOR: [f32; 4] = [1.0, 0.15, 0.15, 0.95];
const ENGINE_ROOM_SCALE_BODY_FOLLOW_ENTER_METERS: f64 = 0.03;
const ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS: f64 = 0.01;
const ENGINE_ROOM_SCALE_BLOCKED_RETRY_METERS: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraInput {
    pub dt_seconds: f64,
    pub mouse_delta_x: f64,
    pub mouse_delta_y: f64,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub descend: bool,
    pub shift: bool,
    pub sprint: bool,
    pub movement_impulse: Option<EngineCameraMovementImpulse>,
    pub movement_yaw_radians: Option<f64>,
    pub hand_push: Option<EngineHandPushInput>,
    pub hand_push_emulation: bool,
    pub thruster: Option<EngineThrusterInput>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraMovementImpulse {
    pub left: f32,
    pub forward: f32,
}

impl EngineCameraMovementImpulse {
    pub fn new(left: f32, forward: f32) -> Self {
        Self { left, forward }
    }

    fn as_player_impulse(self) -> (f32, f32) {
        (self.left, self.forward)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineHandPushInput {
    pub head_position: Vec3d,
    pub left_hand_position: Vec3d,
    pub right_hand_position: Vec3d,
}

impl EngineHandPushInput {
    pub fn new(
        head_position: Vec3d,
        left_hand_position: Vec3d,
        right_hand_position: Vec3d,
    ) -> Self {
        Self {
            head_position,
            left_hand_position,
            right_hand_position,
        }
    }

    fn pose(self) -> HandPushPose {
        HandPushPose {
            head_position: self.head_position,
            left_hand_position: self.left_hand_position,
            right_hand_position: self.right_hand_position,
        }
    }
}

/// Per-hand thrust intent for the Iron Man / repulsor flight mode (tactical 157).
///
/// `palm_normal` is a unit world-space vector pointing out of the palm (the
/// direction the jet fires); the Slice 2 integrator pushes the body along
/// `-palm_normal`. It is zero when the hand pose is unavailable this frame.
/// `throttle` is the analog trigger value in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineThrusterHand {
    pub palm_normal: Vec3d,
    pub throttle: f32,
}

impl EngineThrusterHand {
    pub const NONE: Self = Self {
        palm_normal: Vec3d::ZERO,
        throttle: 0.0,
    };

    pub fn new(palm_normal: Vec3d, throttle: f32) -> Self {
        Self {
            palm_normal,
            throttle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineThrusterInput {
    pub left: EngineThrusterHand,
    pub right: EngineThrusterHand,
}

impl EngineThrusterInput {
    pub fn new(left: EngineThrusterHand, right: EngineThrusterHand) -> Self {
        Self { left, right }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineRoomScaleReconciliation {
    pub body_eye_before: Vec3d,
    pub headset_world_position: Vec3d,
    pub requested_body_movement: Vec3d,
    pub consumed_body_movement: Vec3d,
    pub residual_head_offset: Vec3d,
    pub body_eye_after: Vec3d,
    pub collision: CollisionMovementResult,
}

impl EngineRoomScaleReconciliation {
    fn no_op(body_eye: Vec3d, headset_world_position: Vec3d) -> Self {
        Self {
            body_eye_before: body_eye,
            headset_world_position,
            requested_body_movement: Vec3d::ZERO,
            consumed_body_movement: Vec3d::ZERO,
            residual_head_offset: headset_world_position.subtract(body_eye),
            body_eye_after: body_eye,
            collision: CollisionMovementResult::default(),
        }
    }

    pub fn residual_horizontal_length_sqr(self) -> f64 {
        self.residual_head_offset.x * self.residual_head_offset.x
            + self.residual_head_offset.z * self.residual_head_offset.z
    }
}

impl Default for EngineCameraInput {
    fn default() -> Self {
        Self {
            dt_seconds: 0.0,
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            forward: false,
            backward: false,
            left: false,
            right: false,
            jump: false,
            descend: false,
            shift: false,
            sprint: false,
            movement_impulse: None,
            movement_yaw_radians: None,
            hand_push: None,
            hand_push_emulation: false,
            thruster: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EngineCameraMovementMode {
    #[default]
    Walking,
    Fly,
    HandPush,
    /// Iron Man / repulsor hand-thruster flight (tactical 157). Gravity-bound
    /// like `HandPush`; unlike `Fly` it must not force `NoClip` collision.
    Thruster,
}

impl EngineCameraMovementMode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Walking => Self::Fly,
            Self::Fly => Self::HandPush,
            Self::HandPush => Self::Thruster,
            Self::Thruster => Self::Walking,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Walking => "WALK",
            Self::Fly => "FLY",
            Self::HandPush => "HAND",
            Self::Thruster => "THRUST",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EngineCameraCollisionMode {
    #[default]
    Normal,
    NoClip,
}

impl EngineCameraCollisionMode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Normal => Self::NoClip,
            Self::NoClip => Self::Normal,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::NoClip => "NOCLIP",
        }
    }
}

fn player_dimensions_for_movement_mode(
    movement_mode: EngineCameraMovementMode,
) -> LocalPlayerDimensions {
    match movement_mode {
        EngineCameraMovementMode::HandPush => LocalPlayerDimensions::HAND_PUSH,
        EngineCameraMovementMode::Walking
        | EngineCameraMovementMode::Fly
        | EngineCameraMovementMode::Thruster => LocalPlayerDimensions::STANDING,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EngineCameraViewMode {
    #[default]
    FirstPerson,
    ThirdPersonBack,
}

impl EngineCameraViewMode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPersonBack,
            Self::ThirdPersonBack => Self::FirstPerson,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::FirstPerson => "FIRST_PERSON",
            Self::ThirdPersonBack => "THIRD_PERSON",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "first" | "first-person" | "first_person" | "1p" => Some(Self::FirstPerson),
            "third" | "third-person" | "third_person" | "third-person-back"
            | "third_person_back" | "3p" => Some(Self::ThirdPersonBack),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraSnapshot {
    pub eye: Vec3d,
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub speed_blocks_per_second: f64,
    pub chunk_pos: ChunkPos,
}

impl EngineCameraSnapshot {
    pub fn from_eye_pose(
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) -> Self {
        Self {
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
            chunk_pos: ChunkPos::new(
                block_to_chunk_coord(eye.x.floor() as i32),
                block_to_chunk_coord(eye.z.floor() as i32),
            ),
        }
    }

    pub fn from_player(player: &LocalPlayerController, speed_blocks_per_second: f64) -> Self {
        let pose = player.pose();
        Self {
            eye: pose.eye_position(),
            yaw_radians: pose.native_yaw_radians(),
            pitch_radians: pose.native_pitch_radians(),
            speed_blocks_per_second,
            chunk_pos: pose.chunk_pos(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraFrameState {
    pub camera: EngineCameraSnapshot,
    pub movement_mode: EngineCameraMovementMode,
    pub collision_mode: EngineCameraCollisionMode,
    pub view_mode: EngineCameraViewMode,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub selected_hotbar_slot: u8,
}

impl EngineCameraFrameState {
    pub fn from_player(
        player: &LocalPlayerController,
        movement_mode: EngineCameraMovementMode,
        collision_mode: EngineCameraCollisionMode,
        view_mode: EngineCameraViewMode,
        speed_blocks_per_second: f64,
        selected_hotbar_slot: u8,
    ) -> Self {
        Self {
            camera: EngineCameraSnapshot::from_player(player, speed_blocks_per_second),
            movement_mode,
            collision_mode,
            view_mode,
            on_ground: player.on_ground(),
            horizontal_collision: player.horizontal_collision(),
            vertical_collision: player.vertical_collision(),
            selected_hotbar_slot,
        }
    }

    pub const fn movement_mode_label(&self) -> &'static str {
        self.movement_mode.label()
    }

    pub const fn collision_mode_label(&self) -> &'static str {
        self.collision_mode.label()
    }

    pub const fn view_mode_label(&self) -> &'static str {
        self.view_mode.label()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnginePoseSyncCommandKind {
    Movement,
    CorrectionResync,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnginePoseSyncCommand {
    pub kind: EnginePoseSyncCommandKind,
    pub command: ClientCommand,
    pub camera: EngineCameraSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnginePoseCorrectionAcceptance {
    pub update: PlayerPositionUpdate,
    pub accept_command: ClientCommand,
    pub feet_position: Vec3d,
    pub camera: EngineCameraSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingEvent {
    pub impact_speed: f64,
    pub position: Vec3d,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineCameraController {
    pub(crate) player: LocalPlayerController,
    hand_push: HandPushLocomotionController,
    last_hand_push_input: Option<EngineHandPushInput>,
    last_thruster_input: Option<EngineThrusterInput>,
    last_room_scale_reconciliation: Option<EngineRoomScaleReconciliation>,
    room_scale_body_follow_active: bool,
    room_scale_blocked_residual: Option<Vec3d>,
    movement_mode: EngineCameraMovementMode,
    collision_mode: EngineCameraCollisionMode,
    view_mode: EngineCameraViewMode,
    first_person_player_visible: bool,
    speed_blocks_per_second: f64,
    movement_speed_multiplier: f64,
    landing_events: Vec<LandingEvent>,
    hand_push_emulation_phase: f64,
}

impl EngineCameraController {
    pub fn spawn_for_chunk(center: ChunkPos) -> Self {
        let eye = Vec3d::new(
            f64::from(chunk_middle_block_coord(center.x)),
            ENGINE_CAMERA_SPAWN_Y,
            f64::from(chunk_middle_block_coord(center.z)),
        );
        Self::from_eye_pose(
            eye,
            ENGINE_CAMERA_SPAWN_YAW_RADIANS,
            ENGINE_CAMERA_SPAWN_PITCH_RADIANS,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        )
    }

    pub fn from_eye_pose(
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) -> Self {
        let mut player = LocalPlayerController::new();
        player.set_pose(LocalPlayerPose::from_eye_position(
            eye,
            -yaw_radians.to_degrees(),
            -pitch_radians.to_degrees(),
            LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ));
        Self {
            player,
            hand_push: HandPushLocomotionController::default(),
            last_hand_push_input: None,
            last_thruster_input: None,
            last_room_scale_reconciliation: None,
            room_scale_body_follow_active: false,
            room_scale_blocked_residual: None,
            movement_mode: EngineCameraMovementMode::Walking,
            collision_mode: EngineCameraCollisionMode::Normal,
            view_mode: EngineCameraViewMode::FirstPerson,
            first_person_player_visible: false,
            speed_blocks_per_second: clamp_camera_speed(speed_blocks_per_second),
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER,
            landing_events: Vec::new(),
            hand_push_emulation_phase: 0.0,
        }
    }

    pub const fn player(&self) -> &LocalPlayerController {
        &self.player
    }

    pub const fn last_hand_push_input(&self) -> Option<EngineHandPushInput> {
        self.last_hand_push_input
    }

    pub const fn last_thruster_input(&self) -> Option<EngineThrusterInput> {
        self.last_thruster_input
    }

    pub const fn last_room_scale_reconciliation(&self) -> Option<EngineRoomScaleReconciliation> {
        self.last_room_scale_reconciliation
    }

    fn reset_room_scale_body_follow(&mut self) {
        self.room_scale_body_follow_active = false;
        self.room_scale_blocked_residual = None;
        self.last_room_scale_reconciliation = None;
    }

    pub fn set_eye_pose(&mut self, eye: Vec3d, yaw_radians: f64, pitch_radians: f64) {
        let dimensions = self.player.dimensions();
        self.player.set_pose(LocalPlayerPose::from_eye_position(
            eye,
            -yaw_radians.to_degrees(),
            -pitch_radians.to_degrees(),
            dimensions.eye_height,
        ));
        self.reset_room_scale_body_follow();
    }

    pub fn set_player_feet_pose(
        &mut self,
        feet_position: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
    ) {
        let current_pose = self.player.pose();
        self.player.set_pose(LocalPlayerPose {
            position: feet_position,
            y_rot_degrees: -yaw_radians.to_degrees(),
            x_rot_degrees: -pitch_radians.to_degrees(),
            eye_height: current_pose.eye_height,
        });
        self.player.clear_delta_movement();
        self.hand_push.reset();
        self.last_hand_push_input = None;
        self.last_thruster_input = None;
        self.reset_room_scale_body_follow();
        self.hand_push_emulation_phase = 0.0;
    }

    pub const fn movement_mode(&self) -> EngineCameraMovementMode {
        self.movement_mode
    }

    pub const fn collision_mode(&self) -> EngineCameraCollisionMode {
        self.collision_mode
    }

    pub const fn view_mode(&self) -> EngineCameraViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: EngineCameraViewMode) {
        self.view_mode = view_mode;
    }

    pub fn toggle_view_mode(&mut self) -> EngineCameraViewMode {
        self.set_view_mode(self.view_mode.toggled());
        self.view_mode
    }

    pub const fn first_person_player_visible(&self) -> bool {
        self.first_person_player_visible
    }

    pub fn set_first_person_player_visible(&mut self, visible: bool) {
        self.first_person_player_visible = visible;
    }

    pub fn set_movement_mode(&mut self, movement_mode: EngineCameraMovementMode) {
        if self.movement_mode != movement_mode {
            self.player.clear_delta_movement();
            self.hand_push.reset();
            self.last_hand_push_input = None;
            self.last_thruster_input = None;
            self.reset_room_scale_body_follow();
            self.hand_push_emulation_phase = 0.0;
        }
        self.movement_mode = movement_mode;
        self.set_player_dimensions_for_movement_mode();
        // HandPush is always collision-backed; Thruster defaults to Normal on
        // entry (never forces NoClip like Fly does) but leaves NoClip selectable
        // afterward through the independent Collision axis (tactical 157).
        if matches!(
            self.movement_mode,
            EngineCameraMovementMode::HandPush | EngineCameraMovementMode::Thruster
        ) {
            self.set_collision_mode(EngineCameraCollisionMode::Normal);
        }
    }

    fn set_player_dimensions_for_movement_mode(&mut self) {
        let dimensions = player_dimensions_for_movement_mode(self.movement_mode);
        if self.player.dimensions() != dimensions {
            self.player.set_dimensions(dimensions);
        }
    }

    pub fn toggle_movement_mode(&mut self) -> EngineCameraMovementMode {
        let movement_mode = self.movement_mode.toggled();
        self.set_movement_mode(movement_mode);
        if movement_mode == EngineCameraMovementMode::Fly {
            self.set_collision_mode(EngineCameraCollisionMode::NoClip);
        }
        self.movement_mode
    }

    pub fn set_collision_mode(&mut self, collision_mode: EngineCameraCollisionMode) {
        let collision_mode = if self.movement_mode == EngineCameraMovementMode::HandPush {
            EngineCameraCollisionMode::Normal
        } else {
            collision_mode
        };
        if self.collision_mode != collision_mode {
            self.player.clear_delta_movement();
            self.hand_push.reset();
            self.last_hand_push_input = None;
            self.last_thruster_input = None;
            self.reset_room_scale_body_follow();
            self.hand_push_emulation_phase = 0.0;
        }
        self.collision_mode = collision_mode;
    }

    pub fn toggle_collision_mode(&mut self) -> EngineCameraCollisionMode {
        self.set_collision_mode(self.collision_mode.toggled());
        self.collision_mode
    }

    pub const fn on_ground(&self) -> bool {
        self.player.on_ground()
    }

    pub const fn horizontal_collision(&self) -> bool {
        self.player.horizontal_collision()
    }

    pub const fn vertical_collision(&self) -> bool {
        self.player.vertical_collision()
    }

    pub fn next_move_player_command(&mut self) -> Option<ClientCommand> {
        self.player.next_move_player_command()
    }

    pub fn pos_rot_move_player_command(&mut self) -> ClientCommand {
        self.player.pos_rot_move_player_command()
    }

    pub fn take_landing_events(&mut self) -> Vec<LandingEvent> {
        std::mem::take(&mut self.landing_events)
    }

    pub fn apply_player_position_update(&mut self, update: PlayerPositionUpdate) -> ClientCommand {
        let command = self.player.apply_player_position_update(update);
        self.reset_room_scale_body_follow();
        command
    }

    pub fn next_pose_sync_command(&mut self) -> Option<EnginePoseSyncCommand> {
        let command = self.next_move_player_command()?;
        Some(EnginePoseSyncCommand {
            kind: EnginePoseSyncCommandKind::Movement,
            command,
            camera: self.snapshot(),
        })
    }

    pub fn accept_position_update(
        &mut self,
        update: PlayerPositionUpdate,
    ) -> EnginePoseCorrectionAcceptance {
        let accept_command = self.apply_player_position_update(update);
        let feet_position = self.player.pose().position;
        EnginePoseCorrectionAcceptance {
            update,
            accept_command,
            feet_position,
            camera: self.snapshot(),
        }
    }

    pub fn corrected_pose_sync_command(&mut self) -> EnginePoseSyncCommand {
        let command = self.pos_rot_move_player_command();
        EnginePoseSyncCommand {
            kind: EnginePoseSyncCommandKind::CorrectionResync,
            command,
            camera: self.snapshot(),
        }
    }

    pub const fn speed_blocks_per_second(&self) -> f64 {
        self.speed_blocks_per_second
    }

    pub fn set_speed_blocks_per_second(&mut self, speed_blocks_per_second: f64) {
        self.speed_blocks_per_second = clamp_camera_speed(speed_blocks_per_second);
    }

    /// Current fly speed expressed as a multiplier of the base speed.
    pub fn fly_speed_multiplier(&self) -> f64 {
        self.speed_blocks_per_second / ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND
    }

    /// Set the fly speed from a multiplier of the base speed. The resulting
    /// speed is clamped to the engine's absolute speed limits.
    pub fn set_fly_speed_multiplier(&mut self, multiplier: f64) {
        if !multiplier.is_finite() {
            return;
        }
        self.set_speed_blocks_per_second(multiplier * ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND);
    }

    /// Current walking movement speed multiplier.
    pub const fn movement_speed_multiplier(&self) -> f64 {
        self.movement_speed_multiplier
    }

    pub fn set_movement_speed_multiplier(&mut self, multiplier: f64) {
        self.movement_speed_multiplier = clamp_movement_speed_multiplier(multiplier);
    }

    pub fn adjust_speed(&mut self, wheel_amount: f64) {
        if !wheel_amount.is_finite() {
            return;
        }
        self.set_speed_blocks_per_second(Self::adjusted_speed_blocks_per_second(
            self.speed_blocks_per_second,
            wheel_amount,
        ));
    }

    pub fn adjusted_speed_blocks_per_second(
        speed_blocks_per_second: f64,
        wheel_amount: f64,
    ) -> f64 {
        if !wheel_amount.is_finite() {
            return clamp_camera_speed(speed_blocks_per_second);
        }
        let multiplier = (1.0 + wheel_amount * 0.18).clamp(0.5, 1.8);
        clamp_camera_speed(speed_blocks_per_second * multiplier)
    }

    pub fn snapshot(&self) -> EngineCameraSnapshot {
        EngineCameraSnapshot::from_player(&self.player, self.speed_blocks_per_second)
    }

    pub fn frame_state(&self, interaction: &ClientInteractionController) -> EngineCameraFrameState {
        EngineCameraFrameState::from_player(
            &self.player,
            self.movement_mode,
            self.collision_mode,
            self.view_mode,
            self.speed_blocks_per_second,
            interaction.selected_hotbar_slot(),
        )
    }

    pub fn pick_block(
        &self,
        client: &ClientRuntime,
        interaction: &ClientInteractionController,
    ) -> BlockHitResult {
        let pose = self.player.pose();
        interaction.pick_block(client, pose.eye_position(), pose.view_vector())
    }

    pub fn target_block(
        &self,
        client: &ClientRuntime,
        interaction: &ClientInteractionController,
    ) -> Option<BlockInteractionTarget> {
        let pose = self.player.pose();
        interaction.target_block(client, pose.eye_position(), pose.view_vector())
    }

    pub fn set_key(&mut self, key: PlayerInputKey, down: bool) {
        self.player.set_key(key, down);
    }

    pub fn clear_keys(&mut self) {
        self.player.clear_keys();
    }

    pub fn turn_mouse_delta(&mut self, mouse_delta_x: f64, mouse_delta_y: f64) {
        Self::turn_player_mouse_delta(&mut self.player, mouse_delta_x, mouse_delta_y);
    }

    pub fn turn_yaw_delta(&mut self, yaw_delta_radians: f64) {
        if !yaw_delta_radians.is_finite() {
            return;
        }
        self.player.turn_native_radians(yaw_delta_radians, 0.0);
        self.reset_room_scale_body_follow();
    }

    pub fn turn_player_mouse_delta(
        player: &mut LocalPlayerController,
        mouse_delta_x: f64,
        mouse_delta_y: f64,
    ) {
        if !mouse_delta_x.is_finite() || !mouse_delta_y.is_finite() {
            return;
        }
        player.turn_native_radians(
            -mouse_delta_x * ENGINE_CAMERA_MOUSE_SENSITIVITY,
            -mouse_delta_y * ENGINE_CAMERA_MOUSE_SENSITIVITY,
        );
    }

    pub fn apply_input(&mut self, input: EngineCameraInput) -> EngineCameraSnapshot {
        self.apply_key_input(input);
        let _ = self.tick_no_clip(input);
        self.snapshot()
    }

    pub fn apply_movement_input(
        &mut self,
        client: &ClientRuntime,
        input: EngineCameraInput,
    ) -> EngineCameraSnapshot {
        self.apply_key_input(input);
        match self.movement_mode {
            EngineCameraMovementMode::Walking => {
                if self.collision_mode == EngineCameraCollisionMode::NoClip {
                    let _ = self.tick_no_clip(input);
                } else {
                    let _ = self.tick_walking(client, input);
                }
            }
            EngineCameraMovementMode::Fly => {
                if self.collision_mode == EngineCameraCollisionMode::NoClip {
                    let _ = self.tick_no_clip(input);
                } else {
                    let _ = self.tick_flying(client, input);
                }
            }
            EngineCameraMovementMode::HandPush => {
                let _ = self.tick_hand_push(client, input);
            }
            EngineCameraMovementMode::Thruster => {
                let _ = self.tick_thruster(client, input);
            }
        }
        self.snapshot()
    }

    pub fn tick_movement(&mut self, client: &ClientRuntime, dt_seconds: f64) -> bool {
        let input = EngineCameraInput {
            dt_seconds,
            ..EngineCameraInput::default()
        };
        match self.movement_mode {
            EngineCameraMovementMode::Walking => {
                if self.collision_mode == EngineCameraCollisionMode::NoClip {
                    self.tick_no_clip(input)
                } else {
                    self.tick_walking(client, input)
                }
            }
            EngineCameraMovementMode::Fly => {
                if self.collision_mode == EngineCameraCollisionMode::NoClip {
                    self.tick_no_clip(input)
                } else {
                    self.tick_flying(client, input)
                }
            }
            EngineCameraMovementMode::HandPush => self.tick_hand_push(client, input),
            EngineCameraMovementMode::Thruster => self.tick_thruster(client, input),
        }
    }

    pub fn probe_ground(&mut self, client: &ClientRuntime, distance: f64) {
        if self.collision_mode == EngineCameraCollisionMode::NoClip
            || !distance.is_finite()
            || distance <= 0.0
        {
            return;
        }
        self.player
            .move_colliding(client, Vec3d::new(0.0, -distance, 0.0));
    }

    pub fn reconcile_room_scale_headset(
        &mut self,
        client: &ClientRuntime,
        headset_world_position: Vec3d,
    ) -> EngineRoomScaleReconciliation {
        let body_eye_before = self.player.pose().eye_position();
        if !headset_world_position.is_finite() {
            self.room_scale_body_follow_active = false;
            self.room_scale_blocked_residual = None;
            let result = EngineRoomScaleReconciliation::no_op(body_eye_before, body_eye_before);
            self.last_room_scale_reconciliation = Some(result);
            return result;
        }
        if self.movement_mode == EngineCameraMovementMode::HandPush {
            self.reset_room_scale_body_follow();
            return EngineRoomScaleReconciliation::no_op(body_eye_before, headset_world_position);
        }

        let requested_body_movement = Vec3d::new(
            headset_world_position.x - body_eye_before.x,
            0.0,
            headset_world_position.z - body_eye_before.z,
        );
        let requested_body_movement_sqr = requested_body_movement.length_sqr();
        let follow_threshold = if self.room_scale_body_follow_active {
            ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS
        } else {
            ENGINE_ROOM_SCALE_BODY_FOLLOW_ENTER_METERS
        };
        if requested_body_movement_sqr <= follow_threshold * follow_threshold {
            self.room_scale_body_follow_active = false;
            self.room_scale_blocked_residual = None;
            let result =
                EngineRoomScaleReconciliation::no_op(body_eye_before, headset_world_position);
            self.last_room_scale_reconciliation = Some(result);
            return result;
        }
        if let Some(blocked_residual) = self.room_scale_blocked_residual {
            let blocked_delta = requested_body_movement.subtract(blocked_residual);
            if blocked_delta.length_sqr()
                <= ENGINE_ROOM_SCALE_BLOCKED_RETRY_METERS * ENGINE_ROOM_SCALE_BLOCKED_RETRY_METERS
            {
                self.room_scale_body_follow_active = true;
                let result =
                    EngineRoomScaleReconciliation::no_op(body_eye_before, headset_world_position);
                self.last_room_scale_reconciliation = Some(result);
                return result;
            }
        }
        self.room_scale_blocked_residual = None;

        let collision = self
            .player
            .move_colliding_horizontal_preserving_vertical_contact(client, requested_body_movement);
        let body_eye_after = self.player.pose().eye_position();
        let consumed_body_movement = Vec3d::new(collision.traveled.x, 0.0, collision.traveled.z);
        let residual_head_offset = Vec3d::new(
            requested_body_movement.x - consumed_body_movement.x,
            headset_world_position.y - body_eye_after.y,
            requested_body_movement.z - consumed_body_movement.z,
        );

        let result = EngineRoomScaleReconciliation {
            body_eye_before,
            headset_world_position,
            requested_body_movement,
            consumed_body_movement,
            residual_head_offset,
            body_eye_after,
            collision,
        };
        self.room_scale_body_follow_active = result.residual_horizontal_length_sqr()
            > ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS * ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS;
        self.room_scale_blocked_residual =
            if collision.horizontal_collision && self.room_scale_body_follow_active {
                Some(Vec3d::new(
                    result.residual_head_offset.x,
                    0.0,
                    result.residual_head_offset.z,
                ))
            } else {
                None
            };
        self.last_room_scale_reconciliation = Some(result);
        result
    }

    fn apply_key_input(&mut self, input: EngineCameraInput) {
        self.set_key(PlayerInputKey::Forward, input.forward);
        self.set_key(PlayerInputKey::Backward, input.backward);
        self.set_key(PlayerInputKey::Left, input.left);
        self.set_key(PlayerInputKey::Right, input.right);
        self.set_key(PlayerInputKey::Jump, input.jump);
        self.set_key(PlayerInputKey::Descend, input.descend);
        self.set_key(PlayerInputKey::Shift, input.shift);
        self.set_key(PlayerInputKey::Sprint, input.sprint);
        self.turn_mouse_delta(input.mouse_delta_x, input.mouse_delta_y);
    }

    fn tick_no_clip(&mut self, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        Self::tick_player_no_clip(
            &mut self.player,
            self.speed_blocks_per_second,
            dt_seconds,
            input
                .movement_impulse
                .map(EngineCameraMovementImpulse::as_player_impulse),
            input.movement_yaw_radians,
        )
        .is_some()
    }

    pub fn tick_player_no_clip(
        player: &mut LocalPlayerController,
        speed_blocks_per_second: f64,
        dt_seconds: f64,
        movement_impulse: Option<(f32, f32)>,
        movement_yaw_radians: Option<f64>,
    ) -> Option<Vec3d> {
        let pose = player.pose();
        let movement_yaw = finite_movement_yaw(movement_yaw_radians);
        let yaw_radians = movement_yaw.unwrap_or_else(|| pose.native_yaw_radians());
        let pitch_radians = if movement_yaw.is_some() {
            0.0
        } else {
            pose.native_pitch_radians()
        };
        player.tick_no_clip_movement_with_impulse(
            NoClipMovementStep {
                yaw_radians,
                pitch_radians,
                speed_blocks_per_second: clamp_camera_speed(speed_blocks_per_second),
                dt_seconds,
                descending: false,
                sprinting: false,
            },
            movement_impulse,
        )
    }

    fn tick_flying(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        Self::tick_player_flying(
            &mut self.player,
            client,
            self.speed_blocks_per_second,
            dt_seconds,
            input
                .movement_impulse
                .map(EngineCameraMovementImpulse::as_player_impulse),
            input.movement_yaw_radians,
        )
        .is_some()
    }

    pub fn tick_player_flying(
        player: &mut LocalPlayerController,
        client: &ClientRuntime,
        speed_blocks_per_second: f64,
        dt_seconds: f64,
        movement_impulse: Option<(f32, f32)>,
        movement_yaw_radians: Option<f64>,
    ) -> Option<CollisionMovementResult> {
        let pose = player.pose();
        let movement_yaw = finite_movement_yaw(movement_yaw_radians);
        let yaw_radians = movement_yaw.unwrap_or_else(|| pose.native_yaw_radians());
        let pitch_radians = if movement_yaw.is_some() {
            0.0
        } else {
            pose.native_pitch_radians()
        };
        player.tick_flying_movement_with_impulse(
            client,
            FlyingMovementStep {
                yaw_radians,
                pitch_radians,
                speed_blocks_per_second: clamp_camera_speed(speed_blocks_per_second),
                dt_seconds,
                descending: false,
                sprinting: false,
            },
            movement_impulse,
        )
    }

    fn tick_walking(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let pose = self.player.pose();
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        let y_rot_degrees = finite_movement_yaw(input.movement_yaw_radians)
            .map(|yaw| -yaw.to_degrees())
            .unwrap_or(pose.y_rot_degrees);
        let was_on_ground = self.player.on_ground();
        let pre_move_fall_speed = (-self.player.delta_movement().y).max(0.0);
        let moved = self
            .player
            .tick_walking_movement_with_impulse(
                client,
                WalkingMovementStep {
                    y_rot_degrees,
                    speed_multiplier: self.movement_speed_multiplier,
                    dt_seconds,
                },
                input
                    .movement_impulse
                    .map(EngineCameraMovementImpulse::as_player_impulse),
            )
            .is_some();
        if moved && !was_on_ground && self.player.on_ground() {
            let impact_speed = pre_move_fall_speed * LOCAL_PLAYER_TICKS_PER_SECOND;
            if impact_speed >= LANDING_MIN_IMPACT_SPEED {
                self.landing_events.push(LandingEvent {
                    impact_speed,
                    position: self.player.pose().position,
                });
            }
        }
        moved
    }

    fn tick_hand_push(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        if dt_seconds <= 0.0 {
            return false;
        }

        let was_on_ground = self.player.on_ground();
        let pre_move_fall_speed = (-self.player.delta_movement().y).max(0.0);
        let hand_input = input.hand_push.or_else(|| {
            input
                .hand_push_emulation
                .then(|| self.emulated_hand_push_input(input, dt_seconds))
                .flatten()
        });
        self.last_hand_push_input = hand_input;
        let mut moved_by_hand = false;
        if let Some(hand_input) = hand_input {
            if let Some(result) = self.hand_push.tick(
                client,
                &mut self.player,
                HandPushMovementStep {
                    dt_seconds,
                    pose: hand_input.pose(),
                },
            ) {
                moved_by_hand = result.body_movement.length_sqr() > 1.0e-12;
            }
        } else {
            self.hand_push.reset();
            self.last_hand_push_input = None;
        }

        let pose = self.player.pose();
        let y_rot_degrees = finite_movement_yaw(input.movement_yaw_radians)
            .map(|yaw| -yaw.to_degrees())
            .unwrap_or(pose.y_rot_degrees);
        let moved_by_physics = self
            .player
            .tick_walking_movement_with_impulse(
                client,
                WalkingMovementStep {
                    y_rot_degrees,
                    speed_multiplier: self.movement_speed_multiplier,
                    dt_seconds,
                },
                Some((0.0, 0.0)),
            )
            .is_some();
        if (moved_by_hand || moved_by_physics) && !was_on_ground && self.player.on_ground() {
            let impact_speed = pre_move_fall_speed * LOCAL_PLAYER_TICKS_PER_SECOND;
            if impact_speed >= LANDING_MIN_IMPACT_SPEED {
                self.landing_events.push(LandingEvent {
                    impact_speed,
                    position: self.player.pose().position,
                });
            }
        }
        moved_by_hand || moved_by_physics
    }

    /// Iron Man / repulsor thruster flight (tactical 157, Slice 2). Feeds the
    /// per-hand palm normal + analog throttle into the client integrator, which
    /// runs in SI units against real `dt` so feel is cadence-independent. Gravity
    /// is always applied (this mode is gravity-bound like `Gorilla`/`HandPush`),
    /// so an empty thrust intent still falls. Collision stays `Normal` here — the
    /// per-frame path never forces NoClip, matching the Slice 1 entry rule.
    fn tick_thruster(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        self.last_thruster_input = input.thruster;
        if dt_seconds <= 0.0 {
            return false;
        }
        let thruster = input.thruster.unwrap_or(EngineThrusterInput::new(
            EngineThrusterHand::NONE,
            EngineThrusterHand::NONE,
        ));
        let step = ThrusterMovementStep {
            left: thruster_hand_input(thruster.left),
            right: thruster_hand_input(thruster.right),
            dt_seconds,
        };
        self.player.tick_thruster_movement(client, step).is_some()
    }

    fn emulated_hand_push_input(
        &mut self,
        input: EngineCameraInput,
        dt_seconds: f64,
    ) -> Option<EngineHandPushInput> {
        let pose = self.player.pose();
        let yaw_radians = finite_movement_yaw(input.movement_yaw_radians)
            .unwrap_or_else(|| pose.native_yaw_radians());
        let direction = hand_push_emulation_direction(input, yaw_radians);
        if direction == Vec3d::ZERO {
            return None;
        }
        self.hand_push_emulation_phase = (self.hand_push_emulation_phase
            + dt_seconds * std::f64::consts::TAU * ENGINE_HAND_PUSH_EMULATION_CYCLE_HZ)
            % std::f64::consts::TAU;
        let right = horizontal_right_from_yaw(yaw_radians);
        let feet = pose.position;
        let hand_y = feet.y + HAND_PUSH_DEFAULT_HAND_RADIUS * 0.5;
        let base = Vec3d::new(feet.x, hand_y, feet.z)
            .add(direction.scale(ENGINE_HAND_PUSH_EMULATION_FORWARD_REACH));
        let left_phase = self.hand_push_emulation_phase;
        let right_phase = self.hand_push_emulation_phase + std::f64::consts::PI;
        let hand_at_phase = |side: f64, phase: f64| {
            base.add(right.scale(side * ENGINE_HAND_PUSH_EMULATION_HAND_SPACING))
                .add(direction.scale(-phase.sin() * ENGINE_HAND_PUSH_EMULATION_STROKE))
                .add(Vec3d::new(
                    0.0,
                    phase.cos().max(0.0) * ENGINE_HAND_PUSH_EMULATION_LIFT,
                    0.0,
                ))
        };
        let head = pose.eye_position();
        Some(EngineHandPushInput::new(
            head,
            hand_at_phase(-1.0, left_phase),
            hand_at_phase(1.0, right_phase),
        ))
    }

    pub fn render_pose(&self, render_distance: u32) -> PerspectiveRenderPose {
        render_pose_from_snapshot_with_view_mode(self.snapshot(), self.view_mode, render_distance)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineDebugVisualOptions {
    pub player_collision_box: bool,
}

impl EngineDebugVisualOptions {
    pub const fn new(player_collision_box: bool) -> Self {
        Self {
            player_collision_box,
        }
    }
}

pub fn engine_debug_world_lines(
    camera: &EngineCameraController,
    options: EngineDebugVisualOptions,
) -> Vec<WorldGuiLine> {
    let mut lines = Vec::new();
    if options.player_collision_box {
        push_aabb_wire_lines(
            &mut lines,
            camera.player().bounding_box(),
            ENGINE_DEBUG_PLAYER_BOX_COLOR,
        );
        if let Some(reconciliation) = camera.last_room_scale_reconciliation() {
            if reconciliation.residual_head_offset.length_sqr()
                > ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS
                    * ENGINE_ROOM_SCALE_BODY_FOLLOW_EXIT_METERS
            {
                let start = reconciliation.body_eye_after;
                let end = start.add(reconciliation.residual_head_offset);
                lines.push(WorldGuiLine::new(
                    glam_vec3_from_vec3d(start),
                    glam_vec3_from_vec3d(end),
                    ENGINE_DEBUG_HEADSET_RESIDUAL_COLOR,
                ));
            }
        }
    }
    if camera.movement_mode() == EngineCameraMovementMode::HandPush {
        if let Some(input) = camera.last_hand_push_input() {
            let radius = sanitize_debug_radius(
                camera.hand_push.settings().hand_radius,
                HAND_PUSH_DEFAULT_HAND_RADIUS,
            );
            push_sphere_wire_lines(
                &mut lines,
                input.left_hand_position,
                radius,
                ENGINE_DEBUG_LEFT_HAND_COLOR,
            );
            push_sphere_wire_lines(
                &mut lines,
                input.right_hand_position,
                radius,
                ENGINE_DEBUG_RIGHT_HAND_COLOR,
            );
        }
    }
    lines
}

fn push_aabb_wire_lines(lines: &mut Vec<WorldGuiLine>, aabb: Aabb, color: [f32; 4]) {
    let corners = [
        Vec3d::new(aabb.min_x, aabb.min_y, aabb.min_z),
        Vec3d::new(aabb.max_x, aabb.min_y, aabb.min_z),
        Vec3d::new(aabb.max_x, aabb.min_y, aabb.max_z),
        Vec3d::new(aabb.min_x, aabb.min_y, aabb.max_z),
        Vec3d::new(aabb.min_x, aabb.max_y, aabb.min_z),
        Vec3d::new(aabb.max_x, aabb.max_y, aabb.min_z),
        Vec3d::new(aabb.max_x, aabb.max_y, aabb.max_z),
        Vec3d::new(aabb.min_x, aabb.max_y, aabb.max_z),
    ];
    for (start, end) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(corners[start]),
            glam_vec3_from_vec3d(corners[end]),
            color,
        ));
    }
}

fn push_sphere_wire_lines(
    lines: &mut Vec<WorldGuiLine>,
    center: Vec3d,
    radius: f64,
    color: [f32; 4],
) {
    let radius = sanitize_debug_radius(radius, HAND_PUSH_DEFAULT_HAND_RADIUS);
    for plane in 0..3 {
        for segment in 0..ENGINE_DEBUG_HAND_SPHERE_SEGMENTS {
            let start = debug_sphere_point(center, radius, plane, segment);
            let end = debug_sphere_point(center, radius, plane, segment + 1);
            lines.push(WorldGuiLine::new(
                glam_vec3_from_vec3d(start),
                glam_vec3_from_vec3d(end),
                color,
            ));
        }
    }
}

fn debug_sphere_point(center: Vec3d, radius: f64, plane: usize, segment: usize) -> Vec3d {
    let angle = (segment % ENGINE_DEBUG_HAND_SPHERE_SEGMENTS) as f64 * std::f64::consts::TAU
        / ENGINE_DEBUG_HAND_SPHERE_SEGMENTS as f64;
    let sin = angle.sin() * radius;
    let cos = angle.cos() * radius;
    match plane {
        0 => center.add(Vec3d::new(cos, sin, 0.0)),
        1 => center.add(Vec3d::new(cos, 0.0, sin)),
        _ => center.add(Vec3d::new(0.0, cos, sin)),
    }
}

fn sanitize_debug_radius(radius: f64, fallback: f64) -> f64 {
    if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        fallback
    }
}

/// Compatibility path for fixed overview/headless diagnostics that still
/// consume `ChunkCamera`. Player-controlled flat render paths should use
/// `render_pose_from_snapshot` or `EngineCameraController::render_pose`.
pub fn legacy_chunk_camera_from_snapshot(
    snapshot: EngineCameraSnapshot,
    render_distance: u32,
) -> ChunkCamera {
    legacy_chunk_camera_from_snapshot_with_view_mode(
        snapshot,
        EngineCameraViewMode::FirstPerson,
        render_distance,
    )
}

/// Compatibility path for fixed overview/headless diagnostics that still
/// consume `ChunkCamera`. Player-controlled flat render paths should use
/// `render_pose_from_snapshot_with_view_mode`.
pub fn legacy_chunk_camera_from_snapshot_with_view_mode(
    snapshot: EngineCameraSnapshot,
    view_mode: EngineCameraViewMode,
    render_distance: u32,
) -> ChunkCamera {
    let forward = view_forward(snapshot.yaw_radians, snapshot.pitch_radians);
    let eye = match view_mode {
        EngineCameraViewMode::FirstPerson => snapshot.eye,
        EngineCameraViewMode::ThirdPersonBack => snapshot
            .eye
            .add(forward.scale(-THIRD_PERSON_CAMERA_DISTANCE)),
    };
    let target = eye.add(forward);
    ChunkCamera {
        eye: vec3d_to_f32_array(eye),
        target: vec3d_to_f32_array(target),
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 64.0_f32.to_radians(),
        z_near: 0.05,
        z_far: 700.0 + render_distance as f32 * 128.0,
    }
}

pub fn render_pose_from_snapshot(
    snapshot: EngineCameraSnapshot,
    render_distance: u32,
) -> PerspectiveRenderPose {
    render_pose_from_snapshot_with_view_mode(
        snapshot,
        EngineCameraViewMode::FirstPerson,
        render_distance,
    )
}

pub fn render_pose_from_snapshot_with_view_mode(
    snapshot: EngineCameraSnapshot,
    view_mode: EngineCameraViewMode,
    render_distance: u32,
) -> PerspectiveRenderPose {
    let forward = view_forward(snapshot.yaw_radians, snapshot.pitch_radians);
    let eye = match view_mode {
        EngineCameraViewMode::FirstPerson => snapshot.eye,
        EngineCameraViewMode::ThirdPersonBack => snapshot
            .eye
            .add(forward.scale(-THIRD_PERSON_CAMERA_DISTANCE)),
    };
    PerspectiveRenderPose::new(
        glam_vec3_from_vec3d(eye),
        render_orientation_from_yaw_pitch(snapshot.yaw_radians, snapshot.pitch_radians),
        64.0_f32.to_radians(),
        0.05,
        700.0 + render_distance as f32 * 128.0,
    )
}

fn render_orientation_from_yaw_pitch(yaw_radians: f64, pitch_radians: f64) -> Quat {
    Quat::from_rotation_y((yaw_radians + std::f64::consts::PI) as f32)
        * Quat::from_rotation_x(pitch_radians as f32)
}

fn clamp_camera_speed(speed_blocks_per_second: f64) -> f64 {
    if speed_blocks_per_second.is_finite() {
        speed_blocks_per_second.clamp(
            ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND,
            ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND,
        )
    } else {
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND
    }
}

fn clamp_movement_speed_multiplier(multiplier: f64) -> f64 {
    if multiplier.is_finite() {
        multiplier.clamp(
            ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
            ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
        )
    } else {
        ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER
    }
}

fn finite_movement_yaw(yaw_radians: Option<f64>) -> Option<f64> {
    yaw_radians.filter(|yaw| yaw.is_finite())
}

fn thruster_hand_input(hand: EngineThrusterHand) -> ThrusterHandInput {
    ThrusterHandInput::new(hand.palm_normal, hand.throttle as f64)
}

pub(crate) fn hand_push_emulation_direction(input: EngineCameraInput, yaw_radians: f64) -> Vec3d {
    if !yaw_radians.is_finite() {
        return Vec3d::ZERO;
    }
    let (left_impulse, forward_impulse) = input
        .movement_impulse
        .map(|impulse| (f64::from(impulse.left), f64::from(impulse.forward)))
        .unwrap_or_else(|| {
            (
                axis(input.left, input.right) as f64,
                axis(input.forward, input.backward) as f64,
            )
        });
    if left_impulse.abs() <= 1.0e-5 && forward_impulse.abs() <= 1.0e-5 {
        return Vec3d::ZERO;
    }
    normalize_vec3d_or_zero(
        horizontal_forward_from_yaw(yaw_radians)
            .scale(forward_impulse)
            .add(horizontal_right_from_yaw(yaw_radians).scale(-left_impulse)),
    )
}

fn horizontal_forward_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.sin(), 0.0, yaw_radians.cos())
}

fn horizontal_right_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.cos(), 0.0, -yaw_radians.sin())
}

fn axis(positive: bool, negative: bool) -> f32 {
    let positive = positive as i32;
    let negative = negative as i32;
    (positive - negative) as f32
}

fn view_forward(yaw_radians: f64, pitch_radians: f64) -> Vec3d {
    let yaw_sin = yaw_radians.sin();
    let yaw_cos = yaw_radians.cos();
    let pitch_sin = pitch_radians.sin();
    let pitch_cos = pitch_radians.cos();
    Vec3d::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos)
}

fn normalize_vec3d_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= 1.0e-12 {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

fn vec3d_to_f32_array(value: Vec3d) -> [f32; 3] {
    [value.x as f32, value.y as f32, value.z as f32]
}
