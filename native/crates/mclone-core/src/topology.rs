use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{BlockPos, CHUNK_WIDTH, ChunkPos, Vec3d};

/// Chunk-aligned topology for one horizontal dimension axis.
///
/// Coordinates remain context-free everywhere else. A dimension owner must
/// explicitly apply this value whenever it crosses an authority boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AxisTopology {
    Unbounded,
    Finite {
        minimum_chunk: i32,
        maximum_chunk_exclusive: i32,
    },
    Periodic {
        minimum_chunk: i32,
        period_chunks: u32,
    },
}

impl Default for AxisTopology {
    fn default() -> Self {
        Self::Unbounded
    }
}

impl AxisTopology {
    pub const fn finite(minimum_chunk: i32, maximum_chunk_exclusive: i32) -> Self {
        Self::Finite {
            minimum_chunk,
            maximum_chunk_exclusive,
        }
    }

    pub const fn periodic(minimum_chunk: i32, period_chunks: u32) -> Self {
        Self::Periodic {
            minimum_chunk,
            period_chunks,
        }
    }

    pub fn validate(self) -> Result<(), TopologyError> {
        match self {
            Self::Unbounded => Ok(()),
            Self::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => {
                if maximum_chunk_exclusive <= minimum_chunk {
                    return Err(TopologyError::EmptyFiniteAxis {
                        minimum_chunk,
                        maximum_chunk_exclusive,
                    });
                }
                validate_block_aligned_extent(minimum_chunk, maximum_chunk_exclusive)
            }
            Self::Periodic {
                minimum_chunk,
                period_chunks,
            } => {
                if period_chunks == 0 {
                    return Err(TopologyError::ZeroPeriod);
                }
                let maximum_chunk_exclusive = i64::from(minimum_chunk) + i64::from(period_chunks);
                if maximum_chunk_exclusive > i64::from(i32::MAX) {
                    return Err(TopologyError::ChunkExtentOverflow);
                }
                validate_block_aligned_extent(minimum_chunk, maximum_chunk_exclusive as i32)
            }
        }
    }

    pub const fn is_unbounded(self) -> bool {
        matches!(self, Self::Unbounded)
    }

    pub const fn period_chunks(self) -> Option<u32> {
        match self {
            Self::Periodic { period_chunks, .. } => Some(period_chunks),
            Self::Unbounded | Self::Finite { .. } => None,
        }
    }

    pub fn canonical_chunk(self, coordinate: i32) -> Option<i32> {
        self.canonical_chunk_i64(i64::from(coordinate))
    }

    pub fn canonical_block(self, coordinate: i32) -> Option<i32> {
        match self {
            Self::Unbounded => Some(coordinate),
            Self::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => {
                let minimum = i64::from(minimum_chunk) * i64::from(CHUNK_WIDTH);
                let maximum = i64::from(maximum_chunk_exclusive) * i64::from(CHUNK_WIDTH);
                let coordinate = i64::from(coordinate);
                (minimum..maximum)
                    .contains(&coordinate)
                    .then_some(coordinate as i32)
            }
            Self::Periodic {
                minimum_chunk,
                period_chunks,
            } => {
                let minimum = i64::from(minimum_chunk) * i64::from(CHUNK_WIDTH);
                let period = i64::from(period_chunks) * i64::from(CHUNK_WIDTH);
                Some((minimum + (i64::from(coordinate) - minimum).rem_euclid(period)) as i32)
            }
        }
    }

    pub fn canonical_position(self, coordinate: f64) -> Option<f64> {
        if !coordinate.is_finite() {
            return None;
        }
        match self {
            Self::Unbounded => Some(coordinate),
            Self::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => {
                let minimum = f64::from(minimum_chunk) * f64::from(CHUNK_WIDTH);
                let maximum = f64::from(maximum_chunk_exclusive) * f64::from(CHUNK_WIDTH);
                (minimum..maximum)
                    .contains(&coordinate)
                    .then_some(coordinate)
            }
            Self::Periodic {
                minimum_chunk,
                period_chunks,
            } => {
                let minimum = f64::from(minimum_chunk) * f64::from(CHUNK_WIDTH);
                let period = f64::from(period_chunks) * f64::from(CHUNK_WIDTH);
                Some(minimum + (coordinate - minimum).rem_euclid(period))
            }
        }
    }

