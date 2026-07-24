use super::*;

#[test]
fn blocky_leaf_detail_emits_no_decorative_card_geometry() {
    let catalog = stone_and_leaves_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(8, 8, 8, BlockStateId(2))]);

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(catalog.leaf_detail(), LeafDetail::Blocky);
    assert_eq!(mesh.stats().face_count(), 6);
    assert_eq!(mesh.stats().vertex_count, 24);
    assert_eq!(mesh.cutout_index_count(), 36);
}

#[test]
fn bushy_leaf_detail_adds_four_surface_card_quads() {
    let catalog = stone_and_leaves_textured_catalog().with_leaf_detail(LeafDetail::Bushy);
    let blocks = textured_chunk_blocks(16, &[(8, 8, 8, BlockStateId(2))]);

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().face_count(), 10);
    assert_eq!(mesh.stats().vertex_count, 40);
    assert_eq!(mesh.cutout_index_count(), 60);
    assert!(mesh.vertices.iter().any(|vertex| {
        vertex.position[0] < 8.0
            || vertex.position[0] > 9.0
            || vertex.position[1] < 8.0
            || vertex.position[1] > 9.0
            || vertex.position[2] < 8.0
            || vertex.position[2] > 9.0
    }));
}

#[test]
fn bushy_leaf_detail_suppresses_cards_for_fully_enclosed_leaf() {
    let catalog = stone_and_leaves_textured_catalog().with_leaf_detail(LeafDetail::Bushy);
    let mut filled = Vec::new();
    for y in 7..=9 {
        for z in 7..=9 {
            for x in 7..=9 {
                filled.push((x, y, z, BlockStateId(2)));
            }
        }
    }
    let blocks = textured_chunk_blocks(16, &filled);

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    // Leaves retain their ordinary six cutout faces. The 26 surface leaves
    // receive four card quads each; the one fully enclosed center leaf does not.
    assert_eq!(mesh.stats().face_count(), 27 * 6 + 26 * 4);
}

#[test]
fn textured_liquid_source_emits_exposed_surface_faces() {
    let catalog = liquid_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(2))]);
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().face_count(), 7);
    assert_eq!(mesh.opaque_index_count(), 0);
    assert_eq!(mesh.solid_index_count(), 0);
    assert_eq!(mesh.cutout_index_count(), 0);
    assert_eq!(mesh.translucent_index_count(), mesh.stats().index_count);
    assert!(mesh.vertices.iter().any(|vertex| vertex.color[3] < 1.0));
    assert!(
        mesh.vertices.iter().all(
            |vertex| (0.0..=1.0).contains(&vertex.uv[0]) && (0.0..=1.0).contains(&vertex.uv[1])
        )
    );
}

#[test]
fn textured_liquid_top_emits_reverse_face_for_underwater_surface_visibility() {
    let catalog = liquid_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(2))]);
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(&mesh.indices[0..12], &[0, 1, 2, 0, 2, 3, 4, 7, 6, 4, 6, 5]);
}

#[test]
fn textured_liquid_top_renders_below_overhanging_solid_block() {
    let catalog = liquid_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(2)), (0, 1, 0, BlockStateId(1))],
    );
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();
    let water_top_color = [
        0x3f as f32 / 255.0,
        0x76 as f32 / 255.0,
        0xe4 as f32 / 255.0,
        0.72,
    ];
    let water_top_vertices = mesh
        .vertices
        .iter()
        .filter(|vertex| {
            vertex
                .color
                .iter()
                .zip(water_top_color)
                .all(|(actual, expected)| (actual - expected).abs() < 0.0001)
        })
        .count();

    assert_eq!(water_top_vertices, 8);
}

#[test]
fn textured_liquid_culls_internal_same_fluid_faces() {
    let catalog = liquid_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(2)), (1, 0, 0, BlockStateId(2))],
    );
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().face_count(), 12);
    assert_eq!(mesh.opaque_index_count(), 0);
    assert_eq!(mesh.solid_index_count(), 0);
    assert_eq!(mesh.cutout_index_count(), 0);
    assert_eq!(mesh.translucent_index_count(), mesh.stats().index_count);
}

#[test]
fn flowing_liquid_level_lowers_surface_height() {
    let catalog = liquid_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(72))]);
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();
    let max_y = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.position[1])
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(max_y > 0.0);
    assert!(max_y < 0.2, "level=1 surface should be low, got y={max_y}");
}

