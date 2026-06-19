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
}
