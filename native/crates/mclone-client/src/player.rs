use std::collections::VecDeque;

use mclone_blocks::{BlockFluidKind, block_fluid_height, block_fluid_kind};
use mclone_core::{Aabb, BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    AcceptTeleportCommand, ClientCommand, MovePlayerCommand, PlayerPositionUpdate,
};

use crate::ClientRuntime;

pub const NO_CLIP_BOOST_MULTIPLIER: f64 = 3.0;
pub const MOVING_SLOW_FACTOR: f32 = 0.3;
pub const LOCAL_PLAYER_STANDING_EYE_HEIGHT: f64 = 1.62;
pub const LOCAL_PLAYER_STANDING_WIDTH: f64 = 0.6;
pub const LOCAL_PLAYER_STANDING_HEIGHT: f64 = 1.8;
pub const LOCAL_PLAYER_HAND_PUSH_EYE_HEIGHT: f64 = 0.6;
pub const LOCAL_PLAYER_HAND_PUSH_WIDTH: f64 = 0.4;
pub const LOCAL_PLAYER_HAND_PUSH_HEIGHT: f64 = 0.6;
pub const LOCAL_PLAYER_X_ROT_LIMIT_DEGREES: f64 = 90.0;
pub const LOCAL_PLAYER_TICKS_PER_SECOND: f64 = 20.0;
pub const LOCAL_PLAYER_BASE_MOVEMENT_SPEED: f64 = 0.1;
pub const LOCAL_PLAYER_SPRINT_SPEED_MULTIPLIER: f64 = 1.3;
pub const LOCAL_PLAYER_AIR_SPEED: f64 = 0.02;
pub const LOCAL_PLAYER_AIR_SPRINT_SPEED: f64 = 0.026;
pub const LOCAL_PLAYER_JUMP_POWER: f64 = 0.42;
pub const LOCAL_PLAYER_SPRINT_JUMP_IMPULSE: f64 = 0.2;
pub const LOCAL_PLAYER_GRAVITY: f64 = 0.08;
pub const LOCAL_PLAYER_FLUID_JUMP_POWER: f64 = 0.04;
pub const LOCAL_PLAYER_WATER_MOVEMENT_SPEED: f64 = 0.02;
pub const LOCAL_PLAYER_WATER_SLOWDOWN: f64 = 0.8;
pub const LOCAL_PLAYER_WATER_SPRINT_SLOWDOWN: f64 = 0.9;
pub const LOCAL_PLAYER_WATER_VERTICAL_DRAG: f64 = 0.8;
pub const LOCAL_PLAYER_BLOCK_FRICTION: f64 = 0.6;
pub const LOCAL_PLAYER_FRICTION_MULTIPLIER: f64 = 0.91;
pub const LOCAL_PLAYER_VERTICAL_DRAG: f64 = 0.98;
pub const HAND_PUSH_DEFAULT_HAND_RADIUS: f64 = 0.08;
pub const HAND_PUSH_DEFAULT_HEAD_RADIUS: f64 = 0.18;
pub const HAND_PUSH_DEFAULT_MAX_ARM_LENGTH: f64 = 1.5;
pub const HAND_PUSH_DEFAULT_UNSTICK_DISTANCE: f64 = 1.0;
pub const HAND_PUSH_DEFAULT_VELOCITY_HISTORY_SIZE: usize = 5;
pub const HAND_PUSH_DEFAULT_VELOCITY_LIMIT: f64 = 1.0;
pub const HAND_PUSH_DEFAULT_MAX_JUMP_SPEED: f64 = 7.0;
pub const HAND_PUSH_DEFAULT_JUMP_MULTIPLIER: f64 = 1.1;
const LOCAL_PLAYER_GROUND_ACCELERATION_NUMERATOR: f64 = 0.21600002;
const LOCAL_PLAYER_POSITION_SYNC_DELTA_SQR: f64 = 9.0e-4;
const LOCAL_PLAYER_POSITION_REMINDER_INTERVAL: u32 = 20;
const COLLISION_EPSILON: f64 = 1.0e-7;
const LOCAL_PLAYER_AUTO_JUMP_HEIGHT: f64 = 1.0;
const LOCAL_PLAYER_AUTO_JUMP_MIN_MOVEMENT_DOT: f64 = -0.15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerInputKey {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Shift,
    Descend,
    Sprint,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerInputKeys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub descend: bool,
    pub sprint: bool,
}

impl PlayerInputKeys {
    pub fn set(&mut self, key: PlayerInputKey, down: bool) {
        match key {
            PlayerInputKey::Forward => self.forward = down,
            PlayerInputKey::Backward => self.backward = down,
            PlayerInputKey::Left => self.left = down,
            PlayerInputKey::Right => self.right = down,
            PlayerInputKey::Jump => self.jump = down,
            PlayerInputKey::Shift => self.shift = down,
            PlayerInputKey::Descend => self.descend = down,
            PlayerInputKey::Sprint => self.sprint = down,
        }
    }

