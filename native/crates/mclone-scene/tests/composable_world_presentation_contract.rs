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
    let web_proof = braced_item(&web, "fn render_half_space_terrain_proof(");
    let smoke_export = braced_item(&web, "pub fn render_half_space_terrain_proof(");
    assert!(fixture.contains("pub struct ComplementaryHalfSpaceTerrainFixture"));
    assert!(web_proof.contains("ComplementaryHalfSpaceTerrainFixture::new"));
    assert!(!web_proof.contains("CompositionHalfSpace"));
    assert!(!web_proof.contains("CompositionClip"));
    assert!(smoke_export.contains("host.render_half_space_terrain_proof()"));
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
fn actor_composition_is_opt_in_shared_and_portable() {
    let actor = read("../mclone-render/src/entity.rs");
    let direct = braced_item(&actor, "pub fn render_in_slot(");
    let composed_entry = braced_item(&actor, "pub fn render_composed_in_slot(");
    let composed = braced_item(&actor, "fn render_composed_in_slot_with_preparation(");
    let composed_multiview = braced_item(&actor, "pub fn render_composed_multiview(");
    let direct_uniforms = braced_item(&actor, "fn uniform_bytes(");
    let placed_uniforms = braced_item(&actor, "fn placed_uniform_bytes(");
    let direct_shader = read("../mclone-render/src/shaders/entity_actor.wgsl");
    let direct_multiview_shader = read("../mclone-render/src/shaders/entity_actor_multiview.wgsl");
    let placed_shader = read("../mclone-render/src/shaders/entity_actor_placed.wgsl");
    let placed_multiview_shader =
        read("../mclone-render/src/shaders/entity_actor_placed_multiview.wgsl");

    assert!(!direct.contains("WorldCompositionContext"));
    assert!(!direct.contains("select_composed_actors"));
    assert!(!direct_uniforms.contains("WorldCompositionContext"));
    assert!(!direct_uniforms.contains("placement"));
    assert!(composed_entry.contains("context: WorldCompositionContext"));
    assert!(composed_entry.contains("render_composed_in_slot_with_preparation("));
    assert!(composed_entry.contains("true,"));
    assert!(composed.contains("context: WorldCompositionContext"));
    assert!(composed.contains("select_composed_actors"));
    assert!(composed.contains("renderer.pipeline(context.clip())"));
    assert!(composed_multiview.contains("context: WorldCompositionContext"));
    assert!(composed_multiview.contains("select_composed_actors"));
    assert!(placed_uniforms.contains("context: WorldCompositionContext"));
    for unchanged_shader in [direct_shader, direct_multiview_shader] {
        assert!(!unchanged_shader.contains("source_anchor_scale"));
        assert!(!unchanged_shader.contains("composition_anchor"));
        assert!(!unchanged_shader.contains("clip_plane"));
    }
    for composition_shader in [placed_shader, placed_multiview_shader] {
        assert!(composition_shader.contains("source_anchor_scale"));
        assert!(composition_shader.contains("composition_anchor"));
        assert!(composition_shader.contains("clip_plane"));
        assert!(composition_shader.contains("fs_unbounded"));
        assert!(composition_shader.contains("fs_half_space"));
        assert!(composition_shader.contains("discard;"));
    }

    let fixture = read("../mclone-render/src/actor_composition_fixture.rs");
    let web = read("../../apps/mclone-web-client/src/web_scene_host.rs");
    let web_proof = braced_item(&web, "fn render_actor_composition_proof(");
    let smoke_export = braced_item(&web, "pub fn render_actor_composition_proof(");
    assert!(fixture.contains("pub struct ActorCompositionFixture"));
    assert!(fixture.contains("ActorInstance::cow_model"));
    assert!(fixture.contains("chicken_figure_id()"));
    assert!(fixture.contains("ActorInstance::item_egg"));
    assert!(fixture.contains("ActorInstance::remote_player("));
    assert!(fixture.contains("ActorInstance::local_player("));
    assert!(fixture.contains("with_walk_animation_distance"));
    assert!(fixture.contains("with_chicken_wing_flap_radians"));
    assert!(web_proof.contains("ActorCompositionFixture::new"));
    assert!(!web_proof.contains("ActorInstance"));
    assert!(!web_proof.contains("WorldCompositionContext"));
    assert!(!web_proof.contains("CompositionHalfSpace"));
    assert!(smoke_export.contains("host.render_actor_composition_proof()"));
}

#[test]
fn scene_current_actor_path_is_interpolated_active_only_and_slot_cached() {
    let scene = read("src/lib.rs");
    let host = braced_item(&scene, "pub struct McloneSceneHost {");
    let slot = braced_item(&scene, "struct DrawableWorldSlot {");
    let collect = braced_item(&scene, "fn current_actor_instances(&mut self)");

    assert!(!host.contains("actors: ActorDrawResources"));
    assert!(slot.contains("actors: Option<ActorDrawResources>"));
    assert!(slot.contains("ActorInterpolationState"));
    assert!(slot.contains("last_actor_presentation_update"));
    assert!(collect.contains("self.active_world"));
    assert!(collect.contains("interpolated_actor_presentations"));
    assert!(collect.contains("local_player_actor_instance_for_view("));
    assert!(collect.contains("self.active_world.camera"));
    assert!(collect.contains("self.active_world.player_model"));
    assert!(!collect.contains("standby_world"));
}

