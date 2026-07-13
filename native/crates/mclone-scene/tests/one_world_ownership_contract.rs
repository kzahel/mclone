//! Tactical 174 Slice 1 characterization locks for the flattened one-world
//! scene owner.
//!
//! These tests intentionally describe the pre-extraction installation and
//! reset seams. Slice 2 may update their source shape only while preserving
//! the behavioral groups they name.

use std::path::{Path, PathBuf};

fn scene_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(scene_root().join(relative)).expect("scene source file readable")
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

fn assert_in_order(item: &str, markers: &[&str]) {
    let mut remainder = item;
    for marker in markers {
        let index = remainder
            .find(marker)
            .unwrap_or_else(|| panic!("missing ordered marker `{marker}`"));
        remainder = &remainder[index + marker.len()..];
    }
}

const FLAT_HOST_FIELDS: &[&str] = &[
    "scene",
    "services",
    "color_format",
    "mesh_assets",
    "active_assets",
    "asset_replacement",
    "asset_replacement_status",
    "last_asset_replacement_commit",
    "asset_replacement_started_at",
    "asset_replacement_assets_ready_at",
    "asset_pack_sources",
    "asset_pack_preference",
    "asset_pack_preference_storage",
    "asset_pack_preference_error",
    "pending_restored_asset_pack_selection",
    "external_asset_pack_preparation",
    "pending_external_asset_pack_selection",
    "runtime",
    "local_startup",
    "external_runtime_startup_pending",
    "session",
    "session_runtime_factory",
    "client_experience",
    "camera",
    "interaction",
    "initial_alignment_mode",
    "render_options",
    "player_collision_box_visible",
    "crosshair_visible",
    "travel_assist_mode",
    "player_model",
    "draw",
    "traversal_ready_sections",
    "section_uploads",
    "actors",
    "far_lod",
    "selection_outline",
    "world_gui_renderer",
    "world_gui_overlay_renderer",
    "mono_gui",
    "mono_ui_context",
    "diagnostic_panel",
    "ui",
    "menu_overlay_cache",
    "status_overlay",
    "sky",
    "screen_effects",
    "underwater_effects",
    "last_underwater_update",
    "head_comfort",
    "render_stats",
    "tracking_origin",
    "locomotion_mode",
    "turn_policy",
    "snap_turn_state",
    "blink_teleport",
    "mono_blink_debug",
    "display_refresh_hz",
    "render_admission_policy",
    "render_split_timing_enabled",
    "defer_eye_waits_enabled",
    "overlap_runtime_prefetch_enabled",
    "prefetched_live_upload",
    "render_section_upload_budget",
    "render_section_accept_budget",
    "render_completed_result_accept_budget",
    "per_view_uniform_frame",
    "last_locomotion_update",
    "menu_toggle_down",
    "game_ui_toggle_down",
    "menu_pointer_down",
    "gameplay_interaction_buttons",
    "menu_panel_pose",
    "menu_panel_anchor",
    "menu_panel_recenter_pending",
    "latest_controllers",
    "first_eye_summary",
    "last_ui_panel_stats",
    "last_ui_draw_cache_stats",
    "rendered_frames",
    "seed_reroll",
];

#[test]
fn flattened_host_field_inventory_stays_explicit_until_slot_extraction() {
    let source = read("src/lib.rs");
    let host = braced_item(&source, "pub struct McloneSceneHost {");
    let fields = host
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (name, _) = line.split_once(':')?;
            (!name.contains(' ') && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()))
                .then_some(name)
        })
        .collect::<Vec<_>>();

    assert_eq!(fields, FLAT_HOST_FIELDS);
    assert_eq!(fields.len(), 81);
}

