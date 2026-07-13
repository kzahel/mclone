//! Tactical 174 characterization locks for the concrete drawable-world slot,
//! its one-active/optional-standby ownership, and installation/reset seams.

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

fn field_names(item: &str) -> Vec<&str> {
    item.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (name, _) = line.split_once(':')?;
            (!name.contains(' ') && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()))
                .then_some(name)
        })
        .collect()
}

const DRAWABLE_WORLD_SLOT_FIELDS: &[&str] = &[
    "id",
    "descriptor",
    "storage",
    "lifecycle",
    "asset_epoch",
    "scene",
    "runtime",
    "local_startup",
    "external_runtime_startup_pending",
    "camera",
    "interaction",
    "player_model",
    "draw",
    "traversal_ready_sections",
    "section_uploads",
    "far_lod",
    "render_stats",
    "render_admission_policy",
    "accepted_entry_pose",
    "pending_startup_sections",
];

const SCENE_HOST_FIELDS: &[&str] = &[
    "active_world",
    "standby_world",
    "warm_world_standby",
    "world_gate",
    "opaque_world_gate_renderer",
    "warm_world_switch_sequence",
    "last_warm_world_switch",
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
    "session",
    "session_runtime_factory",
    "client_experience",
    "initial_alignment_mode",
    "render_options",
    "player_collision_box_visible",
    "crosshair_visible",
    "travel_assist_mode",
    "actors",
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
    "tracking_origin",
    "locomotion_mode",
    "turn_policy",
    "snap_turn_state",
    "blink_teleport",
    "mono_blink_debug",
    "display_refresh_hz",
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
fn host_has_one_active_and_one_optional_concrete_drawable_slot() {
    let source = read("src/lib.rs");
    let slot = braced_item(&source, "struct DrawableWorldSlot {");
    let host = braced_item(&source, "pub struct McloneSceneHost {");
    let slot_fields = field_names(slot);
    let host_fields = field_names(host);
    assert_eq!(slot_fields, DRAWABLE_WORLD_SLOT_FIELDS);
    assert_eq!(slot_fields.len(), 20);
    assert_eq!(host_fields, SCENE_HOST_FIELDS);
    assert_eq!(host_fields.len(), 75);
    assert_eq!(host.matches("active_world: DrawableWorldSlot").count(), 1);
    assert_eq!(
        host.matches("standby_world: Option<DrawableWorldSlot>")
            .count(),
        1
    );
    assert_eq!(
        host.matches("warm_world_standby: Option<WarmWorldStandbyState>")
            .count(),
        1
    );
    for collection in [
        "Vec<DrawableWorldSlot>",
        "HashMap<WorldInstanceId",
        "BTreeMap<WorldInstanceId",
    ] {
        assert!(
            !host.contains(collection),
            "Slice 3 must retain exactly one optional standby, not `{collection}`"
        );
    }
}

#[test]
fn ordinary_frame_paths_submit_only_the_active_world_draw_store() {
    let mono_source = read("src/mono.rs");
    let mono = braced_item(&mono_source, "fn render_mono_frame_inner(");
    assert!(mono.contains("&mut self.active_world.draw,"));

    let xr_source = read("src/lib.rs");
    let stereo = braced_item(&xr_source, "fn render_prepared_frame(");
    assert!(stereo.contains("self.active_world.draw.prepare_render_records_with_stats()"));
    assert!(stereo.contains("self.active_world.draw.prepare_stereo_draw"));

    let multiview = braced_item(
        &xr_source,
        "fn render_prepared_terrain_multiview_frame_with_upload_inner(",
    );
    assert!(multiview.contains("self.active_world.draw.prepare_render_records_with_stats()"));
    assert!(multiview.contains(".render_prepared_multiview_stereo_draw_phase_with_options("));

    for path in [mono, stereo, multiview] {
        for absent in [
            "standby_world.draw",
            "EmbeddedWorldPreview",
            "WorldPlacement",
            "composition_anchor",
        ] {
            assert!(
                !path.contains(absent),
                "active-only frame path unexpectedly contains `{absent}`"
            );
        }
    }
}

#[test]
fn every_initial_host_path_constructs_the_same_drawable_slot() {
    let source = read("src/session.rs");
    for marker in [
        "pub fn start_local_async(",
        "pub fn with_runtime<S>(",
        "pub fn with_scene_runtime(",
    ] {
        let constructor = braced_item(&source, marker);
        assert!(constructor.contains("let active_world = DrawableWorldSlot::new("));
        assert!(constructor.contains("DrawableWorldSlotInstall {"));
        assert!(constructor.contains("active_world,"));
        assert!(constructor.contains("standby_world: None,"));
        assert!(constructor.contains("warm_world_standby: None,"));
        assert!(constructor.contains("world_gate: None,"));
        assert!(constructor.contains("opaque_world_gate_renderer: None,"));
        assert!(constructor.contains("warm_world_switch_sequence: 0,"));
        assert!(constructor.contains("last_warm_world_switch: None,"));
    }
}

#[test]
fn warm_world_selection_exchanges_complete_slots_without_reconstruction() {
    let source = read("src/session.rs");
    let select = braced_item(&source, "fn swap_with_switchable_warm_world_using_poses(");
    assert_in_order(
        select,
        &[
            "mapped warm-world return destination is not GPU/traversal ready",
            "Self::commit_world_slot_camera(&mut self.active_world",
            "Self::commit_world_slot_camera(standby",
            "self.active_world.accepted_entry_pose = Some(source_entry_pose);",
            "std::mem::swap(",
            "self.active_world.lifecycle = WorldSlotLifecycle::ActiveReady;",
            ".lifecycle = WorldSlotLifecycle::StandbySwitchable;",
            "self.session.complete_start(destination_descriptor);",
            "self.clear_world_selection_presentation_state();",
            "self.retarget_warm_world_state_after_switch(",
        ],
    );
    for forbidden in [
        "TexturedSectionDrawResources::new(",
        "DrawableWorldSlot::new(",
        "prepare_world_slot(",
        "advance_warm_world_gpu(",
        "clear_stream_state(",
    ] {
        assert!(
            !select.contains(forbidden),
            "selection must not perform `{forbidden}`"
        );
    }
    for conservation in [
        "switch_uploaded_section_count: 0,",
        "switch_submitted_compile_section_count: 0,",
        "switch_accepted_compile_result_count: 0,",
        "switch_materialized_renderer: false,",
    ] {
        assert!(select.contains(conservation));
    }

    let prepare_source = read("src/lib.rs");
    let prepare = braced_item(&prepare_source, "fn poll_runtime_and_upload(");
    assert!(prepare.contains("first_frame_after_warm_world_selection"));
    assert!(prepare.contains("max_compile_requests: first_frame_after_warm_world_selection"));
    assert!(prepare.contains("then_some(Duration::from_micros(750))"));

    let reset = braced_item(&source, "fn clear_world_selection_presentation_state(");
    for presentation_state in [
        "underwater_effects",
        "last_underwater_update",
        "tracking_origin",
        "prefetched_live_upload",
        "last_locomotion_update",
        "first_eye_summary",
        "last_ui_panel_stats",
        "last_ui_draw_cache_stats",
        "rendered_frames",
    ] {
        assert!(
            reset.contains(presentation_state),
            "missing selection reset `{presentation_state}`"
        );
    }
    for slot_state in [
        "clear_stream_state",
        "traversal_ready_sections",
        "section_uploads",
        "draw =",
        "runtime =",
        "camera =",
    ] {
        assert!(
            !reset.contains(slot_state),
            "selection reset must preserve slot-owned `{slot_state}`"
        );
    }
}

#[test]
fn detached_standby_is_opt_in_and_gpu_admission_is_bounded() {
    let source = read("src/session.rs");
    let begin = braced_item(&source, "pub fn begin_warm_world_standby(");
    assert_in_order(
        begin,
        &[
            "scene.world_root = None;",
            "scene.world_dir = None;",
            "scene.use_initial_spawn_center = false;",
            "TexturedSectionDrawResources::new(",
            "LocalIntegratedStartupPump::with_mesh_assets(",
            "id: instance_id,",
            "lifecycle: WorldSlotLifecycle::Starting,",
            "runtime: None,",
            "local_startup: Some(SceneLocalStartup {",
            "self.warm_world_standby = Some(WarmWorldStandbyState {",
        ],
    );
    assert!(begin.contains("&[],"));
    assert!(!begin.contains("self.active_world.install("));
    assert!(begin.contains("OpaqueWorldGateRenderer::new("));
    assert!(begin.contains("self.opaque_world_gate_renderer = Some(gate_renderer);"));

    let advance = braced_item(&source, "fn advance_warm_world_standby(");
    assert!(advance.contains("startup.pump.step(camera_position)"));
    assert!(advance.contains("reconcile_xr_startup_pose("));
    assert!(advance.contains("resolve_world_gate_endpoint("));
    assert!(advance.contains("into_runtime_with_startup_sections"));
    assert!(advance.contains("complete_detached_local_startup("));
    assert!(!advance.contains("TexturedSectionDrawResources::new("));
    assert!(!advance.contains("drain_budgeted("));
    assert!(!advance.contains("render_full_frame"));

    let gpu = braced_item(&source, "pub(crate) fn advance_warm_world_gpu(");
    assert_in_order(
        gpu,
        &[
            "std::mem::take(&mut slot.pending_startup_sections)",
            "RenderSectionCacheUpdate::from_startup_seed(startup_sections)",
            "slot.section_uploads.enqueue_cache_update(update)",
            "poll_budget: RuntimeUpdatePumpBudget::MaxElapsed(Duration::from_micros(500))",
            "upload_budget: Some(1)",
            "accept_budget: Some(1)",
            "completed_result_accept_budget: Some(1)",
            "max_compile_requests: Some(1)",
            "Self::prepare_world_slot(",
        ],
    );
    assert!(gpu.contains("active_frame_deadline"));
    assert!(gpu.contains("entry_section_gpu_resident"));
    assert!(gpu.contains("entry_section_traversal_ready"));
    assert!(!gpu.contains("render_full_frame"));

    let host = read("src/lib.rs");
    let active = braced_item(&host, "fn poll_runtime_and_upload(");
    assert!(active.contains("poll_budget: RuntimeUpdatePumpBudget::unlimited()"));
    assert!(active.contains("upload_budget: selection_budget(self.render_section_upload_budget)"));
    assert!(active.contains("first_frame_after_warm_world_selection"));
    assert!(active.contains("Self::prepare_world_slot("));
    assert_in_order(
        active,
        &[
            "Self::prepare_world_slot(",
            "self.advance_warm_world_gpu(device, standby_deadline)?;",
        ],
    );
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
            "let mesh_assets = runtime.mesh_assets().clone();",
            "self.active_world.install(DrawableWorldSlotInstall {",
            "scene,",
            "runtime: Some(runtime),",
            "local_startup: None,",
            "external_runtime_startup_pending: false,",
            "camera,",
            "draw,",
            "render_stats: RenderStreamStats {",
            "section_count,",
            "index_count,",
            "face_count,",
            "self.mesh_assets = mesh_assets;",
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
            "let draw = TexturedSectionDrawResources::new(",
            "self.active_world.install(DrawableWorldSlotInstall {",
            "scene: pending.scene,",
            "runtime: Some(runtime),",
            "local_startup: None,",
            "external_runtime_startup_pending: true,",
            "camera,",
            "draw,",
            "render_stats: RenderStreamStats::default(),",
            "self.clear_transient_world_state();",
            "self.session.complete_start(active.clone());",
        ],
    );

    let replacement = braced_item(&source, "pub(crate) fn start_replacement_session(");
    assert_in_order(
        replacement,
        &[
            "let started = match factory.start(",
            "let mesh_assets = started.runtime.mesh_assets().clone();",
            "self.active_world.install(DrawableWorldSlotInstall {",
            "scene,",
            "runtime: Some(started.runtime),",
            "local_startup: None,",
            "external_runtime_startup_pending: false,",
            "camera: started.camera,",
            "draw: started.draw,",
            "render_stats: started.render_stats,",
            "self.mesh_assets = mesh_assets;",
            "self.clear_transient_world_state();",
        ],
    );

    let slot_source = read("src/lib.rs");
    let target_neutral = braced_item(&slot_source, "struct DrawableWorldSlotInstall {");
    for field in [
        "scene",
        "runtime",
        "local_startup",
        "external_runtime_startup_pending",
        "camera",
        "draw",
        "render_stats",
    ] {
        assert!(target_neutral.contains(&format!("{field}:")));
    }

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
fn world_and_physical_resets_are_explicit_and_keep_one_world_call_order() {
    let source = read("src/session.rs");
    let slot_source = read("src/lib.rs");
    let stream = braced_item(&slot_source, "fn clear_stream_state(");
    let active = braced_item(&source, "fn clear_active_world_transient_state(");
    let physical = braced_item(&source, "fn clear_physical_presentation_state(");
    let reset = braced_item(&source, "pub(crate) fn clear_transient_world_state(");

    for stream_state in [
        "traversal_ready_sections",
        "section_uploads",
        "render_admission_policy",
    ] {
        assert!(
            stream.contains(stream_state),
            "missing slot stream reset `{stream_state}`"
        );
    }
    for active_state in [
        "underwater_effects",
        "last_underwater_update",
        "prefetched_live_upload",
        "active_world.clear_stream_state()",
    ] {
        assert!(
            active.contains(active_state),
            "missing active-world reset `{active_state}`"
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
            physical.contains(physical_state),
            "missing physical reset `{physical_state}`"
        );
    }
    assert_in_order(
        reset,
        &[
            "self.clear_physical_presentation_state();",
            "self.clear_active_world_transient_state();",
        ],
    );
}

