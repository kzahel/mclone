use super::*;

#[test]
fn textured_single_block_uses_baked_model_faces_and_atlas_uvs() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(1))]);
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().vertex_count, 24);
    assert_eq!(mesh.stats().index_count, 36);
    assert_eq!(mesh.solid_index_count(), mesh.stats().index_count);
    assert_eq!(mesh.cutout_index_count(), 0);
    assert_eq!(mesh.opaque_index_count(), mesh.stats().index_count);
    assert_eq!(mesh.translucent_index_count(), 0);
    assert!(catalog.occludes(BlockStateId(1)));
    assert!(!catalog.occludes(AIR_BLOCK_STATE_ID));
    assert!(
        mesh.vertices.iter().all(
            |vertex| (0.0..=1.0).contains(&vertex.uv[0]) && (0.0..=1.0).contains(&vertex.uv[1])
        )
    );
}

#[test]
fn catalog_applies_blockstate_variant_rotation_to_log_axis_faces() {
    let catalog = log_axis_textured_catalog();
    let y_axis = catalog.get(BlockStateId(1)).unwrap();
    let z_axis = catalog.get(BlockStateId(2)).unwrap();
    let x_axis = catalog.get(BlockStateId(3)).unwrap();
    let end_sprite = model_face_sprite(y_axis, ModelFaceDirection::Up);
    let side_sprite = model_face_sprite(y_axis, ModelFaceDirection::North);

    assert_ne!(end_sprite, side_sprite);
    assert_eq!(
        model_face_sprite(z_axis, ModelFaceDirection::North),
        end_sprite
    );
    assert_eq!(
        model_face_sprite(z_axis, ModelFaceDirection::South),
        end_sprite
    );
    assert_eq!(
        model_face_sprite(z_axis, ModelFaceDirection::Up),
        side_sprite
    );
    assert_eq!(
        model_face_sprite(x_axis, ModelFaceDirection::East),
        end_sprite
    );
    assert_eq!(
        model_face_sprite(x_axis, ModelFaceDirection::West),
        end_sprite
    );
    assert_eq!(
        model_face_sprite(x_axis, ModelFaceDirection::Up),
        side_sprite
    );
}

#[test]
fn catalog_applies_java_render_facts_to_full_cube_leaves() {
    let catalog = stone_and_leaves_textured_catalog();
    let stone = catalog.get(BlockStateId(1)).unwrap();
    let leaves = catalog.get(BlockStateId(2)).unwrap();

    assert!(stone.occludes);
    assert_eq!(stone.light_block, 15);
    assert!(stone.view_blocking);
    assert!(stone.solid_render);
    assert_eq!(stone.render_layer, TexturedTerrainRenderLayer::Solid);

    assert!(!leaves.occludes);
    assert_eq!(leaves.light_block, 1);
    assert!(!leaves.view_blocking);
    assert!(!leaves.solid_render);
    assert!(leaves.collision_shape_full_block);
    assert_eq!(leaves.render_layer, TexturedTerrainRenderLayer::Cutout);
}

#[test]
fn catalog_builds_fluid_models_from_level_states() {
    let catalog = liquid_textured_catalog();
    let source = catalog.get(BlockStateId(2)).unwrap().fluid.unwrap();
    let flowing = catalog.get(BlockStateId(72)).unwrap().fluid.unwrap();
    let lava = catalog.get(BlockStateId(9)).unwrap().fluid.unwrap();

    assert_eq!(source.kind, TexturedFluidKind::Water);
    assert_eq!(source.level, 0);
    assert_eq!(flowing.kind, TexturedFluidKind::Water);
    assert_eq!(flowing.level, 1);
    assert_eq!(lava.kind, TexturedFluidKind::Lava);
    assert_eq!(lava.level, 0);
}