    pub fn is_down(self, key: PlayerInputKey) -> bool {
        match key {
            PlayerInputKey::Forward => self.forward,
            PlayerInputKey::Backward => self.backward,
            PlayerInputKey::Left => self.left,
            PlayerInputKey::Right => self.right,
            PlayerInputKey::Jump => self.jump,
            PlayerInputKey::Shift => self.shift,
            PlayerInputKey::Descend => self.descend,
            PlayerInputKey::Sprint => self.sprint,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    pub left_impulse: f32,
    pub forward_impulse: f32,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub jumping: bool,
    pub shift_key_down: bool,
}

impl PlayerInput {
    pub fn tick(&mut self, keys: PlayerInputKeys, moving_slowly: bool) {
        self.tick_with_movement_impulse(keys, moving_slowly, None);
    }

    pub fn tick_with_movement_impulse(
        &mut self,
        keys: PlayerInputKeys,
        moving_slowly: bool,
        movement_impulse: Option<(f32, f32)>,
    ) {
        self.up = keys.forward;
        self.down = keys.backward;
        self.left = keys.left;
        self.right = keys.right;
        self.forward_impulse = axis(self.up, self.down);
        self.left_impulse = axis(self.left, self.right);
        self.jumping = keys.jump;
        self.shift_key_down = keys.shift;
        if moving_slowly {
            self.left_impulse *= MOVING_SLOW_FACTOR;
            self.forward_impulse *= MOVING_SLOW_FACTOR;
        }
        if let Some((left_impulse, forward_impulse)) = movement_impulse {
            self.left_impulse = sanitize_movement_impulse(left_impulse);
            self.forward_impulse = sanitize_movement_impulse(forward_impulse);
            if moving_slowly {
                self.left_impulse *= MOVING_SLOW_FACTOR;
                self.forward_impulse *= MOVING_SLOW_FACTOR;
            }
            self.up = self.forward_impulse > 1.0e-5;
            self.down = self.forward_impulse < -1.0e-5;
            self.left = self.left_impulse > 1.0e-5;
            self.right = self.left_impulse < -1.0e-5;
        }
    }

    pub const fn move_vector(self) -> (f32, f32) {
        (self.left_impulse, self.forward_impulse)
    }

    pub fn has_forward_impulse(self) -> bool {
        self.forward_impulse > 1.0e-5
    }

    pub fn is_moving(self) -> bool {
        self.forward_impulse.abs() > 1.0e-5 || self.left_impulse.abs() > 1.0e-5
    }

    fn right_axis(self) -> f64 {
        -self.left_impulse as f64
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyingMovementStep {
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub speed_blocks_per_second: f64,
    pub dt_seconds: f64,
    pub descending: bool,
    pub sprinting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoClipMovementStep {
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub speed_blocks_per_second: f64,
    pub dt_seconds: f64,
    pub descending: bool,
    pub sprinting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkingMovementStep {
    pub y_rot_degrees: f64,
    pub speed_multiplier: f64,
    pub dt_seconds: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPushPose {
    pub head_position: Vec3d,
    pub left_hand_position: Vec3d,
    pub right_hand_position: Vec3d,
}

impl HandPushPose {
    pub fn is_finite(self) -> bool {
        self.head_position.is_finite()
            && self.left_hand_position.is_finite()
            && self.right_hand_position.is_finite()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPushMovementStep {
    pub dt_seconds: f64,
    pub pose: HandPushPose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPushLocomotionSettings {
    pub hand_radius: f64,
    pub head_radius: f64,
    pub max_arm_length: f64,
    pub unstick_distance: f64,
    pub velocity_history_size: usize,
    pub velocity_limit: f64,
    pub max_jump_speed: f64,
    pub jump_multiplier: f64,
}

impl Default for HandPushLocomotionSettings {
    fn default() -> Self {
        Self {
            hand_radius: HAND_PUSH_DEFAULT_HAND_RADIUS,
            head_radius: HAND_PUSH_DEFAULT_HEAD_RADIUS,
            max_arm_length: HAND_PUSH_DEFAULT_MAX_ARM_LENGTH,
            unstick_distance: HAND_PUSH_DEFAULT_UNSTICK_DISTANCE,
            velocity_history_size: HAND_PUSH_DEFAULT_VELOCITY_HISTORY_SIZE,
            velocity_limit: HAND_PUSH_DEFAULT_VELOCITY_LIMIT,
            max_jump_speed: HAND_PUSH_DEFAULT_MAX_JUMP_SPEED,
            jump_multiplier: HAND_PUSH_DEFAULT_JUMP_MULTIPLIER,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HandPushMovementResult {
    pub body_movement: Vec3d,
    pub left_hand_touching: bool,
    pub right_hand_touching: bool,
    pub velocity_average: Vec3d,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HandPushHandResult {
    body_offset: Vec3d,
    last_position: Vec3d,
    touching: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandPushLocomotionController {
    settings: HandPushLocomotionSettings,
    initialized: bool,
    last_left_hand_position: Vec3d,
    last_right_hand_position: Vec3d,
    last_head_position: Vec3d,
    last_player_position: Vec3d,
    velocity_history: VecDeque<Vec3d>,
    left_hand_touching: bool,
    right_hand_touching: bool,
}

impl Default for HandPushLocomotionController {
    fn default() -> Self {
        Self::new(HandPushLocomotionSettings::default())
    }
}

impl HandPushLocomotionController {
    pub fn new(settings: HandPushLocomotionSettings) -> Self {
        Self {
            settings,
            initialized: false,
            last_left_hand_position: Vec3d::ZERO,
            last_right_hand_position: Vec3d::ZERO,
            last_head_position: Vec3d::ZERO,
            last_player_position: Vec3d::ZERO,
            velocity_history: VecDeque::new(),
            left_hand_touching: false,
            right_hand_touching: false,
        }
    }

    pub const fn settings(&self) -> HandPushLocomotionSettings {
        self.settings
    }

    pub const fn left_hand_touching(&self) -> bool {
        self.left_hand_touching
    }

    pub const fn right_hand_touching(&self) -> bool {
        self.right_hand_touching
    }

    pub fn reset(&mut self) {
        self.initialized = false;
        self.last_left_hand_position = Vec3d::ZERO;
        self.last_right_hand_position = Vec3d::ZERO;
        self.last_head_position = Vec3d::ZERO;
        self.last_player_position = Vec3d::ZERO;
        self.velocity_history.clear();
        self.left_hand_touching = false;
        self.right_hand_touching = false;
    }

    pub fn tick(
        &mut self,
        client: &ClientRuntime,
        player: &mut LocalPlayerController,
        step: HandPushMovementStep,
    ) -> Option<HandPushMovementResult> {
        if !step.dt_seconds.is_finite() || step.dt_seconds <= 0.0 || !step.pose.is_finite() {
            return None;
        }

        let pose = self.clamped_pose(step.pose);
        if !self.initialized {
            self.initialized = true;
            self.last_left_hand_position = pose.left_hand_position;
            self.last_right_hand_position = pose.right_hand_position;
            self.last_head_position = pose.head_position;
            self.last_player_position = player.pose().position;
            return Some(HandPushMovementResult::default());
        }

        let left = self.resolve_hand(
            client,
            self.last_left_hand_position,
            pose.left_hand_position,
            self.left_hand_touching,
        );
        let right = self.resolve_hand(
            client,
            self.last_right_hand_position,
            pose.right_hand_position,
            self.right_hand_touching,
        );

        let both_hands_braced = (left.touching || self.left_hand_touching)
            && (right.touching || self.right_hand_touching);
        let requested_body_movement = if both_hands_braced {
            left.body_offset.add(right.body_offset).scale(0.5)
        } else {
            left.body_offset.add(right.body_offset)
        };

        let head_validated_body_movement = validate_hand_push_head_movement(
            client,
            self.last_head_position,
            pose.head_position,
            requested_body_movement,
            self.settings.head_radius,
        );

        let body_movement = if head_validated_body_movement.length_sqr() > COLLISION_EPSILON {
            player
                .move_colliding(client, head_validated_body_movement)
                .traveled
        } else {
            Vec3d::ZERO
        };
        self.last_head_position = pose.head_position.add(body_movement);

        self.last_left_hand_position = self.unstuck_hand_position(
            pose.head_position,
            pose.left_hand_position,
            left.last_position,
            left.touching,
        );
        self.last_right_hand_position = self.unstuck_hand_position(
            pose.head_position,
            pose.right_hand_position,
            right.last_position,
            right.touching,
        );
        self.left_hand_touching = left.touching
            && self
                .last_left_hand_position
                .distance_to_sqr(pose.left_hand_position)
                <= self.settings.unstick_distance * self.settings.unstick_distance;
        self.right_hand_touching = right.touching
            && self
                .last_right_hand_position
                .distance_to_sqr(pose.right_hand_position)
                <= self.settings.unstick_distance * self.settings.unstick_distance;

        let velocity_average = self.store_velocity(player.pose().position, step.dt_seconds);
        if (self.left_hand_touching || self.right_hand_touching)
            && velocity_average.length_sqr()
                > self.settings.velocity_limit * self.settings.velocity_limit
        {
            player.set_delta_movement(hand_push_jump_delta_movement(
                velocity_average,
                self.settings,
            ));
        }

        Some(HandPushMovementResult {
            body_movement,
            left_hand_touching: self.left_hand_touching,
            right_hand_touching: self.right_hand_touching,
            velocity_average,
        })
    }

    fn clamped_pose(&self, pose: HandPushPose) -> HandPushPose {
        HandPushPose {
            left_hand_position: clamp_hand_reach(
                pose.head_position,
                pose.left_hand_position,
                self.settings.max_arm_length,
            ),
            right_hand_position: clamp_hand_reach(
                pose.head_position,
                pose.right_hand_position,
                self.settings.max_arm_length,
            ),
            ..pose
        }
    }

    fn resolve_hand(
        &self,
        client: &ClientRuntime,
        last_position: Vec3d,
        current_position: Vec3d,
        was_touching: bool,
    ) -> HandPushHandResult {
        let movement = current_position.subtract(last_position);
        let probe = hand_probe_aabb(current_position, self.settings.hand_radius);
        let currently_intersecting = !solid_block_aabbs_in(client, probe).is_empty();
        let sweep =
            sweep_sphere_against_world(client, last_position, self.settings.hand_radius, movement);
        let sweep_position = sweep
            .map(|hit| hit.center_position)
            .unwrap_or_else(|| last_position.add(movement));
        let sweep_collided = sweep.is_some();
        let touching = currently_intersecting || sweep_collided;

        if !touching {
            return HandPushHandResult {
                body_offset: Vec3d::ZERO,
                last_position: current_position,
                touching: false,
            };
        }

        let body_offset = if was_touching || currently_intersecting {
            last_position.subtract(current_position)
        } else {
            sweep_position.subtract(current_position)
        };

        HandPushHandResult {
            body_offset,
            last_position: sweep_position,
            touching: true,
        }
    }

    fn unstuck_hand_position(
        &self,
        head_position: Vec3d,
        current_position: Vec3d,
        last_position: Vec3d,
        touching: bool,
    ) -> Vec3d {
        if !touching {
            return current_position;
        }
        let distance_sqr = current_position.distance_to_sqr(last_position);
        let unstick_sqr = self.settings.unstick_distance * self.settings.unstick_distance;
        if distance_sqr <= unstick_sqr {
            return last_position;
        }
        let head_to_hand = current_position.subtract(head_position);
        let safe_position = head_position.add(
            normalize_or_zero(head_to_hand).scale(
                self.settings
                    .max_arm_length
                    .min(head_to_hand.length_sqr().sqrt()),
            ),
        );
        if safe_position.is_finite() {
            safe_position
        } else {
            current_position
        }
    }

    fn store_velocity(&mut self, player_position: Vec3d, dt_seconds: f64) -> Vec3d {
        let velocity = player_position
            .subtract(self.last_player_position)
            .scale(1.0 / dt_seconds);
        self.last_player_position = player_position;
        if self.settings.velocity_history_size == 0 {
            return velocity;
        }
        self.velocity_history.push_back(velocity);
        while self.velocity_history.len() > self.settings.velocity_history_size {
            self.velocity_history.pop_front();
        }
        if self.velocity_history.is_empty() {
            return Vec3d::ZERO;
        }
        self.velocity_history
            .iter()
            .copied()
            .fold(Vec3d::ZERO, Vec3d::add)
            .scale(1.0 / self.velocity_history.len() as f64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalPlayerDimensions {
    pub width: f64,
    pub height: f64,
    pub eye_height: f64,
}

impl LocalPlayerDimensions {
    pub const STANDING: Self = Self {
        width: LOCAL_PLAYER_STANDING_WIDTH,
        height: LOCAL_PLAYER_STANDING_HEIGHT,
        eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
    };

    pub const HAND_PUSH: Self = Self {
        width: LOCAL_PLAYER_HAND_PUSH_WIDTH,
        height: LOCAL_PLAYER_HAND_PUSH_HEIGHT,
        eye_height: LOCAL_PLAYER_HAND_PUSH_EYE_HEIGHT,
    };

    pub const fn new(width: f64, height: f64, eye_height: f64) -> Self {
        Self {
            width,
            height,
            eye_height,
        }
    }

    fn sanitized(self) -> Self {
        let fallback = Self::STANDING;
        let width = if self.width.is_finite() && self.width > 0.0 {
            self.width
        } else {
            fallback.width
        };
        let height = if self.height.is_finite() && self.height > 0.0 {
            self.height
        } else {
            fallback.height
        };
        let eye_height = if self.eye_height.is_finite() {
            self.eye_height.clamp(0.0, height)
        } else {
            fallback.eye_height.min(height)
        };
        Self {
            width,
            height,
            eye_height,
        }
    }
}

impl Default for LocalPlayerDimensions {
    fn default() -> Self {
        Self::STANDING
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalPlayerPose {
    pub position: Vec3d,
    pub y_rot_degrees: f64,
    pub x_rot_degrees: f64,
    pub eye_height: f64,
}

impl Default for LocalPlayerPose {
    fn default() -> Self {
        Self {
            position: Vec3d::ZERO,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        }
    }
}

impl LocalPlayerPose {
    pub fn from_eye_position(
        eye_position: Vec3d,
        y_rot_degrees: f64,
        x_rot_degrees: f64,
        eye_height: f64,
    ) -> Self {
        let mut pose = Self {
            position: Vec3d::new(
                eye_position.x,
                eye_position.y - eye_height.max(0.0),
                eye_position.z,
            ),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            eye_height: eye_height.max(0.0),
        };
        pose.set_rot(y_rot_degrees, x_rot_degrees);
        pose
    }

    pub fn set_position(&mut self, position: Vec3d) {
        if position.is_finite() {
            self.position = position;
        }
    }

    pub fn set_eye_position(&mut self, eye_position: Vec3d) {
        self.set_position(Vec3d::new(
            eye_position.x,
            eye_position.y - self.eye_height,
            eye_position.z,
        ));
    }

    pub fn move_by(&mut self, displacement: Vec3d) {
        self.set_position(self.position.add(displacement));
    }

    pub fn set_rot(&mut self, y_rot_degrees: f64, x_rot_degrees: f64) {
        if y_rot_degrees.is_finite() {
            self.y_rot_degrees = y_rot_degrees % 360.0;
        }
        if x_rot_degrees.is_finite() {
            self.x_rot_degrees = x_rot_degrees % 360.0;
        }
    }

    pub fn turn_degrees(&mut self, y_delta_degrees: f64, x_delta_degrees: f64) {
        if y_delta_degrees.is_finite() {
            self.y_rot_degrees = (self.y_rot_degrees + y_delta_degrees) % 360.0;
        }
        if x_delta_degrees.is_finite() {
            self.x_rot_degrees = (self.x_rot_degrees + x_delta_degrees).clamp(
                -LOCAL_PLAYER_X_ROT_LIMIT_DEGREES,
                LOCAL_PLAYER_X_ROT_LIMIT_DEGREES,
            );
        }
    }

    pub fn turn_native_radians(&mut self, yaw_delta_radians: f64, pitch_delta_radians: f64) {
        self.turn_degrees(
            -yaw_delta_radians.to_degrees(),
            -pitch_delta_radians.to_degrees(),
        );
    }

    pub fn eye_position(self) -> Vec3d {
        Vec3d::new(
            self.position.x,
            self.position.y + self.eye_height,
            self.position.z,
        )
    }

    pub fn bounding_box(self) -> Aabb {
        self.bounding_box_with_dimensions(LocalPlayerDimensions::STANDING)
    }

    pub fn bounding_box_with_dimensions(self, dimensions: LocalPlayerDimensions) -> Aabb {
        let dimensions = dimensions.sanitized();
        Aabb::new(
            self.position.x - dimensions.width / 2.0,
            self.position.y,
            self.position.z - dimensions.width / 2.0,
            self.position.x + dimensions.width / 2.0,
            self.position.y + dimensions.height,
            self.position.z + dimensions.width / 2.0,
        )
    }

    pub fn block_position(self) -> BlockPos {
        BlockPos::containing(self.position)
    }

    pub fn eye_block_position(self) -> BlockPos {
        BlockPos::containing(self.eye_position())
    }

    pub fn chunk_pos(self) -> ChunkPos {
        self.block_position().chunk_pos()
    }

    pub fn block_column(self) -> (i32, i32) {
        let block = self.block_position();
        (block.x, block.z)
    }

    pub fn view_vector(self) -> Vec3d {
        view_vector_from_rot_degrees(self.x_rot_degrees, self.y_rot_degrees)
    }

    pub fn native_yaw_radians(self) -> f64 {
        -self.y_rot_degrees.to_radians()
    }

    pub fn native_pitch_radians(self) -> f64 {
        -self.x_rot_degrees.to_radians()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalPlayerController {
    pose: LocalPlayerPose,
    dimensions: LocalPlayerDimensions,
    keys: PlayerInputKeys,
    input: PlayerInput,
    move_sync: LocalPlayerMoveSync,
    delta_movement: Vec3d,
    horizontal_collision: bool,
    vertical_collision: bool,
    on_ground: bool,
    auto_jump_enabled: bool,
    auto_jump_time: u8,
    touching_water: bool,
    eye_in_water: bool,
    water_height: f64,
}

impl Default for LocalPlayerController {
    fn default() -> Self {
        Self {
            pose: LocalPlayerPose::default(),
            dimensions: LocalPlayerDimensions::default(),
            keys: PlayerInputKeys::default(),
            input: PlayerInput::default(),
            move_sync: LocalPlayerMoveSync::default(),
            delta_movement: Vec3d::ZERO,
            horizontal_collision: false,
            vertical_collision: false,
            on_ground: false,
            auto_jump_enabled: true,
            auto_jump_time: 0,
            touching_water: false,
            eye_in_water: false,
            water_height: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WaterContact {
    touching: bool,
    eye_in_water: bool,
    height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LocalPlayerMoveSync {
    last_position: Vec3d,
    last_y_rot_degrees: f32,
    last_x_rot_degrees: f32,
    last_on_ground: bool,
    position_reminder: u32,
}

impl LocalPlayerMoveSync {
    fn next_command(
        &mut self,
        pose: LocalPlayerPose,
        on_ground: bool,
    ) -> Option<MovePlayerCommand> {
        let y_rot_degrees = pose.y_rot_degrees as f32;
        let x_rot_degrees = pose.x_rot_degrees as f32;
        self.position_reminder = self.position_reminder.saturating_add(1);

        let position_delta = pose.position.subtract(self.last_position);
        let moved = position_delta.length_sqr() > LOCAL_PLAYER_POSITION_SYNC_DELTA_SQR
            || self.position_reminder >= LOCAL_PLAYER_POSITION_REMINDER_INTERVAL;
        let rotated =
            y_rot_degrees != self.last_y_rot_degrees || x_rot_degrees != self.last_x_rot_degrees;

        let command = match (moved, rotated, self.last_on_ground != on_ground) {
            (true, true, _) => Some(MovePlayerCommand::PosRot {
                position: pose.position,
                y_rot_degrees,
                x_rot_degrees,
                on_ground,
            }),
            (true, false, _) => Some(MovePlayerCommand::Pos {
                position: pose.position,
                on_ground,
            }),
            (false, true, _) => Some(MovePlayerCommand::Rot {
                y_rot_degrees,
                x_rot_degrees,
                on_ground,
            }),
            (false, false, true) => Some(MovePlayerCommand::StatusOnly { on_ground }),
            (false, false, false) => None,
        };

        if moved {
            self.record_position(pose.position);
        }
        if rotated {
            self.record_rotation(y_rot_degrees, x_rot_degrees);
        }
        self.last_on_ground = on_ground;

        command
    }

    fn pos_rot_command(&mut self, pose: LocalPlayerPose, on_ground: bool) -> MovePlayerCommand {
        let y_rot_degrees = pose.y_rot_degrees as f32;
        let x_rot_degrees = pose.x_rot_degrees as f32;
        self.record_position(pose.position);
        self.record_rotation(y_rot_degrees, x_rot_degrees);
        self.last_on_ground = on_ground;
        MovePlayerCommand::PosRot {
            position: pose.position,
            y_rot_degrees,
            x_rot_degrees,
            on_ground,
        }
    }

    fn record_position(&mut self, position: Vec3d) {
        self.last_position = position;
        self.position_reminder = 0;
    }

    fn record_rotation(&mut self, y_rot_degrees: f32, x_rot_degrees: f32) {
        self.last_y_rot_degrees = y_rot_degrees;
        self.last_x_rot_degrees = x_rot_degrees;
    }
}

impl LocalPlayerController {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn keys(&self) -> PlayerInputKeys {
        self.keys
    }

    pub const fn input(&self) -> PlayerInput {
        self.input
    }

    pub const fn delta_movement(&self) -> Vec3d {
        self.delta_movement
    }

    pub const fn pose(&self) -> LocalPlayerPose {
        self.pose
    }

    pub const fn dimensions(&self) -> LocalPlayerDimensions {
        self.dimensions
    }

    pub fn bounding_box(&self) -> Aabb {
        self.pose.bounding_box_with_dimensions(self.dimensions)
    }

    pub const fn horizontal_collision(&self) -> bool {
        self.horizontal_collision
    }

    pub const fn vertical_collision(&self) -> bool {
        self.vertical_collision
    }

    pub const fn on_ground(&self) -> bool {
        self.on_ground
    }

    pub const fn auto_jump_enabled(&self) -> bool {
        self.auto_jump_enabled
    }

    pub const fn auto_jump_time(&self) -> u8 {
        self.auto_jump_time
    }

    pub const fn touching_water(&self) -> bool {
        self.touching_water
    }

    pub const fn eye_in_water(&self) -> bool {
        self.eye_in_water
    }

    pub const fn water_height(&self) -> f64 {
        self.water_height
    }

    pub fn set_auto_jump_enabled(&mut self, enabled: bool) {
        self.auto_jump_enabled = enabled;
        if !enabled {
            self.auto_jump_time = 0;
        }
    }

    pub fn set_pose(&mut self, pose: LocalPlayerPose) {
        self.pose = pose;
    }

    pub fn set_dimensions(&mut self, dimensions: LocalPlayerDimensions) {
        let dimensions = dimensions.sanitized();
        self.dimensions = dimensions;
        self.pose.eye_height = dimensions.eye_height;
        self.clear_water_contact();
    }

    pub fn next_move_player_command(&mut self) -> Option<ClientCommand> {
        self.move_sync
            .next_command(self.pose, self.on_ground)
            .map(ClientCommand::MovePlayer)
    }

    pub fn pos_rot_move_player_command(&mut self) -> ClientCommand {
        ClientCommand::MovePlayer(self.move_sync.pos_rot_command(self.pose, self.on_ground))
    }

    pub fn apply_player_position_update(&mut self, update: PlayerPositionUpdate) -> ClientCommand {
        let pose = self.pose;
        let position = Vec3d::new(
            if update.relative.x {
                pose.position.x + update.position.x
            } else {
                update.position.x
            },
            if update.relative.y {
                pose.position.y + update.position.y
            } else {
                update.position.y
            },
            if update.relative.z {
                pose.position.z + update.position.z
            } else {
                update.position.z
            },
        );
        let y_rot_degrees = if update.relative.y_rot {
            pose.y_rot_degrees + f64::from(update.y_rot_degrees)
        } else {
            f64::from(update.y_rot_degrees)
        };
        let x_rot_degrees = if update.relative.x_rot {
            pose.x_rot_degrees + f64::from(update.x_rot_degrees)
        } else {
            f64::from(update.x_rot_degrees)
        };
        self.pose.set_position(position);
        self.pose.set_rot(y_rot_degrees, x_rot_degrees);
        self.clear_delta_movement();
        self.auto_jump_time = 0;
        self.clear_water_contact();
        self.on_ground = false;
        ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: update.teleport_id,
        })
    }

    pub fn set_delta_movement(&mut self, delta_movement: Vec3d) {
        if delta_movement.is_finite() {
            self.delta_movement = delta_movement;
        }
    }

    pub fn clear_delta_movement(&mut self) {
        self.delta_movement = Vec3d::ZERO;
    }

    pub fn set_eye_position(&mut self, eye_position: Vec3d) {
        self.pose.set_eye_position(eye_position);
    }

    pub fn turn_native_radians(&mut self, yaw_delta_radians: f64, pitch_delta_radians: f64) {
        self.pose
            .turn_native_radians(yaw_delta_radians, pitch_delta_radians);
    }

    pub fn set_key(&mut self, key: PlayerInputKey, down: bool) {
        self.keys.set(key, down);
    }

    pub fn clear_keys(&mut self) {
        self.keys.clear();
        self.input = PlayerInput::default();
        self.auto_jump_time = 0;
    }

    pub fn tick_input(&mut self, moving_slowly: bool) -> PlayerInput {
        self.tick_input_with_movement_impulse(moving_slowly, None)
    }

    pub fn tick_input_with_movement_impulse(
        &mut self,
        moving_slowly: bool,
        movement_impulse: Option<(f32, f32)>,
    ) -> PlayerInput {
        self.input
            .tick_with_movement_impulse(self.keys, moving_slowly, movement_impulse);
        self.input
    }

    pub fn tick_no_clip_movement(&mut self, step: NoClipMovementStep) -> Option<Vec3d> {
        self.tick_no_clip_movement_with_impulse(step, None)
    }

    pub fn tick_no_clip_movement_with_impulse(
        &mut self,
        step: NoClipMovementStep,
        movement_impulse: Option<(f32, f32)>,
    ) -> Option<Vec3d> {
        let input = self.tick_input_with_movement_impulse(false, movement_impulse);
        let step = NoClipMovementStep {
            descending: self.keys.descend,
            sprinting: self.keys.sprint,
            ..step
        };
        let displacement = no_clip_displacement(input, step)?;
        self.pose.move_by(displacement);
        self.horizontal_collision = false;
        self.vertical_collision = false;
        self.on_ground = false;
        self.delta_movement = Vec3d::ZERO;
        self.auto_jump_time = 0;
        self.clear_water_contact();
        Some(displacement)
    }

    pub fn tick_flying_movement(
        &mut self,
        client: &ClientRuntime,
        step: FlyingMovementStep,
    ) -> Option<CollisionMovementResult> {
        self.tick_flying_movement_with_impulse(client, step, None)
    }

    pub fn tick_flying_movement_with_impulse(
        &mut self,
        client: &ClientRuntime,
        step: FlyingMovementStep,
        movement_impulse: Option<(f32, f32)>,
    ) -> Option<CollisionMovementResult> {
        let input = self.tick_input_with_movement_impulse(false, movement_impulse);
        let step = FlyingMovementStep {
            descending: self.keys.descend,
            sprinting: self.keys.sprint,
            ..step
        };
        let displacement = flying_displacement(input, step)?;
        let collision = self.move_colliding(client, displacement);
        self.delta_movement = Vec3d::ZERO;
        self.auto_jump_time = 0;
        self.clear_water_contact();
        Some(collision)
    }

    pub fn tick_walking_movement(
        &mut self,
        client: &ClientRuntime,
        step: WalkingMovementStep,
    ) -> Option<WalkingMovementResult> {
        self.tick_walking_movement_with_impulse(client, step, None)
    }

    pub fn tick_walking_movement_with_impulse(
        &mut self,
        client: &ClientRuntime,
        step: WalkingMovementStep,
        movement_impulse: Option<(f32, f32)>,
    ) -> Option<WalkingMovementResult> {
        if !step.y_rot_degrees.is_finite() || !step.dt_seconds.is_finite() || step.dt_seconds <= 0.0
        {
            return None;
        }

        let tick_scale = step.dt_seconds * LOCAL_PLAYER_TICKS_PER_SECOND;
        if tick_scale <= 0.0 {
            return None;
        }

        let mut input = self.tick_input_with_movement_impulse(self.keys.shift, movement_impulse);
        let auto_jumped = self.consume_auto_jump_input(&mut input);
        let water_contact = self.update_water_contact(client);
        let can_sprint_in_medium = !water_contact.touching || water_contact.eye_in_water;
        let sprinting = self.keys.sprint
            && input.has_forward_impulse()
            && !input.shift_key_down
            && can_sprint_in_medium;
        if water_contact.touching {
            return Some(self.tick_water_movement(client, input, step, sprinting, tick_scale));
        }

        if input.jumping && self.on_ground {
            self.delta_movement.y = LOCAL_PLAYER_JUMP_POWER;
            if sprinting {
                let y_rot = step.y_rot_degrees.to_radians();
                self.delta_movement = self.delta_movement.add(Vec3d::new(
                    -y_rot.sin() * LOCAL_PLAYER_SPRINT_JUMP_IMPULSE,
                    0.0,
                    y_rot.cos() * LOCAL_PLAYER_SPRINT_JUMP_IMPULSE,
                ));
            }
        }

        let acceleration = walking_input_acceleration(
            input,
            step.y_rot_degrees,
            walking_input_speed(self.on_ground, sprinting)
                * walking_speed_multiplier(step.speed_multiplier),
        )
        .scale(tick_scale);
        self.delta_movement = self.delta_movement.add(acceleration);

        let requested = self.delta_movement.scale(tick_scale);
        let collision = self.move_colliding(client, requested);
        self.update_water_contact(client);
        if self.should_schedule_auto_jump(client, input, step, collision, auto_jumped) {
            self.auto_jump_time = 1;
        }

        let mut post_move_delta = self.delta_movement;
        if !nearly_equal(requested.x, collision.traveled.x) {
            post_move_delta.x = 0.0;
        }
        if !nearly_equal(requested.y, collision.traveled.y) {
            post_move_delta.y = 0.0;
        }
        if !nearly_equal(requested.z, collision.traveled.z) {
            post_move_delta.z = 0.0;
        }

        let horizontal_drag = if self.on_ground {
            LOCAL_PLAYER_BLOCK_FRICTION * LOCAL_PLAYER_FRICTION_MULTIPLIER
        } else {
            LOCAL_PLAYER_FRICTION_MULTIPLIER
        }
        .powf(tick_scale);
        let vertical_drag = LOCAL_PLAYER_VERTICAL_DRAG.powf(tick_scale);
        self.delta_movement = Vec3d::new(
            post_move_delta.x * horizontal_drag,
            (post_move_delta.y - LOCAL_PLAYER_GRAVITY * tick_scale) * vertical_drag,
            post_move_delta.z * horizontal_drag,
        );

        Some(WalkingMovementResult {
            input,
            sprinting,
            collision,
            delta_movement: self.delta_movement,
        })
    }

    fn tick_water_movement(
        &mut self,
        client: &ClientRuntime,
        input: PlayerInput,
        step: WalkingMovementStep,
        sprinting: bool,
        tick_scale: f64,
    ) -> WalkingMovementResult {
        if input.jumping {
            self.delta_movement.y += LOCAL_PLAYER_FLUID_JUMP_POWER * tick_scale;
        }
        if input.shift_key_down {
            self.delta_movement.y -= LOCAL_PLAYER_FLUID_JUMP_POWER * tick_scale;
        }

        let acceleration = walking_input_acceleration(
            input,
            step.y_rot_degrees,
            LOCAL_PLAYER_WATER_MOVEMENT_SPEED * walking_speed_multiplier(step.speed_multiplier),
        )
        .scale(tick_scale);
        self.delta_movement = self.delta_movement.add(acceleration);

        let requested = self.delta_movement.scale(tick_scale);
        let collision = self.move_colliding(client, requested);

        let mut post_move_delta = self.delta_movement;
        if !nearly_equal(requested.x, collision.traveled.x) {
            post_move_delta.x = 0.0;
        }
        if !nearly_equal(requested.y, collision.traveled.y) {
            post_move_delta.y = 0.0;
        }
        if !nearly_equal(requested.z, collision.traveled.z) {
            post_move_delta.z = 0.0;
        }

        let horizontal_drag = if sprinting {
            LOCAL_PLAYER_WATER_SPRINT_SLOWDOWN
        } else {
            LOCAL_PLAYER_WATER_SLOWDOWN
        }
        .powf(tick_scale);
        let vertical_drag = LOCAL_PLAYER_WATER_VERTICAL_DRAG.powf(tick_scale);
        let mut y_delta = post_move_delta.y * vertical_drag;
        if !sprinting {
            y_delta = fluid_falling_adjusted_y(y_delta, tick_scale);
        }

        self.delta_movement = Vec3d::new(
            post_move_delta.x * horizontal_drag,
            y_delta,
            post_move_delta.z * horizontal_drag,
        );
        self.update_water_contact(client);

        WalkingMovementResult {
            input,
            sprinting,
            collision,
            delta_movement: self.delta_movement,
        }
    }

    fn consume_auto_jump_input(&mut self, input: &mut PlayerInput) -> bool {
        if self.auto_jump_time == 0 {
            return false;
        }

        self.auto_jump_time = self.auto_jump_time.saturating_sub(1);
        input.jumping = true;
        self.input = *input;
        true
    }

    fn should_schedule_auto_jump(
        &self,
        client: &ClientRuntime,
        input: PlayerInput,
        step: WalkingMovementStep,
        collision: CollisionMovementResult,
        auto_jumped: bool,
    ) -> bool {
        if !self.auto_jump_enabled
            || self.auto_jump_time != 0
            || auto_jumped
            || !collision.on_ground
            || !collision.horizontal_collision
            || input.shift_key_down
            || input.jumping
            || !input.is_moving()
            || self.touching_water
        {
            return false;
        }

        let requested = horizontal_only(collision.requested);
        let traveled = horizontal_only(collision.traveled);
        let blocked_movement = requested.subtract(traveled);
        let candidate_movement =
            if blocked_movement.length_sqr() > COLLISION_EPSILON * COLLISION_EPSILON {
                blocked_movement
            } else {
                requested
            };
        if candidate_movement.length_sqr() <= COLLISION_EPSILON * COLLISION_EPSILON {
            return false;
        }

        let forward = horizontal_only(view_vector_from_rot_degrees(0.0, step.y_rot_degrees));
        let forward = normalize_or_zero(forward);
        let candidate_direction = normalize_or_zero(candidate_movement);
        if forward != Vec3d::ZERO
            && candidate_direction != Vec3d::ZERO
            && dot(forward, candidate_direction) < LOCAL_PLAYER_AUTO_JUMP_MIN_MOVEMENT_DOT
        {
            return false;
        }

        can_auto_jump_over(client, self.bounding_box(), candidate_movement)
    }

    fn update_water_contact(&mut self, client: &ClientRuntime) -> WaterContact {
        let contact = water_contact_at_pose(client, self.pose, self.dimensions);
        self.touching_water = contact.touching;
        self.eye_in_water = contact.eye_in_water;
        self.water_height = contact.height;
        contact
    }

    fn clear_water_contact(&mut self) {
        self.touching_water = false;
        self.eye_in_water = false;
        self.water_height = 0.0;
    }

    pub fn move_colliding(
        &mut self,
        client: &ClientRuntime,
        requested: Vec3d,
    ) -> CollisionMovementResult {
        let traveled = collide_movement(client, self.bounding_box(), requested);
        if traveled.length_sqr() > COLLISION_EPSILON * COLLISION_EPSILON {
            self.pose.move_by(traveled);
        }

        self.horizontal_collision =
            !nearly_equal(requested.x, traveled.x) || !nearly_equal(requested.z, traveled.z);
        self.vertical_collision = !nearly_equal(requested.y, traveled.y);
        self.on_ground = self.vertical_collision && requested.y < 0.0;

        CollisionMovementResult {
            requested,
            traveled,
            horizontal_collision: self.horizontal_collision,
            vertical_collision: self.vertical_collision,
            on_ground: self.on_ground,
        }
    }

    pub fn move_colliding_horizontal_preserving_vertical_contact(
        &mut self,
        client: &ClientRuntime,
        requested: Vec3d,
    ) -> CollisionMovementResult {
        let requested = Vec3d::new(requested.x, 0.0, requested.z);
        let previous_vertical_collision = self.vertical_collision;
        let previous_on_ground = self.on_ground;
        let mut result = self.move_colliding(client, requested);
        self.vertical_collision = previous_vertical_collision;
        self.on_ground = previous_on_ground;
        result.vertical_collision = previous_vertical_collision;
        result.on_ground = previous_on_ground;
        result
    }
}

fn can_auto_jump_over(client: &ClientRuntime, bounding_box: Aabb, movement: Vec3d) -> bool {
    let movement = horizontal_only(movement);
    if movement.length_sqr() <= COLLISION_EPSILON * COLLISION_EPSILON {
        return false;
    }

    let raised = bounding_box.move_by(Vec3d::new(0.0, LOCAL_PLAYER_AUTO_JUMP_HEIGHT, 0.0));
    if !solid_block_aabbs_in(client, raised).is_empty() {
        return false;
    }

    let raised_movement = collide_movement(client, raised, movement);
    raised_movement.length_sqr() > COLLISION_EPSILON * COLLISION_EPSILON
        && raised_movement.length_sqr() >= movement.length_sqr() * 0.5
}

fn water_contact_at_pose(
    client: &ClientRuntime,
    pose: LocalPlayerPose,
    dimensions: LocalPlayerDimensions,
) -> WaterContact {
    let body = water_contact_in_aabb(
        client,
        deflate_aabb(pose.bounding_box_with_dimensions(dimensions), 0.001),
    );
    let eye = pose.eye_position();
    let eye_probe = Vec3d::new(eye.x, eye.y - 0.11111111, eye.z);
    WaterContact {
        eye_in_water: water_at_position(client, eye_probe),
        ..body
    }
}

fn water_contact_in_aabb(client: &ClientRuntime, area: Aabb) -> WaterContact {
    if !area.is_finite() {
        return WaterContact::default();
    }

    let min_x = area.min_x.floor() as i32;
    let min_y = area.min_y.floor() as i32;
    let min_z = area.min_z.floor() as i32;
    let max_x = area.max_x.ceil() as i32;
    let max_y = area.max_y.ceil() as i32;
    let max_z = area.max_z.ceil() as i32;
    let mut contact = WaterContact::default();

    for y in min_y..max_y {
        for z in min_z..max_z {
            for x in min_x..max_x {
                let pos = BlockPos::new(x, y, z);
                let Some(state) = client.block_state_at_block_pos(pos) else {
                    continue;
                };
                if block_fluid_kind(state) != BlockFluidKind::Water {
                    continue;
                }
                let Some(height) = block_fluid_height(state) else {
                    continue;
                };
                let fluid_top = f64::from(y) + f64::from(height);
                if fluid_top >= area.min_y {
                    contact.touching = true;
                    contact.height = contact.height.max(fluid_top - area.min_y);
                }
            }
        }
    }

    contact
}

fn water_at_position(client: &ClientRuntime, position: Vec3d) -> bool {
    if !position.is_finite() {
        return false;
    }

    let pos = BlockPos::containing(position);
    let Some(state) = client.block_state_at_block_pos(pos) else {
        return false;
    };
    if block_fluid_kind(state) != BlockFluidKind::Water {
        return false;
    }
    let Some(height) = block_fluid_height(state) else {
        return false;
    };
    position.y < f64::from(pos.y) + f64::from(height)
}

fn deflate_aabb(aabb: Aabb, amount: f64) -> Aabb {
    if !amount.is_finite() || amount <= 0.0 {
        return aabb;
    }
    Aabb::new(
        aabb.min_x + amount,
        aabb.min_y + amount,
        aabb.min_z + amount,
        aabb.max_x - amount,
        aabb.max_y - amount,
        aabb.max_z - amount,
    )
}

fn hand_probe_aabb(center: Vec3d, radius: f64) -> Aabb {
    let radius = if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        HAND_PUSH_DEFAULT_HAND_RADIUS
    };
    Aabb::new(
        center.x - radius,
        center.y - radius,
        center.z - radius,
        center.x + radius,
        center.y + radius,
        center.z + radius,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SphereSweepHit {
    center_position: Vec3d,
    normal: Vec3d,
    time: f64,
}

fn sweep_sphere_against_world(
    client: &ClientRuntime,
    start: Vec3d,
    radius: f64,
    movement: Vec3d,
) -> Option<SphereSweepHit> {
    if !start.is_finite() || !movement.is_finite() || movement.length_sqr() <= COLLISION_EPSILON {
        return None;
    }

    let radius = sanitize_hand_push_radius(radius, HAND_PUSH_DEFAULT_HAND_RADIUS);
    let sweep_area = hand_probe_aabb(start, radius).expand_towards(movement);
    let mut best_hit = None;
    for solid in solid_block_aabbs_in(client, sweep_area) {
        let expanded = inflate_aabb(solid, radius);
        let Some((time, normal)) = raycast_expanded_aabb(start, movement, expanded) else {
            continue;
        };
        if best_hit
            .map(|hit: SphereSweepHit| time < hit.time)
            .unwrap_or(true)
        {
            best_hit = Some(SphereSweepHit {
                center_position: start.add(movement.scale(time)),
                normal,
                time,
            });
        }
    }
    best_hit
}

fn validate_hand_push_head_movement(
    client: &ClientRuntime,
    last_head_position: Vec3d,
    current_head_position: Vec3d,
    requested_body_movement: Vec3d,
    radius: f64,
) -> Vec3d {
    if !last_head_position.is_finite()
        || !current_head_position.is_finite()
        || !requested_body_movement.is_finite()
    {
        return Vec3d::ZERO;
    }

    let requested_head_position = current_head_position.add(requested_body_movement);
    let head_movement = requested_head_position.subtract(last_head_position);
    let radius = sanitize_hand_push_radius(radius, HAND_PUSH_DEFAULT_HEAD_RADIUS);
    let Some(hit) = sweep_sphere_against_world(client, last_head_position, radius, head_movement)
    else {
        return requested_body_movement;
    };
    hit.center_position.subtract(last_head_position)
}

fn sanitize_hand_push_radius(radius: f64, fallback: f64) -> f64 {
    if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        fallback
    }
}

fn inflate_aabb(aabb: Aabb, amount: f64) -> Aabb {
    if !amount.is_finite() || amount <= 0.0 {
        return aabb;
    }
    Aabb::new(
        aabb.min_x - amount,
        aabb.min_y - amount,
        aabb.min_z - amount,
        aabb.max_x + amount,
        aabb.max_y + amount,
        aabb.max_z + amount,
    )
}

fn raycast_expanded_aabb(start: Vec3d, movement: Vec3d, target: Aabb) -> Option<(f64, Vec3d)> {
    if !target.is_finite() {
        return None;
    }
    if aabb_contains_point(target, start) {
        let normal = point_exit_normal(target, start);
        if movement.length_sqr() <= COLLISION_EPSILON || dot(movement, normal) <= COLLISION_EPSILON
        {
            return Some((0.0, normal));
        }
        return None;
    }

    let mut t_enter = 0.0;
    let mut t_exit = 1.0;
    let mut hit_normal = Vec3d::ZERO;
    update_raycast_axis(
        start.x,
        movement.x,
        target.min_x,
        target.max_x,
        Vec3d::new(-1.0, 0.0, 0.0),
        Vec3d::new(1.0, 0.0, 0.0),
        &mut t_enter,
        &mut t_exit,
        &mut hit_normal,
    )?;
    update_raycast_axis(
        start.y,
        movement.y,
        target.min_y,
        target.max_y,
        Vec3d::new(0.0, -1.0, 0.0),
        Vec3d::new(0.0, 1.0, 0.0),
        &mut t_enter,
        &mut t_exit,
        &mut hit_normal,
    )?;
    update_raycast_axis(
        start.z,
        movement.z,
        target.min_z,
        target.max_z,
        Vec3d::new(0.0, 0.0, -1.0),
        Vec3d::new(0.0, 0.0, 1.0),
        &mut t_enter,
        &mut t_exit,
        &mut hit_normal,
    )?;

    if t_enter > t_exit || !(-COLLISION_EPSILON..=1.0 + COLLISION_EPSILON).contains(&t_enter) {
        return None;
    }
    Some((t_enter.clamp(0.0, 1.0), hit_normal))
}

#[allow(clippy::too_many_arguments)]
fn update_raycast_axis(
    origin: f64,
    delta: f64,
    min: f64,
    max: f64,
    min_normal: Vec3d,
    max_normal: Vec3d,
    t_enter: &mut f64,
    t_exit: &mut f64,
    hit_normal: &mut Vec3d,
) -> Option<()> {
    if delta.abs() <= COLLISION_EPSILON {
        return (origin >= min - COLLISION_EPSILON && origin <= max + COLLISION_EPSILON)
            .then_some(());
    }

    let t_min_plane = (min - origin) / delta;
    let t_max_plane = (max - origin) / delta;
    let (near, far, normal) = if t_min_plane <= t_max_plane {
        (t_min_plane, t_max_plane, min_normal)
    } else {
        (t_max_plane, t_min_plane, max_normal)
    };

    if near > *t_enter {
        *t_enter = near;
        *hit_normal = normal;
    }
    *t_exit = (*t_exit).min(far);
    (*t_enter <= *t_exit + COLLISION_EPSILON).then_some(())
}

fn aabb_contains_point(aabb: Aabb, point: Vec3d) -> bool {
    point.x >= aabb.min_x - COLLISION_EPSILON
        && point.x <= aabb.max_x + COLLISION_EPSILON
        && point.y >= aabb.min_y - COLLISION_EPSILON
        && point.y <= aabb.max_y + COLLISION_EPSILON
        && point.z >= aabb.min_z - COLLISION_EPSILON
        && point.z <= aabb.max_z + COLLISION_EPSILON
}

fn point_exit_normal(aabb: Aabb, point: Vec3d) -> Vec3d {
    let faces = [
        (point.x - aabb.min_x, Vec3d::new(-1.0, 0.0, 0.0)),
        (aabb.max_x - point.x, Vec3d::new(1.0, 0.0, 0.0)),
        (point.y - aabb.min_y, Vec3d::new(0.0, -1.0, 0.0)),
        (aabb.max_y - point.y, Vec3d::new(0.0, 1.0, 0.0)),
        (point.z - aabb.min_z, Vec3d::new(0.0, 0.0, -1.0)),
        (aabb.max_z - point.z, Vec3d::new(0.0, 0.0, 1.0)),
    ];
    faces
        .into_iter()
        .min_by(|(left, _), (right, _)| left.total_cmp(right))
        .map(|(_, normal)| normal)
        .unwrap_or(Vec3d::new(0.0, 1.0, 0.0))
}

fn clamp_hand_reach(head_position: Vec3d, hand_position: Vec3d, max_arm_length: f64) -> Vec3d {
    if !max_arm_length.is_finite() || max_arm_length <= 0.0 {
        return hand_position;
    }
    let offset = hand_position.subtract(head_position);
    let length_sqr = offset.length_sqr();
    if length_sqr <= max_arm_length * max_arm_length {
        return hand_position;
    }
    head_position.add(normalize_or_zero(offset).scale(max_arm_length))
}

fn hand_push_jump_delta_movement(
    velocity_average: Vec3d,
    settings: HandPushLocomotionSettings,
) -> Vec3d {
    let speed = velocity_average.length_sqr().sqrt();
    if speed <= COLLISION_EPSILON {
        return Vec3d::ZERO;
    }
    let jump_speed = (speed * settings.jump_multiplier).min(settings.max_jump_speed);
    normalize_or_zero(velocity_average).scale(jump_speed / LOCAL_PLAYER_TICKS_PER_SECOND)
}

fn fluid_falling_adjusted_y(y_delta: f64, tick_scale: f64) -> f64 {
    let gravity = (LOCAL_PLAYER_GRAVITY / 16.0) * tick_scale;
    y_delta - gravity
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkingMovementResult {
    pub input: PlayerInput,
    pub sprinting: bool,
    pub collision: CollisionMovementResult,
    pub delta_movement: Vec3d,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollisionMovementResult {
    pub requested: Vec3d,
    pub traveled: Vec3d,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub on_ground: bool,
}

pub fn collide_movement(client: &ClientRuntime, bounding_box: Aabb, movement: Vec3d) -> Vec3d {
    mclone_blocks::collide_movement(
        |pos| client.block_state_at_block_pos(pos),
        bounding_box,
        movement,
    )
}

pub fn sphere_intersects_solid_blocks(client: &ClientRuntime, center: Vec3d, radius: f64) -> bool {
    if !center.is_finite() || !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    let radius_sqr = radius * radius;
    solid_block_aabbs_in(client, hand_probe_aabb(center, radius))
        .into_iter()
        .any(|solid| point_aabb_distance_sqr(center, solid) <= radius_sqr)
}

fn point_aabb_distance_sqr(point: Vec3d, aabb: Aabb) -> f64 {
    let dx = if point.x < aabb.min_x {
        aabb.min_x - point.x
    } else if point.x > aabb.max_x {
        point.x - aabb.max_x
    } else {
        0.0
    };
    let dy = if point.y < aabb.min_y {
        aabb.min_y - point.y
    } else if point.y > aabb.max_y {
        point.y - aabb.max_y
    } else {
        0.0
    };
    let dz = if point.z < aabb.min_z {
        aabb.min_z - point.z
    } else if point.z > aabb.max_z {
        point.z - aabb.max_z
    } else {
        0.0
    };
    dx * dx + dy * dy + dz * dz
}

fn solid_block_aabbs_in(client: &ClientRuntime, area: Aabb) -> Vec<Aabb> {
    mclone_blocks::solid_block_aabbs_in(|pos| client.block_state_at_block_pos(pos), area)
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < COLLISION_EPSILON
}

fn walking_input_speed(on_ground: bool, sprinting: bool) -> f64 {
    if on_ground {
        let base_speed = LOCAL_PLAYER_BASE_MOVEMENT_SPEED
            * if sprinting {
                LOCAL_PLAYER_SPRINT_SPEED_MULTIPLIER
            } else {
                1.0
            };
        base_speed
            * (LOCAL_PLAYER_GROUND_ACCELERATION_NUMERATOR / LOCAL_PLAYER_BLOCK_FRICTION.powi(3))
    } else if sprinting {
        LOCAL_PLAYER_AIR_SPRINT_SPEED
    } else {
        LOCAL_PLAYER_AIR_SPEED
    }
}

fn walking_speed_multiplier(multiplier: f64) -> f64 {
    if multiplier.is_finite() && multiplier > 0.0 {
        multiplier
    } else {
        1.0
    }
}

fn walking_input_acceleration(input: PlayerInput, y_rot_degrees: f64, speed: f64) -> Vec3d {
    if !y_rot_degrees.is_finite() || !speed.is_finite() || speed <= 0.0 {
        return Vec3d::ZERO;
    }

    let input_x = input.left_impulse as f64;
    let input_z = input.forward_impulse as f64;
    let input_len_sqr = input_x * input_x + input_z * input_z;
    if input_len_sqr < COLLISION_EPSILON {
        return Vec3d::ZERO;
    }

    let scale = if input_len_sqr > 1.0 {
        speed / input_len_sqr.sqrt()
    } else {
        speed
    };
    let input_x = input_x * scale;
    let input_z = input_z * scale;
    let y_rot = y_rot_degrees.to_radians();
    let y_sin = y_rot.sin();
    let y_cos = y_rot.cos();
    Vec3d::new(
        input_x * y_cos - input_z * y_sin,
        0.0,
        input_z * y_cos + input_x * y_sin,
    )
}

pub fn no_clip_displacement(input: PlayerInput, step: NoClipMovementStep) -> Option<Vec3d> {
    flying_displacement(
        input,
        FlyingMovementStep {
            yaw_radians: step.yaw_radians,
            pitch_radians: step.pitch_radians,
            speed_blocks_per_second: step.speed_blocks_per_second,
            dt_seconds: step.dt_seconds,
            descending: step.descending,
            sprinting: step.sprinting,
        },
    )
}

pub fn flying_displacement(input: PlayerInput, step: FlyingMovementStep) -> Option<Vec3d> {
    if !step.yaw_radians.is_finite()
        || !step.pitch_radians.is_finite()
        || !step.speed_blocks_per_second.is_finite()
        || !step.dt_seconds.is_finite()
        || step.dt_seconds <= 0.0
        || step.speed_blocks_per_second <= 0.0
    {
        return None;
    }

    let forward = view_vector(step.yaw_radians, step.pitch_radians);
    let right = normalize_or_zero(cross(forward, Vec3d::new(0.0, 1.0, 0.0)));
    let direction = right
        .scale(input.right_axis())
        .add(Vec3d::new(
            0.0,
            axis(input.jumping, step.descending) as f64,
            0.0,
        ))
        .add(forward.scale(input.forward_impulse as f64));
    let direction = normalize_or_zero(direction);
    if direction == Vec3d::ZERO {
        return None;
    }

    let boost = if step.sprinting {
        NO_CLIP_BOOST_MULTIPLIER
    } else {
        1.0
    };
    Some(direction.scale(step.speed_blocks_per_second * boost * step.dt_seconds))
}

pub fn view_vector(yaw_radians: f64, pitch_radians: f64) -> Vec3d {
    if !yaw_radians.is_finite() || !pitch_radians.is_finite() {
        return Vec3d::ZERO;
    }
    let (yaw_sin, yaw_cos) = yaw_radians.sin_cos();
    let (pitch_sin, pitch_cos) = pitch_radians.sin_cos();
    normalize_or_zero(Vec3d::new(
        yaw_sin * pitch_cos,
        pitch_sin,
        yaw_cos * pitch_cos,
    ))
}

pub fn view_vector_from_rot_degrees(x_rot_degrees: f64, y_rot_degrees: f64) -> Vec3d {
    if !x_rot_degrees.is_finite() || !y_rot_degrees.is_finite() {
        return Vec3d::ZERO;
    }
    let x_rot = x_rot_degrees.to_radians();
    let y_rot = -y_rot_degrees.to_radians();
    let y_cos = y_rot.cos();
    let y_sin = y_rot.sin();
    let x_cos = x_rot.cos();
    let x_sin = x_rot.sin();
    normalize_or_zero(Vec3d::new(y_sin * x_cos, -x_sin, y_cos * x_cos))
}

fn axis(positive: bool, negative: bool) -> f32 {
    let positive = positive as i32;
    let negative = negative as i32;
    (positive - negative) as f32
}

fn sanitize_movement_impulse(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn horizontal_only(value: Vec3d) -> Vec3d {
    Vec3d::new(value.x, 0.0, value.z)
}

fn dot(a: Vec3d, b: Vec3d) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: Vec3d, b: Vec3d) -> Vec3d {
    Vec3d::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn normalize_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= 1.0e-12 {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_blocks::WATER_BLOCK_STATE_ID;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot,
        ChunkStatus, SECTION_HEIGHT, chunk_section_index,
    };
    use mclone_protocol::{PlayerPositionRelativeFlags, ServerUpdate};

    fn assert_approx_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
            "expected {actual} to be approximately {expected}"
        );
    }

    fn client_with_blocks(blocks: &[(BlockPos, BlockStateId)]) -> ClientRuntime {
        let mut section_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        for (pos, state) in blocks {
            assert_eq!(pos.chunk_pos(), ChunkPos::new(0, 0));
            assert!((0..SECTION_HEIGHT).contains(&pos.y));
            let index = chunk_section_index(pos.x, pos.y, pos.z);
            section_blocks[index] = *state;
        }
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            SECTION_HEIGHT,
            &section_blocks,
        );
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(snapshot));
        client
    }

    fn one_java_tick_step(y_rot_degrees: f64) -> WalkingMovementStep {
        WalkingMovementStep {
            y_rot_degrees,
            speed_multiplier: 1.0,
            dt_seconds: 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND,
        }
    }

    fn settle_controller_on_ground(controller: &mut LocalPlayerController, client: &ClientRuntime) {
        controller.move_colliding(client, Vec3d::new(0.0, -0.01, 0.0));
        assert!(controller.on_ground());
        controller.set_delta_movement(Vec3d::new(
            0.0,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
            0.0,
        ));
    }

    fn controller_on_ground() -> LocalPlayerController {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 0.5),
            ..Default::default()
        });
        controller
    }

    fn forward_step_client(extra_blocks: &[(BlockPos, BlockStateId)]) -> ClientRuntime {
        let mut blocks = vec![(BlockPos::new(0, 0, 0), BlockStateId(7))];
        blocks.extend_from_slice(extra_blocks);
        client_with_blocks(&blocks)
    }

    fn water_column_client() -> ClientRuntime {
        client_with_blocks(&[
            (BlockPos::new(0, 1, 0), WATER_BLOCK_STATE_ID),
            (BlockPos::new(0, 2, 0), WATER_BLOCK_STATE_ID),
        ])
    }

    fn shallow_water_client() -> ClientRuntime {
        client_with_blocks(&[(BlockPos::new(0, 1, 0), WATER_BLOCK_STATE_ID)])
    }

    fn controller_in_water() -> LocalPlayerController {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 0.5),
            ..Default::default()
        });
        controller
    }

    #[test]
    fn hand_push_controller_moves_player_from_braced_hand() {
        let client = forward_step_client(&[]);
        let mut player = controller_on_ground();
        let settings = HandPushLocomotionSettings {
            max_arm_length: 4.0,
            velocity_limit: 1000.0,
            ..HandPushLocomotionSettings::default()
        };
        let mut hand_push = HandPushLocomotionController::new(settings);
        let head = Vec3d::new(0.5, 2.62, 0.5);
        let right = Vec3d::new(0.75, 1.7, 0.5);

        let initial = HandPushPose {
            head_position: head,
            left_hand_position: Vec3d::new(0.5, 1.04, 0.5),
            right_hand_position: right,
        };
        hand_push.tick(
            &client,
            &mut player,
            HandPushMovementStep {
                dt_seconds: 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND,
                pose: initial,
            },
        );

        let result = hand_push
            .tick(
                &client,
                &mut player,
                HandPushMovementStep {
                    dt_seconds: 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND,
                    pose: HandPushPose {
                        left_hand_position: Vec3d::new(0.5, 1.04, 0.2),
                        ..initial
                    },
                },
            )
            .expect("valid hand-push tick");

        assert!(result.left_hand_touching);
        assert!(result.body_movement.z > 0.0);
        assert!(player.pose().position.z > 0.5);
    }

    #[test]
    fn hand_push_swept_sphere_hits_wall_before_overlap() {
        let client = client_with_blocks(&[(BlockPos::new(1, 2, 0), BlockStateId(7))]);

        let hit = sweep_sphere_against_world(
            &client,
            Vec3d::new(0.5, 2.5, 0.5),
            0.1,
            Vec3d::new(1.0, 0.0, 0.0),
        )
        .expect("sphere should hit the expanded wall");

        assert_approx_eq(hit.center_position.x, 0.9);
        assert_approx_eq(hit.center_position.y, 2.5);
        assert_approx_eq(hit.center_position.z, 0.5);
        assert_approx_eq(hit.normal.x, -1.0);
        assert_approx_eq(hit.normal.y, 0.0);
        assert_approx_eq(hit.normal.z, 0.0);
        assert_approx_eq(hit.time, 0.4);
    }

    #[test]
    fn hand_push_head_validation_clamps_body_motion_before_wall() {
        let client = client_with_blocks(&[(BlockPos::new(0, 2, 1), BlockStateId(7))]);

        let movement = validate_hand_push_head_movement(
            &client,
            Vec3d::new(0.5, 2.5, 0.5),
            Vec3d::new(0.5, 2.5, 0.5),
            Vec3d::new(0.0, 0.0, 1.0),
            0.1,
        );

        assert_approx_eq(movement.x, 0.0);
        assert_approx_eq(movement.y, 0.0);
        assert_approx_eq(movement.z, 0.4);
    }

    #[test]
    fn input_tick_matches_java_keyboard_impulses() {
        let mut keys = PlayerInputKeys::default();
        let mut input = PlayerInput::default();

        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Left, true);
        input.tick(keys, false);

        assert_eq!(input.forward_impulse, 1.0);
        assert_eq!(input.left_impulse, 1.0);
        assert!(input.has_forward_impulse());

        keys.set(PlayerInputKey::Backward, true);
        keys.set(PlayerInputKey::Right, true);
        input.tick(keys, false);

        assert_eq!(input.forward_impulse, 0.0);
        assert_eq!(input.left_impulse, 0.0);
        assert!(!input.has_forward_impulse());
    }

    #[test]
    fn moving_slowly_scales_horizontal_impulses_only() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Jump, true);
        keys.set(PlayerInputKey::Shift, true);
        let mut input = PlayerInput::default();

        input.tick(keys, true);

        assert_eq!(input.forward_impulse, MOVING_SLOW_FACTOR);
        assert!(input.jumping);
        assert!(input.shift_key_down);
    }

    #[test]
    fn analog_movement_impulse_overrides_keyboard_axes() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Left, true);
        let mut input = PlayerInput::default();

        input.tick_with_movement_impulse(keys, false, Some((-0.25, 0.5)));

        assert_eq!(input.forward_impulse, 0.5);
        assert_eq!(input.left_impulse, -0.25);
        assert!(input.up);
        assert!(!input.down);
        assert!(!input.left);
        assert!(input.right);
        assert!(input.has_forward_impulse());
    }

    #[test]
    fn no_clip_movement_preserves_existing_forward_axis() {
        let mut input = PlayerInput::default();
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        input.tick(keys, false);

        let displacement = no_clip_displacement(
            input,
            NoClipMovementStep {
                yaw_radians: 0.0,
                pitch_radians: 0.0,
                speed_blocks_per_second: 10.0,
                dt_seconds: 0.5,
                descending: false,
                sprinting: false,
            },
        )
        .expect("movement");

        assert_eq!(displacement, Vec3d::new(0.0, 0.0, 5.0));
    }

    #[test]
    fn no_clip_movement_normalizes_diagonal_and_applies_boost() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Jump, true);
        let mut input = PlayerInput::default();
        input.tick(keys, false);

        let displacement = no_clip_displacement(
            input,
            NoClipMovementStep {
                yaw_radians: 0.0,
                pitch_radians: 0.0,
                speed_blocks_per_second: 2.0,
                dt_seconds: 1.0,
                descending: false,
                sprinting: true,
            },
        )
        .expect("movement");

        assert!((displacement.length_sqr().sqrt() - 6.0).abs() < 1.0e-9);
        assert!(displacement.y > 0.0);
        assert!(displacement.z > 0.0);
    }

    #[test]
    fn controller_clears_keys_and_cached_input() {
        let mut controller = LocalPlayerController::new();
        controller.set_key(PlayerInputKey::Forward, true);
        controller.tick_input(false);

        controller.clear_keys();

        assert_eq!(controller.keys(), PlayerInputKeys::default());
        assert_eq!(controller.input(), PlayerInput::default());
    }

    #[test]
    fn controller_applies_no_clip_only_descend_and_sprint_keys() {
        let mut controller = LocalPlayerController::new();
        controller.set_key(PlayerInputKey::Descend, true);
        controller.set_key(PlayerInputKey::Sprint, true);

        let displacement = controller
            .tick_no_clip_movement(NoClipMovementStep {
                yaw_radians: 0.0,
                pitch_radians: 0.0,
                speed_blocks_per_second: 2.0,
                dt_seconds: 1.0,
                descending: false,
                sprinting: false,
            })
            .expect("movement");

        assert_eq!(displacement, Vec3d::new(0.0, -6.0, 0.0));
        assert!(!controller.input().shift_key_down);
        assert_eq!(controller.delta_movement(), Vec3d::ZERO);
    }

    #[test]
    fn flying_movement_collides_with_full_block_wall() {
        let client = client_with_blocks(&[(BlockPos::new(1, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 0.0, 0.5),
            ..Default::default()
        });
        controller.set_key(PlayerInputKey::Forward, true);

        let result = controller
            .tick_flying_movement(
                &client,
                FlyingMovementStep {
                    yaw_radians: std::f64::consts::FRAC_PI_2,
                    pitch_radians: 0.0,
                    speed_blocks_per_second: 20.0,
                    dt_seconds: 0.1,
                    descending: false,
                    sprinting: false,
                },
            )
            .expect("flying movement");

        assert_approx_eq(result.requested.x, 2.0);
        assert_approx_eq(result.traveled.x, 0.2);
        assert!(result.horizontal_collision);
        assert!(!result.vertical_collision);
        assert_eq!(controller.delta_movement(), Vec3d::ZERO);
        assert_approx_eq(controller.pose().position.x, 0.7);
    }

    #[test]
    fn no_clip_movement_passes_through_full_block_wall() {
        let _client = client_with_blocks(&[(BlockPos::new(1, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 0.0, 0.5),
            ..Default::default()
        });
        controller.set_key(PlayerInputKey::Forward, true);

        let displacement = controller
            .tick_no_clip_movement(NoClipMovementStep {
                yaw_radians: std::f64::consts::FRAC_PI_2,
                pitch_radians: 0.0,
                speed_blocks_per_second: 20.0,
                dt_seconds: 0.1,
                descending: false,
                sprinting: false,
            })
            .expect("no-clip movement");

        assert_approx_eq(displacement.x, 2.0);
        assert!(!controller.horizontal_collision());
        assert!(!controller.vertical_collision());
        assert_approx_eq(controller.pose().position.x, 2.5);
    }

    #[test]
    fn local_player_pose_tracks_eye_position_and_chunk_from_feet() {
        let pose =
            LocalPlayerPose::from_eye_position(Vec3d::new(17.5, 70.0, -0.25), 0.0, 0.0, 1.62);

        assert_eq!(pose.position, Vec3d::new(17.5, 68.38, -0.25));
        assert_eq!(pose.eye_position(), Vec3d::new(17.5, 70.0, -0.25));
        assert_eq!(pose.block_position(), BlockPos::new(17, 68, -1));
        assert_eq!(pose.eye_block_position(), BlockPos::new(17, 70, -1));
        assert_eq!(pose.chunk_pos(), ChunkPos::new(1, -1));
    }

    #[test]
    fn controller_selects_java_shaped_move_player_command_variants() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(1.25, 63.0, -4.5),
            y_rot_degrees: -181.5,
            x_rot_degrees: 45.25,
            eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        });

        assert_eq!(
            controller.next_move_player_command(),
            Some(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(1.25, 63.0, -4.5),
                y_rot_degrees: -181.5,
                x_rot_degrees: 45.25,
                on_ground: false,
            }))
        );
        assert_eq!(controller.next_move_player_command(), None);

        controller.set_pose(LocalPlayerPose {
            y_rot_degrees: -90.0,
            ..controller.pose()
        });
        assert_eq!(
            controller.next_move_player_command(),
            Some(ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: -90.0,
                x_rot_degrees: 45.25,
                on_ground: false,
            }))
        );
        assert_eq!(controller.next_move_player_command(), None);

        controller.on_ground = true;
        assert_eq!(
            controller.next_move_player_command(),
            Some(ClientCommand::MovePlayer(MovePlayerCommand::StatusOnly {
                on_ground: true,
            }))
        );
        assert_eq!(controller.next_move_player_command(), None);

        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(1.31, 63.0, -4.5),
            ..controller.pose()
        });
        assert_eq!(
            controller.next_move_player_command(),
            Some(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(1.31, 63.0, -4.5),
                on_ground: true,
            }))
        );
    }

