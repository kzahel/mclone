use crate::levelgen::apply_stream_plans;
use crate::levelgen::{
    MCLONE_OVERWORLD_FIELD_REVISION, MCLONE_OVERWORLD_SEA_LEVEL,
    MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS, McloneOverworldBiomeRecipe, McloneOverworldLandformKind,
    McloneOverworldLandformSample, McloneOverworldSampler, McloneOverworldSamplingTopology,
    McloneOverworldStreamPlan, McloneOverworldStreamPlanCache, McloneOverworldSurfaceRecipe,
    McloneOverworldTerrainSample, McloneOverworldVegetationPlanCache,
    McloneOverworldVegetationPlanner, McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily,
    McloneTreeId, McloneTreeOccurrence, McloneTreeRecord, McloneVegetationBounds,
    McloneVegetationPlanCacheReport, McloneVegetationSource, mclone_overworld_biome_recipe,
    mclone_overworld_landform_kind, mclone_overworld_macro_surface_top_material,
    mclone_overworld_preview_visible_material, mclone_overworld_surface_recipe,
};
use crate::levelgen::{
    VANILLA_OVERWORLD_LOD_REVISION, VanillaOverworldLodSample, VanillaOverworldLodSampler,
    VanillaOverworldMacroSampler,
};
use crate::{
    continental_ecoregion::{ContinentalEcoregionDescriptor, ContinentalEcoregionTopology},
    continental_surface::{
        CONTINENTAL_SURFACE_SCHEMA_REVISION, ContinentalSurfacePlan, ContinentalSurfaceSample,
        ContinentalSurfaceSubstrate, ContinentalSurfaceWaterKind, ContinentalSurfaceWindowRequest,
        TerrainCharacterFamily,
    },
};
use mclone_core::ChunkPos;

use crate::placement::BlockPos;

pub const TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION: &str =
    "mclone-terrain-preview-reference-grid-v10";
pub const TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS: u32 = 64;
pub const TERRAIN_PREVIEW_MIN_CELLS_PER_AXIS: u32 = 8;
pub const TERRAIN_PREVIEW_MAX_CELLS_PER_AXIS: u32 = 128;
pub const TERRAIN_PREVIEW_MIN_SAMPLE_SPACING: u32 = 1;
pub const TERRAIN_PREVIEW_MAX_SAMPLE_SPACING: u32 = 1_024;
pub const TERRAIN_PREVIEW_SAMPLE_FLOATS: usize = 32;
pub const TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING: u32 = 4;
pub const CONTINENTAL_PROXY_MAX_TREE_RECORD_SAMPLE_SPACING: u32 = 16;
pub const CONTINENTAL_PROXY_VEGETATION_REVISION: u16 = 2;
pub const CONTINENTAL_PROXY_VEGETATION_SOURCE_REVISION: &str =
    "mclone-continental-proxy-vegetation-v2";

const CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS: i32 = 24;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum TerrainPreviewSurfaceQuality {
    #[default]
    Basic = 0,
    Inferred = 1,
}

impl TerrainPreviewSurfaceQuality {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Inferred => "inferred",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "basic" | "cheap" => Ok(Self::Basic),
            "inferred" | "builder-aware" => Ok(Self::Inferred),
            other => Err(format!(
                "terrain preview surface quality must be basic or inferred, got {other:?}"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TerrainPreviewProfile {
    #[default]
    McloneOverworldV1,
    ContinentalEcoregionCandidate,
    VanillaOverworld,
}

impl TerrainPreviewProfile {
    pub const fn label(self) -> &'static str {
        match self {
            Self::McloneOverworldV1 => "mclone-overworld-v1",
            Self::ContinentalEcoregionCandidate => "continental-ecoregion-candidate-v1",
            Self::VanillaOverworld => "overworld",
        }
    }

    pub const fn source_revision(self) -> &'static str {
        match self {
            Self::McloneOverworldV1 => MCLONE_OVERWORLD_FIELD_REVISION,
            Self::ContinentalEcoregionCandidate => CONTINENTAL_SURFACE_SCHEMA_REVISION,
            Self::VanillaOverworld => VANILLA_OVERWORLD_LOD_REVISION,
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "mclone-overworld-v1" | "mclone" => Ok(Self::McloneOverworldV1),
            "continental-ecoregion-candidate-v1" | "continental" | "candidate" => {
                Ok(Self::ContinentalEcoregionCandidate)
            }
            "overworld" | "vanilla" | "vanilla-1.17.1" => Ok(Self::VanillaOverworld),
            other => Err(format!(
                "terrain preview profile must be mclone-overworld-v1, \
                 continental-ecoregion-candidate-v1, or overworld, got {other:?}"
            )),
        }
    }

    pub const fn supports_gpu_lod(self) -> bool {
        matches!(self, Self::McloneOverworldV1)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum TerrainPreviewContentStage {
    #[default]
    Base = 0,
    Hydrology = 1,
    Structured = 2,
    Surface = 3,
    Cover = 4,
}

impl TerrainPreviewContentStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Hydrology => "hydrology",
            Self::Structured => "structured",
            Self::Surface => "surface",
            Self::Cover => "cover",
        }
    }

    pub const fn includes_natural_hydrology(self) -> bool {
        self as u32 >= Self::Hydrology as u32
    }

