//! Tactical 185 Slice 0 lock for the pre-dimension IndexedDB record shape.
//!
//! The schema is intentionally transitional. Slice 3 must replace these
//! assertions with upgrade and dimension-key coverage rather than silently
//! changing the browser's durable key layout.

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
fn indexed_db_v5_fixture_keys_one_realm_without_a_dimension_component() {
    assert!(WORLD_CATALOG.contains("export const WORLD_DB_VERSION = 5;"));
    assert!(
        WORLD_CATALOG
            .contains("db.createObjectStore(storeName, { keyPath: [\"worldId\", \"x\", \"z\"] })")
    );
    assert!(WORLD_CATALOG.contains("keyPath: [\"worldId\", \"playerKey\"]"));
    assert!(!WORLD_CATALOG.contains("dimensionKey"));

    assert!(INTEGRATED_SERVER_WORKER.contains("key = [worldId, request.x, request.z];"));
    assert!(SERVER_WORKER.contains("set_number(&object, \"x\", f64::from(pos.x))?;"));
    assert!(SERVER_WORKER.contains("set_number(&object, \"z\", f64::from(pos.z))?;"));
    assert!(!SERVER_WORKER.contains("\"dimensionKey\""));
}
