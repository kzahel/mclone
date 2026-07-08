use crate::{ChunkPos, block_to_chunk_coord};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn containing(position: Vec3d) -> Self {
        Self::new(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        )
    }

    pub fn chunk_pos(self) -> ChunkPos {
        ChunkPos::from_block_coords(self.x, self.z)
    }

    pub const fn below(self) -> Self {
        Self::new(self.x, self.y - 1, self.z)
    }

    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub const fn relative(self, direction: Direction) -> Self {
        let [dx, dy, dz] = direction.step();
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Direction {
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Down),
            1 => Some(Self::Up),
            2 => Some(Self::North),
            3 => Some(Self::South),
            4 => Some(Self::West),
            5 => Some(Self::East),
            _ => None,
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    pub const fn step(self) -> [i32; 3] {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }

    pub fn nearest(x: f64, y: f64, z: f64) -> Self {
        let abs_x = x.abs();
        let abs_y = y.abs();
        let abs_z = z.abs();
        if abs_y >= abs_x && abs_y >= abs_z {
            if y >= 0.0 { Self::Up } else { Self::Down }
        } else if abs_z >= abs_x {
            if z >= 0.0 { Self::South } else { Self::North }
        } else if x >= 0.0 {
            Self::East
        } else {
            Self::West
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn subtract(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn scale(self, scale: f64) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }

    pub fn length_sqr(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn distance_to_sqr(self, other: Self) -> f64 {
        self.subtract(other).length_sqr()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl Aabb {
    pub fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
        Self {
            min_x: min_x.min(max_x),
            min_y: min_y.min(max_y),
            min_z: min_z.min(max_z),
            max_x: min_x.max(max_x),
            max_y: min_y.max(max_y),
            max_z: min_z.max(max_z),
        }
    }

    pub fn unit_block(pos: BlockPos) -> Self {
        Self::new(
            pos.x as f64,
            pos.y as f64,
            pos.z as f64,
            pos.x as f64 + 1.0,
            pos.y as f64 + 1.0,
            pos.z as f64 + 1.0,
        )
    }

    pub fn of_size(center: Vec3d, width: f64, height: f64, depth: f64) -> Self {
        Self::new(
            center.x - width / 2.0,
            center.y - height / 2.0,
            center.z - depth / 2.0,
            center.x + width / 2.0,
            center.y + height / 2.0,
            center.z + depth / 2.0,
        )
    }

    pub fn move_by(self, delta: Vec3d) -> Self {
        Self::new(
            self.min_x + delta.x,
            self.min_y + delta.y,
            self.min_z + delta.z,
            self.max_x + delta.x,
            self.max_y + delta.y,
            self.max_z + delta.z,
        )
    }

    pub fn expand_towards(self, delta: Vec3d) -> Self {
        Self::new(
            if delta.x < 0.0 {
                self.min_x + delta.x
            } else {
                self.min_x
            },
            if delta.y < 0.0 {
                self.min_y + delta.y
            } else {
                self.min_y
            },
            if delta.z < 0.0 {
                self.min_z + delta.z
            } else {
                self.min_z
            },
            if delta.x > 0.0 {
                self.max_x + delta.x
            } else {
                self.max_x
            },
            if delta.y > 0.0 {
                self.max_y + delta.y
            } else {
                self.max_y
            },
            if delta.z > 0.0 {
                self.max_z + delta.z
            } else {
                self.max_z
            },
        )
    }

    pub fn contains(self, point: Vec3d) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
            && point.z >= self.min_z
            && point.z <= self.max_z
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
            && self.min_z < other.max_z
            && self.max_z > other.min_z
    }

    pub fn is_finite(self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.min_z.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
            && self.max_z.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HitResultType {
    Miss,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHitResult {
    pub miss: bool,
    pub location: Vec3d,
    pub direction: Direction,
    pub block_pos: BlockPos,
    pub inside: bool,
}

impl BlockHitResult {
    pub const fn new(
        location: Vec3d,
        direction: Direction,
        block_pos: BlockPos,
        inside: bool,
    ) -> Self {
        Self {
            miss: false,
            location,
            direction,
            block_pos,
            inside,
        }
    }

    pub const fn miss(location: Vec3d, direction: Direction, block_pos: BlockPos) -> Self {
        Self {
            miss: true,
            location,
            direction,
            block_pos,
            inside: false,
        }
    }

    pub const fn hit_type(self) -> HitResultType {
        if self.miss {
            HitResultType::Miss
        } else {
            HitResultType::Block
        }
    }
}

pub fn block_pos_to_chunk_coord(pos: BlockPos) -> (i32, i32) {
    (block_to_chunk_coord(pos.x), block_to_chunk_coord(pos.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_pos_containing_floors_negative_coordinates() {
        assert_eq!(
            BlockPos::containing(Vec3d::new(-0.01, 63.9, -16.01)),
            BlockPos::new(-1, 63, -17)
        );
    }

    #[test]
    fn block_pos_relative_uses_minecraft_axis_conventions() {
        let pos = BlockPos::new(10, 20, 30);
        assert_eq!(pos.relative(Direction::Down), BlockPos::new(10, 19, 30));
        assert_eq!(pos.relative(Direction::Up), BlockPos::new(10, 21, 30));
        assert_eq!(pos.relative(Direction::North), BlockPos::new(10, 20, 29));
        assert_eq!(pos.relative(Direction::South), BlockPos::new(10, 20, 31));
        assert_eq!(pos.relative(Direction::West), BlockPos::new(9, 20, 30));
        assert_eq!(pos.relative(Direction::East), BlockPos::new(11, 20, 30));
    }

    #[test]
    fn direction_indices_are_stable_for_protocol() {
        for direction in Direction::ALL {
            assert_eq!(Direction::from_index(direction.index()), Some(direction));
        }
        assert_eq!(Direction::from_index(6), None);
    }

    #[test]
    fn nearest_direction_uses_largest_axis() {
        assert_eq!(Direction::nearest(0.2, -0.7, 0.1), Direction::Down);
        assert_eq!(Direction::nearest(0.2, 0.7, 0.1), Direction::Up);
        assert_eq!(Direction::nearest(0.2, 0.1, -0.7), Direction::North);
        assert_eq!(Direction::nearest(0.2, 0.1, 0.7), Direction::South);
        assert_eq!(Direction::nearest(-0.7, 0.1, 0.2), Direction::West);
        assert_eq!(Direction::nearest(0.7, 0.1, 0.2), Direction::East);
    }

    #[test]
    fn aabb_normalizes_bounds_and_uses_strict_intersection() {
        let a = Aabb::new(1.0, 2.0, 3.0, -1.0, -2.0, -3.0);
        assert_eq!(a.min_x, -1.0);
        assert_eq!(a.max_x, 1.0);

        let touching = Aabb::new(1.0, -2.0, -3.0, 2.0, 2.0, 3.0);
        assert!(!a.intersects(touching));

        let overlapping = Aabb::new(0.999, -2.0, -3.0, 2.0, 2.0, 3.0);
        assert!(a.intersects(overlapping));
    }

    #[test]
    fn aabb_expand_towards_matches_signed_java_behavior() {
        let aabb =
            Aabb::unit_block(BlockPos::new(1, 2, 3)).expand_towards(Vec3d::new(-2.0, 0.5, 4.0));

        assert_eq!(aabb, Aabb::new(-1.0, 2.0, 3.0, 2.0, 3.5, 8.0));
    }
}
