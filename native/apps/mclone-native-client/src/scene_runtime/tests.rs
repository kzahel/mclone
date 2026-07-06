use super::*;
use crate::DEFAULT_SEED;
use crate::camera::SpectatorCamera;
use crate::render_cache::extracted_asset_root;
use mclone_core::{BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot, ChunkStatus};
use mclone_protocol::{ChunkView, SectionBlockUpdate};
use mclone_render::chunk::{
    ChunkCamera, TexturedSectionRenderOptions,
    textured_section_visibility_stats_with_options_and_ready_sections,
};
use mclone_render_session::{
    RenderSectionNeighborReadiness, render_dirty_section_keys_for_block_update,
    render_section_center, render_section_keys_for_snapshot, sort_chunk_positions_by_distance,
};

#[test]
fn camera_water_detection_uses_fluid_height_boundary() {
    let water = mclone_client::block_facts::WATER_BLOCK_STATE_ID;
    let lava = mclone_client::block_facts::LAVA_BLOCK_STATE_ID;
    let air = AIR_BLOCK_STATE_ID;

    assert!(camera_position_inside_water_block(62.999, 62, water));
    assert!(!camera_position_inside_water_block(63.0, 62, water));
    assert!(!camera_position_inside_water_block(62.5, 62, lava));
    assert!(!camera_position_inside_water_block(62.5, 62, air));
    assert!(!camera_position_inside_water_block(f32::NAN, 62, water));
}

#[test]
fn snapshot_block_state_lookup_reads_loaded_sections_and_omitted_air() {
    let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
    block_state_ids[mclone_core::chunk_section_index(1, 15, 15)] = BlockStateId(42);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(1, -1),
        ChunkStatus::Full,
        ChunkRevision(1),
        -16,
        32,
        &block_state_ids,
    );

    assert_eq!(
        snapshot_block_state_at_world(&snapshot, 17, -1, -1),
        Some(BlockStateId(42))
    );
    assert_eq!(
        snapshot_block_state_at_world(&snapshot, 17, 0, -1),
        Some(AIR_BLOCK_STATE_ID)
    );
    assert_eq!(snapshot_block_state_at_world(&snapshot, 0, -1, -1), None);
    assert_eq!(snapshot_block_state_at_world(&snapshot, 17, 16, -1), None);
}

#[test]
fn render_section_readiness_uses_near_exception_and_horizontal_neighbors() {
    let target = ChunkPos::new(2, -3);
    let key = RenderSectionKey::new(target.x, 0, target.z);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(target)));

    assert_eq!(
        render_section_neighbor_readiness(&client, key, render_section_center(key)),
        RenderSectionNeighborReadiness::ReadyNearCamera
    );

    let high_camera = render_section_center(key) + Vec3::new(0.0, 256.0, 0.0);
    assert_eq!(
        render_section_neighbor_readiness(&client, key, high_camera),
        RenderSectionNeighborReadiness::ReadyNearCamera
    );

    let diagonal_neighbor_key = RenderSectionKey::new(target.x + 1, 0, target.z + 1);
    assert_eq!(
        render_section_neighbor_readiness(&client, diagonal_neighbor_key, high_camera),
        RenderSectionNeighborReadiness::ReadyNearCamera
    );

    let far_camera = render_section_center(key) + Vec3::new(128.0, 0.0, 0.0);
    assert_eq!(
        render_section_neighbor_readiness(&client, key, far_camera),
        RenderSectionNeighborReadiness::DeferredMissingNeighbors
    );

    for neighbor in [
        ChunkPos::new(target.x - 1, target.z),
        ChunkPos::new(target.x + 1, target.z),
        ChunkPos::new(target.x, target.z - 1),
        ChunkPos::new(target.x, target.z + 1),
    ] {
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(neighbor)));
    }

    assert_eq!(
        render_section_neighbor_readiness(&client, key, far_camera),
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    );
}

