use super::*;
use mclone_app_runtime::startup_args::StartupSceneOptions;

#[test]
fn cli_parses_repeatable_worldgen_showcase_defaults() {
    let cli = Cli::parse([
        "--worldgen-showcase-card".to_owned(),
        "/tmp/mclone-worldgen-showcase".to_owned(),
        "--generation-profile".to_owned(),
        "small-island-v1".to_owned(),
        "--seed".to_owned(),
        "-42".to_owned(),
        "--render-distance".to_owned(),
        "6".to_owned(),
    ])
    .unwrap();

    let Cli::WorldgenShowcase { options } = cli else {
        panic!("expected worldgen showcase CLI mode");
    };
    assert_eq!(
        options.directory,
        PathBuf::from("/tmp/mclone-worldgen-showcase")
    );
    assert_eq!([options.width, options.height], [640, 400]);
    assert_eq!(options.scene.seed, -42);
    assert_eq!(options.scene.render_distance, 16);
    assert_eq!(
        options.scene.world_generation_profile,
        mclone_server::WorldGenerationProfile::SmallIslandV1
    );
    assert_eq!(options.scene.day_time_override, Some(6000));
    assert!(options.scene.freeze_time);
    assert!(!options.scene.debug_passive_showcase);
}

#[test]
fn cli_rejects_worldgen_showcase_for_persisted_or_remote_worlds() {
    for storage in [
        ["--world-dir", "/tmp/persisted"],
        ["--remote-addr", "127.0.0.1:25565"],
    ] {
        let error = Cli::parse([
            "--worldgen-showcase-card".to_owned(),
            "/tmp/mclone-worldgen-showcase".to_owned(),
            storage[0].to_owned(),
            storage[1].to_owned(),
        ])
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires a transient local integrated world")
        );
    }
}

#[test]
fn cli_parses_warm_world_swap_smoke_options() {
    let cli = Cli::parse([
        "--warm-world-swap-smoke".to_owned(),
        "/tmp/mclone-warm-world-swap".to_owned(),
        "--width".to_owned(),
        "640".to_owned(),
        "--height".to_owned(),
        "360".to_owned(),
        "--seed".to_owned(),
        "12345".to_owned(),
        "--warm-world-standby-seed".to_owned(),
        "67890".to_owned(),
        "--warm-world-standby-cadence".to_owned(),
        "5/5/5".to_owned(),
        "--warm-world-cost-sample-ms".to_owned(),
        "1000".to_owned(),
    ])
    .unwrap();

    let Cli::WarmWorldSwapSmoke { options } = cli else {
        panic!("expected warm-world swap smoke CLI mode");
    };
    assert_eq!(
        options.directory,
        PathBuf::from("/tmp/mclone-warm-world-swap")
    );
    assert_eq!([options.width, options.height], [640, 360]);
    assert_eq!(options.scene.seed, 12345);
    assert_eq!(options.scene.warm_world_standby_seed, Some(67890));
    assert_eq!(
        options.scene.warm_world_standby_cadence,
        Some(mclone_server::SimulationCadenceConfig::new(5, 5, 5))
    );
    assert_eq!(options.cost_sample_ms, 1000);
    assert!(options.scene.debug_auxiliary_player_script);
}

#[test]
fn cli_parses_live_diorama_smoke_options() {
    let cli = Cli::parse([
        "--live-diorama-smoke".to_owned(),
        "/tmp/live-diorama".to_owned(),
        "--world-dir".to_owned(),
        "/tmp/table-a".to_owned(),
        "--live-diorama-world-dir".to_owned(),
        "/tmp/island-b".to_owned(),
        "--width".to_owned(),
        "800".to_owned(),
        "--height".to_owned(),
        "600".to_owned(),
        "--live-diorama-soak-seconds".to_owned(),
        "600".to_owned(),
    ])
    .unwrap();

    let Cli::LiveDioramaSmoke { options } = cli else {
        panic!("expected live-diorama smoke CLI");
    };
    assert_eq!(options.directory, PathBuf::from("/tmp/live-diorama"));
    assert_eq!([options.width, options.height], [800, 600]);
    assert_eq!(options.soak_seconds, 600);
    assert_eq!(
        options
            .scene
            .live_diorama
            .expect("live diorama config")
            .world_dir,
        PathBuf::from("/tmp/island-b")
    );
}

