use super::*;

#[test]
fn third_person_render_camera_tracks_player_view_from_behind() {
    let snapshot = EngineCameraSnapshot::from_eye_pose(Vec3d::new(8.0, 70.0, 8.0), 0.0, 0.0, 24.0);

    let first = legacy_chunk_camera_from_snapshot(snapshot, 2);
    let third = legacy_chunk_camera_from_snapshot_with_view_mode(
        snapshot,
        EngineCameraViewMode::ThirdPersonBack,
        2,
    );

    assert_eq!(first.eye, [8.0, 70.0, 8.0]);
    assert_eq!(first.target, [8.0, 70.0, 9.0]);
    assert_eq!(third.eye, [8.0, 70.0, 4.0]);
    assert_eq!(third.target, [8.0, 70.0, 5.0]);
    assert_eq!(third.z_far, first.z_far);

    let first_pose = render_pose_from_snapshot(snapshot, 2);
    let third_pose = render_pose_from_snapshot_with_view_mode(
        snapshot,
        EngineCameraViewMode::ThirdPersonBack,
        2,
    );
    assert_vec3_close(first_pose.eye, Vec3::new(8.0, 70.0, 8.0));
    assert_vec3_close(third_pose.eye, Vec3::new(8.0, 70.0, 4.0));
    assert_eq!(third_pose.z_far, first_pose.z_far);
}

#[test]
fn render_pose_uses_engine_yaw_pitch_convention() {
    let eye = Vec3d::new(8.0, 70.0, 8.0);
    let forward =
        render_pose_from_snapshot(EngineCameraSnapshot::from_eye_pose(eye, 0.0, 0.0, 24.0), 0)
            .render_view(1280, 720)
            .expect("forward render pose")
            .camera_forward;
    let east = render_pose_from_snapshot(
        EngineCameraSnapshot::from_eye_pose(eye, std::f64::consts::FRAC_PI_2, 0.0, 24.0),
        0,
    )
    .render_view(1280, 720)
    .expect("east render pose")
    .camera_forward;
    let up = render_pose_from_snapshot(
        EngineCameraSnapshot::from_eye_pose(eye, 0.0, std::f64::consts::FRAC_PI_2, 24.0),
        0,
    )
    .render_view(1280, 720)
    .expect("up render pose")
    .camera_forward;
    let down = render_pose_from_snapshot(
        EngineCameraSnapshot::from_eye_pose(eye, 0.0, -std::f64::consts::FRAC_PI_2, 24.0),
        0,
    )
    .render_view(1280, 720)
    .expect("down render pose")
    .camera_forward;

    assert_vec3_close(forward, Vec3::Z);
    assert_vec3_close(east, Vec3::X);
    assert_vec3_close(up, Vec3::Y);
    assert_vec3_close(down, Vec3::NEG_Y);
}

#[test]
fn local_player_actor_instance_uses_controller_feet_pose() {
    let client = ClientRuntime::local_integrated();
    let camera = EngineCameraController::from_eye_pose(
        Vec3d::new(1.25, 70.62, -3.5),
        std::f64::consts::FRAC_PI_2,
        0.0,
        24.0,
    );

    let actor = local_player_actor_instance(&camera, &client, default_player_figure_id());

    assert_eq!(actor.feet_position, Vec3::new(1.25, 69.0, -3.5));
    assert!((actor.yaw_radians - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
    assert_eq!(
        actor.shape,
        mclone_render::entity::ActorInstanceShape::Figure(default_player_figure_id())
    );
}

#[test]
fn local_player_actor_instance_uses_selected_figure() {
    let client = ClientRuntime::local_integrated();
    let camera =
        EngineCameraController::from_eye_pose(Vec3d::new(1.25, 70.62, -3.5), 0.0, 0.0, 24.0);

    let actor =
        local_player_actor_instance(&camera, &client, mclone_assets::upright_bear_figure_id());

    assert_eq!(
        actor.shape,
        mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::upright_bear_figure_id())
    );
}

#[test]
fn chicken_entity_actor_uses_chicken_figure() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(7)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Chicken),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.4,
        height: 0.7,
        animation: Some(mclone_core::AnimationState::distance(
            mclone_core::AnimationClipId::from_static("walk"),
            0,
        )),
        animation_clock_tick: 0,
        walk_animation_distance: 0.25,
        chicken_wing_flap_radians: Some(0.4),
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].id,
        Some(mclone_render::entity::ActorInstanceId::Entity(7))
    );
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::chicken_figure_id())
    );
    assert_eq!(actors[0].width, 0.4);
    assert_eq!(actors[0].height, 0.7);
    assert!(actors[0].animation.is_some());
    assert_eq!(actors[0].chicken_wing_flap_radians, Some(0.4));
}

#[test]
fn cow_entity_actor_uses_authored_cow_figure() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(8)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Cow),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.9,
        height: 1.4,
        animation: Some(mclone_core::AnimationState::distance(
            mclone_core::AnimationClipId::from_static("walk"),
            0,
        )),
        animation_clock_tick: 0,
        walk_animation_distance: 0.25,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::cow_figure_id())
    );
    assert_eq!(actors[0].width, 0.9);
    assert_eq!(actors[0].height, 1.4);
    assert!(actors[0].animation.is_some());
}

