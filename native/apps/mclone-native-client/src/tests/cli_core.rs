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
