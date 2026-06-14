#![forbid(unsafe_code)]

pub const CHUNK_WIDTH: i32 = 16;
pub const AIR_BLOCK_ID: u8 = 0;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SectionMeshStats {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisibleChunkMesh {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<u32>,
}

impl VisibleChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn stats(&self) -> SectionMeshStats {
        SectionMeshStats {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChunkMeshInput<'a> {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: &'a [u8],
}

impl<'a> ChunkMeshInput<'a> {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, height: i32, blocks: &'a [u8]) -> Self {
        if height <= 0 || height % CHUNK_WIDTH != 0 {
            panic!("chunk height {height} must be a positive multiple of {CHUNK_WIDTH}");
        }
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        if blocks.len() != expected_len {
            panic!(
                "chunk mesh input has {} blocks; expected {expected_len}",
                blocks.len()
            );
        }
        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
        }
    }

    fn block_at_or_air(&self, local_x: i32, local_y: i32, local_z: i32) -> u8 {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return AIR_BLOCK_ID;
        }
        self.blocks[block_index(local_x, local_y, local_z)]
    }
}

pub fn build_visible_chunk_mesh(input: ChunkMeshInput<'_>) -> VisibleChunkMesh {
    build_visible_chunk_area_mesh(&[input])
}

pub fn build_visible_chunk_area_mesh(inputs: &[ChunkMeshInput<'_>]) -> VisibleChunkMesh {
    let mut mesh = VisibleChunkMesh::default();
    for input in inputs {
        add_chunk_to_mesh(&mut mesh, *input, inputs);
    }
    mesh
}

fn add_chunk_to_mesh(
    mesh: &mut VisibleChunkMesh,
    input: ChunkMeshInput<'_>,
    area: &[ChunkMeshInput<'_>],
) {
    let world_origin_x = chunk_world_origin(input.chunk_x);
    let world_origin_z = chunk_world_origin(input.chunk_z);

    for local_y in 0..input.height {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let block_id = input.block_at_or_air(local_x, local_y, local_z);
                if block_id == AIR_BLOCK_ID {
                    continue;
                }

                let world_x = world_origin_x + local_x;
                let world_y = input.min_y + local_y;
                let world_z = world_origin_z + local_z;
                for face in FACES {
                    let neighbor = block_at_world_or_air(
                        area,
                        world_x + face.neighbor[0],
                        world_y + face.neighbor[1],
                        world_z + face.neighbor[2],
                    );
                    if neighbor == AIR_BLOCK_ID {
                        add_face(mesh, world_x, world_y, world_z, block_id, face);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Face {
    neighbor: [i32; 3],
    corners: [[f32; 3]; 4],
    shade: f32,
}

const FACES: [Face; 6] = [
    Face {
        neighbor: [0, 1, 0],
        corners: [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        shade: 1.0,
    },
    Face {
        neighbor: [0, -1, 0],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        shade: 0.5,
    },
    Face {
        neighbor: [1, 0, 0],
        corners: [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
        shade: 0.86,
    },
    Face {
        neighbor: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        shade: 0.86,
    },
    Face {
        neighbor: [0, 0, 1],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        shade: 0.78,
    },
    Face {
        neighbor: [0, 0, -1],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
        shade: 0.72,
    },
];

fn add_face(
    mesh: &mut VisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block_id: u8,
    face: Face,
) {
    let base_index = mesh.vertices.len() as u32;
    let color = shaded_color(block_id, face.shade);
    for corner in face.corners {
        mesh.vertices.push(ChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            color,
        });
    }
    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn shaded_color(block_id: u8, shade: f32) -> [f32; 4] {
    let [r, g, b] = base_color(block_id);
    [r * shade, g * shade, b * shade, 1.0]
}

fn base_color(block_id: u8) -> [f32; 3] {
    match block_id {
        1 => [0.48, 0.48, 0.48],
        2 => [0.18, 0.34, 0.78],
        3 => [0.13, 0.12, 0.13],
        4 => [0.22, 0.56, 0.19],
        5 => [0.42, 0.28, 0.14],
        6 => [0.76, 0.68, 0.45],
        7 => [0.43, 0.42, 0.40],
        13 => [0.36, 0.24, 0.12],
        14 => [0.31, 0.24, 0.10],
        15 => [0.38, 0.28, 0.42],
        16 => [0.58, 0.32, 0.22],
        17 => [0.78, 0.70, 0.62],
        18 => [0.70, 0.36, 0.13],
        19 => [0.58, 0.28, 0.46],
        20 => [0.43, 0.52, 0.68],
        21 => [0.78, 0.66, 0.22],
        22 => [0.47, 0.60, 0.20],
        23 => [0.70, 0.42, 0.50],
        24 => [0.28, 0.25, 0.24],
        25 => [0.53, 0.49, 0.46],
        26 => [0.32, 0.45, 0.45],
        27 => [0.43, 0.28, 0.48],
        28 => [0.26, 0.32, 0.54],
        29 => [0.34, 0.22, 0.14],
        30 => [0.28, 0.36, 0.18],
        31 => [0.55, 0.25, 0.20],
        32 => [0.12, 0.10, 0.09],
        33 => [0.70, 0.62, 0.40],
        34 => [0.65, 0.35, 0.18],
        35 => [0.62, 0.78, 0.86],
        38 => [0.74, 0.35, 0.15],
        39 => [0.55, 0.76, 0.88],
        40 => [0.86, 0.90, 0.92],
        _ => [0.82, 0.22, 0.70],
    }
}

fn block_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    ((local_y << 8) | (local_z << 4) | local_x) as usize
}

fn block_at_world_or_air(inputs: &[ChunkMeshInput<'_>], world_x: i32, y: i32, world_z: i32) -> u8 {
    let chunk_x = world_x.div_euclid(CHUNK_WIDTH);
    let chunk_z = world_z.div_euclid(CHUNK_WIDTH);
    let local_x = world_x.rem_euclid(CHUNK_WIDTH);
    let local_z = world_z.rem_euclid(CHUNK_WIDTH);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.block_at_or_air(local_x, y - input.min_y, local_z))
        .unwrap_or(AIR_BLOCK_ID)
}

fn chunk_world_origin(chunk_coord: i32) -> i32 {
    chunk_coord * CHUNK_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_blocks(height: i32, filled: &[(i32, i32, i32, u8)]) -> Vec<u8> {
        let mut blocks = vec![AIR_BLOCK_ID; height as usize * 16 * 16];
        for &(x, y, z, block_id) in filled {
            blocks[block_index(x, y, z)] = block_id;
        }
        blocks
    }

    #[test]
    fn empty_mesh_stats_are_zero() {
        assert_eq!(SectionMeshStats::default().vertex_count, 0);
        assert_eq!(SectionMeshStats::default().index_count, 0);
    }

    #[test]
    fn single_block_emits_six_faces() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
    }

    #[test]
    fn adjacent_blocks_cull_shared_face() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
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
}
