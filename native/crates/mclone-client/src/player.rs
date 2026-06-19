use mclone_core::{Aabb, BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    AcceptTeleportCommand, ClientCommand, MovePlayerCommand, PlayerPositionUpdate,
};

use crate::{ClientRuntime, block_shapes::block_collision_aabb};

pub const NO_CLIP_BOOST_MULTIPLIER: f64 = 3.0;
pub const MOVING_SLOW_FACTOR: f32 = 0.3;
pub const LOCAL_PLAYER_STANDING_EYE_HEIGHT: f64 = 1.62;
pub const LOCAL_PLAYER_STANDING_WIDTH: f64 = 0.6;
pub const LOCAL_PLAYER_STANDING_HEIGHT: f64 = 1.8;
pub const LOCAL_PLAYER_X_ROT_LIMIT_DEGREES: f64 = 90.0;
pub const LOCAL_PLAYER_TICKS_PER_SECOND: f64 = 20.0;
pub const LOCAL_PLAYER_BASE_MOVEMENT_SPEED: f64 = 0.1;
pub const LOCAL_PLAYER_SPRINT_SPEED_MULTIPLIER: f64 = 1.3;
pub const LOCAL_PLAYER_AIR_SPEED: f64 = 0.02;
pub const LOCAL_PLAYER_AIR_SPRINT_SPEED: f64 = 0.026;
pub const LOCAL_PLAYER_JUMP_POWER: f64 = 0.42;
pub const LOCAL_PLAYER_SPRINT_JUMP_IMPULSE: f64 = 0.2;
pub const LOCAL_PLAYER_GRAVITY: f64 = 0.08;
pub const LOCAL_PLAYER_BLOCK_FRICTION: f64 = 0.6;
pub const LOCAL_PLAYER_FRICTION_MULTIPLIER: f64 = 0.91;
pub const LOCAL_PLAYER_VERTICAL_DRAG: f64 = 0.98;
const LOCAL_PLAYER_GROUND_ACCELERATION_NUMERATOR: f64 = 0.21600002;
const LOCAL_PLAYER_POSITION_SYNC_DELTA_SQR: f64 = 9.0e-4;
const LOCAL_PLAYER_POSITION_REMINDER_INTERVAL: u32 = 20;
const COLLISION_EPSILON: f64 = 1.0e-7;

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
    }

    pub const fn move_vector(self) -> (f32, f32) {
        (self.left_impulse, self.forward_impulse)
    }

    pub fn has_forward_impulse(self) -> bool {
        self.forward_impulse > 1.0e-5
    }

    fn right_axis(self) -> f64 {
        -self.left_impulse as f64
    }
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
    pub dt_seconds: f64,
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
        Aabb::new(
            self.position.x - LOCAL_PLAYER_STANDING_WIDTH / 2.0,
            self.position.y,
            self.position.z - LOCAL_PLAYER_STANDING_WIDTH / 2.0,
            self.position.x + LOCAL_PLAYER_STANDING_WIDTH / 2.0,
            self.position.y + LOCAL_PLAYER_STANDING_HEIGHT,
            self.position.z + LOCAL_PLAYER_STANDING_WIDTH / 2.0,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalPlayerController {
    pose: LocalPlayerPose,
    keys: PlayerInputKeys,
    input: PlayerInput,
    move_sync: LocalPlayerMoveSync,
    delta_movement: Vec3d,
    horizontal_collision: bool,
    vertical_collision: bool,
    on_ground: bool,
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

    pub const fn horizontal_collision(&self) -> bool {
        self.horizontal_collision
    }

    pub const fn vertical_collision(&self) -> bool {
        self.vertical_collision
    }

    pub const fn on_ground(&self) -> bool {
        self.on_ground
    }

    pub fn set_pose(&mut self, pose: LocalPlayerPose) {
        self.pose = pose;
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
    }

    pub fn tick_input(&mut self, moving_slowly: bool) -> PlayerInput {
        self.input.tick(self.keys, moving_slowly);
        self.input
    }

    pub fn tick_no_clip_movement(&mut self, step: NoClipMovementStep) -> Option<Vec3d> {
        let input = self.tick_input(false);
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
        Some(displacement)
    }

    pub fn tick_walking_movement(
        &mut self,
        client: &ClientRuntime,
        step: WalkingMovementStep,
    ) -> Option<WalkingMovementResult> {
        if !step.y_rot_degrees.is_finite() || !step.dt_seconds.is_finite() || step.dt_seconds <= 0.0
        {
            return None;
        }

        let tick_scale = step.dt_seconds * LOCAL_PLAYER_TICKS_PER_SECOND;
        if tick_scale <= 0.0 {
            return None;
        }

        let input = self.tick_input(self.keys.shift);
        let sprinting = self.keys.sprint && input.has_forward_impulse() && !input.shift_key_down;
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
            walking_input_speed(self.on_ground, sprinting),
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

    pub fn move_colliding(
        &mut self,
        client: &ClientRuntime,
        requested: Vec3d,
    ) -> CollisionMovementResult {
        let traveled = collide_movement(client, self.pose.bounding_box(), requested);
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
    if !bounding_box.is_finite() || !movement.is_finite() || movement.length_sqr() == 0.0 {
        return Vec3d::ZERO;
    }

    let solid_blocks = solid_block_aabbs_in(client, bounding_box.expand_towards(movement));
    collide_with_aabbs(bounding_box, movement, &solid_blocks)
}

fn collide_with_aabbs(mut bounding_box: Aabb, movement: Vec3d, solids: &[Aabb]) -> Vec3d {
    let mut x = movement.x;
    let mut y = movement.y;
    let mut z = movement.z;

    if y != 0.0 {
        y = clip_axis(Axis::Y, bounding_box, solids, y);
        if y != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(0.0, y, 0.0));
        }
    }

    let z_first = x.abs() < z.abs();
    if z_first && z != 0.0 {
        z = clip_axis(Axis::Z, bounding_box, solids, z);
        if z != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(0.0, 0.0, z));
        }
    }

    if x != 0.0 {
        x = clip_axis(Axis::X, bounding_box, solids, x);
        if !z_first && x != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(x, 0.0, 0.0));
        }
    }

    if !z_first && z != 0.0 {
        z = clip_axis(Axis::Z, bounding_box, solids, z);
    }

    Vec3d::new(x, y, z)
}