    pub fn shortest_chunk_displacement(self, from: i32, to: i32) -> i64 {
        match self {
            Self::Periodic { period_chunks, .. } => shortest_integer_displacement(
                i64::from(from),
                i64::from(to),
                i64::from(period_chunks),
            ),
            Self::Unbounded | Self::Finite { .. } => i64::from(to) - i64::from(from),
        }
    }

    pub fn shortest_block_displacement(self, from: i32, to: i32) -> i64 {
        match self {
            Self::Periodic { period_chunks, .. } => shortest_integer_displacement(
                i64::from(from),
                i64::from(to),
                i64::from(period_chunks) * i64::from(CHUNK_WIDTH),
            ),
            Self::Unbounded | Self::Finite { .. } => i64::from(to) - i64::from(from),
        }
    }

    pub fn shortest_position_displacement(self, from: f64, to: f64) -> f64 {
        match self {
            Self::Periodic { period_chunks, .. } => {
                let period = f64::from(period_chunks) * f64::from(CHUNK_WIDTH);
                let delta = (to - from).rem_euclid(period);
                if delta > period * 0.5 {
                    delta - period
                } else {
                    delta
                }
            }
            Self::Unbounded | Self::Finite { .. } => to - from,
        }
    }

    pub fn nearest_chunk_lift(self, canonical: i32, observer_lift: f64) -> i64 {
        match self {
            Self::Periodic { period_chunks, .. } => {
                let reference_canonical = self
                    .canonical_position(observer_lift * f64::from(CHUNK_WIDTH))
                    .expect("finite observer lift must canonicalize")
                    / f64::from(CHUNK_WIDTH);
                let delta = shortest_float_displacement(
                    reference_canonical,
                    f64::from(canonical),
                    f64::from(period_chunks),
                );
                (observer_lift + delta).round() as i64
            }
            Self::Unbounded | Self::Finite { .. } => i64::from(canonical),
        }
    }

    pub fn nearest_position_lift(self, canonical: f64, observer_lift: f64) -> f64 {
        match self {
            Self::Periodic { .. } => {
                let reference_canonical = self
                    .canonical_position(observer_lift)
                    .expect("finite observer lift must canonicalize");
                observer_lift + self.shortest_position_displacement(reference_canonical, canonical)
            }
            Self::Unbounded | Self::Finite { .. } => canonical,
        }
    }

