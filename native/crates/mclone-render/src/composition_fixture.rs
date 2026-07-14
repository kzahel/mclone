//! Shared renderer acceptance fixture for complementary world half-spaces.
//!
//! This is intentionally renderer-owned rather than native- or browser-owned:
//! platform validation lanes provide only a target and presentation cadence.

use anyhow::Result;
use glam::{Vec3, vec3};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilitySet,
};

use crate::chunk::{
    ChunkCamera, ChunkMultiviewRenderTarget, ChunkRenderTarget, ChunkRenderView, ChunkTextureAtlas,
    PlacedTexturedSectionRenderer, PreparedTexturedSectionStereoDraw, TexturedSectionDrawResources,
    TexturedSectionRenderOptions, TexturedSectionRenderStats,
};
use crate::placement::{
    CompositionClip, CompositionHalfSpace, WorldCompositionContext, WorldPlacement,
};
use crate::uniform::PerViewSlot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ComplementaryHalfSpaceTerrainReport {
    pub left: TexturedSectionRenderStats,
    pub right: TexturedSectionRenderStats,
    pub left_translucent_section_count: usize,
    pub right_translucent_section_count: usize,
}

pub struct ComplementaryHalfSpaceTerrainStereo {
    left: PreparedTexturedSectionStereoDraw,
    right: PreparedTexturedSectionStereoDraw,
}

impl ComplementaryHalfSpaceTerrainStereo {
    pub fn stats(&self) -> [[TexturedSectionRenderStats; 2]; 2] {
        [self.left.stats(), self.right.stats()]
    }
}

/// Two independent terrain stores rendered through identity placements and
/// complementary `x=0` composition half-spaces.
pub struct ComplementaryHalfSpaceTerrainFixture {
    left: TexturedSectionDrawResources,
    right: TexturedSectionDrawResources,
    left_renderer: PlacedTexturedSectionRenderer,
    right_renderer: PlacedTexturedSectionRenderer,
    left_context: WorldCompositionContext,
    right_context: WorldCompositionContext,
    options: TexturedSectionRenderOptions,
}

impl ComplementaryHalfSpaceTerrainFixture {
    pub fn clear_color() -> wgpu::Color {
        wgpu::Color {
            r: 0.02,
            g: 0.025,
            b: 0.03,
            a: 1.0,
        }
    }

