use super::*;
use crate::{ChunkTicketType, MemoryWorldStore, TopologyChunkState};
use mclone_worldgen::block::GRASS_BLOCK;

fn moon_record() -> DimensionRecord {
    DimensionRecord {
        key: DimensionKey::parse("mclone:moon").unwrap(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition: crate::DimensionDefinition::overworld(
            54_321,
            WorldGenerationProfile::authored_only(),
        ),
    }
}

fn finite_flat_record() -> DimensionRecord {
    let mut definition =
        crate::DimensionDefinition::overworld(76_543, WorldGenerationProfile::FlatGrassV1);
    definition.topology =
        HorizontalTopology::new(AxisTopology::finite(0, 2), AxisTopology::finite(0, 2));
    DimensionRecord {
        key: DimensionKey::parse("mclone:finite_canary").unwrap(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition,
    }
}

fn cylinder_flat_record() -> DimensionRecord {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology =
        HorizontalTopology::new(AxisTopology::periodic(0, 32), AxisTopology::Unbounded);
    DimensionRecord {
        key: DimensionKey::parse("mclone:cylinder_canary").unwrap(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition,
    }
}

#[test]
fn periodic_dimension_deduplicates_laps_and_opposing_seam_views() {
    let record = cylinder_flat_record();
    let cylinder = record.key.clone();
    let topology = record.definition.topology;
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(record).unwrap());

    let player = server.add_player_in_dimension(cylinder.clone()).unwrap();
    for center_x in 0..=32 {
        server
            .try_handle_command_for_player(
                player,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(center_x, 0),
                    render_distance: 2,
                    chunk_tracking_radius: 2,
                }),
            )
            .unwrap();
        let positions = server.chunk_tracking.aggregate_resident_positions();
        assert_eq!(positions.len(), 25);
        assert!(
            positions
                .iter()
                .all(|pos| topology.canonicalize_chunk(*pos) == Some(*pos))
        );
    }
    let diagnostics = server.chunk_tracking_diagnostics();
    assert_eq!(
        diagnostics.players[0]
            .accepted_view
            .as_ref()
            .unwrap()
            .center,
        ChunkPos::new(0, 0)
    );
    assert_eq!(
        server.scheduler().world_generation_descriptor().topology,
        topology
    );
    assert_eq!(
        server
            .scheduler()
            .topology_chunk_state(ChunkPos::new(-1, 0)),
        TopologyChunkState::Loaded {
            canonical: ChunkPos::new(31, 0),
        }
    );
    let seam_ticket_count = server.scheduler().ticket_count_at(ChunkPos::new(31, 0));
    server
        .scheduler_mut()
        .set_chunk_forced(ChunkPos::new(-1, 0), true)
        .unwrap();
    assert_eq!(
        server.scheduler().ticket_count_at(ChunkPos::new(-1, 0)),
        seam_ticket_count + 1
    );

    let observer = server
        .add_observer(
            cylinder,
            ChunkView {
                center: ChunkPos::new(31, 0),
                render_distance: 2,
                chunk_tracking_radius: 2,
            },
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    let diagnostics = server.chunk_tracking_diagnostics();
    assert_eq!(diagnostics.aggregate_player_ticket_chunks, 25);
    assert_eq!(diagnostics.aggregate_resident_chunks, 30);
    assert_eq!(diagnostics.total_player_visible_chunks, 25);
    assert_eq!(diagnostics.total_observer_visible_chunks, 25);
    assert_eq!(
        diagnostics.observers[0]
            .accepted_view
            .as_ref()
            .unwrap()
            .center,
        ChunkPos::new(31, 0)
    );

    server.remove_observer(observer).unwrap();
}

#[test]
fn periodic_player_crossing_retains_canonical_authority_position() {
    let record = cylinder_flat_record();
    let cylinder = record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(record).unwrap());
    let player = server.add_player_in_dimension(cylinder).unwrap();

    for position in [511.75, 512.25, 1_024.5, -0.25] {
        server
            .try_handle_command_for_player(
                player,
                ClientCommand::move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(position, 64.0, 2.0),
                    on_ground: true,
                }),
            )
            .unwrap();
        let canonical = position.rem_euclid(512.0);
        assert_eq!(
            server.player_position(player),
            Some(Vec3d::new(canonical, 64.0, 2.0))
        );
    }
}

