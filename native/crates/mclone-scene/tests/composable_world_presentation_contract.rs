//! Tactical 179 locks the evolving terrain/actor composition seams.
//!
//! Several assertions are deliberately transitional. Later tactical slices
//! update them only when shared ownership replaces the characterized shape.

use std::path::{Path, PathBuf};

fn scene_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(scene_root().join(relative)).expect("source file readable")
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

#[test]
fn existing_terrain_placement_is_opt_in_and_bounded_separately() {
    let placement = read("../mclone-render/src/placement.rs");
    let terrain = read("../mclone-render/src/chunk.rs");
    let direct_shader = read("../mclone-render/src/shaders/chunk_textured.wgsl");
    let placed_shader = read("../mclone-render/src/shaders/chunk_textured_placed.wgsl");

    assert!(placement.contains("pub struct WorldPlacement"));
    assert!(placement.contains("pub struct EmbeddedChunkRegion"));
    assert!(placement.contains("pub struct WorldCompositionContext"));
    assert!(placement.contains("pub struct WorldSourceBounds"));
    assert!(placement.contains("pub struct CompositionHalfSpace"));
    assert!(placement.contains("pub enum CompositionClip"));
    assert!(terrain.contains("prepare_render_records_for_context"));
    assert!(terrain.contains("pub struct PlacedTexturedSectionRenderer"));
    assert!(!direct_shader.contains("source_anchor_scale"));
    assert!(!direct_shader.contains("composition_anchor"));
    assert!(placed_shader.contains("source_anchor_scale"));
    assert!(placed_shader.contains("composition_anchor"));
}

#[test]
fn composition_context_stays_out_of_the_direct_single_world_path() {
    let terrain = read("../mclone-render/src/chunk.rs");
    let direct = braced_item(&terrain, "fn render_with_options_inner(");
    let direct_uniforms = braced_item(&terrain, "fn uniform_bytes(");
    let placed_uniforms = braced_item(&terrain, "fn placed_uniform_bytes(");
    assert!(!direct.contains("WorldCompositionContext"));
    assert!(!direct.contains("context"));
    assert!(!direct_uniforms.contains("WorldCompositionContext"));
    assert!(!direct_uniforms.contains("placement"));
    assert!(!placed_uniforms.contains("WorldCompositionContext"));
    assert!(placed_uniforms.contains("placement: WorldPlacement"));

    let session = read("src/session.rs");
    let install = braced_item(&session, "fn install_prepared_warm_world_slot(");
    let preview_branch = install
        .split("self.embedded_world_preview = match")
        .nth(1)
        .expect("preview/no-preview construction branch remains explicit");
    assert!(preview_branch.contains("WarmWorldPresentationRequest::Diorama"));
    assert!(preview_branch.contains("WorldCompositionContext::unbounded"));
    assert!(preview_branch.contains("WarmWorldPresentationRequest::OpaqueGate"));
    let opaque_branch = preview_branch
        .split("WarmWorldPresentationRequest::OpaqueGate")
        .nth(1)
        .expect("opaque no-preview branch remains explicit");
    assert!(!opaque_branch.contains("WorldCompositionContext::unbounded"));
}

#[test]
fn half_space_clipping_is_an_opt_in_shared_renderer_topology() {
    let terrain = read("../mclone-render/src/chunk.rs");
    let direct_shader = read("../mclone-render/src/shaders/chunk_textured.wgsl");
    let placed_shader = read("../mclone-render/src/shaders/chunk_textured_placed.wgsl");
    let placed_multiview_shader =
        read("../mclone-render/src/shaders/chunk_textured_placed_multiview.wgsl");
    let selection = braced_item(&terrain, "fn selected_mono_renderer(");
    let clipped_shader = terrain
        .split("fn clipped_placed_shader_source(")
        .nth(1)
        .expect("clipped placed shader helper present")
        .split("fn clipped_placed_multiview_shader_source(")
        .next()
        .expect("mono helper ends before multiview helper");

    for unchanged_shader in [direct_shader, placed_shader, placed_multiview_shader] {
        assert!(!unchanged_shader.contains("clip_plane"));
    }
    assert!(selection.contains("CompositionClip::Unbounded"));
    assert!(selection.contains("SelectedPlacedMonoRenderer::Unbounded(self)"));
    assert!(selection.contains("CompositionClip::HalfSpace"));
    assert!(selection.contains("self.clipped_renderer"));
    assert!(clipped_shader.contains("clip_plane: vec4<f32>"));
    assert!(clipped_shader.contains("discard;"));

    let fixture = read("../mclone-render/src/composition_fixture.rs");
    let web = read("../../apps/mclone-web-client/src/web_scene_host.rs");
    let web_proof = braced_item(&web, "pub fn render_half_space_terrain_proof(");
    assert!(fixture.contains("pub struct ComplementaryHalfSpaceTerrainFixture"));
    assert!(web_proof.contains("ComplementaryHalfSpaceTerrainFixture::new"));
    assert!(!web_proof.contains("CompositionHalfSpace"));
    assert!(!web_proof.contains("CompositionClip"));
}