#[test]
fn cli_lobby_scenario_smoke_uses_an_isolated_app_private_root() {
    let cli = Cli::parse([
        "--lobby-scenario-smoke".to_owned(),
        "/tmp/lobby-product".to_owned(),
        "--width".to_owned(),
        "640".to_owned(),
        "--height".to_owned(),
        "400".to_owned(),
    ])
    .unwrap();
    let Cli::LobbyScenarioSmoke { options } = cli else {
        panic!("expected lobby scenario smoke CLI");
    };
    assert_eq!(options.directory, PathBuf::from("/tmp/lobby-product"));
    assert_eq!([options.width, options.height], [640, 400]);
    assert_eq!(
        options.scene.world_root,
        Some(PathBuf::from("/tmp/lobby-product/app-data/worlds"))
    );
    assert!(options.scene.world_dir.is_none());
    assert!(options.scene.live_diorama.is_none());
    assert_eq!(options.preview_chunk_span, 2);
    assert!(!options.catalog_destination);
}

#[test]
fn cli_lobby_scenario_catalog_smoke_uses_a_four_chunk_catalog_destination() {
    let cli = Cli::parse([
        "--lobby-scenario-catalog-smoke".to_owned(),
        "/tmp/lobby-catalog".to_owned(),
    ])
    .unwrap();
    let Cli::LobbyScenarioCatalogSmoke { options } = cli else {
        panic!("expected catalog lobby scenario smoke CLI");
    };
    assert_eq!(options.directory, PathBuf::from("/tmp/lobby-catalog"));
    assert_eq!([options.width, options.height], [640, 400]);
    assert_eq!(
        options.scene.world_root,
        Some(PathBuf::from("/tmp/lobby-catalog/app-data/worlds"))
    );
    assert!(options.scene.world_dir.is_none());
    assert!(options.scene.live_diorama.is_none());
    assert_eq!(options.preview_chunk_span, 4);
    assert!(options.catalog_destination);
}

#[test]
fn cli_lobby_scenario_stereo_smoke_uses_an_isolated_app_private_root() {
    let cli = Cli::parse([
        "--lobby-scenario-stereo-smoke".to_owned(),
        "/tmp/lobby-stereo".to_owned(),
    ])
    .unwrap();
    let Cli::LobbyScenarioStereoSmoke { options } = cli else {
        panic!("expected lobby scenario stereo smoke CLI");
    };
    assert_eq!(options.directory, PathBuf::from("/tmp/lobby-stereo"));
    assert_eq!([options.width, options.height], [640, 640]);
    assert_eq!(
        options.scene.world_root,
        Some(PathBuf::from("/tmp/lobby-stereo/app-data/worlds"))
    );
    assert!(options.scene.world_dir.is_none());
    assert!(options.scene.live_diorama.is_none());
    assert_eq!(options.preview_chunk_span, 2);
    assert!(!options.catalog_destination);
}

#[test]
fn cli_rejects_live_diorama_soak_without_smoke() {
    let error =
        Cli::parse(["--live-diorama-soak-seconds".to_owned(), "600".to_owned()]).unwrap_err();
    assert!(error.to_string().contains("requires --live-diorama-smoke"));
}

#[test]
fn cli_requires_standby_seed_for_warm_world_swap_smoke() {
    let error = Cli::parse([
        "--warm-world-swap-smoke".to_owned(),
        "/tmp/mclone-warm-world-swap".to_owned(),
    ])
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("requires --warm-world-standby-seed")
    );
}

