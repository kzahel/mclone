use super::support::*;

fn assert_liquid_oracle_fixture_matches(server: &LocalRealmSession, fixture_json: &str) {
    let fixture = serde_json::from_str::<Value>(fixture_json).unwrap();
    let bounds = &fixture["bounds"];
    let min_x = fixture_i32(bounds, "minX");
    let min_y = fixture_i32(bounds, "minY");
    let min_z = fixture_i32(bounds, "minZ");
    let size_x = fixture_i32(bounds, "sizeX");
    let size_y = fixture_i32(bounds, "sizeY");
    let size_z = fixture_i32(bounds, "sizeZ");
    let palette = fixture["palette"]
        .as_array()
        .expect("liquid fixture palette must be an array")
        .iter()
        .map(fixture_palette_block_id)
        .collect::<Vec<_>>();
    let blocks = fixture["blocks"]
        .as_array()
        .expect("liquid fixture blocks must be an array");
    assert_eq!(
        blocks.len(),
        (size_x * size_y * size_z) as usize,
        "liquid fixture block volume should match bounds"
    );

    for (index, block) in blocks.iter().enumerate() {
        let index = index as i32;
        let x = index % size_x;
        let z = (index / size_x) % size_z;
        let y = index / (size_x * size_z);
        let palette_index = block
            .as_u64()
            .expect("liquid fixture block entry must be a palette index")
            as usize;
        let expected = palette[palette_index];
        let pos = WorldBlockPos::new(min_x + x, min_y + y, min_z + z);
        assert_eq!(
            server.scheduler().block_at_world(pos),
            Some(expected),
            "liquid fixture block mismatch at {pos:?}"
        );
    }

    let mut expected_ticks = fixture["liquidTicks"]
        .as_array()
        .expect("liquid fixture liquidTicks must be an array")
        .iter()
        .map(|tick| {
            (
                WorldBlockPos::new(
                    fixture_i32(tick, "x"),
                    fixture_i32(tick, "y"),
                    fixture_i32(tick, "z"),
                ),
                FluidKind::from_target(
                    tick["target"]
                        .as_str()
                        .expect("liquid tick target must be a string"),
                )
                .expect("liquid tick target must be a known fluid"),
                fixture_i32(tick, "delay"),
            )
        })
        .collect::<Vec<_>>();
    expected_ticks.sort();
    assert_eq!(
        server
            .liquid_ticks
            .scheduled_tick_entries(server.simulation_tick()),
        expected_ticks
    );
}

fn new_liquid_oracle_server() -> LocalRealmSession {
    new_liquid_oracle_server_with_radius(0)
}

fn new_liquid_oracle_server_with_radius(radius_chunks: u32) -> LocalRealmSession {
    let mut server = LocalRealmSession::new(12_345);
    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: radius_chunks,
            chunk_tracking_radius: radius_chunks,
        }),
    );
    server.liquid_ticks = FluidTickList::new();
    server
}

fn fill_blocks(
    server: &mut LocalRealmSession,
    min: WorldBlockPos,
    max: WorldBlockPos,
    block: RawBlockId,
) {
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                server
                    .scheduler_mut()
                    .set_block_at_world(WorldBlockPos::new(x, y, z), block);
            }
        }
    }
}

fn run_simulation_ticks(server: &mut LocalRealmSession, ticks: usize) {
    for _ in 0..ticks {
        server.simulation_tick_report();
    }
}

fn setup_liquid_slope(server: &mut LocalRealmSession, source_block: RawBlockId, fluid: FluidKind) {
    fill_blocks(
        server,
        WorldBlockPos::new(0, 78, 0),
        WorldBlockPos::new(12, 84, 4),
        AIR,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 79, 0),
        WorldBlockPos::new(12, 79, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 1),
        WorldBlockPos::new(12, 81, 1),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 3),
        WorldBlockPos::new(12, 81, 3),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 2),
        WorldBlockPos::new(0, 81, 2),
        STONE,
    );

    let source = WorldBlockPos::new(1, 80, 2);
    server
        .scheduler_mut()
        .set_block_at_world(source, source_block);
    server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
}

