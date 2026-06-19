use mclone_assets::ModelFaceDirection;

const DIRECTIONS: [ModelFaceDirection; 6] = [
    ModelFaceDirection::Down,
    ModelFaceDirection::Up,
    ModelFaceDirection::North,
    ModelFaceDirection::South,
    ModelFaceDirection::West,
    ModelFaceDirection::East,
];

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AmbientOcclusionShape {
    values: [f32; 12],
    pub use_face_neighbor: bool,
    use_non_cubic_weight: bool,
}

pub(crate) fn calculate_ambient_occlusion_shape(
    corners: [[f32; 3]; 4],
    direction: ModelFaceDirection,
    block_collision_shape_full: bool,
) -> AmbientOcclusionShape {
    const EPSILON: f32 = 1.0E-4;
    const FULL_FACE_EPSILON: f32 = 0.9999;

    let mut min_x = 32.0;
    let mut min_y = 32.0;
    let mut min_z = 32.0;
    let mut max_x = -32.0;
    let mut max_y = -32.0;
    let mut max_z = -32.0;

    for [x, y, z] in corners {
        min_x = f32::min(min_x, x);
        min_y = f32::min(min_y, y);
        min_z = f32::min(min_z, z);
        max_x = f32::max(max_x, x);
        max_y = f32::max(max_y, y);
        max_z = f32::max(max_z, z);
    }

    let mut values = [0.0; 12];
    values[shape_index(ModelFaceDirection::West, false)] = min_x;
    values[shape_index(ModelFaceDirection::East, false)] = max_x;
    values[shape_index(ModelFaceDirection::Down, false)] = min_y;
    values[shape_index(ModelFaceDirection::Up, false)] = max_y;
    values[shape_index(ModelFaceDirection::North, false)] = min_z;
    values[shape_index(ModelFaceDirection::South, false)] = max_z;
    values[shape_index(ModelFaceDirection::West, true)] = 1.0 - min_x;
    values[shape_index(ModelFaceDirection::East, true)] = 1.0 - max_x;
    values[shape_index(ModelFaceDirection::Down, true)] = 1.0 - min_y;
    values[shape_index(ModelFaceDirection::Up, true)] = 1.0 - max_y;
    values[shape_index(ModelFaceDirection::North, true)] = 1.0 - min_z;
    values[shape_index(ModelFaceDirection::South, true)] = 1.0 - max_z;

    let use_non_cubic_weight = match direction {
        ModelFaceDirection::Down | ModelFaceDirection::Up => {
            min_x >= EPSILON
                || min_z >= EPSILON
                || max_x <= FULL_FACE_EPSILON
                || max_z <= FULL_FACE_EPSILON
        }
        ModelFaceDirection::North | ModelFaceDirection::South => {
            min_x >= EPSILON
                || min_y >= EPSILON
                || max_x <= FULL_FACE_EPSILON
                || max_y <= FULL_FACE_EPSILON
        }
        ModelFaceDirection::West | ModelFaceDirection::East => {
            min_y >= EPSILON
                || min_z >= EPSILON
                || max_y <= FULL_FACE_EPSILON
                || max_z <= FULL_FACE_EPSILON
        }
    };
    let use_face_neighbor = match direction {
        ModelFaceDirection::Down => {
            min_y == max_y && (min_y < EPSILON || block_collision_shape_full)
        }
        ModelFaceDirection::Up => {
            min_y == max_y && (max_y > FULL_FACE_EPSILON || block_collision_shape_full)
        }
        ModelFaceDirection::North => {
            min_z == max_z && (min_z < EPSILON || block_collision_shape_full)
        }
        ModelFaceDirection::South => {
            min_z == max_z && (max_z > FULL_FACE_EPSILON || block_collision_shape_full)
        }
        ModelFaceDirection::West => {
            min_x == max_x && (min_x < EPSILON || block_collision_shape_full)
        }
        ModelFaceDirection::East => {
            min_x == max_x && (max_x > FULL_FACE_EPSILON || block_collision_shape_full)
        }
    };

    AmbientOcclusionShape {
        values,
        use_face_neighbor,
        use_non_cubic_weight,
    }
}