#[test]
fn mallard_entity_actor_uses_authored_mallard_figure() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(9)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Mallard),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: Some(mclone_protocol::MallardLifeStage::Adult),
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.7,
        height: 0.75,
        animation: Some(mclone_core::AnimationState::distance(
            mclone_core::AnimationClipId::from_static("waddle"),
            0,
        )),
        animation_clock_tick: 0,
        walk_animation_distance: 0.25,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::Figure(mclone_assets::mallard_duck_figure_id())
    );
    assert_eq!(actors[0].width, 0.7);
    assert_eq!(actors[0].height, 0.75);
    assert_eq!(actors[0].feet_position, Vec3::new(1.0, 64.0, 2.0));
    assert!(actors[0].animation.is_some());
    assert_eq!(
        actors[0].animation.expect("mallard animation").clip,
        mclone_core::AnimationClipId::from_static("waddle")
    );
}

#[test]
fn elapsed_action_uses_the_replicated_world_clock_without_restarting() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(91)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Cow),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.9,
        height: 1.4,
        animation: Some(mclone_core::AnimationState::elapsed(
            mclone_core::AnimationClipId::from_static("alert"),
            6,
            100,
        )),
        animation_clock_tick: 140,
        walk_animation_distance: 7.5,
        chicken_wing_flap_radians: None,
    };

    let actor = actor_instances_from_presentations(&[presentation], &client)[0];

    assert_eq!(
        actor.animation,
        Some(mclone_render::entity::ActorAnimation {
            clip: mclone_core::AnimationClipId::from_static("alert"),
            phase: mclone_render::entity::ActorAnimationPhase::ElapsedSeconds(2.0),
        })
    );
}

#[test]
fn swimming_mallard_sinks_its_figure_below_the_waterline() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(10)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Mallard),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: Some(mclone_protocol::MallardLifeStage::Adult),
        in_water: true,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.88, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: false,
        width: 0.7,
        height: 0.75,
        animation: Some(mclone_core::AnimationState::distance(
            mclone_core::AnimationClipId::from_static("waddle"),
            0,
        )),
        animation_clock_tick: 0,
        walk_animation_distance: 0.25,
        chicken_wing_flap_radians: None,
    };

    let actor = actor_instances_from_presentations(&[presentation], &client)
        .into_iter()
        .next()
        .expect("swimming mallard actor");

    assert_eq!(
        actor.id,
        Some(mclone_render::entity::ActorInstanceId::Entity(10))
    );
    assert!((actor.feet_position.x - 1.0).abs() < 1.0e-6);
    assert!((actor.feet_position.z - 2.0).abs() < 1.0e-6);
    assert!(
        (actor.feet_position.y - (64.88 - actor.height * MALLARD_SWIM_VISUAL_SINK_HEIGHT_FACTOR))
            .abs()
            < 1.0e-5
    );
    assert!(actor.feet_position.y < 64.88);
}

#[test]
fn actor_instance_identity_is_stable_across_presentation_reordering() {
    let client = ClientRuntime::local_integrated();
    let make = |id, kind| ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(id)),
        kind: ActorPresentationKind::Entity(kind),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(id as f64, 64.0, 2.0),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.6,
        height: 1.8,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };
    let presentations = [
        make(22, mclone_protocol::EntityKind::Mannequin),
        make(11, mclone_protocol::EntityKind::Chicken),
    ];

    let actors = actor_instances_from_presentations(&presentations, &client);

    assert_eq!(
        actors.iter().map(|actor| actor.id).collect::<Vec<_>>(),
        vec![
            Some(mclone_render::entity::ActorInstanceId::Entity(22)),
            Some(mclone_render::entity::ActorInstanceId::Entity(11)),
        ]
    );
}

#[test]
fn actor_instance_selects_the_lift_nearest_its_observer() {
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(mclone_protocol::ServerUpdate::WorldInfo {
        dimension: mclone_protocol::DimensionKey::overworld(),
        biome_zoom_seed: 12_345,
        topology: mclone_core::HorizontalTopology::cylinder_x(0, 32),
    });
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(7)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Mannequin),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(0.25, 64.0, 2.0),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.6,
        height: 1.8,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };

    let seam_actors = actor_instances_from_presentations_near_observer(
        &[presentation],
        &client,
        Vec3d::new(511.75, 64.0, 2.0),
    );
    let origin_actors = actor_instances_from_presentations_near_observer(
        &[presentation],
        &client,
        Vec3d::new(0.75, 64.0, 2.0),
    );

    assert_eq!(seam_actors[0].feet_position.x, 512.25);
    assert_eq!(origin_actors[0].feet_position.x, 0.25);
    assert_eq!(seam_actors[0].id, origin_actors[0].id);
    assert_eq!(seam_actors[0].id, Some(ActorInstanceId::Entity(7)));
}