fn setup_water_slope(server: &mut LocalRealmSession) {
    setup_liquid_slope(server, WATER, FluidKind::Water);
}

fn setup_lava_slope(server: &mut LocalRealmSession) {
    setup_liquid_slope(server, LAVA, FluidKind::Lava);
}

fn setup_cross_chunk_water_slope(server: &mut LocalRealmSession) {
    fill_blocks(
        server,
        WorldBlockPos::new(14, 78, 0),
        WorldBlockPos::new(18, 84, 4),
        AIR,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(14, 79, 0),
        WorldBlockPos::new(18, 79, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(14, 80, 1),
        WorldBlockPos::new(18, 81, 1),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(14, 80, 3),
        WorldBlockPos::new(18, 81, 3),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(14, 80, 2),
        WorldBlockPos::new(14, 81, 2),
        STONE,
    );

    let source = WorldBlockPos::new(15, 80, 2);
    server.scheduler_mut().set_block_at_world(source, WATER);
    server.schedule_fluid_tick(source, FluidKind::Water, FluidKind::Water.tick_delay());
}

fn setup_liquid_fall(server: &mut LocalRealmSession, source_block: RawBlockId, fluid: FluidKind) {
    fill_blocks(
        server,
        WorldBlockPos::new(0, 78, 0),
        WorldBlockPos::new(4, 88, 4),
        AIR,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 79, 0),
        WorldBlockPos::new(4, 79, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 0),
        WorldBlockPos::new(0, 88, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(4, 80, 0),
        WorldBlockPos::new(4, 88, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 0),
        WorldBlockPos::new(4, 88, 0),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 4),
        WorldBlockPos::new(4, 88, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(1, 80, 1),
        WorldBlockPos::new(3, 87, 1),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(1, 80, 3),
        WorldBlockPos::new(3, 87, 3),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(1, 80, 2),
        WorldBlockPos::new(1, 87, 2),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(3, 80, 2),
        WorldBlockPos::new(3, 87, 2),
        STONE,
    );

    let source = WorldBlockPos::new(2, 86, 2);
    server
        .scheduler_mut()
        .set_block_at_world(source, source_block);
    server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
}

fn setup_water_fall(server: &mut LocalRealmSession) {
    setup_liquid_fall(server, WATER, FluidKind::Water);
}

fn setup_lava_fall(server: &mut LocalRealmSession) {
    setup_liquid_fall(server, LAVA, FluidKind::Lava);
}

fn setup_water_source_conversion(server: &mut LocalRealmSession) {
    fill_blocks(
        server,
        WorldBlockPos::new(0, 78, 0),
        WorldBlockPos::new(4, 84, 4),
        AIR,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 79, 0),
        WorldBlockPos::new(4, 79, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 1),
        WorldBlockPos::new(4, 81, 1),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 3),
        WorldBlockPos::new(4, 81, 3),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 2),
        WorldBlockPos::new(0, 81, 2),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(4, 80, 2),
        WorldBlockPos::new(4, 81, 2),
        STONE,
    );

    for source in [WorldBlockPos::new(1, 80, 2), WorldBlockPos::new(3, 80, 2)] {
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.schedule_fluid_tick(source, FluidKind::Water, 5);
    }
}

fn setup_lava_source_water_contact(server: &mut LocalRealmSession) {
    fill_blocks(
        server,
        WorldBlockPos::new(0, 78, 0),
        WorldBlockPos::new(4, 84, 4),
        AIR,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 79, 0),
        WorldBlockPos::new(4, 79, 4),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 1),
        WorldBlockPos::new(4, 81, 1),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 3),
        WorldBlockPos::new(4, 81, 3),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(0, 80, 2),
        WorldBlockPos::new(0, 81, 2),
        STONE,
    );
    fill_blocks(
        server,
        WorldBlockPos::new(4, 80, 2),
        WorldBlockPos::new(4, 81, 2),
        STONE,
    );

    let lava = WorldBlockPos::new(1, 80, 2);
    let water = WorldBlockPos::new(2, 80, 2);
    server.scheduler_mut().set_block_at_world(lava, LAVA);
    server.schedule_fluid_tick(lava, FluidKind::Lava, FluidKind::Lava.tick_delay());
    server.scheduler_mut().set_block_at_world(water, WATER);
    server.schedule_fluid_tick(water, FluidKind::Water, FluidKind::Water.tick_delay());
    assert!(
        server.scheduler_mut().resolve_lava_source_contact_at(lava),
        "water neighbor should convert lava source to obsidian"
    );
}