#[test]
fn render_distance_derives_java_shaped_tracking_radius() {
    assert_eq!(chunk_tracking_radius_for_render_distance(0), 0);
    assert_eq!(chunk_tracking_radius_for_render_distance(1), 1);
    assert_eq!(chunk_tracking_radius_for_render_distance(2), 3);
    assert_eq!(chunk_tracking_radius_for_render_distance(3), 4);
    assert_eq!(chunk_tracking_radius_for_render_distance(4), 5);
}

#[test]
fn window_runtime_local_integrated_defaults_to_native_thread_runner() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let runtime = WindowSceneRuntime::new(&scene).unwrap();
    let stats = runtime.stats();

    assert_eq!(
        stats.server_runner_kind,
        Some(ServerRunnerKind::NativeThread)
    );
    assert_ne!(
        stats.server_runner_kind,
        Some(ServerRunnerKind::InlineFallback)
    );
}

#[test]
fn render_section_keys_cover_snapshot_height() {
    let snapshot = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(-1, 4),
        ChunkStatus::Full,
        ChunkRevision(1),
        -16,
        32,
        &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2],
    );

    assert_eq!(
        render_section_keys_for_snapshot(&snapshot),
        vec![
            RenderSectionKey::new(-1, -1, 4),
            RenderSectionKey::new(-1, 0, 4)
        ]
    );
}

#[test]
fn block_delta_dirty_sections_stay_local_for_interior_blocks() {
    let keys = render_dirty_section_keys_for_block_update(
        ChunkPos::new(2, -3),
        5,
        &SectionBlockUpdate {
            local_x: 8,
            local_y: 8,
            local_z: 8,
            block_state: BlockStateId(42),
        },
    );

    assert_eq!(keys, BTreeSet::from([RenderSectionKey::new(2, 5, -3)]));
}

#[test]
fn block_delta_dirty_sections_cross_section_boundaries() {
    let keys = render_dirty_section_keys_for_block_update(
        ChunkPos::new(0, 0),
        0,
        &SectionBlockUpdate {
            local_x: 0,
            local_y: 0,
            local_z: 15,
            block_state: BlockStateId(42),
        },
    );

    assert_eq!(
        keys,
        BTreeSet::from([
            RenderSectionKey::new(-1, -1, 0),
            RenderSectionKey::new(-1, -1, 1),
            RenderSectionKey::new(-1, 0, 0),
            RenderSectionKey::new(-1, 0, 1),
            RenderSectionKey::new(0, -1, 0),
            RenderSectionKey::new(0, -1, 1),
            RenderSectionKey::new(0, 0, 0),
            RenderSectionKey::new(0, 0, 1),
        ])
    );
}