/// Java `ModelBlockRenderer.AmbientOcclusionFace.calculate(...)`.
pub(crate) fn calculate_ambient_occlusion_face(
    sampler: &impl AmbientOcclusionSampler,
    block_pos: BlockPos,
    direction: ModelFaceDirection,
    shape: AmbientOcclusionShape,
    shade: bool,
) -> AmbientOcclusionFace {
    let base_pos = if shape.use_face_neighbor {
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
    if shape.use_face_neighbor || !sampler.solid_render(neighbor_pos) {
        face_light = sampler.light_color(neighbor_pos);
    }
    let center_shade = sampler.shade_brightness(base_pos);
    let remap = ambient_vertex_remap(direction);
    let direction_shade = directional_shade(direction, shade);
    let corner_brightness = [
        average4(shade3, shade0, shade03, center_shade),
        average4(shade2, shade0, shade02, center_shade),
        average4(shade2, shade1, shade12, center_shade),
        average4(shade3, shade1, shade13, center_shade),
    ];
    let corner_light = [
        blend_light(light3, light0, light03, face_light),
        blend_light(light2, light0, light02, face_light),
        blend_light(light2, light1, light12, face_light),
        blend_light(light3, light1, light13, face_light),
    ];

    let mut result = AmbientOcclusionFace {
        brightness: [0.0; 4],
        lightmap: [0; 4],
    };
    if shape.use_non_cubic_weight && adjacency.do_non_cubic_weight {
        let weights = [
            shape_weights(shape, adjacency.vert0_weights),
            shape_weights(shape, adjacency.vert1_weights),
            shape_weights(shape, adjacency.vert2_weights),
            shape_weights(shape, adjacency.vert3_weights),
        ];
        result.brightness[remap.vert0] = weighted_brightness(corner_brightness, weights[0]);
        result.brightness[remap.vert1] = weighted_brightness(corner_brightness, weights[1]);
        result.brightness[remap.vert2] = weighted_brightness(corner_brightness, weights[2]);
        result.brightness[remap.vert3] = weighted_brightness(corner_brightness, weights[3]);
        result.lightmap[remap.vert0] = blend_light_weighted(corner_light, weights[0]);
        result.lightmap[remap.vert1] = blend_light_weighted(corner_light, weights[1]);
        result.lightmap[remap.vert2] = blend_light_weighted(corner_light, weights[2]);
        result.lightmap[remap.vert3] = blend_light_weighted(corner_light, weights[3]);
    } else {
        result.brightness[remap.vert0] = corner_brightness[0];
        result.brightness[remap.vert1] = corner_brightness[1];
        result.brightness[remap.vert2] = corner_brightness[2];
        result.brightness[remap.vert3] = corner_brightness[3];
        result.lightmap[remap.vert0] = corner_light[0];
        result.lightmap[remap.vert1] = corner_light[1];
        result.lightmap[remap.vert2] = corner_light[2];
        result.lightmap[remap.vert3] = corner_light[3];
    }

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

fn blend_light_weighted(lights: [u32; 4], weights: [f32; 4]) -> u32 {
    let high = ((lights[0] >> 16 & 0xFF) as f32 * weights[0]
        + (lights[1] >> 16 & 0xFF) as f32 * weights[1]
        + (lights[2] >> 16 & 0xFF) as f32 * weights[2]
        + (lights[3] >> 16 & 0xFF) as f32 * weights[3]) as u32
        & 0xFF;
    let low = ((lights[0] & 0xFF) as f32 * weights[0]
        + (lights[1] & 0xFF) as f32 * weights[1]
        + (lights[2] & 0xFF) as f32 * weights[2]
        + (lights[3] & 0xFF) as f32 * weights[3]) as u32
        & 0xFF;
    high << 16 | low
}

fn weighted_brightness(values: [f32; 4], weights: [f32; 4]) -> f32 {
    values[0] * weights[0]
        + values[1] * weights[1]
        + values[2] * weights[2]
        + values[3] * weights[3]
}

fn shape_weights(shape: AmbientOcclusionShape, weights: [SizeInfo; 8]) -> [f32; 4] {
    [
        shape.values[weights[0].shape_index()] * shape.values[weights[1].shape_index()],
        shape.values[weights[2].shape_index()] * shape.values[weights[3].shape_index()],
        shape.values[weights[4].shape_index()] * shape.values[weights[5].shape_index()],
        shape.values[weights[6].shape_index()] * shape.values[weights[7].shape_index()],
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SizeInfo {
    Down,
    Up,
    North,
    South,
    West,
    East,
    FlipDown,
    FlipUp,
    FlipNorth,
    FlipSouth,
    FlipWest,
    FlipEast,
}

impl SizeInfo {
    fn shape_index(self) -> usize {
        match self {
            Self::Down => shape_index(ModelFaceDirection::Down, false),
            Self::Up => shape_index(ModelFaceDirection::Up, false),
            Self::North => shape_index(ModelFaceDirection::North, false),
            Self::South => shape_index(ModelFaceDirection::South, false),
            Self::West => shape_index(ModelFaceDirection::West, false),
            Self::East => shape_index(ModelFaceDirection::East, false),
            Self::FlipDown => shape_index(ModelFaceDirection::Down, true),
            Self::FlipUp => shape_index(ModelFaceDirection::Up, true),
            Self::FlipNorth => shape_index(ModelFaceDirection::North, true),
            Self::FlipSouth => shape_index(ModelFaceDirection::South, true),
            Self::FlipWest => shape_index(ModelFaceDirection::West, true),
            Self::FlipEast => shape_index(ModelFaceDirection::East, true),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdjacencyInfo {
    corners: [ModelFaceDirection; 4],
    do_non_cubic_weight: bool,
    vert0_weights: [SizeInfo; 8],
    vert1_weights: [SizeInfo; 8],
    vert2_weights: [SizeInfo; 8],
    vert3_weights: [SizeInfo; 8],
}

fn adjacency_info(direction: ModelFaceDirection) -> AdjacencyInfo {
    use ModelFaceDirection::{Down, East, North, South, Up, West};
    use SizeInfo::{
        Down as SDown, East as SEast, FlipDown, FlipEast, FlipNorth, FlipSouth, FlipUp, FlipWest,
        North as SNorth, South as SSouth, Up as SUp, West as SWest,
    };

    match direction {
        Down => AdjacencyInfo {
            corners: [West, East, North, South],
            do_non_cubic_weight: true,
            vert0_weights: [
                FlipWest, SSouth, FlipWest, FlipSouth, SWest, FlipSouth, SWest, SSouth,
            ],
            vert1_weights: [
                FlipWest, SNorth, FlipWest, FlipNorth, SWest, FlipNorth, SWest, SNorth,
            ],
            vert2_weights: [
                FlipEast, SNorth, FlipEast, FlipNorth, SEast, FlipNorth, SEast, SNorth,
            ],
            vert3_weights: [
                FlipEast, SSouth, FlipEast, FlipSouth, SEast, FlipSouth, SEast, SSouth,
            ],
        },
        Up => AdjacencyInfo {
            corners: [East, West, North, South],
            do_non_cubic_weight: true,
            vert0_weights: [
                SEast, SSouth, SEast, FlipSouth, FlipEast, FlipSouth, FlipEast, SSouth,
            ],
            vert1_weights: [
                SEast, SNorth, SEast, FlipNorth, FlipEast, FlipNorth, FlipEast, SNorth,
            ],
            vert2_weights: [
                SWest, SNorth, SWest, FlipNorth, FlipWest, FlipNorth, FlipWest, SNorth,
            ],
            vert3_weights: [
                SWest, SSouth, SWest, FlipSouth, FlipWest, FlipSouth, FlipWest, SSouth,
            ],
        },
        North => AdjacencyInfo {
            corners: [Up, Down, East, West],
            do_non_cubic_weight: true,
            vert0_weights: [SUp, FlipWest, SUp, SWest, FlipUp, SWest, FlipUp, FlipWest],
            vert1_weights: [SUp, FlipEast, SUp, SEast, FlipUp, SEast, FlipUp, FlipEast],
            vert2_weights: [
                SDown, FlipEast, SDown, SEast, FlipDown, SEast, FlipDown, FlipEast,
            ],
            vert3_weights: [
                SDown, FlipWest, SDown, SWest, FlipDown, SWest, FlipDown, FlipWest,
            ],
        },
        South => AdjacencyInfo {
            corners: [West, East, Down, Up],
            do_non_cubic_weight: true,
            vert0_weights: [SUp, FlipWest, FlipUp, FlipWest, FlipUp, SWest, SUp, SWest],
            vert1_weights: [
                SDown, FlipWest, FlipDown, FlipWest, FlipDown, SWest, SDown, SWest,
            ],
            vert2_weights: [
                SDown, FlipEast, FlipDown, FlipEast, FlipDown, SEast, SDown, SEast,
            ],
            vert3_weights: [SUp, FlipEast, FlipUp, FlipEast, FlipUp, SEast, SUp, SEast],
        },
        West => AdjacencyInfo {
            corners: [Up, Down, North, South],
            do_non_cubic_weight: true,
            vert0_weights: [
                SUp, SSouth, SUp, FlipSouth, FlipUp, FlipSouth, FlipUp, SSouth,
            ],
            vert1_weights: [
                SUp, SNorth, SUp, FlipNorth, FlipUp, FlipNorth, FlipUp, SNorth,
            ],
            vert2_weights: [
                SDown, SNorth, SDown, FlipNorth, FlipDown, FlipNorth, FlipDown, SNorth,
            ],
            vert3_weights: [
                SDown, SSouth, SDown, FlipSouth, FlipDown, FlipSouth, FlipDown, SSouth,
            ],
        },
        East => AdjacencyInfo {
            corners: [Down, Up, North, South],
            do_non_cubic_weight: true,
            vert0_weights: [
                FlipDown, SSouth, FlipDown, FlipSouth, SDown, FlipSouth, SDown, SSouth,
            ],
            vert1_weights: [
                FlipDown, SNorth, FlipDown, FlipNorth, SDown, FlipNorth, SDown, SNorth,
            ],
            vert2_weights: [
                FlipUp, SNorth, FlipUp, FlipNorth, SUp, FlipNorth, SUp, SNorth,
            ],
            vert3_weights: [
                FlipUp, SSouth, FlipUp, FlipSouth, SUp, FlipSouth, SUp, SSouth,
            ],
        },
    }
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

fn direction_index(direction: ModelFaceDirection) -> usize {
    match direction {
        ModelFaceDirection::Down => 0,
        ModelFaceDirection::Up => 1,
        ModelFaceDirection::North => 2,
        ModelFaceDirection::South => 3,
        ModelFaceDirection::West => 4,
        ModelFaceDirection::East => 5,
    }
}

fn shape_index(direction: ModelFaceDirection, flipped: bool) -> usize {
    direction_index(direction) + if flipped { DIRECTIONS.len() } else { 0 }
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
        let shape = calculate_ambient_occlusion_shape(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            ModelFaceDirection::Up,
            true,
        );
        let face = calculate_ambient_occlusion_face(
            &sampler,
            BlockPos::new(0, 0, 0),
            ModelFaceDirection::Up,
            shape,
            true,
        );

        assert_eq!(face.lightmap, [FULL_BRIGHT; 4]);
        assert!(face.brightness[2] < face.brightness[0]);
        assert!(face.brightness[3] < face.brightness[1]);
    }

    #[test]
    fn calculate_shape_matches_java_partial_top_flags() {
        let shape = calculate_ambient_occlusion_shape(
            [
                [0.25, 1.0, 0.25],
                [0.25, 1.0, 0.75],
                [0.75, 1.0, 0.75],
                [0.75, 1.0, 0.25],
            ],
            ModelFaceDirection::Up,
            false,
        );

        assert!(shape.use_face_neighbor);
        assert!(shape.use_non_cubic_weight);
        assert_eq!(
            shape.values[shape_index(ModelFaceDirection::West, false)],
            0.25
        );
        assert_eq!(
            shape.values[shape_index(ModelFaceDirection::East, true)],
            0.25
        );
    }

    #[test]
    fn non_cubic_up_face_uses_java_size_info_weights() {
        let shape = calculate_ambient_occlusion_shape(
            [
                [0.25, 1.0, 0.25],
                [0.25, 1.0, 0.75],
                [0.75, 1.0, 0.75],
                [0.75, 1.0, 0.25],
            ],
            ModelFaceDirection::Up,
            false,
        );
        let adjacency = adjacency_info(ModelFaceDirection::Up);

        assert_eq!(
            shape_weights(shape, adjacency.vert0_weights),
            [0.5625, 0.1875, 0.0625, 0.1875]
        );
        assert_eq!(
            shape_weights(shape, adjacency.vert2_weights),
            [0.0625, 0.1875, 0.5625, 0.1875]
        );
    }

    #[test]
    fn flat_face_keeps_uniform_brightness_and_light() {
        let face = AmbientOcclusionFace::flat(ModelFaceDirection::East, true, FULL_BRIGHT);

        assert_eq!(face.brightness, [0.6; 4]);
        assert_eq!(face.lightmap, [FULL_BRIGHT; 4]);
    }
}
