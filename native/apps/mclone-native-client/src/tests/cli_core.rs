use super::*;

#[test]
fn cli_defaults_to_window() {
    assert_eq!(
        Cli::parse([]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
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
        }
    );

    assert_eq!(
        Cli::parse(["--start-in-world".to_owned(), "false".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::Menu,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
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
        }
    );

    assert_eq!(
        Cli::parse(["--startup-wait".to_owned(), "progress".to_owned()]).unwrap(),
        Cli::Window {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::Progress,
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
        }
    );
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
