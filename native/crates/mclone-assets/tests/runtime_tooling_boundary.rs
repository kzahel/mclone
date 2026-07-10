//! Static Slice 0 lock: shipped Rust runtime/app code may consume prepared pack
//! artifacts, but it may not invoke or import authoring implementations.

use std::path::{Path, PathBuf};

fn native_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("native workspace root")
}

fn collect_source_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "target" || name == "vendor")
            {
                continue;
            }
            collect_source_files(&path, files);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml")
            || path.extension().is_some_and(|extension| {
                matches!(extension.to_str(), Some("rs" | "js" | "mjs" | "ts"))
            })
        {
            files.push(path);
        }
    }
}

#[test]
fn runtime_and_app_sources_do_not_invoke_asset_authoring_tools() {
    let native = native_root();
    let this_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/runtime_tooling_boundary.rs")
        .canonicalize()
        .expect("this integration test path");
    let forbidden = [
        ["tools/texture", "-lab"].concat(),
        ["overlay_pack", ".py"].concat(),
        ["asset_pack", ".py"].concat(),
        ["scripts/run-", "python"].concat(),
        ["texture-lab:", "runtime-compat"].concat(),
    ];
    let mut files = Vec::new();
    collect_source_files(&native.join("apps"), &mut files);
    collect_source_files(&native.join("crates"), &mut files);

    let mut offenders = Vec::new();
    for file in files {
        if file.canonicalize().ok().as_deref() == Some(this_test.as_path()) {
            continue;
        }
        let contents = std::fs::read_to_string(&file).expect("runtime source is readable");
        for (line_index, line) in contents.lines().enumerate() {
            if forbidden.iter().any(|pattern| line.contains(pattern)) {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    line_index + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "runtime/app crates must consume prepared asset packs and must not invoke texture-lab \
         TypeScript or pack-builder Python. Offending references:\n{}",
        offenders.join("\n")
    );
}
