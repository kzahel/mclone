use super::*;

#[test]
fn engine_camera_controller_moves_no_clip_and_crosses_chunk_boundary() {
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(15.5, 96.0, 8.0),
        std::f64::consts::FRAC_PI_2,
        0.0,
        32.0,
    );

    let snapshot = camera.apply_input(EngineCameraInput {
        dt_seconds: 0.1,
        forward: true,
        ..EngineCameraInput::default()
    });

    assert!(snapshot.eye.x > 16.0);
    assert_eq!(snapshot.chunk_pos, ChunkPos::new(1, 0));
}

#[test]
fn engine_camera_controller_accepts_analog_movement_impulse() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    let before = camera.snapshot();

    let after = camera.apply_input(EngineCameraInput {
        dt_seconds: 0.1,
        movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 0.5)),
        ..EngineCameraInput::default()
    });

    assert!(after.eye.z > before.eye.z);
    assert!((after.eye.z - before.eye.z) < ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND * 0.1);
}

#[test]
fn engine_camera_controller_applies_mouse_look_and_sprint_boost() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
    let before = camera.snapshot();

    let after = camera.apply_input(EngineCameraInput {
        dt_seconds: 0.1,
        mouse_delta_x: 20.0,
        mouse_delta_y: -10.0,
        forward: true,
        sprint: true,
        ..EngineCameraInput::default()
    });

    assert!((after.yaw_radians - before.yaw_radians).abs() > 1.0e-6);
    assert!((after.pitch_radians - before.pitch_radians).abs() > 1.0e-6);
    assert!(
        after.eye.subtract(before.eye).length_sqr()
            > (ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND * 0.1).powi(2)
    );
}

#[test]
fn engine_camera_controller_toggles_movement_mode() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
    let standing_eye = camera.snapshot().eye;

    assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Walking);
    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::STANDING
    );
    assert_eq!(camera.toggle_movement_mode(), EngineCameraMovementMode::Fly);
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::NoClip);
    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::STANDING
    );
    assert_eq!(
        camera.toggle_movement_mode(),
        EngineCameraMovementMode::HandPush
    );
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::HAND_PUSH
    );
    assert_eq!(
        camera.player().bounding_box(),
        camera
            .player()
            .pose()
            .bounding_box_with_dimensions(LocalPlayerDimensions::HAND_PUSH)
    );
    assert!(camera.snapshot().eye.y < standing_eye.y);
    assert_eq!(
        camera.toggle_movement_mode(),
        EngineCameraMovementMode::Thruster
    );
    // Thruster (tactical 157) defaults to Normal collision on entry — it must
    // NOT force NoClip the way Fly does — and uses STANDING dimensions.
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::STANDING
    );
    assert_eq!(
        camera.toggle_movement_mode(),
        EngineCameraMovementMode::Walking
    );
    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::STANDING
    );
    assert_eq!(camera.snapshot().eye, standing_eye);
    assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
    assert_eq!(EngineCameraMovementMode::Fly.label(), "FLY");
    assert_eq!(EngineCameraMovementMode::HandPush.label(), "HAND");
    assert_eq!(EngineCameraMovementMode::Thruster.label(), "THRUST");
    assert_eq!(EngineCameraCollisionMode::Normal.label(), "NORMAL");
    assert_eq!(EngineCameraCollisionMode::NoClip.label(), "NOCLIP");
}

#[test]
fn thruster_defaults_normal_collision_but_leaves_noclip_selectable() {
    // Tactical 157: entering Thruster must default to Normal collision (never
    // force NoClip like Fly), yet — unlike HandPush, which is always
    // collision-backed — NoClip must remain selectable afterward.
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));

    camera.set_movement_mode(EngineCameraMovementMode::Thruster);
    assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Thruster);
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);

    camera.set_collision_mode(EngineCameraCollisionMode::NoClip);
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::NoClip);
    assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Thruster);

    // Contrast: HandPush forces Normal collision back.
    camera.set_movement_mode(EngineCameraMovementMode::HandPush);
    camera.set_collision_mode(EngineCameraCollisionMode::NoClip);
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
}