#[test]
fn actor_renderer_shared_topology_and_per_world_state_are_exact() {
    let actor = read("../mclone-render/src/entity.rs");
    let shared = braced_item(&actor, "pub struct ActorSharedResources {");
    for field in [
        "renderer: ActorRenderer",
        "atlas: GpuActorTextureAtlas",
        "texture_layout: ActorTextureLayout",
        "atlas_size: [u32; 2]",
        "actor_figures: ActorFigureSet",
    ] {
        assert!(
            shared.contains(field),
            "missing shared actor resource `{field}`"
        );
    }
    let state = braced_item(&actor, "pub struct ActorDrawResources {");
    for field in [
        "shared: Arc<ActorSharedResources>",
        "uniforms: PerViewUniformBuffer",
        "bind_group: wgpu::BindGroup",
        "multiview: RefCell<Option<ActorMultiviewDrawState>>",
        "mesh_cache: ActorMeshCache",
    ] {
        assert!(
            state.contains(field),
            "missing per-world actor state `{field}`"
        );
    }
    let cache = braced_item(&actor, "struct ActorMeshCache {");
    for field in [
        "actors: Vec<ActorInstance>",
        "mesh: ActorMesh",
        "scratch: ActorMeshBuildScratch",
        "vertex_bytes: Vec<u8>",
        "index_bytes: Vec<u8>",
        "vertex_buffer: Option<wgpu::Buffer>",
        "index_buffer: Option<wgpu::Buffer>",
    ] {
        assert!(cache.contains(field), "missing actor cache field `{field}`");
    }
    assert!(actor.contains("const ACTOR_MESH_MIN_VERTEX_CAPACITY: usize = 8192;"));
    assert!(actor.contains("const ACTOR_MESH_MIN_INDEX_CAPACITY: usize = 12_288;"));
    let prepare = braced_item(&actor, "fn prepare(");
    assert_in_order(
        prepare,
        &[
            "if self.actors.as_slice() != actors",
            "rebuild_actor_mesh(",
            "self.uploaded = false;",
            "if !self.uploaded",
            "queue.write_buffer(buffer, 0, &self.vertex_bytes);",
            "queue.write_buffer(buffer, 0, &self.index_bytes);",
            "self.uploaded = true;",
        ],
    );
}

#[test]
fn scene_current_actor_path_is_raw_active_only_and_slot_cached() {
    let scene = read("src/lib.rs");
    let host = braced_item(&scene, "pub struct McloneSceneHost {");
    let slot = braced_item(&scene, "struct DrawableWorldSlot {");
    let collect = braced_item(&scene, "fn current_actor_instances(&self)");

    assert!(!host.contains("actors: ActorDrawResources"));
    assert!(slot.contains("actors: Option<ActorDrawResources>"));
    assert!(!slot.contains("ActorInterpolationState"));
    assert!(collect.contains("self.active_world"));
    assert!(collect.contains("runtime.client().actor_presentations()"));
    assert!(collect.contains("local_player_actor_instance_for_view("));
    assert!(collect.contains("self.active_world.camera"));
    assert!(collect.contains("self.active_world.player_model"));
    assert!(!collect.contains("standby_world"));
    assert!(!collect.contains("ActorInterpolationState"));
}

