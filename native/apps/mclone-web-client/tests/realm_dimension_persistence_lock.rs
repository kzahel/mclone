//! Tactical 185 Slice 3 lock for dimension-qualified IndexedDB records and
//! the explicit v5-to-v6 Overworld migration.

const WORLD_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-world-catalog.ts"
));
const INTEGRATED_SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-integrated-server-worker.ts"
));
const SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_server_worker.rs"
));

#[test]
fn indexed_db_v6_qualifies_dimension_records_and_migrates_v5_to_overworld() {
    assert!(WORLD_CATALOG.contains("export const WORLD_DB_VERSION = 6;"));
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"dimensionKey\", \"x\", \"z\"]"));
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"dimensionKey\"]"));
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"playerKey\"]"));
    assert!(WORLD_CATALOG.contains("dimensionKey: \"minecraft:overworld\""));
    assert!(WORLD_CATALOG.contains("migrateLegacyWorldRecordsToOverworld"));

    assert!(INTEGRATED_SERVER_WORKER.contains("request.dimensionKey ?? \"minecraft:overworld\""));
    assert!(INTEGRATED_SERVER_WORKER.contains("WORLD_DIMENSION_STORE"));
    assert!(SERVER_WORKER.contains("set_string(&object, \"dimensionKey\", dimension.as_str())?;"));
    assert!(SERVER_WORKER.contains("set_number(&object, \"x\", f64::from(pos.x))?;"));
    assert!(SERVER_WORKER.contains("set_number(&object, \"z\", f64::from(pos.z))?;"));
}

#[test]
fn indexed_db_player_lifecycle_records_remain_opaque_rust_owned_blobs() {
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"playerKey\"]"));
    assert!(INTEGRATED_SERVER_WORKER.contains("request.kind === \"player\""));
    assert!(INTEGRATED_SERVER_WORKER.contains("key = [worldId, request.playerKey];"));
    assert!(INTEGRATED_SERVER_WORKER.contains("putIndexedDbPlayerRecords"));
    assert!(INTEGRATED_SERVER_WORKER.contains("normalizeIndexedDbPlayerRecord"));
    assert!(SERVER_WORKER.contains("decode_player_record"));
    assert!(SERVER_WORKER.contains("encode_player_record(record)"));
    assert!(SERVER_WORKER.contains("\"indexedDbPlayers\""));

    for platform_source in [WORLD_CATALOG, INTEGRATED_SERVER_WORKER] {
        assert!(!platform_source.contains("pending_death_cause"));
        assert!(!platform_source.contains("pendingDeathCause"));
        assert!(!platform_source.contains("PlayerDamageCause"));
    }
}
