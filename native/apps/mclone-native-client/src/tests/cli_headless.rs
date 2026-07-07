use super::*;

#[test]
fn cli_parses_headless_dual_view_scene_options() {
    let cli = Cli::parse([
        "--headless-dual-view".to_owned(),
        "/tmp/mclone-dual-view".to_owned(),
        "--width".to_owned(),
        "960".to_owned(),
        "--height".to_owned(),
        "640".to_owned(),
        "--seed".to_owned(),
        "54321".to_owned(),
        "--chunk-x".to_owned(),
        "2".to_owned(),
        "--chunk-z".to_owned(),
        "-1".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessDualView {
            options: HeadlessDualViewOptions {
                directory: PathBuf::from("/tmp/mclone-dual-view"),
                width: 960,
                height: 640,
                scene: SceneOptions {
                    seed: 54321,
                    chunk_x: 2,
                    chunk_z: -1,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            },
        }
    );
}

#[test]
fn cli_parses_renderer_rebuild_smoke_options() {
    let cli = Cli::parse([
        "--renderer-rebuild-smoke".to_owned(),
        "/tmp/mclone-render-rebuild".to_owned(),
        "--width".to_owned(),
        "640".to_owned(),
        "--height".to_owned(),
        "360".to_owned(),
        "--seed".to_owned(),
        "99".to_owned(),
        "--render-color-profile".to_owned(),
        "stylized-bright".to_owned(),
        "--rebuild-render-scale".to_owned(),
        "0.5".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::RendererRebuildSmoke {
            options: RendererRebuildSmokeOptions {
                directory: PathBuf::from("/tmp/mclone-render-rebuild"),
                width: 640,
                height: 360,
                scene: SceneOptions {
                    seed: 99,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions {
                    color_profile: RenderColorProfile::StylizedBright,
                    ..TexturedSectionRenderOptions::default()
                },
                rebuild_render_scale: Some(0.5),
            },
        }
    );
}

#[test]
fn cli_parses_torch_light_probe_options() {
    let cli = Cli::parse([
        "--torch-light-probe".to_owned(),
        "/tmp/mclone-torch-light-probe".to_owned(),
        "--width".to_owned(),
        "640".to_owned(),
        "--height".to_owned(),
        "360".to_owned(),
        "--fullbright".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::TorchLightProbe {
            options: TorchLightProbeOptions {
                directory: PathBuf::from("/tmp/mclone-torch-light-probe"),
                width: 640,
                height: 360,
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: false,
                    ..TexturedSectionRenderOptions::default()
                },
            },
        }
    );
}

#[test]
fn cli_parses_remote_player_visual_smoke_options() {
    let cli = Cli::parse([
        "--remote-player-visual-smoke".to_owned(),
        "/tmp/mclone-remote-player-visual-smoke.png".to_owned(),
        "--width".to_owned(),
        "800".to_owned(),
        "--height".to_owned(),
        "450".to_owned(),
        "--seed".to_owned(),
        "77".to_owned(),
        "--lighting".to_owned(),
        "false".to_owned(),
        "--fullbright".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::RemotePlayerVisualSmoke {
            options: RemotePlayerVisualSmokeOptions {
                path: PathBuf::from("/tmp/mclone-remote-player-visual-smoke.png"),
                width: 800,
                height: 450,
                scene: SceneOptions {
                    seed: 77,
                    lighting_enabled: false,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            },
        }
    );
}

#[test]
fn cli_parses_headless_clear_dimensions_after_path() {
    let cli = Cli::parse([
        "--headless-clear".to_owned(),
        "/tmp/mclone.png".to_owned(),
        "--width".to_owned(),
        "32".to_owned(),
        "--height".to_owned(),
        "16".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessClear {
            path: PathBuf::from("/tmp/mclone.png"),
            width: 32,
            height: 16,
        }
    );
}

#[test]
fn cli_parses_dimensions_before_headless_path() {
    let cli = Cli::parse([
        "--width".to_owned(),
        "32".to_owned(),
        "--height".to_owned(),
        "16".to_owned(),
        "--headless-clear".to_owned(),
        "/tmp/mclone.png".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessClear {
            path: PathBuf::from("/tmp/mclone.png"),
            width: 32,
            height: 16,
        }
    );
}

#[test]
fn cli_rejects_retired_narrow_headless_modes() {
    for flag in [
        "--headless-ui",
        "--headless-chunk",
        "--headless-chunk-scenarios",
    ] {
        let err = Cli::parse([flag.to_owned(), "/tmp/retired.png".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown argument"), "{flag}: {err}");
    }
}

#[test]
fn cli_parses_full_frame_screenshot_options() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--width".to_owned(),
        "960".to_owned(),
        "--height".to_owned(),
        "540".to_owned(),
        "--screenshot-ui".to_owned(),
        "pause".to_owned(),
        "--screenshot-hud".to_owned(),
        "true".to_owned(),
        "--screenshot-debug-pane".to_owned(),
        "true".to_owned(),
        "--screenshot-player-box".to_owned(),
        "true".to_owned(),
        "--screenshot-blink-debug".to_owned(),
        "true".to_owned(),
        "--screenshot-scripted-interaction".to_owned(),
        "true".to_owned(),
        "--screenshot-remote-settle-ms".to_owned(),
        "750".to_owned(),
        "--screenshot-eye".to_owned(),
        "1.5,62.25,-3".to_owned(),
        "--screenshot-target".to_owned(),
        "8,64,8".to_owned(),
        "--screenshot-camera-view".to_owned(),
        "third-person".to_owned(),
        "--fullbright".to_owned(),
        "true".to_owned(),
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessScreenshot {
            options: HeadlessScreenshotOptions {
                path: PathBuf::from("/tmp/mclone-frame.png"),
                width: 960,
                height: 540,
                scene: SceneOptions {
                    remote_addr: Some("127.0.0.1:25565".to_owned()),
                    adaptive_chunk_publication_budget: false,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
                startup_wait: StartupWaitPolicy::OFFSCREEN_SCREENSHOT_DEFAULT,
                camera_view: mclone_render_session::EngineCameraViewMode::ThirdPersonBack,
                ui: HeadlessScreenshotUi::Pause,
                hud: true,
                frame_pipeline_overlay: false,
                debug_pane: true,
                player_collision_box: true,
                blink_debug: true,
                scripted_interaction: true,
                remote_settle_ms: 750,
                eye: Some([1.5, 62.25, -3.0]),
                target: Some([8.0, 64.0, 8.0]),
            },
        }
    );
}

#[test]
fn cli_parses_frame_pipeline_overlay_screenshot_flag() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame-pipeline.png".to_owned(),
        "--screenshot-frame-pipeline-overlay".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected screenshot CLI");
    };
    assert!(options.hud);
    assert!(options.frame_pipeline_overlay);
}

#[test]
fn parse_screenshot_ui_accepts_named_screens() {
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("none".to_owned())).unwrap(),
        HeadlessScreenshotUi::None
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("title".to_owned())).unwrap(),
        HeadlessScreenshotUi::Title
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("world-list".to_owned())).unwrap(),
        HeadlessScreenshotUi::WorldList
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("world-create".to_owned())).unwrap(),
        HeadlessScreenshotUi::WorldCreate
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("world-delete-confirm".to_owned()))
            .unwrap(),
        HeadlessScreenshotUi::WorldDeleteConfirm
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("new-world".to_owned())).unwrap(),
        HeadlessScreenshotUi::NewWorld
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("join-remote".to_owned())).unwrap(),
        HeadlessScreenshotUi::JoinRemote
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("help".to_owned())).unwrap(),
        HeadlessScreenshotUi::Help
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("controls".to_owned())).unwrap(),
        HeadlessScreenshotUi::Help
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("block-palette".to_owned())).unwrap(),
        HeadlessScreenshotUi::BlockPalette
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("options-title".to_owned())).unwrap(),
        HeadlessScreenshotUi::OptionsTitle
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("options".to_owned())).unwrap(),
        HeadlessScreenshotUi::OptionsPause
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("server-settings-pause".to_owned()))
            .unwrap(),
        HeadlessScreenshotUi::ServerSettingsPause
    );
    assert!(parse_screenshot_ui_arg("--screenshot-ui", Some("bad".to_owned())).is_err());
}

#[test]
fn cli_parses_screenshot_scene_options() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--seed".to_owned(),
        "-9".to_owned(),
        "--chunk-x".to_owned(),
        "2".to_owned(),
        "--chunk-z".to_owned(),
        "-3".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                seed: -9,
                chunk_x: 2,
                chunk_z: -3,
                render_distance: DEFAULT_RENDER_DISTANCE,
                render_compile_worker_count:
                    mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
                remote_addr: None,
                world_root: SceneOptions::default().world_root,
                world_dir: None,
                day_time_override: None,
                freeze_time: false,
                movement_speed_multiplier: 1.0,
                simulation_cadence: mclone_server::SimulationCadenceConfig::default(),
                first_person_player_visible: false,
                debug_passive_showcase: true,
                lighting_enabled: true,
                adaptive_chunk_publication_budget: true,
                far_lod: Default::default(),
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}
