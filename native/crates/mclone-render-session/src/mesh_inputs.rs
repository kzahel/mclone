use super::*;

pub fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs_with_biome_zoom_seed(&chunks, client.biome_zoom_seed());
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

pub fn build_render_sections_from_snapshots<S: std::borrow::Borrow<ChunkSnapshot>>(
    snapshots: &[S],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport> {
    build_render_sections_from_snapshots_with_biome_zoom_seed(
        snapshots,
        catalog,
        target_sections,
        None,
    )
}

pub fn build_render_sections_from_snapshots_with_biome_zoom_seed<
    S: std::borrow::Borrow<ChunkSnapshot>,
>(
    snapshots: &[S],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
    biome_zoom_seed: Option<i64>,
) -> Result<TexturedRenderSectionBuildReport> {
    // Generic over `Borrow<ChunkSnapshot>` so a caller holding owned snapshots
    // (`&[ChunkSnapshot]`, desktop + the web full-view helpers) and one holding borrowed
    // snapshots (`&[&ChunkSnapshot]`, the web worker's resident mirror, 067 Stage 4) both
    // build sections without forcing the latter to deep-clone its resident map every compile.
    let chunks = snapshots
        .iter()
        .map(|snapshot| snapshot_mesh_block_state_ids(snapshot.borrow()))
        .collect::<Result<Vec<_>>>()?;
    let inputs = textured_mesh_inputs_with_biome_zoom_seed(&chunks, biome_zoom_seed);
    build_textured_render_sections_for_section_set_with_stats(&inputs, catalog, target_sections)
        .context("failed to build queued textured render sections")
}

pub fn mesh_chunks_from_client(client: &ClientRuntime) -> Result<Vec<MeshChunkBlocks>> {
    client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()
}

pub fn textured_mesh_inputs(chunks: &[MeshChunkBlocks]) -> Vec<TexturedChunkMeshInput<'_>> {
    textured_mesh_inputs_with_biome_zoom_seed(chunks, None)
}

pub fn textured_mesh_inputs_with_biome_zoom_seed(
    chunks: &[MeshChunkBlocks],
    biome_zoom_seed: Option<i64>,
) -> Vec<TexturedChunkMeshInput<'_>> {
    chunks
        .iter()
        .map(|chunk| {
            let input = TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
            .with_biomes(&chunk.biomes)
            .with_light_sections(&chunk.light_sections);
            if let Some(seed) = biome_zoom_seed {
                input.with_biome_zoom_seed(seed)
            } else {
                input
            }
        })
        .collect()
}