#[test]
fn periodic_crossing_publishes_one_canonical_remote_player_to_opposing_views() {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology = HorizontalTopology::cylinder_x(0, 32);
    let mut server = LocalRealmSession::with_player_chunk_tracking_policy_and_dimension_definition(
        definition,
        PlayerChunkTrackingPolicy::default(),
    );
    server.set_lighting_enabled(false);
    let local = server.player_id();
    load_chunk_view(&mut server, ChunkPos::new(31, 0));
    send_player_move(&mut server, Vec3d::new(511.75, 80.0, 8.5));

    let opposing = server.add_player();
    let initial = set_dedicated_chunk_view_and_poll(&mut server, opposing, ChunkPos::new(0, 0), 1);
    let add = remote_player_add(&initial, local)
        .expect("opposing seam observer should receive the canonical player");
    assert_eq!(add.position, Vec3d::new(511.75, 80.0, 8.5));

    server
        .try_handle_command_for_player(
            opposing,
            ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(0.5, 80.0, 8.5),
                on_ground: true,
            }),
        )
        .unwrap();
    let opposite_block = BlockPos::new(511, 80, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(opposite_block, STONE)
    );
    server.scheduler_mut().drain_pending_block_delta_events();
    server
        .try_handle_command_for_player(
            opposing,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos: opposite_block,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .unwrap();
    assert_eq!(server.scheduler().block_at_world(opposite_block), Some(AIR));

    send_player_move(&mut server, Vec3d::new(512.25, 80.0, 8.5));
    let updates = server
        .try_drain_updates_for_player(opposing)
        .expect("drain seam crossing for opposing observer");
    let moved = remote_player_update(&updates, local)
        .expect("opposing seam observer should receive the crossing");

    assert_eq!(moved.position, Vec3d::new(0.25, 80.0, 8.5));
    assert_eq!(server.player_position(local), Some(moved.position));
}

#[test]
fn periodic_showcase_actor_tracks_one_identity_through_both_crossings() {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology = HorizontalTopology::cylinder_x(0, 32);
    let topology = definition.topology;
    let mut server = LocalRealmSession::with_player_chunk_tracking_policy_and_dimension_definition(
        definition,
        PlayerChunkTrackingPolicy::default(),
    );
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(true);
    let mut updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 1,
            chunk_tracking_radius: 1,
        }))
        .unwrap();
    accept_player_position_updates(&mut server, &updates);
    for _ in 0..60_000 {
        let polled = server.try_poll().unwrap();
        accept_player_position_updates(&mut server, &polled);
        updates.extend(polled);
        if server.pending_job_count() == 0 {
            break;
        }
        if server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    for _ in 0..2 {
        updates.extend(server.try_simulation_tick_report().unwrap().updates);
    }
    server
        .entities
        .ensure_debug_passive_showcase_near_spawn(Vec3d::new(510.0, 4.0, 8.0), true);
    let actor = server
        .entities
        .states()
        .into_iter()
        .find(|entity| entity.kind == EntityKind::Cow)
        .expect("cylinder showcase actor should exist at the seam");
    let mut positions = vec![actor.position];

    for _ in 0..161 {
        let report = server.try_simulation_tick_report().unwrap();
        positions.extend(report.updates.iter().filter_map(|update| match update {
            ServerUpdate::EntityUpdate(update) if update.id == actor.id => Some(update.position),
            _ => None,
        }));
    }
    let chunks = positions
        .iter()
        .map(|position| BlockPos::containing(*position).chunk_pos().x)
        .collect::<Vec<_>>();

    assert!(positions.iter().all(|position| {
        topology
            .canonicalize_position(*position)
            .is_some_and(|canonical| canonical == *position)
    }));
    assert!(
        positions
            .iter()
            .all(|position| position.x <= 2.0 || position.x >= 510.0)
    );
    assert!(chunks.windows(2).any(|pair| pair == [31, 0]));
    assert!(chunks.windows(2).any(|pair| pair == [0, 31]));
    assert!(server.entities.state(actor.id).is_some());
}

