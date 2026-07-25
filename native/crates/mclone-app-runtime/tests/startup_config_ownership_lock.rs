//! Tactical 173 ownership lock for canonical shared startup configuration.
//!
//! Platform sources may collect launch values and retain platform resources,
//! but desktop/scene DTOs must carry `StartupSceneOptions` whole and the browser
//! must keep it behind `WebStartupConfig`. These source checks turn the most
//! likely copy-schema regressions into test failures.

use std::path::{Path, PathBuf};

fn native_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("native root resolvable")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(native_root().join(relative)).expect("source file readable")
}

fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source.find(marker).expect("item marker present");
    let body = &source[start..];
    let mut depth = 0usize;
    let mut opened = false;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'{' => {
                opened = true;
                depth += 1;
            }
            b'}' if opened => {
                depth -= 1;
                if depth == 0 {
                    return &body[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated braced item for {marker}")
}

const SHARED_FIELD_NAMES: &[&str] = &[
    "seed",
    "chunk_x",
    "chunk_z",
    "render_distance",
    "render_compile_worker_count",
    "render_compile_max_pending_jobs",
    "remote_addr",
    "day_time_override",
    "freeze_time",
    "movement_speed_multiplier",
    "movement_mode",
    "debug_passive_showcase",
    "lighting_enabled",
    "light_status_batch_size",
];

fn assert_nested_canonical_config(item: &str, owner: &str) {
    assert!(
        item.contains("startup:") && item.contains("StartupSceneOptions"),
        "{owner} must retain StartupSceneOptions as a nested canonical value"
    );
    for field in SHARED_FIELD_NAMES {
        assert!(
            !item.lines().any(|line| {
                let line = line.trim();
                line.starts_with("pub") && line.contains(&format!(" {field}:"))
            }),
            "{owner} restates canonical shared field `{field}`"
        );
    }
}

#[test]
fn desktop_and_scene_host_retain_canonical_startup_config() {
    let desktop = read("apps/mclone-native-client/src/cli.rs");
    assert_nested_canonical_config(
        braced_item(&desktop, "pub(crate) struct SceneOptions"),
        "desktop SceneOptions",
    );

    let scene = read("crates/mclone-scene/src/options.rs");
    assert_nested_canonical_config(
        braced_item(&scene, "pub struct McloneSceneHostOptions"),
        "McloneSceneHostOptions",
    );
}

#[test]
fn browser_startup_boundary_stays_opaque() {
    let rust = read("apps/mclone-web-client/src/web_scene_host.rs");
    for function in ["pub async fn mclone_web_create_scene_host_with_startup"] {
        let start = rust.find(function).expect("browser constructor present");
        let signature = &rust[start..rust[start..].find(") ->").expect("signature end") + start];
        assert!(
            signature.contains("startup: WebStartupConfig"),
            "{function} must consume the opaque Rust startup handle"
        );
        for scalar in [
            "movement_speed_multiplier:",
            "render_distance:",
            "section_occlusion_culling:",
            "force_fullbright:",
        ] {
            assert!(
                !signature.contains(scalar),
                "{function} regained shared scalar parameter `{scalar}`"
            );
        }
    }

    let typescript = read("apps/mclone-web-client/www/mclone-web-app.ts");
    assert!(!typescript.contains("mclone_web_create_worker_scene_host_with_startup"));
    assert!(!typescript.contains("mclone_web_create_remote_scene_host_with_startup"));
    assert!(typescript.contains("mclone_web_create_scene_host_with_startup"));
    for retired in [
        "movementSpeedMultiplier",
        "lightStatusBatchSize",
        "debugPassiveShowcase",
        "lightingEnabled",
        "clearWorldStorage",
    ] {
        assert!(
            !typescript.contains(retired),
            "TypeScript regained retired shared startup field `{retired}`"
        );
    }
}

#[test]
fn app_startup_literals_are_named_overlays_not_complete_schemas() {
    fn collect_rs(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("app source directory readable") {
            let path = entry.expect("directory entry readable").path();
            if path.is_dir() {
                collect_rs(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect_rs(&native_root().join("apps"), &mut files);
    for file in files {
        let source = std::fs::read_to_string(&file).expect("app source readable");
        let mut remainder = source.as_str();
        while let Some(index) = remainder.find("StartupSceneOptions {") {
            let line_start = remainder[..index].rfind('\n').map_or(0, |start| start + 1);
            let line = &remainder[line_start..index];
            if !line.contains("->") {
                let literal = braced_item(&remainder[index..], "StartupSceneOptions {");
                assert!(
                    literal.contains(".."),
                    "{} contains a complete app-owned StartupSceneOptions literal; use Default or a named overlay:\n{}",
                    file.display(),
                    literal
                );
            }
            remainder = &remainder[index + "StartupSceneOptions {".len()..];
        }
    }
}
