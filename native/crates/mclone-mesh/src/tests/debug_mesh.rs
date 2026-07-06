use super::*;

#[test]
fn empty_mesh_stats_are_zero() {
    assert_eq!(SectionMeshStats::default().vertex_count, 0);
    assert_eq!(SectionMeshStats::default().index_count, 0);
    assert_eq!(SectionMeshStats::default().face_count(), 0);
}

#[test]
fn single_block_emits_six_faces() {
    let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

    assert_eq!(mesh.stats().vertex_count, 24);
    assert_eq!(mesh.stats().index_count, 36);
    assert_eq!(mesh.stats().face_count(), 6);
}

#[test]
fn adjacent_blocks_cull_shared_face() {
    let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, 1)]);
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

    assert_eq!(mesh.stats().vertex_count, 40);
    assert_eq!(mesh.stats().index_count, 60);
    assert_eq!(mesh.stats().face_count(), 10);
}

#[test]
fn solid_section_culls_internal_faces_in_debug_mesh() {
    let blocks = vec![1; (16 * 16 * 16) as usize];
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

    assert_eq!(mesh.stats().face_count(), 6 * 16 * 16);
    assert_eq!(mesh.stats().vertex_count, 6 * 16 * 16 * 4);
    assert_eq!(mesh.stats().index_count, 6 * 16 * 16 * 6);
}

#[test]
fn cave_air_is_invisible_and_non_occluding_in_debug_mesh() {
    let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, CAVE_AIR_BLOCK_ID)]);
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

    assert_eq!(mesh.stats().vertex_count, 24);
    assert_eq!(mesh.stats().index_count, 36);
}

#[test]
fn mesh_positions_use_world_chunk_offset() {
    let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(2, -1, 4, 16, &blocks));

    assert!(
        mesh.vertices
            .iter()
            .any(|vertex| vertex.position == [32.0, 4.0, -16.0])
    );
}

#[test]
fn area_mesh_culls_faces_across_chunk_boundaries() {
    let left = chunk_blocks(16, &[(15, 0, 0, 1)]);
    let right = chunk_blocks(16, &[(0, 0, 0, 1)]);
    let mesh = build_visible_chunk_area_mesh(&[
        ChunkMeshInput::new(0, 0, 0, 16, &left),
        ChunkMeshInput::new(1, 0, 0, 16, &right),
    ]);

    assert_eq!(mesh.stats().vertex_count, 40);
    assert_eq!(mesh.stats().index_count, 60);
}

#[test]
fn area_mesh_uses_euclidean_chunk_coordinates_for_negative_world_positions() {
    let chunk = chunk_blocks(16, &[(15, 0, 15, 1)]);
    assert_eq!(
        block_at_world_or_air(&[ChunkMeshInput::new(-1, -1, 0, 16, &chunk)], -1, 0, -1),
        1
    );
}