#[test]
fn periodic_seam_break_and_place_share_one_canonical_block() {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology = HorizontalTopology::cylinder_x(0, 32);
    let mut server = LocalRealmSession::with_player_chunk_tracking_policy_and_dimension_definition(
        definition,
        PlayerChunkTrackingPolicy::default(),
    );
    load_chunk_view(&mut server, ChunkPos::new(0, 0));
    send_player_move(&mut server, Vec3d::new(511.5, 80.0, 8.5));

    let canonical_base = BlockPos::new(0, 79, 8);
    let canonical_target = canonical_base.relative(Direction::Up);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(canonical_base, STONE)
    );
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(canonical_target, STONE)
    );
    server.scheduler_mut().drain_pending_block_delta_events();

    server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: BlockPos::new(512, 80, 8),
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .unwrap();
    assert_eq!(
        server.scheduler().block_at_world(canonical_target),
        Some(AIR)
    );

    assign_debug_hotbar_slot(&mut server, 0, generated_block_state_id(DIRT));
    sync_carried_slot(&mut server, 0);
    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(512.5, 80.0, 8.5),
            Direction::Up,
            BlockPos::new(512, 79, 8),
            false,
        )))
        .unwrap();

    assert_eq!(
        server.scheduler().block_at_world(canonical_target),
        Some(DIRT)
    );
    assert_eq!(
        server.scheduler().block_at_world(BlockPos::new(512, 80, 8)),
        Some(DIRT)
    );
}

#[test]
fn periodic_fluid_tick_spreads_into_the_canonical_seam_neighbor() {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology = HorizontalTopology::cylinder_x(0, 32);
    let mut server = LocalRealmSession::with_player_chunk_tracking_policy_and_dimension_definition(
        definition,
        PlayerChunkTrackingPolicy::default(),
    );
    server.set_lighting_enabled(false);
    let updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 1,
            chunk_tracking_radius: 1,
        }))
        .unwrap();
    accept_player_position_updates(&mut server, &updates);
    for _ in 0..60_000 {
        let updates = server.try_poll().unwrap();
        accept_player_position_updates(&mut server, &updates);
        if server.pending_job_count() == 0 {
            break;
        }
        if server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }

    let source = BlockPos::new(0, 80, 8);
    let seam_neighbor = BlockPos::new(511, 80, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(source.below(), STONE)
    );
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(seam_neighbor.below(), STONE)
    );
    assert!(server.scheduler_mut().set_block_at_world(source, WATER));
    for wall in [
        BlockPos::new(1, 80, 8),
        BlockPos::new(0, 80, 7),
        BlockPos::new(0, 80, 9),
    ] {
        assert!(server.scheduler_mut().set_block_at_world(wall, STONE));
    }
    if server.scheduler().block_at_world(seam_neighbor) != Some(AIR) {
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(seam_neighbor, AIR)
        );
    }
    server.liquid_ticks = FluidTickList::new();
    server.schedule_fluid_tick(source, FluidKind::Water, 0);

    let report = server.try_simulation_tick_report().unwrap();
    assert_eq!(report.fluid_ticks_executed, 1, "{report:?}");

    assert_eq!(
        server
            .scheduler()
            .block_at_world(seam_neighbor)
            .and_then(FluidKind::from_block_id),
        Some(FluidKind::Water)
    );
}

