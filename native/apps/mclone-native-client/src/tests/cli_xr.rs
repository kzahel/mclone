use super::*;
use crate::cli::XrRenderModeOption;
use mclone_app_runtime::startup_args::StartupSceneOptions;

#[test]
fn cli_parses_xr_clear_smoke_options() {
    let cli = Cli::parse([
        "--xr-clear-smoke".to_owned(),
        "--frames".to_owned(),
        "12".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrClearSmoke {
            options: XrClearSmokeOptions {
                frame_limit: Some(12),
            },
        }
    );
}

#[test]
fn cli_parses_xr_mclone_smoke_options() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--frames".to_owned(),
        "24".to_owned(),
        "--seed".to_owned(),
        "54321".to_owned(),
        "--chunk-x".to_owned(),
        "2".to_owned(),
        "--chunk-z".to_owned(),
        "-1".to_owned(),
        "--render-distance".to_owned(),
        "3".to_owned(),
        "--day-time".to_owned(),
        "6000".to_owned(),
        "--freeze-time".to_owned(),
        "--fullbright".to_owned(),
        "true".to_owned(),
        "--view-pose".to_owned(),
        "8,72,-12,180".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions {
                    startup: StartupSceneOptions {
                        seed: 54321,
                        chunk_x: 2,
                        chunk_z: -1,
                        local_entry_intent: mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::ExplicitCoordinate,
                        render_distance: 3,
                        day_time_override: Some(6000),
                        freeze_time: true,
                        ..Default::default()
                    },
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
                frame_limit: Some(24),
                view_pose: Some(XrViewPose {
                    position: [8.0, 72.0, -12.0],
                    yaw_degrees: 180.0,
                }),
                underwater_mode: XrUnderwaterMode::Midpoint,
                debug_ui_screen: None,
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
        }
    );
}

#[test]
fn cli_parses_xr_view_pose_default() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--view-pose=default".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                frame_limit: Some(120),
                view_pose: None,
                underwater_mode: XrUnderwaterMode::Midpoint,
                debug_ui_screen: None,
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
        }
    );
}

#[test]
fn cli_parses_xr_underwater_mode() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--xr-underwater-mode".to_owned(),
        "per-eye".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                frame_limit: Some(120),
                view_pose: None,
                underwater_mode: XrUnderwaterMode::PerEye,
                debug_ui_screen: None,
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
        }
    );
}

#[test]
fn cli_parses_xr_debug_ui_screen() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--xr-debug-ui".to_owned(),
        "controls".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                frame_limit: Some(120),
                view_pose: None,
                underwater_mode: XrUnderwaterMode::Midpoint,
                debug_ui_screen: Some(XrDebugUiScreen::Controls),
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
        }
    );
}

#[test]
fn cli_parses_xr_graphics_debug_screen() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--xr-debug-ui".to_owned(),
        "graphics".to_owned(),
    ])
    .unwrap();

    assert!(matches!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                debug_ui_screen: Some(XrDebugUiScreen::Graphics),
                ..
            },
        }
    ));
}

#[test]
fn cli_parses_xr_seasonal_debug_screen() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--xr-debug-ui".to_owned(),
        "seasonal-debug".to_owned(),
    ])
    .unwrap();

    assert!(matches!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                debug_ui_screen: Some(XrDebugUiScreen::SeasonalDebug),
                ..
            },
        }
    ));
}

#[test]
fn cli_parses_xr_forever_smoke_options() {
    let cli = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--xr-forever".to_owned(),
        "--view-pose=0,78,-96,180".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrMcloneSmoke {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                frame_limit: None,
                view_pose: Some(XrViewPose {
                    position: [0.0, 78.0, -96.0],
                    yaw_degrees: 180.0,
                }),
                underwater_mode: XrUnderwaterMode::Midpoint,
                debug_ui_screen: None,
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
        }
    );
}

