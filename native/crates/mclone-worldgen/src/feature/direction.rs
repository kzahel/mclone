use crate::placement::BlockPos;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub(crate) const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    pub(crate) const GLOW_LICHEN_VALID: [Self; 5] =
        [Self::Up, Self::North, Self::East, Self::South, Self::West];

    pub(crate) const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }

    pub(crate) const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    pub(crate) const fn axis(self) -> i32 {
        match self {
            Self::Down | Self::Up => 0,
            Self::North | Self::South => 1,
            Self::West | Self::East => 2,
        }
    }

    pub(crate) const fn bit(self) -> u8 {
        match self {
            Self::Down => 1 << 0,
            Self::Up => 1 << 1,
            Self::North => 1 << 2,
            Self::South => 1 << 3,
            Self::West => 1 << 4,
            Self::East => 1 << 5,
        }
    }
}

pub(crate) fn offset_pos(pos: BlockPos, direction: Direction) -> BlockPos {
    let (dx, dy, dz) = direction.offset();
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}