#[test]
fn thruster_full_throttle_climbs_and_retains_per_hand_input() {
    // Slice 2: full throttle on both palms-down hands out-thrusts gravity, so the
    // body climbs; the per-hand intent is still retained for the client to read.
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 80.0, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Thruster);
    let eye_before = camera.snapshot().eye;

    let thruster = EngineThrusterInput::new(
        EngineThrusterHand::new(Vec3d::new(0.0, -1.0, 0.0), 1.0),
        EngineThrusterHand::new(Vec3d::new(0.0, -1.0, 0.0), 1.0),
    );
    for _ in 0..30 {
        camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 1.0 / 60.0,
                thruster: Some(thruster),
                ..EngineCameraInput::default()
            },
        );
    }

    assert_eq!(camera.last_thruster_input(), Some(thruster));
    assert!(
        camera.snapshot().eye.y > eye_before.y + 0.1,
        "full thrust should climb: {} -> {}",
        eye_before.y,
        camera.snapshot().eye.y
    );
    // The mode stays gravity-bound and never forces NoClip.
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
}

#[test]
fn thruster_without_thrust_falls_under_gravity() {
    // Thruster is gravity-bound: with no thrust intent the body still falls.
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 80.0, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Thruster);
    let eye_before = camera.snapshot().eye;

    for _ in 0..10 {
        camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 1.0 / 60.0,
                ..EngineCameraInput::default()
            },
        );
    }

    assert!(
        camera.snapshot().eye.y < eye_before.y,
        "no thrust should fall: {} -> {}",
        eye_before.y,
        camera.snapshot().eye.y
    );
}

#[test]
fn thruster_desktop_emulation_climbs_on_jump_with_palms_down() {
    // Slice 3: desktop-XR emulation lets the integrator be tuned without a
    // headset. Holding jump should synthesize both palms facing down (thrust up)
    // and climb against gravity.
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 80.0, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Thruster);
    let eye_before = camera.snapshot().eye;

    for _ in 0..30 {
        camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 1.0 / 60.0,
                jump: true,
                thruster_emulation: true,
                ..EngineCameraInput::default()
            },
        );
    }

    let thruster = camera
        .last_thruster_input()
        .expect("emulation should synthesize thrust from jump");
    assert!(
        thruster.left.palm_normal.y < 0.0 && thruster.right.palm_normal.y < 0.0,
        "jump should fire palms down (thrust up): {:?}",
        thruster.left.palm_normal
    );
    assert!(
        camera.snapshot().eye.y > eye_before.y + 0.1,
        "emulated jump-thrust should climb: {} -> {}",
        eye_before.y,
        camera.snapshot().eye.y
    );
}

#[test]
fn thruster_desktop_emulation_idle_falls_under_gravity() {
    // Emulation enabled but no keys held: no synthetic thrust, so gravity wins.
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 80.0, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Thruster);
    let eye_before = camera.snapshot().eye;

    for _ in 0..10 {
        camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 1.0 / 60.0,
                thruster_emulation: true,
                ..EngineCameraInput::default()
            },
        );
    }

    assert_eq!(camera.last_thruster_input(), None);
    assert!(camera.snapshot().eye.y < eye_before.y);
}

#[test]
fn thruster_tuning_setter_is_sanitized_and_readable() {
    // Slice 3: the camera exposes the dev-adjustable feel knobs; the setter
    // sanitizes so a bad override cannot break flight.
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 80.0, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_thruster_tuning(ThrusterTuning {
        gravity_scale: 0.35,
        drag: 5.0,
        ..ThrusterTuning::default()
    });
    let tuning = camera.thruster_tuning();
    assert!((tuning.gravity_scale - 0.35).abs() < 1.0e-9);
    assert_eq!(
        tuning.drag, 1.0,
        "drag > 1 should clamp to the no-drag ceiling"
    );
}