fn solid_block_aabbs_in(client: &ClientRuntime, area: Aabb) -> Vec<Aabb> {
    if !area.is_finite() {
        return Vec::new();
    }

    let min_x = area.min_x.floor() as i32;
    let min_y = area.min_y.floor() as i32;
    let min_z = area.min_z.floor() as i32;
    let max_x = area.max_x.floor() as i32;
    let max_y = area.max_y.floor() as i32;
    let max_z = area.max_z.floor() as i32;
    let mut solids = Vec::new();
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let pos = BlockPos::new(x, y, z);
                let Some(block_state) = client.block_state_at_block_pos(pos) else {
                    continue;
                };
                if let Some(block_box) = block_collision_aabb(block_state, pos) {
                    if block_box.intersects(area) {
                        solids.push(block_box);
                    }
                }
            }
        }
    }
    solids
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

fn clip_axis(axis: Axis, bounding_box: Aabb, solids: &[Aabb], mut delta: f64) -> f64 {
    if delta.abs() < COLLISION_EPSILON {
        return 0.0;
    }

    for solid in solids {
        if !overlaps_other_axes(axis, bounding_box, *solid) {
            continue;
        }
        if delta > 0.0 {
            let distance = min_axis(axis, *solid) - max_axis(axis, bounding_box);
            if distance >= -COLLISION_EPSILON && distance < delta {
                delta = distance.max(0.0);
            }
        } else {
            let distance = max_axis(axis, *solid) - min_axis(axis, bounding_box);
            if distance <= COLLISION_EPSILON && distance > delta {
                delta = distance.min(0.0);
            }
        }
    }
    delta
}

fn overlaps_other_axes(axis: Axis, a: Aabb, b: Aabb) -> bool {
    match axis {
        Axis::X => {
            ranges_overlap(a.min_y, a.max_y, b.min_y, b.max_y)
                && ranges_overlap(a.min_z, a.max_z, b.min_z, b.max_z)
        }
        Axis::Y => {
            ranges_overlap(a.min_x, a.max_x, b.min_x, b.max_x)
                && ranges_overlap(a.min_z, a.max_z, b.min_z, b.max_z)
        }
        Axis::Z => {
            ranges_overlap(a.min_x, a.max_x, b.min_x, b.max_x)
                && ranges_overlap(a.min_y, a.max_y, b.min_y, b.max_y)
        }
    }
}

fn ranges_overlap(a_min: f64, a_max: f64, b_min: f64, b_max: f64) -> bool {
    a_min < b_max && a_max > b_min
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < COLLISION_EPSILON
}

fn min_axis(axis: Axis, aabb: Aabb) -> f64 {
    match axis {
        Axis::X => aabb.min_x,
        Axis::Y => aabb.min_y,
        Axis::Z => aabb.min_z,
    }
}

fn max_axis(axis: Axis, aabb: Aabb) -> f64 {
    match axis {
        Axis::X => aabb.max_x,
        Axis::Y => aabb.max_y,
        Axis::Z => aabb.max_z,
    }
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
        let mut controller = LocalPlayerController::new();
        controller.set_pose(LocalPlayerPose {
            position: Vec3d::new(0.5, 1.0, 0.5),
            ..Default::default()
        });
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
