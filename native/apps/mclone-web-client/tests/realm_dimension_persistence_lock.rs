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
const RECORD_EXECUTOR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-persistence-executor.ts"
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

    assert!(RECORD_EXECUTOR.contains("store: WORLD_DIMENSION_STORE"));
    assert!(RECORD_EXECUTOR.contains("valueFields: [\"dimensionKey\", \"x\", \"z\"]"));
    assert!(SERVER_WORKER.contains("record_read_for_world_store_request"));
    assert!(SERVER_WORKER.contains("persistenceRecordRequests"));
    assert!(!INTEGRATED_SERVER_WORKER.contains("dimensionKey"));
}

#[test]
fn indexed_db_player_lifecycle_records_remain_opaque_rust_owned_blobs() {
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"playerKey\"]"));
    assert!(RECORD_EXECUTOR.contains("store: WORLD_PLAYER_STORE"));
    assert!(RECORD_EXECUTOR.contains("valueFields: [\"playerKey\"]"));
    assert!(RECORD_EXECUTOR.contains("record: uint8ArrayFromUnknown(stored.record)"));
    assert!(SERVER_WORKER.contains("world_store_completion_from_record_read"));
    assert!(!INTEGRATED_SERVER_WORKER.contains("playerKey"));

    for platform_source in [WORLD_CATALOG, INTEGRATED_SERVER_WORKER, RECORD_EXECUTOR] {
        assert!(!platform_source.contains("pending_death_cause"));
        assert!(!platform_source.contains("pendingDeathCause"));
        assert!(!platform_source.contains("PlayerDamageCause"));
    }
}
