use std::fs;
use std::path::PathBuf;

fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn ordinary_catalog_policy_is_rust_owned() {
    let root = app_root();
    let app = fs::read_to_string(root.join("www/mclone-web-app.ts")).unwrap();
    let indexed_db = fs::read_to_string(root.join("www/mclone-web-world-catalog.ts")).unwrap();
    let scene = fs::read_to_string(root.join("src/web_scene_host.rs")).unwrap();
    let execution = fs::read_to_string(root.join("src/web_catalog_execution.rs")).unwrap();
    let storage_plan = fs::read_to_string(
        root.join("../../crates/mclone-app-runtime/src/catalog_storage_plan.rs"),
    )
    .unwrap();

    assert!(app.contains("takeSceneOperation("));
    assert!(app.contains("operation.takeIndexedDbExecution()"));
    assert!(app.contains("operation.completeIndexedDbExecution(execution)"));
    assert!(app.contains("session.completeSceneOperation(operation)"));
    assert!(!app.contains("takeWorldCatalogExecution"));
    assert!(!app.contains("applyWorldCatalogExecution"));
    assert!(!app.contains("catalogRequestId"));
    assert!(!app.contains("catalogOperation"));
    assert!(!app.contains("catalogGenerationProfile"));
    assert!(!app.contains("catalogRequestedId"));
    assert!(!app.contains("executeWorldCatalogRequest"));

    assert!(indexed_db.contains("executeIndexedDbCatalogExecution"));
    assert!(indexed_db.contains("enqueueCatalogStorageAction"));
    assert!(indexed_db.contains("acceptStorageRead"));
    assert!(!indexed_db.contains("listIndexedDbCatalogWorlds"));
    assert!(!indexed_db.contains("createIndexedDbCatalogWorld"));
    assert!(!indexed_db.contains("mclone_web_catalog_prepare_create_world"));

    assert!(scene.contains("self.platform.take_catalog_operation()"));
    assert!(!scene.contains("PendingWebCatalogOperation"));
    assert!(!scene.contains("write_catalog_request"));
    assert!(!scene.contains("catalogOperation"));

    assert!(execution.contains("CatalogExecutionCore"));
    assert!(execution.contains("token: PlatformOperationToken"));
    assert!(!execution.contains("token: Option<PlatformOperationToken>"));
    assert!(execution.contains("pub struct WebCatalogSmokeExecution"));
    assert!(!execution.contains("mclone_web_catalog_smoke_execution"));
    assert!(execution.contains("encode_storage_step"));
    assert!(execution.contains("decode_web_local_world_summaries"));
    assert!(!execution.contains("enum DeleteManyStage"));
    assert!(!execution.contains("fn clear_world_transactions"));

    assert!(storage_plan.contains("pub struct CatalogExecutionCore"));
    assert!(storage_plan.contains("WorldCatalogRequest::CreateWorld"));
    assert!(storage_plan.contains("enum DeleteManyStage"));
    assert!(storage_plan.contains("fn clear_world_transactions"));
}

#[test]
fn indexed_db_schema_and_record_played_atomicity_stay_locked() {
    let indexed_db =
        fs::read_to_string(app_root().join("www/mclone-web-world-catalog.ts")).unwrap();

    assert!(indexed_db.contains("WORLD_DB_VERSION = 6"));
    assert!(indexed_db.contains("Enqueue read-dependent writes synchronously"));
    assert!(indexed_db.contains("record-played get/put must remain in one"));
    assert!(indexed_db.contains("Promise.all(step.transactions.map"));
}