#[test]
fn current_local_player_body_is_active_view_policy_and_preview_observers_have_no_body() {
    let inputs = read("../mclone-render-session/src/mesh_inputs.rs");
    let local = braced_item(&inputs, "pub fn local_player_actor_instance_for_view(");
    assert!(local.contains("EngineCameraViewMode::ThirdPersonBack"));
    assert!(
        local.contains("EngineCameraViewMode::FirstPerson if camera.first_person_player_visible()")
    );
    assert!(local.contains("with_first_person_body_only(true)"));

    let scene = read("src/lib.rs");
    let collect = braced_item(&scene, "fn current_actor_instances(&mut self)");
    assert!(collect.contains("self.active_world.camera"));
    assert!(!collect.contains("standby_world"));
    assert!(!collect.contains("source-local"));

    let preview_collect = braced_item(&scene, "fn current_preview_actor_instances(&mut self)");
    assert!(preview_collect.contains(".standby_world"));
    assert!(preview_collect.contains("EmbeddedWorldPreviewPhase::Visible"));
    assert!(preview_collect.contains("actor_instances_from_presentations"));
    assert!(!preview_collect.contains("local_player_actor_instance("));
    assert!(preview_collect.contains("source_local_player_count: 0"));
    assert!(!preview_collect.contains("local_player_actor_instance_for_view"));
}

#[test]
fn standby_preview_starts_as_an_observer_and_activation_exchanges_authority() {
    let session = read("src/session.rs");
    let attach = braced_item(
        &session,
        "fn begin_prepared_warm_world_standby_with_native_world_dir(",
    );
    assert!(attach.contains("LocalIntegratedStartupPump::with_mesh_assets("));
    assert!(attach.contains("with_observer_only(observer_only)"));

    let activate = braced_item(&session, "pub fn request_embedded_world_activation(");
    assert!(activate.contains("promote_observer_to_player()"));
    assert!(activate.contains("demote_player_to_observer()"));

    let server = read("../mclone-server/src/integrated.rs");
    let legacy_constructor = braced_item(
        &server,
        "fn with_scheduler_and_player_chunk_tracking_policy(",
    );
    assert!(
        legacy_constructor.contains(
            "Self::with_scheduler_dimension_definition_and_player_chunk_tracking_policy("
        )
    );
    let constructor = braced_item(
        &server,
        "fn with_scheduler_dimension_definition_and_player_chunk_tracking_policy(",
    );
    assert!(constructor.contains("players: ServerPlayerList::default()"));
    assert!(!constructor.contains(".add_player("));

    let local_session = braced_item(&server, "pub fn from_server_with_capabilities(");
    assert!(local_session.contains("server.add_player_with_capabilities(capabilities)"));
    assert!(local_session.contains("LocalRealmSessionRole::Player(player_id)"));

    let observe = braced_item(&server, "pub fn begin_observing(");
    assert!(observe.contains("self.server.add_observer("));
    assert!(observe.contains("LocalRealmSessionRole::Observer(observer_id)"));

    let promote = braced_item(&server, "fn promote_observer_to_player_with_identity_load(");
    assert!(promote.contains("self.server.remove_observer(observer_id)?"));
    assert!(promote.contains("self.server.add_player_in_dimension(dimension)?"));

    let demote = braced_item(
        &server,
        "fn demote_player_to_observer_with_persistence_flush(",
    );
    assert!(demote.contains("self.server.save_player_record(player_id)?"));
    assert!(demote.contains("self.server.remove_player(player_id)"));
    assert!(demote.contains("self.server.add_observer("));

    let scene = read("src/lib.rs");
    let preview_collect = braced_item(&scene, "fn current_preview_actor_instances(&mut self)");
    assert!(!preview_collect.contains("local_player_actor_instance("));
    assert!(preview_collect.contains("source_local_player_count: 0"));
}

#[test]
fn composition_phase_order_is_all_actors_between_all_opaque_and_translucent() {
    let frame = read("../mclone-app-runtime/src/frame_render.rs");
    let entry = braced_item(&frame, "fn render_full_frame_for_view_inner<BuildGuiDraw>(");
    let backdrop = braced_item(
        &frame,
        "fn render_full_frame_for_view_inner_with_backdrop<BuildGuiDraw>(",
    );
    let render = braced_item(
        &frame,
        "fn render_full_frame_for_view_inner_with_actor_preparation<BuildGuiDraw>(",
    );
    assert!(entry.contains("render_full_frame_for_view_inner_with_backdrop("));
    assert!(backdrop.contains("render_full_frame_for_view_inner_with_actor_preparation("));
    assert!(backdrop.contains("FrameActorPreparation::Refresh"));
    assert_in_order(
        render,
        &[
            "TexturedSectionRenderPhase::Opaque",
            "Some(OpaqueWorldInsertion::Composition(composition))",
            ".render_placed_prepared_with_options",
            "if !actor_instances.is_empty()",
            ".render_in_slot(",
            "composition.actors.as_mut()",
            "render_composed_in_slot(",
            "if split_translucent_terrain",
            "render_composed_translucent_terrain(",
        ],
    );
}

