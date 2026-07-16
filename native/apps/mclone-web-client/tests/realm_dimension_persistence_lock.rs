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