#[test]
fn local_startup_installs_one_coherent_drawable_world() {
    let source = read("src/session.rs");
    let install = braced_item(&source, "pub(crate) fn complete_local_startup(");

    assert!(install.contains("let sections = startup_sections;"));
    assert!(install.contains("TexturedSectionDrawResources::new("));
    assert!(install.contains("&sections,"));
    assert!(install.contains("draw.set_traversal_ready_sections("));
    assert!(!install.contains("section_count: draw.section_count()"));
    assert_in_order(
        install,
        &[
            "let section_count = draw.section_count();",
            "let index_count = draw.index_count();",
            "self.scene = scene;",
            "self.mesh_assets = runtime.mesh_assets().clone();",
            "self.runtime = Some(runtime);",
            "self.camera = camera;",
            "self.draw = draw;",
            "self.render_stats = RenderStreamStats {",
            "section_count,",
            "index_count,",
            "face_count,",
            "self.clear_transient_world_state();",
            "self.session.complete_start(descriptor.clone());",
        ],
    );

    // The startup seed still bypasses the budgeted upload coordinator. Slice 4
    // must add an explicit zero-compile-grant cache-update conversion.
    assert!(!install.contains("enqueue_cache_update"));
    assert!(!install.contains("drain_budgeted"));
}

#[test]
fn external_and_native_replacement_installs_keep_the_same_core_cluster() {
    let source = read("src/session.rs");
    let external = braced_item(&source, "pub fn complete_external_session_start(");
    assert_in_order(
        external,
        &[
            "self.draw = TexturedSectionDrawResources::new(",
            "self.scene = pending.scene;",
            "self.runtime = Some(runtime);",
            "self.external_runtime_startup_pending = true;",
            "self.camera = camera;",
            "self.render_stats = RenderStreamStats::default();",
            "self.clear_transient_world_state();",
            "self.session.complete_start(active.clone());",
        ],
    );

    let replacement = braced_item(&source, "pub(crate) fn start_replacement_session(");
    assert_in_order(
        replacement,
        &[
            "let started = match factory.start(",
            "self.scene = scene;",
            "self.mesh_assets = started.runtime.mesh_assets().clone();",
            "self.runtime = Some(started.runtime);",
            "self.camera = started.camera;",
            "self.draw = started.draw;",
            "self.render_stats = started.render_stats;",
            "self.clear_transient_world_state();",
        ],
    );

    let staged = braced_item(&source, "pub(crate) struct StartedSceneRuntime {");
    for field in ["runtime", "camera", "draw", "render_stats"] {
        assert!(staged.contains(&format!("{field}:")));
    }
    assert!(
        source[..source
            .find("pub(crate) struct StartedSceneRuntime")
            .unwrap()]
            .ends_with("#[cfg(not(target_arch = \"wasm32\"))]\n")
    );
}

#[test]
fn current_world_reset_mixes_slot_and_physical_transients() {
    let source = read("src/session.rs");
    let reset = braced_item(&source, "pub(crate) fn clear_transient_world_state(");

    for slot_state in [
        "underwater_effects",
        "last_underwater_update",
        "prefetched_live_upload",
        "traversal_ready_sections",
        "section_uploads",
        "render_admission_policy",
    ] {
        assert!(
            reset.contains(slot_state),
            "missing slot reset `{slot_state}`"
        );
    }
    for physical_state in [
        "tracking_origin",
        "last_locomotion_update",
        "head_comfort",
        "blink_teleport",
        "latest_controllers",
        "first_eye_summary",
        "rendered_frames",
    ] {
        assert!(
            reset.contains(physical_state),
            "missing physical reset `{physical_state}`"
        );
    }
}

#[test]
fn asset_epoch_replacement_preserves_world_identity_and_resets_streaming() {
    let source = read("src/asset_replacement.rs");
    let commit = braced_item(&source, "fn commit_asset_replacement(");

    for snapshot in [
        "let session_before = self.session.state().clone();",
        "let camera_before = self.camera.snapshot();",
        "let command_count_before = runtime.core().command_count();",
        "let update_count_before = runtime.core().update_count();",
    ] {
        assert!(commit.contains(snapshot));
    }
    assert_in_order(
        commit,
        &[
            "runtime.replace_asset_epoch(",
            "self.mesh_assets = assets.mesh.clone();",
            "self.draw = draw;",
            "self.active_assets = assets;",
            "self.traversal_ready_sections.clear();",
            "self.section_uploads.clear();",
            "self.prefetched_live_upload = None;",
            "epoch: self.active_assets.epoch,",
            "section_count: sections.sections.len(),",
            "session_preserved:",
            "camera_preserved:",
            "command_count_unchanged:",
            "update_count_unchanged:",
        ],
    );
}