fn fixture_palette_block_id(entry: &Value) -> RawBlockId {
    let name = entry["name"]
        .as_str()
        .expect("fixture palette entry name must be a string");
    match name {
        "minecraft:air" => AIR,
        "minecraft:stone" => STONE,
        "minecraft:obsidian" => OBSIDIAN,
        "minecraft:water" => {
            let level = entry
                .get("properties")
                .and_then(|properties| properties.get("level"))
                .and_then(Value::as_str)
                .unwrap_or("0")
                .parse::<u8>()
                .expect("fixture water level must be a u8");
            water_block_for_level(level).expect("fixture water level must be supported")
        }
        "minecraft:lava" => {
            let level = entry
                .get("properties")
                .and_then(|properties| properties.get("level"))
                .and_then(Value::as_str)
                .unwrap_or("0")
                .parse::<u8>()
                .expect("fixture lava level must be a u8");
            lava_block_for_level(level).expect("fixture lava level must be supported")
        }
        _ => panic!("unsupported liquid fixture palette entry `{name}`"),
    }
}

#[test]
fn fluid_tick_list_dedupes_position_and_fluid() {
    let mut ticks = FluidTickList::new();
    let pos = WorldBlockPos::new(8, 120, 8);

    ticks.schedule_tick(pos, FluidKind::Water, 5, 10);
    ticks.schedule_tick(pos, FluidKind::Water, 1, 10);
    ticks.schedule_tick(pos, FluidKind::Lava, 1, 10);

    assert_eq!(ticks.size(), 2);
    assert!(ticks.has_scheduled_tick(pos, FluidKind::Water));
    assert!(ticks.has_scheduled_tick(pos, FluidKind::Lava));
}

#[test]
fn fluid_tick_list_packs_and_removes_chunk_ticks() {
    let mut ticks = FluidTickList::new();
    let water = WorldBlockPos::new(8, 120, 8);
    let lava = WorldBlockPos::new(9, 121, 8);
    let other_chunk = WorldBlockPos::new(24, 120, 8);

    ticks.schedule_tick(water, FluidKind::Water, 5, 10);
    ticks.schedule_tick(lava, FluidKind::Lava, 1, 12);
    ticks.schedule_tick(other_chunk, FluidKind::Water, 3, 10);

    assert_eq!(
        ticks.scheduled_chunk_tick_records(ChunkPos::new(0, 0), 12),
        vec![
            ScheduledTickRecord::new(lava, "minecraft:lava", 1),
            ScheduledTickRecord::new(water, "minecraft:water", 3),
        ]
    );

    assert_eq!(ticks.remove_chunk_ticks(ChunkPos::new(0, 0)), 2);
    assert!(!ticks.has_scheduled_tick(water, FluidKind::Water));
    assert!(!ticks.has_scheduled_tick(lava, FluidKind::Lava));
    assert!(ticks.has_scheduled_tick(other_chunk, FluidKind::Water));
    assert_eq!(ticks.size(), 1);
}

#[test]
fn block_tick_list_packs_and_removes_chunk_ticks() {
    let mut ticks = crate::falling_block::BlockTickList::new();
    let sand = WorldBlockPos::new(8, 120, 8);
    let gravel = WorldBlockPos::new(9, 121, 8);
    let other_chunk = WorldBlockPos::new(24, 120, 8);

    ticks.schedule_tick(sand, "minecraft:sand", 5, 10);
    ticks.schedule_tick(sand, "minecraft:gravel", 1, 10);
    ticks.schedule_tick(gravel, "minecraft:gravel", 1, 12);
    ticks.schedule_tick(other_chunk, "minecraft:red_sand", 3, 10);

    assert_eq!(
        ticks.scheduled_tick_entries(12),
        vec![
            (gravel, "minecraft:gravel".to_owned(), 1),
            (other_chunk, "minecraft:red_sand".to_owned(), 1),
            (sand, "minecraft:sand".to_owned(), 3),
        ]
    );
    assert_eq!(
        ticks.scheduled_chunk_tick_records(ChunkPos::new(0, 0), 12),
        vec![
            ScheduledTickRecord::new(gravel, "minecraft:gravel", 1),
            ScheduledTickRecord::new(sand, "minecraft:sand", 3),
        ]
    );

    assert_eq!(ticks.remove_chunk_ticks(ChunkPos::new(0, 0)), 2);
    assert!(!ticks.has_scheduled_tick(sand));
    assert!(!ticks.has_scheduled_tick(gravel));
    assert!(ticks.has_scheduled_tick(other_chunk));
    assert_eq!(ticks.size(), 1);
}

