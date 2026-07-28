//! Shared renderer acceptance fixture for placed and clipped actors.
//!
//! Native and browser lanes supply only targets. Actor lists, source bounds,
//! placements, complementary clipping, animation, and lighting remain one
//! shared Rust fixture.

use std::sync::Arc;

use anyhow::Result;
use glam::{Vec3, vec3};
use mclone_assets::{
    MemoryAssetSource, chicken_figure_id, chicken_figure_path, cow_figure_id, cow_figure_path,
    default_player_figure_path, upright_bear_figure_path,
};
use mclone_core::Vec3d;

use crate::asset_lab_figure::load_first_party_actor_figures;
use crate::chunk::{
    ChunkMultiviewRenderTarget, ChunkRenderTarget, ChunkRenderView, TexturedSectionRenderOptions,
};
use crate::composition_fixture::{
    ComplementaryHalfSpaceTerrainFixture, ComplementaryHalfSpaceTerrainReport,
    ComplementaryHalfSpaceTerrainStereo,
};
use crate::entity::{
    ActorDrawResourceSnapshot, ActorDrawResources, ActorInstance, ActorInstanceId,
    ActorRenderStats, ActorSharedResources, ActorTextureAtlas, ActorTextureLayout,
    ActorTextureRegion,
};
use crate::placement::{
    CompositionClip, CompositionHalfSpace, WorldCompositionContext, WorldPlacement,
    WorldSourceBounds,
};
use crate::target::RenderFrameTarget;
use crate::uniform::PerViewSlot;

const LEFT_VISIBLE_ACTOR_COUNT: usize = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorCompositionFixtureReport {
    pub terrain: ComplementaryHalfSpaceTerrainReport,
    pub unbounded: ActorRenderStats,
    pub left: ActorRenderStats,
    pub right: ActorRenderStats,
    pub left_resources: ActorDrawResourceSnapshot,
    pub right_resources: ActorDrawResourceSnapshot,
}

/// Two compatible per-world actor states placed from distant source anchors
/// into the renderer-owned complementary-half-space terrain fixture.
pub struct ActorCompositionFixture {
    terrain: ComplementaryHalfSpaceTerrainFixture,
    left_resources: ActorDrawResources,
    right_resources: ActorDrawResources,
    left_context: WorldCompositionContext,
    left_unbounded_context: WorldCompositionContext,
    right_context: WorldCompositionContext,
    left_actors: Vec<ActorInstance>,
    right_actors: Vec<ActorInstance>,
    options: TexturedSectionRenderOptions,
}

impl ActorCompositionFixture {
    pub fn clear_color() -> wgpu::Color {
        ComplementaryHalfSpaceTerrainFixture::clear_color()
    }