#[test]
#[ignore = "GPU characterization; run explicitly on a host with the extracted assets"]
fn empty_terrain_shell_reports_real_atlas_and_lazy_multiview_cost() -> anyhow::Result<()> {
    use std::time::Instant;

    use mclone_app_runtime::render_assets::load_textured_mesh_assets;
    use mclone_render::chunk::{
        ChunkCamera, ChunkMultiviewDepthTarget, ChunkMultiviewRenderTarget,
        TexturedSectionDrawResources, TexturedSectionRenderOptions,
    };
    use mclone_render::headless::{HEADLESS_FORMAT, create_headless_device};

    let chunk_source = read("../mclone-render/src/chunk.rs");
    assert!(chunk_source.contains("const CHUNK_ATLAS_MAX_MIP_LEVEL: u32 = 4;"));
    let assets = load_textured_mesh_assets()?;
    let (device, queue) = create_headless_device()?;
    let shell_started = Instant::now();
    let draw = TexturedSectionDrawResources::new(
        &device,
        &queue,
        HEADLESS_FORMAT,
        &[],
        assets.atlas.as_upload(),
    )?;
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| anyhow::anyhow!("wait for terrain shell upload: {error:?}"))?;
    let flat_shell_ms = shell_started.elapsed().as_secs_f64() * 1_000.0;

    let mut mip_width = assets.atlas.width.max(1);
    let mut mip_height = assets.atlas.height.max(1);
    let mut mip_bytes = 0usize;
    let mut mip_levels = 0u32;
    for _ in 0..=4 {
        mip_bytes = mip_bytes.saturating_add(mip_width as usize * mip_height as usize * 4);
        mip_levels += 1;
        if mip_width == 1 && mip_height == 1 {
            break;
        }
        mip_width = (mip_width / 2).max(1);
        mip_height = (mip_height / 2).max(1);
    }

    eprintln!(
        "terrain_shell flat_ms={flat_shell_ms:.3} atlas={}x{} base_bytes={} mip_levels={} mip_bytes={} sections={}",
        assets.atlas.width,
        assets.atlas.height,
        assets.atlas.byte_len(),
        mip_levels,
        mip_bytes,
        draw.section_count(),
    );
    assert_eq!(draw.section_count(), 0);

    if !device.features().contains(wgpu::Features::MULTIVIEW) {
        eprintln!("terrain_shell multiview=unavailable");
        return Ok(());
    }

    let width = 16;
    let height = 16;
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_174_empty_shell_multiview_color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HEADLESS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mclone_174_empty_shell_multiview_color_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        base_array_layer: 0,
        array_layer_count: Some(2),
        ..Default::default()
    });
    let depth = ChunkMultiviewDepthTarget::new(&device, width, height);
    let records = draw.prepare_render_records();
    let view = ChunkCamera::overview_for_chunk(0, 0).render_view(width, height);

    let render_once = || -> anyhow::Result<f64> {
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_174_empty_shell_multiview_encoder"),
        });
        let stats = draw.render_prepared_multiview_with_options(
            &records,
            &device,
            &queue,
            &mut encoder,
            ChunkMultiviewRenderTarget::new(
                &color_view,
                &depth.view,
                [width, height],
                wgpu::Color::BLACK,
            ),
            [view, view],
            [TexturedSectionRenderOptions::default(); 2],
        )?;
        let submission = queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map_err(|error| anyhow::anyhow!("wait for empty multiview render: {error:?}"))?;
        assert_eq!(stats[0].drawn_section_count, 0);
        assert_eq!(stats[1].drawn_section_count, 0);
        Ok(started.elapsed().as_secs_f64() * 1_000.0)
    };

    let first_multiview_ms = render_once()?;
    let steady_multiview_ms = render_once()?;
    eprintln!(
        "terrain_shell first_multiview_ms={first_multiview_ms:.3} steady_empty_multiview_ms={steady_multiview_ms:.3}"
    );
    Ok(())
}