#[test]
fn asset_replacement_cancels_mid_warm_standby_before_preserving_active_identity() {
    let source = read("src/asset_replacement.rs");
    let commit = braced_item(&source, "fn commit_asset_replacement(");

    for snapshot in [
        "let session_before = self.session.state().clone();",
        "let camera_before = self.active_world.camera.snapshot();",
        "let command_count_before = runtime.core().command_count();",
        "let update_count_before = runtime.core().update_count();",
    ] {
        assert!(commit.contains(snapshot));
    }
    assert_in_order(
        commit,
        &[
            "self.cancel_warm_world_standby(\"asset replacement\");",
            "runtime.replace_asset_epoch(",
            "self.mesh_assets = assets.mesh.clone();",
            "self.active_world.draw = draw;",
            "self.active_assets = assets;",
            "self.active_world.traversal_ready_sections.clear();",
            "self.active_world.section_uploads.clear();",
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

    let multiview_started = Instant::now();
    assert!(draw.materialize_multiview_renderer(&device)?);
    device.poll(wgpu::PollType::Wait).map_err(|error| {
        anyhow::anyhow!("wait for terrain multiview materialization: {error:?}")
    })?;
    let multiview_materialize_ms = multiview_started.elapsed().as_secs_f64() * 1_000.0;
    assert!(!draw.materialize_multiview_renderer(&device)?);
    assert!(draw.multiview_renderer_materialized());

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
        "terrain_shell multiview_materialize_ms={multiview_materialize_ms:.3} first_warm_multiview_ms={first_multiview_ms:.3} steady_empty_multiview_ms={steady_multiview_ms:.3}"
    );
    Ok(())
}
