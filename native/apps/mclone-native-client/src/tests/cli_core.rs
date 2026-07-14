use super::*;
use crate::cli::DESKTOP_LOCAL_ARG_FLAGS;
use std::collections::BTreeSet;

fn collect_cli_source_flags(source: &str) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    let bytes = source.as_bytes();
    let mut index = 0;
    while let Some(offset) = source[index..].find("--") {
        let start = index + offset;
        let mut end = start + 2;
        while end < bytes.len() {
            let byte = bytes[end];
            if byte.is_ascii_alphanumeric() || byte == b'-' {
                end += 1;
            } else {
                break;
            }
        }
        if end > start + 2 {
            flags.insert(source[start..end].to_owned());
        }
        index = end.max(start + 2);
    }
    flags
}

#[test]
fn desktop_cli_flags_are_classified_as_shared_or_desktop_local() {
    let shared = mclone_app_runtime::startup_args::STARTUP_ARG_FLAGS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let desktop_local = DESKTOP_LOCAL_ARG_FLAGS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(
        shared.is_disjoint(&desktop_local),
        "desktop-local flags must not duplicate shared startup flags"
    );

    let unclassified = collect_cli_source_flags(include_str!("../cli.rs"))
        .into_iter()
        .filter(|flag| !shared.contains(flag.as_str()) && !desktop_local.contains(flag.as_str()))
        .collect::<Vec<_>>();
    assert!(
        unclassified.is_empty(),
        "classify desktop CLI flags as shared startup or desktop-local: {unclassified:?}"
    );
}

#[test]
fn live_diorama_productization_boundary_is_explicit_before_refactor() {
    let desktop = include_str!("../desktop_scene_host.rs");
    let scene = include_str!("../../../../crates/mclone-scene/src/session.rs");
    let session = include_str!("../../../../crates/mclone-app-runtime/src/session.rs");
    let ui = include_str!("../../../../crates/mclone-ui/src/v2.rs");

    assert_eq!(
        desktop
            .matches("if let Some(diorama) = scene.live_diorama.as_ref()")
            .count(),
        1,
        "Slice 0 expects exactly one desktop launch projection to extract"
    );
    for marker in [
        "authored_world_fixture_marker_path(&diorama.world_dir)",
        "WarmWorldStandbyRequest::new(",
        ".with_persistent_world_dir(",
        ".with_embedded_preview(",
        "host.begin_warm_world_standby(device, queue, request)",
    ] {
        assert!(
            desktop.contains(marker),
            "desktop live-diorama assembly lost `{marker}` before extraction"
        );
    }
    assert_eq!(
        scene.matches("pub fn begin_warm_world_standby(").count(),
        1,
        "scene host remains the sole detached-world execution owner"
    );
    assert!(session.contains("pub enum SessionStartRequest"));
    for variant in [
        "CreateLocalWorld { options: LocalWorldCreateOptions }",
        "OpenLocalWorld { id: LocalWorldId }",
        "JoinRemote { endpoint: RemoteSessionEndpoint }",
    ] {
        assert!(session.contains(variant));
    }
    assert!(!session.contains("Lobby"));
    assert!(!session.contains("Scenario"));

    let title_start = ui.find("fn title_layout(").expect("shared title layout");
    let title_end = ui[title_start..]
        .find("fn world_list_row_id(")
        .map(|offset| title_start + offset)
        .expect("title layout end");
    let title = &ui[title_start..title_end];
    for action in [
        "GameUiAction::OpenWorldList",
        "GameUiAction::OpenJoinRemote",
        "GameUiAction::OpenOptions",
        "GameUiAction::Quit",
    ] {
        assert!(title.contains(action));
    }
    assert!(!title.contains("Lobby"));
    assert!(!title.contains("Scenario"));
}

