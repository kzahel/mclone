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
    build_render_sections_from_snapshots_with_biome_zoom_seed_and_topology(
        snapshots,
        catalog,
        target_sections,
        biome_zoom_seed,
        HorizontalTopology::UNBOUNDED,
    )
}

pub fn build_render_sections_from_snapshots_with_biome_zoom_seed_and_topology<
    S: std::borrow::Borrow<ChunkSnapshot>,
>(
    snapshots: &[S],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
) -> Result<TexturedRenderSectionBuildReport> {
    build_render_sections_from_snapshots_with_biome_zoom_seed_topology_and_grass(
        snapshots,
        catalog,
        target_sections,
        biome_zoom_seed,
        topology,
        false,
    )
}

pub fn build_render_sections_from_snapshots_with_biome_zoom_seed_topology_and_grass<
    S: std::borrow::Borrow<ChunkSnapshot>,
>(
    snapshots: &[S],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
    grass_patches: bool,
) -> Result<TexturedRenderSectionBuildReport> {
    // Generic over `Borrow<ChunkSnapshot>` so a caller holding owned snapshots
    // (`&[ChunkSnapshot]`, desktop + the web full-view helpers) and one holding borrowed
    // snapshots (`&[&ChunkSnapshot]`, the web worker's resident mirror, 067 Stage 4) both
    // build sections without forcing the latter to deep-clone its resident map every compile.
    let chunks = snapshots
        .iter()
        .map(|snapshot| snapshot_mesh_block_state_ids(snapshot.borrow()))
        .collect::<Result<Vec<_>>>()?;
    let inputs =
        textured_mesh_inputs_with_biome_zoom_seed_and_topology(&chunks, biome_zoom_seed, topology);
    build_textured_render_sections_for_section_set_with_stats_and_options(
        &inputs,
        catalog,
        target_sections,
        TexturedRenderSectionBuildOptions::OFF
            .with_topology(topology)
            .with_grass_patches(grass_patches),
    )
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
    textured_mesh_inputs_with_biome_zoom_seed_and_topology(
        chunks,
        biome_zoom_seed,
        HorizontalTopology::UNBOUNDED,
    )
}

pub fn textured_mesh_inputs_with_biome_zoom_seed_and_topology(
    chunks: &[MeshChunkBlocks],
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
) -> Vec<TexturedChunkMeshInput<'_>> {
    let mut inputs = Vec::new();
    for chunk in chunks {
        let x_coordinates = seam_alias_coordinates(topology.x, chunk.chunk_x);
        let z_coordinates = seam_alias_coordinates(topology.z, chunk.chunk_z);
        for chunk_x in x_coordinates {
            for chunk_z in z_coordinates.iter().copied() {
                let input = TexturedChunkMeshInput::new(
                    chunk_x,
                    chunk_z,
                    chunk.min_y,
                    chunk.height,
                    &chunk.blocks,
                )
                .with_biomes(&chunk.biomes)
                .with_light_sections(&chunk.light_sections);
                inputs.push(if let Some(seed) = biome_zoom_seed {
                    input.with_biome_zoom_seed(seed)
                } else {
                    input
                });
            }
        }
    }
    inputs
}

fn seam_alias_coordinates(axis: AxisTopology, canonical: i32) -> Vec<i32> {
    let mut coordinates = vec![canonical];
    if let AxisTopology::Periodic {
        minimum_chunk,
        period_chunks,
    } = axis
    {
        let period = i32::try_from(period_chunks).expect("validated topology period fits i32");
        if canonical == minimum_chunk {
            coordinates.push(canonical + period);
        }
        if canonical == minimum_chunk + period - 1 {
            coordinates.push(canonical - period);
        }
    }
    coordinates
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    #[test]
    fn cylinder_mesh_inputs_alias_only_the_two_canonical_seam_columns() {
        let blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        let chunks = [
            MeshChunkBlocks {
                chunk_x: 0,
                chunk_z: 0,
                min_y: 0,
                height: SECTION_HEIGHT,
                blocks: blocks.clone(),
                biomes: Vec::new(),
                light_sections: Vec::new(),
            },
            MeshChunkBlocks {
                chunk_x: 31,
                chunk_z: 0,
                min_y: 0,
                height: SECTION_HEIGHT,
                blocks,
                biomes: Vec::new(),
                light_sections: Vec::new(),
            },
        ];

        let inputs = textured_mesh_inputs_with_biome_zoom_seed_and_topology(
            &chunks,
            None,
            HorizontalTopology::cylinder_x(0, 32),
        );
        assert_eq!(
            inputs.iter().map(|input| input.chunk_x).collect::<Vec<_>>(),
            vec![0, 32, 31, -1]
        );
    }
}