#[test]
fn engine_debug_world_lines_include_player_box_and_hand_colliders() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    assert!(engine_debug_world_lines(&camera, EngineDebugVisualOptions::default()).is_empty());
    assert_eq!(
        engine_debug_world_lines(&camera, EngineDebugVisualOptions::new(true)).len(),
        12
    );

    camera.set_movement_mode(EngineCameraMovementMode::HandPush);
    camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 1.0 / 60.0,
            hand_push: Some(EngineHandPushInput::new(
                Vec3d::new(0.5, 2.62, 0.5),
                Vec3d::new(0.25, 1.6, 0.45),
                Vec3d::new(0.75, 1.6, 0.45),
            )),
            ..EngineCameraInput::default()
        },
    );

    let hand_lines = engine_debug_world_lines(&camera, EngineDebugVisualOptions::default());
    assert_eq!(hand_lines.len(), ENGINE_DEBUG_HAND_SPHERE_SEGMENTS * 3 * 2);
    let all_lines = engine_debug_world_lines(&camera, EngineDebugVisualOptions::new(true));
    assert_eq!(all_lines.len(), hand_lines.len() + 12);
}

#[test]
fn room_scale_reconciliation_ignores_micro_headset_jitter() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.52, 2.62, 0.5));

    assert_eq!(result.requested_body_movement, Vec3d::ZERO);
    assert_eq!(result.consumed_body_movement, Vec3d::ZERO);
    assert_close(result.residual_head_offset.x, 0.02);
    assert_close(camera.player().pose().eye_position().x, 0.5);
}

#[test]
fn room_scale_reconciliation_stops_after_clear_space_catch_up() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.54, 2.62, 0.5));

    assert_close(result.requested_body_movement.x, 0.04);
    assert_eq!(
        result.consumed_body_movement,
        result.requested_body_movement
    );
    assert_close(result.residual_head_offset.x, 0.0);
    assert_close(camera.player().pose().eye_position().x, 0.54);

    let jitter = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.555, 2.62, 0.5));

    assert_eq!(jitter.requested_body_movement, Vec3d::ZERO);
    assert_eq!(jitter.consumed_body_movement, Vec3d::ZERO);
    assert_close(jitter.residual_head_offset.x, 0.015);
    assert_close(camera.player().pose().eye_position().x, 0.54);
}

#[test]
fn room_scale_reconciliation_exits_active_follow_below_deadband() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let blocked = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));

    assert!(blocked.collision.horizontal_collision);
    assert_close(blocked.residual_head_offset.x, 0.8);
    assert_close(camera.player().pose().eye_position().x, 0.7);

    let tiny = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.705, 2.62, 0.5));

    assert_eq!(tiny.requested_body_movement, Vec3d::ZERO);
    assert_eq!(tiny.consumed_body_movement, Vec3d::ZERO);
    assert_close(camera.player().pose().eye_position().x, 0.7);

    let below_enter = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.72, 2.62, 0.5));

    assert_eq!(below_enter.requested_body_movement, Vec3d::ZERO);
    assert_eq!(below_enter.consumed_body_movement, Vec3d::ZERO);
    assert_close(camera.player().pose().eye_position().x, 0.7);
}

#[test]
fn room_scale_reconciliation_holds_blocked_wall_jitter() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let blocked = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));

    assert!(blocked.collision.horizontal_collision);
    assert_close(blocked.consumed_body_movement.x, 0.2);
    assert_close(blocked.residual_head_offset.x, 0.8);
    assert_close(camera.player().pose().eye_position().x, 0.7);
    assert_close(camera.player().pose().eye_position().z, 0.5);

    let jitter = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.52));

    assert_eq!(jitter.requested_body_movement, Vec3d::ZERO);
    assert_eq!(jitter.consumed_body_movement, Vec3d::ZERO);
    assert_close(jitter.residual_head_offset.x, 0.8);
    assert_close(jitter.residual_head_offset.z, 0.02);
    assert_close(camera.player().pose().eye_position().x, 0.7);
    assert_close(camera.player().pose().eye_position().z, 0.5);
}

