use crate::{
    BoundedRepresentationOwnershipSnapshot, McloneTreeOccurrenceId,
    TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE, TerrainClipmap, TerrainClipmapConfig,
    TerrainExactCoverageMode, TerrainHorizonFrameStats, TerrainHorizonPresentation,
    TerrainHorizonRenderTarget, TerrainHorizonRenderer, TerrainPreparedExactFrame,
    TerrainPreviewMaterialAtlas, TerrainVegetationExecutor, TerrainViewSourceIdentity,
};
use mclone_render_color::{RenderColorProfile, RenderTargetColorTransform};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TerrainPreviewContentStage, TerrainPreviewProfile,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainViewEngineConfig {
    pub width: u32,
    pub height: u32,
    pub source: TerrainViewSourceIdentity,
    pub clipmap: TerrainClipmapConfig,
    /// Number of computed sample cells covered by one rendered mesh cell.
    ///
    /// This changes presentation density only; residency, source identity,
    /// exact coverage, and bounded feature ownership remain unchanged.
    pub render_cell_stride: u32,
    pub vegetation_enabled: bool,
    pub color_profile: RenderColorProfile,
}

impl TerrainViewEngineConfig {
    pub fn validated(mut self) -> Result<Self, String> {
        self.width = self.width.max(1);
        self.height = self.height.max(1);
        let procedural = self.source.composition_source()?;
        if procedural.profile != TerrainPreviewProfile::McloneOverworldV1 {
            return Err(
                "the shared procedural horizon currently requires mclone-overworld-v1".to_owned(),
            );
        }
        self.clipmap = TerrainClipmap::new(self.clipmap)?.config();
        if self.render_cell_stride == 0
            || !self.render_cell_stride.is_power_of_two()
            || TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS % self.render_cell_stride != 0
        {
            return Err(format!(
                "terrain render cell stride {} must be a nonzero power-of-two \
                 divisor of {}",
                self.render_cell_stride, TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
            ));
        }
        if self.render_cell_stride > TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE {
            return Err(format!(
                "terrain render cell stride {} exceeds the proven geometry/normal \
                 transition contract {}; add stride-aware stitching and halo \
                 evidence before enabling it",
                self.render_cell_stride, TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE,
            ));
        }
        Ok(self)
    }
}

/// Shared terrain representation engine used by detached and live hosts.
///
/// Camera/navigation policy and exact-data acquisition remain outside this
/// owner. The engine commits procedural residency, source-qualified exact
/// coverage, bounded feature ownership, and one prepared draw submission for
/// any host-provided render view.
pub struct TerrainViewEngine {
    renderer: TerrainHorizonRenderer,
    config: TerrainViewEngineConfig,
    color_format: wgpu::TextureFormat,
    target_color_transform: RenderTargetColorTransform,
    center: [i32; 2],
    content_stage: TerrainPreviewContentStage,
    last_stats: Option<TerrainHorizonFrameStats>,
}

impl TerrainViewEngine {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: TerrainViewEngineConfig,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self, String> {
        let config = config.validated()?;
        if config.vegetation_enabled != vegetation_executor.is_some() {
            return Err(
                "terrain-view vegetation enablement must match executor availability".to_owned(),
            );
        }
        let target_color_transform = config.color_profile.target_color_transform(color_format);
        let renderer = TerrainHorizonRenderer::new_with_target_color_transform_and_cell_stride(
            device,
            queue,
            color_format,
            config.width,
            config.height,
            material_atlas,
            config.clipmap,
            config.render_cell_stride,
            vegetation_executor,
            target_color_transform,
        )?;
        let mut engine = Self {
            renderer,
            config,
            color_format,
            target_color_transform,
            center: [0, 0],
            content_stage: TerrainPreviewContentStage::Cover,
            last_stats: None,
        };
        engine.replan();
        Ok(engine)
    }

    pub const fn source(&self) -> TerrainViewSourceIdentity {
        self.config.source
    }

    pub fn replace_source(&mut self, source: TerrainViewSourceIdentity) -> Result<bool, String> {
        let mut next = self.config;
        next.source = source;
        let next = next.validated()?;
        if next.source == self.config.source {
            return Ok(false);
        }
        self.config.source = next.source;
        self.renderer.reset_source();
        self.last_stats = None;
        self.replan();
        Ok(true)
    }

    pub const fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    pub const fn target_color_transform(&self) -> RenderTargetColorTransform {
        self.target_color_transform
    }

    pub const fn config(&self) -> TerrainViewEngineConfig {
        self.config
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.renderer.resize(device, width, height);
        self.config.width = width;
        self.config.height = height;
    }

    pub fn set_residency(
        &mut self,
        center_x: i32,
        center_z: i32,
        content_stage: TerrainPreviewContentStage,
    ) {
        let next = [center_x, center_z];
        if self.center == next && self.content_stage == content_stage {
            return;
        }
        self.center = next;
        self.content_stage = content_stage;
        self.replan();
    }

