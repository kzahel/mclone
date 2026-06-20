#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

const VIS_GRAPH_SIZE: usize = 4096;
const VIS_GRAPH_WORD_COUNT: usize = VIS_GRAPH_SIZE / u64::BITS as usize;
const VIS_GRAPH_MASK: usize = 15;
const VIS_GRAPH_LIGHT_OPAQUE_THRESHOLD: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SectionFace {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl SectionFace {
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn mask(self) -> u8 {
        1 << self.index()
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

    pub fn section_delta(self) -> [i32; 3] {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VisibilitySet {
    bits: u64,
}

impl VisibilitySet {
    const FACE_COUNT: usize = SectionFace::ALL.len();

    pub fn new() -> Self {
        Self::default()
    }

    pub fn all_visible() -> Self {
        let mut visibility = Self::new();
        visibility.set_all(true);
        visibility
    }

    pub fn set_all(&mut self, value: bool) {
        if value {
            self.bits = (1_u64 << (Self::FACE_COUNT * Self::FACE_COUNT)) - 1;
        } else {
            self.bits = 0;
        }
    }

    pub fn add_faces(&mut self, faces: u8) {
        for first in SectionFace::ALL {
            if faces & first.mask() == 0 {
                continue;
            }
            for second in SectionFace::ALL {
                if faces & second.mask() != 0 {
                    self.set(first, second, true);
                }
            }
        }
    }

    pub fn set(&mut self, first: SectionFace, second: SectionFace, value: bool) {
        self.set_one_way(first, second, value);
        self.set_one_way(second, first, value);
    }

    pub fn visibility_between(self, first: SectionFace, second: SectionFace) -> bool {
        self.bits & Self::pair_bit(first, second) != 0
    }

    pub fn bits(self) -> u64 {
        self.bits
    }

    pub fn from_bits(bits: u64) -> Self {
        let mask = (1_u64 << (Self::FACE_COUNT * Self::FACE_COUNT)) - 1;
        Self { bits: bits & mask }
    }

    fn set_one_way(&mut self, first: SectionFace, second: SectionFace, value: bool) {
        let bit = Self::pair_bit(first, second);
        if value {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }

    fn pair_bit(first: SectionFace, second: SectionFace) -> u64 {
        let index = first.index() + second.index() * Self::FACE_COUNT;
        1_u64 << index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisGraph {
    bitset: [u64; VIS_GRAPH_WORD_COUNT],
    empty: usize,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VisibilityGraphTimer {
    start: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl VisibilityGraphTimer {
    pub(crate) fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub(crate) fn elapsed_ms(self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VisibilityGraphTimer;

#[cfg(target_arch = "wasm32")]
impl VisibilityGraphTimer {
    pub(crate) fn start() -> Self {
        Self
    }

    pub(crate) fn elapsed_ms(self) -> f64 {
        0.0
    }
}

impl Default for VisGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl VisGraph {
    pub fn new() -> Self {
        Self {
            bitset: [0; VIS_GRAPH_WORD_COUNT],
            empty: VIS_GRAPH_SIZE,
        }
    }

    pub fn index(local_x: usize, local_y: usize, local_z: usize) -> usize {
        debug_assert!(local_x < 16);
        debug_assert!(local_y < 16);
        debug_assert!(local_z < 16);
        local_x | (local_z << 4) | (local_y << 8)
    }

    pub fn set_opaque_local(&mut self, local_x: usize, local_y: usize, local_z: usize) {
        self.set_opaque_index(Self::index(local_x, local_y, local_z));
    }

    pub fn set_opaque_index(&mut self, index: usize) {
        debug_assert!(index < VIS_GRAPH_SIZE);
        let word = index / u64::BITS as usize;
        let bit = 1_u64 << (index % u64::BITS as usize);
        if self.bitset[word] & bit == 0 {
            self.bitset[word] |= bit;
            self.empty -= 1;
        }
    }

    pub fn opaque_count(&self) -> usize {
        VIS_GRAPH_SIZE - self.empty
    }

    pub fn resolve(mut self) -> VisibilitySet {
        let mut visibility = VisibilitySet::new();
        if self.opaque_count() < VIS_GRAPH_LIGHT_OPAQUE_THRESHOLD {
            visibility.set_all(true);
        } else if self.empty == 0 {
            visibility.set_all(false);
        } else {
            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        if x != 0
                            && x != VIS_GRAPH_MASK
                            && y != 0
                            && y != VIS_GRAPH_MASK
                            && z != 0
                            && z != VIS_GRAPH_MASK
                        {
                            continue;
                        }
                        let index = Self::index(x, y, z);
                        if !self.is_set(index) {
                            let faces = self.flood_fill(index);
                            visibility.add_faces(faces);
                        }
                    }
                }
            }
        }

        visibility
    }

    fn flood_fill(&mut self, start: usize) -> u8 {
        let mut faces = 0_u8;
        let mut queue = [0_usize; VIS_GRAPH_SIZE];
        let mut read = 0_usize;
        let mut write = 0_usize;
        queue[write] = start;
        write += 1;
        self.set_opaque_index(start);

        while read < write {
            let index = queue[read];
            read += 1;
            faces |= edge_faces(index);

            for face in SectionFace::ALL {
                let Some(neighbor_index) = neighbor_index_at_face(index, face) else {
                    continue;
                };
                if !self.is_set(neighbor_index) {
                    self.set_opaque_index(neighbor_index);
                    queue[write] = neighbor_index;
                    write += 1;
                }
            }
        }

        faces
    }

    fn is_set(&self, index: usize) -> bool {
        let word = index / u64::BITS as usize;
        let bit = 1_u64 << (index % u64::BITS as usize);
        self.bitset[word] & bit != 0
    }
}

fn edge_faces(index: usize) -> u8 {
    let mut faces = 0_u8;
    let x = index & VIS_GRAPH_MASK;
    if x == 0 {
        faces |= SectionFace::West.mask();
    } else if x == VIS_GRAPH_MASK {
        faces |= SectionFace::East.mask();
    }

    let y = (index >> 8) & VIS_GRAPH_MASK;
    if y == 0 {
        faces |= SectionFace::Down.mask();
    } else if y == VIS_GRAPH_MASK {
        faces |= SectionFace::Up.mask();
    }

    let z = (index >> 4) & VIS_GRAPH_MASK;
    if z == 0 {
        faces |= SectionFace::North.mask();
    } else if z == VIS_GRAPH_MASK {
        faces |= SectionFace::South.mask();
    }

    faces
}

fn neighbor_index_at_face(index: usize, face: SectionFace) -> Option<usize> {
    match face {
        SectionFace::Down => {
            if (index >> 8) & VIS_GRAPH_MASK == 0 {
                None
            } else {
                Some(index - 256)
            }
        }
        SectionFace::Up => {
            if (index >> 8) & VIS_GRAPH_MASK == VIS_GRAPH_MASK {
                None
            } else {
                Some(index + 256)
            }
        }
        SectionFace::North => {
            if (index >> 4) & VIS_GRAPH_MASK == 0 {
                None
            } else {
                Some(index - 16)
            }
        }
        SectionFace::South => {
            if (index >> 4) & VIS_GRAPH_MASK == VIS_GRAPH_MASK {
                None
            } else {
                Some(index + 16)
            }
        }
        SectionFace::West => {
            if index & VIS_GRAPH_MASK == 0 {
                None
            } else {
                Some(index - 1)
            }
        }
        SectionFace::East => {
            if index & VIS_GRAPH_MASK == VIS_GRAPH_MASK {
                None
            } else {
                Some(index + 1)
            }
        }
    }
}