#[test]
fn periodic_runtime_block_light_recomputes_the_opposite_seam_chunk() {
    let mut definition =
        crate::DimensionDefinition::overworld(98_765, WorldGenerationProfile::FlatGrassV1);
    definition.topology = HorizontalTopology::cylinder_x(0, 32);
    let mut server = LocalRealmSession::with_player_chunk_tracking_policy_and_dimension_definition(
        definition,
        PlayerChunkTrackingPolicy::default(),
    );
    let updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 1,
            chunk_tracking_radius: 1,
        }))
        .unwrap();
    accept_player_position_updates(&mut server, &updates);
    for _ in 0..60_000 {
        let updates = server.try_poll().unwrap();
        accept_player_position_updates(&mut server, &updates);
        if server.pending_job_count() == 0 {
            break;
        }
        if server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
            server.wait_for_light_completion(Duration::from_secs(1));
        }
    }
    assert_eq!(server.pending_job_count(), 0, "cylinder light view settled");

    // Keep the canary in the terrain's non-empty surface section. The current
    // light engine intentionally omits storage for wholly empty sections.
    let source = BlockPos::new(0, 4, 8);
    let seam_neighbor = BlockPos::new(511, 4, 8);
    assert!(server.set_block_from_simulation(source, TORCH));

    assert_eq!(
        server.scheduler().raw_brightness_at_world(source, 15),
        Some(14)
    );
    assert_eq!(
        server
            .scheduler()
            .raw_brightness_at_world(seam_neighbor, 15),
        Some(13)
    );

    assert!(server.set_block_from_simulation(source, AIR));
    assert_eq!(
        server
            .scheduler()
            .raw_brightness_at_world(seam_neighbor, 15),
        Some(0)
    );
}

#[test]
fn finite_dimension_rejects_authority_outside_its_bounds() {
    let record = finite_flat_record();
    let finite = record.key.clone();
    let topology = record.definition.topology;
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(record).unwrap());

    let player = server.add_player_in_dimension(finite.clone()).unwrap();
    assert_eq!(
        server.scheduler().topology_chunk_state(ChunkPos::new(1, 1)),
        TopologyChunkState::ValidUnloaded {
            canonical: ChunkPos::new(1, 1),
        }
    );
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 2,
                chunk_tracking_radius: 2,
            }),
        )
        .unwrap();
    assert_eq!(
        server
            .chunk_tracking
            .diagnostics()
            .total_player_visible_chunks,
        4
    );
    let readiness = server.view_readiness_snapshot(player).unwrap();
    assert_eq!(readiness.stats.target_chunk_count, 4);
    assert_eq!(readiness.stats.playable_gate_chunk_count, 4);
    assert!(
        server
            .chunk_tracking
            .aggregate_resident_positions()
            .iter()
            .all(|pos| topology.canonicalize_chunk(*pos) == Some(*pos))
    );

    let outside_view = server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(-1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .unwrap_err();
    assert!(outside_view.to_string().contains("outside"));

    move_player(&mut server, player, Vec3d::new(1.5, 80.0, 1.5));
    let correction = server
        .try_handle_command_for_player(
            player,
            ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(-0.1, 80.0, 1.5),
                on_ground: true,
            }),
        )
        .unwrap();
    assert!(matches!(
        correction.as_slice(),
        [ServerUpdate::PlayerPosition(_)]
    ));
    assert_eq!(
        server.player_position(player),
        Some(Vec3d::new(1.5, 80.0, 1.5))
    );

    assert!(
        !server
            .scheduler_mut()
            .set_block_at_world(BlockPos::new(-1, 64, 0), DIRT)
    );
    assert!(
        server
            .scheduler_mut()
            .add_region_ticket(ChunkTicketType::Forced, ChunkPos::new(-1, 0), 0)
            .unwrap_err()
            .to_string()
            .contains("outside")
    );
    assert!(!server.try_schedule_fluid_tick(BlockPos::new(-1, 64, 0), FluidKind::Water, 0,));
    assert_eq!(
        server
            .scheduler()
            .topology_chunk_state(ChunkPos::new(-1, 0)),
        TopologyChunkState::OutsideTopology
    );
    assert_eq!(
        server.scheduler().topology_chunk_state(ChunkPos::new(1, 1)),
        TopologyChunkState::Loaded {
            canonical: ChunkPos::new(1, 1),
        }
    );
    assert_eq!(
        server.scheduler().topology_chunk_state(ChunkPos::new(0, 3)),
        TopologyChunkState::OutsideTopology
    );
    assert!(
        server
            .scheduler_mut()
            .set_chunk_forced(ChunkPos::new(-1, 0), true)
            .unwrap_err()
            .to_string()
            .contains("outside")
    );

    let overworld_player = server.add_player();
    move_player(&mut server, overworld_player, Vec3d::new(0.5, 80.0, 0.5));
    assert!(
        server
            .transfer_player_dimension(overworld_player, finite, Vec3d::new(32.0, 80.0, 1.5),)
            .unwrap_err()
            .to_string()
            .contains("outside topology")
    );
    assert_eq!(
        server.player_dimension(overworld_player),
        Some(&DimensionKey::overworld())
    );
}