#[test]
fn textured_mesh_samples_packed_light_from_face_neighbor() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(8, 0, 8, BlockStateId(1))]);
    let mut sky = DataLayer::new();
    sky.set(8, 1, 8, 15);
    let light_sections = [PackedLightSection::new(0, sky.into_bytes(), None)];
    let input =
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks).with_light_sections(&light_sections);

    let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

    assert_eq!(mesh.stats().face_count(), 6);
    let top_face_light = pack_light(0, 15);
    assert!(mesh.vertices.chunks_exact(4).any(|face| {
        face.iter()
            .all(|vertex| vertex.packed_light == top_face_light)
    }));
    assert!(
        mesh.vertices
            .iter()
            .any(|vertex| vertex.packed_light != FULL_BRIGHT)
    );
}

#[test]
fn textured_mesh_treats_missing_sky_section_above_data_as_open_sky() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(32, &[(8, 15, 8, BlockStateId(1))]);
    let mut sky = DataLayer::new();
    sky.get_data();
    let light_sections = [PackedLightSection::new(0, sky.into_bytes(), None)];
    let input =
        TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks).with_light_sections(&light_sections);

    let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

    let top_face_light = pack_light(0, 15);
    assert!(mesh.vertices.chunks_exact(4).any(|face| {
        face.iter()
            .all(|vertex| vertex.packed_light == top_face_light)
    }));
}

#[test]
fn textured_mesh_climbs_to_next_sky_layer_for_missing_sections() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(48, &[(8, 15, 8, BlockStateId(1))]);
    let mut sky = DataLayer::new();
    sky.set(8, 0, 8, 4);
    let light_sections = [PackedLightSection::new(2, sky.into_bytes(), None)];
    let input =
        TexturedChunkMeshInput::new(0, 0, 0, 48, &blocks).with_light_sections(&light_sections);

    let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

    let top_face_light = pack_light(0, 4);
    assert!(mesh.vertices.chunks_exact(4).any(|face| {
        face.iter()
            .all(|vertex| vertex.packed_light == top_face_light)
    }));
}

#[test]
fn textured_mesh_uses_fullbright_without_light_payload() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(1))]);
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.packed_light == FULL_BRIGHT)
    );
}

#[test]
fn textured_mesh_applies_ambient_occlusion_on_full_cube_faces() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(1))],
    );

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();
    let top_face = mesh
        .vertices
        .chunks_exact(4)
        .find(|face| {
            face.iter().all(|vertex| {
                (vertex.position[1] - 1.0).abs() < 0.0001
                    && (0.0..=1.0).contains(&vertex.position[0])
                    && (0.0..=1.0).contains(&vertex.position[2])
            })
        })
        .expect("top face for origin block should be emitted");
    let west_brightness = top_face
        .iter()
        .filter(|vertex| (vertex.position[0] - 0.0).abs() < 0.0001)
        .map(|vertex| vertex.color[0])
        .sum::<f32>();
    let east_brightness = top_face
        .iter()
        .filter(|vertex| (vertex.position[0] - 1.0).abs() < 0.0001)
        .map(|vertex| vertex.color[0])
        .sum::<f32>();

    assert!(
        east_brightness < west_brightness,
        "east vertices beside the occluder should be darker"
    );
}

#[test]
fn textured_mesh_respects_model_ambient_occlusion_flag() {
    let catalog = stone_textured_catalog_with_ambient_occlusion(false);
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(1))],
    );

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();
    let top_face = mesh
        .vertices
        .chunks_exact(4)
        .find(|face| {
            face.iter().all(|vertex| {
                (vertex.position[1] - 1.0).abs() < 0.0001
                    && (0.0..=1.0).contains(&vertex.position[0])
                    && (0.0..=1.0).contains(&vertex.position[2])
            })
        })
        .expect("top face for origin block should be emitted");

    assert!(
        top_face
            .iter()
            .all(|vertex| (vertex.color[0] - top_face[0].color[0]).abs() < 0.0001),
        "ambientocclusion=false should keep flat face brightness"
    );
}

#[test]
fn textured_mesh_applies_ambient_occlusion_on_partial_faces() {
    let catalog = partial_and_stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(2))],
    );

    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();
    let partial_top_face = mesh
        .vertices
        .chunks_exact(4)
        .find(|face| {
            face.iter().all(|vertex| {
                (vertex.position[1] - 1.0).abs() < 0.0001
                    && (0.25..=0.75).contains(&vertex.position[0])
                    && (0.25..=0.75).contains(&vertex.position[2])
            })
        })
        .expect("partial block top face should be emitted");
    let min_brightness = partial_top_face
        .iter()
        .map(|vertex| vertex.color[0])
        .fold(f32::INFINITY, f32::min);
    let max_brightness = partial_top_face
        .iter()
        .map(|vertex| vertex.color[0])
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        min_brightness < max_brightness,
        "partial faces should receive per-vertex AO instead of flat brightness"
    );
}