#[test]
fn fluid_kind_accepts_generated_and_persisted_tick_targets() {
    assert_eq!(
        FluidKind::from_target("minecraft:water"),
        Some(FluidKind::Water)
    );
    assert_eq!(
        FluidKind::from_target("minecraft:flowing_water"),
        Some(FluidKind::Water)
    );
    assert_eq!(
        FluidKind::from_target("minecraft:lava"),
        Some(FluidKind::Lava)
    );
    assert_eq!(
        FluidKind::from_target("minecraft:flowing_lava"),
        Some(FluidKind::Lava)
    );
    assert_eq!(FluidKind::from_target("minecraft:empty"), None);
}

#[test]
fn generated_liquid_ticks_are_registered_when_watery_chunk_is_published() {
    let mut server = LocalRealmSession::new(12_345);
    let center = ChunkPos::new(117, -128);

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert!(
        snapshot_update_for(&updates, center).is_some(),
        "watery oracle chunk should publish a visible snapshot"
    );
    let scheduled_before_tick = server.scheduled_fluid_tick_count();
    assert!(
        scheduled_before_tick >= 90,
        "seed 12345 chunk (117,-128) should carry at least the 90 liquid-carved oracle ticks"
    );

    let report = server.simulation_tick_report();

    assert!(
        report.fluid_ticks_executed > 0,
        "generated liquid ticks should execute once the chunk is entity-ticking"
    );
    assert!(
        report.fluid_ticks_executed < scheduled_before_tick,
        "only liquid ticks in entity-ticking chunks should execute immediately; ticking-lane neighbors remain pending"
    );
}

#[test]
fn generated_mclone_flat_reach_is_quiescent_when_every_source_is_woken() {
    let seed = -98_765;
    let center = ChunkPos::new(-118, -159);
    let definition =
        DimensionDefinition::overworld(seed, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_dimension_definition(definition);
    server.set_lighting_enabled(false);
    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center,
            render_distance: 2,
            chunk_tracking_radius: 2,
        }),
    );

    assert_eq!(
        server.scheduled_fluid_tick_count(),
        0,
        "ordinary Mclone reaches must not carry generation-time liquid ticks"
    );

    let mut sources = Vec::new();
    for x in center.min_block_x()..=center.min_block_x() + 15 {
        for z in center.min_block_z()..=center.min_block_z() + 15 {
            for y in 0..256 {
                let pos = WorldBlockPos::new(x, y, z);
                if server.scheduler().block_at_world(pos) == Some(WATER) {
                    sources.push(pos);
                }
            }
        }
    }
    assert!(
        sources.len() > 100,
        "reviewed lowland reach should contain a meaningful source-water body"
    );
    for source in &sources {
        server.schedule_fluid_tick(*source, FluidKind::Water, 0);
    }

    let mut executed = 0;
    let mut mutated = 0;
    for _ in 0..2 {
        let report = server.simulation_tick_report();
        executed += report.fluid_ticks_executed;
        mutated += report.fluid_mutated_blocks;
    }

    assert_eq!(executed, sources.len());
    assert_eq!(mutated, 0, "waking a flat contained reach must not spill");
    assert_eq!(
        server.scheduled_fluid_tick_count(),
        0,
        "a quiescent reach must drain the synthetic wake queue"
    );
}