pub fn actor_instances_from_presentations(
    presentations: &[ActorPresentation],
    client: &ClientRuntime,
) -> Vec<ActorInstance> {
    actor_instances_from_presentations_near_observer(presentations, client, Vec3d::ZERO)
}

/// A mallard figure's body begins roughly one quarter of its normalized height
/// above its feet. Sinking by that fraction puts the feet below a full water
/// surface while leaving the breast and most of the body above it.
pub const MALLARD_SWIM_VISUAL_SINK_HEIGHT_FACTOR: f32 = 0.24;

pub fn actor_instances_from_presentations_near_observer(
    presentations: &[ActorPresentation],
    client: &ClientRuntime,
    observer_lift: Vec3d,
) -> Vec<ActorInstance> {
    presentations
        .iter()
        .map(|actor| {
            let packed_light =
                client.packed_light_at_world_or_fullbright(actor_light_probe_block_pos(actor));
            let feet_position = client
                .topology()
                .nearest_position_lift(actor.feet_position, observer_lift);
            let instance = match actor.kind {
                ActorPresentationKind::RemotePlayer => actor
                    .appearance
                    .figure
                    .map_or_else(
                        || {
                            ActorInstance::remote_player(
                                glam_vec3_from_vec3d(feet_position),
                                actor.y_rot_degrees,
                            )
                        },
                        |figure| {
                            ActorInstance::remote_player_with_figure(
                                glam_vec3_from_vec3d(feet_position),
                                actor.y_rot_degrees,
                                figure,
                            )
                        },
                    )
                    .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Cow) => {
                    ActorInstance::remote_player_with_figure(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::cow_figure_id(),
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Chicken) => {
                    ActorInstance::remote_player_with_figure(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::chicken_figure_id(),
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_chicken_wing_flap_radians(actor.chicken_wing_flap_radians)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Mallard) => {
                    let is_duckling = actor.mallard_life_stage
                        == Some(mclone_protocol::MallardLifeStage::Duckling);
                    let presented_width = if is_duckling {
                        actor.width * 0.58
                    } else {
                        actor.width
                    };
                    let presented_height = if is_duckling {
                        actor.height * 0.62
                    } else {
                        actor.height
                    };
                    let mut presented_feet = glam_vec3_from_vec3d(feet_position);
                    if actor.in_water {
                        presented_feet.y -=
                            presented_height * MALLARD_SWIM_VISUAL_SINK_HEIGHT_FACTOR;
                    }
                    ActorInstance::remote_player_with_figure(
                        presented_feet,
                        actor.y_rot_degrees,
                        mclone_assets::mallard_duck_figure_id(),
                    )
                    .with_dimensions(presented_width, presented_height)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::MallardNest) => {
                    ActorInstance::mallard_nest(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        actor.width,
                        actor.height,
                    )
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Deer) => {
                    ActorInstance::remote_player_with_figure(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::deer_figure_id(),
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_hidden_part_prefix(
                        actor
                            .deer
                            .is_some_and(|deer| !deer.antlered)
                            .then_some("antler_"),
                    )
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Bee) => {
                    ActorInstance::remote_player_with_figure(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::bee_figure_id(),
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::DeerBed) => ActorInstance::semantic_prop(
                    glam_vec3_from_vec3d(feet_position),
                    actor.y_rot_degrees,
                    mclone_assets::deer_bed_figure_id(),
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::BeeNest) => ActorInstance::semantic_prop(
                    glam_vec3_from_vec3d(feet_position),
                    actor.y_rot_degrees,
                    mclone_assets::bee_nest_figure_id(),
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::BeeHotel) => {
                    ActorInstance::semantic_prop(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                        mclone_assets::bee_hotel_figure_id(),
                        actor.width,
                        actor.height,
                    )
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::Mannequin) => {
                    ActorInstance::remote_player(
                        glam_vec3_from_vec3d(feet_position),
                        actor.y_rot_degrees,
                    )
                    .with_dimensions(actor.width, actor.height)
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::DebugCube) => ActorInstance::debug_cube(
                    glam_vec3_from_vec3d(feet_position),
                    actor.y_rot_degrees,
                    actor.x_rot_degrees,
                    actor.rotation.map(glam_quat_from_entity_rotation),
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Item) => {
                    match actor.item_stack.map(|stack| stack.kind) {
                        Some(ItemKind::Egg | ItemKind::MallardEgg) | None => {
                            ActorInstance::item_egg(
                                glam_vec3_from_vec3d(feet_position),
                                actor.y_rot_degrees,
                                actor.width,
                                actor.height,
                            )
                            .with_packed_light(packed_light)
                        }
                        Some(ItemKind::MallardFeather) => ActorInstance::mallard_feather(
                            glam_vec3_from_vec3d(feet_position),
                            actor.y_rot_degrees,
                            actor.width,
                            actor.height,
                        )
                        .with_packed_light(packed_light),
                        Some(
                            kind @ (ItemKind::HuntingSpear
                            | ItemKind::Venison
                            | ItemKind::DeerHide
                            | ItemKind::ShedAntler
                            | ItemKind::BeeHotel
                            | ItemKind::Beeswax),
                        ) => ActorInstance::semantic_prop(
                            glam_vec3_from_vec3d(feet_position),
                            actor.y_rot_degrees,
                            match kind {
                                ItemKind::HuntingSpear => mclone_assets::hunting_spear_figure_id(),
                                ItemKind::Venison => mclone_assets::venison_figure_id(),
                                ItemKind::DeerHide => mclone_assets::deer_hide_figure_id(),
                                ItemKind::ShedAntler => mclone_assets::shed_antler_figure_id(),
                                ItemKind::BeeHotel => mclone_assets::bee_hotel_item_figure_id(),
                                ItemKind::Beeswax => mclone_assets::beeswax_figure_id(),
                                _ => unreachable!(),
                            },
                            actor.width,
                            actor.height,
                        )
                        .with_packed_light(packed_light),
                    }
                }
            };
            let instance = with_presentation_animation(instance, actor);
            let id = match actor.id {
                ActorPresentationId::RemotePlayer(id) => ActorInstanceId::RemotePlayer(id.0),
                ActorPresentationId::Entity(id) => ActorInstanceId::Entity(id.0),
            };
            instance.with_id(id)
        })
        .collect()
}

fn with_presentation_animation(
    instance: ActorInstance,
    actor: &ActorPresentation,
) -> ActorInstance {
    let Some(animation) = actor.animation else {
        return instance;
    };
    match animation.phase_source {
        mclone_core::AnimationPhaseSource::Distance => {
            instance.with_animation_distance(animation.clip, actor.walk_animation_distance)
        }
        mclone_core::AnimationPhaseSource::Elapsed => {
            let elapsed_ticks = actor
                .animation_clock_tick
                .saturating_sub(animation.start_tick);
            instance.with_animation_elapsed_seconds(
                animation.clip,
                elapsed_ticks as f32 / LOCAL_PLAYER_TICKS_PER_SECOND as f32,
            )
        }
    }
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
        ActorPresentationKind::Entity(EntityKind::Mallard) => f64::from(actor.height) * 0.82,
        ActorPresentationKind::Entity(EntityKind::MallardNest) => f64::from(actor.height) * 0.5,
        ActorPresentationKind::Entity(EntityKind::Deer) => f64::from(actor.height) * 0.88,
        ActorPresentationKind::Entity(EntityKind::DeerBed) => f64::from(actor.height) * 0.5,
        ActorPresentationKind::Entity(EntityKind::Bee) => f64::from(actor.height) * 0.5,
        ActorPresentationKind::Entity(EntityKind::BeeNest) => f64::from(actor.height) * 0.5,
        ActorPresentationKind::Entity(EntityKind::BeeHotel) => f64::from(actor.height) * 0.5,
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