#[test]
fn window_runtime_streams_chunks_when_spectator_crosses_boundary() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    let initial_center = ChunkPos::new(0, 0);
    let initial_stats = runtime.stats();
    assert_eq!(initial_stats.interest_center, initial_center);
    assert_eq!(
        initial_stats.server_runner_kind,
        Some(ServerRunnerKind::NativeThread)
    );
    assert_eq!(initial_stats.loaded_chunks, 1);
    assert_eq!(initial_stats.pending_jobs, 0);
    assert!(runtime.client().chunk_snapshot(initial_center).is_some());
    assert!(
        runtime
            .highest_non_air_block_y_at_world(8, 8)
            .is_some_and(|y| (-64..320).contains(&y))
    );

    let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
    let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
    assert_eq!(initial_submit.rebuilt_section_count(), 0);
    assert!(initial_submit.submitted_compile_section_count > 0);
    assert!(runtime.stats().pending_render_compile_jobs > 0);

    let initial_update = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    assert!(initial_update.rebuilt_section_count() > 0);
    assert_eq!(initial_update.removed_section_count(), 0);
    let initial_sections = runtime.cached_sections();
    assert!(!initial_sections.is_empty());
    assert!(section_index_count(&initial_sections) > 0);
    assert!(
        initial_sections
            .iter()
            .all(|section| section.key.chunk_x == 0 && section.key.chunk_z == 0)
    );

    spectator.position.x = 16.25;
    let next_center = spectator.chunk_pos();
    assert_eq!(next_center, ChunkPos::new(1, 0));
    assert!(runtime.set_interest_center(next_center).unwrap());
    assert_eq!(runtime.stats().interest_center, next_center);

    poll_window_runtime_until_idle(&mut runtime).unwrap();

    let moved_stats = runtime.stats();
    assert_eq!(moved_stats.interest_center, next_center);
    assert_eq!(moved_stats.loaded_chunks, 1);
    assert_eq!(moved_stats.pending_jobs, 0);
    assert!(runtime.client().chunk_snapshot(initial_center).is_none());
    assert!(runtime.client().chunk_snapshot(next_center).is_some());

    let moved_update = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    assert!(moved_update.rebuilt_section_count() > 0);
    assert!(moved_update.removed_section_count() > 0);
    let moved_sections = runtime.cached_sections();
    assert!(!moved_sections.is_empty());
    assert!(section_index_count(&moved_sections) > 0);
    assert!(
        moved_sections
            .iter()
            .all(|section| section.key.chunk_x == 1 && section.key.chunk_z == 0)
    );
}

#[test]
fn window_runtime_defers_far_boundary_sections_without_neighbors() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    let far_camera = spectator.position + Vec3::new(128.0, 0.0, 0.0);
    let update = runtime.sync_all_render_sections(far_camera).unwrap();

    assert_eq!(update.rebuilt_section_count(), 0);
    assert_eq!(update.near_exception_section_count, 0);
    assert!(update.deferred_section_count > 0);
    assert!(runtime.pending_render_chunk_count() > 0);
    assert!(!runtime.has_pending_render_work(far_camera));
    assert!(runtime.cached_sections().is_empty());

    let deferred_key = *runtime
        .render_session()
        .dirty()
        .dirty_sections
        .iter()
        .next()
        .expect("deferred dirty section should be retained");
    let ready_position = render_section_center(deferred_key);
    assert!(runtime.has_pending_render_work(ready_position));
    let ready_update = runtime.sync_all_render_sections(ready_position).unwrap();
    assert!(ready_update.rebuilt_section_count() > 0);
    assert!(
        !runtime
            .render_session()
            .dirty()
            .dirty_sections
            .contains(&deferred_key)
    );
}

#[test]
fn window_runtime_marks_section_block_updates_without_chunk_dirtying() {
    if !extracted_asset_root().exists() {
        return;
    }

    let mut runtime = WindowSceneRuntime::new(&SceneOptions::default()).unwrap();
    runtime
        .render_session_mut()
        .dirty_mut()
        .dirty_chunks
        .clear();
    runtime
        .render_session_mut()
        .dirty_mut()
        .dirty_sections
        .clear();

    runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
        pos: ChunkPos::new(0, 0),
        section_y: 7,
        updates: vec![SectionBlockUpdate {
            local_x: 8,
            local_y: 8,
            local_z: 8,
            block_state: AIR_BLOCK_STATE_ID,
        }],
    }]);

    assert!(runtime.render_session().dirty().dirty_chunks.is_empty());
    assert_eq!(
        runtime.render_session().dirty().dirty_sections,
        BTreeSet::from([RenderSectionKey::new(0, 7, 0)])
    );
}

#[test]
fn render_compile_scheduling_orders_dirty_chunks_by_camera_distance() {
    let camera = Vec3::new(8.0, 88.0, 8.0);
    let sorted = sort_chunk_positions_by_distance(
        [
            ChunkPos::new(4, 0),
            ChunkPos::new(0, 0),
            ChunkPos::new(-2, 0),
        ],
        camera,
    );

    assert_eq!(
        sorted,
        vec![
            ChunkPos::new(0, 0),
            ChunkPos::new(-2, 0),
            ChunkPos::new(4, 0)
        ]
    );
}

