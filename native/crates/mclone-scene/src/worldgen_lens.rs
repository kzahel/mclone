use std::collections::BTreeSet;

use glam::{Vec3, vec3};
use mclone_core::{ChunkPos, HorizontalTopology, block_to_chunk_coord};
use mclone_render::world_color_mesh::{WorldColorMesh, WorldColorVertex};
use mclone_server::WorldGenerationProfile;
use mclone_worldgen::levelgen::{
    McloneOverworldBiomeRecipe, McloneOverworldDebugSample, McloneOverworldHydrologyKind,
    McloneOverworldLandformKind, McloneOverworldSampler, McloneOverworldSamplingTopology,
    McloneOverworldSurfaceRecipe, McloneOverworldTerrainSample, mclone_overworld_debug_sample,
};

use crate::McloneSceneHost;

const CELL_SIZE: i32 = 4;
const REGION_RADIUS_BLOCKS: i32 = 160;
const CACHE_ALIGNMENT_BLOCKS: i32 = 16;
const OVERLAY_HEIGHT_BIAS: f32 = 1.035;
const COLOR_ALPHA: f32 = 0.48;
const HYDROLOGY_DRY_ALPHA: f32 = 0.04;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorldgenLensLayer {
    #[default]
    Biome,
    Landform,
    Surface,
    Hydrology,
}

impl WorldgenLensLayer {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Biome => "BIOME",
            Self::Landform => "LANDFORM",
            Self::Surface => "SURFACE",
            Self::Hydrology => "HYDROLOGY",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Biome => Self::Landform,
            Self::Landform => Self::Surface,
            Self::Surface => Self::Hydrology,
            Self::Hydrology => Self::Biome,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorldgenLensCacheKey {
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    layer: WorldgenLensLayer,
    center_x: i32,
    center_z: i32,
    loaded_chunk_count: usize,
    loaded_chunk_signature: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct WorldgenLensCache {
    key: WorldgenLensCacheKey,
    mesh: WorldColorMesh,
}

#[derive(Debug, Default)]
pub(crate) struct WorldgenLensState {
    enabled: bool,
    layer: WorldgenLensLayer,
    cache: Option<WorldgenLensCache>,
}

impl WorldgenLensState {
    pub(crate) fn toggle(&mut self) -> Option<WorldgenLensLayer> {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.cache = None;
            None
        } else {
            Some(self.layer)
        }
    }

    pub(crate) fn cycle(&mut self) -> WorldgenLensLayer {
        if self.enabled {
            self.layer = self.layer.next();
        } else {
            self.enabled = true;
        }
        self.cache = None;
        self.layer
    }

    pub(crate) fn active_layer(&self) -> Option<WorldgenLensLayer> {
        self.enabled.then_some(self.layer)
    }

    pub(crate) fn prepare_mesh(
        &mut self,
        seed: i64,
        profile: WorldGenerationProfile,
        topology: HorizontalTopology,
        camera_position: Vec3,
        loaded_chunks: &BTreeSet<ChunkPos>,
    ) -> Option<&WorldColorMesh> {
        if !self.enabled || profile != WorldGenerationProfile::McloneOverworldV1 {
            self.cache = None;
            return None;
        }
        let topology = McloneOverworldSamplingTopology::from_horizontal_topology(topology).ok()?;
        let camera_x = camera_position.x.floor() as i32;
        let camera_z = camera_position.z.floor() as i32;
        let key = WorldgenLensCacheKey {
            seed,
            topology,
            layer: self.layer,
            center_x: camera_x.div_euclid(CACHE_ALIGNMENT_BLOCKS) * CACHE_ALIGNMENT_BLOCKS,
            center_z: camera_z.div_euclid(CACHE_ALIGNMENT_BLOCKS) * CACHE_ALIGNMENT_BLOCKS,
            loaded_chunk_count: loaded_chunks.len(),
            loaded_chunk_signature: loaded_chunk_signature(loaded_chunks),
        };
        if self.cache.as_ref().is_none_or(|cache| cache.key != key) {
            self.cache = Some(WorldgenLensCache {
                key,
                mesh: build_overlay_mesh(key, loaded_chunks),
            });
        }
        self.cache.as_ref().map(|cache| &cache.mesh)
    }

    pub(crate) fn inspection_lines(
        &self,
        seed: i64,
        profile: WorldGenerationProfile,
        topology: HorizontalTopology,
        position: Vec3,
    ) -> Vec<String> {
        let Some(layer) = self.active_layer() else {
            return Vec::new();
        };
        let Some(topology) =
            McloneOverworldSamplingTopology::from_horizontal_topology(topology).ok()
        else {
            return vec![
                format!("WORLDGEN LENS {} [F3 OFF F4 NEXT]", layer.label()),
                "LENS UNAVAILABLE FOR THIS TOPOLOGY".to_owned(),
            ];
        };
        if profile != WorldGenerationProfile::McloneOverworldV1 {
            return vec![
                format!("WORLDGEN LENS {} [F3 OFF F4 NEXT]", layer.label()),
                format!(
                    "LENS UNAVAILABLE FOR {}",
                    profile.label().to_ascii_uppercase()
                ),
            ];
        }
        let world_x = position.x.floor() as i32;
        let world_z = position.z.floor() as i32;
        let sample = mclone_overworld_debug_sample(
            McloneOverworldSampler::new_with_topology(seed, topology),
            world_x,
            world_z,
        );
        debug_lines(layer, sample)
    }
}

impl McloneSceneHost {
    pub fn toggle_worldgen_lens(&mut self) -> Option<WorldgenLensLayer> {
        self.worldgen_lens.toggle()
    }

