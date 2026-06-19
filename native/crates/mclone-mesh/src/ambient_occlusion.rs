use mclone_assets::ModelFaceDirection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub(crate) const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    fn offset(self, direction: ModelFaceDirection) -> Self {
        let [dx, dy, dz] = direction_offset(direction);
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }
}

pub(crate) trait AmbientOcclusionSampler {
    fn light_color(&self, pos: BlockPos) -> u32;
    fn shade_brightness(&self, pos: BlockPos) -> f32;
    fn light_block(&self, pos: BlockPos) -> u8;
    fn view_blocking(&self, pos: BlockPos) -> bool;
    fn solid_render(&self, pos: BlockPos) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AmbientOcclusionFace {
    pub brightness: [f32; 4],
    pub lightmap: [u32; 4],
}

impl AmbientOcclusionFace {
    pub(crate) fn flat(direction: ModelFaceDirection, shade: bool, lightmap: u32) -> Self {
        Self {
            brightness: [directional_shade(direction, shade); 4],
            lightmap: [lightmap; 4],
        }
    }
}

/// Java `ModelBlockRenderer.AmbientOcclusionFace.calculate(...)` for cubic
/// block-model faces. The non-cubic shape-weight branch is a follow-up that can
/// live behind the same helper.
pub(crate) fn calculate_cubic_ambient_occlusion_face(
    sampler: &impl AmbientOcclusionSampler,
    block_pos: BlockPos,
    direction: ModelFaceDirection,
    use_face_neighbor: bool,
    shade: bool,
) -> AmbientOcclusionFace {
    let base_pos = if use_face_neighbor {
        block_pos.offset(direction)
    } else {
        block_pos
    };
    let adjacency = adjacency_info(direction);
    let side0 = base_pos.offset(adjacency.corners[0]);
    let side1 = base_pos.offset(adjacency.corners[1]);
    let side2 = base_pos.offset(adjacency.corners[2]);
    let side3 = base_pos.offset(adjacency.corners[3]);

    let light0 = sampler.light_color(side0);
    let shade0 = sampler.shade_brightness(side0);
    let light1 = sampler.light_color(side1);
    let shade1 = sampler.shade_brightness(side1);
    let light2 = sampler.light_color(side2);
    let shade2 = sampler.shade_brightness(side2);
    let light3 = sampler.light_color(side3);
    let shade3 = sampler.shade_brightness(side3);

    let free0 = is_free_for_corner(sampler, side0.offset(direction));
    let free1 = is_free_for_corner(sampler, side1.offset(direction));
    let free2 = is_free_for_corner(sampler, side2.offset(direction));
    let free3 = is_free_for_corner(sampler, side3.offset(direction));

    let (shade02, light02) = if !free2 && !free0 {
        (shade0, light0)
    } else {
        sample_brightness_and_light(sampler, side0.offset(adjacency.corners[2]))
    };
    let (shade03, light03) = if !free3 && !free0 {
        (shade0, light0)
    } else {
        sample_brightness_and_light(sampler, side0.offset(adjacency.corners[3]))
    };
    let (shade12, light12) = if !free2 && !free1 {
        (shade0, light0)
    } else {
        sample_brightness_and_light(sampler, side1.offset(adjacency.corners[2]))
    };
    let (shade13, light13) = if !free3 && !free1 {
        (shade0, light0)
    } else {
        sample_brightness_and_light(sampler, side1.offset(adjacency.corners[3]))
    };

    let mut face_light = sampler.light_color(block_pos);
    let neighbor_pos = block_pos.offset(direction);
    if use_face_neighbor || !sampler.solid_render(neighbor_pos) {
        face_light = sampler.light_color(neighbor_pos);
    }
    let center_shade = sampler.shade_brightness(base_pos);
    let remap = ambient_vertex_remap(direction);
    let direction_shade = directional_shade(direction, shade);

    let mut result = AmbientOcclusionFace {
        brightness: [0.0; 4],
        lightmap: [0; 4],
    };
    result.brightness[remap.vert0] = average4(shade3, shade0, shade03, center_shade);
    result.brightness[remap.vert1] = average4(shade2, shade0, shade02, center_shade);
    result.brightness[remap.vert2] = average4(shade2, shade1, shade12, center_shade);
    result.brightness[remap.vert3] = average4(shade3, shade1, shade13, center_shade);
    result.lightmap[remap.vert0] = blend_light(light3, light0, light03, face_light);
    result.lightmap[remap.vert1] = blend_light(light2, light0, light02, face_light);
    result.lightmap[remap.vert2] = blend_light(light2, light1, light12, face_light);
    result.lightmap[remap.vert3] = blend_light(light3, light1, light13, face_light);

    for brightness in &mut result.brightness {
        *brightness *= direction_shade;
    }
    result
}

pub(crate) fn directional_shade(direction: ModelFaceDirection, shade: bool) -> f32 {
    if !shade {
        return 1.0;
    }
    match direction {
        ModelFaceDirection::Down => 0.5,
        ModelFaceDirection::Up => 1.0,
        ModelFaceDirection::North | ModelFaceDirection::South => 0.8,
        ModelFaceDirection::West | ModelFaceDirection::East => 0.6,
    }
}

fn sample_brightness_and_light(
    sampler: &impl AmbientOcclusionSampler,
    pos: BlockPos,
) -> (f32, u32) {
    (sampler.shade_brightness(pos), sampler.light_color(pos))
}

fn is_free_for_corner(sampler: &impl AmbientOcclusionSampler, pos: BlockPos) -> bool {
    !sampler.view_blocking(pos) || sampler.light_block(pos) == 0
}

fn average4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    (a + b + c + d) * 0.25
}

