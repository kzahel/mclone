use super::*;

#[test]
fn day_time_advances_each_tick_by_default() {
    let mut server = LocalRealmSession::new(0);
    let start = server.day_time();
    let report = server.try_simulation_tick_report().expect("tick");
    assert_eq!(server.day_time(), start + 1);
    assert_eq!(last_time_update(&report), start + 1);
}

#[cfg(feature = "physics-rapier")]
#[test]
fn debug_physics_cube_steps_against_live_terrain_section() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);

    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block = if y == 0 { STONE } else { AIR };
                server
                    .scheduler_mut()
                    .set_block_at_world(BlockPos::new(x, y, z), block);
            }
        }
    }
    let terrain = server
        .scheduler()
        .physics_terrain_section_at_block(BlockPos::new(8, 4, 8))
        .expect("physics terrain section");
    assert_eq!(terrain.solid_cell_count(), 16 * 16);

    assert!(server.spawn_debug_physics_cube(Vec3d::new(8.0, 4.0, 8.0), Vec3d::ZERO));
    let spawned = server.physics_diagnostics();
    assert!(spawned.enabled);
    assert_eq!(spawned.body_count, 1);
    assert!((1..=27).contains(&spawned.terrain_collider_count));
    assert_eq!(
        spawned.test_cube_position.map(|position| position.y),
        Some(4.0)
    );

    let mut updated_while_falling = false;
    let mut last_physics = spawned;
    let terrain_collider_count = spawned.terrain_collider_count;
    for _ in 0..120 {
        let report = server
            .try_simulation_tick_report()
            .expect("physics simulation tick");
        assert!(report.physics.enabled);
        assert!(report.physics.test_cube_spawned);
        assert_eq!(report.physics.body_count, 1);
        assert_eq!(
            report.physics.terrain_collider_count,
            terrain_collider_count
        );
        assert!(report.physics.collider_count > terrain_collider_count);
        updated_while_falling |= report.physics.body_pose_update_count > 0;
        last_physics = report.physics;
    }

    let final_y = last_physics
        .test_cube_position
        .expect("debug cube position")
        .y;
    assert!(updated_while_falling);
    assert!(
        final_y > 1.45 && final_y < 1.8,
        "debug cube should settle on the live terrain floor, got y={final_y}"
    );
}

#[cfg(feature = "physics-rapier")]
#[test]
fn debug_physics_cube_applies_minecraft_vertical_drag() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);

    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block = if y == 0 { STONE } else { AIR };
                server
                    .scheduler_mut()
                    .set_block_at_world(BlockPos::new(x, y, z), block);
            }
        }
    }

    assert!(server.spawn_debug_physics_cube(Vec3d::new(8.0, 6.0, 8.0), Vec3d::ZERO));
    let initial_velocity = server
        .physics
        .debug_cube_velocity()
        .expect("initial debug cube velocity");
    assert_eq!(initial_velocity.linear, Vec3d::ZERO);
    assert!(initial_velocity.angular.length_sqr() > 0.0);

    server.physics.step();

    let velocity = server
        .physics
        .debug_cube_velocity()
        .expect("debug cube velocity after physics step");
    let expected_y = (-32.0 * 0.05) * 0.98;
    assert!(
        (velocity.linear.y - expected_y).abs() < 1.0e-5,
        "debug cube vertical velocity should apply Minecraft gravity and vertical drag, got {} expected {}",
        velocity.linear.y,
        expected_y
    );
}

