use super::*;

#[test]
fn actor_instances_follow_client_actor_presentations() {
    let integrated = mclone_client::ClientRuntime::local_integrated();
    assert!(integrated.actor_presentations().is_empty());
    assert!(
        actor_instances_from_presentations(&integrated.actor_presentations(), &integrated)
            .is_empty()
    );

    let mut remote = mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
    remote.apply_update(mclone_protocol::ServerUpdate::RemotePlayerAdd(
        mclone_protocol::RemotePlayerUpdate {
            id: mclone_protocol::RemotePlayerId(9),
            appearance: mclone_protocol::PlayerAppearance::default(),
            position: Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        },
    ));

    let mut interpolation =
        ActorInterpolationState::from_authoritative(remote.actor_presentations());
    interpolation.step(1.0, ActorInterpolationConfig::default());
    let actors = actor_instances_from_presentations(&interpolation.presentations(), &remote);

    assert_eq!(actors.len(), 1);
    assert_eq!(actors[0].feet_position, glam::Vec3::new(1.0, 64.0, 2.0));
    assert!((actors[0].yaw_radians - 90.0_f32.to_radians()).abs() < 1.0e-6);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::default_player_figure_id())
    );
    assert_eq!(
        actors[0].packed_light,
        mclone_render::light_texture::FULL_BRIGHT
    );

    let mut entity_client =
        mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
    entity_client.apply_update(mclone_protocol::ServerUpdate::EntitySnapshot(
        mclone_protocol::EntitySnapshot {
            id: mclone_protocol::EntityId(1),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            position: Vec3d::new(3.0, 64.0, 4.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 0,
        },
    ));
    let entity_actors =
        actor_instances_from_presentations(&entity_client.actor_presentations(), &entity_client);

    assert_eq!(entity_actors.len(), 1);
    assert_eq!(
        entity_actors[0].shape,
        mclone_render::entity::ActorInstanceShape::CowModel
    );
    assert_eq!(entity_actors[0].width, 0.9);
    assert_eq!(entity_actors[0].height, 1.4);
    assert_eq!(
        entity_actors[0].packed_light,
        mclone_render::light_texture::FULL_BRIGHT
    );
}

#[test]
fn commit_player_pose_change_syncs_spectator_and_interest_center() {
    let scene = SceneOptions {
        render_distance: 1,
        ..SceneOptions::default()
    };
    let mut app = test_app_with_runtime(scene);
    let target_eye = Vec3d::new(16.25, 96.0, 8.0);
    app.driver.camera.set_eye_pose(target_eye, 0.0, 0.0);

    assert!(app.commit_player_pose_change().unwrap());

    assert_eq!(
        app.driver.spectator.position,
        glam::Vec3::new(
            target_eye.x as f32,
            target_eye.y as f32,
            target_eye.z as f32
        )
    );
    assert_eq!(
        app.driver.camera_frame_state().camera.chunk_pos,
        mclone_core::ChunkPos::new(1, 0)
    );
    assert_eq!(
        app.driver.runtime.as_ref().unwrap().stats().interest_center,
        mclone_core::ChunkPos::new(1, 0)
    );
}

#[test]
fn window_camera_view_uses_controller_snapshot_not_spectator_mirror() {
    let scene = SceneOptions::default();
    let mut app = test_app_with_runtime(scene);
    let target_eye = Vec3d::new(32.25, 80.0, -0.25);

    app.driver.camera.set_eye_pose(target_eye, 0.25, -0.125);
    let view = app.window_camera_view();

    assert_eq!(
        view.eye,
        glam::Vec3::new(
            target_eye.x as f32,
            target_eye.y as f32,
            target_eye.z as f32
        )
    );
    assert_eq!(view.snapshot.chunk_pos, mclone_core::ChunkPos::new(2, -1));
    assert_ne!(app.driver.spectator.position, view.eye);
}
