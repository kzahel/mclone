//! Fluid material state and flow rules.
//!
//! Move-only home for the fluid value type (`NativeFluidState`), the fluid-space
//! `FluidDirection` and its direction tables, and the pure flow rules (drop-off,
//! slope distance, source conversion, replacement, legacy block mapping, and the
//! flow cache key). This mirrors Java's `world/level/material/*` (fluid state and
//! `FlowingFluid` rules) and stays distinct from the per-tick fluid scheduling,
//! which remains coupled to the chunk scheduler.

use mclone_worldgen::block::{RawBlockId, fluid_level};

use crate::{FluidKind, WorldBlockPos};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeFluidState {
    pub(crate) kind: Option<FluidKind>,
    pub(crate) amount: u8,
    pub(crate) falling: bool,
    pub(crate) source: bool,
}

impl NativeFluidState {
    pub(crate) const EMPTY: Self = Self {
        kind: None,
        amount: 0,
        falling: false,
        source: false,
    };

    pub(crate) const fn source(kind: FluidKind) -> Self {
        Self {
            kind: Some(kind),
            amount: 8,
            falling: false,
            source: true,
        }
    }

    pub(crate) const fn flowing(kind: FluidKind, amount: u8, falling: bool) -> Self {
        Self {
            kind: Some(kind),
            amount,
            falling,
            source: false,
        }
    }

    pub(crate) fn from_block_id(block_id: RawBlockId) -> Self {
        let Some(kind) = FluidKind::from_block_id(block_id) else {
            return Self::EMPTY;
        };
        match fluid_level(block_id).unwrap_or(0) {
            0 => Self::source(kind),
            8 => Self::flowing(kind, 8, true),
            level => Self::flowing(kind, 8_u8.saturating_sub(level), false),
        }
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.kind.is_none()
    }

    pub(crate) const fn is_same_fluid(self, fluid: FluidKind) -> bool {
        matches!(self.kind, Some(kind) if kind as u8 == fluid as u8)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FluidDirection {
    Down,
    Up,
    North,
    East,
    South,
    West,
}

impl FluidDirection {
    pub(crate) const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::East => (1, 0, 0),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
        }
    }

    pub(crate) const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

pub(crate) const HORIZONTAL_FLUID_DIRECTIONS: [FluidDirection; 4] = [
    FluidDirection::North,
    FluidDirection::East,
    FluidDirection::South,
    FluidDirection::West,
];

pub(crate) const ALL_FLUID_DIRECTIONS: [FluidDirection; 6] = [
    FluidDirection::Down,
    FluidDirection::Up,
    FluidDirection::North,
    FluidDirection::South,
    FluidDirection::West,
    FluidDirection::East,
];

pub(crate) const LAVA_SOURCE_CONTACT_DIRECTIONS: [FluidDirection; 5] = [
    FluidDirection::Up,
    FluidDirection::North,
    FluidDirection::South,
    FluidDirection::West,
    FluidDirection::East,
];

pub(crate) fn offset_pos(pos: WorldBlockPos, direction: FluidDirection) -> WorldBlockPos {
    let (dx, dy, dz) = direction.offset();
    pos.offset(dx, dy, dz)
}

pub(crate) fn legacy_block_for_fluid_state(state: NativeFluidState) -> Option<RawBlockId> {
    let fluid = state.kind?;
    let legacy_level = if state.source {
        0
    } else {
        8_u8.saturating_sub(state.amount.min(8))
            .saturating_add(if state.falling { 8 } else { 0 })
            .min(8)
    };
    fluid.block_for_level(legacy_level)
}

pub(crate) fn fluid_can_convert_to_source(fluid: FluidKind) -> bool {
    matches!(fluid, FluidKind::Water)
}

pub(crate) fn fluid_drop_off(fluid: FluidKind) -> u8 {
    match fluid {
        FluidKind::Water => 1,
        FluidKind::Lava => 2,
    }
}

pub(crate) fn fluid_slope_find_distance(fluid: FluidKind) -> i32 {
    match fluid {
        FluidKind::Water => 4,
        FluidKind::Lava => 2,
    }
}

pub(crate) fn target_fluid_can_be_replaced_with(
    target: NativeFluidState,
    incoming: FluidKind,
    direction: FluidDirection,
) -> bool {
    match target.kind {
        None => true,
        Some(FluidKind::Water) => direction == FluidDirection::Down && incoming != FluidKind::Water,
        Some(FluidKind::Lava) => direction == FluidDirection::Down && incoming != FluidKind::Lava,
    }
}

pub(crate) fn fluid_cache_key(origin: WorldBlockPos, pos: WorldBlockPos) -> i32 {
    let x = pos.x - origin.x;
    let z = pos.z - origin.z;
    ((x + 128) & 0xff) << 8 | ((z + 128) & 0xff)
}