#[test]
fn room_scale_reconciliation_retries_after_meaningful_blocked_residual_change() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let blocked = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));

    assert!(blocked.collision.horizontal_collision);
    assert_close(camera.player().pose().eye_position().x, 0.7);
    assert_close(camera.player().pose().eye_position().z, 0.5);

    let retry = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.57));

    assert!(retry.collision.horizontal_collision);
    assert_close(retry.requested_body_movement.x, 0.8);
    assert_close(retry.requested_body_movement.z, 0.07);
    assert_close(retry.consumed_body_movement.x, 0.0);
    assert_close(retry.consumed_body_movement.z, 0.07);
    assert_close(retry.residual_head_offset.x, 0.8);
    assert_close(retry.residual_head_offset.z, 0.0);
    assert_close(camera.player().pose().eye_position().x, 0.7);
    assert_close(camera.player().pose().eye_position().z, 0.57);
}

#[test]
fn room_scale_reconciliation_moves_body_toward_headset_in_clear_space() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.85, 2.62, 0.3));

    assert_close(result.requested_body_movement.x, 0.35);
    assert_close(result.requested_body_movement.y, 0.0);
    assert_close(result.requested_body_movement.z, -0.2);
    assert_eq!(
        result.consumed_body_movement,
        result.requested_body_movement
    );
    assert_close(result.residual_head_offset.x, 0.0);
    assert_close(result.residual_head_offset.y, 0.0);
    assert_close(result.residual_head_offset.z, 0.0);
    assert_close(camera.player().pose().eye_position().x, 0.85);
    assert_close(camera.player().pose().eye_position().z, 0.3);
}

#[test]
fn room_scale_reconciliation_leaves_collision_residual_when_blocked() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));

    assert_close(result.requested_body_movement.x, 1.0);
    assert_close(result.consumed_body_movement.x, 0.2);
    assert_close(result.residual_head_offset.x, 0.8);
    assert_close(result.residual_head_offset.y, 0.0);
    assert!(result.collision.horizontal_collision);
    assert_close(camera.player().pose().eye_position().x, 0.7);
}

#[test]
fn room_scale_reconciliation_does_not_auto_step_from_vertical_hmd_offset() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.75, 3.37, 0.5));

    assert_close(result.requested_body_movement.x, 0.25);
    assert_close(result.requested_body_movement.y, 0.0);
    assert_close(result.consumed_body_movement.x, 0.25);
    assert_close(result.consumed_body_movement.y, 0.0);
    assert_close(result.body_eye_after.y, 2.62);
    assert_close(result.residual_head_offset.y, 0.75);
    assert_close(camera.player().pose().eye_position().y, 2.62);
}

#[test]
fn room_scale_reconciliation_does_not_move_hand_push_body() {
    let client = ClientRuntime::local_integrated();
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.set_movement_mode(EngineCameraMovementMode::HandPush);

    let result = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.85, 2.62, 0.3));

    assert_eq!(result.requested_body_movement, Vec3d::ZERO);
    assert_eq!(result.consumed_body_movement, Vec3d::ZERO);
    assert_close(result.residual_head_offset.x, 0.35);
    assert_close(result.residual_head_offset.z, -0.2);
    assert_close(camera.player().pose().eye_position().x, 0.5);
    assert_close(camera.player().pose().eye_position().z, 0.5);
    assert_eq!(camera.last_room_scale_reconciliation(), None);
}

