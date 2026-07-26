use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{
    AxisTopology, BlockPos, CHUNK_WIDTH, ChunkPos, HorizontalTopology, chunk_min_block_coord,
    expected_chunk_biome_count,
};

use crate::block::{
    BEDROCK, BLUE_TERRACOTTA, DIRT, GRASS_BLOCK, GRAVEL, GREEN_TERRACOTTA, RED_TERRACOTTA, STONE,
    STONE_BRICKS, TORCH, WATER, YELLOW_TERRACOTTA,
};

use super::{GeneratedChunk, MutableChunkBlockBuffer};

pub const TOPOLOGY_PROBE_MIN_Y: i32 = 0;
pub const TOPOLOGY_PROBE_HEIGHT: i32 = 256;
pub const TOPOLOGY_PROBE_SEA_LEVEL: i32 = 63;
pub const TOPOLOGY_PROBE_BASE_SURFACE_Y: i32 = 66;
pub const TOPOLOGY_PROBE_MIN_PERIOD_CHUNKS: u32 = 8;
pub const TOPOLOGY_PROBE_PLAINS_BIOME_ID: i32 = 1;

const TOPOLOGY_PROBE_RIDGE_CENTER_Z: i32 = -40;
const TOPOLOGY_PROBE_POND_CENTER_Z: i32 = 32;
const TOPOLOGY_PROBE_ARCH_CENTER_Z: i32 = -24;
const TOPOLOGY_PROBE_MATERIAL_BAND_Z: i32 = 52;
const TOPOLOGY_PROBE_ARCH_CENTER_Y: i32 = 71;
const TOPOLOGY_PROBE_ARCH_OUTER_RADIUS: i64 = 8;
const TOPOLOGY_PROBE_ARCH_INNER_RADIUS: i64 = 5;
const TOPOLOGY_PROBE_PLAN_DOMAIN: u64 = 0x746f_706f_2d70_726f;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyProbeColumnSample {
    pub surface_y: i32,
    pub water_surface_y: Option<i32>,
    pub ridge_lift: i32,
    pub is_channel: bool,
    pub is_island: bool,
    pub is_pond: bool,
    pub top_block: u8,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TopologyProbePlanKind {
    SeamArch,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TopologyProbePlan {
    pub id: u64,
    pub kind: TopologyProbePlanKind,
    pub owner: ChunkPos,
    pub anchor: BlockPos,
    pub influence_radius_blocks: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyProbeWorkPlan {
    pub canonical: TopologyProbePlan,
    pub work_anchor_x: i64,
    pub work_anchor_z: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyProbeSource {
    seed: i64,
    topology: HorizontalTopology,
    origin_x: i32,
    origin_z: i32,
}

impl TopologyProbeSource {
    pub fn new(seed: i64, topology: HorizontalTopology) -> Result<Self, String> {
        validate_topology_probe_topology(topology)?;
        Ok(Self {
            seed,
            topology,
            origin_x: topology_axis_origin_block(topology.x),
            origin_z: topology_axis_origin_block(topology.z),
        })
    }

    pub const fn seed(self) -> i64 {
        self.seed
    }

    pub const fn topology(self) -> HorizontalTopology {
        self.topology
    }

    pub fn sample_column(self, world_x: i32, world_z: i32) -> TopologyProbeColumnSample {
        let dx = self
            .topology
            .x
            .shortest_block_displacement(self.origin_x, world_x);
        let dz = self
            .topology
            .z
            .shortest_block_displacement(self.origin_z, world_z);
        let ridge_lift = topology_probe_ridge_lift(dx, dz);
        let is_channel = topology_probe_channel_contains(dx, dz);
        let is_island = dx.abs() <= 10 && dz.abs() <= 2 && !is_channel;
        let pond_dz = dz - i64::from(TOPOLOGY_PROBE_POND_CENTER_Z);
        let is_pond =
            dx.abs() <= 10 && pond_dz.abs() <= 10 && dx * dx + pond_dz * pond_dz <= 10 * 10;
        let water_surface_y = (is_channel || is_pond).then_some(TOPOLOGY_PROBE_SEA_LEVEL);
        let surface_y = if is_channel {
            TOPOLOGY_PROBE_SEA_LEVEL - 3
        } else if is_pond {
            TOPOLOGY_PROBE_SEA_LEVEL - 2
        } else {
            TOPOLOGY_PROBE_BASE_SURFACE_Y + ridge_lift
        };
        let top_block = if is_channel {
            GRAVEL
        } else if is_pond {
            GRAVEL
        } else if dz == i64::from(TOPOLOGY_PROBE_MATERIAL_BAND_Z) {
            self.material_band_block(world_x)
        } else {
            GRASS_BLOCK
        };

        TopologyProbeColumnSample {
            surface_y,
            water_surface_y,
            ridge_lift,
            is_channel,
            is_island,
            is_pond,
            top_block,
        }
    }

    pub fn arch_plan(self) -> TopologyProbePlan {
        let anchor = self
            .topology
            .canonicalize_block(BlockPos::new(
                self.origin_x,
                TOPOLOGY_PROBE_ARCH_CENTER_Y,
                self.origin_z + TOPOLOGY_PROBE_ARCH_CENTER_Z,
            ))
            .expect("supported probe topology must contain its canonical arch anchor");
        TopologyProbePlan {
            id: topology_probe_plan_id(self.seed, anchor),
            kind: TopologyProbePlanKind::SeamArch,
            owner: anchor.chunk_pos(),
            anchor,
            influence_radius_blocks: TOPOLOGY_PROBE_ARCH_OUTER_RADIUS as u32,
        }
    }

    pub fn arch_plan_near(self, query: BlockPos) -> TopologyProbeWorkPlan {
        let canonical = self.arch_plan();
        TopologyProbeWorkPlan {
            canonical,
            work_anchor_x: self
                .topology
                .x
                .nearest_position_lift(f64::from(canonical.anchor.x), f64::from(query.x))
                .round() as i64,
            work_anchor_z: self
                .topology
                .z
                .nearest_position_lift(f64::from(canonical.anchor.z), f64::from(query.z))
                .round() as i64,
        }
    }

    pub fn generate_chunk(self, requested: ChunkPos) -> GeneratedChunk {
        let canonical = self
            .topology
            .canonicalize_chunk(requested)
            .expect("supported probe topology has no finite excluded chunks");
        let mut buffer = MutableChunkBlockBuffer::new(
            canonical.x,
            canonical.z,
            TOPOLOGY_PROBE_MIN_Y,
            TOPOLOGY_PROBE_HEIGHT,
        );
        let min_x = chunk_min_block_coord(canonical.x);
        let min_z = chunk_min_block_coord(canonical.z);

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let world_x = min_x + local_x;
                let world_z = min_z + local_z;
                let sample = self.sample_column(world_x, world_z);
                buffer.set_block_at_y(local_x, 0, local_z, BEDROCK);
                let stone_top = (sample.surface_y - 3).max(1);
                for y in 1..stone_top {
                    buffer.set_block_at_y(local_x, y, local_z, STONE);
                }
                for y in stone_top..sample.surface_y {
                    buffer.set_block_at_y(local_x, y, local_z, DIRT);
                }
                buffer.set_block_at_y(local_x, sample.surface_y, local_z, sample.top_block);
                if let Some(water_y) = sample.water_surface_y {
                    for y in sample.surface_y + 1..=water_y {
                        buffer.set_block_at_y(local_x, y, local_z, WATER);
                    }
                }

                for y in TOPOLOGY_PROBE_BASE_SURFACE_Y..=TOPOLOGY_PROBE_ARCH_CENTER_Y + 9 {
                    if self.arch_contains(world_x, y, world_z) {
                        buffer.set_block_at_y(local_x, y, local_z, STONE_BRICKS);
                    }
                }
                if self.torch_site(world_x, world_z) {
                    buffer.set_block_at_y(
                        local_x,
                        TOPOLOGY_PROBE_ARCH_CENTER_Y + 9,
                        local_z,
                        TORCH,
                    );
                }
            }
        }

        buffer.prime_worldgen_heightmaps();
        GeneratedChunk::from_mutable_buffer_with_biomes(
            buffer,
            vec![TOPOLOGY_PROBE_PLAINS_BIOME_ID; expected_chunk_biome_count(TOPOLOGY_PROBE_HEIGHT)],
        )
    }

    pub fn generate_chunks(
        self,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> BTreeMap<ChunkPos, GeneratedChunk> {
        targets
            .into_iter()
            .map(|target| {
                self.topology
                    .canonicalize_chunk(target)
                    .expect("supported probe topology has no finite excluded chunks")
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|canonical| (canonical, self.generate_chunk(canonical)))
            .collect()
    }

    fn material_band_block(self, world_x: i32) -> u8 {
        let canonical_x = self
            .topology
            .x
            .canonical_block(world_x)
            .expect("supported probe topology has no finite excluded blocks");
        let phase = (stable_mix64(self.seed as u64) & 3) as i32;
        match ((canonical_x - self.origin_x).div_euclid(4) + phase).rem_euclid(4) {
            0 => RED_TERRACOTTA,
            1 => YELLOW_TERRACOTTA,
            2 => GREEN_TERRACOTTA,
            _ => BLUE_TERRACOTTA,
        }
    }

    fn arch_contains(self, world_x: i32, y: i32, world_z: i32) -> bool {
        let dx = self
            .topology
            .x
            .shortest_block_displacement(self.origin_x, world_x);
        let dz = self
            .topology
            .z
            .shortest_block_displacement(self.origin_z + TOPOLOGY_PROBE_ARCH_CENTER_Z, world_z);
        let dy = i64::from(y - TOPOLOGY_PROBE_ARCH_CENTER_Y);
        if dx.abs() > TOPOLOGY_PROBE_ARCH_OUTER_RADIUS
            || dy.abs() > TOPOLOGY_PROBE_ARCH_OUTER_RADIUS
        {
            return false;
        }
        let radius_squared = dx * dx + dy * dy;
        dz.abs() <= 1
            && y >= TOPOLOGY_PROBE_BASE_SURFACE_Y
            && radius_squared <= TOPOLOGY_PROBE_ARCH_OUTER_RADIUS.pow(2)
            && radius_squared >= TOPOLOGY_PROBE_ARCH_INNER_RADIUS.pow(2)
    }

    fn torch_site(self, world_x: i32, world_z: i32) -> bool {
        self.topology
            .x
            .shortest_block_displacement(self.origin_x, world_x)
            == 0
            && self
                .topology
                .z
                .shortest_block_displacement(self.origin_z + TOPOLOGY_PROBE_ARCH_CENTER_Z, world_z)
                == 0
    }
}

pub fn validate_topology_probe_topology(topology: HorizontalTopology) -> Result<(), String> {
    topology
        .validate()
        .map_err(|error| format!("invalid topology-probe-v1 topology: {error}"))?;
    match (topology.x, topology.z) {
        (AxisTopology::Unbounded, AxisTopology::Unbounded) => Ok(()),
        (AxisTopology::Periodic { period_chunks, .. }, AxisTopology::Unbounded) => {
            validate_probe_period("X", period_chunks)
        }
        (
            AxisTopology::Periodic {
                period_chunks: period_x,
                ..
            },
            AxisTopology::Periodic {
                period_chunks: period_z,
                ..
            },
        ) => {
            validate_probe_period("X", period_x)?;
            validate_probe_period("Z", period_z)
        }
        _ => Err(
            "topology-probe-v1 supports only the unbounded plane, an X-periodic cylinder, or a flat torus"
                .to_owned(),
        ),
    }
}

pub fn generate_topology_probe_chunk(
    seed: i64,
    topology: HorizontalTopology,
    chunk_x: i32,
    chunk_z: i32,
) -> Result<GeneratedChunk, String> {
    Ok(TopologyProbeSource::new(seed, topology)?.generate_chunk(ChunkPos::new(chunk_x, chunk_z)))
}

pub fn generate_topology_probe_chunks(
    seed: i64,
    topology: HorizontalTopology,
    targets: impl IntoIterator<Item = ChunkPos>,
) -> Result<BTreeMap<ChunkPos, GeneratedChunk>, String> {
    Ok(TopologyProbeSource::new(seed, topology)?.generate_chunks(targets))
}

fn validate_probe_period(axis: &str, period_chunks: u32) -> Result<(), String> {
    if period_chunks < TOPOLOGY_PROBE_MIN_PERIOD_CHUNKS {
        Err(format!(
            "topology-probe-v1 {axis} period must be at least {TOPOLOGY_PROBE_MIN_PERIOD_CHUNKS} chunks, got {period_chunks}"
        ))
    } else {
        Ok(())
    }
}

fn topology_axis_origin_block(axis: AxisTopology) -> i32 {
    match axis {
        AxisTopology::Periodic { minimum_chunk, .. }
        | AxisTopology::Finite { minimum_chunk, .. } => minimum_chunk * CHUNK_WIDTH,
        AxisTopology::Unbounded => 0,
    }
}

fn topology_probe_ridge_lift(dx: i64, dz: i64) -> i32 {
    let dz = dz - i64::from(TOPOLOGY_PROBE_RIDGE_CENTER_Z);
    let radius_x = 24_i64;
    let radius_z = 16_i64;
    if dx.abs() >= radius_x || dz.abs() >= radius_z {
        return 0;
    }
    let x_weight = radius_x * radius_x - dx * dx;
    let z_weight = radius_z * radius_z - dz * dz;
    (12 * x_weight * z_weight / (radius_x * radius_x * radius_z * radius_z)) as i32
}

fn topology_probe_channel_contains(dx: i64, dz: i64) -> bool {
    let distance = dx.abs();
    if distance <= 12 {
        (dz - 5).abs() <= 2 || (dz + 5).abs() <= 2
    } else if distance < 20 {
        let offset = (5 * (20 - distance) + 4) / 8;
        (dz - offset).abs() <= 2 || (dz + offset).abs() <= 2
    } else {
        dz.abs() <= 3
    }
}

fn topology_probe_plan_id(seed: i64, anchor: BlockPos) -> u64 {
    let mut state = stable_mix64((seed as u64) ^ TOPOLOGY_PROBE_PLAN_DOMAIN);
    state = stable_mix64(state ^ (anchor.x as u32 as u64));
    stable_mix64(state ^ (anchor.z as u32 as u64))
}

fn stable_mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, STONE_BRICKS, TORCH, WATER};

    const PERIOD_CHUNKS: u32 = 8;
    const PERIOD_BLOCKS: i32 = PERIOD_CHUNKS as i32 * CHUNK_WIDTH;

    fn cylinder() -> HorizontalTopology {
        HorizontalTopology::cylinder_x(0, PERIOD_CHUNKS)
    }

    fn torus() -> HorizontalTopology {
        HorizontalTopology::new(
            AxisTopology::periodic(0, PERIOD_CHUNKS),
            AxisTopology::periodic(0, PERIOD_CHUNKS),
        )
    }

    #[test]
    fn support_is_explicit_and_periods_cover_the_complete_fixture() {
        assert!(TopologyProbeSource::new(1, HorizontalTopology::UNBOUNDED).is_ok());
        assert!(TopologyProbeSource::new(1, cylinder()).is_ok());
        assert!(TopologyProbeSource::new(1, torus()).is_ok());
        assert!(TopologyProbeSource::new(1, HorizontalTopology::cylinder_x(0, 7)).is_err());
        assert!(
            TopologyProbeSource::new(
                1,
                HorizontalTopology::new(AxisTopology::finite(0, 8), AxisTopology::Unbounded,),
            )
            .is_err()
        );
        assert!(
            TopologyProbeSource::new(
                1,
                HorizontalTopology::new(AxisTopology::Unbounded, AxisTopology::periodic(0, 8),),
            )
            .is_err()
        );
    }

    #[test]
    fn cylinder_samples_and_seam_slopes_repeat_across_signed_lifts() {
        let source = TopologyProbeSource::new(-98_765, cylinder()).unwrap();
        for (x, z) in [(-129, -40), (-1, 0), (0, 0), (127, 32), (159, 52)] {
            assert_eq!(
                source.sample_column(x, z),
                source.sample_column(x + PERIOD_BLOCKS, z)
            );
        }
        let before = source.sample_column(-1, -40).surface_y;
        let seam = source.sample_column(0, -40).surface_y;
        let repeated_before = source.sample_column(PERIOD_BLOCKS - 1, -40).surface_y;
        let repeated_seam = source.sample_column(PERIOD_BLOCKS, -40).surface_y;
        assert_eq!(seam - before, repeated_seam - repeated_before);
    }

    #[test]
    fn torus_samples_repeat_at_both_axes_and_the_corner() {
        let source = TopologyProbeSource::new(12_345, torus()).unwrap();
        for (x, z) in [(-1, -1), (0, 0), (127, 127), (32, 52)] {
            let expected = source.sample_column(x, z);
            assert_eq!(source.sample_column(x + PERIOD_BLOCKS, z), expected);
            assert_eq!(source.sample_column(x, z + PERIOD_BLOCKS), expected);
            assert_eq!(
                source.sample_column(x + PERIOD_BLOCKS, z + PERIOD_BLOCKS),
                expected
            );
        }
    }

    #[test]
    fn fixture_guarantees_ridge_channel_island_pond_and_arch() {
        let source = TopologyProbeSource::new(12_345, cylinder()).unwrap();
        assert!(source.sample_column(0, -40).ridge_lift >= 10);
        assert!(source.sample_column(24, 0).is_channel);
        assert!(source.sample_column(0, 5).is_channel);
        assert!(source.sample_column(0, 0).is_island);
        assert!(source.sample_column(0, 32).is_pond);

        let chunk_zero = source.generate_chunk(ChunkPos::new(0, -2));
        let chunk_last = source.generate_chunk(ChunkPos::new(-1, -2));
        assert_eq!(chunk_last.chunk_x, PERIOD_CHUNKS as i32 - 1);
        assert_eq!(
            chunk_zero
                .block_at_y(0, TOPOLOGY_PROBE_ARCH_CENTER_Y + 8, 8)
                .0,
            STONE_BRICKS
        );
        assert_eq!(
            chunk_zero
                .block_at_y(0, TOPOLOGY_PROBE_ARCH_CENTER_Y + 9, 8)
                .0,
            TORCH
        );
        assert_eq!(
            chunk_zero.block_at_y(0, TOPOLOGY_PROBE_ARCH_CENTER_Y, 8).0,
            AIR
        );
        assert_eq!(
            chunk_last.block_at_y(15, TOPOLOGY_PROBE_ARCH_CENTER_Y, 8).0,
            AIR
        );
    }

    #[test]
    fn arch_keeps_one_canonical_identity_and_chooses_nearby_work_lifts() {
        let source = TopologyProbeSource::new(-98_765, cylinder()).unwrap();
        let canonical = source.arch_plan();
        let negative = source.arch_plan_near(BlockPos::new(-1, 70, -24));
        let positive = source.arch_plan_near(BlockPos::new(PERIOD_BLOCKS, 70, -24));

        assert_eq!(negative.canonical, canonical);
        assert_eq!(positive.canonical, canonical);
        assert_eq!(negative.work_anchor_x, 0);
        assert_eq!(positive.work_anchor_x, i64::from(PERIOD_BLOCKS));
        assert_eq!(canonical.owner, ChunkPos::new(0, -2));
    }

    #[test]
    fn lifted_generation_and_batch_partitioning_have_one_canonical_output() {
        let source = TopologyProbeSource::new(8_675_309, cylinder()).unwrap();
        assert_eq!(
            source.generate_chunk(ChunkPos::new(-1, 0)),
            source.generate_chunk(ChunkPos::new(PERIOD_CHUNKS as i32 - 1, 0))
        );
        assert_eq!(
            source.generate_chunk(ChunkPos::new(PERIOD_CHUNKS as i32, 0)),
            source.generate_chunk(ChunkPos::new(0, 0))
        );

        let targets = [
            ChunkPos::new(PERIOD_CHUNKS as i32 - 1, 0),
            ChunkPos::new(0, 0),
        ];
        let combined = source.generate_chunks(targets);
        let reversed = source.generate_chunks([targets[1], targets[0]]);
        let partitioned = targets
            .into_iter()
            .flat_map(|target| source.generate_chunks([target]))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(combined, reversed);
        assert_eq!(combined, partitioned);
        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn generated_channel_and_pond_are_flat_source_water() {
        let source = TopologyProbeSource::new(12_345, cylinder()).unwrap();
        let chunk = source.generate_chunk(ChunkPos::new(0, 0));
        assert_eq!(chunk.block_at_y(0, TOPOLOGY_PROBE_SEA_LEVEL, 5).0, WATER);
        assert_eq!(chunk.block_at_y(0, TOPOLOGY_PROBE_SEA_LEVEL + 1, 5).0, AIR);
        assert!(chunk.liquid_ticks().is_empty());

        let pond = source.generate_chunk(ChunkPos::new(0, 2));
        assert_eq!(pond.block_at_y(0, TOPOLOGY_PROBE_SEA_LEVEL, 0).0, WATER);
        assert!(pond.liquid_ticks().is_empty());
    }
}