    pub fn render_view(size: [u32; 2], eye_offset: f32) -> ChunkRenderView {
        ComplementaryHalfSpaceTerrainFixture::render_view(size, eye_offset)
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let terrain = ComplementaryHalfSpaceTerrainFixture::new(device, queue, color_format)?;
        let (atlas_rgba, atlas_layout) = actor_fixture_atlas();
        let figures = actor_fixture_figures()?;
        let shared = ActorSharedResources::new(
            device,
            queue,
            color_format,
            ActorTextureAtlas {
                width: 65,
                height: 32,
                rgba: &atlas_rgba,
                layout: atlas_layout,
            },
            Some(&figures),
        )?;
        let left_resources =
            ActorDrawResources::new_with_shared_resources(device, Arc::clone(&shared));
        let right_resources = ActorDrawResources::new_with_shared_resources(device, shared);

        let left_placement = WorldPlacement::new(
            Vec3d::new(1_000.0, 64.0, 1_000.0),
            Vec3d::new(-4.0, 0.0, 0.0),
            0.75,
        )?;
        let right_placement = WorldPlacement::new(
            Vec3d::new(-1_000.0, 64.0, -1_000.0),
            Vec3d::new(4.0, 0.0, 0.0),
            0.75,
        )?;
        let left_bounds = WorldSourceBounds::new(
            Vec3d::new(980.0, 60.0, 980.0),
            Vec3d::new(1_020.0, 140.0, 1_020.0),
        )?;
        let right_bounds = WorldSourceBounds::new(
            Vec3d::new(-1_020.0, 60.0, -1_020.0),
            Vec3d::new(-980.0, 140.0, -980.0),
        )?;
        let left_clip = CompositionClip::HalfSpace(CompositionHalfSpace::new(-Vec3::X, 0.0)?);
        let right_clip = CompositionClip::HalfSpace(CompositionHalfSpace::new(Vec3::X, 0.0)?);
        let left_context =
            WorldCompositionContext::new(left_placement, Some(left_bounds), left_clip);
        let left_unbounded_context =
            WorldCompositionContext::unbounded(left_placement, Some(left_bounds));
        let right_context =
            WorldCompositionContext::new(right_placement, Some(right_bounds), right_clip);

        let left_actors = vec![
            ActorInstance::remote_player_with_figure(
                vec3(997.5, 64.0, 1_000.0),
                25.0,
                cow_figure_id(),
            )
            .with_dimensions(0.9, 1.4)
            .with_id(ActorInstanceId::Entity(9))
            .with_walk_animation_distance(0.31),
            ActorInstance::item_egg(vec3(1_000.0, 64.0, 1_000.0), -20.0, 0.3, 0.3)
                .with_packed_light(0),
            ActorInstance::remote_player(vec3(1_004.7, 64.0, 1_000.0), -35.0)
                .with_walk_animation_distance(0.42),
            // Source-membership rejection witness.
            ActorInstance::debug_cube(vec3(1_200.0, 64.0, 1_000.0), 0.0, 0.0, None, 1.0, 1.0),
            // Entirely outside the left half-space after placement.
            ActorInstance::debug_cube(vec3(1_008.0, 64.0, 1_000.0), 0.0, 0.0, None, 1.0, 1.0),
            // Inside source bounds and the half-space, but above the frustum.
            ActorInstance::debug_cube(vec3(995.0, 125.0, 1_000.0), 0.0, 0.0, None, 1.0, 1.0),
        ];
        let right_actors = vec![
            ActorInstance::remote_player_with_figure(
                vec3(-1_002.0, 64.0, -1_000.0),
                20.0,
                chicken_figure_id(),
            )
            .with_dimensions(0.4, 0.7)
            .with_id(ActorInstanceId::Entity(10))
            .with_walk_animation_distance(0.36)
            .with_chicken_wing_flap_radians(Some(0.48)),
            ActorInstance::remote_player(vec3(-998.0, 64.0, -1_000.0), -18.0)
                .with_id(ActorInstanceId::Entity(11))
                .with_walk_animation_distance(0.51),
            ActorInstance::local_player(vec3(-1_005.0, 64.0, -1_000.0), 42.0)
                .with_walk_animation_distance(0.28),
        ];
        let options = TexturedSectionRenderOptions {
            force_fullbright: false,
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };
        Ok(Self {
            terrain,
            left_resources,
            right_resources,
            left_context,
            left_unbounded_context,
            right_context,
            left_actors,
            right_actors,
            options,
        })
    }

    pub fn shares_immutable_resources(&self) -> bool {
        self.left_resources
            .shares_immutable_resources_with(&self.right_resources)
    }