#[test]
fn hand_push_reconciliation_clears_stale_blocked_residual() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    let blocked = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));
    assert!(blocked.collision.horizontal_collision);
    assert!(camera.last_room_scale_reconciliation().is_some());

    camera.set_movement_mode(EngineCameraMovementMode::HandPush);
    let hand_push = camera.reconcile_room_scale_headset(&client, Vec3d::new(1.5, 2.62, 0.5));

    assert_eq!(hand_push.requested_body_movement, Vec3d::ZERO);
    assert_eq!(hand_push.consumed_body_movement, Vec3d::ZERO);
    assert_eq!(camera.last_room_scale_reconciliation(), None);
}

#[test]
fn room_scale_reconciliation_preserves_grounded_jump_input() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(0, 0, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        0.0,
        0.0,
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
    );
    camera.probe_ground(&client, 0.02);
    assert!(camera.on_ground());

    let reconciliation = camera.reconcile_room_scale_headset(&client, Vec3d::new(0.55, 2.62, 0.5));
    assert!(reconciliation.collision.on_ground);
    assert!(camera.on_ground());
    let before_jump = camera.snapshot();

    let after_jump = camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND,
            jump: true,
            ..EngineCameraInput::default()
        },
    );

    assert!(after_jump.eye.y > before_jump.eye.y);
}

#[test]
fn hand_push_emulation_direction_uses_camera_yaw() {
    let forward = hand_push_emulation_direction(
        EngineCameraInput {
            forward: true,
            hand_push_emulation: true,
            ..EngineCameraInput::default()
        },
        0.0,
    );
    assert!(forward.z > 0.99);
    assert!(forward.x.abs() < 1.0e-6);

    let right = hand_push_emulation_direction(
        EngineCameraInput {
            right: true,
            hand_push_emulation: true,
            ..EngineCameraInput::default()
        },
        0.0,
    );
    assert!(right.x > 0.99);
    assert!(right.z.abs() < 1.0e-6);
}

#[test]
fn engine_camera_snapshot_from_eye_pose_floors_negative_chunks() {
    let snapshot =
        EngineCameraSnapshot::from_eye_pose(Vec3d::new(-16.01, 91.0, -0.01), 0.2, -0.1, 24.0);

    assert_eq!(snapshot.eye, Vec3d::new(-16.01, 91.0, -0.01));
    assert_eq!(snapshot.yaw_radians, 0.2);
    assert_eq!(snapshot.pitch_radians, -0.1);
    assert_eq!(snapshot.speed_blocks_per_second, 24.0);
    assert_eq!(snapshot.chunk_pos, ChunkPos::new(-2, -1));
}

#[test]
fn engine_camera_frame_state_reports_controller_and_interaction_state() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
    let mut interaction = ClientInteractionController::new();
    camera.toggle_movement_mode();
    assert!(interaction.select_hotbar_slot(4));

    let state = camera.frame_state(&interaction);

    assert_eq!(state.camera, camera.snapshot());
    assert_eq!(state.movement_mode, EngineCameraMovementMode::Fly);
    assert_eq!(state.collision_mode, EngineCameraCollisionMode::NoClip);
    assert_eq!(state.view_mode, EngineCameraViewMode::FirstPerson);
    assert_eq!(state.movement_mode_label(), "FLY");
    assert_eq!(state.collision_mode_label(), "NOCLIP");
    assert_eq!(state.view_mode_label(), "FIRST_PERSON");
    assert_eq!(state.on_ground, camera.on_ground());
    assert_eq!(state.horizontal_collision, camera.horizontal_collision());
    assert_eq!(state.vertical_collision, camera.vertical_collision());
    assert_eq!(state.selected_hotbar_slot, 4);
}

#[test]
fn engine_camera_controller_sets_explicit_eye_pose() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
    let eye = Vec3d::new(16.25, 72.0, -0.5);

    camera.set_eye_pose(eye, 0.25, -0.125);
    let snapshot = camera.snapshot();

    assert_eq!(snapshot.eye, eye);
    assert_eq!(snapshot.chunk_pos, ChunkPos::new(1, -1));
    assert!((snapshot.yaw_radians - 0.25).abs() < 1.0e-12);
    assert!((snapshot.pitch_radians + 0.125).abs() < 1.0e-12);
}