#[test]
fn current_local_player_body_is_active_view_policy_not_preview_policy() {
    let inputs = read("../mclone-render-session/src/mesh_inputs.rs");
    let local = braced_item(&inputs, "pub fn local_player_actor_instance_for_view(");
    assert!(local.contains("EngineCameraViewMode::ThirdPersonBack"));
    assert!(
        local.contains("EngineCameraViewMode::FirstPerson if camera.first_person_player_visible()")
    );
    assert!(local.contains("with_first_person_body_only(true)"));

    let scene = read("src/lib.rs");
    let collect = braced_item(&scene, "fn current_actor_instances(&self)");
    assert!(collect.contains("self.active_world.camera"));
    assert!(!collect.contains("standby_world"));
    assert!(!collect.contains("source-local"));
}

#[test]
fn composition_phase_order_is_active_actors_between_all_opaque_and_translucent() {
    let frame = read("../mclone-app-runtime/src/frame_render.rs");
    let render = braced_item(&frame, "fn render_full_frame_for_view_inner<BuildGuiDraw>(");
    assert_in_order(
        render,
        &[
            "TexturedSectionRenderPhase::Opaque",
            "Some(OpaqueWorldInsertion::Composition(composition))",
            ".render_placed_prepared_with_options",
            "if !actor_instances.is_empty()",
            ".render_in_slot(",
            "if split_translucent_terrain",
            "render_composed_translucent_terrain(",
        ],
    );
}

#[test]
fn retained_destination_presentations_exist_below_scene_omission() {
    let client = read("../mclone-client/src/lib.rs");
    let presentations = braced_item(&client, "pub fn actor_presentations(&self)");
    assert!(presentations.contains("self.remote_players"));
    assert!(presentations.contains("self.entities"));

    let scene = read("src/lib.rs");
    let collect = braced_item(&scene, "fn current_actor_instances(&self)");
    assert!(!collect.contains("standby_world"));
    assert!(!collect.contains("embedded_world_preview"));
}

#[test]
fn complete_slot_exchange_carries_actor_state_and_replacement_drops_stale_topology() {
    let session = read("src/session.rs");
    let swap = braced_item(&session, "fn swap_with_switchable_warm_world_using_poses(");
    assert!(swap.contains("std::mem::swap("));
    assert!(!swap.contains("ActorDrawResources::new"));

    let gpu_advance = braced_item(&session, "pub(crate) fn advance_warm_world_gpu(");
    assert!(gpu_advance.contains("if slot.actors.is_none()"));
    assert!(gpu_advance.contains("ActorDrawResources::new_with_shared_resources"));
    assert_in_order(
        gpu_advance,
        &[
            "state.readiness.switchable",
            "if slot.actors.is_none()",
            "slot.lifecycle = WorldSlotLifecycle::StandbySwitchable",
        ],
    );

    let asset_replacement = read("src/asset_replacement.rs");
    let replacement = braced_item(&asset_replacement, "fn commit_asset_replacement(");
    assert_in_order(
        replacement,
        &[
            "self.cancel_warm_world_standby(\"asset replacement\");",
            "let actors = ActorDrawResources::new(",
            "self.active_world.actors = Some(actors);",
        ],
    );
}

#[test]
fn managed_scenario_payload_currently_authors_no_entity_records() {
    let scenario = read("../mclone-app-runtime/src/scenario_content.rs");
    let payload = braced_item(&scenario, "pub struct ManagedScenarioWorldPayload {");
    assert!(payload.contains("chunk_records: Vec<ManagedScenarioChunkRecord>"));
    assert!(!payload.contains("entity_chunk_records"));

    let web = read("../../apps/mclone-web-client/src/web_canvas.rs");
    let encode = braced_item(&web, "fn encode_web_managed_scenario_payload(");
    assert!(encode.contains("&JsValue::from_str(\"entityChunks\")"));
    assert!(encode.contains("&js_sys::Array::new()"));
}

#[test]
fn integrated_remote_player_tracking_currently_excludes_local_pairs() {
    let server = read("../mclone-server/src/integrated.rs");
    let observer = braced_item(&server, "fn reconcile_remote_players_for_target_observer(");
    let subject = braced_item(&server, "fn reconcile_remote_player_subject(");
    let states = braced_item(&server, "fn remote_player_states(&self)");
    assert!(observer.contains("target.dedicated_player_id()"));
    assert!(subject.contains("self.dedicated_players.contains(subject)"));
    assert!(states.contains("self.dedicated_players"));
    assert!(!states.contains("ServerPlayerId::LOCAL"));
}