#[test]
fn scheduled_water_tick_spreads_down_and_publishes_section_update() {
    let mut server = LocalRealmSession::new(12_345);
    let initial_updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let mut client_snapshot = snapshot_update_for(&initial_updates, ChunkPos::new(0, 0))
        .expect("initial interest should publish visible chunk snapshot")
        .clone();
    let source = WorldBlockPos::new(8, 120, 8);
    let below = source.below();

    server.scheduler_mut().set_block_at_world(source, WATER);
    server.scheduler_mut().set_block_at_world(below, AIR);
    server.schedule_fluid_tick(source, FluidKind::Water, 0);

    let report = server.simulation_tick_report();

    assert!(report.fluid_ticks_executed >= 1);
    assert!(report.fluid_mutated_blocks >= 1);
    assert!(report.scheduled_fluid_ticks >= 1);
    assert_eq!(
        server.scheduler().block_at_world(below),
        Some(WATER_LEVEL_8)
    );
    assert_eq!(
        report.fluid_snapshot_events, 0,
        "fluid mutation should no longer publish per-block chunk snapshots"
    );
    assert!(
        snapshot_update_for(&report.updates, ChunkPos::new(0, 0)).is_none(),
        "fluid mutation should publish section deltas instead of chunk snapshots"
    );
    assert!(
        apply_section_updates_to_snapshot(&mut client_snapshot, &report.updates) >= 1,
        "fluid mutation should publish at least one section block update"
    );
    assert_eq!(
        snapshot_block_state(&client_snapshot, source),
        generated_block_state_id(WATER)
    );
    assert_eq!(
        snapshot_block_state(&client_snapshot, below),
        generated_block_state_id(WATER_LEVEL_8)
    );
}

#[test]
fn scheduled_water_slope_matches_oracle_fixture_after_5_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_water_slope(&mut server);

    run_simulation_ticks(&mut server, 5);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(1, 80, 2)),
        Some(WATER)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 80, 2)),
        Some(WATER_LEVEL_1)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(3, 80, 2)),
        Some(AIR)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/water-slope-5-ticks.json"),
    );
}

#[test]
fn scheduled_water_slope_matches_oracle_fixture_after_10_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_water_slope(&mut server);

    run_simulation_ticks(&mut server, 10);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(1, 80, 2)),
        Some(WATER)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 80, 2)),
        Some(WATER_LEVEL_1)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(3, 80, 2)),
        Some(WATER_LEVEL_2)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(4, 80, 2)),
        Some(AIR)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/water-slope-10-ticks.json"),
    );
}

#[test]
fn scheduled_water_cross_chunk_slope_matches_oracle_fixture_after_10_ticks() {
    let mut server = new_liquid_oracle_server_with_radius(1);
    setup_cross_chunk_water_slope(&mut server);

    run_simulation_ticks(&mut server, 10);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(15, 80, 2)),
        Some(WATER)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(16, 80, 2)),
        Some(WATER_LEVEL_1)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(17, 80, 2)),
        Some(WATER_LEVEL_2)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/water-cross-chunk-slope-10-ticks.json"),
    );
}

#[test]
fn cross_chunk_water_tick_waits_until_neighbor_is_entity_ticking() {
    let mut server = new_liquid_oracle_server();
    setup_cross_chunk_water_slope(&mut server);

    run_simulation_ticks(&mut server, 10);

    assert_eq!(
        server
            .scheduler()
            .holder(ChunkPos::new(1, 0))
            .unwrap()
            .full_status(),
        FullChunkStatus::Ticking
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(16, 80, 2)),
        Some(WATER_LEVEL_1),
        "entity-ticking chunk (0,0) should mutate the loaded neighbor chunk"
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(17, 80, 2)),
        Some(AIR),
        "non-entity-ticking chunk (1,0) should not run its due follow-up tick yet"
    );
    let pending_ticks = server
        .liquid_ticks
        .scheduled_tick_entries(server.simulation_tick());
    assert!(
        pending_ticks.contains(&(WorldBlockPos::new(16, 80, 2), FluidKind::Water, 0)),
        "the cross-chunk follow-up tick should remain due and pending: {pending_ticks:?}"
    );

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    run_simulation_ticks(&mut server, 1);

    assert_eq!(
        server
            .scheduler()
            .holder(ChunkPos::new(1, 0))
            .unwrap()
            .full_status(),
        FullChunkStatus::EntityTicking
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(17, 80, 2)),
        Some(WATER_LEVEL_2),
        "the due cross-chunk tick should run after the neighbor becomes entity-ticking"
    );
}

