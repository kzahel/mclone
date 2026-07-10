use super::*;

#[test]
fn local_world_start_plan_sets_loading_status_without_closing_menu() {
    let scene = SceneOptions {
        remote_addr: Some("127.0.0.1:25565".to_owned()),
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );
    app.driver.set_ui_screen(Some(GameScreen::NewWorld));

    let result = app
        .driver
        .apply_ui_action(GameUiAction::CreateWorld(44), ui_action_context());

    assert_eq!(
        app.driver.session.state(),
        &GameSessionState::Starting {
            request: SessionStartRequest::new_seed_local_world(44)
        }
    );
    let status = app.driver.session.status().unwrap();
    assert!(status.ok);
    assert_eq!(status.message, "Creating world...");
    let pending = result.session_start.expect("local session plan");
    assert_eq!(
        pending.session.request,
        SessionStartRequest::new_seed_local_world(44)
    );
    assert_eq!(pending.session.payload.options.seed, 44);
    assert_eq!(pending.session.payload.options.remote_addr, None);
    assert!(pending.arm_mouse_lock);
    assert!(!pending.show_title_on_failure);
    assert_eq!(app.driver.ui_screen(), Some(GameScreen::NewWorld));
    assert!(!app.mouse_lock_requested);
}

#[test]
fn remote_session_start_plan_sets_connecting_status_without_closing_menu() {
    let scene = SceneOptions::default();
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );
    app.driver.set_ui_screen(Some(GameScreen::JoinRemote));

    app.driver.set_join_remote_addr("10.0.0.5:25565");
    let result = app
        .driver
        .apply_ui_action(GameUiAction::JoinRemote, ui_action_context());

    assert_eq!(
        app.driver.session.state(),
        &GameSessionState::Starting {
            request: SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565")
            }
        }
    );
    let status = app.driver.session.status().unwrap();
    assert!(status.ok);
    assert_eq!(status.message, "Connecting...");
    let pending = result.session_start.expect("remote session plan");
    assert_eq!(
        pending.session.request,
        SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565")
        }
    );
    assert_eq!(
        pending.session.payload.options.remote_addr,
        Some("10.0.0.5:25565".to_owned())
    );
    assert_eq!(pending.session.payload.options.seed, DEFAULT_SEED);
    assert!(pending.arm_mouse_lock);
    assert!(!pending.show_title_on_failure);
    assert_eq!(app.driver.ui_screen(), Some(GameScreen::JoinRemote));
    assert_eq!(app.driver.join_remote_addr(), "10.0.0.5:25565");
    assert!(!app.mouse_lock_requested);
}

#[test]
fn planned_local_world_start_creates_startup_pump_without_blocking_until_active() {
    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );
    let mut result = app
        .driver
        .apply_ui_action(GameUiAction::CreateWorld(77), ui_action_context());
    app.start_session_plan(result.session_start.take().unwrap());

    assert!(app.driver.startup.is_some());
    assert!(app.driver.runtime.is_none());
    assert_eq!(
        app.driver.session.state(),
        &GameSessionState::Starting {
            request: SessionStartRequest::new_seed_local_world(77)
        }
    );
    assert!(!app.mouse_lock_requested);
}

#[test]
fn mid_session_replacement_tears_down_active_runtime_and_starts_shared_plan() {
    let scene = SceneOptions {
        seed: 11,
        render_distance: 0,
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene.clone(),
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );
    app.start_world_from_scene(scene).unwrap();
    app.driver
        .session
        .complete_start(ActiveSessionDescriptor::new_seed_local_world(11));

    let mut result = app
        .driver
        .apply_ui_action(GameUiAction::CreateWorld(22), ui_action_context());
    let plan = result.session_start.take().expect("replacement plan");
    assert!(plan.transition.teardown_world);
    app.start_session_plan(plan);

    assert!(app.driver.runtime.is_none());
    assert!(app.driver.startup.is_some());
    assert_eq!(
        app.driver.session.state(),
        &GameSessionState::Starting {
            request: SessionStartRequest::new_seed_local_world(22)
        }
    );
}

