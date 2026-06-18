use mclone_core::SECTION_HEIGHT;

pub type BlockPosKey = i64;
pub type SectionPosKey = i64;

const BLOCK_PACKED_X_LENGTH: i32 = 26;
const BLOCK_PACKED_Y_LENGTH: i32 = 12;
const BLOCK_PACKED_Z_LENGTH: i32 = 26;
const BLOCK_PACKED_X_MASK: i64 = (1_i64 << BLOCK_PACKED_X_LENGTH) - 1;
const BLOCK_PACKED_Y_MASK: i64 = (1_i64 << BLOCK_PACKED_Y_LENGTH) - 1;
const BLOCK_PACKED_Z_MASK: i64 = (1_i64 << BLOCK_PACKED_Z_LENGTH) - 1;
const BLOCK_Y_OFFSET: i32 = 0;
const BLOCK_Z_OFFSET: i32 = BLOCK_PACKED_Y_LENGTH;
const BLOCK_X_OFFSET: i32 = BLOCK_PACKED_Y_LENGTH + BLOCK_PACKED_Z_LENGTH;

const SECTION_PACKED_X_MASK: i64 = 4_194_303;
const SECTION_PACKED_Y_MASK: i64 = 1_048_575;
const SECTION_PACKED_Z_MASK: i64 = 4_194_303;
const SECTION_Y_OFFSET: i32 = 0;
const SECTION_Z_OFFSET: i32 = 20;
const SECTION_X_OFFSET: i32 = 42;

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
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    pub const HORIZONTALS: [Self; 4] = [Self::North, Self::South, Self::West, Self::East];

    pub fn step(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }
}

pub fn block_pos_as_long(x: i32, y: i32, z: i32) -> BlockPosKey {
    let mut packed = 0_i64;
    packed |= ((x as i64) & BLOCK_PACKED_X_MASK) << BLOCK_X_OFFSET;
    packed |= ((y as i64) & BLOCK_PACKED_Y_MASK) << BLOCK_Y_OFFSET;
    packed | (((z as i64) & BLOCK_PACKED_Z_MASK) << BLOCK_Z_OFFSET)
}

pub fn block_pos_get_x(pos: BlockPosKey) -> i32 {
    (pos << (64 - BLOCK_X_OFFSET - BLOCK_PACKED_X_LENGTH) >> (64 - BLOCK_PACKED_X_LENGTH)) as i32
}

pub fn block_pos_get_y(pos: BlockPosKey) -> i32 {
    (pos << (64 - BLOCK_PACKED_Y_LENGTH) >> (64 - BLOCK_PACKED_Y_LENGTH)) as i32
}

pub fn block_pos_get_z(pos: BlockPosKey) -> i32 {
    (pos << (64 - BLOCK_Z_OFFSET - BLOCK_PACKED_Z_LENGTH) >> (64 - BLOCK_PACKED_Z_LENGTH)) as i32
}

pub fn block_pos_offset(pos: BlockPosKey, dx: i32, dy: i32, dz: i32) -> BlockPosKey {
    block_pos_as_long(
        block_pos_get_x(pos) + dx,
        block_pos_get_y(pos) + dy,
        block_pos_get_z(pos) + dz,
    )
}

pub fn block_pos_flat_index(pos: BlockPosKey) -> BlockPosKey {
    pos & !15
}

pub fn section_as_long(x: i32, y: i32, z: i32) -> SectionPosKey {
    let mut packed = 0_i64;
    packed |= ((x as i64) & SECTION_PACKED_X_MASK) << SECTION_X_OFFSET;
    packed |= ((y as i64) & SECTION_PACKED_Y_MASK) << SECTION_Y_OFFSET;
    packed | (((z as i64) & SECTION_PACKED_Z_MASK) << SECTION_Z_OFFSET)
}

pub fn section_offset(section: SectionPosKey, dx: i32, dy: i32, dz: i32) -> SectionPosKey {
    section_as_long(
        section_x(section) + dx,
        section_y(section) + dy,
        section_z(section) + dz,
    )
}

pub fn section_x(section: SectionPosKey) -> i32 {
    (section >> 42) as i32
}