#[test]
fn render_compile_revisions_stale_only_changed_sections() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
    assert!(initial_submit.submitted_compile_section_count > 1);
    let changed_key = RenderSectionKey::new(0, 5, 0);
    assert!(
        runtime
            .render_session()
            .dirty()
            .inflight_sections
            .contains(&changed_key)
    );

    runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
        pos: ChunkPos::new(0, 0),
        section_y: 5,
        updates: vec![SectionBlockUpdate {
            local_x: 8,
            local_y: 8,
            local_z: 8,
            block_state: AIR_BLOCK_STATE_ID,
        }],
    }]);

    let completed = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();

    assert_eq!(completed.stale_compile_section_count, 1);
    assert!(
        completed.completed_compile_section_count >= initial_submit.submitted_compile_section_count
    );
    assert!(!runtime.has_pending_render_work(spectator.position));
}

#[test]
fn window_runtime_updates_render_distance_live() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    assert_eq!(runtime.render_distance(), 0);
    assert_eq!(runtime.chunk_tracking_radius(), 0);
    assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

    assert!(runtime.set_render_distance(1).unwrap());
    assert_eq!(runtime.render_distance(), 1);
    assert_eq!(runtime.chunk_tracking_radius(), 1);
    assert_eq!(runtime.client().chunk_view().unwrap().render_distance, 1);
    assert_eq!(
        runtime.client().chunk_view().unwrap().chunk_tracking_radius,
        1
    );
    poll_window_runtime_until_idle(&mut runtime).unwrap();
    assert_eq!(runtime.stats().loaded_chunks, square_count(1).unwrap());

    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    let grown_update = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    assert!(grown_update.rebuilt_section_count() > 0);
    assert_eq!(grown_update.removed_section_count(), 0);
    assert!(runtime.cached_sections().iter().any(|section| {
        section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
    }));

    assert!(runtime.set_render_distance(0).unwrap());
    poll_window_runtime_until_idle(&mut runtime).unwrap();
    assert_eq!(runtime.render_distance(), 0);
    assert_eq!(runtime.chunk_tracking_radius(), 0);
    assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

    let shrunk_update = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    assert!(shrunk_update.removed_section_count() > 0);
    assert!(runtime.cached_sections().iter().all(|section| {
        section.key.chunk_x == scene.chunk_x && section.key.chunk_z == scene.chunk_z
    }));
}

#[test]
fn window_runtime_mesh_queue_processes_ready_work_by_chunk_budget() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 1,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    assert_eq!(runtime.stats().loaded_chunks, 9);
    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    let first_update = runtime.sync_render_sections(spectator.position).unwrap();
    assert_eq!(first_update.rebuilt_section_count(), 0);
    assert!(first_update.submitted_compile_section_count > 0);
    assert!(runtime.stats().pending_render_compile_jobs > 0);
    assert!(runtime.pending_render_chunk_count() > 0);

    let first_sections = runtime.cached_sections();
    assert!(first_sections.is_empty());

    let remaining_update = runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    assert!(remaining_update.rebuilt_section_count() > 0);
    assert!(!runtime.has_pending_render_work(spectator.position));
    assert!(!runtime.cached_sections().is_empty());
}

#[test]
fn render_distance_two_tracks_extra_ring_without_drawing_it() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 2,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    assert_eq!(runtime.render_distance(), 2);
    assert_eq!(runtime.chunk_tracking_radius(), 3);
    assert_eq!(runtime.stats().loaded_chunks, square_count(3).unwrap());
    assert!(runtime.client().chunk_snapshots().any(|snapshot| {
        chunk_distance_from_scene_center(&scene, snapshot.pos.x, snapshot.pos.z) == 3
    }));

    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    let ready_sections = runtime.traversal_ready_render_section_keys(spectator.position);
    assert!(!ready_sections.is_empty());
    assert!(
        ready_sections
            .iter()
            .all(|key| { chunk_distance_from_scene_center(&scene, key.chunk_x, key.chunk_z) <= 2 })
    );
}