#[test]
fn engine_camera_controller_set_eye_pose_preserves_active_hand_push_dimensions() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
    camera.set_movement_mode(EngineCameraMovementMode::HandPush);
    let eye = Vec3d::new(16.25, 72.0, -0.5);

    camera.set_eye_pose(eye, 0.25, -0.125);

    assert_eq!(
        camera.player().dimensions(),
        LocalPlayerDimensions::HAND_PUSH
    );
    assert_eq!(camera.snapshot().eye, eye);
    assert_eq!(
        camera.player().pose().position,
        Vec3d::new(
            16.25,
            72.0 - LocalPlayerDimensions::HAND_PUSH.eye_height,
            -0.5
        )
    );
}

#[test]
fn engine_camera_controller_sets_explicit_feet_pose() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 70.0, 8.0), 0.0, 0.0, 24.0);
    camera
        .player
        .set_delta_movement(Vec3d::new(0.25, 0.5, -0.25));

    camera.set_player_feet_pose(Vec3d::new(2.5, 64.0, -3.5), 0.5, -0.25);
    let snapshot = camera.snapshot();

    assert_eq!(camera.player().pose().position, Vec3d::new(2.5, 64.0, -3.5));
    assert_eq!(camera.player().delta_movement(), Vec3d::ZERO);
    assert_eq!(
        snapshot.eye,
        Vec3d::new(2.5, 64.0 + LOCAL_PLAYER_STANDING_EYE_HEIGHT, -3.5)
    );
    assert!((snapshot.yaw_radians - 0.5).abs() < 1.0e-12);
    assert!((snapshot.pitch_radians + 0.25).abs() < 1.0e-12);
}

#[test]
fn engine_camera_controller_applies_explicit_yaw_delta() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.25, -0.1, 32.0);
    let before = camera.snapshot();

    camera.turn_yaw_delta(std::f64::consts::FRAC_PI_4);
    let after = camera.snapshot();

    assert_eq!(after.eye, before.eye);
    assert!(
        (after.yaw_radians - (before.yaw_radians + std::f64::consts::FRAC_PI_4)).abs() < 1.0e-12
    );
    assert!((after.pitch_radians - before.pitch_radians).abs() < 1.0e-12);
}

#[test]
fn engine_camera_controller_ticks_current_no_clip_keys() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(15.5, 96.0, 8.0), 0.0, 0.0, 32.0);
    let client = ClientRuntime::local_integrated();
    camera.set_movement_mode(EngineCameraMovementMode::Fly);
    camera.set_collision_mode(EngineCameraCollisionMode::NoClip);
    camera.set_key(PlayerInputKey::Forward, true);

    let moved = camera.tick_movement(&client, 0.1);

    assert!(moved);
    assert!(camera.snapshot().eye.z > 8.0);
}

#[test]
fn engine_camera_controller_fly_normal_collides_with_blocks() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        std::f64::consts::FRAC_PI_2,
        0.0,
        20.0,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Fly);
    camera.set_collision_mode(EngineCameraCollisionMode::Normal);

    let before = camera.snapshot();
    let after = camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 0.1,
            forward: true,
            ..EngineCameraInput::default()
        },
    );

    assert!(after.eye.x > before.eye.x);
    assert!(after.eye.x < 1.0);
    assert!(camera.horizontal_collision());
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
}

#[test]
fn engine_camera_controller_fly_no_clip_passes_through_blocks() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(1, 1, 0),
        BlockStateId(7),
    )));
    let mut camera = EngineCameraController::from_eye_pose(
        Vec3d::new(0.5, 2.62, 0.5),
        std::f64::consts::FRAC_PI_2,
        0.0,
        20.0,
    );
    camera.set_movement_mode(EngineCameraMovementMode::Fly);
    camera.set_collision_mode(EngineCameraCollisionMode::NoClip);

    let after = camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 0.1,
            forward: true,
            ..EngineCameraInput::default()
        },
    );

    assert!(after.eye.x > 2.0);
    assert!(!camera.horizontal_collision());
    assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::NoClip);
}