pub fn section_y(section: SectionPosKey) -> i32 {
    (section << 44 >> 44) as i32
}

pub fn section_z(section: SectionPosKey) -> i32 {
    (section << 22 >> 42) as i32
}

pub fn block_to_section_key(block: BlockPosKey) -> SectionPosKey {
    section_as_long(
        block_to_section_coord(block_pos_get_x(block)),
        block_to_section_coord(block_pos_get_y(block)),
        block_to_section_coord(block_pos_get_z(block)),
    )
}

pub fn section_get_zero_node(section: SectionPosKey) -> SectionPosKey {
    section & !SECTION_PACKED_Y_MASK
}

pub fn block_to_section_coord(block_coord: i32) -> i32 {
    block_coord >> 4
}

pub fn section_relative(block_coord: i32) -> i32 {
    block_coord & 15
}

pub fn section_to_block_coord(section_coord: i32) -> i32 {
    section_coord << 4
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightSectionRange {
    pub min_section: i32,
    pub section_count: i32,
    pub min_light_section: i32,
    pub light_section_count: i32,
}

impl LightSectionRange {
    pub fn from_world(min_y: i32, height: i32) -> Self {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "world min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "world height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let min_section = block_to_section_coord(min_y);
        let section_count = height / SECTION_HEIGHT;
        Self {
            min_section,
            section_count,
            min_light_section: min_section - 1,
            light_section_count: section_count + 2,
        }
    }

    pub fn max_light_section_exclusive(self) -> i32 {
        self.min_light_section + self.light_section_count
    }

    pub fn contains_light_section(self, section_y: i32) -> bool {
        (self.min_light_section..self.max_light_section_exclusive()).contains(&section_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_pos_key_matches_java_layout() {
        let pos = block_pos_as_long(1, 2, 3);
        assert_eq!(pos, 274_877_919_234);
        assert_eq!(block_pos_get_x(pos), 1);
        assert_eq!(block_pos_get_y(pos), 2);
        assert_eq!(block_pos_get_z(pos), 3);

        let negative = block_pos_as_long(-1, -64, -2);
        assert_eq!(block_pos_get_x(negative), -1);
        assert_eq!(block_pos_get_y(negative), -64);
        assert_eq!(block_pos_get_z(negative), -2);
    }

    #[test]
    fn section_pos_key_matches_java_layout() {
        let section = section_as_long(1, 2, 3);
        assert_eq!(section, 4_398_049_656_834);
        assert_eq!(section_x(section), 1);
        assert_eq!(section_y(section), 2);
        assert_eq!(section_z(section), 3);
        assert_eq!(section_get_zero_node(section), section_as_long(1, 0, 3));

        let negative = section_as_long(-1, -4, -2);
        assert_eq!(section_x(negative), -1);
        assert_eq!(section_y(negative), -4);
        assert_eq!(section_z(negative), -2);
    }

    #[test]
    fn section_helpers_match_java_floor_and_local_coords() {
        assert_eq!(block_to_section_coord(31), 1);
        assert_eq!(block_to_section_coord(16), 1);
        assert_eq!(block_to_section_coord(15), 0);
        assert_eq!(block_to_section_coord(-1), -1);
        assert_eq!(section_relative(-1), 15);
        assert_eq!(section_relative(16), 0);
        assert_eq!(section_to_block_coord(-2), -32);
    }

    #[test]
    fn block_to_section_key_uses_packed_block_coordinates() {
        let block = block_pos_as_long(31, -1, -17);
        let section = block_to_section_key(block);
        assert_eq!(section_x(section), 1);
        assert_eq!(section_y(section), -1);
        assert_eq!(section_z(section), -2);
    }

    #[test]
    fn light_section_range_adds_vertical_padding() {
        let range = LightSectionRange::from_world(-64, 384);

        assert_eq!(range.min_section, -4);
        assert_eq!(range.section_count, 24);
        assert_eq!(range.min_light_section, -5);
        assert_eq!(range.light_section_count, 26);
        assert!(range.contains_light_section(-5));
        assert!(range.contains_light_section(20));
        assert!(!range.contains_light_section(21));
    }
}