#[test]
fn start_local_world_rebuilds_runtime_for_new_seed_without_stale_sections() {
    let scene = SceneOptions {
        seed: 12_345,
        render_distance: 0,
        ..SceneOptions::default()
    };
    let center = mclone_core::ChunkPos::new(scene.chunk_x, scene.chunk_z);
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene.clone(),
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );

    app.start_world_from_scene(scene).unwrap();
    let first_runtime = app.driver.runtime.as_ref().unwrap();
    assert!(first_runtime.client().chunk_snapshot(center).is_some());
    let first_signature = center_chunk_signature(first_runtime, center);

    assert_eq!(app.driver.render_stats.section_count, 0);
    assert_eq!(app.driver.render_stats.index_count, 0);

    app.start_local_world(98_765).unwrap();
    let second_runtime = app.driver.runtime.as_ref().unwrap();
    assert_eq!(app.driver.scene.seed, 98_765);
    assert_eq!(app.driver.scene.remote_addr, None);
    assert!(second_runtime.client().chunk_snapshot(center).is_some());
    let second_signature = center_chunk_signature(second_runtime, center);

    assert_ne!(first_signature, second_signature);
    assert_eq!(app.driver.render_stats.section_count, 0);
    assert_eq!(app.driver.render_stats.index_count, 0);
}

