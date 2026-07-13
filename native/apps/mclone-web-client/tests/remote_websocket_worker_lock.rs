use std::fs;
use std::path::PathBuf;

fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn production_remote_websocket_is_worker_owned() {
    let root = app_root();
    let rust_adapter = fs::read_to_string(root.join("src/web_remote_session.rs")).unwrap();
    let worker = fs::read_to_string(root.join("www/mclone-remote-websocket-worker.ts")).unwrap();

    assert!(rust_adapter.contains("Worker::new_with_options"));
    assert!(rust_adapter.contains("post_message_with_transfer"));
    assert!(!rust_adapter.contains("WebSocket::new"));
    assert!(!rust_adapter.contains("web_sys::{BinaryType"));

    assert!(worker.contains("new WebSocket(message.url)"));
    assert!(worker.contains("mclone_web_canonicalize_remote_command"));
    assert!(worker.contains("mclone_web_decode_remote_update_batch"));
    assert!(worker.contains("MAX_UNCONSUMED_UPDATE_BYTES"));
    assert!(worker.contains("updates-drained"));
}

#[test]
fn dedicated_websocket_adapter_does_not_loop_back_through_tcp() {
    let server_root = app_root().join("../mclone-dedicated-server/src");
    let adapter = fs::read_to_string(server_root.join("websocket_connection.rs")).unwrap();

    assert!(adapter.contains("DedicatedNetworkEvent::Connected"));
    assert!(adapter.contains("DedicatedNetworkEvent::Command"));
    assert!(adapter.contains("DedicatedOutbound::channel"));
    assert!(!adapter.contains("NativeClientSession"));
    assert!(!adapter.contains("upstream_addr"));
}