    fn canonical_chunk_i64(self, coordinate: i64) -> Option<i32> {
        match self {
            Self::Unbounded => i32::try_from(coordinate).ok(),
            Self::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => (i64::from(minimum_chunk)..i64::from(maximum_chunk_exclusive))
                .contains(&coordinate)
                .then_some(coordinate as i32),
            Self::Periodic {
                minimum_chunk,
                period_chunks,
            } => {
                let minimum = i64::from(minimum_chunk);
                let period = i64::from(period_chunks);
                Some((minimum + (coordinate - minimum).rem_euclid(period)) as i32)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct HorizontalTopology {
    pub x: AxisTopology,
    pub z: AxisTopology,
}

impl HorizontalTopology {
    pub const UNBOUNDED: Self = Self {
        x: AxisTopology::Unbounded,
        z: AxisTopology::Unbounded,
    };

    pub const fn new(x: AxisTopology, z: AxisTopology) -> Self {
        Self { x, z }
    }

    pub const fn cylinder_x(minimum_chunk: i32, period_chunks: u32) -> Self {
        Self::new(
            AxisTopology::periodic(minimum_chunk, period_chunks),
            AxisTopology::Unbounded,
        )
    }

    pub fn validate(self) -> Result<(), TopologyError> {
        self.x.validate()?;
        self.z.validate()
    }

    pub const fn is_unbounded(self) -> bool {
        self.x.is_unbounded() && self.z.is_unbounded()
    }

    pub fn canonicalize_chunk(self, pos: ChunkPos) -> Option<ChunkPos> {
        Some(ChunkPos::new(
            self.x.canonical_chunk(pos.x)?,
            self.z.canonical_chunk(pos.z)?,
        ))
    }

    pub fn canonicalize_block(self, pos: BlockPos) -> Option<BlockPos> {
        Some(BlockPos::new(
            self.x.canonical_block(pos.x)?,
            pos.y,
            self.z.canonical_block(pos.z)?,
        ))
    }

    pub fn canonicalize_position(self, position: Vec3d) -> Option<Vec3d> {
        Some(Vec3d::new(
            self.x.canonical_position(position.x)?,
            position.y,
            self.z.canonical_position(position.z)?,
        ))
        .filter(|position| position.is_finite())
    }

    pub fn neighbor_chunk(self, pos: ChunkPos, dx: i32, dz: i32) -> Option<ChunkPos> {
        Some(ChunkPos::new(
            self.x
                .canonical_chunk_i64(i64::from(pos.x) + i64::from(dx))?,
            self.z
                .canonical_chunk_i64(i64::from(pos.z) + i64::from(dz))?,
        ))
    }

    pub fn neighbor_block(self, pos: BlockPos, dx: i32, dy: i32, dz: i32) -> Option<BlockPos> {
        let x = i64::from(pos.x) + i64::from(dx);
        let y = i64::from(pos.y) + i64::from(dy);
        let z = i64::from(pos.z) + i64::from(dz);
        let raw = BlockPos::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
            i32::try_from(z).ok()?,
        );
        self.canonicalize_block(raw)
    }

    pub fn shortest_chunk_displacement(self, from: ChunkPos, to: ChunkPos) -> [i64; 2] {
        [
            self.x.shortest_chunk_displacement(from.x, to.x),
            self.z.shortest_chunk_displacement(from.z, to.z),
        ]
    }

    pub fn shortest_position_displacement(self, from: Vec3d, to: Vec3d) -> Vec3d {
        Vec3d::new(
            self.x.shortest_position_displacement(from.x, to.x),
            to.y - from.y,
            self.z.shortest_position_displacement(from.z, to.z),
        )
    }

    pub fn validate_one_lift_radius(self, radius: u32) -> Result<(), TopologyError> {
        for axis in [self.x, self.z] {
            if let Some(period) = axis.period_chunks()
                && u64::from(radius) * 2 + 1 > u64::from(period)
            {
                return Err(TopologyError::ViewContainsDuplicateLift { radius, period });
            }
        }
        Ok(())
    }

    pub fn chunk_view(
        self,
        center_lift: ChunkPos,
        radius: u32,
    ) -> Result<Vec<ChunkLift>, TopologyError> {
        self.validate()?;
        self.validate_one_lift_radius(radius)?;
        let radius = i32::try_from(radius).map_err(|_| TopologyError::ViewRadiusOverflow)?;
        let mut canonical_seen = BTreeSet::new();
        let mut result = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let lifted_x = i64::from(center_lift.x) + i64::from(dx);
                let lifted_z = i64::from(center_lift.z) + i64::from(dz);
                let Some(canonical) = (|| {
                    Some(ChunkPos::new(
                        self.x.canonical_chunk_i64(lifted_x)?,
                        self.z.canonical_chunk_i64(lifted_z)?,
                    ))
                })() else {
                    continue;
                };
                if !canonical_seen.insert(canonical) {
                    return Err(TopologyError::DuplicateCanonicalChunk(canonical));
                }
                result.push(ChunkLift {
                    canonical,
                    lifted: LiftedChunkPos::new(lifted_x, lifted_z),
                });
            }
        }
        Ok(result)
    }

    pub fn nearest_chunk_lift(self, canonical: ChunkPos, observer_lift: Vec3d) -> LiftedChunkPos {
        LiftedChunkPos::new(
            self.x
                .nearest_chunk_lift(canonical.x, observer_lift.x / f64::from(CHUNK_WIDTH)),
            self.z
                .nearest_chunk_lift(canonical.z, observer_lift.z / f64::from(CHUNK_WIDTH)),
        )
    }

    pub fn nearest_position_lift(self, canonical: Vec3d, observer_lift: Vec3d) -> Vec3d {
        Vec3d::new(
            self.x.nearest_position_lift(canonical.x, observer_lift.x),
            canonical.y,
            self.z.nearest_position_lift(canonical.z, observer_lift.z),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiftedChunkPos {
    pub x: i64,
    pub z: i64,
}

impl LiftedChunkPos {
    pub const fn new(x: i64, z: i64) -> Self {
        Self { x, z }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ChunkLift {
    pub canonical: ChunkPos,
    pub lifted: LiftedChunkPos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyError {
    EmptyFiniteAxis {
        minimum_chunk: i32,
        maximum_chunk_exclusive: i32,
    },
    ZeroPeriod,
    ChunkExtentOverflow,
    BlockExtentOverflow,
    ViewContainsDuplicateLift {
        radius: u32,
        period: u32,
    },
    ViewRadiusOverflow,
    DuplicateCanonicalChunk(ChunkPos),
}

impl fmt::Display for TopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFiniteAxis {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => write!(
                formatter,
                "finite topology axis [{minimum_chunk}, {maximum_chunk_exclusive}) is empty"
            ),
            Self::ZeroPeriod => formatter.write_str("periodic topology axis has zero period"),
            Self::ChunkExtentOverflow => {
                formatter.write_str("topology chunk extent exceeds signed coordinates")
            }
            Self::BlockExtentOverflow => {
                formatter.write_str("topology chunk extent exceeds signed block coordinates")
            }
            Self::ViewContainsDuplicateLift { radius, period } => write!(
                formatter,
                "view radius {radius} would contain duplicate lifts for period {period}"
            ),
            Self::ViewRadiusOverflow => {
                formatter.write_str("topology view radius exceeds signed coordinates")
            }
            Self::DuplicateCanonicalChunk(pos) => write!(
                formatter,
                "topology view produced duplicate canonical chunk ({}, {})",
                pos.x, pos.z
            ),
        }
    }
}

impl Error for TopologyError {}

fn validate_block_aligned_extent(
    minimum_chunk: i32,
    maximum_chunk_exclusive: i32,
) -> Result<(), TopologyError> {
    let minimum_block = i64::from(minimum_chunk) * i64::from(CHUNK_WIDTH);
    let maximum_block = i64::from(maximum_chunk_exclusive) * i64::from(CHUNK_WIDTH);
    if minimum_block < i64::from(i32::MIN) || maximum_block > i64::from(i32::MAX) {
        Err(TopologyError::BlockExtentOverflow)
    } else {
        Ok(())
    }
}

fn shortest_integer_displacement(from: i64, to: i64, period: i64) -> i64 {
    let delta = (to - from).rem_euclid(period);
    if delta * 2 > period {
        delta - period
    } else {
        delta
    }
}

fn shortest_float_displacement(from: f64, to: f64, period: f64) -> f64 {
    let delta = (to - from).rem_euclid(period);
    if delta > period * 0.5 {
        delta - period
    } else {
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CYLINDER: HorizontalTopology = HorizontalTopology::cylinder_x(0, 32);

    #[test]
    fn unbounded_topology_is_an_exact_identity() {
        for coordinate in [i32::MIN, -33, -1, 0, 1, 77, i32::MAX] {
            assert_eq!(
                AxisTopology::Unbounded.canonical_chunk(coordinate),
                Some(coordinate)
            );
            assert_eq!(
                AxisTopology::Unbounded.canonical_block(coordinate),
                Some(coordinate)
            );
        }
        let pos = Vec3d::new(-9.25, 65.0, 17.75);
        assert_eq!(
            HorizontalTopology::UNBOUNDED.canonicalize_position(pos),
            Some(pos)
        );
    }

    #[test]
    fn invalid_and_block_overflowing_extents_are_rejected() {
        assert!(AxisTopology::finite(2, 2).validate().is_err());
        assert!(AxisTopology::periodic(0, 0).validate().is_err());
        assert!(AxisTopology::finite(i32::MIN, i32::MAX).validate().is_err());
        assert!(AxisTopology::periodic(i32::MAX, 2).validate().is_err());
    }

    #[test]
    fn periodic_coordinates_use_euclidean_remainder() {
        let axis = AxisTopology::periodic(0, 32);
        for (input, expected) in [
            (-65, 31),
            (-33, 31),
            (-32, 0),
            (-1, 31),
            (0, 0),
            (31, 31),
            (32, 0),
            (65, 1),
        ] {
            assert_eq!(axis.canonical_chunk(input), Some(expected));
        }
        for (input, expected) in [
            (-513, 511),
            (-512, 0),
            (-1, 511),
            (0, 0),
            (511, 511),
            (512, 0),
            (1025, 1),
        ] {
            assert_eq!(axis.canonical_block(input), Some(expected));
        }
    }

    #[test]
    fn canonicalization_is_idempotent_and_steps_back_across_seam() {
        for x in -1024..=1024 {
            let first = CYLINDER.canonicalize_chunk(ChunkPos::new(x, 7)).unwrap();
            assert_eq!(CYLINDER.canonicalize_chunk(first), Some(first));
        }
        let zero = ChunkPos::new(0, 4);
        let west = CYLINDER.neighbor_chunk(zero, -1, 0).unwrap();
        assert_eq!(west, ChunkPos::new(31, 4));
        assert_eq!(CYLINDER.neighbor_chunk(west, 1, 0), Some(zero));
    }

    #[test]
    fn finite_axes_reject_outside_chunks_blocks_and_positions() {
        let topology =
            HorizontalTopology::new(AxisTopology::finite(-2, 3), AxisTopology::finite(4, 6));
        assert_eq!(
            topology.canonicalize_chunk(ChunkPos::new(-2, 4)),
            Some(ChunkPos::new(-2, 4))
        );
        assert_eq!(topology.canonicalize_chunk(ChunkPos::new(3, 4)), None);
        assert_eq!(
            topology.canonicalize_block(BlockPos::new(-33, 64, 64)),
            None
        );
        assert_eq!(
            topology.canonicalize_block(BlockPos::new(-32, 64, 95)),
            Some(BlockPos::new(-32, 64, 95))
        );
        assert_eq!(
            topology.canonicalize_position(Vec3d::new(47.999, 64.0, 95.999)),
            Some(Vec3d::new(47.999, 64.0, 95.999))
        );
        assert_eq!(
            topology.canonicalize_position(Vec3d::new(48.0, 64.0, 96.0)),
            None
        );
    }

    #[test]
    fn shortest_displacement_treats_seam_as_one_step_and_ties_positive() {
        let axis = AxisTopology::periodic(0, 32);
        assert_eq!(axis.shortest_chunk_displacement(31, 0), 1);
        assert_eq!(axis.shortest_chunk_displacement(0, 31), -1);
        assert_eq!(axis.shortest_chunk_displacement(0, 16), 16);
        assert_eq!(axis.shortest_position_displacement(511.75, 0.25), 0.5);
        assert_eq!(axis.shortest_position_displacement(0.25, 511.75), -0.5);
    }

    #[test]
    fn view_enumeration_retains_canonical_identity_and_local_lift() {
        let view = CYLINDER.chunk_view(ChunkPos::new(0, 8), 2).unwrap();
        let center_row = view
            .iter()
            .filter(|entry| entry.lifted.z == 8)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            center_row,
            vec![
                ChunkLift {
                    canonical: ChunkPos::new(30, 8),
                    lifted: LiftedChunkPos::new(-2, 8)
                },
                ChunkLift {
                    canonical: ChunkPos::new(31, 8),
                    lifted: LiftedChunkPos::new(-1, 8)
                },
                ChunkLift {
                    canonical: ChunkPos::new(0, 8),
                    lifted: LiftedChunkPos::new(0, 8)
                },
                ChunkLift {
                    canonical: ChunkPos::new(1, 8),
                    lifted: LiftedChunkPos::new(1, 8)
                },
                ChunkLift {
                    canonical: ChunkPos::new(2, 8),
                    lifted: LiftedChunkPos::new(2, 8)
                },
            ]
        );
        assert_eq!(view.len(), 25);
        assert!(CYLINDER.chunk_view(ChunkPos::new(0, 0), 16).is_err());
    }

    #[test]
    fn nearest_lifts_preserve_continuity_beyond_canonical_period() {
        assert_eq!(
            CYLINDER.nearest_chunk_lift(ChunkPos::new(0, 3), Vec3d::new(511.9, 64.0, 48.0)),
            LiftedChunkPos::new(32, 3)
        );
        assert_eq!(
            CYLINDER
                .nearest_position_lift(Vec3d::new(0.1, 64.0, 2.0), Vec3d::new(511.9, 64.0, 2.0)),
            Vec3d::new(512.1, 64.0, 2.0)
        );
    }
}