#[test]
fn catalog_world_create_edit_quit_reopen_preserves_block_edit() {
    let root = unique_temp_world_root("catalog-create-edit-reopen");
    let scene = SceneOptions {
        seed: 24_680,
        render_distance: 0,
        lighting_enabled: false,
        debug_passive_showcase: false,
        world_root: Some(root.clone()),
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );

    app.driver.set_new_world_seed(24_680);
    let create = app
        .driver
        .apply_ui_action(GameUiAction::CreateCatalogWorld, ui_action_context());
    assert!(create.session_start.is_some());
    start_planned_session_immediately(&mut app, create);

    let created_id = match app.driver.session.state() {
        GameSessionState::Active { session } => session
            .local_world_id()
            .expect("catalog-created session must carry a local world id")
            .clone(),
        state => panic!("expected active catalog session, got {state:?}"),
    };
    let center = mclone_core::ChunkPos::new(app.driver.scene.chunk_x, app.driver.scene.chunk_z);
    let snapshot = app
        .driver
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.client().chunk_snapshot(center))
        .expect("catalog-created world should load center chunk")
        .clone();
    let target = first_non_air_block(&snapshot).expect("generated center chunk has blocks");
    assert_ne!(snapshot_block_state(&snapshot, target), AIR_BLOCK_STATE_ID);

    let runtime = app.driver.runtime.as_mut().expect("runtime is active");
    runtime
        .send_gameplay_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: Vec3d::new(
                target.x as f64 + 0.5,
                target.y as f64,
                target.z as f64 + 0.5,
            ),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .unwrap();
    app.driver.poll_runtime().unwrap();
    let runtime = app.driver.runtime.as_mut().expect("runtime is active");
    runtime
        .send_gameplay_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: target,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .unwrap();
    wait_for_client_block_state(&mut app, target, AIR_BLOCK_STATE_ID);

    let transition = app.driver.quit_to_title_transition_effects();
    if transition.teardown_world {
        app.teardown_world().unwrap();
    }
    app.driver
        .apply_client_session_transition_effects(transition);
    assert_eq!(app.driver.ui_screen(), Some(GameScreen::Title));
    assert!(app.driver.runtime.is_none());

    app.driver
        .apply_ui_action(GameUiAction::OpenWorldList, ui_action_context());
    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    let row = state
        .world_catalog
        .entries
        .iter()
        .flatten()
        .find(|entry| entry.display_name.as_str() == "New World")
        .expect("created world should be listed after quit");
    let open = app
        .driver
        .apply_ui_action(GameUiAction::OpenWorld(row.id), ui_action_context());
    assert!(open.session_start.is_some());
    start_planned_session_immediately(&mut app, open);

    match app.driver.session.state() {
        GameSessionState::Active { session } => {
            assert_eq!(session.local_world_id(), Some(&created_id));
        }
        state => panic!("expected reopened catalog session, got {state:?}"),
    }
    assert_eq!(
        app.driver
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().block_state_at_block_pos(target)),
        Some(AIR_BLOCK_STATE_ID)
    );

    app.teardown_world().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn catalog_world_delete_after_quit_removes_entry_and_blocks_reopen() {
    let root = unique_temp_world_root("catalog-delete-after-quit");
    let scene = SceneOptions {
        seed: 13_579,
        render_distance: 0,
        lighting_enabled: false,
        debug_passive_showcase: false,
        world_root: Some(root.clone()),
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );

    app.driver.set_new_world_seed(13_579);
    let create = app
        .driver
        .apply_ui_action(GameUiAction::CreateCatalogWorld, ui_action_context());
    assert!(create.session_start.is_some());
    start_planned_session_immediately(&mut app, create);

    let created_id = match app.driver.session.state() {
        GameSessionState::Active { session } => session
            .local_world_id()
            .expect("catalog-created session must carry a local world id")
            .clone(),
        state => panic!("expected active catalog session, got {state:?}"),
    };
    let world_dir = root.join(created_id.as_str());
    assert!(world_dir.join("world.json").is_file());
    assert!(world_dir.join("world.sqlite3").is_file());

    let transition = app.driver.quit_to_title_transition_effects();
    if transition.teardown_world {
        app.teardown_world().unwrap();
    }
    app.driver
        .apply_client_session_transition_effects(transition);
    assert_eq!(app.driver.ui_screen(), Some(GameScreen::Title));
    assert!(app.driver.runtime.is_none());

    app.driver
        .apply_ui_action(GameUiAction::OpenWorldList, ui_action_context());
    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    let row_id = state
        .world_catalog
        .entries
        .iter()
        .flatten()
        .find(|entry| entry.display_name.as_str() == "New World")
        .expect("created world should be listed after quit")
        .id;

    let delete = app
        .driver
        .apply_ui_action(GameUiAction::DeleteWorld(row_id), ui_action_context());
    assert!(delete.session_start.is_none());
    assert!(!world_dir.exists());

    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    assert!(
        state
            .world_catalog
            .entries
            .iter()
            .flatten()
            .all(|entry| entry.id != row_id)
    );
    assert!(state.world_catalog.status.visible);
    assert!(state.world_catalog.status.ok);

    let reopen = app
        .driver
        .apply_ui_action(GameUiAction::OpenWorld(row_id), ui_action_context());
    assert!(reopen.session_start.is_none());
    assert!(app.driver.session.take_pending_start().is_none());
    assert!(app.driver.runtime.is_none());
    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    assert!(state.world_catalog.status.visible);
    assert!(!state.world_catalog.status.ok);
    assert!(
        state
            .world_catalog
            .status
            .message
            .as_str()
            .contains("Selected world is unavailable")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn catalog_world_delete_active_world_is_rejected_without_teardown() {
    let root = unique_temp_world_root("catalog-delete-active");
    let scene = SceneOptions {
        seed: 97_531,
        render_distance: 0,
        lighting_enabled: false,
        debug_passive_showcase: false,
        world_root: Some(root.clone()),
        ..SceneOptions::default()
    };
    let assets = WindowSceneAssets::load().unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::Menu,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );

    app.driver.set_new_world_seed(97_531);
    let create = app
        .driver
        .apply_ui_action(GameUiAction::CreateCatalogWorld, ui_action_context());
    assert!(create.session_start.is_some());
    start_planned_session_immediately(&mut app, create);

    let created_id = match app.driver.session.state() {
        GameSessionState::Active { session } => session
            .local_world_id()
            .expect("catalog-created session must carry a local world id")
            .clone(),
        state => panic!("expected active catalog session, got {state:?}"),
    };
    let world_dir = root.join(created_id.as_str());
    assert!(world_dir.join("world.json").is_file());
    assert!(world_dir.join("world.sqlite3").is_file());
    assert!(app.driver.runtime.is_some());

    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    let active_row = state
        .world_catalog
        .active
        .expect("active catalog session should mark its world row");
    assert!(
        state
            .world_catalog
            .entries
            .iter()
            .flatten()
            .any(|entry| entry.id == active_row)
    );

    let delete = app
        .driver
        .apply_ui_action(GameUiAction::DeleteWorld(active_row), ui_action_context());
    assert!(delete.session_start.is_none());
    assert!(app.driver.session.take_pending_start().is_none());
    assert!(app.driver.runtime.is_some());
    assert!(world_dir.join("world.json").is_file());
    assert!(world_dir.join("world.sqlite3").is_file());
    match app.driver.session.state() {
        GameSessionState::Active { session } => {
            assert_eq!(session.local_world_id(), Some(&created_id));
        }
        state => panic!("expected active catalog session after rejected delete, got {state:?}"),
    }

    let state = app.driver.current_ui_render_state(
        crate::frame_pacing::FramePacingUiState::default(),
        mclone_ui::BlockPaletteOverlay::hidden(),
    );
    assert_eq!(state.world_catalog.active, Some(active_row));
    assert!(state.world_catalog.status.visible);
    assert!(!state.world_catalog.status.ok);
    assert!(
        state
            .world_catalog
            .status
            .message
            .as_str()
            .contains("cannot delete active local world")
    );

    app.teardown_world().unwrap();
    let _ = std::fs::remove_dir_all(root);
}
