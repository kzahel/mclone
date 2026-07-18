use mclone_core::ChunkPos;

use crate::noise::{SeedDomain, ValueNoise2d};

pub const MCLONE_OVERWORLD_SEA_LEVEL: i32 = 63;
pub const MCLONE_OVERWORLD_FIELD_REVISION: &str = "mclone-overworld-v1-fields-2";

const CONTINENT_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e31);
const CONTINENT_MEDIUM_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e32);
const CONTINENT_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e33);
const RELIEF_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c31);
const RELIEF_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c32);
const RELIEF_FINE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c33);

const CONTINENT_LARGE_SCALE: i32 = 2_048;
const CONTINENT_MEDIUM_SCALE: i32 = 1_024;
const CONTINENT_DETAIL_SCALE: i32 = 512;
const RELIEF_LARGE_SCALE: i32 = 384;
const RELIEF_DETAIL_SCALE: i32 = 128;
const RELIEF_FINE_SCALE: i32 = 48;
const MAX_REGION_SAMPLE_COUNT: usize = 16 * 1024 * 1024;
const SPAWN_SEARCH_RADIUS_CHUNKS: i32 = 128;
const SPAWN_MIN_SURFACE_Y: i32 = MCLONE_OVERWORLD_SEA_LEVEL + 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldTerrainSample {
    pub continentalness: f64,
    pub relief: f64,
    pub surface_y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldSampleRegionRequest {
    pub min_x: i32,
    pub min_z: i32,
    pub width: u32,
    pub depth: u32,
    pub step: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneOverworldSampleRegion {
    pub request: McloneOverworldSampleRegionRequest,
    pub samples: Vec<McloneOverworldTerrainSample>,
}

impl McloneOverworldSampleRegion {
    pub fn sample(&self, offset_x: u32, offset_z: u32) -> Option<McloneOverworldTerrainSample> {
        if offset_x >= self.request.width || offset_z >= self.request.depth {
            return None;
        }
        let index = usize::try_from(offset_z).ok()? * usize::try_from(self.request.width).ok()?
            + usize::try_from(offset_x).ok()?;
        self.samples.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldSampler {
    continent_large: ValueNoise2d,
    continent_medium: ValueNoise2d,
    continent_detail: ValueNoise2d,
    relief_large: ValueNoise2d,
    relief_detail: ValueNoise2d,
    relief_fine: ValueNoise2d,
}

impl McloneOverworldSampler {
    pub fn new(seed: i64) -> Self {
        Self {
            continent_large: ValueNoise2d::new(seed, CONTINENT_LARGE_DOMAIN, CONTINENT_LARGE_SCALE),
            continent_medium: ValueNoise2d::new(
                seed,
                CONTINENT_MEDIUM_DOMAIN,
                CONTINENT_MEDIUM_SCALE,
            ),
            continent_detail: ValueNoise2d::new(
                seed,
                CONTINENT_DETAIL_DOMAIN,
                CONTINENT_DETAIL_SCALE,
            ),
            relief_large: ValueNoise2d::new(seed, RELIEF_LARGE_DOMAIN, RELIEF_LARGE_SCALE),
            relief_detail: ValueNoise2d::new(seed, RELIEF_DETAIL_DOMAIN, RELIEF_DETAIL_SCALE),
            relief_fine: ValueNoise2d::new(seed, RELIEF_FINE_DOMAIN, RELIEF_FINE_SCALE),
        }
    }

    pub fn sample(self, world_x: i32, world_z: i32) -> McloneOverworldTerrainSample {
        let continentalness = (self.continent_large.sample(world_x, world_z) * 0.55
            + self.continent_medium.sample(world_x, world_z) * 0.30
            + self.continent_detail.sample(world_x, world_z) * 0.15)
            .clamp(-1.0, 1.0);
        let relief = (self.relief_large.sample(world_x, world_z) * 0.50
            + self.relief_detail.sample(world_x, world_z) * 0.30
            + self.relief_fine.sample(world_x, world_z) * 0.20)
            .clamp(-1.0, 1.0);
        McloneOverworldTerrainSample {
            continentalness,
            relief,
            surface_y: surface_height(continentalness, relief),
        }
    }

    pub fn sample_region(
        self,
        request: McloneOverworldSampleRegionRequest,
    ) -> Result<McloneOverworldSampleRegion, String> {
        let width = usize::try_from(request.width)
            .map_err(|_| "mclone overworld sample width does not fit usize")?;
        let depth = usize::try_from(request.depth)
            .map_err(|_| "mclone overworld sample depth does not fit usize")?;
        let sample_count = width
            .checked_mul(depth)
            .ok_or("mclone overworld sample region area overflow")?;
        if sample_count > MAX_REGION_SAMPLE_COUNT {
            return Err(format!(
                "mclone overworld sample region has {sample_count} points, maximum is {MAX_REGION_SAMPLE_COUNT}"
            ));
        }
        let step = i32::try_from(request.step)
            .map_err(|_| "mclone overworld sample region step exceeds i32 coordinates")?;
        if step == 0 {
            return Err("mclone overworld sample region step must be at least 1".to_owned());
        }
        if let Some(max_offset_x) = request.width.checked_sub(1) {
            let max_offset_x = i32::try_from(max_offset_x)
                .map_err(|_| "mclone overworld sample region width exceeds i32 coordinates")?
                .checked_mul(step)
                .ok_or("mclone overworld sample region x step overflow")?;
            request
                .min_x
                .checked_add(max_offset_x)
                .ok_or("mclone overworld sample region x coordinate overflow")?;
        }
        if let Some(max_offset_z) = request.depth.checked_sub(1) {
            let max_offset_z = i32::try_from(max_offset_z)
                .map_err(|_| "mclone overworld sample region depth exceeds i32 coordinates")?
                .checked_mul(step)
                .ok_or("mclone overworld sample region z step overflow")?;
            request
                .min_z
                .checked_add(max_offset_z)
                .ok_or("mclone overworld sample region z coordinate overflow")?;
        }

        let mut samples = Vec::with_capacity(sample_count);
        for offset_z in 0..request.depth {
            for offset_x in 0..request.width {
                samples.push(
                    self.sample(
                        request.min_x
                            + i32::try_from(offset_x)
                                .expect("validated mclone overworld sample x offset")
                                * step,
                        request.min_z
                            + i32::try_from(offset_z)
                                .expect("validated mclone overworld sample z offset")
                                * step,
                    ),
                );
            }
        }
        Ok(McloneOverworldSampleRegion { request, samples })
    }
}

pub fn mclone_overworld_spawn_chunk(seed: i64) -> ChunkPos {
    let sampler = McloneOverworldSampler::new(seed);
    for radius in 0..=SPAWN_SEARCH_RADIUS_CHUNKS {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                let world_x = x * 16 + 8;
                let world_z = z * 16 + 8;
                if sampler.sample(world_x, world_z).surface_y >= SPAWN_MIN_SURFACE_Y {
                    return ChunkPos::new(x, z);
                }
            }
        }
    }
    ChunkPos::new(0, 0)
}

fn surface_height(continentalness: f64, relief: f64) -> i32 {
    if continentalness <= 0.0 {
        let shallow_water = smoothstep(((continentalness + 0.55) / 0.55).clamp(0.0, 1.0));
        let floor = 46.0 + shallow_water * 15.0 + relief * 1.5;
        return floor.round().clamp(42.0, 61.0) as i32;
    }

    let land_strength = smoothstep((continentalness / 0.45).clamp(0.0, 1.0));
    let base = 64.0 + land_strength * 18.0;
    let rolling_relief = relief * (2.0 + land_strength * 7.0);
    (base + rolling_relief).round().clamp(62.0, 96.0) as i32
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_region_is_row_major_and_uses_the_point_sampler() {
        let sampler = McloneOverworldSampler::new(-98_765);
        let request = McloneOverworldSampleRegionRequest {
            min_x: -19,
            min_z: 27,
            width: 5,
            depth: 3,
            step: 7,
        };
        let region = sampler.sample_region(request).unwrap();

        assert_eq!(region.samples.len(), 15);
        for offset_z in 0..request.depth {
            for offset_x in 0..request.width {
                assert_eq!(
                    region.sample(offset_x, offset_z),
                    Some(sampler.sample(
                        request.min_x + offset_x as i32 * request.step as i32,
                        request.min_z + offset_z as i32 * request.step as i32
                    ))
                );
            }
        }
        assert_eq!(region.sample(request.width, 0), None);
    }

    #[test]
    fn bounded_region_rejects_unbounded_or_wrapping_requests() {
        let sampler = McloneOverworldSampler::new(1);
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: 0,
                    min_z: 0,
                    width: 4_097,
                    depth: 4_097,
                    step: 1,
                })
                .is_err()
        );
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: i32::MAX,
                    min_z: 0,
                    width: 2,
                    depth: 1,
                    step: 1,
                })
                .is_err()
        );
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: 0,
                    min_z: 0,
                    width: 1,
                    depth: 1,
                    step: 0,
                })
                .is_err()
        );
    }

    #[test]
    fn selected_seeds_find_a_dry_spawn_chunk() {
        for seed in [12_345, -98_765, 8_675_309] {
            let spawn = mclone_overworld_spawn_chunk(seed);
            let sample = McloneOverworldSampler::new(seed)
                .sample(spawn.min_block_x() + 8, spawn.min_block_z() + 8);
            assert!(sample.surface_y >= SPAWN_MIN_SURFACE_Y, "{seed}: {spawn:?}");
        }
    }
}