#[test]
fn cli_rejects_warm_world_cost_controls_outside_the_gate_smoke() {
    let cadence_error = Cli::parse([
        "--warm-world-standby-cadence".to_owned(),
        "5/5/5".to_owned(),
    ])
    .unwrap_err();
    assert!(
        cadence_error
            .to_string()
            .contains("requires --warm-world-standby-seed")
    );

    let sample_error =
        Cli::parse(["--warm-world-cost-sample-ms".to_owned(), "1000".to_owned()]).unwrap_err();
    assert!(
        sample_error
            .to_string()
            .contains("requires --warm-world-swap-smoke")
    );
}

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
                    startup: StartupSceneOptions {
                        seed: 54321,
                        chunk_x: 2,
                        chunk_z: -1,
                        local_entry_intent: mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::ExplicitCoordinate,
                        ..Default::default()
                    },
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                hud: false,
            },
        }
    );
}

#[test]
fn cli_parses_headless_dual_view_hud() {
    let cli = Cli::parse([
        "--headless-dual-view".to_owned(),
        "/tmp/mclone-dual-view-hud".to_owned(),
        "--headless-dual-view-hud".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();
    let Cli::HeadlessDualView { options } = cli else {
        panic!("expected headless dual-view CLI mode");
    };
    assert!(options.hud);
}

#[test]
fn cli_parses_xr_emulation_screenshot_and_keyboard_input() {
    let cli = Cli::parse([
        "--xr-emulation-screenshot".to_owned(),
        "/tmp/mclone-xr-emulation.png".to_owned(),
        "--width".to_owned(),
        "800".to_owned(),
        "--height".to_owned(),
        "700".to_owned(),
        "--xr-emulation-key".to_owned(),
        "KeyW".to_owned(),
        "--xr-emulation-key".to_owned(),
        "ArrowLeft".to_owned(),
        "--xr-emulation-input-frames".to_owned(),
        "12".to_owned(),
        "--xr-emulation-pause-panel".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::XrEmulationScreenshot {
            options: XrEmulationScreenshotOptions {
                path: PathBuf::from("/tmp/mclone-xr-emulation.png"),
                eye_width: 800,
                eye_height: 700,
                scene: SceneOptions {
                    startup: StartupSceneOptions::default().with_graphics_platform_profile(
                        mclone_app_runtime::graphics_preferences::ClientGraphicsPlatformProfile::DesktopOpenXr,
                    ),
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                held_keys: vec![
                    mclone_input::KeyboardKey::KeyW,
                    mclone_input::KeyboardKey::ArrowLeft,
                ],
                input_frames: 12,
                pause_panel: false,
                season_preview: Default::default(),
                celestial_debug: Default::default(),
            },
        }
    );
}

#[test]
fn cli_rejects_xr_emulation_input_without_capture_mode() {
    let error = Cli::parse(["--xr-emulation-key".to_owned(), "KeyW".to_owned()]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("XR emulation input options require --xr-emulation-screenshot")
    );
}

#[test]
fn cli_rejects_xr_emulation_input_frames_without_key() {
    let error = Cli::parse([
        "--xr-emulation-screenshot".to_owned(),
        "/tmp/mclone-xr-emulation.png".to_owned(),
        "--xr-emulation-input-frames".to_owned(),
        "4".to_owned(),
    ])
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("--xr-emulation-input-frames requires --xr-emulation-key")
    );
}

#[test]
fn cli_rejects_headless_dual_view_hud_without_dual_view_capture() {
    let error = Cli::parse([
        "--headless-dual-view-hud".to_owned(),
        "true".to_owned(),
        "--headless-clear".to_owned(),
        "/tmp/mclone-clear.png".to_owned(),
    ])
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("--headless-dual-view-hud requires --headless-dual-view")
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
                    startup: StartupSceneOptions {
                        seed: 99,
                        ..Default::default()
                    },
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
                    startup: StartupSceneOptions {
                        seed: 77,
                        lighting_enabled: false,
                        ..Default::default()
                    },
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
        "--screenshot-controller-focus".to_owned(),
        "true".to_owned(),
        "--screenshot-scripted-interaction".to_owned(),
        "true".to_owned(),
        "--screenshot-remote-settle-ms".to_owned(),
        "750".to_owned(),
        "--screenshot-settle-frames".to_owned(),
        "90".to_owned(),
        "--screenshot-terrain-horizon-diagnostic".to_owned(),
        "environmental-illumination".to_owned(),
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
                    startup: StartupSceneOptions {
                        remote_addr: Some("127.0.0.1:25565".to_owned()),
                        ..Default::default()
                    },
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
                worldgen_lens: None,
                player_collision_box: true,
                blink_debug: true,
                controller_focus: true,
                scripted_interaction: true,
                scripted_sleep: false,
                remote_settle_ms: 750,
                settle_frames: 90,
                terrain_horizon_diagnostic:
                    mclone_scene::TerrainHorizonDiagnostic::EnvironmentalIllumination,
                eye: Some([1.5, 62.25, -3.0]),
                target: Some([8.0, 64.0, 8.0]),
                season_preview: Default::default(),
                celestial_debug: Default::default(),
            },
        }
    );
}