#[test]
fn engine_camera_controller_reports_pose_sync_command() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    camera
        .next_pose_sync_command()
        .expect("initial pose should establish the movement baseline");
    camera.turn_mouse_delta(5.0, 0.0);

    let report = camera
        .next_pose_sync_command()
        .expect("rotation should produce pose sync");

    assert_eq!(report.kind, EnginePoseSyncCommandKind::Movement);
    let ClientCommand::MovePlayer(mclone_protocol::MovePlayerCommand::Rot {
        y_rot_degrees,
        x_rot_degrees,
        ..
    }) = report.command
    else {
        panic!("stationary mouse look must publish a rotation-only command");
    };
    assert_eq!(
        y_rot_degrees,
        -report.camera.yaw_radians.to_degrees() as f32
    );
    assert_eq!(
        x_rot_degrees,
        -report.camera.pitch_radians.to_degrees() as f32
    );
    assert_eq!(report.camera, camera.snapshot());
    assert!(camera.next_pose_sync_command().is_none());
}

#[test]
fn engine_camera_controller_reports_correction_acceptance_and_resync() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    let update = PlayerPositionUpdate {
        position: Vec3d::new(4.0, 70.0, -3.0),
        y_rot_degrees: 90.0,
        x_rot_degrees: 30.0,
        relative: mclone_protocol::PlayerPositionRelativeFlags::ABSOLUTE,
        teleport_id: 42,
        dismount_vehicle: false,
    };

    let accepted = camera.accept_position_update(update);

    assert_eq!(accepted.update, update);
    assert_eq!(accepted.feet_position, update.position);
    assert_eq!(accepted.camera, camera.snapshot());
    assert!(matches!(
        accepted.accept_command,
        ClientCommand::AcceptTeleport(mclone_protocol::AcceptTeleportCommand { id: 42 })
    ));

    let resync = camera.corrected_pose_sync_command();

    assert_eq!(resync.kind, EnginePoseSyncCommandKind::CorrectionResync);
    assert!(matches!(resync.command, ClientCommand::MovePlayer(_)));
    assert_eq!(resync.camera, camera.snapshot());
    assert!(camera.next_pose_sync_command().is_none());
}

#[test]
fn engine_camera_controller_can_tick_walking_path() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    let client = ClientRuntime::local_integrated();
    let before = camera.snapshot();

    let after = camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 0.05,
            forward: true,
            ..EngineCameraInput::default()
        },
    );

    assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Walking);
    assert!(after.eye.z > before.eye.z);
    assert!(camera.player().delta_movement().y < 0.0);
}

#[test]
fn engine_camera_controller_applies_walking_speed_multiplier() {
    let client = ClientRuntime::local_integrated();
    let input = EngineCameraInput {
        dt_seconds: 0.05,
        forward: true,
        ..EngineCameraInput::default()
    };
    let mut normal =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    let mut faster =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    faster.set_movement_speed_multiplier(2.0);

    let normal_before = normal.snapshot();
    let fast_before = faster.snapshot();
    let normal_after = normal.apply_movement_input(&client, input);
    let fast_after = faster.apply_movement_input(&client, input);

    assert_eq!(normal.movement_speed_multiplier(), 1.0);
    assert_eq!(faster.movement_speed_multiplier(), 2.0);
    assert!(fast_after.eye.z - fast_before.eye.z > normal_after.eye.z - normal_before.eye.z);

    faster.set_movement_speed_multiplier(20.0);
    assert_eq!(
        faster.movement_speed_multiplier(),
        ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER
    );
    faster.set_movement_speed_multiplier(f64::NAN);
    assert_eq!(
        faster.movement_speed_multiplier(),
        ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER
    );
}