pub fn actor_instances_from_presentations(
    presentations: &[ActorPresentation],
    client: &ClientRuntime,
) -> Vec<ActorInstance> {
    presentations
        .iter()
        .map(|actor| {
            let packed_light =
                client.packed_light_at_world_or_fullbright(actor_light_probe_block_pos(actor));
            match actor.kind {
                ActorPresentationKind::RemotePlayer => actor
                    .appearance
                    .figure
                    .map_or_else(
                        || {
                            ActorInstance::remote_player(
                                glam_vec3_from_vec3d(actor.feet_position),
                                actor.y_rot_degrees,
                            )
                            .with_walk_animation_distance(actor.walk_animation_distance)
                        },
                        |figure| {
                            ActorInstance::remote_player_with_figure(
                                glam_vec3_from_vec3d(actor.feet_position),
                                actor.y_rot_degrees,
                                figure,
                            )
                            .with_walk_animation_distance(actor.walk_animation_distance)
                        },
                    )
                    .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Cow) => ActorInstance::cow_model(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Chicken) => {
                    ActorInstance::remote_player_with_figure(
                        glam_vec3_from_vec3d(actor.feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::chicken_figure_id(),
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_walk_animation_distance(actor.walk_animation_distance)
                    .with_chicken_wing_flap_radians(actor.chicken_wing_flap_radians)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Mannequin) => {
                    ActorInstance::remote_player(
                        glam_vec3_from_vec3d(actor.feet_position),
                        actor.y_rot_degrees,
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_walk_animation_distance(actor.walk_animation_distance)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::DebugCube) => ActorInstance::debug_cube(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                    actor.x_rot_degrees,
                    actor.rotation.map(glam_quat_from_entity_rotation),
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Item) => {
                    match actor.item_stack.map(|stack| stack.kind) {
                        Some(ItemKind::Egg) | None => ActorInstance::item_egg(
                            glam_vec3_from_vec3d(actor.feet_position),
                            actor.y_rot_degrees,
                            actor.width,
                            actor.height,
                        )
                        .with_packed_light(packed_light),
                    }
                }
            }
        })
        .collect()
}

pub fn local_player_actor_instance(
    camera: &EngineCameraController,
    client: &ClientRuntime,
    figure: ActorFigureId,
) -> ActorInstance {
    let pose = camera.player().pose();
    let packed_light = client.packed_light_at_world_or_fullbright(BlockPos::containing(
        pose.position
            .add(Vec3d::new(0.0, LOCAL_PLAYER_STANDING_HEIGHT * 0.5, 0.0)),
    ));
    ActorInstance::local_player_with_figure(
        glam_vec3_from_vec3d(pose.position),
        pose.y_rot_degrees as f32,
        figure,
    )
    .with_packed_light(packed_light)
}

pub fn local_player_actor_instance_for_view(
    camera: &EngineCameraController,
    client: &ClientRuntime,
    figure: ActorFigureId,
) -> Option<ActorInstance> {
    match camera.view_mode() {
        EngineCameraViewMode::ThirdPersonBack => {
            Some(local_player_actor_instance(camera, client, figure))
        }
        EngineCameraViewMode::FirstPerson if camera.first_person_player_visible() => Some(
            local_player_actor_instance(camera, client, figure).with_first_person_body_only(true),
        ),
        EngineCameraViewMode::FirstPerson => None,
    }
}

pub fn actor_light_probe_block_pos(actor: &ActorPresentation) -> BlockPos {
    BlockPos::containing(actor.feet_position.add(Vec3d::new(
        0.0,
        actor_light_probe_height(actor),
        0.0,
    )))
}

pub fn actor_light_probe_height(actor: &ActorPresentation) -> f64 {
    match actor.kind {
        ActorPresentationKind::RemotePlayer => LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ActorPresentationKind::Entity(EntityKind::Cow) => 1.3,
        ActorPresentationKind::Entity(EntityKind::Chicken) => f64::from(actor.height) * 0.92,
        ActorPresentationKind::Entity(EntityKind::Mannequin) => 1.62,
        ActorPresentationKind::Entity(EntityKind::DebugCube) => f64::from(actor.height) * 0.5,
        ActorPresentationKind::Entity(EntityKind::Item) => f64::from(actor.height) * 0.5,
    }
}

pub fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn glam_quat_from_entity_rotation(rotation: mclone_protocol::EntityRotation) -> glam::Quat {
    glam::Quat::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w).normalize()
}
pub struct MeshChunkBlocks {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<mclone_core::BlockStateId>,
    pub biomes: Vec<i32>,
    pub light_sections: Vec<PackedLightSection>,
}

pub fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            bail!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            );
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            bail!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            );
        }
        let start = section_offset as usize * CHUNK_SECTION_VOLUME;
        for (index, state_id) in unpacked.into_iter().enumerate() {
            blocks[start + index] = state_id;
        }
    }

    Ok(MeshChunkBlocks {
        chunk_x: snapshot.pos.x,
        chunk_z: snapshot.pos.z,
        min_y: snapshot.min_y,
        height: snapshot.height,
        blocks,
        biomes: snapshot.biomes.clone(),
        light_sections: snapshot.light_sections.clone(),
    })
}