#[cfg(feature = "physics-rapier")]
#[test]
fn shoot_debug_physics_cube_command_publishes_debug_entity() {
    let mut server = LocalRealmSession::new(0);
    prepare_debug_physics_floor(&mut server);

    let updates = server
        .try_handle_command(ClientCommand::ShootDebugPhysicsCube)
        .expect("shoot debug physics cube");
    let snapshot = first_entity_snapshot_of_kind(&updates, EntityKind::DebugCube)
        .expect("debug cube entity snapshot");

    assert_eq!(snapshot.width, 1.0);
    assert_eq!(snapshot.height, 1.0);
    assert_eq!(snapshot.position, Vec3d::new(8.0, 4.0, 7.25));
    assert_eq!(snapshot.rotation, Some(EntityRotation::IDENTITY));

    let report = server.try_simulation_tick_report().expect("physics tick");
    let update = first_entity_update(&report.updates, snapshot.id)
        .expect("debug cube should publish pose updates");

    assert!(report.physics.test_cube_spawned);
    assert_eq!(update.id, snapshot.id);
    assert!(update.position.y < snapshot.position.y);
    assert!(
        update.y_rot_degrees.abs() > 0.1 || update.x_rot_degrees.abs() > 0.1,
        "debug cube should publish physics rotation after stepping, got y={} x={}",
        update.y_rot_degrees,
        update.x_rot_degrees
    );
    let update_rotation = update
        .rotation
        .expect("debug cube should publish full physics rotation");
    let rotation_len_sqr = update_rotation.x * update_rotation.x
        + update_rotation.y * update_rotation.y
        + update_rotation.z * update_rotation.z
        + update_rotation.w * update_rotation.w;
    assert!((rotation_len_sqr - 1.0).abs() < 1.0e-5);
    assert!(
        update_rotation.x.abs() > 0.01
            || update_rotation.y.abs() > 0.01
            || update_rotation.z.abs() > 0.01,
        "debug cube should publish non-identity quaternion after stepping, got {:?}",
        update_rotation
    );

    let second_updates = server
        .try_handle_command(ClientCommand::ShootDebugPhysicsCube)
        .expect("shoot second debug physics cube");
    let second_snapshot = first_entity_snapshot_of_kind(&second_updates, EntityKind::DebugCube)
        .expect("second debug cube entity snapshot");

    assert_ne!(second_snapshot.id, snapshot.id);
    assert!(has_entity_remove(&second_updates, snapshot.id));
}

#[cfg(feature = "physics-rapier")]
#[test]
fn physics_step_report_advances_debug_cube_without_gameplay_tick() {
    let mut server = LocalRealmSession::new(0);
    prepare_debug_physics_floor(&mut server);

    let updates = server
        .try_handle_command(ClientCommand::ShootDebugPhysicsCube)
        .expect("shoot debug physics cube");
    let snapshot = first_entity_snapshot_of_kind(&updates, EntityKind::DebugCube)
        .expect("debug cube entity snapshot");
    let spawned_body_position = server
        .debug_physics_cube_pose()
        .expect("debug cube body pose")
        .position;

    let gameplay = server
        .try_simulation_tick_report_with_physics_steps(0)
        .expect("gameplay tick without physics");
    assert_eq!(gameplay.simulation_tick, 1);
    assert_eq!(server.simulation_tick(), 1);
    assert_eq!(
        gameplay.physics.test_cube_position,
        Some(spawned_body_position)
    );
    assert!(first_entity_update(&gameplay.updates, snapshot.id).is_none());

    let physics = server
        .try_physics_step_report(1)
        .expect("physics-only step");
    assert_eq!(physics.simulation_tick, 1);
    assert_eq!(server.simulation_tick(), 1);
    assert_eq!(physics.physics_steps, 1);
    assert!(physics.physics.test_cube_spawned);
    let update = first_entity_update(&physics.updates, snapshot.id)
        .expect("physics-only step should publish debug cube update");
    assert!(
        update.position.y < snapshot.position.y || update.position.z > snapshot.position.z,
        "debug cube should move on a physics-only step, got {:?} from {:?}",
        update.position,
        snapshot.position
    );
}

#[cfg(feature = "physics-rapier")]
#[test]
fn debug_physics_cube_collides_with_player_collider() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);

    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block = if y == 0 { STONE } else { AIR };
                server
                    .scheduler_mut()
                    .set_block_at_world(BlockPos::new(x, y, z), block);
            }
        }
    }

    let player_position = Vec3d::new(8.0, 1.0, 8.0);
    {
        let scheduler = &server.scheduler;
        let physics = &mut server.physics;
        assert!(physics.spawn_debug_cube(
            scheduler,
            Vec3d::new(5.0, 2.0, 8.0),
            Vec3d::new(12.0, 0.0, 0.0),
            Some(player_position),
        ));
    }

    for _ in 0..20 {
        server.physics.sync_player_collider(player_position);
        server.physics.step();
    }

    let final_x = server
        .debug_physics_cube_pose()
        .expect("debug cube pose")
        .position
        .x;
    let player_left_face_x = player_position.x - 0.3;
    assert!(
        final_x < player_left_face_x,
        "debug cube should collide before its center passes into the player AABB, got x={final_x}"
    );
}