#[test]
fn cli_parses_scripted_sleep_capture() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-sleep.png".to_owned(),
        "--screenshot-scripted-sleep".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected headless screenshot mode");
    };
    assert!(options.scripted_sleep);
    assert!(options.hud);
}

#[test]
fn cli_rejects_overlapping_scripted_interactions() {
    let error = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-sleep.png".to_owned(),
        "--screenshot-scripted-interaction".to_owned(),
        "true".to_owned(),
        "--screenshot-scripted-sleep".to_owned(),
        "true".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(error.contains("mutually exclusive"), "{error}");
}

#[test]
fn cli_parses_typed_season_preview_capture_options() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-season.png".to_owned(),
        "--season-preview".to_owned(),
        "true".to_owned(),
        "--season-orbital-phase".to_owned(),
        "0.25".to_owned(),
        "--season-latitude-source".to_owned(),
        "manual".to_owned(),
        "--season-latitude".to_owned(),
        "75".to_owned(),
        "--season-solar-time-source".to_owned(),
        "manual".to_owned(),
        "--season-solar-time".to_owned(),
        "23.5".to_owned(),
        "--season-recent-snow".to_owned(),
        "0.5".to_owned(),
        "--season-recent-snow-center".to_owned(),
        "1024,-2048".to_owned(),
    ])
    .unwrap();
    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected screenshot mode")
    };
    assert_eq!(
        options.season_preview,
        mclone_season::SeasonPreviewSettings {
            enabled: true,
            appearance_enabled: true,
            phase_source: mclone_season::SeasonPhaseSource::ManualPreview,
            orbital_phase: mclone_season::OrbitalPhase::NORTHERN_SOLSTICE,
            latitude_source: mclone_season::LatitudeSource::Manual,
            manual_latitude: mclone_season::PreviewLatitude::from_tenths_clamped(750),
            solar_time_source: mclone_season::SolarTimeSource::Manual,
            manual_solar_time: mclone_season::PreviewSolarTime::from_minutes_wrapped(1_410),
            recent_snow: Some(mclone_season::LocalSnowPulse {
                center_x: 1024,
                center_z: -2048,
                radius_blocks: mclone_season::RECENT_SNOW_RADIUS_BLOCKS,
                intensity: mclone_season::UnitU16::from_raw(32_768),
            }),
        }
    );

    for seasonal_args in [
        ["--season-preview", "true", "--season-appearance", "false"],
        ["--season-appearance", "false", "--season-preview", "true"],
    ] {
        let cli = Cli::parse(
            ["--screenshot", "/tmp/mclone-season-solar-only.png"]
                .into_iter()
                .chain(seasonal_args)
                .map(str::to_owned),
        )
        .unwrap();
        let Cli::HeadlessScreenshot { options } = cli else {
            panic!("expected screenshot mode")
        };
        assert!(options.season_preview.enabled);
        assert!(!options.season_preview.appearance_enabled);
    }

    let error = Cli::parse(["--season-preview".to_owned(), "true".to_owned()]).unwrap_err();
    assert!(error.to_string().contains("require --screenshot"));

    let error = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-season.png".to_owned(),
        "--season-recent-snow-center".to_owned(),
        "0,0".to_owned(),
    ])
    .unwrap_err();
    assert!(error.to_string().contains("requires --season-recent-snow"));
}