#[test]
fn engine_camera_movement_yaw_override_walks_without_turning_view() {
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
    let client = ClientRuntime::local_integrated();
    let before = camera.snapshot();

    let after = camera.apply_movement_input(
        &client,
        EngineCameraInput {
            dt_seconds: 0.05,
            movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
            movement_yaw_radians: Some(std::f64::consts::FRAC_PI_2),
            ..EngineCameraInput::default()
        },
    );

    assert!(after.eye.x > before.eye.x);
    assert!((after.eye.z - before.eye.z).abs() < 1.0e-6);
    assert!((after.yaw_radians - before.yaw_radians).abs() < 1.0e-12);
    assert!((after.pitch_radians - before.pitch_radians).abs() < 1.0e-12);
}

#[test]
fn engine_camera_controller_queues_landing_event_once_on_airborne_to_ground() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(0, 0, 0),
        BlockStateId(7),
    )));
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(0.5, 2.63, 0.5), 0.0, 0.0, 32.0);
    let tick_dt = 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND;

    assert!(camera.tick_movement(&client, tick_dt));
    assert!(camera.take_landing_events().is_empty());

    assert!(camera.tick_movement(&client, tick_dt));
    let events = camera.take_landing_events();

    assert_eq!(events.len(), 1);
    assert!(events[0].impact_speed >= LANDING_MIN_IMPACT_SPEED);
    assert!((events[0].position.y - 1.0).abs() < 1.0e-9);
    assert!(camera.take_landing_events().is_empty());

    assert!(camera.tick_movement(&client, tick_dt));
    assert!(camera.take_landing_events().is_empty());
}

#[test]
fn engine_camera_controller_probe_ground_does_not_queue_landing_event() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        BlockPos::new(0, 0, 0),
        BlockStateId(7),
    )));
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(0.5, 2.62, 0.5), 0.0, 0.0, 32.0);

    camera.probe_ground(&client, 0.02);

    assert!(camera.on_ground());
    assert!(camera.take_landing_events().is_empty());
}

#[test]
fn engine_camera_walking_forward_tracks_crosshair_view_direction() {
    // Guards the reported "in portrait I don't walk toward the crosshair":
    // walking forward must move along the same horizontal direction the
    // render camera / crosshair points, for every yaw. The math is
    // orientation-independent, so a regression here would be a real vector
    // bug rather than a touch-feel issue.
    let client = ClientRuntime::local_integrated();
    for yaw in [0.0_f64, 0.6, 1.5, 2.4, 3.1, -0.9, -2.2] {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), yaw, -0.35, 32.0);
        let before = camera.snapshot();

        let render = legacy_chunk_camera_from_snapshot(before, 1);
        let view_dx = f64::from(render.target[0]) - f64::from(render.eye[0]);
        let view_dz = f64::from(render.target[2]) - f64::from(render.eye[2]);
        let view_len = view_dx.hypot(view_dz);
        assert!(view_len > 1.0e-9, "degenerate view direction at yaw {yaw}");

        let after = camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 0.05,
                movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
                ..EngineCameraInput::default()
            },
        );
        let move_dx = after.eye.x - before.eye.x;
        let move_dz = after.eye.z - before.eye.z;
        let move_len = move_dx.hypot(move_dz);
        assert!(move_len > 1.0e-6, "no horizontal movement at yaw {yaw}");

        let dot = (move_dx * view_dx + move_dz * view_dz) / (move_len * view_len);
        assert!(
            dot > 0.9999,
            "forward walk diverged from crosshair at yaw {yaw}: dot={dot}",
        );
    }
}

#[test]
fn engine_camera_view_mode_toggles_between_first_and_third_person() {
    let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));

    assert_eq!(camera.view_mode(), EngineCameraViewMode::FirstPerson);
    assert_eq!(
        camera.toggle_view_mode(),
        EngineCameraViewMode::ThirdPersonBack
    );
    assert_eq!(camera.toggle_view_mode(), EngineCameraViewMode::FirstPerson);
}