#[test]
fn cli_rejects_xr_clear_smoke_with_headless_mode() {
    let err = Cli::parse([
        "--xr-clear-smoke".to_owned(),
        "--headless-clear".to_owned(),
        "/tmp/mclone-clear.png".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("XR modes cannot be combined"));
}

#[test]
fn cli_rejects_xr_frames_without_xr_clear_smoke() {
    let err = Cli::parse(["--frames".to_owned(), "12".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(err.contains("--frames requires --xr-clear-smoke, --xr-mclone-smoke, or --desktop-xr"));
}

#[test]
fn cli_rejects_xr_forever_without_xr_smoke() {
    let err = Cli::parse(["--xr-forever".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("--xr-forever requires --xr-clear-smoke, --xr-mclone-smoke, or --desktop-xr")
    );
}

#[test]
fn cli_rejects_xr_forever_with_frame_count() {
    let err = Cli::parse([
        "--xr-clear-smoke".to_owned(),
        "--frames".to_owned(),
        "12".to_owned(),
        "--xr-forever".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("--xr-forever cannot be combined"));
}

#[test]
fn cli_rejects_xr_view_pose_without_mclone_smoke() {
    let err = Cli::parse(["--view-pose".to_owned(), "0,64,0,0".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(err.contains("--view-pose requires --xr-mclone-smoke"));
}

#[test]
fn cli_rejects_xr_underwater_mode_without_mclone_smoke() {
    let err = Cli::parse(["--xr-underwater-mode".to_owned(), "per-eye".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(err.contains("--xr-underwater-mode requires --xr-mclone-smoke"));
}

#[test]
fn cli_rejects_invalid_xr_view_pose() {
    let err = Cli::parse([
        "--xr-mclone-smoke".to_owned(),
        "--view-pose".to_owned(),
        "0,64,0".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("--view-pose expects X,Y,Z,YAW_DEGREES"));
}

#[test]
fn cli_rejects_combined_xr_smoke_modes() {
    let err = Cli::parse([
        "--xr-clear-smoke".to_owned(),
        "--xr-mclone-smoke".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("--xr-mclone-smoke cannot be combined"));
}

#[test]
fn cli_parses_desktop_xr_persistent_with_window_by_default() {
    let cli = Cli::parse(["--desktop-xr".to_owned()]).unwrap();

    assert_eq!(
        cli,
        Cli::DesktopXr {
            options: XrMcloneSmokeOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                frame_limit: None,
                view_pose: None,
                underwater_mode: XrUnderwaterMode::Midpoint,
                debug_ui_screen: None,
                render_mode: XrRenderModeOption::DualPerEye,
                render_mode_cycle: false,
            },
            window: true,
            start_intent: WindowStartIntent::Menu,
        }
    );
}

#[test]
fn cli_parses_desktop_xr_no_window() {
    let cli = Cli::parse(["--desktop-xr".to_owned(), "--no-window".to_owned()]).unwrap();

    assert!(matches!(cli, Cli::DesktopXr { window: false, .. }));
}

#[test]
fn cli_parses_desktop_xr_frame_bound() {
    let cli = Cli::parse([
        "--desktop-xr".to_owned(),
        "--frames".to_owned(),
        "300".to_owned(),
    ])
    .unwrap();

    assert!(matches!(
        cli,
        Cli::DesktopXr {
            options: XrMcloneSmokeOptions {
                frame_limit: Some(300),
                ..
            },
            window: true,
            start_intent: WindowStartIntent::Menu,
        }
    ));
}

#[test]
fn cli_parses_explicit_desktop_xr_world_entry() {
    let cli = Cli::parse([
        "--desktop-xr".to_owned(),
        "--start-in-world".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert!(matches!(
        cli,
        Cli::DesktopXr {
            start_intent: WindowStartIntent::InWorld,
            ..
        }
    ));
}

#[test]
fn cli_parses_desktop_xr_forever_stays_persistent() {
    let cli = Cli::parse(["--desktop-xr".to_owned(), "--xr-forever".to_owned()]).unwrap();

    assert!(matches!(
        cli,
        Cli::DesktopXr {
            options: XrMcloneSmokeOptions {
                frame_limit: None,
                ..
            },
            ..
        }
    ));
}

#[test]
fn cli_parses_desktop_xr_mclone_modifiers() {
    let cli = Cli::parse([
        "--desktop-xr".to_owned(),
        "--view-pose".to_owned(),
        "0,72,0,0".to_owned(),
        "--xr-underwater-mode".to_owned(),
        "per-eye".to_owned(),
        "--xr-debug-ui".to_owned(),
        "pause".to_owned(),
    ])
    .unwrap();

    assert!(matches!(
        cli,
        Cli::DesktopXr {
            options: XrMcloneSmokeOptions {
                view_pose: Some(_),
                underwater_mode: XrUnderwaterMode::PerEye,
                debug_ui_screen: Some(XrDebugUiScreen::Pause),
                ..
            },
            ..
        }
    ));
}

#[test]
fn cli_rejects_no_window_without_desktop_xr() {
    let err = Cli::parse(["--xr-mclone-smoke".to_owned(), "--no-window".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(err.contains("--no-window requires --desktop-xr"));
}

#[test]
fn cli_rejects_desktop_xr_with_smoke_mode() {
    let err = Cli::parse(["--desktop-xr".to_owned(), "--xr-mclone-smoke".to_owned()])
        .unwrap_err()
        .to_string();

    assert!(err.contains("--xr-mclone-smoke cannot be combined"));
}

#[test]
fn cli_rejects_desktop_xr_with_headless_mode() {
    let err = Cli::parse([
        "--desktop-xr".to_owned(),
        "--headless-clear".to_owned(),
        "/tmp/mclone-clear.png".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("XR modes cannot be combined"));
}