#[test]
fn high_altitude_radius_one_keeps_neighbor_chunks_draw_ready() {
    if !extracted_asset_root().exists() {
        return;
    }

    let scene = SceneOptions {
        render_distance: 1,
        ..SceneOptions::default()
    };
    let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
    poll_window_runtime_until_idle(&mut runtime).unwrap();

    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    runtime
        .sync_all_render_sections(spectator.position)
        .unwrap();
    let sections = runtime.cached_sections();
    assert!(sections.iter().any(|section| {
        section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
    }));

    let high_position = spectator.position + Vec3::new(0.0, 256.0, 0.0);
    let ready_sections = runtime.traversal_ready_render_section_keys(high_position);
    assert!(
        ready_sections
            .iter()
            .any(|key| { key.chunk_x != scene.chunk_x || key.chunk_z != scene.chunk_z })
    );

    let center_drawable_sections = sections
        .iter()
        .filter(|section| {
            section.key.chunk_x == scene.chunk_x
                && section.key.chunk_z == scene.chunk_z
                && !section.is_empty()
        })
        .count();
    let camera = ChunkCamera {
        eye: high_position.to_array(),
        target: (high_position + Vec3::NEG_Y).to_array(),
        up: [0.0, 0.0, 1.0],
        fov_y_radians: 100.0_f32.to_radians(),
        z_near: 0.05,
        z_far: 700.0,
    };
    let stats = textured_section_visibility_stats_with_options_and_ready_sections(
        &sections,
        camera.render_view(960, 640),
        TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        },
        Some(&ready_sections),
    );

    assert_eq!(stats.readiness_culled_section_count, 0);
    assert!(stats.drawn_section_count > center_drawable_sections);
}

#[test]
fn scene_chunk_positions_cover_square_radius() {
    let positions = SceneOptions {
        seed: 0,
        chunk_x: -2,
        chunk_z: 3,
        render_distance: 1,
        ..SceneOptions::default()
    }
    .chunk_positions()
    .collect::<Vec<_>>();

    assert_eq!(positions.len(), 9);
    assert!(positions.contains(&(-3, 2)));
    assert!(positions.contains(&(-2, 3)));
    assert!(positions.contains(&(-1, 4)));
}

#[test]
fn scene_client_runtime_loads_center_chunk_from_integrated_server() {
    let scene = SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    };
    let client = build_scene_client_runtime(&scene).unwrap();

    assert_eq!(client.loaded_chunk_count(), 1);
    assert!(
        client
            .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
            .is_some()
    );
}

#[test]
fn scene_client_runtime_loads_center_chunk_from_remote_server() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        mclone_net::complete_server_handshake(&mut stream).unwrap();
        let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        let mut updates = server.try_handle_command(command).unwrap();
        updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
        mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
    });
    let scene = SceneOptions {
        render_distance: 0,
        remote_addr: Some(addr.to_string()),
        ..SceneOptions::default()
    };

    let client = build_scene_client_runtime(&scene).unwrap();
    server.join().unwrap();

    assert_eq!(client.host(), ClientHost::RemoteDedicated);
    assert_eq!(client.loaded_chunk_count(), 1);
    assert!(
        client
            .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
            .is_some()
    );
}