    pub fn prepare_stereo(
        &self,
        render_views: [ChunkRenderView; 2],
    ) -> ComplementaryHalfSpaceTerrainStereo {
        self.terrain.prepare_stereo(render_views)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_mono(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<ActorCompositionFixtureReport> {
        let terrain =
            self.terrain
                .render_mono(device, queue, encoder, target, render_view, view_slot)?;
        let target = actor_frame_target(target);
        let unbounded = self.left_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.left_actors[..LEFT_VISIBLE_ACTOR_COUNT],
            self.left_unbounded_context,
            view_slot,
        )?;
        let left = self.left_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.left_actors,
            self.left_context,
            view_slot,
        )?;
        let right = self.right_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.right_actors,
            self.right_context,
            view_slot,
        )?;
        Ok(ActorCompositionFixtureReport {
            terrain,
            unbounded,
            left,
            right,
            left_resources: self.left_resources.resource_snapshot(),
            right_resources: self.right_resources.resource_snapshot(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_stereo_eye(
        &mut self,
        prepared: &ComplementaryHalfSpaceTerrainStereo,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<ActorCompositionFixtureReport> {
        self.terrain.render_prepared_stereo_eye(
            prepared,
            device,
            queue,
            encoder,
            target,
            render_view,
            view_slot,
        )?;
        let target = actor_frame_target(target);
        let unbounded = self.left_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.left_actors[..LEFT_VISIBLE_ACTOR_COUNT],
            self.left_unbounded_context,
            view_slot,
        )?;
        let left = self.left_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.left_actors,
            self.left_context,
            view_slot,
        )?;
        let right = self.right_resources.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            &self.right_actors,
            self.right_context,
            view_slot,
        )?;
        Ok(ActorCompositionFixtureReport {
            unbounded,
            left,
            right,
            left_resources: self.left_resources.resource_snapshot(),
            right_resources: self.right_resources.resource_snapshot(),
            ..ActorCompositionFixtureReport::default()
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_multiview(
        &mut self,
        prepared: &ComplementaryHalfSpaceTerrainStereo,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
    ) -> Result<ActorCompositionFixtureReport> {
        self.terrain.render_prepared_multiview(
            prepared,
            device,
            queue,
            encoder,
            target,
            render_views,
        )?;
        let target = actor_multiview_frame_target(target);
        let unbounded = self.left_resources.render_composed_multiview(
            device,
            queue,
            encoder,
            target,
            render_views,
            [self.options; 2],
            &self.left_actors[..LEFT_VISIBLE_ACTOR_COUNT],
            self.left_unbounded_context,
        )?;
        let left = self.left_resources.render_composed_multiview(
            device,
            queue,
            encoder,
            target,
            render_views,
            [self.options; 2],
            &self.left_actors,
            self.left_context,
        )?;
        let right = self.right_resources.render_composed_multiview(
            device,
            queue,
            encoder,
            target,
            render_views,
            [self.options; 2],
            &self.right_actors,
            self.right_context,
        )?;
        Ok(ActorCompositionFixtureReport {
            unbounded,
            left,
            right,
            left_resources: self.left_resources.resource_snapshot(),
            right_resources: self.right_resources.resource_snapshot(),
            ..ActorCompositionFixtureReport::default()
        })
    }
}

fn actor_frame_target(target: ChunkRenderTarget<'_>) -> RenderFrameTarget<'_> {
    RenderFrameTarget {
        color_view: target.color_view,
        depth_view: Some(target.depth_view),
        size: target.size,
        gpu_timestamps: target.gpu_timestamps,
    }
}

fn actor_multiview_frame_target(target: ChunkMultiviewRenderTarget<'_>) -> RenderFrameTarget<'_> {
    RenderFrameTarget {
        color_view: target.color_view,
        depth_view: Some(target.depth_view),
        size: target.size,
        gpu_timestamps: target.gpu_timestamps,
    }
}

fn actor_fixture_figures() -> Result<crate::entity::ActorFigureSet> {
    let mut source = MemoryAssetSource::new();
    source.insert_text(
        default_player_figure_path(),
        include_str!("../../../../assets/mclone/figures/player.figure.json"),
    );
    source.insert_text(
        upright_bear_figure_path(),
        include_str!("../../../../assets/mclone/figures/upright_bear.figure.json"),
    );
    source.insert_text(
        chicken_figure_path(),
        include_str!("../../../../assets/mclone/figures/chicken.figure.json"),
    );
    source.insert_text(
        cow_figure_path(),
        include_str!("../../../../assets/mclone/figures/cow.figure.json"),
    );
    load_first_party_actor_figures(&source)
}

fn actor_fixture_atlas() -> (Vec<u8>, ActorTextureLayout) {
    let width = 65u32;
    let height = 32u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    rgba[..4].copy_from_slice(&[255, 255, 255, 255]);
    for y in 0..height {
        for x in 1..width {
            let offset = ((y * width + x) * 4) as usize;
            let light = ((x / 8 + y / 8) & 1) == 0;
            rgba[offset..offset + 4].copy_from_slice(if light {
                &[126, 82, 52, 255]
            } else {
                &[238, 224, 201, 255]
            });
        }
    }
    (
        rgba,
        ActorTextureLayout {
            white: ActorTextureRegion {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            cow: ActorTextureRegion {
                x: 1,
                y: 0,
                width: 64,
                height: 32,
            },
        },
    )
}