    pub const fn includes_structured_hydrology(self) -> bool {
        self as u32 >= Self::Structured as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPreviewRequest {
    pub profile: TerrainPreviewProfile,
    pub seed: i64,
    pub center_x: i32,
    pub center_z: i32,
    pub sample_spacing: u32,
    pub cells_per_axis: u32,
    pub topology: McloneOverworldSamplingTopology,
    pub content_stage: TerrainPreviewContentStage,
    pub surface_quality: TerrainPreviewSurfaceQuality,
}

impl TerrainPreviewRequest {
    pub const fn new(seed: i64, center_x: i32, center_z: i32, sample_spacing: u32) -> Self {
        Self {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed,
            center_x,
            center_z,
            sample_spacing,
            cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
            topology: McloneOverworldSamplingTopology::Unbounded,
            content_stage: TerrainPreviewContentStage::Base,
            surface_quality: TerrainPreviewSurfaceQuality::Basic,
        }
    }

    pub const fn with_content_stage(mut self, content_stage: TerrainPreviewContentStage) -> Self {
        self.content_stage = content_stage;
        self
    }

    pub const fn with_profile(mut self, profile: TerrainPreviewProfile) -> Self {
        self.profile = profile;
        self
    }

    pub const fn with_surface_quality(
        mut self,
        surface_quality: TerrainPreviewSurfaceQuality,
    ) -> Self {
        self.surface_quality = surface_quality;
        self
    }

    pub fn validate(self) -> Result<ValidatedTerrainPreviewRequest, String> {
        if !self.sample_spacing.is_power_of_two()
            || !(TERRAIN_PREVIEW_MIN_SAMPLE_SPACING..=TERRAIN_PREVIEW_MAX_SAMPLE_SPACING)
                .contains(&self.sample_spacing)
        {
            return Err(format!(
                "terrain preview sample spacing must be a power of two from \
                 {TERRAIN_PREVIEW_MIN_SAMPLE_SPACING} through \
                 {TERRAIN_PREVIEW_MAX_SAMPLE_SPACING}, got {}",
                self.sample_spacing
            ));
        }
        if !self.cells_per_axis.is_power_of_two()
            || !(TERRAIN_PREVIEW_MIN_CELLS_PER_AXIS..=TERRAIN_PREVIEW_MAX_CELLS_PER_AXIS)
                .contains(&self.cells_per_axis)
        {
            return Err(format!(
                "terrain preview cells per axis must be a power of two from \
                 {TERRAIN_PREVIEW_MIN_CELLS_PER_AXIS} through \
                 {TERRAIN_PREVIEW_MAX_CELLS_PER_AXIS}, got {}",
                self.cells_per_axis
            ));
        }

        let footprint_blocks = self
            .cells_per_axis
            .checked_mul(self.sample_spacing)
            .ok_or("terrain preview footprint overflow")?;
        let half_footprint = i32::try_from(footprint_blocks / 2)
            .map_err(|_| "terrain preview half footprint exceeds i32 coordinates")?;
        let min_x = self
            .center_x
            .checked_sub(half_footprint)
            .ok_or("terrain preview minimum X coordinate overflow")?;
        let min_z = self
            .center_z
            .checked_sub(half_footprint)
            .ok_or("terrain preview minimum Z coordinate overflow")?;
        self.center_x
            .checked_add(half_footprint)
            .ok_or("terrain preview maximum X coordinate overflow")?;
        self.center_z
            .checked_add(half_footprint)
            .ok_or("terrain preview maximum Z coordinate overflow")?;

        let samples_per_axis = self
            .cells_per_axis
            .checked_add(1)
            .ok_or("terrain preview sample axis overflow")?;
        let sample_count = samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain preview sample count overflow")?;

        Ok(ValidatedTerrainPreviewRequest {
            request: self,
            min_x,
            min_z,
            footprint_blocks,
            samples_per_axis,
            sample_count,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedTerrainPreviewRequest {
    request: TerrainPreviewRequest,
    min_x: i32,
    min_z: i32,
    footprint_blocks: u32,
    samples_per_axis: u32,
    sample_count: u32,
}

impl ValidatedTerrainPreviewRequest {
    pub const fn request(self) -> TerrainPreviewRequest {
        self.request
    }

    pub const fn min_x(self) -> i32 {
        self.min_x
    }

    pub const fn min_z(self) -> i32 {
        self.min_z
    }

    pub const fn footprint_blocks(self) -> u32 {
        self.footprint_blocks
    }

    pub const fn samples_per_axis(self) -> u32 {
        self.samples_per_axis
    }

    pub const fn sample_count(self) -> u32 {
        self.sample_count
    }

    pub fn world_x(self, sample_x: u32) -> Option<i32> {
        if sample_x >= self.samples_per_axis {
            return None;
        }
        let offset = sample_x.checked_mul(self.request.sample_spacing)?;
        self.min_x.checked_add(i32::try_from(offset).ok()?)
    }

    pub fn world_z(self, sample_z: u32) -> Option<i32> {
        if sample_z >= self.samples_per_axis {
            return None;
        }
        let offset = sample_z.checked_mul(self.request.sample_spacing)?;
        self.min_z.checked_add(i32::try_from(offset).ok()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewSample {
    pub surface_y: f32,
    pub display_y: f32,
    pub continentalness: f32,
    pub relief: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub water: f32,
    pub ruggedness: f32,
    pub base_surface_y: f32,
    pub base_display_y: f32,
    pub ocean_water: f32,
    pub macro_surface_material: f32,
    pub river_signed_distance: f32,
    pub channel_influence: f32,
    pub bank_influence: f32,
    pub river_half_width: f32,
    pub wetland_influence: f32,
    pub wetland_pool_influence: f32,
    pub submerged_outlet_influence: f32,
    pub visible_surface_material: f32,
    pub planned_stream_influence: f32,
    pub biome_recipe: f32,
    pub landform_kind: f32,
    pub surface_recipe: f32,
    pub forest_coverage: f32,
    pub forest_density: f32,
    pub forest_family: f32,
    pub forest_family_mix: f32,
    pub mean_canopy_height: f32,
    pub canopy_height_variation: f32,
    pub grove_or_opening_influence: f32,
    pub forest_summary_available: f32,
}

impl TerrainPreviewSample {
    pub const fn packed(self) -> [f32; TERRAIN_PREVIEW_SAMPLE_FLOATS] {
        [
            self.surface_y,
            self.display_y,
            self.continentalness,
            self.relief,
            self.temperature,
            self.moisture,
            self.water,
            self.ruggedness,
            self.base_surface_y,
            self.base_display_y,
            self.ocean_water,
            self.macro_surface_material,
            self.river_signed_distance,
            self.channel_influence,
            self.bank_influence,
            self.river_half_width,
            self.wetland_influence,
            self.wetland_pool_influence,
            self.submerged_outlet_influence,
            self.visible_surface_material,
            self.planned_stream_influence,
            self.biome_recipe,
            self.landform_kind,
            self.surface_recipe,
            self.forest_coverage,
            self.forest_density,
            self.forest_family,
            self.forest_family_mix,
            self.mean_canopy_height,
            self.canopy_height_variation,
            self.grove_or_opening_influence,
            self.forest_summary_available,
        ]
    }

    pub const fn from_packed(values: [f32; TERRAIN_PREVIEW_SAMPLE_FLOATS]) -> Self {
        Self {
            surface_y: values[0],
            display_y: values[1],
            continentalness: values[2],
            relief: values[3],
            temperature: values[4],
            moisture: values[5],
            water: values[6],
            ruggedness: values[7],
            base_surface_y: values[8],
            base_display_y: values[9],
            ocean_water: values[10],
            macro_surface_material: values[11],
            river_signed_distance: values[12],
            channel_influence: values[13],
            bank_influence: values[14],
            river_half_width: values[15],
            wetland_influence: values[16],
            wetland_pool_influence: values[17],
            submerged_outlet_influence: values[18],
            visible_surface_material: values[19],
            planned_stream_influence: values[20],
            biome_recipe: values[21],
            landform_kind: values[22],
            surface_recipe: values[23],
            forest_coverage: values[24],
            forest_density: values[25],
            forest_family: values[26],
            forest_family_mix: values[27],
            mean_canopy_height: values[28],
            canopy_height_variation: values[29],
            grove_or_opening_influence: values[30],
            forest_summary_available: values[31],
        }
    }

    pub fn is_water(self) -> bool {
        self.water >= 0.5
    }

    pub fn is_ocean_water(self) -> bool {
        self.ocean_water >= 0.5
    }

    pub fn macro_surface_material(self) -> u8 {
        self.macro_surface_material
            .round()
            .clamp(0.0, f32::from(u8::MAX)) as u8
    }

    pub fn visible_surface_material(self) -> u8 {
        self.visible_surface_material
            .round()
            .clamp(0.0, f32::from(u8::MAX)) as u8
    }

    pub fn is_channel(self) -> bool {
        self.channel_influence > 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainPreviewCompileWork {
    pub sample_lattice_points: u64,
    pub terrain_sample_evaluations: u64,
    pub forest_intent_evaluations: u64,
    pub forest_footprint_summaries: u64,
}

impl TerrainPreviewCompileWork {
    pub const fn saturating_add(self, other: Self) -> Self {
        Self {
            sample_lattice_points: self
                .sample_lattice_points
                .saturating_add(other.sample_lattice_points),
            terrain_sample_evaluations: self
                .terrain_sample_evaluations
                .saturating_add(other.terrain_sample_evaluations),
            forest_intent_evaluations: self
                .forest_intent_evaluations
                .saturating_add(other.forest_intent_evaluations),
            forest_footprint_summaries: self
                .forest_footprint_summaries
                .saturating_add(other.forest_footprint_summaries),
        }
    }
}

pub fn terrain_preview_reference_compile_work(
    request: ValidatedTerrainPreviewRequest,
) -> TerrainPreviewCompileWork {
    let source = request.request();
    let sample_lattice_points = request.sample_count() as u64;
    if source.profile != TerrainPreviewProfile::McloneOverworldV1
        || source.content_stage != TerrainPreviewContentStage::Cover
    {
        return TerrainPreviewCompileWork {
            sample_lattice_points,
            terrain_sample_evaluations: sample_lattice_points,
            forest_intent_evaluations: 0,
            forest_footprint_summaries: 0,
        };
    }
    let coarse = source.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING;
    TerrainPreviewCompileWork {
        sample_lattice_points,
        terrain_sample_evaluations: sample_lattice_points.saturating_mul(5),
        forest_intent_evaluations: sample_lattice_points.saturating_mul(if coarse { 4 } else { 1 }),
        forest_footprint_summaries: if coarse { sample_lattice_points } else { 0 },
    }
}

pub fn terrain_preview_gpu_compile_work(
    request: ValidatedTerrainPreviewRequest,
) -> TerrainPreviewCompileWork {
    let source = request.request();
    let sample_lattice_points = request.sample_count() as u64;
    if source.profile != TerrainPreviewProfile::McloneOverworldV1
        || source.content_stage != TerrainPreviewContentStage::Cover
    {
        return TerrainPreviewCompileWork {
            sample_lattice_points,
            terrain_sample_evaluations: sample_lattice_points,
            forest_intent_evaluations: 0,
            forest_footprint_summaries: 0,
        };
    }
    let coarse = source.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING;
    TerrainPreviewCompileWork {
        sample_lattice_points,
        terrain_sample_evaluations: sample_lattice_points.saturating_mul(if coarse {
            5
        } else {
            1
        }),
        forest_intent_evaluations: sample_lattice_points.saturating_mul(if coarse { 4 } else { 1 }),
        forest_footprint_summaries: if coarse { sample_lattice_points } else { 0 },
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainPreviewReferenceGrid {
    request: ValidatedTerrainPreviewRequest,
    samples: Vec<TerrainPreviewSample>,
}

impl TerrainPreviewReferenceGrid {
    pub fn compile(request: TerrainPreviewRequest) -> Result<Self, String> {
        match request.profile {
            TerrainPreviewProfile::McloneOverworldV1 => Self::compile_mclone(request),
            TerrainPreviewProfile::ContinentalEcoregionCandidate => {
                Self::compile_continental(request)
            }
            TerrainPreviewProfile::VanillaOverworld => {
                let mut sampler = VanillaOverworldLodSampler::new(request.seed);
                Self::compile_with_vanilla_sampler(request, &mut sampler)
            }
        }
    }

    fn compile_continental(request: TerrainPreviewRequest) -> Result<Self, String> {
        let (grid, _height_halo) = Self::compile_continental_with_height_halo(request, 0)?;
        Ok(grid)
    }

    /// Compile the continental candidate and the display-height halo used by
    /// the shared horizon's stitched normal contract in one direct query.
    pub fn compile_continental_with_height_halo(
        request: TerrainPreviewRequest,
        halo_radius: u32,
    ) -> Result<(Self, Vec<f32>), String> {
        if request.profile != TerrainPreviewProfile::ContinentalEcoregionCandidate {
            return Err(format!(
                "continental terrain preview compiler cannot compile profile {}",
                request.profile.label()
            ));
        }
        if request.topology != McloneOverworldSamplingTopology::Unbounded {
            return Err(
                "continental terrain preview currently exposes only the unbounded plane; \
                 direct worldgen queries retain the independently proven cylinder contract"
                    .to_owned(),
            );
        }
        let request = request.validate()?;
        let source = request.request();
        let halo_blocks = halo_radius
            .checked_mul(source.sample_spacing)
            .ok_or("continental terrain preview halo span overflow")?;
        let min_x = request
            .min_x()
            .checked_sub(
                i32::try_from(halo_blocks)
                    .map_err(|_| "continental terrain preview halo span exceeds i32 coordinates")?,
            )
            .ok_or("continental terrain preview halo minimum X overflow")?;
        let min_z = request
            .min_z()
            .checked_sub(
                i32::try_from(halo_blocks)
                    .map_err(|_| "continental terrain preview halo span exceeds i32 coordinates")?,
            )
            .ok_or("continental terrain preview halo minimum Z overflow")?;
        let halo_samples_per_axis = request
            .samples_per_axis()
            .checked_add(
                halo_radius
                    .checked_mul(2)
                    .ok_or("continental terrain preview halo sample count overflow")?,
            )
            .ok_or("continental terrain preview halo sample count overflow")?;
        let surface = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::new(
            source.seed,
            ContinentalEcoregionTopology::Plane,
        ))
        .map_err(|error| error.to_string())?;
        let surface_window = surface
            .query_window(ContinentalSurfaceWindowRequest::new(
                min_x,
                min_z,
                halo_samples_per_axis,
                halo_samples_per_axis,
                source.sample_spacing,
            ))
            .map_err(|error| error.to_string())?;
        let mut samples = Vec::with_capacity(
            usize::try_from(request.sample_count())
                .map_err(|_| "terrain preview sample count does not fit usize")?,
        );
        for sample_z in 0..request.samples_per_axis() {
            let source_z = sample_z + halo_radius;
            for sample_x in 0..request.samples_per_axis() {
                let source_x = sample_x + halo_radius;
                let index = usize::try_from(
                    source_z
                        .checked_mul(halo_samples_per_axis)
                        .and_then(|row| row.checked_add(source_x))
                        .ok_or("continental terrain preview sample index overflow")?,
                )
                .map_err(|_| "continental terrain preview sample index exceeds usize")?;
                samples.push(continental_preview_sample(
                    surface_window.samples[index],
                    source.content_stage,
                ));
            }
        }
        let height_halo = surface_window
            .samples
            .iter()
            .map(|sample| sample.display_surface_y)
            .collect();
        Ok((Self { request, samples }, height_halo))
    }

    fn compile_mclone(request: TerrainPreviewRequest) -> Result<Self, String> {
        let request = request.validate()?;
        let source = request.request();
        let sampler = McloneOverworldSampler::new_with_topology(source.seed, source.topology);
        let stream_plans = preview_stream_plans(request)?;
        let vegetation_planner = McloneOverworldVegetationPlanner::new(
            McloneVegetationSource::new(source.seed, source.topology),
        );
        let mut samples = Vec::with_capacity(
            usize::try_from(request.sample_count())
                .map_err(|_| "terrain preview sample count does not fit usize")?,
        );

        for sample_z in 0..request.samples_per_axis() {
            let world_z = request
                .world_z(sample_z)
                .expect("validated terrain preview Z coordinate");
            for sample_x in 0..request.samples_per_axis() {
                let world_x = request
                    .world_x(sample_x)
                    .expect("validated terrain preview X coordinate");
                let mut terrain = sampler.sample(world_x, world_z);
                if let Some(plans) = &stream_plans {
                    let _ = apply_stream_plans(&mut terrain, world_x, world_z, plans);
                }
                let water = terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL
                    || terrain.watercourse.is_water();
                let water_y = if terrain.watercourse.is_water() {
                    terrain.watercourse.water_surface_y
                } else {
                    MCLONE_OVERWORLD_SEA_LEVEL
                };
                let ocean_water = terrain.continentalness <= 0.0;
                let macro_landform = McloneOverworldLandformSample {
                    terrain,
                    slope: 0.0,
                };
                let biome_recipe = mclone_overworld_biome_recipe(macro_landform);
                let surface_recipe = mclone_overworld_surface_recipe(macro_landform);
                let forest_intent = if source.content_stage == TerrainPreviewContentStage::Cover {
                    preview_forest_intent(
                        &sampler,
                        stream_plans.as_deref(),
                        &vegetation_planner,
                        source.sample_spacing,
                        terrain,
                        world_x,
                        world_z,
                    )?
                } else {
                    crate::levelgen::McloneForestIntentSample::EMPTY
                };
                samples.push(TerrainPreviewSample {
                    surface_y: terrain.surface_y as f32,
                    display_y: if water {
                        terrain.surface_y.max(water_y) as f32
                    } else {
                        terrain.surface_y as f32
                    },
                    continentalness: terrain.continentalness as f32,
                    relief: terrain.relief as f32,
                    temperature: terrain.climate.temperature as f32,
                    moisture: terrain.climate.moisture as f32,
                    water: if water { 1.0 } else { 0.0 },
                    ruggedness: terrain.ruggedness as f32,
                    base_surface_y: terrain.base_surface_y as f32,
                    base_display_y: if ocean_water {
                        MCLONE_OVERWORLD_SEA_LEVEL as f32
                    } else {
                        terrain.base_surface_y as f32
                    },
                    ocean_water: if ocean_water { 1.0 } else { 0.0 },
                    macro_surface_material: f32::from(mclone_overworld_macro_surface_top_material(
                        terrain,
                    )),
                    river_signed_distance: terrain.watercourse.signed_distance as f32,
                    channel_influence: terrain.watercourse.channel_influence as f32,
                    bank_influence: terrain.watercourse.bank_influence as f32,
                    river_half_width: terrain.watercourse.half_width as f32,
                    wetland_influence: terrain.watercourse.wetland_influence as f32,
                    wetland_pool_influence: terrain.watercourse.wetland_pool_influence as f32,
                    submerged_outlet_influence: terrain.watercourse.submerged_outlet_influence
                        as f32,
                    visible_surface_material: f32::from(mclone_overworld_preview_visible_material(
                        terrain,
                    )),
                    planned_stream_influence: terrain.watercourse.planned_stream_influence as f32,
                    biome_recipe: biome_recipe_code(biome_recipe),
                    landform_kind: landform_kind_code(mclone_overworld_landform_kind(
                        macro_landform,
                    )),
                    surface_recipe: surface_recipe_code(surface_recipe),
                    forest_coverage: forest_intent.coverage,
                    forest_density: forest_intent.density,
                    forest_family: forest_family_code(forest_intent.dominant_family),
                    forest_family_mix: forest_intent.family_mix,
                    mean_canopy_height: forest_intent.mean_canopy_height,
                    canopy_height_variation: forest_intent.canopy_height_variation,
                    grove_or_opening_influence: forest_intent.grove_or_opening_influence,
                    forest_summary_available: if source.content_stage
                        == TerrainPreviewContentStage::Cover
                    {
                        1.0
                    } else {
                        0.0
                    },
                });
            }
        }

        Ok(Self { request, samples })
    }

    pub fn compile_with_vanilla_sampler(
        request: TerrainPreviewRequest,
        sampler: &mut VanillaOverworldLodSampler,
    ) -> Result<Self, String> {
        if request.profile != TerrainPreviewProfile::VanillaOverworld {
            return Err(format!(
                "vanilla terrain preview compiler cannot compile profile {}",
                request.profile.label()
            ));
        }
        if request.seed != sampler.seed() {
            return Err(format!(
                "vanilla terrain preview seed {} does not match sampler seed {}",
                request.seed,
                sampler.seed()
            ));
        }
        if request.topology != McloneOverworldSamplingTopology::Unbounded {
            return Err("vanilla terrain preview supports only unbounded topology".to_owned());
        }
        let request = request.validate()?;
        let mut samples = Vec::with_capacity(
            usize::try_from(request.sample_count())
                .map_err(|_| "terrain preview sample count does not fit usize")?,
        );
        for sample_z in 0..request.samples_per_axis() {
            let world_z = request
                .world_z(sample_z)
                .expect("validated terrain preview Z coordinate");
            for sample_x in 0..request.samples_per_axis() {
                let world_x = request
                    .world_x(sample_x)
                    .expect("validated terrain preview X coordinate");
                let sample = sampler.sample_with_surface_inference(
                    world_x,
                    world_z,
                    request.request().surface_quality == TerrainPreviewSurfaceQuality::Inferred,
                );
                samples.push(vanilla_preview_sample(sample));
            }
        }
        Ok(Self { request, samples })
    }

    pub fn compile_with_vanilla_macro_sampler(
        request: TerrainPreviewRequest,
        sampler: &mut VanillaOverworldMacroSampler,
    ) -> Result<Self, String> {
        if request.profile != TerrainPreviewProfile::VanillaOverworld {
            return Err(format!(
                "vanilla macro terrain preview compiler cannot compile profile {}",
                request.profile.label()
            ));
        }
        if request.seed != sampler.seed() {
            return Err(format!(
                "vanilla macro terrain preview seed {} does not match sampler seed {}",
                request.seed,
                sampler.seed()
            ));
        }
        if request.topology != McloneOverworldSamplingTopology::Unbounded {
            return Err(
                "vanilla macro terrain preview supports only unbounded topology".to_owned(),
            );
        }
        let request = request.validate()?;
        let mut samples = Vec::with_capacity(
            usize::try_from(request.sample_count())
                .map_err(|_| "terrain preview sample count does not fit usize")?,
        );
        for sample_z in 0..request.samples_per_axis() {
            let world_z = request
                .world_z(sample_z)
                .expect("validated terrain preview Z coordinate");
            for sample_x in 0..request.samples_per_axis() {
                let world_x = request
                    .world_x(sample_x)
                    .expect("validated terrain preview X coordinate");
                samples.push(vanilla_preview_sample(
                    sampler.sample_with_surface_inference(
                        world_x,
                        world_z,
                        request.request().surface_quality == TerrainPreviewSurfaceQuality::Inferred,
                    ),
                ));
            }
        }
        Ok(Self { request, samples })
    }

    pub fn from_packed_f32(request: TerrainPreviewRequest, packed: &[f32]) -> Result<Self, String> {
        let request = request.validate()?;
        let expected = usize::try_from(request.sample_count())
            .map_err(|_| "terrain preview sample count does not fit usize")?
            .checked_mul(TERRAIN_PREVIEW_SAMPLE_FLOATS)
            .ok_or("terrain preview packed sample count overflow")?;
        if packed.len() != expected {
            return Err(format!(
                "terrain preview packed grid has {} floats, expected {expected}",
                packed.len()
            ));
        }
        let mut samples = Vec::with_capacity(expected / TERRAIN_PREVIEW_SAMPLE_FLOATS);
        for values in packed.chunks_exact(TERRAIN_PREVIEW_SAMPLE_FLOATS) {
            if !values.iter().all(|value| value.is_finite()) {
                return Err("terrain preview packed grid contains a non-finite value".to_owned());
            }
            let values: [f32; TERRAIN_PREVIEW_SAMPLE_FLOATS] = values
                .try_into()
                .expect("chunks_exact returns one complete terrain preview sample");
            samples.push(TerrainPreviewSample::from_packed(values));
        }
        Ok(Self { request, samples })
    }

    pub const fn request(&self) -> ValidatedTerrainPreviewRequest {
        self.request
    }

    pub fn samples(&self) -> &[TerrainPreviewSample] {
        &self.samples
    }

    pub fn compile_work(&self) -> TerrainPreviewCompileWork {
        terrain_preview_reference_compile_work(self.request)
    }

    pub fn sample(&self, sample_x: u32, sample_z: u32) -> Option<TerrainPreviewSample> {
        if sample_x >= self.request.samples_per_axis()
            || sample_z >= self.request.samples_per_axis()
        {
            return None;
        }
        let index = usize::try_from(sample_z).ok()?
            * usize::try_from(self.request.samples_per_axis()).ok()?
            + usize::try_from(sample_x).ok()?;
        self.samples.get(index).copied()
    }

    pub fn packed_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            self.samples.len() * TERRAIN_PREVIEW_SAMPLE_FLOATS * std::mem::size_of::<f32>(),
        );
        for sample in &self.samples {
            for value in sample.packed() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    pub fn packed_f32(&self) -> Vec<f32> {
        self.samples
            .iter()
            .flat_map(|sample| sample.packed())
            .collect()
    }
}

fn continental_preview_sample(
    sample: ContinentalSurfaceSample,
    content_stage: TerrainPreviewContentStage,
) -> TerrainPreviewSample {
    let coast = sample.family_weights[TerrainCharacterFamily::CoastAndHeadland as usize];
    let upland = sample.family_weights[TerrainCharacterFamily::UplandAndEscarpment as usize];
    let water = sample.is_water();
    let ocean = sample.water_kind == ContinentalSurfaceWaterKind::Ocean;
    let river = sample.water_kind == ContinentalSurfaceWaterKind::River;
    let wetland_pool = sample.water_kind == ContinentalSurfaceWaterKind::WetlandPool;
    let river_influence = if river { sample.route.max(0.55) } else { 0.0 };
    let forest_coverage = if content_stage == TerrainPreviewContentStage::Cover {
        (sample.forest_core * 0.88 + sample.forest_edge * 0.24).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let biome_recipe = if ocean {
        0.0
    } else if coast > 0.55 {
        1.0
    } else if river {
        2.0
    } else if sample.dominant_family == TerrainCharacterFamily::AridRainShadow {
        5.0
    } else if sample.forest_core > 0.46 {
        6.0
    } else {
        7.0
    };
    let landform_kind = match sample.dominant_family {
        TerrainCharacterFamily::CoastAndHeadland => 1.0,
        TerrainCharacterFamily::RollingInterior => 5.0,
        TerrainCharacterFamily::FluvialAndWetland => {
            if sample.wetland > river_influence {
                3.0
            } else {
                2.0
            }
        }
        TerrainCharacterFamily::UplandAndEscarpment => 8.0,
        TerrainCharacterFamily::AridRainShadow => 7.0,
    };
    let surface_recipe = match (sample.water_kind, sample.substrate) {
        (ContinentalSurfaceWaterKind::Ocean, ContinentalSurfaceSubstrate::Sand) => 1.0,
        (ContinentalSurfaceWaterKind::Ocean, ContinentalSurfaceSubstrate::Gravel) => 9.0,
        (ContinentalSurfaceWaterKind::Ocean, _) => 0.0,
        (ContinentalSurfaceWaterKind::River, _) => 2.0,
        (ContinentalSurfaceWaterKind::Lake, _) | (ContinentalSurfaceWaterKind::WetlandPool, _) => {
            3.0
        }
        (_, ContinentalSurfaceSubstrate::Stone) => 8.0,
        (_, ContinentalSurfaceSubstrate::Grass) => 5.0,
        (_, ContinentalSurfaceSubstrate::Sand) => 1.0,
        (_, ContinentalSurfaceSubstrate::Gravel) => 6.0,
        (_, ContinentalSurfaceSubstrate::CoarseSoil) => 6.0,
    };
    let base_surface_y = MCLONE_OVERWORLD_SEA_LEVEL as f32 + sample.continental_height;
    TerrainPreviewSample {
        surface_y: sample.solid_surface_y,
        display_y: sample.display_surface_y,
        continentalness: if ocean { -0.72 } else { 0.92 - coast * 0.52 },
        relief: (sample.province_height.abs() / 52.0).clamp(0.0, 1.0),
        temperature: sample.temperature,
        // The fixed preview carrier has one ecological moisture lane. For the
        // candidate it carries effective moisture after rain-shadow drying;
        // the richer surface query retains raw moisture and aridity separately.
        moisture: 1.0 - sample.aridity,
        water: f32::from(water),
        ruggedness: (upland * 0.78 + (sample.local_height.abs() / 16.0) * 0.22).clamp(0.0, 1.0),
        base_surface_y,
        base_display_y: if ocean {
            MCLONE_OVERWORLD_SEA_LEVEL as f32
        } else {
            base_surface_y
        },
        ocean_water: f32::from(ocean),
        macro_surface_material: f32::from(sample.substrate.block_id()),
        river_signed_distance: if river {
            -sample.route.max(0.05) * 24.0
        } else {
            (1.0 - sample.route) * 64.0
        },
        channel_influence: river_influence,
        bank_influence: if river {
            (1.0 - river_influence) * sample.route
        } else {
            0.0
        },
        river_half_width: if river {
            4.0 + sample.route * 18.0
        } else {
            0.0
        },
        wetland_influence: sample.wetland,
        wetland_pool_influence: if wetland_pool { 1.0 } else { 0.0 },
        submerged_outlet_influence: 0.0,
        visible_surface_material: f32::from(sample.visible_material()),
        planned_stream_influence: river_influence,
        biome_recipe,
        landform_kind,
        surface_recipe,
        forest_coverage,
        forest_density: forest_coverage * (0.48 + sample.forest_core * 0.42),
        forest_family: if forest_coverage > 0.0 { 1.0 } else { 0.0 },
        forest_family_mix: sample.forest_edge * 0.34,
        mean_canopy_height: 8.0 + sample.forest_core * 7.0,
        canopy_height_variation: 1.5 + sample.forest_edge * 4.0,
        grove_or_opening_influence: sample.clearing.max(sample.openness * 0.35),
        forest_summary_available: f32::from(content_stage == TerrainPreviewContentStage::Cover),
    }
}

fn preview_terrain_sample(
    sampler: &McloneOverworldSampler,
    stream_plans: Option<&[McloneOverworldStreamPlan]>,
    world_x: i32,
    world_z: i32,
) -> McloneOverworldTerrainSample {
    let mut terrain = sampler.sample(world_x, world_z);
    if let Some(plans) = stream_plans {
        let _ = apply_stream_plans(&mut terrain, world_x, world_z, plans);
    }
    terrain
}

pub(crate) fn preview_forest_intent(
    sampler: &McloneOverworldSampler,
    stream_plans: Option<&[McloneOverworldStreamPlan]>,
    vegetation_planner: &McloneOverworldVegetationPlanner,
    sample_spacing: u32,
    terrain: McloneOverworldTerrainSample,
    world_x: i32,
    world_z: i32,
) -> Result<crate::levelgen::McloneForestIntentSample, String> {
    if sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING {
        preview_point_forest_intent(
            sampler,
            stream_plans,
            vegetation_planner,
            terrain,
            world_x,
            world_z,
        )
    } else {
        preview_footprint_forest_summary(
            sampler,
            stream_plans,
            vegetation_planner,
            sample_spacing,
            world_x,
            world_z,
        )
    }
}

fn preview_point_forest_intent(
    sampler: &McloneOverworldSampler,
    stream_plans: Option<&[McloneOverworldStreamPlan]>,
    vegetation_planner: &McloneOverworldVegetationPlanner,
    terrain: McloneOverworldTerrainSample,
    world_x: i32,
    world_z: i32,
) -> Result<crate::levelgen::McloneForestIntentSample, String> {
    let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
    let west_x = world_x
        .checked_sub(radius)
        .ok_or("Terrain Lab vegetation west sample overflow")?;
    let east_x = world_x
        .checked_add(radius)
        .ok_or("Terrain Lab vegetation east sample overflow")?;
    let north_z = world_z
        .checked_sub(radius)
        .ok_or("Terrain Lab vegetation north sample overflow")?;
    let south_z = world_z
        .checked_add(radius)
        .ok_or("Terrain Lab vegetation south sample overflow")?;
    let forest_landform = McloneOverworldLandformSample::from_cardinal_samples(
        terrain,
        preview_terrain_sample(sampler, stream_plans, west_x, world_z),
        preview_terrain_sample(sampler, stream_plans, east_x, world_z),
        preview_terrain_sample(sampler, stream_plans, world_x, north_z),
        preview_terrain_sample(sampler, stream_plans, world_x, south_z),
    );
    Ok(vegetation_planner.forest_intent(forest_landform, world_x, world_z))
}

fn preview_footprint_forest_summary(
    sampler: &McloneOverworldSampler,
    stream_plans: Option<&[McloneOverworldStreamPlan]>,
    vegetation_planner: &McloneOverworldVegetationPlanner,
    sample_spacing: u32,
    world_x: i32,
    world_z: i32,
) -> Result<crate::levelgen::McloneForestIntentSample, String> {
    let offset = i32::try_from(sample_spacing / 4)
        .map_err(|_| "Terrain Lab vegetation footprint offset exceeds i32")?;
    let min_x = world_x
        .checked_sub(offset)
        .ok_or("Terrain Lab vegetation footprint minimum X overflow")?;
    let max_x = world_x
        .checked_add(offset)
        .ok_or("Terrain Lab vegetation footprint maximum X overflow")?;
    let min_z = world_z
        .checked_sub(offset)
        .ok_or("Terrain Lab vegetation footprint minimum Z overflow")?;
    let max_z = world_z
        .checked_add(offset)
        .ok_or("Terrain Lab vegetation footprint maximum Z overflow")?;
    let coordinates = [
        (min_x, min_z),
        (max_x, min_z),
        (min_x, max_z),
        (max_x, max_z),
    ];
    let taps = coordinates.map(|(tap_x, tap_z)| {
        (
            McloneOverworldLandformSample {
                terrain: preview_terrain_sample(sampler, stream_plans, tap_x, tap_z),
                slope: 0.0,
            },
            tap_x,
            tap_z,
        )
    });
    Ok(vegetation_planner.forest_footprint_summary(taps))
}

fn vanilla_preview_sample(sample: VanillaOverworldLodSample) -> TerrainPreviewSample {
    TerrainPreviewSample {
        surface_y: sample.solid_surface_y as f32,
        display_y: sample.display_y as f32,
        continentalness: 0.0,
        relief: 0.0,
        temperature: 0.0,
        moisture: 0.0,
        water: if sample.water { 1.0 } else { 0.0 },
        ruggedness: 0.0,
        base_surface_y: sample.solid_surface_y as f32,
        base_display_y: sample.display_y as f32,
        ocean_water: if sample.water { 1.0 } else { 0.0 },
        macro_surface_material: f32::from(sample.visible_material),
        river_signed_distance: 0.0,
        channel_influence: 0.0,
        bank_influence: 0.0,
        river_half_width: 0.0,
        wetland_influence: 0.0,
        wetland_pool_influence: 0.0,
        submerged_outlet_influence: 0.0,
        visible_surface_material: f32::from(sample.visible_material),
        planned_stream_influence: 0.0,
        biome_recipe: sample.biome.id() as f32,
        landform_kind: 0.0,
        surface_recipe: f32::from(sample.approximate_surface_material),
        forest_coverage: 0.0,
        forest_density: 0.0,
        forest_family: 0.0,
        forest_family_mix: 0.0,
        mean_canopy_height: 0.0,
        canopy_height_variation: 0.0,
        grove_or_opening_influence: 0.0,
        forest_summary_available: 0.0,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainPreviewVegetationProduct {
    request: ValidatedTerrainPreviewRequest,
    summary_available: bool,
    records_requested: bool,
    records_aggregated: bool,
    occurrences: Vec<McloneTreeOccurrence>,
    cache_report: McloneVegetationPlanCacheReport,
}

impl TerrainPreviewVegetationProduct {
    pub fn compile(request: TerrainPreviewRequest) -> Result<Self, String> {
        let source = McloneVegetationSource::new(request.seed, request.topology);
        let mut cache = McloneOverworldVegetationPlanCache::new(source);
        Self::compile_with_cache(request, &mut cache)
    }

    pub fn compile_with_cache(
        request: TerrainPreviewRequest,
        cache: &mut McloneOverworldVegetationPlanCache,
    ) -> Result<Self, String> {
        let request = request.validate()?;
        let source = request.request();
        let summary_available = matches!(
            source.profile,
            TerrainPreviewProfile::McloneOverworldV1
                | TerrainPreviewProfile::ContinentalEcoregionCandidate
        ) && source.content_stage == TerrainPreviewContentStage::Cover;
        let records_requested = summary_available
            && terrain_preview_requests_tree_records_for_profile(
                source.profile,
                source.sample_spacing,
            );
        if !records_requested {
            return Ok(Self {
                request,
                summary_available,
                records_requested: false,
                records_aggregated: summary_available,
                occurrences: Vec::new(),
                cache_report: McloneVegetationPlanCacheReport::default(),
            });
        }

        if source.profile == TerrainPreviewProfile::ContinentalEcoregionCandidate {
            let occurrences = compile_continental_proxy_vegetation(request)?;
            return Ok(Self {
                request,
                summary_available,
                records_requested: true,
                records_aggregated: false,
                occurrences,
                cache_report: McloneVegetationPlanCacheReport::default(),
            });
        }

        let vegetation_source = McloneVegetationSource::new(source.seed, source.topology);
        if !cache.matches(vegetation_source) {
            *cache = McloneOverworldVegetationPlanCache::new(vegetation_source);
        }
        let before = cache.report();
        let footprint = i32::try_from(request.footprint_blocks())
            .map_err(|_| "terrain preview vegetation footprint exceeds i32")?;
        let max_x = request
            .min_x()
            .checked_add(footprint - 1)
            .ok_or("terrain preview vegetation maximum X overflow")?;
        let max_z = request
            .min_z()
            .checked_add(footprint - 1)
            .ok_or("terrain preview vegetation maximum Z overflow")?;
        let bounds = McloneVegetationBounds::new(request.min_x(), request.min_z(), max_x, max_z)
            .map_err(|error| error.to_string())?;
        let mut occurrences = cache
            .tree_records_intersecting(bounds)
            .map_err(|error| error.to_string())?;
        occurrences.retain(|occurrence| {
            let Ok(base) = occurrence.working_base() else {
                return false;
            };
            base.x >= request.min_x()
                && base.x < request.min_x() + footprint
                && base.z >= request.min_z()
                && base.z < request.min_z() + footprint
                && terrain_preview_tree_record_admitted(
                    source.sample_spacing,
                    occurrence.record.landmark_rank,
                )
        });
        let after = cache.report();

        Ok(Self {
            request,
            summary_available,
            records_requested: true,
            records_aggregated: false,
            occurrences,
            cache_report: McloneVegetationPlanCacheReport {
                cell_requests: after.cell_requests.saturating_sub(before.cell_requests),
                cell_hits: after.cell_hits.saturating_sub(before.cell_hits),
                cell_misses: after.cell_misses.saturating_sub(before.cell_misses),
                retained_cells: after.retained_cells,
                retained_preliminary_candidates: after.retained_preliminary_candidates,
            },
        })
    }

    pub const fn request(&self) -> ValidatedTerrainPreviewRequest {
        self.request
    }

    pub const fn summary_available(&self) -> bool {
        self.summary_available
    }

    pub const fn records_requested(&self) -> bool {
        self.records_requested
    }

    pub const fn records_aggregated(&self) -> bool {
        self.records_aggregated
    }

    pub fn occurrences(&self) -> &[McloneTreeOccurrence] {
        &self.occurrences
    }

    pub const fn cache_report(&self) -> McloneVegetationPlanCacheReport {
        self.cache_report
    }

    pub fn estimated_record_bytes(&self) -> usize {
        self.occurrences
            .len()
            .saturating_mul(std::mem::size_of::<McloneTreeOccurrence>())
    }

    pub(crate) fn from_codec_parts(
        request: TerrainPreviewRequest,
        summary_available: bool,
        records_requested: bool,
        records_aggregated: bool,
        occurrences: Vec<McloneTreeOccurrence>,
        cache_report: McloneVegetationPlanCacheReport,
    ) -> Result<Self, String> {
        let request = request.validate()?;
        let source = request.request();
        let expected_summary = matches!(
            source.profile,
            TerrainPreviewProfile::McloneOverworldV1
                | TerrainPreviewProfile::ContinentalEcoregionCandidate
        ) && source.content_stage == TerrainPreviewContentStage::Cover;
        let expected_records = expected_summary
            && terrain_preview_requests_tree_records_for_profile(
                source.profile,
                source.sample_spacing,
            );
        if summary_available != expected_summary
            || records_requested != expected_records
            || records_aggregated != (expected_summary && !expected_records)
        {
            return Err("terrain vegetation product flags do not match its request".to_owned());
        }
        if !expected_records && !occurrences.is_empty() {
            return Err(
                "terrain vegetation product contains records for an aggregate-only request"
                    .to_owned(),
            );
        }
        if cache_report.cell_hits > cache_report.cell_requests
            || cache_report.cell_misses
                != cache_report
                    .cell_requests
                    .saturating_sub(cache_report.cell_hits)
        {
            return Err("terrain vegetation cache report is inconsistent".to_owned());
        }

        let footprint = i32::try_from(request.footprint_blocks())
            .map_err(|_| "terrain vegetation product footprint exceeds i32")?;
        for occurrence in &occurrences {
            let base = occurrence
                .working_base()
                .map_err(|error| error.to_string())?;
            if base.x < request.min_x()
                || base.x >= request.min_x() + footprint
                || base.z < request.min_z()
                || base.z >= request.min_z() + footprint
                || !terrain_preview_tree_record_admitted_for_profile(
                    source.profile,
                    source.sample_spacing,
                    occurrence.record.landmark_rank,
                )
            {
                return Err("terrain vegetation occurrence is outside request admission".to_owned());
            }
        }
        if occurrences.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err("terrain vegetation occurrences are not stably ordered".to_owned());
        }

        Ok(Self {
            request,
            summary_available,
            records_requested,
            records_aggregated,
            occurrences,
            cache_report,
        })
    }
}

/// Build review-only tree proxies from the candidate's own cover facts.
///
/// The fixed global lattice makes ownership independent of preview tile
/// partitioning and clipmap residency. These records deliberately use the
/// existing neutral proxy carrier, but never consult the production forest
/// planner or its cache.
fn compile_continental_proxy_vegetation(
    request: ValidatedTerrainPreviewRequest,
) -> Result<Vec<McloneTreeOccurrence>, String> {
    let source = request.request();
    if source.topology != McloneOverworldSamplingTopology::Unbounded {
        return Err(
            "continental proxy vegetation currently supports only the candidate's unbounded \
             World Explorer source"
                .to_owned(),
        );
    }
    let footprint = i32::try_from(request.footprint_blocks())
        .map_err(|_| "continental proxy vegetation footprint exceeds i32")?;
    let max_x = request
        .min_x()
        .checked_add(footprint)
        .ok_or("continental proxy vegetation maximum X overflow")?;
    let max_z = request
        .min_z()
        .checked_add(footprint)
        .ok_or("continental proxy vegetation maximum Z overflow")?;
    let last_x = max_x
        .checked_sub(1)
        .ok_or("continental proxy vegetation empty X footprint")?;
    let last_z = max_z
        .checked_sub(1)
        .ok_or("continental proxy vegetation empty Z footprint")?;
    let min_cell_x = request
        .min_x()
        .div_euclid(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS);
    let max_cell_x = last_x.div_euclid(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS);
    let min_cell_z = request
        .min_z()
        .div_euclid(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS);
    let max_cell_z = last_z.div_euclid(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS);
    let surface = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::new(
        source.seed,
        ContinentalEcoregionTopology::Plane,
    ))
    .map_err(|error| error.to_string())?;
    let mut occurrences = Vec::new();

    for cell_x in min_cell_x..=max_cell_x {
        for cell_z in min_cell_z..=max_cell_z {
            let position_hash = continental_proxy_hash(source.seed, cell_x, cell_z, 0);
            let landmark_rank =
                (continental_proxy_hash(source.seed, cell_x, cell_z, 2) >> 56) as u8;
            if !continental_proxy_tree_record_admitted(source.sample_spacing, landmark_rank) {
                continue;
            }
            let inset = 2_i32;
            let jitter_span = CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS - inset * 2;
            let world_x = cell_x
                .checked_mul(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS)
                .and_then(|value| {
                    value.checked_add(inset + (position_hash as i32).rem_euclid(jitter_span))
                })
                .ok_or("continental proxy vegetation X coordinate overflow")?;
            let world_z = cell_z
                .checked_mul(CONTINENTAL_PROXY_VEGETATION_CELL_BLOCKS)
                .and_then(|value| {
                    value
                        .checked_add(inset + ((position_hash >> 32) as i32).rem_euclid(jitter_span))
                })
                .ok_or("continental proxy vegetation Z coordinate overflow")?;
            if world_x < request.min_x()
                || world_x >= max_x
                || world_z < request.min_z()
                || world_z >= max_z
            {
                continue;
            }

            let sample = surface.query_point(world_x, world_z).sample;
            if sample.is_water() {
                continue;
            }
            let forest_structure = sample.forest_core.max(sample.forest_edge * 0.72);
            let arid_scrub = ((sample.aridity - 0.58).max(0.0) * 0.24)
                * (1.0 - sample.wetland)
                * (1.0 - sample.clearing * 0.45);
            if forest_structure < 0.12 && arid_scrub < 0.02 {
                continue;
            }
            let density = (0.10 + sample.forest_core * 0.78 + sample.forest_edge * 0.30)
                * (1.0 - sample.clearing * 0.94)
                * (1.0 - sample.openness * 0.38)
                * (1.0 - sample.wetland * 0.38)
                + arid_scrub;
            let admission_hash = continental_proxy_hash(source.seed, cell_x, cell_z, 1);
            let admission = (admission_hash as u32) as f64 / f64::from(u32::MAX);
            if admission >= f64::from(density.clamp(0.0, 0.92)) {
                continue;
            }

            let family = if sample.aridity > 0.68 && sample.temperature > 0.48 {
                McloneTreeFamily::WarmDryAcacia
            } else if sample.moisture > 0.64 && sample.temperature < 0.58 {
                McloneTreeFamily::CoolWetConifer
            } else {
                McloneTreeFamily::TemperateBroadleaf
            };
            let silhouette = continental_proxy_hash(source.seed, cell_x, cell_z, 3);
            let (archetype, trunk_height, crown_radius, crown_depth) = match family {
                McloneTreeFamily::TemperateBroadleaf => (
                    McloneTreeArchetype::RoundedBroadleaf,
                    8 + (silhouette % 5) as u16,
                    3 + ((silhouette >> 8) % 3) as u16,
                    4 + ((silhouette >> 16) % 4) as u16,
                ),
                McloneTreeFamily::CoolWetConifer => (
                    McloneTreeArchetype::LayeredConifer,
                    11 + (silhouette % 7) as u16,
                    3 + ((silhouette >> 8) % 3) as u16,
                    7 + ((silhouette >> 16) % 5) as u16,
                ),
                McloneTreeFamily::WarmDryAcacia => (
                    McloneTreeArchetype::ForkedAcacia,
                    6 + (silhouette % 5) as u16,
                    4 + ((silhouette >> 8) % 3) as u16,
                    3 + ((silhouette >> 16) % 3) as u16,
                ),
            };
            let base_y = (sample.solid_surface_y.floor() as i32)
                .checked_add(1)
                .ok_or("continental proxy vegetation Y coordinate overflow")?;
            let base = BlockPos::new(world_x, base_y, world_z);
            let horizontal_radius =
                i32::from(crown_radius) + i32::from(family == McloneTreeFamily::WarmDryAcacia) * 2;
            let max_y = base_y
                .checked_add(i32::from(trunk_height))
                .and_then(|value| value.checked_add(2))
                .ok_or("continental proxy vegetation height overflow")?;
            let bounds = McloneTreeBounds::new(
                world_x
                    .checked_sub(horizontal_radius)
                    .ok_or("continental proxy vegetation bounds overflow")?,
                base_y
                    .checked_sub(1)
                    .ok_or("continental proxy vegetation bounds overflow")?,
                world_z
                    .checked_sub(horizontal_radius)
                    .ok_or("continental proxy vegetation bounds overflow")?,
                world_x
                    .checked_add(horizontal_radius)
                    .ok_or("continental proxy vegetation bounds overflow")?,
                max_y,
                world_z
                    .checked_add(horizontal_radius)
                    .ok_or("continental proxy vegetation bounds overflow")?,
            )
            .map_err(|error| error.to_string())?;
            let record = McloneTreeRecord {
                id: McloneTreeId {
                    planning_cell_x: cell_x,
                    planning_cell_z: cell_z,
                    candidate_slot: 0,
                    vegetation_revision: CONTINENTAL_PROXY_VEGETATION_REVISION,
                },
                canonical_base: base,
                family,
                archetype,
                trunk_height,
                crown_radius,
                crown_depth,
                orientation: (continental_proxy_hash(source.seed, cell_x, cell_z, 4) & 3) as u8,
                landmark_rank,
                variant_seed: continental_proxy_hash(source.seed, cell_x, cell_z, 5),
                bounds,
            };
            occurrences.push(McloneTreeOccurrence {
                record,
                x_lift: 0,
                working_bounds: bounds,
            });
        }
    }
    occurrences.sort_unstable();
    Ok(occurrences)
}

fn continental_proxy_hash(seed: i64, cell_x: i32, cell_z: i32, lane: u64) -> u64 {
    let mut value =
        (seed as u64) ^ 0x6376_6567_7072_6f78 ^ lane.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= (cell_x as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = splitmix64(value);
    value ^= (cell_z as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    splitmix64(value)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub const fn terrain_preview_requests_tree_records(sample_spacing: u32) -> bool {
    sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
}

pub const fn terrain_preview_max_tree_record_sample_spacing(profile: TerrainPreviewProfile) -> u32 {
    match profile {
        TerrainPreviewProfile::ContinentalEcoregionCandidate => {
            CONTINENTAL_PROXY_MAX_TREE_RECORD_SAMPLE_SPACING
        }
        TerrainPreviewProfile::McloneOverworldV1 | TerrainPreviewProfile::VanillaOverworld => {
            TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
        }
    }
}

pub const fn terrain_preview_requests_tree_records_for_profile(
    profile: TerrainPreviewProfile,
    sample_spacing: u32,
) -> bool {
    sample_spacing <= terrain_preview_max_tree_record_sample_spacing(profile)
}

pub const fn terrain_preview_tree_record_admitted(sample_spacing: u32, landmark_rank: u8) -> bool {
    match sample_spacing {
        0 | 1 => true,
        2 => landmark_rank >= 2,
        3 | 4 => landmark_rank >= 3,
        _ => false,
    }
}

pub const fn terrain_preview_tree_record_admitted_for_profile(
    profile: TerrainPreviewProfile,
    sample_spacing: u32,
    landmark_rank: u8,
) -> bool {
    match profile {
        TerrainPreviewProfile::ContinentalEcoregionCandidate => {
            continental_proxy_tree_record_admitted(sample_spacing, landmark_rank)
        }
        TerrainPreviewProfile::McloneOverworldV1 | TerrainPreviewProfile::VanillaOverworld => {
            terrain_preview_tree_record_admitted(sample_spacing, landmark_rank)
        }
    }
}

const fn continental_proxy_tree_record_admitted(sample_spacing: u32, _landmark_rank: u8) -> bool {
    sample_spacing <= CONTINENTAL_PROXY_MAX_TREE_RECORD_SAMPLE_SPACING
}

fn preview_stream_plans(
    request: ValidatedTerrainPreviewRequest,
) -> Result<Option<Vec<McloneOverworldStreamPlan>>, String> {
    let source = request.request();
    if !source.content_stage.includes_structured_hydrology() || source.sample_spacing > 4 {
        return Ok(None);
    }
    let max_x = request
        .min_x()
        .checked_add(
            i32::try_from(request.footprint_blocks())
                .map_err(|_| "terrain preview footprint exceeds i32")?,
        )
        .ok_or("terrain preview maximum X coordinate overflow")?;
    let max_z = request
        .min_z()
        .checked_add(
            i32::try_from(request.footprint_blocks())
                .map_err(|_| "terrain preview footprint exceeds i32")?,
        )
        .ok_or("terrain preview maximum Z coordinate overflow")?;
    let min_chunk = ChunkPos::new(
        request.min_x().div_euclid(16),
        request.min_z().div_euclid(16),
    );
    let max_chunk = ChunkPos::new(max_x.div_euclid(16), max_z.div_euclid(16));
    let mut cache = McloneOverworldStreamPlanCache::new(source.seed, source.topology);
    cache
        .plans_intersecting_chunks(min_chunk, max_chunk)
        .map(Some)
        .map_err(|error| format!("failed to compile Terrain Lab planned streams: {error}"))
}

const fn biome_recipe_code(recipe: McloneOverworldBiomeRecipe) -> f32 {
    match recipe {
        McloneOverworldBiomeRecipe::Ocean => 0.0,
        McloneOverworldBiomeRecipe::Shore => 1.0,
        McloneOverworldBiomeRecipe::River => 2.0,
        McloneOverworldBiomeRecipe::SnowyAlpine => 3.0,
        McloneOverworldBiomeRecipe::CoolWetConifer => 4.0,
        McloneOverworldBiomeRecipe::WarmDrySteppe => 5.0,
        McloneOverworldBiomeRecipe::TemperateWoodland => 6.0,
        McloneOverworldBiomeRecipe::TemperateMeadow => 7.0,
    }
}

const fn forest_family_code(family: Option<McloneTreeFamily>) -> f32 {
    match family {
        None => 0.0,
        Some(McloneTreeFamily::TemperateBroadleaf) => 1.0,
        Some(McloneTreeFamily::CoolWetConifer) => 2.0,
        Some(McloneTreeFamily::WarmDryAcacia) => 3.0,
    }
}

const fn landform_kind_code(kind: McloneOverworldLandformKind) -> f32 {
    match kind {
        McloneOverworldLandformKind::Ocean => 0.0,
        McloneOverworldLandformKind::Coast => 1.0,
        McloneOverworldLandformKind::River => 2.0,
        McloneOverworldLandformKind::Wetland => 3.0,
        McloneOverworldLandformKind::QuietPlain => 4.0,
        McloneOverworldLandformKind::RollingUpland => 5.0,
        McloneOverworldLandformKind::RidgeValley => 6.0,
        McloneOverworldLandformKind::BroadBasin => 7.0,
        McloneOverworldLandformKind::MountainRange => 8.0,
    }
}

const fn surface_recipe_code(recipe: McloneOverworldSurfaceRecipe) -> f32 {
    match recipe {
        McloneOverworldSurfaceRecipe::OceanFloor => 0.0,
        McloneOverworldSurfaceRecipe::SandyCoast => 1.0,
        McloneOverworldSurfaceRecipe::GravelCoast => 9.0,
        McloneOverworldSurfaceRecipe::RockyCoast => 10.0,
        McloneOverworldSurfaceRecipe::SnowCover => 11.0,
        McloneOverworldSurfaceRecipe::RiverBed => 2.0,
        McloneOverworldSurfaceRecipe::WetlandBed => 3.0,
        McloneOverworldSurfaceRecipe::RiverBank => 4.0,
        McloneOverworldSurfaceRecipe::GrassSoil => 5.0,
        McloneOverworldSurfaceRecipe::ErodedSlope => 6.0,
        McloneOverworldSurfaceRecipe::AlpineSnow => 7.0,
        McloneOverworldSurfaceRecipe::ExposedStone => 8.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewComparison {
    pub sample_count: usize,
    pub max_absolute_surface_error: f32,
    pub mean_absolute_surface_error: f32,
    pub p95_absolute_surface_error: f32,
    pub max_absolute_display_error: f32,
    pub mean_absolute_display_error: f32,
    pub p95_absolute_display_error: f32,
    pub water_presence_agreement: f32,
    pub max_absolute_base_surface_error: f32,
    pub mean_absolute_base_surface_error: f32,
    pub p95_absolute_base_surface_error: f32,
    pub ocean_water_presence_agreement: f32,
    pub mean_absolute_continentalness_error: f32,
    pub mean_absolute_relief_error: f32,
    pub mean_absolute_temperature_error: f32,
    pub mean_absolute_moisture_error: f32,
    pub mean_absolute_ruggedness_error: f32,
    pub macro_surface_material_agreement: f32,
    pub channel_presence_agreement: f32,
    pub mean_absolute_river_signed_distance_error: f32,
    pub mean_absolute_channel_influence_error: f32,
    pub mean_absolute_bank_influence_error: f32,
    pub mean_absolute_wetland_influence_error: f32,
    pub visible_surface_material_agreement: f32,
    pub landform_kind_agreement: f32,
    pub biome_recipe_agreement: f32,
    pub surface_recipe_agreement: f32,
}

impl TerrainPreviewComparison {
    pub fn compare(
        reference: &TerrainPreviewReferenceGrid,
        candidate: &[TerrainPreviewSample],
    ) -> Result<Self, String> {
        Self::compare_samples(reference.samples(), candidate)
    }

    pub fn compare_samples(
        reference: &[TerrainPreviewSample],
        candidate: &[TerrainPreviewSample],
    ) -> Result<Self, String> {
        if candidate.len() != reference.len() {
            return Err(format!(
                "terrain preview candidate has {} samples, reference has {}",
                candidate.len(),
                reference.len()
            ));
        }
        if candidate.is_empty() {
            return Err("terrain preview comparison requires at least one sample".to_owned());
        }

        let mut errors = Vec::with_capacity(candidate.len());
        let mut error_sum = 0.0_f64;
        let mut max_error = 0.0_f32;
        let mut display_errors = Vec::with_capacity(candidate.len());
        let mut display_error_sum = 0.0_f64;
        let mut max_display_error = 0.0_f32;
        let mut water_matches = 0_usize;
        let mut base_errors = Vec::with_capacity(candidate.len());
        let mut base_error_sum = 0.0_f64;
        let mut max_base_error = 0.0_f32;
        let mut ocean_water_matches = 0_usize;
        let mut continentalness_error_sum = 0.0_f64;
        let mut relief_error_sum = 0.0_f64;
        let mut temperature_error_sum = 0.0_f64;
        let mut moisture_error_sum = 0.0_f64;
        let mut ruggedness_error_sum = 0.0_f64;
        let mut macro_material_matches = 0_usize;
        let mut channel_matches = 0_usize;
        let mut river_signed_distance_error_sum = 0.0_f64;
        let mut channel_influence_error_sum = 0.0_f64;
        let mut bank_influence_error_sum = 0.0_f64;
        let mut wetland_influence_error_sum = 0.0_f64;
        let mut visible_material_matches = 0_usize;
        let mut landform_matches = 0_usize;
        let mut biome_matches = 0_usize;
        let mut surface_recipe_matches = 0_usize;
        for (expected, actual) in reference.iter().zip(candidate) {
            if !actual.packed().iter().all(|value| value.is_finite()) {
                return Err("terrain preview candidate contains a non-finite field".to_owned());
            }
            let error = (expected.surface_y - actual.surface_y).abs();
            errors.push(error);
            error_sum += f64::from(error);
            max_error = max_error.max(error);
            let display_error = (expected.display_y - actual.display_y).abs();
            display_errors.push(display_error);
            display_error_sum += f64::from(display_error);
            max_display_error = max_display_error.max(display_error);
            water_matches += usize::from(expected.is_water() == actual.is_water());
            let base_error = (expected.base_surface_y - actual.base_surface_y).abs();
            base_errors.push(base_error);
            base_error_sum += f64::from(base_error);
            max_base_error = max_base_error.max(base_error);
            ocean_water_matches +=
                usize::from(expected.is_ocean_water() == actual.is_ocean_water());
            continentalness_error_sum +=
                f64::from((expected.continentalness - actual.continentalness).abs());
            relief_error_sum += f64::from((expected.relief - actual.relief).abs());
            temperature_error_sum += f64::from((expected.temperature - actual.temperature).abs());
            moisture_error_sum += f64::from((expected.moisture - actual.moisture).abs());
            ruggedness_error_sum += f64::from((expected.ruggedness - actual.ruggedness).abs());
            macro_material_matches +=
                usize::from(expected.macro_surface_material() == actual.macro_surface_material());
            channel_matches += usize::from(expected.is_channel() == actual.is_channel());
            river_signed_distance_error_sum +=
                f64::from((expected.river_signed_distance - actual.river_signed_distance).abs());
            channel_influence_error_sum +=
                f64::from((expected.channel_influence - actual.channel_influence).abs());
            bank_influence_error_sum +=
                f64::from((expected.bank_influence - actual.bank_influence).abs());
            wetland_influence_error_sum +=
                f64::from((expected.wetland_influence - actual.wetland_influence).abs());
            visible_material_matches += usize::from(
                expected.visible_surface_material() == actual.visible_surface_material(),
            );
            landform_matches += usize::from(
                expected.landform_kind.round() as i32 == actual.landform_kind.round() as i32,
            );
            biome_matches += usize::from(
                expected.biome_recipe.round() as i32 == actual.biome_recipe.round() as i32,
            );
            surface_recipe_matches += usize::from(
                expected.surface_recipe.round() as i32 == actual.surface_recipe.round() as i32,
            );
        }
        errors.sort_by(f32::total_cmp);
        display_errors.sort_by(f32::total_cmp);
        base_errors.sort_by(f32::total_cmp);
        let p95_index = ((errors.len() - 1) * 95) / 100;
        let sample_count = candidate.len() as f64;

        Ok(Self {
            sample_count: candidate.len(),
            max_absolute_surface_error: max_error,
            mean_absolute_surface_error: (error_sum / sample_count) as f32,
            p95_absolute_surface_error: errors[p95_index],
            max_absolute_display_error: max_display_error,
            mean_absolute_display_error: (display_error_sum / sample_count) as f32,
            p95_absolute_display_error: display_errors[p95_index],
            water_presence_agreement: water_matches as f32 / candidate.len() as f32,
            max_absolute_base_surface_error: max_base_error,
            mean_absolute_base_surface_error: (base_error_sum / sample_count) as f32,
            p95_absolute_base_surface_error: base_errors[p95_index],
            ocean_water_presence_agreement: ocean_water_matches as f32 / candidate.len() as f32,
            mean_absolute_continentalness_error: (continentalness_error_sum / sample_count) as f32,
            mean_absolute_relief_error: (relief_error_sum / sample_count) as f32,
            mean_absolute_temperature_error: (temperature_error_sum / sample_count) as f32,
            mean_absolute_moisture_error: (moisture_error_sum / sample_count) as f32,
            mean_absolute_ruggedness_error: (ruggedness_error_sum / sample_count) as f32,
            macro_surface_material_agreement: macro_material_matches as f32
                / candidate.len() as f32,
            channel_presence_agreement: channel_matches as f32 / candidate.len() as f32,
            mean_absolute_river_signed_distance_error: (river_signed_distance_error_sum
                / sample_count) as f32,
            mean_absolute_channel_influence_error: (channel_influence_error_sum / sample_count)
                as f32,
            mean_absolute_bank_influence_error: (bank_influence_error_sum / sample_count) as f32,
            mean_absolute_wetland_influence_error: (wetland_influence_error_sum / sample_count)
                as f32,
            visible_surface_material_agreement: visible_material_matches as f32
                / candidate.len() as f32,
            landform_kind_agreement: landform_matches as f32 / candidate.len() as f32,
            biome_recipe_agreement: biome_matches as f32 / candidate.len() as f32,
            surface_recipe_agreement: surface_recipe_matches as f32 / candidate.len() as f32,
        })
    }
}

pub const fn terrain_preview_field_revision() -> &'static str {
    MCLONE_OVERWORLD_FIELD_REVISION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::McloneOverworldStreamPlanner;

    #[test]
    fn surface_quality_defaults_to_basic_and_parses_durable_labels() {
        assert_eq!(
            TerrainPreviewRequest::new(12_345, 0, 0, 32).surface_quality,
            TerrainPreviewSurfaceQuality::Basic
        );
        assert_eq!(
            TerrainPreviewSurfaceQuality::parse_label("inferred").unwrap(),
            TerrainPreviewSurfaceQuality::Inferred
        );
        assert_eq!(
            TerrainPreviewSurfaceQuality::parse_label("cheap").unwrap(),
            TerrainPreviewSurfaceQuality::Basic
        );
        assert!(TerrainPreviewSurfaceQuality::parse_label("sampled").is_err());
    }

    #[test]
    fn centered_request_uses_shared_corner_grid() {
        let request = TerrainPreviewRequest::new(12_345, -64, 96, 16)
            .validate()
            .unwrap();
        assert_eq!(request.footprint_blocks(), 1_024);
        assert_eq!(request.samples_per_axis(), 65);
        assert_eq!(request.sample_count(), 4_225);
        assert_eq!(request.min_x(), -576);
        assert_eq!(request.min_z(), -416);
        assert_eq!(request.world_x(0), Some(-576));
        assert_eq!(request.world_x(32), Some(-64));
        assert_eq!(request.world_x(64), Some(448));
        assert_eq!(request.world_x(65), None);
    }

    #[test]
    fn one_block_spacing_is_available_for_close_diagnostic_views() {
        let request = TerrainPreviewRequest::new(12_345, -64, 96, 1)
            .validate()
            .unwrap();
        assert_eq!(request.footprint_blocks(), 64);
        assert_eq!(request.world_x(64), Some(-32));
    }

    #[test]
    fn rejects_invalid_spacing_cells_and_coordinate_overflow() {
        assert!(TerrainPreviewRequest::new(1, 0, 0, 0).validate().is_err());
        assert!(TerrainPreviewRequest::new(1, 0, 0, 3).validate().is_err());
        assert!(
            TerrainPreviewRequest::new(1, 0, 0, 2_048)
                .validate()
                .is_err()
        );

        let mut request = TerrainPreviewRequest::new(1, 0, 0, 16);
        request.cells_per_axis = 63;
        assert!(request.validate().is_err());
        request.cells_per_axis = 256;
        assert!(request.validate().is_err());

        let request = TerrainPreviewRequest::new(1, i32::MAX, 0, 1_024);
        assert!(request.validate().is_err());
    }

    #[test]
    fn reference_grid_matches_direct_production_samples() {
        let request = TerrainPreviewRequest::new(-98_765, -304, 336, 32);
        let grid = TerrainPreviewReferenceGrid::compile(request).unwrap();
        let validated = request.validate().unwrap();
        let sampler = McloneOverworldSampler::new(request.seed);

        for (sample_x, sample_z) in [(0, 0), (1, 63), (32, 32), (64, 64)] {
            let world_x = validated.world_x(sample_x).unwrap();
            let world_z = validated.world_z(sample_z).unwrap();
            let terrain = sampler.sample(world_x, world_z);
            let sample = grid.sample(sample_x, sample_z).unwrap();
            assert_eq!(sample.surface_y, terrain.surface_y as f32);
            assert_eq!(sample.continentalness, terrain.continentalness as f32);
            assert_eq!(sample.relief, terrain.relief as f32);
            assert_eq!(sample.temperature, terrain.climate.temperature as f32);
            assert_eq!(sample.moisture, terrain.climate.moisture as f32);
            assert_eq!(sample.base_surface_y, terrain.base_surface_y as f32);
            assert_eq!(sample.is_ocean_water(), terrain.continentalness <= 0.0);
        }
        assert_eq!(
            grid.packed_bytes().len(),
            4_225 * TERRAIN_PREVIEW_SAMPLE_FLOATS * std::mem::size_of::<f32>()
        );
    }

    #[test]
    fn reference_grid_preserves_periodic_x_topology() {
        let mut left = TerrainPreviewRequest::new(8_675_309, 0, 0, 64);
        left.topology = McloneOverworldSamplingTopology::PeriodicX;
        let mut right = left;
        right.center_x += crate::levelgen::MCLONE_OVERWORLD_PERIOD_BLOCKS;
        assert_eq!(
            TerrainPreviewReferenceGrid::compile(left)
                .unwrap()
                .samples(),
            TerrainPreviewReferenceGrid::compile(right)
                .unwrap()
                .samples()
        );
    }

    #[test]
    fn vanilla_reference_grid_uses_direct_density_samples() {
        let mut request = TerrainPreviewRequest::new(12_345, -1, -1, 1)
            .with_profile(TerrainPreviewProfile::VanillaOverworld);
        request.cells_per_axis = 8;
        request.content_stage = TerrainPreviewContentStage::Surface;
        let grid = TerrainPreviewReferenceGrid::compile(request).unwrap();
        let validated = request.validate().unwrap();
        let mut sampler = VanillaOverworldLodSampler::new(request.seed);

        for (sample_x, sample_z) in [(0, 0), (1, 7), (4, 4), (8, 8)] {
            let world_x = validated.world_x(sample_x).unwrap();
            let world_z = validated.world_z(sample_z).unwrap();
            let direct = sampler.sample(world_x, world_z);
            let sample = grid.sample(sample_x, sample_z).unwrap();
            assert_eq!(sample.surface_y, direct.solid_surface_y as f32);
            assert_eq!(sample.display_y, direct.display_y as f32);
            assert_eq!(sample.is_water(), direct.water);
            assert_eq!(sample.visible_surface_material(), direct.visible_material);
            assert_eq!(sample.biome_recipe, direct.biome.id() as f32);
        }
        assert_eq!(
            grid.packed_bytes().len(),
            81 * TERRAIN_PREVIEW_SAMPLE_FLOATS * std::mem::size_of::<f32>()
        );
        let packed = grid.packed_f32();
        assert_eq!(
            TerrainPreviewReferenceGrid::from_packed_f32(request, &packed).unwrap(),
            grid
        );
        assert!(
            TerrainPreviewReferenceGrid::from_packed_f32(request, &packed[..packed.len() - 1])
                .is_err()
        );
        let mut non_finite = packed;
        non_finite[0] = f32::NAN;
        assert!(TerrainPreviewReferenceGrid::from_packed_f32(request, &non_finite).is_err());
    }

    #[test]
    fn vanilla_grid_is_identical_across_sampler_cache_boundaries() {
        let mut request = TerrainPreviewRequest::new(-98_765, -304, 336, 4)
            .with_profile(TerrainPreviewProfile::VanillaOverworld);
        request.cells_per_axis = 8;
        request.content_stage = TerrainPreviewContentStage::Surface;
        let cold = TerrainPreviewReferenceGrid::compile(request).unwrap();
        let mut sampler = VanillaOverworldLodSampler::new(request.seed);
        let warm = TerrainPreviewReferenceGrid::compile_with_vanilla_sampler(request, &mut sampler)
            .unwrap();
        let repeated =
            TerrainPreviewReferenceGrid::compile_with_vanilla_sampler(request, &mut sampler)
                .unwrap();

        assert_eq!(cold, warm);
        assert_eq!(warm, repeated);
        assert!(sampler.reused_density_columns() > 0);
    }

    #[test]
    fn vanilla_macro_grid_is_deterministic_and_comparable_to_exact() {
        for (seed, center_x, center_z) in [(12_345, 0, 0), (-98_765, -304, 336)] {
            let mut request = TerrainPreviewRequest::new(seed, center_x, center_z, 32)
                .with_profile(TerrainPreviewProfile::VanillaOverworld);
            request.cells_per_axis = 8;
            request.content_stage = TerrainPreviewContentStage::Surface;
            let mut exact_sampler = VanillaOverworldLodSampler::new(seed);
            let exact = TerrainPreviewReferenceGrid::compile_with_vanilla_sampler(
                request,
                &mut exact_sampler,
            )
            .unwrap();
            let mut macro_sampler = VanillaOverworldMacroSampler::new(seed);
            let macro_grid = TerrainPreviewReferenceGrid::compile_with_vanilla_macro_sampler(
                request,
                &mut macro_sampler,
            )
            .unwrap();
            let repeated = TerrainPreviewReferenceGrid::compile_with_vanilla_macro_sampler(
                request,
                &mut macro_sampler,
            )
            .unwrap();
            let comparison =
                TerrainPreviewComparison::compare(&exact, macro_grid.samples()).unwrap();

            assert_eq!(macro_grid, repeated);
            assert_eq!(comparison.sample_count, 81);
            assert!(comparison.mean_absolute_surface_error.is_finite());
            assert!(comparison.mean_absolute_display_error.is_finite());
            assert!((0.0..=1.0).contains(&comparison.water_presence_agreement));
            assert!(macro_sampler.reused_density_columns() > 0);
        }
    }

    #[test]
    fn structured_near_detail_reconstructs_planned_stream_records() {
        let seed = -98_765;
        let plan =
            McloneOverworldStreamPlanner::new(seed, McloneOverworldSamplingTopology::Unbounded)
                .plans_intersecting_chunks(ChunkPos::new(86, 54), ChunkPos::new(91, 62))
                .unwrap()
                .into_iter()
                .next()
                .expect("review region has a planned stream");
        let node = plan.nodes[plan.nodes.len() / 2];
        let structured = TerrainPreviewReferenceGrid::compile(
            TerrainPreviewRequest::new(seed, node.x, node.z, 1)
                .with_content_stage(TerrainPreviewContentStage::Structured),
        )
        .unwrap();
        assert!(
            structured
                .samples()
                .iter()
                .any(|sample| sample.planned_stream_influence > 0.0)
        );
        let hydrology = TerrainPreviewReferenceGrid::compile(TerrainPreviewRequest::new(
            seed, node.x, node.z, 1,
        ))
        .unwrap();
        assert!(
            hydrology
                .samples()
                .iter()
                .all(|sample| sample.planned_stream_influence == 0.0)
        );
    }

    #[test]
    fn cover_samples_use_production_forest_intent() {
        let request = TerrainPreviewRequest::new(12_345, -80, 48, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let grid = TerrainPreviewReferenceGrid::compile(request).unwrap();
        let samples = grid.samples();

        assert!(
            samples
                .iter()
                .all(|sample| sample.forest_summary_available == 1.0)
        );
        assert!(samples.iter().any(|sample| sample.forest_coverage > 0.5));
        assert!(samples.iter().any(|sample| sample.forest_coverage == 0.0));
        assert!(
            samples
                .iter()
                .filter(|sample| sample.forest_coverage > 0.0)
                .all(|sample| (1.0..=3.0).contains(&sample.forest_family))
        );
    }

    #[test]
    fn cover_compile_work_is_fixed_per_lattice_point() {
        let near = TerrainPreviewRequest::new(12_345, 0, 0, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover)
            .validate()
            .unwrap();
        let coarse = TerrainPreviewRequest::new(12_345, 0, 0, 1_024)
            .with_content_stage(TerrainPreviewContentStage::Cover)
            .validate()
            .unwrap();

        assert_eq!(
            terrain_preview_reference_compile_work(near),
            TerrainPreviewCompileWork {
                sample_lattice_points: 4_225,
                terrain_sample_evaluations: 21_125,
                forest_intent_evaluations: 4_225,
                forest_footprint_summaries: 0,
            }
        );
        assert_eq!(
            terrain_preview_reference_compile_work(coarse),
            TerrainPreviewCompileWork {
                sample_lattice_points: 4_225,
                terrain_sample_evaluations: 21_125,
                forest_intent_evaluations: 16_900,
                forest_footprint_summaries: 4_225,
            }
        );
        assert_eq!(
            terrain_preview_gpu_compile_work(coarse),
            terrain_preview_reference_compile_work(coarse)
        );
        assert_eq!(
            terrain_preview_gpu_compile_work(near),
            TerrainPreviewCompileWork {
                sample_lattice_points: 4_225,
                terrain_sample_evaluations: 4_225,
                forest_intent_evaluations: 4_225,
                forest_footprint_summaries: 0,
            }
        );
    }

    #[test]
    fn coarse_cover_aggregates_without_record_queries() {
        let request = TerrainPreviewRequest::new(12_345, 0, 0, 8)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let source =
            McloneVegetationSource::new(request.seed, McloneOverworldSamplingTopology::Unbounded);
        let mut cache = McloneOverworldVegetationPlanCache::new(source);
        let product =
            TerrainPreviewVegetationProduct::compile_with_cache(request, &mut cache).unwrap();

        assert!(product.summary_available());
        assert!(!product.records_requested());
        assert!(product.records_aggregated());
        assert!(product.occurrences().is_empty());
        assert_eq!(product.cache_report().cell_requests, 0);
        assert_eq!(cache.report().cell_requests, 0);
    }

    #[test]
    fn near_cover_records_use_stable_rank_admission_and_cache() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let mut cache = McloneOverworldVegetationPlanCache::new(source);
        let request = TerrainPreviewRequest::new(12_345, -80, 48, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let first =
            TerrainPreviewVegetationProduct::compile_with_cache(request, &mut cache).unwrap();
        let repeated =
            TerrainPreviewVegetationProduct::compile_with_cache(request, &mut cache).unwrap();

        assert!(first.records_requested());
        assert!(!first.records_aggregated());
        assert!(!first.occurrences().is_empty());
        assert_eq!(first.occurrences(), repeated.occurrences());
        assert!(
            first
                .occurrences()
                .iter()
                .all(|occurrence| occurrence.record.landmark_rank == 3)
        );
        assert!(repeated.cache_report().cell_hits > 0);
        assert_eq!(
            repeated.cache_report().cell_requests,
            repeated.cache_report().cell_hits + repeated.cache_report().cell_misses
        );
    }

    #[test]
    fn record_admission_is_global_and_monotonic() {
        for rank in 0..=3 {
            assert!(terrain_preview_tree_record_admitted(1, rank));
            assert_eq!(terrain_preview_tree_record_admitted(2, rank), rank >= 2);
            assert_eq!(terrain_preview_tree_record_admitted(4, rank), rank >= 3);
            assert!(!terrain_preview_tree_record_admitted(8, rank));
        }
    }

    #[test]
    fn comparison_reports_height_and_water_disagreement() {
        let request = TerrainPreviewRequest::new(12_345, 0, 0, 1_024);
        let reference = TerrainPreviewReferenceGrid::compile(request).unwrap();
        let mut candidate = reference.samples().to_vec();
        candidate[0].surface_y += 10.0;
        candidate[1].surface_y -= 2.0;
        candidate[0].display_y += 7.0;
        candidate[1].display_y -= 3.0;
        candidate[2].water = if candidate[2].is_water() { 0.0 } else { 1.0 };
        candidate[3].base_surface_y += 4.0;
        candidate[4].ocean_water = if candidate[4].is_ocean_water() {
            0.0
        } else {
            1.0
        };
        candidate[5].continentalness += 0.25;

        let report = TerrainPreviewComparison::compare(&reference, &candidate).unwrap();
        assert_eq!(report.sample_count, 4_225);
        assert_eq!(report.max_absolute_surface_error, 10.0);
        assert!(report.mean_absolute_surface_error > 0.0);
        assert_eq!(report.p95_absolute_surface_error, 0.0);
        assert_eq!(report.max_absolute_display_error, 7.0);
        assert!(report.mean_absolute_display_error > 0.0);
        assert_eq!(report.p95_absolute_display_error, 0.0);
        assert_eq!(
            report.water_presence_agreement,
            (report.sample_count - 1) as f32 / report.sample_count as f32
        );
        assert_eq!(report.max_absolute_base_surface_error, 4.0);
        assert!(report.mean_absolute_base_surface_error > 0.0);
        assert_eq!(
            report.ocean_water_presence_agreement,
            (report.sample_count - 1) as f32 / report.sample_count as f32
        );
        assert!(report.mean_absolute_continentalness_error > 0.0);
    }

    #[test]
    fn exposes_production_field_and_reference_schema_revisions() {
        assert_eq!(
            terrain_preview_field_revision(),
            MCLONE_OVERWORLD_FIELD_REVISION
        );
        assert_eq!(
            TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
            "mclone-terrain-preview-reference-grid-v10"
        );
        assert_eq!(
            TerrainPreviewProfile::VanillaOverworld.source_revision(),
            VANILLA_OVERWORLD_LOD_REVISION
        );
        assert_eq!(
            TerrainPreviewProfile::ContinentalEcoregionCandidate.source_revision(),
            CONTINENTAL_SURFACE_SCHEMA_REVISION
        );
    }

    #[test]
    fn continental_candidate_compiles_one_grid_and_stitched_height_halo() {
        let mut request = TerrainPreviewRequest::new(12_345, -4_096, 8_192, 512)
            .with_profile(TerrainPreviewProfile::ContinentalEcoregionCandidate)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        request.cells_per_axis = 8;
        let (grid, height_halo) =
            TerrainPreviewReferenceGrid::compile_continental_with_height_halo(request, 2).unwrap();
        assert_eq!(grid.samples().len(), 81);
        assert_eq!(height_halo.len(), 13 * 13);
        assert!(
            grid.samples()
                .iter()
                .all(|sample| sample.packed().iter().all(|value| value.is_finite()))
        );
        let validated = grid.request();
        let direct = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(12_345))
            .unwrap()
            .query_point(validated.world_x(0).unwrap(), validated.world_z(0).unwrap())
            .sample;
        assert_eq!(grid.sample(0, 0).unwrap().surface_y, direct.solid_surface_y);
        assert_eq!(
            grid.sample(0, 0).unwrap().display_y,
            direct.display_surface_y
        );
        assert_eq!(
            grid.compile_work(),
            TerrainPreviewCompileWork {
                sample_lattice_points: 81,
                terrain_sample_evaluations: 81,
                forest_intent_evaluations: 0,
                forest_footprint_summaries: 0,
            }
        );
    }

    #[test]
    fn continental_proxy_vegetation_is_partition_independent_and_source_owned() {
        let candidate_request = |center_x, center_z, cells_per_axis| TerrainPreviewRequest {
            profile: TerrainPreviewProfile::ContinentalEcoregionCandidate,
            center_x,
            center_z,
            cells_per_axis,
            content_stage: TerrainPreviewContentStage::Cover,
            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
            ..TerrainPreviewRequest::new(12_345, center_x, center_z, 1)
        };
        let whole =
            TerrainPreviewVegetationProduct::compile(candidate_request(-13_824, 9_216, 128))
                .unwrap();
        let mut partitioned = Vec::new();
        for center_z in [9_184, 9_248] {
            for center_x in [-13_856, -13_792] {
                partitioned.extend(
                    TerrainPreviewVegetationProduct::compile(candidate_request(
                        center_x, center_z, 64,
                    ))
                    .unwrap()
                    .occurrences()
                    .iter()
                    .copied(),
                );
            }
        }
        partitioned.sort_unstable();

        assert!(whole.summary_available());
        assert!(whole.records_requested());
        assert!(!whole.occurrences().is_empty());
        assert_eq!(whole.occurrences(), partitioned);
        assert_eq!(
            whole.cache_report(),
            McloneVegetationPlanCacheReport::default()
        );
        assert!(whole.occurrences().iter().all(|occurrence| {
            occurrence.record.id.vegetation_revision == CONTINENTAL_PROXY_VEGETATION_REVISION
        }));
    }

    #[test]
    fn continental_proxy_vegetation_keeps_one_lattice_across_review_levels() {
        let request = |sample_spacing, cells_per_axis| TerrainPreviewRequest {
            profile: TerrainPreviewProfile::ContinentalEcoregionCandidate,
            center_x: -13_824,
            center_z: 9_216,
            sample_spacing,
            cells_per_axis,
            content_stage: TerrainPreviewContentStage::Cover,
            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
            ..TerrainPreviewRequest::new(12_345, -13_824, 9_216, sample_spacing)
        };
        let fine = TerrainPreviewVegetationProduct::compile(request(4, 64)).unwrap();
        let middle = TerrainPreviewVegetationProduct::compile(request(8, 32)).unwrap();
        let far = TerrainPreviewVegetationProduct::compile(request(16, 16)).unwrap();

        for product in [&fine, &middle, &far] {
            assert!(product.records_requested());
            assert!(!product.records_aggregated());
            assert!(!product.occurrences().is_empty());
        }
        assert_eq!(fine.occurrences(), middle.occurrences());
        assert_eq!(middle.occurrences(), far.occurrences());
        assert_eq!(
            terrain_preview_max_tree_record_sample_spacing(
                TerrainPreviewProfile::ContinentalEcoregionCandidate,
            ),
            16
        );
        assert_eq!(
            terrain_preview_max_tree_record_sample_spacing(
                TerrainPreviewProfile::McloneOverworldV1,
            ),
            4
        );
    }
}
