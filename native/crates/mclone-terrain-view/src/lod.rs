use mclone_core::TerrainLodPreset;
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING, TERRAIN_PREVIEW_MIN_SAMPLE_SPACING,
};

use crate::{
    TERRAIN_CLIPMAP_DEFAULT_TILES_PER_AXIS, TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE,
    TerrainClipmapConfig,
};

pub const TERRAIN_LOD_LOW_LEVEL_COUNT: u32 = 6;
pub const TERRAIN_LOD_MEDIUM_LEVEL_COUNT: u32 = 8;
pub const TERRAIN_LOD_HIGH_LEVEL_COUNT: u32 = 10;

/// One platform-independent bounded quality descriptor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainLodPresetDescriptor {
    pub preset: TerrainLodPreset,
    pub clipmap: Option<TerrainClipmapConfig>,
    pub render_cell_stride: u32,
    pub vegetation_max_sample_spacing: Option<u32>,
}

impl TerrainLodPresetDescriptor {
    pub const fn for_preset(preset: TerrainLodPreset) -> Self {
        let (level_count, vegetation_max_sample_spacing) = match preset {
            TerrainLodPreset::Off => (0, None),
            TerrainLodPreset::Low => (TERRAIN_LOD_LOW_LEVEL_COUNT, Some(1)),
            TerrainLodPreset::Medium => (TERRAIN_LOD_MEDIUM_LEVEL_COUNT, Some(2)),
            TerrainLodPreset::High => (
                TERRAIN_LOD_HIGH_LEVEL_COUNT,
                Some(TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING),
            ),
        };
        Self {
            preset,
            clipmap: if level_count == 0 {
                None
            } else {
                Some(TerrainClipmapConfig {
                    level_count,
                    tiles_per_axis: TERRAIN_CLIPMAP_DEFAULT_TILES_PER_AXIS,
                    base_sample_spacing: TERRAIN_PREVIEW_MIN_SAMPLE_SPACING,
                })
            },
            render_cell_stride: 1,
            vegetation_max_sample_spacing,
        }
    }

    pub fn validated(self) -> Result<Self, String> {
        if self.preset == TerrainLodPreset::Off {
            if self.clipmap.is_some() || self.vegetation_max_sample_spacing.is_some() {
                return Err("terrain LOD Off must allocate no horizon or vegetation".to_owned());
            }
            return Ok(self);
        }
        let clipmap = self
            .clipmap
            .ok_or("enabled terrain LOD preset requires a clipmap")?
            .validate()?;
        if self.render_cell_stride == 0
            || !self.render_cell_stride.is_power_of_two()
            || self.render_cell_stride > TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE
        {
            return Err(format!(
                "terrain LOD render stride {} exceeds the proven bound {}",
                self.render_cell_stride, TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE
            ));
        }
        let vegetation_max_sample_spacing = self
            .vegetation_max_sample_spacing
            .ok_or("enabled terrain LOD preset requires a proxy-vegetation bound")?;
        if !vegetation_max_sample_spacing.is_power_of_two()
            || vegetation_max_sample_spacing < clipmap.base_sample_spacing
            || vegetation_max_sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
        {
            return Err(format!(
                "terrain LOD vegetation spacing {vegetation_max_sample_spacing} must be a power of two from {} through {}",
                clipmap.base_sample_spacing, TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
            ));
        }
        Ok(Self {
            clipmap: Some(clipmap),
            ..self
        })
    }

    pub const fn allocation_slots(self) -> u32 {
        match self.clipmap {
            Some(clipmap) => clipmap.allocation_slots(),
            None => 0,
        }
    }

    pub fn terrain_visibility_distance(self) -> f32 {
        self.clipmap
            .map_or(0.0, |clipmap| clipmap.conservative_view_distance_blocks())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_presets_have_stable_bounded_descriptors() {
        let off = TerrainLodPresetDescriptor::for_preset(TerrainLodPreset::Off);
        assert_eq!(off.allocation_slots(), 0);
        assert_eq!(off.terrain_visibility_distance(), 0.0);
        assert!(off.validated().is_ok());

        let low = TerrainLodPresetDescriptor::for_preset(TerrainLodPreset::Low)
            .validated()
            .unwrap();
        let medium = TerrainLodPresetDescriptor::for_preset(TerrainLodPreset::Medium)
            .validated()
            .unwrap();
        let high = TerrainLodPresetDescriptor::for_preset(TerrainLodPreset::High)
            .validated()
            .unwrap();
        assert_eq!(low.allocation_slots(), 96);
        assert_eq!(medium.allocation_slots(), 128);
        assert_eq!(high.allocation_slots(), 160);
        assert!(low.terrain_visibility_distance() < medium.terrain_visibility_distance());
        assert!(medium.terrain_visibility_distance() < high.terrain_visibility_distance());
        assert_eq!(high.clipmap, Some(TerrainClipmapConfig::default()));
        assert_eq!(low.render_cell_stride, 1);
        assert_eq!(medium.render_cell_stride, 1);
        assert_eq!(high.render_cell_stride, 1);
    }
}