#[test]
fn window_runtime_reuses_remote_session_for_interest_updates() {
    if !extracted_asset_root().exists() {
        return;
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        mclone_net::complete_server_handshake(&mut stream).unwrap();
        assert_eq!(
            mclone_net::read_client_command_frame(&mut stream).unwrap(),
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        mclone_net::write_server_update_batch(
            &mut stream,
            &[ServerUpdate::TimeUpdate { day_time: 1 }],
        )
        .unwrap();
        assert_eq!(
            mclone_net::read_client_command_frame(&mut stream).unwrap(),
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        mclone_net::write_server_update_batch(
            &mut stream,
            &[ServerUpdate::TimeUpdate { day_time: 2 }],
        )
        .unwrap();
        assert!(
            mclone_net::try_read_client_command_frame(&mut stream)
                .unwrap()
                .is_none()
        );
    });

    {
        let scene = SceneOptions {
            render_distance: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        assert!(runtime.set_interest_center(ChunkPos::new(1, 0)).unwrap());
    }
    server.join().unwrap();
}

#[test]
fn window_runtime_reconnects_remote_session_and_resyncs_chunk_cache() {
    if !extracted_asset_root().exists() {
        return;
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let initial_center = ChunkPos::new(0, 0);
    let moved_center = ChunkPos::new(1, 0);
    let server = std::thread::spawn(move || {
        let (mut first_stream, _) = listener.accept().unwrap();
        mclone_net::complete_server_handshake(&mut first_stream).unwrap();
        assert_eq!(
            mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
            ClientCommand::SetChunkView(ChunkView {
                center: initial_center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        mclone_net::write_server_update_batch(
            &mut first_stream,
            &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                initial_center,
            ))],
        )
        .unwrap();
        assert_eq!(
            mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
            ClientCommand::SetChunkView(ChunkView {
                center: moved_center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        drop(first_stream);

        let (mut second_stream, _) = listener.accept().unwrap();
        mclone_net::complete_server_handshake(&mut second_stream).unwrap();
        assert_eq!(
            mclone_net::read_client_command_frame(&mut second_stream).unwrap(),
            ClientCommand::SetChunkView(ChunkView {
                center: moved_center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        mclone_net::write_server_update_batch(
            &mut second_stream,
            &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                moved_center,
            ))],
        )
        .unwrap();
    });

    {
        let scene = SceneOptions {
            render_distance: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        assert!(runtime.client().chunk_snapshot(initial_center).is_some());
        runtime
            .render_session_mut()
            .dirty_mut()
            .dirty_chunks
            .clear();
        runtime
            .render_session_mut()
            .dirty_mut()
            .dirty_sections
            .clear();

        assert!(runtime.set_interest_center(moved_center).unwrap());

        assert_eq!(runtime.client().loaded_chunk_count(), 1);
        assert!(runtime.client().chunk_snapshot(initial_center).is_none());
        assert!(runtime.client().chunk_snapshot(moved_center).is_some());
        assert!(!runtime.render_session().contains_chunk(initial_center));
        assert!(
            !runtime
                .render_session()
                .dirty()
                .dirty_chunks
                .contains(&initial_center)
        );
        assert!(
            runtime
                .render_session()
                .dirty()
                .dirty_chunks
                .contains(&moved_center)
        );
    }
    server.join().unwrap();
}

#[test]
fn build_scene_textured_sections_uses_client_runtime_snapshots() {
    if !extracted_asset_root().exists() {
        return;
    }
    let scene_mesh = build_scene_textured_sections(&SceneOptions {
        render_distance: 0,
        ..SceneOptions::default()
    })
    .unwrap();

    assert!(scene_mesh.section_count() > 0);
    assert!(scene_mesh.index_count() > 0);
    assert!(scene_mesh.atlas.width > 0);
    assert!(scene_mesh.atlas.height > 0);
}

fn chunk_distance_from_scene_center(scene: &SceneOptions, chunk_x: i32, chunk_z: i32) -> i32 {
    (chunk_x - scene.chunk_x)
        .abs()
        .max((chunk_z - scene.chunk_z).abs())
}

fn empty_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
    ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Full,
        ChunkRevision(1),
        0,
        16,
        &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
    )
}

fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<()> {
    runtime.scene.poll_until_idle().map(|_| ())
}

fn section_index_count(sections: &[TexturedRenderSectionMesh]) -> u32 {
    sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum()
}