#[test]
fn finite_dimension_rejects_unsupported_generator_profiles() {
    let mut record = finite_flat_record();
    record.definition.generation_profile = WorldGenerationProfile::Overworld;
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));

    let error = server.register_dimension(record).unwrap_err();

    assert!(error.to_string().contains("does not support"));
}

#[test]
fn finite_dimension_rejects_an_outside_initial_spawn_before_persistence() {
    let mut record = finite_flat_record();
    record.definition.topology =
        HorizontalTopology::new(AxisTopology::finite(5, 7), AxisTopology::finite(5, 7));
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));

    let error = server.register_dimension(record).unwrap_err();

    assert!(error.to_string().contains("initial spawn chunk"));
    assert!(error.to_string().contains("outside"));
}

fn request_zero_radius_view(server: &mut RealmServer, player: ServerPlayerId) {
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .unwrap();
}

fn wait_for_origin_in_both_dimensions(server: &mut RealmServer, moon: &DimensionKey) {
    for _ in 0..60_000 {
        server.try_tick_report_global().unwrap();
        let overworld_ready = server
            .dimension_runtime(&DimensionKey::overworld())
            .unwrap()
            .scheduler()
            .client_visible_snapshot(ChunkPos::new(0, 0))
            .is_some();
        let moon_ready = server
            .dimension_runtime(moon)
            .unwrap()
            .scheduler()
            .client_visible_snapshot(ChunkPos::new(0, 0))
            .is_some();
        if overworld_ready && moon_ready {
            return;
        }
        for key in [DimensionKey::overworld(), moon.clone()] {
            server.activate_dimension(&key).unwrap();
            if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
    }
    panic!("timed out loading equal origin chunks in both dimensions");
}

fn move_player(server: &mut RealmServer, player: ServerPlayerId, position: Vec3d) {
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
                position,
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .unwrap();
}

fn snapshot_block(record: ChunkRecord, pos: BlockPos) -> u8 {
    crate::mutable_buffer_from_snapshot(&record.snapshot).get_block_at_y(
        local_block_coord(pos.x),
        pos.y,
        local_block_coord(pos.z),
    )
}

fn accept_initial_position(server: &mut RealmServer, player: ServerPlayerId) {
    let updates = server.try_drain_updates_for_player(player).unwrap();
    let teleport_id = updates.iter().find_map(|update| match update {
        ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
        _ => None,
    });
    if let Some(id) = teleport_id {
        server
            .try_handle_command_for_player(
                player,
                ClientCommand::AcceptTeleport(AcceptTeleportCommand { id }),
            )
            .unwrap();
    }
}

#[test]
fn concurrent_dimension_runtimes_route_players_ticks_and_mutations() {
    let dimension_record = moon_record();
    let moon = dimension_record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(dimension_record).unwrap());

    let overworld_player = server.add_player();
    let moon_player = server.add_player_in_dimension(moon.clone()).unwrap();
    assert_eq!(
        server.player_dimension(overworld_player),
        Some(&DimensionKey::overworld())
    );
    assert_eq!(server.player_dimension(moon_player), Some(&moon));

    request_zero_radius_view(&mut server, overworld_player);
    request_zero_radius_view(&mut server, moon_player);
    wait_for_origin_in_both_dimensions(&mut server, &moon);
    accept_initial_position(&mut server, overworld_player);
    accept_initial_position(&mut server, moon_player);

    let overworld_tick_before = server
        .dimension_runtime(&DimensionKey::overworld())
        .unwrap()
        .scheduler()
        .ticket_tick();
    let moon_tick_before = server
        .dimension_runtime(&moon)
        .unwrap()
        .scheduler()
        .ticket_tick();
    let realm_tick_before = server.simulation_tick();
    server.try_simulation_tick_report_global().unwrap();
    assert_eq!(server.simulation_tick(), realm_tick_before + 1);
    assert_eq!(
        server
            .dimension_runtime(&DimensionKey::overworld())
            .unwrap()
            .scheduler()
            .ticket_tick(),
        overworld_tick_before + 1
    );
    assert_eq!(
        server
            .dimension_runtime(&moon)
            .unwrap()
            .scheduler()
            .ticket_tick(),
        moon_tick_before + 1
    );

    let position = Vec3d::new(8.5, 80.0, 8.5);
    let block = BlockPos::new(8, 80, 8);
    move_player(&mut server, overworld_player, position);
    assert!(server.scheduler_mut().set_block_at_world(block, DIRT));
    server.scheduler_mut().drain_pending_block_delta_events();
    move_player(&mut server, moon_player, Vec3d::new(9.5, 80.0, 8.5));
    assert!(server.scheduler_mut().set_block_at_world(block, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let overworld_updates = server
        .try_handle_command_for_player(
            overworld_player,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos: block,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .unwrap();
    assert!(has_section_block_updates(&overworld_updates));
    assert_eq!(server.scheduler().block_at_world(block), Some(AIR));

    server.activate_player_dimension(moon_player).unwrap();
    assert_eq!(server.scheduler().block_at_world(block), Some(STONE));
    let moon_updates = server.try_drain_updates_for_player(moon_player).unwrap();
    assert!(!has_section_block_updates(&moon_updates));
    assert!(moon_updates.iter().all(|update| !matches!(
        update,
        ServerUpdate::RemotePlayerAdd(_)
            | ServerUpdate::RemotePlayerUpdate(_)
            | ServerUpdate::RemotePlayerRemove { .. }
    )));
    assert_eq!(server.player_position(overworld_player), Some(position));
    assert_eq!(
        server.player_position(moon_player),
        Some(Vec3d::new(9.5, 80.0, 8.5))
    );

    assert!(!server.unload_dimension(&moon).unwrap());
    assert!(server.remove_player(moon_player));
    assert!(server.unload_dimension(&moon).unwrap());
    assert!(!server.loaded_dimension_keys().contains(&moon));
    assert!(server.register_dimension(moon_record()).unwrap());
    assert!(server.loaded_dimension_keys().contains(&moon));
}

#[test]
fn dimensions_generate_with_independent_stored_profiles_and_seeds() {
    let island = DimensionKey::parse("mclone:small_island").unwrap();
    let island_record = DimensionRecord {
        key: island.clone(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition: crate::DimensionDefinition::overworld(
            -98_765,
            WorldGenerationProfile::SmallIslandV1,
        ),
    };
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
        .unwrap();
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(island_record).unwrap());

    let mclone_player = server.add_player();
    let island_player = server.add_player_in_dimension(island.clone()).unwrap();
    request_zero_radius_view(&mut server, mclone_player);
    request_zero_radius_view(&mut server, island_player);
    wait_for_origin_in_both_dimensions(&mut server, &island);

    let generated_mclone = mclone_worldgen::levelgen::generate_mclone_overworld_chunk(12_345, 0, 0);
    let sample_x = 8;
    let sample_z = 8;
    let sample_y = (generated_mclone.min_y..generated_mclone.min_y + generated_mclone.height)
        .rev()
        .find(|y| {
            generated_mclone.block_at_y(sample_x, *y, sample_z).0 != mclone_worldgen::block::AIR
        })
        .expect("Mclone origin column should contain terrain");
    let generated_overworld = server
        .dimension_runtime(&DimensionKey::overworld())
        .unwrap();
    assert_eq!(
        generated_overworld.definition().generation_profile,
        WorldGenerationProfile::McloneOverworldV1
    );
    assert_eq!(generated_overworld.definition().seed, 12_345);
    assert_eq!(
        generated_overworld
            .scheduler()
            .block_at_world(BlockPos::new(sample_x, sample_y, sample_z)),
        Some(generated_mclone.block_at_y(sample_x, sample_y, sample_z).0)
    );

    let generated_island = server.dimension_runtime(&island).unwrap();
    assert_eq!(
        generated_island.definition().generation_profile,
        WorldGenerationProfile::SmallIslandV1
    );
    assert_eq!(generated_island.definition().seed, -98_765);
    assert_eq!(
        generated_island
            .scheduler()
            .block_at_world(BlockPos::new(0, 80, 0)),
        Some(GRASS_BLOCK)
    );
    assert_eq!(
        generated_island
            .scheduler()
            .block_at_world(BlockPos::new(0, 81, 0)),
        Some(AIR)
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dimension_membership_and_runtime_chunks_resume_from_one_realm_store() {
    let root = std::env::temp_dir().join(format!(
        "mclone-realm-dimension-runtime-{}-{}",
        std::process::id(),
        current_unix_millis()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let moon_record = moon_record();
    let moon = moon_record.key.clone();
    let overworld_identity =
        ClientIdentity::new(PlayerProfileId::new([0x31; 16]), "Overworld Player").unwrap();
    let moon_identity =
        ClientIdentity::new(PlayerProfileId::new([0x42; 16]), "Moon Player").unwrap();
    let block = BlockPos::new(8, 80, 8);

    {
        let mut server = RealmServer::try_with_threaded_sqlite_world_dir(12_345, &root).unwrap();
        server.initialize_world_metadata_blocking().unwrap();
        server
            .set_world_generation_profile(WorldGenerationProfile::authored_only())
            .unwrap();
        server.set_lighting_enabled(false);
        server.register_dimension(moon_record.clone()).unwrap();
        let overworld_player = server
            .add_player_with_identity(overworld_identity.clone())
            .unwrap();
        let moon_player = server.add_player_in_dimension(moon.clone()).unwrap();
        server
            .configure_player_identity_blocking(moon_player, moon_identity.clone())
            .unwrap();
        request_zero_radius_view(&mut server, overworld_player);
        request_zero_radius_view(&mut server, moon_player);
        wait_for_origin_in_both_dimensions(&mut server, &moon);
        accept_initial_position(&mut server, overworld_player);
        accept_initial_position(&mut server, moon_player);

        move_player(&mut server, overworld_player, Vec3d::new(8.5, 80.0, 8.5));
        assert!(server.scheduler_mut().set_block_at_world(block, DIRT));
        move_player(&mut server, moon_player, Vec3d::new(24.5, 90.0, 24.5));
        assert!(server.scheduler_mut().set_block_at_world(block, STONE));
        server.save_dirty_chunks().unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let mut store = crate::SqliteWorldStore::open_world_dir(&root).unwrap();
        assert_eq!(
            snapshot_block(
                store
                    .load_chunk(&DimensionKey::overworld(), ChunkPos::new(0, 0))
                    .unwrap()
                    .unwrap(),
                block,
            ),
            DIRT
        );
        assert_eq!(
            snapshot_block(
                store
                    .load_chunk(&moon, ChunkPos::new(0, 0))
                    .unwrap()
                    .unwrap(),
                block,
            ),
            STONE
        );
    }

    {
        let mut server = RealmServer::try_with_threaded_sqlite_world_dir(12_345, &root).unwrap();
        server.initialize_world_metadata_blocking().unwrap();
        server.register_dimension(moon_record).unwrap();
        let overworld_player = server.add_player_with_identity(overworld_identity).unwrap();
        let moon_player = server.add_player_with_identity(moon_identity).unwrap();
        assert_eq!(
            server.player_dimension(overworld_player),
            Some(&DimensionKey::overworld())
        );
        assert_eq!(server.player_dimension(moon_player), Some(&moon));
        assert_eq!(
            server
                .players
                .get(overworld_player)
                .unwrap()
                .resume_record
                .as_ref()
                .unwrap()
                .position,
            Vec3d::new(8.5, 80.0, 8.5)
        );
        assert_eq!(
            server
                .players
                .get(moon_player)
                .unwrap()
                .resume_record
                .as_ref()
                .unwrap()
                .position,
            Vec3d::new(24.5, 90.0, 24.5)
        );
        server.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}
