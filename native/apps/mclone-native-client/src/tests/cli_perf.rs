use super::*;

#[test]
fn cli_parses_movement_perf_options() {
    let cli = Cli::parse([
        "--movement-perf".to_owned(),
        "--movement-steps".to_owned(),
        "6".to_owned(),
        "--path-radius".to_owned(),
        "3".to_owned(),
        "--render-distance".to_owned(),
        "2".to_owned(),
        "--width".to_owned(),
        "800".to_owned(),
        "--height".to_owned(),
        "600".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::MovementPerf {
            options: MovementPerfOptions {
                scene: SceneOptions {
                    render_distance: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 800,
                height: 600,
                steps: 6,
                path_radius_chunks: 3,
            },
        }
    );
}

#[test]
fn cli_parses_loading_settle_perf_options() {
    let cli = Cli::parse([
        "--loading-settle-perf".to_owned(),
        "--loading-settle-distances".to_owned(),
        "5,10,30".to_owned(),
        "--seed".to_owned(),
        "99".to_owned(),
        "--render-compile-workers".to_owned(),
        "2".to_owned(),
        "--debug-passive-showcase".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::LoadingSettlePerf {
            options: LoadingSettlePerfOptions {
                scene: SceneOptions {
                    seed: 99,
                    render_compile_worker_count: 2,
                    debug_passive_showcase: false,
                    ..SceneOptions::default()
                },
                distances: vec![5, 10, 30],
            },
        }
    );
}

#[test]
fn cli_rejects_invalid_loading_settle_distances() {
    let err = Cli::parse([
        "--loading-settle-perf".to_owned(),
        "--loading-settle-distances".to_owned(),
        "5,5".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("duplicate distance 5"));

    let err = Cli::parse([
        "--loading-settle-perf".to_owned(),
        "--loading-settle-distances".to_owned(),
        "33".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("distances must be between 2 and 32"));
}

#[test]
fn cli_parses_startup_streaming_perf_options() {
    let cli = Cli::parse([
        "--startup-streaming-perf".to_owned(),
        "--startup-streaming-frames".to_owned(),
        "30000".to_owned(),
        "--target-hz".to_owned(),
        "120".to_owned(),
        "--render-distance".to_owned(),
        "20".to_owned(),
        "--render-compile-workers".to_owned(),
        "2".to_owned(),
        "--width".to_owned(),
        "1024".to_owned(),
        "--height".to_owned(),
        "768".to_owned(),
        "--debug-passive-showcase".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::StartupStreamingPerf {
            options: StartupStreamingPerfOptions {
                scene: SceneOptions {
                    render_distance: 20,
                    render_compile_worker_count: 2,
                    debug_passive_showcase: false,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 1024,
                height: 768,
                frames: 30000,
                target_hz: 120.0,
                persisted_world: false,
            },
        }
    );
}

#[test]
fn cli_parses_adaptive_chunk_publication_budget_flag() {
    let cli = Cli::parse([
        "--startup-streaming-perf".to_owned(),
        "--adaptive-chunk-publication-budget".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::StartupStreamingPerf {
            options: StartupStreamingPerfOptions {
                scene: SceneOptions {
                    adaptive_chunk_publication_budget: true,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 1280,
                height: 720,
                frames: DEFAULT_STARTUP_STREAMING_PERF_FRAMES,
                target_hz: DEFAULT_FRAME_BUDGET_TARGET_HZ,
                persisted_world: false,
            },
        }
    );

    let cli = Cli::parse([
        "--startup-streaming-perf".to_owned(),
        "--adaptive-chunk-publication-budget".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    let Cli::StartupStreamingPerf { options } = cli else {
        panic!("expected startup streaming perf");
    };
    assert!(!options.scene.adaptive_chunk_publication_budget);
}

#[test]
fn cli_parses_adaptive_render_admission_budget_flag() {
    let cli = Cli::parse([
        "--adaptive-render-admission-budget".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    let Cli::Window { scene, .. } = cli else {
        panic!("expected window mode");
    };
    assert!(scene.adaptive_render_admission_budget);

    let cli = Cli::parse([
        "--adaptive-render-admission-budget".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();
    let Cli::Window { scene, .. } = cli else {
        panic!("expected window mode");
    };
    assert!(!scene.adaptive_render_admission_budget);
}

#[test]
fn cli_rejects_adaptive_render_admission_budget_for_remote_sessions() {
    let err = Cli::parse([
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
        "--adaptive-render-admission-budget".to_owned(),
        "true".to_owned(),
    ])
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("--adaptive-render-admission-budget applies only to local integrated worlds")
    );
}

#[test]
fn cli_allows_startup_streaming_target_hz_before_mode_flag() {
    let cli = Cli::parse([
        "--target-hz".to_owned(),
        "90".to_owned(),
        "--startup-streaming-frames".to_owned(),
        "120".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::StartupStreamingPerf {
            options: StartupStreamingPerfOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                width: 1280,
                height: 720,
                frames: 120,
                target_hz: 90.0,
                persisted_world: false,
            },
        }
    );
}

#[test]
fn cli_parses_startup_streaming_persisted_world() {
    let cli = Cli::parse([
        "--startup-streaming-persisted-world".to_owned(),
        "--startup-streaming-frames".to_owned(),
        "120".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::StartupStreamingPerf {
            options: StartupStreamingPerfOptions {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                width: 1280,
                height: 720,
                frames: 120,
                target_hz: DEFAULT_FRAME_BUDGET_TARGET_HZ,
                persisted_world: true,
            },
        }
    );
}

#[test]
fn cli_rejects_startup_streaming_with_other_perf_modes() {
    let err = Cli::parse([
        "--startup-streaming-perf".to_owned(),
        "--loading-settle-perf".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("mutually exclusive"));

    let err = Cli::parse([
        "--frame-budget-frames".to_owned(),
        "10".to_owned(),
        "--startup-streaming-perf".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("mutually exclusive"));
}

#[test]
fn cli_parses_timedemo_options() {
    let cli = Cli::parse([
        "--timedemo".to_owned(),
        "--timedemo-frames".to_owned(),
        "48".to_owned(),
        "--path-radius".to_owned(),
        "5".to_owned(),
        "--frame-accounting".to_owned(),
        "false".to_owned(),
        "--render-distance".to_owned(),
        "2".to_owned(),
        "--width".to_owned(),
        "1024".to_owned(),
        "--height".to_owned(),
        "768".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::Timedemo {
            options: TimedemoOptions {
                scene: SceneOptions {
                    render_distance: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 1024,
                height: 768,
                frames: 48,
                path_radius_chunks: 5,
            },
        }
    );
}

#[test]
fn cli_parses_frame_budget_probe_options() {
    let cli = Cli::parse([
        "--frame-budget-probe".to_owned(),
        "--frame-budget-frames".to_owned(),
        "36".to_owned(),
        "--target-hz".to_owned(),
        "120".to_owned(),
        "--path-radius".to_owned(),
        "5".to_owned(),
        "--frame-accounting".to_owned(),
        "false".to_owned(),
        "--render-distance".to_owned(),
        "2".to_owned(),
        "--width".to_owned(),
        "1024".to_owned(),
        "--height".to_owned(),
        "768".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::FrameBudgetProbe {
            options: FrameBudgetProbeOptions {
                scene: SceneOptions {
                    render_distance: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 1024,
                height: 768,
                mode: FrameBudgetProbeMode::StressOrbit,
                frames: 36,
                path_radius_chunks: 5,
                target_hz: 120.0,
                movement_speed: SPECTATOR_BASE_SPEED,
                frame_accounting_enabled: false,
            },
        }
    );
}

#[test]
fn cli_target_hz_selects_frame_budget_probe() {
    let cli = Cli::parse(["--target-hz".to_owned(), "90".to_owned()]).unwrap();

    assert_eq!(
        cli,
        Cli::FrameBudgetProbe {
            options: FrameBudgetProbeOptions {
                target_hz: 90.0,
                ..FrameBudgetProbeOptions::default()
            },
        }
    );
}

#[test]
fn cli_parses_movement_frame_probe_options() {
    let cli = Cli::parse([
        "--target-hz".to_owned(),
        "120".to_owned(),
        "--movement-frame-probe".to_owned(),
        "--frame-budget-frames".to_owned(),
        "36".to_owned(),
        "--movement-frame-speed".to_owned(),
        "48".to_owned(),
        "--path-radius".to_owned(),
        "5".to_owned(),
        "--render-distance".to_owned(),
        "2".to_owned(),
        "--width".to_owned(),
        "1024".to_owned(),
        "--height".to_owned(),
        "768".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::FrameBudgetProbe {
            options: FrameBudgetProbeOptions {
                scene: SceneOptions {
                    render_distance: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                width: 1024,
                height: 768,
                mode: FrameBudgetProbeMode::MovementWalk,
                frames: 36,
                path_radius_chunks: 5,
                target_hz: 120.0,
                movement_speed: 48.0,
                frame_accounting_enabled: true,
            },
        }
    );
}

#[test]
fn cli_parses_remote_addr() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                remote_addr: Some("127.0.0.1:25565".to_owned()),
                adaptive_chunk_publication_budget: false,
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}
