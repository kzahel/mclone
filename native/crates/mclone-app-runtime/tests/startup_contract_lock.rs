//! docs/tactical/167 Slice 5: enforcement lock for the shared native startup
//! contract.
//!
//! Startup draw resources must be seeded from the startup pump's render seed
//! (`NativeSessionStartupPump::complete().startup_sections`), never by marking
//! every resident section dirty and recompiling. That dirty-all recompile is a
//! renderer/surface **resource-rebuild** path and is deliberately named
//! `recompile_all_render_section_meshes_for_resource_rebuild(...)`. It is owned
//! by `mclone-app-runtime/src/native_session_runtime.rs` (definition + host-mode
//! dispatch) and nothing else in the native tree may reference it: no startup
//! path, no platform adapter, no probe. Real resource rebuilds already go
//! through `mark_all_render_sections_dirty_for_resource_rebuild(...)` +
//! `sync_all_render_sections(...)` inline, so a fresh call site to the combined
//! helper is almost certainly a startup path trying to bypass the seed — this
//! test turns that into a compile-of-tests failure instead of a silent
//! double-compile regression.
//!
//! This mirrors the ABI-lock tests in `mclone-web-client/tests/` (source-scan
//! integration crates that fail loudly on drift).

use std::path::{Path, PathBuf};

/// The renamed resource-rebuild recompile helper. Allowed to appear only inside
/// its owner file; any other reference (outside a comment) is a contract bypass.
const FORBIDDEN_STARTUP_RECOMPILE: &str =
    "recompile_all_render_section_meshes_for_resource_rebuild";

/// `native/`, derived from this crate's manifest dir (`native/crates/mclone-app-runtime`).
fn native_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("native root resolvable from CARGO_MANIFEST_DIR")
}

/// The single owner file that is permitted to define and dispatch the helper.
fn owner_file() -> PathBuf {
    native_root()
        .join("crates")
        .join("mclone-app-runtime")
        .join("src")
        .join("native_session_runtime.rs")
        .canonicalize()
        .expect("owner file exists")
}

/// Recursively collect `.rs` files under `dir`, skipping the vendored tree.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "vendor" || name == "target")
            {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn resource_rebuild_recompile_helper_is_confined_to_its_owner() {
    let native = native_root();
    let owner = owner_file();

    let mut files = Vec::new();
    collect_rs_files(&native.join("apps"), &mut files);
    collect_rs_files(&native.join("crates"), &mut files);

    let mut offenders: Vec<String> = Vec::new();
    for file in files {
        if file.canonicalize().ok().as_deref() == Some(owner.as_path()) {
            continue;
        }
        // This lock file names the forbidden helper in its own constant/asserts.
        if file
            .file_name()
            .is_some_and(|name| name == "startup_contract_lock.rs")
        {
            continue;
        }
        let contents = std::fs::read_to_string(&file).expect("readable rust source");
        for (index, line) in contents.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                // Doc/comment mentions (e.g. "must not call ...") are allowed.
                continue;
            }
            if line.contains(FORBIDDEN_STARTUP_RECOMPILE) {
                offenders.push(format!("{}:{}: {}", file.display(), index + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "docs/tactical/167: `{FORBIDDEN_STARTUP_RECOMPILE}` is a resource-rebuild-only path owned by \
         native_session_runtime.rs and must not be called elsewhere. Startup callers must seed \
         from `NativeSessionStartupPump` completion sections. Offending references:\n{}",
        offenders.join("\n")
    );
}