    pub fn render_view(size: [u32; 2], eye_offset: f32) -> ChunkRenderView {
        ChunkCamera {
            eye: [eye_offset, 4.5, 18.0],
            target: [0.0, 2.5, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 55.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 100.0,
        }
        .render_view(size[0], size[1])
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let atlas_rgba = [255, 255, 255, 255];
        let atlas = ChunkTextureAtlas {
            width: 1,
            height: 1,
            rgba: &atlas_rgba,
        };
        let left_sections = [fixture_section(FixturePalette::LEFT)];
        let right_sections = [fixture_section(FixturePalette::RIGHT)];
        let left =
            TexturedSectionDrawResources::new(device, queue, color_format, &left_sections, atlas)?;
        let right = TexturedSectionDrawResources::new_with_shared_resources(
            device,
            queue,
            &right_sections,
            left.shared_resources(),
        )?;
        let left_renderer = left.create_placed_renderer(device);
        let right_renderer = right.create_placed_renderer(device);
        let left_context = WorldCompositionContext::new(
            WorldPlacement::identity(),
            None,
            CompositionClip::HalfSpace(CompositionHalfSpace::new(-Vec3::X, 0.0)?),
        );
        let right_context = WorldCompositionContext::new(
            WorldPlacement::identity(),
            None,
            CompositionClip::HalfSpace(CompositionHalfSpace::new(Vec3::X, 0.0)?),
        );
        let mut options = TexturedSectionRenderOptions::default();
        options.force_fullbright = true;
        options.section_occlusion_culling = false;
        Ok(Self {
            left,
            right,
            left_renderer,
            right_renderer,
            left_context,
            right_context,
            options,
        })
    }

    pub fn shares_immutable_resources(&self) -> bool {
        self.left.shares_immutable_resources_with(&self.right)
    }

    pub fn clipped_renderer_materialized(&self) -> bool {
        self.left_renderer.clipped_renderer_materialized()
            && self.right_renderer.clipped_renderer_materialized()
    }

    pub fn clipped_multiview_renderer_materialized(&self) -> bool {
        self.left_renderer.clipped_multiview_renderer_materialized()
            && self
                .right_renderer
                .clipped_multiview_renderer_materialized()
    }

    pub fn render_mono(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<ComplementaryHalfSpaceTerrainReport> {
        let left_records = self
            .left
            .prepare_render_records_for_context(self.left_context);
        let right_records = self
            .right
            .prepare_render_records_for_context(self.right_context);
        let left = self.left.render_placed_prepared_with_options_in_slot(
            &self.left_renderer,
            device,
            &left_records,
            queue,
            encoder,
            target,
            render_view,
            self.options,
            self.left_context,
            view_slot,
        )?;
        let loaded = target.with_loaded_color().with_loaded_depth();
        let right = self.right.render_placed_prepared_with_options_in_slot(
            &self.right_renderer,
            device,
            &right_records,
            queue,
            encoder,
            loaded,
            render_view,
            self.options,
            self.right_context,
            view_slot,
        )?;
        let left_translucent = self.left.prepare_placed_translucent_records(
            &left_records,
            render_view,
            self.options,
            self.left_context,
        );
        let right_translucent = self.right.prepare_placed_translucent_records(
            &right_records,
            render_view,
            self.options,
            self.right_context,
        );
        let left_keys = left_translucent
            .iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        let right_keys = right_translucent
            .iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        self.left
            .render_ordered_placed_translucent_sections_in_slot(
                &self.left_renderer,
                device,
                &left_keys,
                None,
                queue,
                encoder,
                loaded,
                render_view,
                self.options,
                self.left_context,
                view_slot,
            );
        self.right
            .render_ordered_placed_translucent_sections_in_slot(
                &self.right_renderer,
                device,
                &right_keys,
                None,
                queue,
                encoder,
                loaded,
                render_view,
                self.options,
                self.right_context,
                view_slot,
            );
        Ok(ComplementaryHalfSpaceTerrainReport {
            left,
            right,
            left_translucent_section_count: left_keys.len(),
            right_translucent_section_count: right_keys.len(),
        })
    }

    pub fn prepare_stereo(
        &self,
        render_views: [ChunkRenderView; 2],
    ) -> ComplementaryHalfSpaceTerrainStereo {
        let left_records = self
            .left
            .prepare_render_records_for_context(self.left_context);
        let right_records = self
            .right
            .prepare_render_records_for_context(self.right_context);
        ComplementaryHalfSpaceTerrainStereo {
            left: self.left.prepare_placed_stereo_draw(
                &left_records,
                render_views,
                [self.options; 2],
                self.left_context,
            ),
            right: self.right.prepare_placed_stereo_draw(
                &right_records,
                render_views,
                [self.options; 2],
                self.right_context,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_prepared_stereo_eye(
        &self,
        prepared: &ComplementaryHalfSpaceTerrainStereo,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<()> {
        self.left
            .render_placed_prepared_stereo_draw_with_options_in_slot(
                &self.left_renderer,
                device,
                &prepared.left,
                queue,
                encoder,
                target,
                render_view,
                self.options,
                self.left_context,
                view_slot,
            )?;
        let loaded = target.with_loaded_color().with_loaded_depth();
        self.right
            .render_placed_prepared_stereo_draw_with_options_in_slot(
                &self.right_renderer,
                device,
                &prepared.right,
                queue,
                encoder,
                loaded,
                render_view,
                self.options,
                self.right_context,
                view_slot,
            )?;
        let left_keys = prepared
            .left
            .translucent_records(self.left_context.placement())
            .into_iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        let right_keys = prepared
            .right
            .translucent_records(self.right_context.placement())
            .into_iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        self.left
            .render_ordered_placed_translucent_sections_in_slot(
                &self.left_renderer,
                device,
                &left_keys,
                Some(&prepared.left),
                queue,
                encoder,
                loaded,
                render_view,
                self.options,
                self.left_context,
                view_slot,
            );
        self.right
            .render_ordered_placed_translucent_sections_in_slot(
                &self.right_renderer,
                device,
                &right_keys,
                Some(&prepared.right),
                queue,
                encoder,
                loaded,
                render_view,
                self.options,
                self.right_context,
                view_slot,
            );
        Ok(())
    }

    pub fn render_prepared_multiview(
        &self,
        prepared: &ComplementaryHalfSpaceTerrainStereo,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
    ) -> Result<()> {
        self.left
            .render_placed_prepared_multiview_stereo_draw_with_options(
                &self.left_renderer,
                &prepared.left,
                device,
                queue,
                encoder,
                target,
                render_views,
                [self.options; 2],
                self.left_context,
            )?;
        let loaded = target.with_loaded_color().with_loaded_depth();
        self.right
            .render_placed_prepared_multiview_stereo_draw_with_options(
                &self.right_renderer,
                &prepared.right,
                device,
                queue,
                encoder,
                loaded,
                render_views,
                [self.options; 2],
                self.right_context,
            )?;
        let left_keys = prepared
            .left
            .translucent_records(self.left_context.placement())
            .into_iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        let right_keys = prepared
            .right
            .translucent_records(self.right_context.placement())
            .into_iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        self.left
            .render_ordered_placed_translucent_sections_multiview(
                &self.left_renderer,
                &left_keys,
                device,
                queue,
                encoder,
                loaded,
                render_views,
                [self.options; 2],
                self.left_context,
            )?;
        self.right
            .render_ordered_placed_translucent_sections_multiview(
                &self.right_renderer,
                &right_keys,
                device,
                queue,
                encoder,
                loaded,
                render_views,
                [self.options; 2],
                self.right_context,
            )?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct FixturePalette {
    solid: [f32; 4],
    cutout: [f32; 4],
    translucent: [f32; 4],
}

impl FixturePalette {
    const LEFT: Self = Self {
        solid: [0.88, 0.18, 0.04, 1.0],
        cutout: [1.0, 0.72, 0.08, 1.0],
        translucent: [0.95, 0.20, 0.05, 0.45],
    };
    const RIGHT: Self = Self {
        solid: [0.04, 0.30, 0.92, 1.0],
        cutout: [0.08, 0.85, 0.95, 1.0],
        translucent: [0.05, 0.38, 1.0, 0.45],
    };
}

fn fixture_section(palette: FixturePalette) -> TexturedRenderSectionMesh {
    let mut mesh = TexturedVisibleChunkMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        solid_index_count: 0,
        opaque_index_count: 0,
    };

    // Floor and back-wall panels leave a deliberately authored doorway at
    // the x=0 seam. The later boundary-face milestone is not implicated.
    append_cube(
        &mut mesh,
        vec3(-10.0, -0.5, -4.0),
        vec3(10.0, 0.0, 2.0),
        palette.solid,
    );
    append_cube(
        &mut mesh,
        vec3(-10.0, 0.0, -0.3),
        vec3(-2.0, 7.0, 0.0),
        palette.solid,
    );
    append_cube(
        &mut mesh,
        vec3(2.0, 0.0, -0.3),
        vec3(10.0, 7.0, 0.0),
        palette.solid,
    );
    mesh.solid_index_count = mesh.indices.len() as u32;

    // A cutout-phase frame surrounds the open seam.
    append_cube(
        &mut mesh,
        vec3(-2.0, 0.0, -0.6),
        vec3(-1.55, 6.0, 0.1),
        palette.cutout,
    );
    append_cube(
        &mut mesh,
        vec3(1.55, 0.0, -0.6),
        vec3(2.0, 6.0, 0.1),
        palette.cutout,
    );
    append_cube(
        &mut mesh,
        vec3(-2.0, 5.55, -0.6),
        vec3(2.0, 6.0, 0.1),
        palette.cutout,
    );
    mesh.opaque_index_count = mesh.indices.len() as u32;

    // Two translucent panes exercise the clipped blend pipeline without
    // covering the open doorway.
    append_cube(
        &mut mesh,
        vec3(-8.5, 1.0, 0.15),
        vec3(-3.0, 5.0, 0.25),
        palette.translucent,
    );
    append_cube(
        &mut mesh,
        vec3(3.0, 1.0, 0.15),
        vec3(8.5, 5.0, 0.25),
        palette.translucent,
    );

    TexturedRenderSectionMesh {
        key: RenderSectionKey::new(0, 0, 0),
        mesh,
        visibility: VisibilitySet::all_visible(),
    }
}

fn append_cube(mesh: &mut TexturedVisibleChunkMesh, min: Vec3, max: Vec3, color: [f32; 4]) {
    let faces = [
        [
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
        ],
        [
            [max.x, min.y, min.z],
            [min.x, min.y, min.z],
            [min.x, max.y, min.z],
            [max.x, max.y, min.z],
        ],
        [
            [max.x, min.y, max.z],
            [max.x, min.y, min.z],
            [max.x, max.y, min.z],
            [max.x, max.y, max.z],
        ],
        [
            [min.x, min.y, min.z],
            [min.x, min.y, max.z],
            [min.x, max.y, max.z],
            [min.x, max.y, min.z],
        ],
        [
            [min.x, max.y, max.z],
            [max.x, max.y, max.z],
            [max.x, max.y, min.z],
            [min.x, max.y, min.z],
        ],
        [
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [max.x, min.y, max.z],
            [min.x, min.y, max.z],
        ],
    ];
    for face in faces {
        let base = mesh.vertices.len() as u32;
        mesh.vertices
            .extend(face.map(|position| TexturedChunkVertex {
                position,
                uv: [0.5, 0.5],
                color,
                packed_light: 15_728_880,
            }));
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