    fn replan(&mut self) {
        let source = self
            .config
            .source
            .composition_source()
            .expect("validated terrain-view engine source remains reconstructible");
        self.renderer.set_view(
            source.seed,
            self.center[0],
            self.center[1],
            self.content_stage,
        );
    }

    fn apply_exact_and_ownership(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        exact: Option<(&TerrainPreparedExactFrame, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<(), String> {
        if let Some((exact, mode)) = exact {
            validate_prepared_exact(self.config.source, exact)?;
            self.renderer
                .set_exact_painted_coverage(queue, exact.coverage(), mode)?;
        } else {
            self.renderer.clear_exact_painted_coverage();
        }
        if let Some(ownership) = tree_ownership {
            self.renderer.set_tree_ownership(device, queue, ownership)?;
        } else if !self.renderer.authoritative_tree_ownership() {
            self.renderer.clear_tree_ownership(device, queue)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        presentation: TerrainHorizonPresentation,
        exact: Option<(&TerrainPreparedExactFrame, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.apply_exact_and_ownership(device, queue, exact, tree_ownership)?;
        let stats = self.renderer.encode(
            device,
            queue,
            encoder,
            color_view,
            self.config.width,
            self.config.height,
            presentation,
        )?;
        self.last_stats = Some(stats);
        Ok(stats)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        presentation: TerrainHorizonPresentation,
        exact: Option<(&TerrainPreparedExactFrame, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.apply_exact_and_ownership(device, queue, exact, tree_ownership)?;
        let stats = self.renderer.encode_to_target(
            device,
            queue,
            encoder,
            target,
            self.config.width,
            self.config.height,
            presentation,
        )?;
        self.last_stats = Some(stats);
        Ok(stats)
    }

    pub const fn last_stats(&self) -> Option<TerrainHorizonFrameStats> {
        self.last_stats
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        self.renderer
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.renderer.set_depth_capture_enabled(enabled);
    }

    /// Let live exact coverage select complete natural-feature records.
    ///
    /// Exact-ready columns own both a present feature and an authoritative
    /// edited absence. Detached hosts continue supplying their explicit
    /// canonical ownership snapshot instead.
    pub fn set_authoritative_tree_ownership(&mut self, enabled: bool) {
        self.renderer.set_authoritative_tree_ownership(enabled);
    }

    pub fn shutdown(&mut self) {
        self.renderer.shutdown_vegetation();
    }

    pub fn shutdown_complete(&self) -> bool {
        self.renderer.vegetation_shutdown_complete()
    }
}

fn validate_prepared_exact(
    source: TerrainViewSourceIdentity,
    exact: &TerrainPreparedExactFrame,
) -> Result<(), String> {
    if exact.source() != source {
        return Err(
            "prepared exact frame does not match the active terrain-view engine source".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mclone_core::HorizontalTopology;
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    use super::*;
    use crate::{ExactPaintedCoverageSnapshot, TerrainCompositionSourceIdentity};

    fn source(seed: i64, generation: u64) -> TerrainViewSourceIdentity {
        TerrainViewSourceIdentity::detached(
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, seed),
            HorizontalTopology::UNBOUNDED,
            generation,
            1,
        )
        .unwrap()
    }

    #[test]
    fn engine_config_requires_a_reconstructible_mclone_source() {
        let hidden = TerrainViewSourceIdentity::new(
            None,
            crate::TerrainViewTruthRole::BoundedObserver,
            HorizontalTopology::UNBOUNDED,
            1,
            1,
        )
        .unwrap();
        let error = TerrainViewEngineConfig {
            width: 0,
            height: 0,
            source: hidden,
            clipmap: TerrainClipmapConfig::default(),
            render_cell_stride: 1,
            vegetation_enabled: false,
            color_profile: RenderColorProfile::Vanilla,
        }
        .validated()
        .unwrap_err();
        assert!(error.contains("does not disclose"));
    }

    #[test]
    fn engine_config_rejects_unproven_render_cell_strides() {
        let config_for_stride = |render_cell_stride| TerrainViewEngineConfig {
            width: 1,
            height: 1,
            source: source(12_345, 1),
            clipmap: TerrainClipmapConfig::default(),
            render_cell_stride,
            vegetation_enabled: false,
            color_profile: RenderColorProfile::Vanilla,
        };
        assert!(config_for_stride(1).validated().is_ok());
        for stride in [0, 2, 3, 4, 8, 16, 32, 64, 128] {
            assert!(config_for_stride(stride).validated().is_err());
        }
        assert!(
            config_for_stride(8)
                .validated()
                .unwrap_err()
                .contains("proven geometry/normal transition contract")
        );
    }

    #[test]
    fn engine_rejects_an_exact_frame_from_another_generation() {
        let active = source(12_345, 2);
        let retired = source(12_345, 1);
        let coverage =
            ExactPaintedCoverageSnapshot::new(retired.composition_source().unwrap(), 3, [])
                .unwrap();
        let frame = TerrainPreparedExactFrame::new(retired, coverage).unwrap();
        assert_eq!(
            validate_prepared_exact(active, &frame).unwrap_err(),
            "prepared exact frame does not match the active terrain-view engine source"
        );
    }
}