#[test]
fn cave_air_is_invisible_and_non_occluding_in_textured_mesh() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[
            (0, 0, 0, BlockStateId(1)),
            (1, 0, 0, CAVE_AIR_BLOCK_STATE_ID),
        ],
    );
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().vertex_count, 24);
    assert_eq!(mesh.stats().index_count, 36);
    assert!(!catalog.occludes(CAVE_AIR_BLOCK_STATE_ID));
}

#[test]
fn textured_adjacent_blocks_cull_shared_face() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        16,
        &[(0, 0, 0, BlockStateId(1)), (1, 0, 0, BlockStateId(1))],
    );
    let mesh = build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        &catalog,
    )
    .unwrap();

    assert_eq!(mesh.stats().vertex_count, 40);
    assert_eq!(mesh.stats().index_count, 60);
    assert_eq!(mesh.stats().face_count(), 10);
}

#[test]
fn solid_textured_section_culls_internal_model_faces() {
    let catalog = stone_textured_catalog();
    let blocks = vec![BlockStateId(1); (16 * 16 * 16) as usize];
    let sections = build_textured_render_sections(
        &[TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks)],
        &catalog,
    )
    .unwrap();

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].stats().face_count(), 6 * 16 * 16);
    assert_eq!(sections[0].stats().vertex_count, 6 * 16 * 16 * 4);
    assert_eq!(sections[0].stats().index_count, 6 * 16 * 16 * 6);
    assert_visibility(
        sections[0].visibility,
        SectionFace::North,
        SectionFace::South,
        false,
    );
}

#[test]
fn adjacent_solid_textured_chunks_cull_shared_boundary_side() {
    let catalog = stone_textured_catalog();
    let left = vec![BlockStateId(1); (16 * 16 * 16) as usize];
    let right = vec![BlockStateId(1); (16 * 16 * 16) as usize];
    let sections = build_textured_render_sections(
        &[
            TexturedChunkMeshInput::new(0, 0, 0, 16, &left),
            TexturedChunkMeshInput::new(1, 0, 0, 16, &right),
        ],
        &catalog,
    )
    .unwrap();

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].stats().face_count(), 5 * 16 * 16);
    assert_eq!(sections[1].stats().face_count(), 5 * 16 * 16);
    assert_eq!(
        sections
            .iter()
            .map(|section| section.stats().face_count())
            .sum::<u32>(),
        2 * (32 * 16 + 32 * 16 + 16 * 16)
    );
}

#[test]
fn selective_textured_section_build_uses_neighbors_for_culling() {
    let catalog = stone_textured_catalog();
    let left = vec![BlockStateId(1); (16 * 16 * 16) as usize];
    let right = vec![BlockStateId(1); (16 * 16 * 16) as usize];
    let sections = build_textured_render_sections_for_chunk_set(
        &[
            TexturedChunkMeshInput::new(0, 0, 0, 16, &left),
            TexturedChunkMeshInput::new(1, 0, 0, 16, &right),
        ],
        &catalog,
        &BTreeSet::from([(1, 0)]),
    )
    .unwrap();

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].key, RenderSectionKey::new(1, 0, 0));
    assert_eq!(sections[0].stats().face_count(), 5 * 16 * 16);
}

#[test]
fn textured_render_sections_split_by_vertical_section() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        32,
        &[(0, 0, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
    );
    let sections = build_textured_render_sections(
        &[TexturedChunkMeshInput::new(2, -3, -16, 32, &blocks)],
        &catalog,
    )
    .unwrap();

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].key, RenderSectionKey::new(2, -1, -3));
    assert_eq!(sections[1].key, RenderSectionKey::new(2, 0, -3));
    assert_eq!(sections[0].stats().index_count, 36);
    assert_eq!(sections[1].stats().index_count, 36);
}

#[test]
fn textured_render_sections_cull_across_section_boundary() {
    let catalog = stone_textured_catalog();
    let blocks = textured_chunk_blocks(
        32,
        &[(0, 15, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
    );
    let sections = build_textured_render_sections(
        &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
        &catalog,
    )
    .unwrap();
    let combined = build_textured_visible_chunk_area_mesh(
        &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
        &catalog,
    )
    .unwrap();

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].stats().index_count, 30);
    assert_eq!(sections[1].stats().index_count, 30);
    assert_eq!(combined.stats().index_count, 60);
}