#[test]
fn retained_destination_presentations_exist_below_scene_omission() {
    let client = read("../mclone-client/src/lib.rs");
    let presentations = braced_item(&client, "pub fn actor_presentations(&self)");
    assert!(presentations.contains("actor_presentations_at"));
    let timed_presentations = braced_item(&client, "pub fn actor_presentations_at(");
    assert!(timed_presentations.contains("self.remote_players"));
    assert!(timed_presentations.contains("self.entities"));

    let scene = read("src/lib.rs");
    let collect = braced_item(&scene, "fn current_actor_instances(&mut self)");
    assert!(!collect.contains("standby_world"));
    assert!(!collect.contains("embedded_world_preview"));

    let preview_collect = braced_item(&scene, "fn current_preview_actor_instances(&mut self)");
    assert!(preview_collect.contains("standby_world"));
    assert!(preview_collect.contains("runtime.client()"));
    assert!(preview_collect.contains("interpolated_actor_presentations"));
    assert!(preview_collect.contains("client.entity_count()"));
    assert!(preview_collect.contains("client.remote_player_count()"));
}

#[test]
fn preview_actor_submission_is_shared_scene_policy_with_portable_diagnostics() {
    let frame = read("../mclone-app-runtime/src/frame_render.rs");
    let placed = braced_item(&frame, "pub struct PlacedActorFrame<'a> {");
    assert!(placed.contains("draw: &'a mut ActorDrawResources"));
    assert!(placed.contains("instances: &'a [ActorInstance]"));
    assert!(placed.contains("context: WorldCompositionContext"));

    let scene = read("src/lib.rs");
    let mono = read("src/mono.rs");
    let multiview = braced_item(
        &scene,
        "fn render_prepared_terrain_multiview_frame_with_upload_inner(",
    );
    assert!(mono.contains("PlacedActorFrame"));
    assert!(mono.contains("preview_actor_instances"));
    assert!(multiview.contains("render_composed_multiview("));
    assert!(multiview.contains("record_actor_render("));

    let warm = read("src/warm_world.rs");
    let diagnostics = braced_item(&warm, "pub struct EmbeddedWorldPreviewRenderSnapshot {");
    for fact in [
        "last_actor_entity_count",
        "last_actor_remote_player_count",
        "last_actor_source_local_player_count",
        "last_source_rejected_actor_count",
        "actor_mesh_rebuild_count",
        "actor_mesh_upload_count",
        "actor_gpu_capacity_bytes",
        "placed_actor_pipeline_count",
        "remote_player_observation_count",
        "remote_player_motion_sequence",
        "last_remote_player_motion_from",
        "last_remote_player_motion_to",
        "last_remote_player_update_to_visible_frame_count",
    ] {
        assert!(
            diagnostics.contains(fact),
            "missing preview actor fact {fact}"
        );
    }

    let web = read("../../apps/mclone-web-client/src/web_scene_host.rs");
    assert!(web.contains("embeddedPreviewActorSourceLocalPlayerCount"));
    assert!(web.contains("embeddedPreviewActorMeshRebuildCount"));
    assert!(web.contains("embeddedPreviewRemotePlayerMotionSequence"));
    assert!(web.contains("(\"SourceX\", observation.source_feet_position.x)"));
    assert!(web.contains("embeddedPreviewFirstRemotePlayer{suffix}"));
    assert!(!web.contains("actor_instances_from_presentations"));
    assert!(!web.contains("local_player_actor_instance("));
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
    assert!(
        gpu_advance
            .matches("ensure_composed_topology(device)")
            .count()
            >= 2
    );
    assert_in_order(
        gpu_advance,
        &[
            "state.readiness.switchable",
            "if slot.actors.is_none()",
            "ensure_composed_topology(device)",
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
fn integrated_remote_player_tracking_pairs_local_and_dedicated_symmetrically() {
    let server = read("../mclone-server/src/integrated.rs");
    let observer = braced_item(&server, "fn reconcile_remote_players_for_target_observer(");
    let subject = braced_item(&server, "fn reconcile_remote_player_subject(");
    let states = braced_item(&server, "fn remote_player_states(&self)");
    let state = braced_item(&server, "fn remote_player_state(&self");
    assert!(observer.contains("let player_id = target.player_id()"));
    assert!(observer.contains("self.players.contains(player_id)"));
    assert!(observer.contains("DimensionInterestSource::Player(player_id)"));
    assert!(!observer.contains("local_player"));
    let non_player_observer = braced_item(&server, "fn reconcile_remote_players_for_observer(");
    assert!(non_player_observer.contains("DimensionInterestSource::Observer(observer_id)"));
    assert!(subject.contains("self.players.contains(subject)"));
    assert!(!subject.contains("ServerPlayerId::LOCAL"));
    assert!(states.contains("self.player_observers()"));
    assert!(state.contains("self.players.get(player_id)?"));
    assert!(!state.contains("local_player"));
}