fn blend_light(mut a: u32, mut b: u32, mut c: u32, d: u32) -> u32 {
    if a == 0 {
        a = d;
    }
    if b == 0 {
        b = d;
    }
    if c == 0 {
        c = d;
    }
    ((a + b + c + d) >> 2) & 0x00FF_00FF
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdjacencyInfo {
    corners: [ModelFaceDirection; 4],
}

fn adjacency_info(direction: ModelFaceDirection) -> AdjacencyInfo {
    let corners = match direction {
        ModelFaceDirection::Down => [
            ModelFaceDirection::West,
            ModelFaceDirection::East,
            ModelFaceDirection::North,
            ModelFaceDirection::South,
        ],
        ModelFaceDirection::Up => [
            ModelFaceDirection::East,
            ModelFaceDirection::West,
            ModelFaceDirection::North,
            ModelFaceDirection::South,
        ],
        ModelFaceDirection::North => [
            ModelFaceDirection::Up,
            ModelFaceDirection::Down,
            ModelFaceDirection::East,
            ModelFaceDirection::West,
        ],
        ModelFaceDirection::South => [
            ModelFaceDirection::West,
            ModelFaceDirection::East,
            ModelFaceDirection::Down,
            ModelFaceDirection::Up,
        ],
        ModelFaceDirection::West => [
            ModelFaceDirection::Up,
            ModelFaceDirection::Down,
            ModelFaceDirection::North,
            ModelFaceDirection::South,
        ],
        ModelFaceDirection::East => [
            ModelFaceDirection::Down,
            ModelFaceDirection::Up,
            ModelFaceDirection::North,
            ModelFaceDirection::South,
        ],
    };
    AdjacencyInfo { corners }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AmbientVertexRemap {
    vert0: usize,
    vert1: usize,
    vert2: usize,
    vert3: usize,
}

fn ambient_vertex_remap(direction: ModelFaceDirection) -> AmbientVertexRemap {
    match direction {
        ModelFaceDirection::Down | ModelFaceDirection::South => AmbientVertexRemap {
            vert0: 0,
            vert1: 1,
            vert2: 2,
            vert3: 3,
        },
        ModelFaceDirection::Up => AmbientVertexRemap {
            vert0: 2,
            vert1: 3,
            vert2: 0,
            vert3: 1,
        },
        ModelFaceDirection::North | ModelFaceDirection::West => AmbientVertexRemap {
            vert0: 3,
            vert1: 0,
            vert2: 1,
            vert3: 2,
        },
        ModelFaceDirection::East => AmbientVertexRemap {
            vert0: 1,
            vert1: 2,
            vert2: 3,
            vert3: 0,
        },
    }
}

fn direction_offset(direction: ModelFaceDirection) -> [i32; 3] {
    match direction {
        ModelFaceDirection::Down => [0, -1, 0],
        ModelFaceDirection::Up => [0, 1, 0],
        ModelFaceDirection::North => [0, 0, -1],
        ModelFaceDirection::South => [0, 0, 1],
        ModelFaceDirection::West => [-1, 0, 0],
        ModelFaceDirection::East => [1, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use mclone_light::FULL_BRIGHT;

    #[derive(Default)]
    struct TestSampler {
        solid: BTreeSet<BlockPosKey>,
    }

    impl TestSampler {
        fn with_solid(mut self, pos: BlockPos) -> Self {
            self.solid.insert(pos.into());
            self
        }

        fn is_solid(&self, pos: BlockPos) -> bool {
            self.solid.contains(&pos.into())
        }
    }

    impl AmbientOcclusionSampler for TestSampler {
        fn light_color(&self, _pos: BlockPos) -> u32 {
            FULL_BRIGHT
        }

        fn shade_brightness(&self, pos: BlockPos) -> f32 {
            if self.is_solid(pos) { 0.2 } else { 1.0 }
        }

        fn light_block(&self, pos: BlockPos) -> u8 {
            if self.is_solid(pos) { 15 } else { 0 }
        }

        fn view_blocking(&self, pos: BlockPos) -> bool {
            self.is_solid(pos)
        }

        fn solid_render(&self, pos: BlockPos) -> bool {
            self.is_solid(pos)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
    struct BlockPosKey(i32, i32, i32);

    impl From<BlockPos> for BlockPosKey {
        fn from(value: BlockPos) -> Self {
            Self(value.x, value.y, value.z)
        }
    }

    #[test]
    fn directional_shade_matches_clear_overworld_get_shade() {
        assert_eq!(directional_shade(ModelFaceDirection::Up, true), 1.0);
        assert_eq!(directional_shade(ModelFaceDirection::Down, true), 0.5);
        assert_eq!(directional_shade(ModelFaceDirection::North, true), 0.8);
        assert_eq!(directional_shade(ModelFaceDirection::South, true), 0.8);
        assert_eq!(directional_shade(ModelFaceDirection::West, true), 0.6);
        assert_eq!(directional_shade(ModelFaceDirection::East, true), 0.6);
        assert_eq!(directional_shade(ModelFaceDirection::East, false), 1.0);
    }

    #[test]
    fn cubic_top_face_darkens_vertices_next_to_occluder() {
        let sampler = TestSampler::default().with_solid(BlockPos::new(1, 1, 0));
        let face = calculate_cubic_ambient_occlusion_face(
            &sampler,
            BlockPos::new(0, 0, 0),
            ModelFaceDirection::Up,
            true,
            true,
        );

        assert_eq!(face.lightmap, [FULL_BRIGHT; 4]);
        assert!(face.brightness[2] < face.brightness[0]);
        assert!(face.brightness[3] < face.brightness[1]);
    }

    #[test]
    fn flat_face_keeps_uniform_brightness_and_light() {
        let face = AmbientOcclusionFace::flat(ModelFaceDirection::East, true, FULL_BRIGHT);

        assert_eq!(face.brightness, [0.6; 4]);
        assert_eq!(face.lightmap, [FULL_BRIGHT; 4]);
    }
}