#[test]
fn cli_defaults_to_window() {
    assert_eq!(
        Cli::parse([]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );
}

#[test]
fn cli_parses_window_menu_start_intent() {
    assert_eq!(
        Cli::parse(["--menu".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::Menu,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );

    assert_eq!(
        Cli::parse(["--start-in-world".to_owned(), "false".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::Menu,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );
}

#[test]
fn cli_parses_startup_wait_policy() {
    assert_eq!(
        Cli::parse(["--startup-wait".to_owned(), "idle".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::Idle,
            frame_report: None,
        }
    );

    assert_eq!(
        Cli::parse(["--startup-wait".to_owned(), "progress".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::Progress,
            frame_report: None,
        }
    );

    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--startup-wait".to_owned(),
        "frames:2".to_owned(),
    ])
    .unwrap();
    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected screenshot cli");
    };
    assert_eq!(options.startup_wait, StartupWaitPolicy::Frames(2));
}

#[test]
fn cli_rejects_startup_wait_for_non_host_modes() {
    let err = Cli::parse([
        "--headless-dual-view".to_owned(),
        "/tmp/mclone-dual-view".to_owned(),
        "--startup-wait".to_owned(),
        "idle".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("applies to window mode and --screenshot"));

    let err = Cli::parse([
        "--torch-light-probe".to_owned(),
        "/tmp/mclone-torch-light-probe".to_owned(),
        "--startup-wait".to_owned(),
        "idle".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("applies to window mode and --screenshot"));

    let err = Cli::parse(["--startup-wait".to_owned(), "frames:4097".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("frames must be between 0 and 4096"));
}

#[test]
fn cli_parses_local_world_dir() {
    let cli = Cli::parse(["--world-dir".to_owned(), "/tmp/mclone-world".to_owned()]).unwrap();

    assert_eq!(
        cli,
        Cli::Window {
            scene: SceneOptions {
                world_dir: Some(PathBuf::from("/tmp/mclone-world")),
                ..SceneOptions::default()
            },
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );
}

#[test]
fn cli_parses_launch_only_warm_world_standby_seed() {
    let cli = Cli::parse(["--warm-world-standby-seed".to_owned(), "-98765".to_owned()]).unwrap();
    let Cli::Window { scene, .. } = cli else {
        panic!("expected window mode");
    };
    assert_eq!(scene.warm_world_standby_seed, Some(-98765));
}

#[test]
fn cli_parses_launch_only_live_diorama_placement() {
    let cli = Cli::parse([
        "--world-dir".to_owned(),
        "/tmp/table-a".to_owned(),
        "--live-diorama-world-dir".to_owned(),
        "/tmp/island-b".to_owned(),
        "--live-diorama-source-region".to_owned(),
        "2,-3,1,4,6".to_owned(),
        "--live-diorama-source-anchor".to_owned(),
        "8.5,65,8.5".to_owned(),
        "--live-diorama-composition-anchor".to_owned(),
        "10,70,-2".to_owned(),
        "--live-diorama-scale".to_owned(),
        "0.25".to_owned(),
        "--warm-world-standby-cadence".to_owned(),
        "5/5/5".to_owned(),
    ])
    .unwrap();
    let Cli::Window { scene, .. } = cli else {
        panic!("expected window mode");
    };
    let diorama = scene.live_diorama.expect("live diorama parsed");
    assert_eq!(diorama.world_dir, PathBuf::from("/tmp/island-b"));
    assert_eq!(diorama.source_region.center(), ChunkPos::new(2, -3));
    assert_eq!(diorama.source_region.horizontal_radius(), 1);
    assert_eq!(diorama.source_region.min_section_y(), 4);
    assert_eq!(diorama.source_region.max_section_y(), 6);
    assert_eq!(
        diorama.placement.source_anchor(),
        mclone_core::Vec3d::new(8.5, 65.0, 8.5)
    );
    assert_eq!(
        diorama.placement.composition_anchor(),
        mclone_core::Vec3d::new(10.0, 70.0, -2.0)
    );
    assert_eq!(diorama.placement.uniform_scale(), 0.25);
    assert_eq!(
        scene.warm_world_standby_cadence,
        Some(mclone_server::SimulationCadenceConfig::new(5, 5, 5))
    );
}

#[test]
fn cli_defaults_live_diorama_to_authored_fixture_anchors() {
    let cli = Cli::parse([
        "--world-dir".to_owned(),
        "/tmp/table-a".to_owned(),
        "--live-diorama-world-dir".to_owned(),
        "/tmp/island-b".to_owned(),
    ])
    .unwrap();
    let Cli::Window { scene, .. } = cli else {
        panic!("expected window mode");
    };
    let diorama = scene.live_diorama.expect("live diorama parsed");
    assert_eq!(
        diorama.placement.source_anchor(),
        mclone_core::Vec3d::new(8.5, 65.0, 8.5)
    );
    assert_eq!(
        diorama.placement.composition_anchor(),
        mclone_core::Vec3d::new(8.0, 65.03125, 8.0)
    );
    assert_eq!(
        diorama.return_placement.source_anchor(),
        mclone_core::Vec3d::new(8.0, 65.03125, 8.0)
    );
    assert_eq!(
        diorama.return_placement.composition_anchor(),
        mclone_core::Vec3d::new(4.0, 67.03125, 8.0)
    );
}

#[test]
fn cli_rejects_conflicting_retained_world_presentations() {
    let error = Cli::parse([
        "--live-diorama-world-dir".to_owned(),
        "/tmp/island-b".to_owned(),
        "--warm-world-standby-seed".to_owned(),
        "17502".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(error.contains("mutually exclusive retained-world presentations"));
}

#[test]
fn cli_rejects_warm_world_standby_for_remote_world() {
    let error = Cli::parse([
        "--warm-world-standby-seed".to_owned(),
        "-98765".to_owned(),
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(error.contains("applies only to local integrated worlds"));
}

#[test]
fn cli_parses_world_root_and_transient_catalog_mode() {
    let cli = Cli::parse(["--world-root".to_owned(), "/tmp/mclone-worlds".to_owned()]).unwrap();

    assert_eq!(
        cli,
        Cli::Window {
            scene: SceneOptions {
                world_root: Some(PathBuf::from("/tmp/mclone-worlds")),
                ..SceneOptions::default()
            },
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );

    let cli = Cli::parse(["--transient".to_owned()]).unwrap();
    assert_eq!(
        cli,
        Cli::Window {
            scene: SceneOptions {
                world_root: None,
                ..SceneOptions::default()
            },
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );
}

#[test]
fn cli_parses_window_frame_report_options() {
    assert_eq!(
        Cli::parse([
            "--window-frame-report".to_owned(),
            "/tmp/mclone-window-report.json".to_owned(),
            "--window-frame-report-frames".to_owned(),
            "120".to_owned(),
        ])
        .unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: Some(WindowFrameReportOptions {
                path: PathBuf::from("/tmp/mclone-window-report.json"),
                frames: 120,
            }),
        }
    );
}

#[test]
fn cli_rejects_window_frame_report_for_non_window_modes() {
    let err = Cli::parse([
        "--window-frame-report".to_owned(),
        "/tmp/mclone-window-report.json".to_owned(),
        "--startup-streaming-perf".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("applies only to window mode"));

    let err = Cli::parse(["--window-frame-report-frames".to_owned(), "120".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("requires --window-frame-report"));
}

#[test]
fn cli_rejects_conflicting_world_storage_args() {
    let err = Cli::parse([
        "--transient".to_owned(),
        "--world-dir".to_owned(),
        "/tmp/mclone-world".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("--transient cannot be combined with --world-dir"));

    let err = Cli::parse([
        "--transient".to_owned(),
        "--world-root".to_owned(),
        "/tmp/mclone-worlds".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("--transient cannot be combined with --world-root"));

    let err = Cli::parse([
        "--world-dir".to_owned(),
        "/tmp/mclone-world".to_owned(),
        "--world-root".to_owned(),
        "/tmp/mclone-worlds".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("--world-dir cannot be combined with --world-root"));

    let err = Cli::parse([
        "--world-dir".to_owned(),
        "/tmp/mclone-world".to_owned(),
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("--world-dir applies only to local integrated worlds"));
}