#[test]
fn cli_parses_every_typed_celestial_capture_control() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-celestial.png".to_owned(),
        "--celestial-sun".to_owned(),
        "false".to_owned(),
        "--celestial-sun-halo".to_owned(),
        "false".to_owned(),
        "--celestial-horizon-glow".to_owned(),
        "false".to_owned(),
        "--celestial-moon".to_owned(),
        "false".to_owned(),
        "--celestial-moonlight".to_owned(),
        "false".to_owned(),
        "--celestial-stars".to_owned(),
        "quarter".to_owned(),
        "--moon-phase-source".to_owned(),
        "manual-preview".to_owned(),
        "--moon-phase".to_owned(),
        "0.25".to_owned(),
    ])
    .unwrap();
    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected screenshot mode")
    };
    assert_eq!(
        options.celestial_debug,
        mclone_season::CelestialDebugSettings {
            sun_body_enabled: false,
            sun_halo_enabled: false,
            horizon_glow_enabled: false,
            moon_body_enabled: false,
            moonlight_enabled: false,
            moon_phase_source: mclone_season::MoonPhaseSource::ManualPreview,
            manual_lunar_phase: mclone_season::LunarPhase::FIRST_QUARTER,
            star_density: mclone_season::CelestialStarDensity::Quarter,
        }
    );

    let error = Cli::parse(["--celestial-stars".to_owned(), "off".to_owned()]).unwrap_err();
    assert!(error.to_string().contains("require --screenshot"));
}

#[test]
fn cli_rejects_unknown_terrain_horizon_diagnostic() {
    let error = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--screenshot-terrain-horizon-diagnostic".to_owned(),
        "ambient-ish".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(error.contains("environmental-illumination"));
    assert!(error.contains("ambient-ish"));
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
fn cli_parses_worldgen_lens_screenshot_layer() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-worldgen-lens.png".to_owned(),
        "--screenshot-worldgen-lens".to_owned(),
        "hydrology".to_owned(),
        "--screenshot-debug-pane".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    let Cli::HeadlessScreenshot { options } = cli else {
        panic!("expected screenshot CLI");
    };
    assert_eq!(
        options.worldgen_lens,
        Some(mclone_scene::WorldgenLensLayer::Hydrology)
    );
    assert!(options.hud);
    assert!(options.debug_pane);
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
        parse_screenshot_ui_arg("--screenshot-ui", Some("death".to_owned())).unwrap(),
        HeadlessScreenshotUi::Death
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
        parse_screenshot_ui_arg("--screenshot-ui", Some("options-local-play".to_owned()),).unwrap(),
        HeadlessScreenshotUi::OptionsLocalPlayPause
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("options-seasonal-debug".to_owned()),)
            .unwrap(),
        HeadlessScreenshotUi::OptionsSeasonalDebugPause
    );
    assert_eq!(
        parse_screenshot_ui_arg(
            "--screenshot-ui",
            Some("options-celestial-debug".to_owned()),
        )
        .unwrap(),
        HeadlessScreenshotUi::OptionsCelestialDebugPause
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("storage-profile-title".to_owned()))
            .unwrap(),
        HeadlessScreenshotUi::StorageProfileTitle
    );
    assert_eq!(
        parse_screenshot_ui_arg(
            "--screenshot-ui",
            Some("storage-factory-confirm".to_owned()),
        )
        .unwrap(),
        HeadlessScreenshotUi::StorageFactoryConfirm
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("server-settings-pause".to_owned()))
            .unwrap(),
        HeadlessScreenshotUi::ServerSettingsPause
    );
    assert_eq!(
        parse_screenshot_ui_arg("--screenshot-ui", Some("asset-packs-pause".to_owned())).unwrap(),
        HeadlessScreenshotUi::AssetPacksPause
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
                startup: StartupSceneOptions {
                    seed: -9,
                    chunk_x: 2,
                    chunk_z: -3,
                    local_entry_intent: mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::ExplicitCoordinate,
                    ..Default::default()
                },
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}