#[test]
fn item_entity_actor_uses_egg_item_shape() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(8)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Item),
        appearance: ActorAppearance::NONE,
        item_stack: Some(mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::Egg,
            count: 1,
        }),
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: false,
        width: 0.25,
        height: 0.25,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::ItemEgg
    );
    assert_eq!(actors[0].width, 0.25);
    assert_eq!(actors[0].height, 0.25);
}

#[test]
fn mallard_egg_item_actor_uses_egg_item_shape() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(10)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Item),
        appearance: ActorAppearance::NONE,
        item_stack: Some(mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::MallardEgg,
            count: 1,
        }),
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: false,
        width: 0.25,
        height: 0.25,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::ItemEgg
    );
}

#[test]
fn mallard_feather_item_actor_uses_live_semantic_prop() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(12)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::Item),
        appearance: ActorAppearance::NONE,
        item_stack: Some(mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::MallardFeather,
            count: 1,
        }),
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: None,
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 45.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: false,
        width: 0.25,
        height: 0.25,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::SemanticProp(
            mclone_assets::mallard_feather_figure_id()
        )
    );
}

#[test]
fn mallard_nest_entity_actor_uses_live_semantic_prop() {
    let client = ClientRuntime::local_integrated();
    let presentation = ActorPresentation {
        id: ActorPresentationId::Entity(mclone_protocol::EntityId(13)),
        kind: ActorPresentationKind::Entity(mclone_protocol::EntityKind::MallardNest),
        appearance: ActorAppearance::NONE,
        item_stack: None,
        mallard_life_stage: None,
        in_water: false,
        mallard_nest: Some(mclone_protocol::MallardNestSnapshotData {
            incubation_progress: 100,
            incubation_required: 2_400,
            attended: true,
        }),
        feet_position: Vec3d::new(1.0, 64.0, 2.0),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        rotation: None,
        on_ground: true,
        width: 0.8,
        height: 0.32,
        animation: None,
        animation_clock_tick: 0,
        walk_animation_distance: 0.0,
        chicken_wing_flap_radians: None,
    };

    let actors = actor_instances_from_presentations(&[presentation], &client);

    assert_eq!(actors.len(), 1);
    assert_eq!(
        actors[0].shape,
        mclone_render::entity::ActorInstanceShape::SemanticProp(
            mclone_assets::mallard_nest_figure_id()
        )
    );
}

#[test]
fn local_player_actor_for_view_hides_first_person_by_default() {
    let client = ClientRuntime::local_integrated();
    let camera =
        EngineCameraController::from_eye_pose(Vec3d::new(1.25, 70.62, -3.5), 0.0, 0.0, 24.0);

    assert!(
        local_player_actor_instance_for_view(&camera, &client, default_player_figure_id())
            .is_none()
    );
}

#[test]
fn local_player_actor_for_view_can_render_first_person_body_only() {
    let client = ClientRuntime::local_integrated();
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(1.25, 70.62, -3.5), 0.0, 0.0, 24.0);
    camera.set_first_person_player_visible(true);

    let actor =
        local_player_actor_instance_for_view(&camera, &client, default_player_figure_id()).unwrap();

    assert_eq!(actor.feet_position, Vec3::new(1.25, 69.0, -3.5));
    assert!(actor.first_person_body_only);
}

#[test]
fn local_player_actor_for_view_keeps_third_person_full_body() {
    let client = ClientRuntime::local_integrated();
    let mut camera =
        EngineCameraController::from_eye_pose(Vec3d::new(1.25, 70.62, -3.5), 0.0, 0.0, 24.0);
    camera.set_view_mode(EngineCameraViewMode::ThirdPersonBack);
    camera.set_first_person_player_visible(true);

    let actor =
        local_player_actor_instance_for_view(&camera, &client, default_player_figure_id()).unwrap();

    assert!(!actor.first_person_body_only);
}

#[test]
fn engine_camera_controller_picks_block_from_player_view() {
    let target = BlockPos::new(1, 2, 4);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
        ChunkPos::new(0, 0),
        target,
        BlockStateId(1),
    )));
    let interaction = ClientInteractionController::new();
    let camera = EngineCameraController::from_eye_pose(Vec3d::new(1.5, 2.5, 1.5), 0.0, 0.0, 32.0);

    let hit = camera.pick_block(&client, &interaction);

    assert_eq!(hit.hit_type(), HitResultType::Block);
    assert_eq!(hit.block_pos, target);
}

#[test]
fn engine_camera_render_pose_targets_forward_direction() {
    let camera = EngineCameraController::from_eye_pose(Vec3d::new(1.0, 2.0, 3.0), 0.0, 0.0, 32.0);

    let render_view = camera
        .render_pose(2)
        .render_view(1280, 720)
        .expect("render pose should build");

    assert_vec3_close(render_view.camera_position, Vec3::new(1.0, 2.0, 3.0));
    assert_vec3_close(render_view.camera_forward, Vec3::Z);
    assert!(render_view.z_far > 700.0);
}