    #[test]
    fn controller_forces_position_sync_after_java_reminder_interval() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.01, 0.0, 0.0),
            ..controller.pose()
        });

        for _ in 0..(LOCAL_PLAYER_POSITION_REMINDER_INTERVAL - 1) {
            assert_eq!(controller.next_move_player_command(), None);
        }
        assert_eq!(
            controller.next_move_player_command(),
            Some(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(0.01, 0.0, 0.0),
                on_ground: false,
            }))
        );
        assert_eq!(controller.next_move_player_command(), None);
    }

    #[test]
    fn controller_can_force_pos_rot_sync_after_server_position_update() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(1.25, 63.0, -4.5),
            y_rot_degrees: -181.5,
            x_rot_degrees: 45.25,
            eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        });

        assert_eq!(
            controller.pos_rot_move_player_command(),
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(1.25, 63.0, -4.5),
                y_rot_degrees: -181.5,
                x_rot_degrees: 45.25,
                on_ground: false,
            })
        );
        assert_eq!(controller.next_move_player_command(), None);
    }

    #[test]
    fn controller_applies_player_position_update_and_builds_teleport_ack() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: 10.0,
            x_rot_degrees: -20.0,
            eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        });
        controller.set_delta_movement(Vec3d::new(1.0, 0.5, -1.0));
        controller.on_ground = true;

        let ack = controller.apply_player_position_update(PlayerPositionUpdate {
            position: Vec3d::new(4.0, 5.0, 6.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 30.0,
            relative: PlayerPositionRelativeFlags::ABSOLUTE,
            teleport_id: 12,
            dismount_vehicle: false,
        });

        assert_eq!(
            ack,
            ClientCommand::AcceptTeleport(AcceptTeleportCommand { id: 12 })
        );
        assert_eq!(controller.pose().position, Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(controller.pose().y_rot_degrees, 90.0);
        assert_eq!(controller.pose().x_rot_degrees, 30.0);
        assert_eq!(controller.delta_movement(), Vec3d::ZERO);
        assert!(!controller.on_ground());
    }

    #[test]
    fn controller_applies_relative_player_position_update() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: 10.0,
            x_rot_degrees: -20.0,
            eye_height: LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        });

        controller.apply_player_position_update(PlayerPositionUpdate {
            position: Vec3d::new(4.0, 5.0, 6.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 30.0,
            relative: PlayerPositionRelativeFlags {
                x: true,
                y: false,
                z: true,
                y_rot: true,
                x_rot: false,
            },
            teleport_id: 13,
            dismount_vehicle: false,
        });

        assert_eq!(controller.pose().position, Vec3d::new(5.0, 5.0, 9.0));
        assert_eq!(controller.pose().y_rot_degrees, 100.0);
        assert_eq!(controller.pose().x_rot_degrees, 30.0);
    }

    #[test]
    fn local_player_pose_uses_java_view_vector_signs() {
        assert_eq!(
            view_vector_from_rot_degrees(0.0, 0.0),
            Vec3d::new(0.0, 0.0, 1.0)
        );

        let east = view_vector_from_rot_degrees(0.0, -90.0);
        assert!((east.x - 1.0).abs() < 1.0e-12);
        assert!(east.y.abs() < 1.0e-12);
        assert!(east.z.abs() < 1.0e-12);

        let down = view_vector_from_rot_degrees(90.0, 0.0);
        assert!(down.x.abs() < 1.0e-12);
        assert!((down.y + 1.0).abs() < 1.0e-12);
        assert!(down.z.abs() < 1.0e-12);
    }

    #[test]
    fn local_player_pose_turn_clamps_x_rot_like_entity_turn() {
        let mut pose = LocalPlayerPose::default();

        pose.turn_degrees(-45.0, 180.0);

        assert_eq!(pose.y_rot_degrees, -45.0);
        assert_eq!(pose.x_rot_degrees, LOCAL_PLAYER_X_ROT_LIMIT_DEGREES);
    }

    #[test]
    fn local_player_pose_bounding_box_uses_java_standing_dimensions() {
        let pose = LocalPlayerPose {
            position: Vec3d::new(0.5, 2.0, 0.5),
            ..Default::default()
        };

        assert_eq!(pose.bounding_box(), Aabb::new(0.2, 2.0, 0.2, 0.8, 3.8, 0.8));
    }

    #[test]
    fn local_player_controller_uses_active_dimensions_for_collision_box() {
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 2.0, 0.5),
            ..Default::default()
        });

        assert_eq!(
            controller.bounding_box(),
            Aabb::new(0.2, 2.0, 0.2, 0.8, 3.8, 0.8)
        );

        controller.set_dimensions(LocalPlayerDimensions::HAND_PUSH);

        assert_eq!(controller.pose().eye_position(), Vec3d::new(0.5, 2.6, 0.5));
        assert_eq!(
            controller.bounding_box(),
            Aabb::new(0.3, 2.0, 0.3, 0.7, 2.6, 0.7)
        );
    }

    #[test]
    fn colliding_movement_stops_at_full_block_wall() {
        let client = client_with_blocks(&[(BlockPos::new(1, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 0.0, 0.5),
            ..Default::default()
        });

        let result = controller.move_colliding(&client, Vec3d::new(2.0, 0.0, 0.0));

        assert_approx_eq(result.traveled.x, 0.2);
        assert_approx_eq(result.traveled.y, 0.0);
        assert_approx_eq(result.traveled.z, 0.0);
        assert!(result.horizontal_collision);
        assert!(!result.vertical_collision);
        assert!(!result.on_ground);
        assert!(controller.horizontal_collision());
        assert!(!controller.vertical_collision());
        assert!(!controller.on_ground());
        assert_approx_eq(controller.pose().position.x, 0.7);
    }

    #[test]
    fn colliding_movement_lands_on_full_block() {
        let client = client_with_blocks(&[(BlockPos::new(0, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 2.0, 0.5),
            ..Default::default()
        });

        let result = controller.move_colliding(&client, Vec3d::new(0.0, -3.0, 0.0));

        assert_approx_eq(result.traveled.x, 0.0);
        assert_approx_eq(result.traveled.y, -1.0);
        assert_approx_eq(result.traveled.z, 0.0);
        assert!(!result.horizontal_collision);
        assert!(result.vertical_collision);
        assert!(result.on_ground);
        assert!(controller.on_ground());
        assert_approx_eq(controller.pose().position.y, 1.0);
    }

    #[test]
    fn colliding_movement_slides_along_wall() {
        let client = client_with_blocks(&[(BlockPos::new(1, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 0.0, 0.5),
            ..Default::default()
        });

        let result = controller.move_colliding(&client, Vec3d::new(2.0, 0.0, 1.0));

        assert_approx_eq(result.traveled.x, 0.2);
        assert_approx_eq(result.traveled.y, 0.0);
        assert_approx_eq(result.traveled.z, 1.0);
        assert!(result.horizontal_collision);
        assert!(!result.vertical_collision);
        assert_eq!(controller.pose().position, Vec3d::new(0.7, 0.0, 1.5));
    }

    #[test]
    fn colliding_movement_ignores_air_and_unloaded_blocks() {
        let client = ClientRuntime::local_integrated();
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 0.0, 0.5),
            ..Default::default()
        });

        let result = controller.move_colliding(&client, Vec3d::new(2.0, -1.0, 1.0));

        assert_eq!(result.traveled, Vec3d::new(2.0, -1.0, 1.0));
        assert!(!result.horizontal_collision);
        assert!(!result.vertical_collision);
        assert!(!result.on_ground);
        assert_eq!(controller.pose().position, Vec3d::new(2.5, -1.0, 1.5));
    }

    #[test]
    fn colliding_movement_ignores_java_empty_collision_blocks() {
        for state in [
            BlockStateId(2),
            BlockStateId(8),
            BlockStateId(9),
            BlockStateId(43),
            BlockStateId(44),
            BlockStateId(45),
            BlockStateId(50),
            BlockStateId(51),
            BlockStateId(68),
            BlockStateId(69),
            BlockStateId(70),
        ] {
            let client = client_with_blocks(&[(BlockPos::new(1, 0, 0), state)]);
            let mut controller = LocalPlayerController::new();
            controller.set_pose(LocalPlayerPose {
                position: Vec3d::new(0.5, 0.0, 0.5),
                ..Default::default()
            });

            let result = controller.move_colliding(&client, Vec3d::new(2.0, 0.0, 0.0));

            assert_eq!(result.traveled, Vec3d::new(2.0, 0.0, 0.0), "{state:?}");
            assert!(!result.horizontal_collision, "{state:?}");
            assert!(!result.vertical_collision, "{state:?}");
            assert_eq!(controller.pose().position, Vec3d::new(2.5, 0.0, 0.5));
        }
    }

    #[test]
    fn sphere_intersection_detects_solid_collision_shape() {
        let client = client_with_blocks(&[(BlockPos::new(1, 1, 0), BlockStateId(7))]);

        assert!(sphere_intersects_solid_blocks(
            &client,
            Vec3d::new(1.5, 1.5, 0.5),
            0.2
        ));
        assert!(!sphere_intersects_solid_blocks(
            &client,
            Vec3d::new(0.5, 1.5, 0.5),
            0.2
        ));
    }

    #[test]
    fn sphere_intersection_uses_sphere_distance_not_probe_box_corners() {
        let client = client_with_blocks(&[(BlockPos::new(1, 1, 1), BlockStateId(7))]);

        assert!(!sphere_intersects_solid_blocks(
            &client,
            Vec3d::new(0.8, 1.5, 0.8),
            0.25
        ));
    }

    #[test]
    fn walking_movement_uses_java_yaw_rotated_air_input() {
        let client = ClientRuntime::local_integrated();
        let mut controller = LocalPlayerController::new();
        controller.set_key(PlayerInputKey::Forward, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert_approx_eq(result.collision.traveled.x, 0.0);
        assert_approx_eq(result.collision.traveled.y, 0.0);
        assert_approx_eq(result.collision.traveled.z, LOCAL_PLAYER_AIR_SPEED);
        assert_approx_eq(result.delta_movement.x, 0.0);
        assert_approx_eq(
            result.delta_movement.y,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
        );
        assert_approx_eq(
            result.delta_movement.z,
            LOCAL_PLAYER_AIR_SPEED * LOCAL_PLAYER_FRICTION_MULTIPLIER,
        );

        let mut controller = LocalPlayerController::new();
        controller.set_key(PlayerInputKey::Forward, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(-90.0))
            .expect("walking movement");

        assert_approx_eq(result.collision.traveled.x, LOCAL_PLAYER_AIR_SPEED);
        assert_approx_eq(result.collision.traveled.z, 0.0);
    }

    #[test]
    fn walking_movement_jumps_from_ground_and_applies_gravity_drag() {
        let client = client_with_blocks(&[(BlockPos::new(0, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 0.5),
            ..Default::default()
        });
        settle_controller_on_ground(&mut controller, &client);
        controller.set_key(PlayerInputKey::Jump, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert_approx_eq(result.collision.traveled.y, LOCAL_PLAYER_JUMP_POWER);
        assert!(!result.collision.on_ground);
        assert_approx_eq(controller.pose().position.y, 1.0 + LOCAL_PLAYER_JUMP_POWER);
        assert_approx_eq(
            result.delta_movement.y,
            (LOCAL_PLAYER_JUMP_POWER - LOCAL_PLAYER_GRAVITY) * LOCAL_PLAYER_VERTICAL_DRAG,
        );
    }

    #[test]
    fn walking_movement_sneak_scales_ground_input_and_keeps_ground_probe() {
        let client = client_with_blocks(&[(BlockPos::new(0, 0, 0), BlockStateId(7))]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 0.5),
            ..Default::default()
        });
        settle_controller_on_ground(&mut controller, &client);
        controller.set_key(PlayerInputKey::Forward, true);
        controller.set_key(PlayerInputKey::Shift, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        let expected_ground_speed = LOCAL_PLAYER_BASE_MOVEMENT_SPEED
            * (LOCAL_PLAYER_GROUND_ACCELERATION_NUMERATOR / LOCAL_PLAYER_BLOCK_FRICTION.powi(3));
        let expected_z = expected_ground_speed * MOVING_SLOW_FACTOR as f64;
        assert_approx_eq(
            result.input.forward_impulse as f64,
            MOVING_SLOW_FACTOR as f64,
        );
        assert_approx_eq(result.collision.traveled.y, 0.0);
        assert!(result.collision.on_ground);
        assert_approx_eq(result.collision.traveled.z, expected_z);
        assert_approx_eq(
            result.delta_movement.z,
            expected_z * (LOCAL_PLAYER_BLOCK_FRICTION * LOCAL_PLAYER_FRICTION_MULTIPLIER),
        );
    }

    #[test]
    fn walking_movement_zeroes_velocity_on_wall_collision() {
        let client = client_with_blocks(&[
            (BlockPos::new(0, 0, 0), BlockStateId(7)),
            (BlockPos::new(0, 1, 1), BlockStateId(7)),
        ]);
        let mut controller = controller_on_ground();
        settle_controller_on_ground(&mut controller, &client);
        controller.set_delta_movement(Vec3d::new(
            0.0,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
            1.0,
        ));

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert_approx_eq(result.collision.traveled.z, 0.2);
        assert!(result.collision.horizontal_collision);
        assert!(result.collision.on_ground);
        assert_approx_eq(result.delta_movement.z, 0.0);
    }

    #[test]
    fn walking_movement_auto_jumps_over_one_block_obstacle() {
        let client = forward_step_client(&[(BlockPos::new(0, 1, 1), BlockStateId(7))]);
        let mut controller = controller_on_ground();
        settle_controller_on_ground(&mut controller, &client);
        controller.set_key(PlayerInputKey::Forward, true);
        controller.set_delta_movement(Vec3d::new(
            0.0,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
            1.0,
        ));

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(controller.auto_jump_enabled());
        assert_approx_eq(result.collision.traveled.z, 0.2);
        assert!(result.collision.horizontal_collision);
        assert!(result.collision.on_ground);
        assert!(!result.input.jumping);
        assert_eq!(controller.auto_jump_time(), 1);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(result.input.jumping);
        assert_approx_eq(result.collision.traveled.y, LOCAL_PLAYER_JUMP_POWER);
        assert!(!result.collision.on_ground);
        assert_eq!(controller.auto_jump_time(), 0);
        assert_approx_eq(controller.pose().position.y, 1.0 + LOCAL_PLAYER_JUMP_POWER);
    }

    #[test]
    fn walking_movement_auto_jump_respects_sneak_and_disabled_option() {
        let client = forward_step_client(&[(BlockPos::new(0, 1, 1), BlockStateId(7))]);
        for (sneaking, enabled) in [(true, true), (false, false)] {
            let mut controller = controller_on_ground();
            settle_controller_on_ground(&mut controller, &client);
            controller.set_key(PlayerInputKey::Forward, true);
            controller.set_key(PlayerInputKey::Shift, sneaking);
            controller.set_auto_jump_enabled(enabled);
            controller.set_delta_movement(Vec3d::new(
                0.0,
                -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
                1.0,
            ));

            let result = controller
                .tick_walking_movement(&client, one_java_tick_step(0.0))
                .expect("walking movement");

            assert!(result.collision.horizontal_collision);
            assert!(result.collision.on_ground);
            assert_eq!(controller.auto_jump_time(), 0);
        }
    }

    #[test]
    fn walking_movement_auto_jump_ignores_backwards_collisions() {
        let client = client_with_blocks(&[
            (BlockPos::new(0, 0, 1), BlockStateId(7)),
            (BlockPos::new(0, 1, 0), BlockStateId(7)),
        ]);
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 1.5),
            ..Default::default()
        });
        settle_controller_on_ground(&mut controller, &client);
        controller.set_key(PlayerInputKey::Backward, true);
        controller.set_delta_movement(Vec3d::new(
            0.0,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
            -1.0,
        ));

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert_approx_eq(result.collision.traveled.z, -0.2);
        assert!(result.collision.horizontal_collision);
        assert!(result.collision.on_ground);
        assert_eq!(controller.auto_jump_time(), 0);
    }

    #[test]
    fn walking_movement_auto_jump_requires_headroom() {
        let client = forward_step_client(&[
            (BlockPos::new(0, 1, 1), BlockStateId(7)),
            (BlockPos::new(0, 2, 1), BlockStateId(7)),
        ]);
        let mut controller = controller_on_ground();
        settle_controller_on_ground(&mut controller, &client);
        controller.set_key(PlayerInputKey::Forward, true);
        controller.set_delta_movement(Vec3d::new(
            0.0,
            -LOCAL_PLAYER_GRAVITY * LOCAL_PLAYER_VERTICAL_DRAG,
            1.0,
        ));

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(result.collision.horizontal_collision);
        assert!(result.collision.on_ground);
        assert_eq!(controller.auto_jump_time(), 0);
    }

    #[test]
    fn walking_movement_swims_in_water_with_java_drag() {
        let client = water_column_client();
        let mut controller = controller_in_water();
        controller.set_key(PlayerInputKey::Forward, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(controller.touching_water());
        assert!(controller.eye_in_water());
        assert!(controller.water_height() > 1.0);
        assert_approx_eq(
            result.collision.traveled.z,
            LOCAL_PLAYER_WATER_MOVEMENT_SPEED,
        );
        assert_approx_eq(
            result.delta_movement.z,
            LOCAL_PLAYER_WATER_MOVEMENT_SPEED * LOCAL_PLAYER_WATER_SLOWDOWN,
        );
        assert_approx_eq(result.delta_movement.y, -LOCAL_PLAYER_GRAVITY / 16.0);
    }

    #[test]
    fn walking_movement_jumps_up_in_water() {
        let client = water_column_client();
        let mut controller = controller_in_water();
        controller.set_key(PlayerInputKey::Jump, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(controller.touching_water());
        assert!(result.input.jumping);
        assert_approx_eq(result.collision.traveled.y, LOCAL_PLAYER_FLUID_JUMP_POWER);
        assert_approx_eq(
            result.delta_movement.y,
            LOCAL_PLAYER_FLUID_JUMP_POWER * LOCAL_PLAYER_WATER_VERTICAL_DRAG
                - LOCAL_PLAYER_GRAVITY / 16.0,
        );
    }

    #[test]
    fn walking_movement_shift_descends_in_water() {
        let client = water_column_client();
        let mut controller = controller_in_water();
        controller.set_key(PlayerInputKey::Shift, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(controller.touching_water());
        assert!(result.input.shift_key_down);
        assert_approx_eq(result.collision.traveled.y, -LOCAL_PLAYER_FLUID_JUMP_POWER);
        assert_approx_eq(
            result.delta_movement.y,
            -LOCAL_PLAYER_FLUID_JUMP_POWER * LOCAL_PLAYER_WATER_VERTICAL_DRAG
                - LOCAL_PLAYER_GRAVITY / 16.0,
        );
    }

    #[test]
    fn walking_movement_sprint_swims_with_java_water_slowdown() {
        let client = water_column_client();
        let mut controller = controller_in_water();
        controller.set_key(PlayerInputKey::Forward, true);
        controller.set_key(PlayerInputKey::Sprint, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(result.sprinting);
        assert_approx_eq(
            result.delta_movement.z,
            LOCAL_PLAYER_WATER_MOVEMENT_SPEED * LOCAL_PLAYER_WATER_SPRINT_SLOWDOWN,
        );
        assert_approx_eq(result.delta_movement.y, 0.0);
    }

    #[test]
    fn walking_movement_shallow_water_disables_sprint_swim() {
        let client = shallow_water_client();
        let mut controller = controller_in_water();
        controller.set_key(PlayerInputKey::Forward, true);
        controller.set_key(PlayerInputKey::Sprint, true);

        let result = controller
            .tick_walking_movement(&client, one_java_tick_step(0.0))
            .expect("walking movement");

        assert!(controller.touching_water());
        assert!(!controller.eye_in_water());
        assert!(!result.sprinting);
        assert_approx_eq(
            result.delta_movement.z,
            LOCAL_PLAYER_WATER_MOVEMENT_SPEED * LOCAL_PLAYER_WATER_SLOWDOWN,
        );
        assert_approx_eq(result.delta_movement.y, -LOCAL_PLAYER_GRAVITY / 16.0);
    }

    #[test]
    fn no_clip_movement_rejects_non_finite_step_inputs() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        let mut input = PlayerInput::default();
        input.tick(keys, false);

        assert_eq!(
            no_clip_displacement(
                input,
                NoClipMovementStep {
                    yaw_radians: f64::NAN,
                    pitch_radians: 0.0,
                    speed_blocks_per_second: 1.0,
                    dt_seconds: 1.0,
                    descending: false,
                    sprinting: false,
                },
            ),
            None
        );
        assert_eq!(view_vector(f64::NAN, 0.0), Vec3d::ZERO);
    }
}