#[test]
fn scheduled_water_fall_matches_oracle_fixture_after_10_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_water_fall(&mut server);

    run_simulation_ticks(&mut server, 10);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 86, 2)),
        Some(WATER)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 85, 2)),
        Some(WATER_LEVEL_8)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 84, 2)),
        Some(WATER_LEVEL_8)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/water-fall-10-ticks.json"),
    );
}

#[test]
fn scheduled_water_source_conversion_matches_oracle_fixture_after_5_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_water_source_conversion(&mut server);

    run_simulation_ticks(&mut server, 5);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 80, 2)),
        Some(WATER)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/water-source-conversion-5-ticks.json"),
    );
}

#[test]
fn scheduled_lava_slope_matches_oracle_fixture_after_30_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_lava_slope(&mut server);

    run_simulation_ticks(&mut server, 30);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(1, 80, 2)),
        Some(LAVA)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 80, 2)),
        Some(LAVA_LEVEL_2)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/lava-slope-30-ticks.json"),
    );
}

#[test]
fn scheduled_lava_fall_matches_oracle_fixture_after_60_ticks() {
    let mut server = new_liquid_oracle_server();
    setup_lava_fall(&mut server);

    run_simulation_ticks(&mut server, 60);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 86, 2)),
        Some(LAVA)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 85, 2)),
        Some(LAVA_LEVEL_8)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 84, 2)),
        Some(LAVA_LEVEL_8)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/lava-fall-60-ticks.json"),
    );
}

#[test]
fn lava_source_water_contact_matches_oracle_fixture_after_1_script_tick() {
    let mut server = new_liquid_oracle_server();
    setup_lava_source_water_contact(&mut server);

    run_simulation_ticks(&mut server, 2);

    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(1, 80, 2)),
        Some(OBSIDIAN)
    );
    assert_eq!(
        server
            .scheduler()
            .block_at_world(WorldBlockPos::new(2, 80, 2)),
        Some(WATER)
    );
    assert_liquid_oracle_fixture_matches(
        &server,
        include_str!("../../../../../test/fixtures/liquid/lava-source-water-contact-1-ticks.json"),
    );
}

#[test]
fn scheduled_fluid_tick_waits_until_chunk_is_entity_ticking() {
    let mut server = LocalRealmSession::new(12_345);
    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let source = WorldBlockPos::new(8, 120, 8);
    let below = source.below();
    server.scheduler_mut().set_block_at_world(source, WATER);
    server.scheduler_mut().set_block_at_world(below, AIR);
    server.schedule_fluid_tick(source, FluidKind::Water, 0);

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let report = server.simulation_tick_report();

    assert_eq!(
        server
            .scheduler()
            .holder(ChunkPos::new(0, 0))
            .unwrap()
            .full_status(),
        FullChunkStatus::Ticking
    );
    assert!(
        report.scheduled_fluid_ticks >= 1,
        "the manually scheduled tick in the old non-entity-ticking chunk should remain pending"
    );
    assert_ne!(server.scheduler().block_at_world(below), Some(WATER));
}

#[test]
fn frozen_scheduled_fluid_ticks_remain_pending_without_mutation() {
    let mut server = new_liquid_oracle_server();
    let source = WorldBlockPos::new(2, 84, 2);
    let below = source.below();
    server.scheduler_mut().set_block_at_world(source, WATER);
    server.scheduler_mut().set_block_at_world(below, AIR);
    server.schedule_fluid_tick(source, FluidKind::Water, 0);
    let scheduled_before = server.scheduled_fluid_tick_count();

    server.set_scheduled_fluid_ticks_frozen(true);
    let report = server.simulation_tick_report();

    assert_eq!(report.fluid_ticks_executed, 0);
    assert_eq!(report.fluid_mutated_blocks, 0);
    assert_eq!(server.scheduled_fluid_tick_count(), scheduled_before);
    assert_ne!(server.scheduler().block_at_world(below), Some(WATER));
}