    pub fn cycle_worldgen_lens(&mut self) -> WorldgenLensLayer {
        self.worldgen_lens.cycle()
    }

    pub fn worldgen_lens_layer(&self) -> Option<WorldgenLensLayer> {
        self.worldgen_lens.active_layer()
    }
}

fn build_overlay_mesh(
    key: WorldgenLensCacheKey,
    loaded_chunks: &BTreeSet<ChunkPos>,
) -> WorldColorMesh {
    let sampler = McloneOverworldSampler::new_with_topology(key.seed, key.topology);
    let min_x = key.center_x - REGION_RADIUS_BLOCKS;
    let min_z = key.center_z - REGION_RADIUS_BLOCKS;
    let cell_count = (REGION_RADIUS_BLOCKS * 2 / CELL_SIZE) as usize;
    let row_width = cell_count + 1;
    let mut corner_samples = Vec::with_capacity(row_width * row_width);
    for z in 0..=cell_count {
        for x in 0..=cell_count {
            corner_samples
                .push(sampler.sample(min_x + x as i32 * CELL_SIZE, min_z + z as i32 * CELL_SIZE));
        }
    }
    let mut vertices = Vec::with_capacity(cell_count * cell_count * 6);
    for z in 0..cell_count {
        for x in 0..cell_count {
            let world_x = min_x + x as i32 * CELL_SIZE;
            let world_z = min_z + z as i32 * CELL_SIZE;
            let offset_x = world_x + CELL_SIZE / 2 - key.center_x;
            let offset_z = world_z + CELL_SIZE / 2 - key.center_z;
            if offset_x * offset_x + offset_z * offset_z
                > REGION_RADIUS_BLOCKS * REGION_RADIUS_BLOCKS
            {
                continue;
            }
            if !loaded_chunks.contains(&ChunkPos::new(
                block_to_chunk_coord(world_x + 2),
                block_to_chunk_coord(world_z + 2),
            )) {
                continue;
            }
            let semantic = mclone_overworld_debug_sample(sampler, world_x + 2, world_z + 2);
            let color = lens_color(key.layer, semantic);
            let north_west = overlay_position(world_x, world_z, corner_samples[z * row_width + x]);
            let north_east = overlay_position(
                world_x + CELL_SIZE,
                world_z,
                corner_samples[z * row_width + x + 1],
            );
            let south_west = overlay_position(
                world_x,
                world_z + CELL_SIZE,
                corner_samples[(z + 1) * row_width + x],
            );
            let south_east = overlay_position(
                world_x + CELL_SIZE,
                world_z + CELL_SIZE,
                corner_samples[(z + 1) * row_width + x + 1],
            );
            push_triangle(&mut vertices, north_west, south_east, north_east, color);
            push_triangle(&mut vertices, north_west, south_west, south_east, color);
        }
    }
    WorldColorMesh::new(vertices)
}

fn loaded_chunk_signature(chunks: &BTreeSet<ChunkPos>) -> u64 {
    chunks.iter().fold(0xcbf2_9ce4_8422_2325, |hash, chunk| {
        let hash = hash ^ chunk.x as u32 as u64;
        let hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        let hash = hash ^ chunk.z as u32 as u64;
        hash.wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn overlay_position(world_x: i32, world_z: i32, sample: McloneOverworldTerrainSample) -> Vec3 {
    let top_block_y = if sample.watercourse.is_water() {
        sample.surface_y.max(sample.watercourse.water_surface_y)
    } else if sample.surface_y < 63 {
        63
    } else {
        sample.surface_y
    };
    vec3(
        world_x as f32,
        top_block_y as f32 + OVERLAY_HEIGHT_BIAS,
        world_z as f32,
    )
}

fn push_triangle(vertices: &mut Vec<WorldColorVertex>, a: Vec3, b: Vec3, c: Vec3, color: [f32; 4]) {
    vertices.extend([
        WorldColorVertex::new(a, color),
        WorldColorVertex::new(b, color),
        WorldColorVertex::new(c, color),
    ]);
}

fn lens_color(layer: WorldgenLensLayer, sample: McloneOverworldDebugSample) -> [f32; 4] {
    match layer {
        WorldgenLensLayer::Biome => biome_color(sample.biome.recipe),
        WorldgenLensLayer::Landform => landform_color(sample.landform),
        WorldgenLensLayer::Surface => surface_color(sample.surface),
        WorldgenLensLayer::Hydrology => hydrology_color(sample.hydrology),
    }
}

fn biome_color(recipe: McloneOverworldBiomeRecipe) -> [f32; 4] {
    let rgb = match recipe {
        McloneOverworldBiomeRecipe::Ocean => [0.08, 0.24, 0.92],
        McloneOverworldBiomeRecipe::Shore => [0.95, 0.76, 0.24],
        McloneOverworldBiomeRecipe::River => [0.04, 0.78, 1.0],
        McloneOverworldBiomeRecipe::SnowyAlpine => [0.92, 0.88, 1.0],
        McloneOverworldBiomeRecipe::CoolWetConifer => [0.03, 0.45, 0.34],
        McloneOverworldBiomeRecipe::WarmDrySteppe => [0.94, 0.35, 0.08],
        McloneOverworldBiomeRecipe::TemperateWoodland => [0.13, 0.68, 0.18],
        McloneOverworldBiomeRecipe::TemperateMeadow => [0.58, 0.94, 0.12],
    };
    with_alpha(rgb, COLOR_ALPHA)
}

fn landform_color(kind: McloneOverworldLandformKind) -> [f32; 4] {
    let rgb = match kind {
        McloneOverworldLandformKind::Ocean => [0.08, 0.22, 0.90],
        McloneOverworldLandformKind::Coast => [0.94, 0.76, 0.24],
        McloneOverworldLandformKind::River => [0.02, 0.78, 1.0],
        McloneOverworldLandformKind::Wetland => [0.05, 0.68, 0.55],
        McloneOverworldLandformKind::Lowland => [0.40, 0.86, 0.16],
        McloneOverworldLandformKind::Upland => [0.76, 0.58, 0.13],
        McloneOverworldLandformKind::MountainValley => [0.55, 0.30, 0.82],
        McloneOverworldLandformKind::MountainShoulder => [0.90, 0.24, 0.62],
        McloneOverworldLandformKind::MountainMassif => [0.82, 0.82, 0.86],
    };
    with_alpha(rgb, COLOR_ALPHA)
}

fn surface_color(recipe: McloneOverworldSurfaceRecipe) -> [f32; 4] {
    let rgb = match recipe {
        McloneOverworldSurfaceRecipe::OceanFloor => [0.12, 0.28, 0.76],
        McloneOverworldSurfaceRecipe::SandyCoast => [0.96, 0.78, 0.25],
        McloneOverworldSurfaceRecipe::GravelCoast => [0.57, 0.55, 0.50],
        McloneOverworldSurfaceRecipe::RockyCoast => [0.34, 0.36, 0.40],
        McloneOverworldSurfaceRecipe::ColdCoast => [0.88, 0.96, 0.98],
        McloneOverworldSurfaceRecipe::RiverBed => [0.08, 0.74, 0.94],
        McloneOverworldSurfaceRecipe::WetlandBed => [0.16, 0.62, 0.52],
        McloneOverworldSurfaceRecipe::RiverBank => [0.94, 0.45, 0.10],
        McloneOverworldSurfaceRecipe::GrassSoil => [0.30, 0.88, 0.18],
        McloneOverworldSurfaceRecipe::ErodedSlope => [0.64, 0.43, 0.20],
        McloneOverworldSurfaceRecipe::AlpineSnow => [0.94, 0.94, 1.0],
        McloneOverworldSurfaceRecipe::ExposedStone => [0.55, 0.58, 0.64],
    };
    with_alpha(rgb, COLOR_ALPHA)
}

fn hydrology_color(kind: McloneOverworldHydrologyKind) -> [f32; 4] {
    match kind {
        McloneOverworldHydrologyKind::Dry => [0.48, 0.48, 0.48, HYDROLOGY_DRY_ALPHA],
        McloneOverworldHydrologyKind::RiverBank => [1.0, 0.48, 0.05, COLOR_ALPHA],
        McloneOverworldHydrologyKind::Wetland => [0.08, 0.70, 0.46, COLOR_ALPHA],
        McloneOverworldHydrologyKind::WetlandPool => [0.03, 0.90, 0.70, COLOR_ALPHA],
        McloneOverworldHydrologyKind::MajorRiver => [0.02, 0.66, 1.0, COLOR_ALPHA],
        McloneOverworldHydrologyKind::SubmergedOutlet => [0.50, 0.18, 1.0, COLOR_ALPHA],
        McloneOverworldHydrologyKind::PlannedStream => [0.00, 1.0, 1.0, COLOR_ALPHA],
    }
}

const fn with_alpha(rgb: [f32; 3], alpha: f32) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], alpha]
}

fn debug_lines(layer: WorldgenLensLayer, sample: McloneOverworldDebugSample) -> Vec<String> {
    let terrain = sample.landform_sample.terrain;
    let water = terrain.watercourse;
    let adjusted_temperature = terrain
        .climate
        .altitude_adjusted_temperature(terrain.surface_y);
    let mut lines = vec![format!("WORLDGEN LENS {} [F3 OFF F4 NEXT]", layer.label())];
    lines.extend(legend_lines(layer).map(str::to_owned));
    lines.extend([
        format!(
            "LENS BLOCK {} {} CHUNK {} {} QUART {} {}",
            sample.world_x,
            sample.world_z,
            block_to_chunk_coord(sample.world_x),
            block_to_chunk_coord(sample.world_z),
            sample.quart_x,
            sample.quart_z,
        ),
        format!(
            "BIOME {} RULE {}",
            sample.biome.recipe.label().to_ascii_uppercase(),
            sample.biome.reason.label().to_ascii_uppercase()
        ),
        format!(
            "FORM {} SURFACE {}",
            sample.landform.label().to_ascii_uppercase(),
            sample.surface.label().to_ascii_uppercase()
        ),
        format!(
            "HEIGHT BASE {} FINAL {} SLOPE {:.2}",
            terrain.base_surface_y, terrain.surface_y, sample.landform_sample.slope
        ),
        format!(
            "MOUNTAIN {:.2} EXPOSURE {:.2} RIDGE {:.2}",
            terrain.mountain_strength(),
            sample.landform_sample.exposure(),
            terrain.ridges
        ),
        format!(
            "CLIMATE T{:.2} M{:.2} ADJ{:.2}",
            terrain.climate.temperature, terrain.climate.moisture, adjusted_temperature
        ),
        format!(
            "WATER {} CH{:.2} BANK{:.2} WET{:.2}",
            sample.hydrology.label().to_ascii_uppercase(),
            water.channel_influence,
            water.bank_influence,
            water.wetland_influence
        ),
        format!(
            "FLOW ({:.2},{:.2}) WIDTH {:.1} GRADE {:.3}",
            water.flow_x,
            water.flow_z,
            water.half_width * 2.0,
            water.grade
        ),
        format!("LEVEL BED {} WATER {}", water.bed_y, water.water_surface_y,),
    ]);
    lines
}

fn legend_lines(layer: WorldgenLensLayer) -> [&'static str; 2] {
    match layer {
        WorldgenLensLayer::Biome => [
            "LEGEND BLU OCEAN CYN RIVER TAN SHORE WHT ALPINE",
            "       DGN CONIFER ORG STEPPE GRN WOOD LIM MEADOW",
        ],
        WorldgenLensLayer::Landform => [
            "LEGEND BLU WATER TAN COAST GRN LOW GOLD UPLAND",
            "       PUR VALLEY PINK SHOULDER GRAY MASSIF",
        ],
        WorldgenLensLayer::Surface => [
            "LEGEND BLU BEDS TAN BEACH ORG BANK GRN SOIL",
            "       BRN ERODED WHT SNOW GRAY STONE",
        ],
        WorldgenLensLayer::Hydrology => [
            "LEGEND GRAY DRY ORG BANK GRN WETLAND BLU RIVER",
            "       PUR OUTLET (PLANNED STREAMS: LATER LAYER)",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded_region() -> BTreeSet<ChunkPos> {
        (-10..10)
            .flat_map(|z| (-10..10).map(move |x| ChunkPos::new(x, z)))
            .collect()
    }

    #[test]
    fn toggle_releases_mesh_but_retains_selected_layer() {
        let mut state = WorldgenLensState::default();
        assert_eq!(state.toggle(), Some(WorldgenLensLayer::Biome));
        assert_eq!(state.cycle(), WorldgenLensLayer::Landform);
        assert_eq!(state.toggle(), None);
        assert!(state.cache.is_none());
        assert_eq!(state.toggle(), Some(WorldgenLensLayer::Landform));
    }

    #[test]
    fn cycle_from_off_enables_current_layer_before_advancing() {
        let mut state = WorldgenLensState::default();
        assert_eq!(state.cycle(), WorldgenLensLayer::Biome);
        assert_eq!(state.cycle(), WorldgenLensLayer::Landform);
    }

    #[test]
    fn disabled_and_unsupported_profiles_do_not_build_meshes() {
        let mut state = WorldgenLensState::default();
        assert!(
            state
                .prepare_mesh(
                    1,
                    WorldGenerationProfile::McloneOverworldV1,
                    HorizontalTopology::UNBOUNDED,
                    Vec3::ZERO,
                    &loaded_region(),
                )
                .is_none()
        );
        state.toggle();
        assert!(
            state
                .prepare_mesh(
                    1,
                    WorldGenerationProfile::FlatGrassV1,
                    HorizontalTopology::UNBOUNDED,
                    Vec3::ZERO,
                    &loaded_region(),
                )
                .is_none()
        );
        assert!(state.cache.is_none());
    }

    #[test]
    fn cached_mesh_rebuilds_only_across_alignment_or_layer() {
        let mut state = WorldgenLensState::default();
        state.toggle();
        let loaded = loaded_region();
        let first_count = state
            .prepare_mesh(
                8_675_309,
                WorldGenerationProfile::McloneOverworldV1,
                HorizontalTopology::UNBOUNDED,
                Vec3::new(1.0, 100.0, 1.0),
                &loaded,
            )
            .unwrap()
            .vertex_count();
        let first_key = state.cache.as_ref().unwrap().key;
        state.prepare_mesh(
            8_675_309,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
            Vec3::new(15.9, 120.0, 15.9),
            &loaded,
        );
        assert_eq!(state.cache.as_ref().unwrap().key, first_key);
        state.prepare_mesh(
            8_675_309,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
            Vec3::new(16.0, 120.0, 1.0),
            &loaded,
        );
        assert_ne!(state.cache.as_ref().unwrap().key, first_key);
        assert_eq!(first_count % 6, 0);
        assert!(first_count > 30_000);
        assert!(first_count < 80 * 80 * 6);
    }

    #[test]
    fn inspector_identifies_sampling_cells_and_selection_rule() {
        let mut state = WorldgenLensState::default();
        state.toggle();
        let lines = state.inspection_lines(
            8_675_309,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
            Vec3::new(-1.0, 100.0, -1.0),
        );
        assert!(lines[0].contains("WORLDGEN LENS BIOME"));
        assert!(lines.iter().any(|line| line.contains("QUART -1 -1")));
        assert!(lines.iter().any(|line| line.starts_with("BIOME ")));
        assert!(lines.iter().any(|line| line.starts_with("WATER ")));
    }
}
