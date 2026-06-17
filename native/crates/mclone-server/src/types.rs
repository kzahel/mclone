//! Server data/enum types with no entangled runtime behavior.
//!
//! Move-only home for the server's plain data types: mode/status enums, world
//! block positions, fluid kinds, chunk ticket types, chunk job identifiers, the
//! worldgen mailbox kind, and the dimension/level distance constants. Behavior
//! that reaches into scheduler/holder/fluid state stays in the parent module.

use mclone_core::ChunkPos;
use mclone_worldgen::block::{
    LAVA, RawBlockId, WATER, is_lava, is_water, lava_block_for_level, water_block_for_level,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMode {
    Integrated,
    Dedicated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkStatusStep {
    Scheduled,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkResidency {
    NotResident,
    Generated,
    LoadedFromStore,
    Saved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FullChunkStatus {
    Inaccessible,
    Border,
    Ticking,
    EntityTicking,
}

impl FullChunkStatus {
    pub fn is_or_after(self, status: Self) -> bool {
        self >= status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorldBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldBlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn below(self) -> Self {
        Self::new(self.x, self.y - 1, self.z)
    }

    pub(crate) fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub(crate) fn chunk_pos(self) -> ChunkPos {
        ChunkPos::from_block_coords(self.x, self.z)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FluidKind {
    Water,
    Lava,
}

impl FluidKind {
    pub const fn from_block_id(block_id: RawBlockId) -> Option<Self> {
        if is_water(block_id) {
            Some(Self::Water)
        } else if is_lava(block_id) {
            Some(Self::Lava)
        } else {
            None
        }
    }

    pub fn from_target(target: &str) -> Option<Self> {
        match target {
            "minecraft:water" | "minecraft:flowing_water" => Some(Self::Water),
            "minecraft:lava" | "minecraft:flowing_lava" => Some(Self::Lava),
            _ => None,
        }
    }

    pub const fn block_id(self) -> RawBlockId {
        match self {
            Self::Water => WATER,
            Self::Lava => LAVA,
        }
    }

    pub const fn block_for_level(self, level: u8) -> Option<RawBlockId> {
        match self {
            Self::Water => water_block_for_level(level),
            Self::Lava => lava_block_for_level(level),
        }
    }

    pub const fn tick_delay(self) -> i32 {
        match self {
            Self::Water => 5,
            Self::Lava => 30,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Water => "minecraft:water",
            Self::Lava => "minecraft:lava",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ChunkTicketType {
    Start,
    Dragon,
    Player,
    Forced,
    Light,
    Portal,
    PostTeleport,
    Unknown,
}

impl ChunkTicketType {
    const fn timeout_ticks(self) -> Option<u64> {
        match self {
            Self::Portal => Some(300),
            Self::PostTeleport => Some(5),
            Self::Unknown => Some(1),
            Self::Start | Self::Dragon | Self::Player | Self::Forced | Self::Light => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ChunkTicketKey {
    Chunk(ChunkPos),
    Named(String),
}

#[derive(Clone, Debug)]
pub struct ChunkTicket {
    pub ticket_type: ChunkTicketType,
    pub level: i32,
    pub key: ChunkTicketKey,
    pub created_tick: u64,
}

impl ChunkTicket {
    pub fn new(ticket_type: ChunkTicketType, level: i32, key: ChunkTicketKey) -> Self {
        Self {
            ticket_type,
            level,
            key,
            created_tick: 0,
        }
    }

    pub(crate) fn timed_out(&self, current_tick: u64) -> bool {
        self.ticket_type
            .timeout_ticks()
            .is_some_and(|timeout| current_tick.saturating_sub(self.created_tick) > timeout)
    }
}

impl PartialEq for ChunkTicket {
    fn eq(&self, other: &Self) -> bool {
        self.ticket_type == other.ticket_type && self.level == other.level && self.key == other.key
    }
}

impl Eq for ChunkTicket {}

impl PartialOrd for ChunkTicket {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkTicket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.level
            .cmp(&other.level)
            .then_with(|| self.ticket_type.cmp(&other.ticket_type))
            .then_with(|| self.key.cmp(&other.key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldgenMailboxKind {
    Inline,
    NativeThread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ChunkJobId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkJobState {
    Queued,
    Running,
    Complete,
}

pub const CHUNK_LEVEL_FULL: i32 = 33;
pub const PLAYER_TICKET_LEVEL: i32 = CHUNK_LEVEL_FULL - ENTITY_TICKING_RANGE;
pub const FORCED_TICKET_LEVEL: i32 = 31;
pub const MAX_CHUNK_DISTANCE: i32 = CHUNK_LEVEL_FULL + VANILLA_STATUS_AROUND_FULL_DISTANCE;
pub const UNLOADED_CHUNK_LEVEL: i32 = MAX_CHUNK_DISTANCE + 1;

const ENTITY_TICKING_RANGE: i32 = 2;
const VANILLA_STATUS_AROUND_FULL_DISTANCE: i32 = 11;
