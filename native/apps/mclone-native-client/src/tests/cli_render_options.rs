use super::*;

#[test]
fn cli_parses_render_distance() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--render-distance".to_owned(),
        "32".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                render_distance: 32,
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}

#[test]
fn cli_parses_render_compile_workers() {
    let cli = Cli::parse([
        "--movement-perf".to_owned(),
        "--render-compile-workers".to_owned(),
        "2".to_owned(),
        "--render-compile-max-pending-jobs".to_owned(),
        "6".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::MovementPerf {
            options: MovementPerfOptions {
                scene: SceneOptions {
                    render_compile_worker_count: 2,
                    render_compile_max_pending_jobs: Some(6),
                    ..SceneOptions::default()
                },
                ..MovementPerfOptions::default()
            },
        }
    );
}

#[test]
fn cli_parses_far_lod_opt_in() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--far-lod".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                far_lod: mclone_app_runtime::far_lod::FarTerrainLodConfig::enabled(),
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}

#[test]
fn cli_parses_movement_speed_multiplier() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--movement-speed-multiplier".to_owned(),
        "2.25".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                movement_speed_multiplier: 2.25,
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}

#[test]
fn cli_parses_simulation_cadence() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--simulation-cadence".to_owned(),
        "60/20/60".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                simulation_cadence: mclone_server::SimulationCadenceConfig::new(60, 20, 60),
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );

    let cli = Cli::parse(["--cadence".to_owned(), "60/60/120".to_owned()]).unwrap();
    let Cli::Window { scene, .. } = cli else {
        panic!("expected window cli");
    };
    assert_eq!(
        scene.simulation_cadence,
        mclone_server::SimulationCadenceConfig::new(60, 60, 120)
    );
}

#[test]
fn cli_rejects_invalid_simulation_cadence() {
    let err = Cli::parse(["--simulation-cadence".to_owned(), "30/20/60".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("clean integer relationships"));

    let err = Cli::parse(["--simulation-cadence".to_owned(), "60/20".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("HOST/GAMEPLAY/PHYSICS"));

    let err = Cli::parse([
        "--remote-addr".to_owned(),
        "127.0.0.1:25565".to_owned(),
        "--simulation-cadence".to_owned(),
        "60/20/60".to_owned(),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("local integrated worlds"));
}

#[test]
fn cli_parses_first_person_player_visibility() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--first-person-player".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions {
                first_person_player_visible: true,
                ..SceneOptions::default()
            },
            TexturedSectionRenderOptions::default(),
        )
    );
}

#[test]
fn cli_rejects_render_distance_below_java_minimum() {
    let err = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--render-distance".to_owned(),
        "1".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(err.contains("--render-distance must be between 2 and 32"));
}

#[test]
fn cli_parses_section_occlusion_toggle() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--section-occlusion".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions::default(),
            TexturedSectionRenderOptions {
                section_occlusion_culling: false,
                ..TexturedSectionRenderOptions::default()
            },
        )
    );

    let err = Cli::parse(["--disable-section-occlusion".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown argument"));
}

#[test]
fn cli_parses_fullbright_toggle() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--fullbright".to_owned(),
        "true".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions::default(),
            TexturedSectionRenderOptions {
                force_fullbright: true,
                ..TexturedSectionRenderOptions::default()
            },
        )
    );

    let cli = Cli::parse(["--fullbright".to_owned(), "false".to_owned()]).unwrap();

    assert_eq!(
        cli,
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
fn cli_parses_render_color_profile() {
    let cli = Cli::parse([
        "--screenshot".to_owned(),
        "/tmp/mclone-frame.png".to_owned(),
        "--render-color-profile".to_owned(),
        "stylized-bright".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        screenshot_cli(
            "/tmp/mclone-frame.png",
            SceneOptions::default(),
            TexturedSectionRenderOptions {
                color_profile: RenderColorProfile::StylizedBright,
                ..TexturedSectionRenderOptions::default()
            },
        )
    );

    let err = Cli::parse(["--render-color-profile".to_owned(), "neon".to_owned()])
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected vanilla"));
}

#[test]
fn cli_parses_lighting_false_as_runtime_bypass() {
    let cli = Cli::parse(["--lighting".to_owned(), "false".to_owned()]).unwrap();

    assert_eq!(
        cli,
        Cli::Window {
            scene: SceneOptions {
                lighting_enabled: false,
                ..SceneOptions::default()
            },
            render_options: TexturedSectionRenderOptions {
                force_fullbright: true,
                ..TexturedSectionRenderOptions::default()
            },
            start_intent: WindowStartIntent::InWorld,
            startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            frame_report: None,
        }
    );

    let cli = Cli::parse([
        "--lighting".to_owned(),
        "false".to_owned(),
        "--fullbright".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::Window {
            scene: SceneOptions {
                lighting_enabled: false,
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
fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
    let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
    block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(0, 0),
        ChunkStatus::Surface,
        ChunkRevision(1),
        0,
        32,
        &block_state_ids,
    );

    let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

    assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
    assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
    assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
}
