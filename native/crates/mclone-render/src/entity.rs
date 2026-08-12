use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result, bail};
use glam::{EulerRot, Mat4, Quat, Vec3};
use mclone_assets::{ActorFigureId, default_player_figure_id};
use mclone_core::Vec3d;
use mclone_diagnostics::GpuPassId;

use crate::asset_lab_figure::{CompiledFigureClip, CompiledFigureTransform};
use crate::chunk::{
    ChunkRenderView, DEPTH_FORMAT, TexturedSectionRenderOptions, render_view_aabb_visible,
};
use crate::light_texture::FULL_BRIGHT;
use crate::placement::{CompositionClip, WorldCompositionContext};
use crate::prepared_actor::{
    PreparedActorDrawResources, PreparedActorDrawSnapshot, PreparedActorSharedResources,
    PreparedActorSharedSnapshot,
};
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

pub use crate::asset_lab_figure::{ActorFigureSet, CompiledFigure as ActorFigure};

const ACTOR_VERTEX_BYTE_LEN: usize = 3 * std::mem::size_of::<f32>()
    + 2 * std::mem::size_of::<f32>()
    + 4 * std::mem::size_of::<f32>()
    + std::mem::size_of::<u32>();
const ACTOR_VERTEX_BYTE_SIZE: wgpu::BufferAddress = ACTOR_VERTEX_BYTE_LEN as wgpu::BufferAddress;
const UNIFORM_FLOAT_COUNT: usize = 32;
const UNIFORM_BYTE_LEN: usize = UNIFORM_FLOAT_COUNT * std::mem::size_of::<f32>();
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_BYTE_LEN: usize = UNIFORM_BYTE_LEN * 2;
const MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const PLACED_UNIFORM_FLOAT_COUNT: usize = UNIFORM_FLOAT_COUNT + 12;
const PLACED_UNIFORM_BYTE_LEN: usize = PLACED_UNIFORM_FLOAT_COUNT * std::mem::size_of::<f32>();
const PLACED_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    PLACED_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const PLACED_MULTIVIEW_UNIFORM_BYTE_LEN: usize = PLACED_UNIFORM_BYTE_LEN * 2;
const PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    PLACED_MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const ACTOR_MESH_MIN_VERTEX_CAPACITY: usize = 8192;
const ACTOR_MESH_MIN_INDEX_CAPACITY: usize = 12_288;
const ACTOR_VERTEX_BUFFER_MIN_BYTE_SIZE: wgpu::BufferAddress =
    (ACTOR_MESH_MIN_VERTEX_CAPACITY * ACTOR_VERTEX_BYTE_LEN) as wgpu::BufferAddress;
const ACTOR_INDEX_BUFFER_MIN_BYTE_SIZE: wgpu::BufferAddress =
    (ACTOR_MESH_MIN_INDEX_CAPACITY * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
const MODEL_PIXEL_SCALE: f32 = 1.0 / 16.0;
const MODEL_FEET_Y_PIXELS: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorInstance {
    /// Stable source identity. Prepared actor records must never use the
    /// transient position in an input vector as identity.
    pub id: Option<ActorInstanceId>,
    pub feet_position: Vec3,
    /// Native world yaw in radians. Local actor +Z is the forward/front side.
    pub yaw_radians: f32,
    pub pitch_radians: f32,
    pub rotation_pivot: Vec3,
    pub orientation: Option<Quat>,
    pub shape: ActorInstanceShape,
    pub first_person_body_only: bool,
    pub width: f32,
    pub height: f32,
    pub body_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub packed_light: u32,
    pub opacity: f32,
    pub animation: Option<ActorAnimation>,
    pub chicken_wing_flap_radians: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ActorInstanceId {
    LocalPlayer,
    RemotePlayer(u64),
    Entity(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorInstanceShape {
    Figure(ActorFigureId),
    Humanoid,
    QuadrupedPlaceholder,
    CowModel,
    DebugCube,
    ItemEgg,
    MallardNest,
    MallardFeather,
    MallardTrack,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorAnimation {
    pub clip: ActorAnimationClip,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorAnimationClip {
    Walk,
}

impl ActorInstance {
    pub fn local_player(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self::local_player_with_figure(feet_position, y_rot_degrees, default_player_figure_id())
    }

    pub fn local_player_with_figure(
        feet_position: Vec3,
        y_rot_degrees: f32,
        figure: ActorFigureId,
    ) -> Self {
        Self {
            id: Some(ActorInstanceId::LocalPlayer),
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::Figure(figure),
            first_person_body_only: false,
            width: 0.6,
            height: 1.8,
            body_color: [0.18, 0.38, 0.82, 1.0],
            accent_color: [0.92, 0.70, 0.54, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn remote_player(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::Figure(default_player_figure_id()),
            first_person_body_only: false,
            width: 0.6,
            height: 1.8,
            body_color: [0.10, 0.58, 0.68, 1.0],
            accent_color: [0.95, 0.80, 0.24, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn remote_player_with_figure(
        feet_position: Vec3,
        y_rot_degrees: f32,
        figure: ActorFigureId,
    ) -> Self {
        Self {
            shape: ActorInstanceShape::Figure(figure),
            ..Self::remote_player(feet_position, y_rot_degrees)
        }
    }

    pub fn cow_placeholder(
        feet_position: Vec3,
        y_rot_degrees: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.33, 0.19, 0.10, 1.0],
            accent_color: [0.92, 0.86, 0.74, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn cow_model(feet_position: Vec3, y_rot_degrees: f32, width: f32, height: f32) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::CowModel,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.28, 0.17, 0.10, 1.0],
            accent_color: [0.90, 0.86, 0.72, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn chicken_placeholder(
        feet_position: Vec3,
        y_rot_degrees: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.92, 0.90, 0.82, 1.0],
            accent_color: [0.92, 0.18, 0.12, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn debug_cube(
        feet_position: Vec3,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        orientation: Option<Quat>,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: x_rot_degrees.to_radians(),
            rotation_pivot: Vec3::new(0.0, height.max(0.1) * 0.5, 0.0),
            orientation: orientation.map(|rotation| rotation.normalize()),
            shape: ActorInstanceShape::DebugCube,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.13, 0.48, 0.72, 1.0],
            accent_color: [0.95, 0.78, 0.22, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn item_egg(feet_position: Vec3, y_rot_degrees: f32, width: f32, height: f32) -> Self {
        Self {
            id: None,
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::ItemEgg,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.92, 0.89, 0.78, 1.0],
            accent_color: [0.74, 0.58, 0.24, 1.0],
            packed_light: FULL_BRIGHT,
            opacity: 1.0,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn mallard_nest(feet_position: Vec3, y_rot_degrees: f32, width: f32, height: f32) -> Self {
        Self {
            shape: ActorInstanceShape::MallardNest,
            body_color: [0.31, 0.20, 0.09, 1.0],
            accent_color: [0.84, 0.75, 0.49, 1.0],
            ..Self::item_egg(feet_position, y_rot_degrees, width, height)
        }
    }

    pub fn mallard_feather(
        feet_position: Vec3,
        y_rot_degrees: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            shape: ActorInstanceShape::MallardFeather,
            body_color: [0.73, 0.70, 0.62, 1.0],
            accent_color: [0.16, 0.34, 0.62, 1.0],
            ..Self::item_egg(feet_position, y_rot_degrees, width, height)
        }
    }

    pub fn mallard_track(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self {
            shape: ActorInstanceShape::MallardTrack,
            body_color: [0.20, 0.13, 0.06, 0.72],
            accent_color: [0.12, 0.08, 0.04, 0.72],
            ..Self::item_egg(feet_position, y_rot_degrees, 0.34, 0.015)
        }
    }

    pub fn with_packed_light(mut self, packed_light: u32) -> Self {
        self.packed_light = packed_light;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        if opacity.is_finite() {
            self.opacity = opacity.clamp(0.0, 1.0);
        }
        self
    }

    pub fn with_id(mut self, id: ActorInstanceId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_first_person_body_only(mut self, first_person_body_only: bool) -> Self {
        self.first_person_body_only = first_person_body_only;
        self
    }

    pub fn with_dimensions(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_walk_animation_distance(mut self, distance: f32) -> Self {
        if distance.is_finite() {
            self.animation = Some(ActorAnimation {
                clip: ActorAnimationClip::Walk,
                distance,
            });
        }
        self
    }

    pub fn with_chicken_wing_flap_radians(mut self, radians: Option<f32>) -> Self {
        self.chicken_wing_flap_radians = radians.filter(|radians| radians.is_finite());
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorRenderStats {
    pub submitted_actor_count: usize,
    pub drawn_actor_count: usize,
    pub source_rejected_actor_count: usize,
    pub clip_rejected_actor_count: usize,
    pub frustum_rejected_actor_count: usize,
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ActorTextureAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub layout: ActorTextureLayout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorTextureLayout {
    pub white: ActorTextureRegion,
    pub cow: ActorTextureRegion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorTextureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Device/asset-epoch actor topology shared by every compatible world slot.
///
/// This owns only immutable texture, figure, layout, shader, and pipeline
/// state. Per-world uniforms, actor lists, mesh staging, and GPU mesh buffers
/// live in `ActorDrawResources` so equal actor ids cannot alias across worlds.
pub struct ActorSharedResources {
    renderer: ActorRenderer,
    atlas: GpuActorTextureAtlas,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: ActorFigureSet,
    prepared: PreparedActorSharedResources,
}

/// Mutable actor presentation state for exactly one drawable world slot.
pub struct ActorDrawResources {
    shared: Arc<ActorSharedResources>,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    multiview: RefCell<Option<ActorMultiviewDrawState>>,
    placed: RefCell<Option<ActorPlacedDrawState>>,
    placed_multiview: RefCell<Option<ActorPlacedMultiviewDrawState>>,
    composed_actor_scratch: Vec<ActorInstance>,
    mesh_cache: ActorMeshCache,
    prepared: PreparedActorDrawResources,
}

/// Read-only ownership and allocation facts for one actor renderer.
///
/// Tactical 179 uses this snapshot to distinguish immutable topology from the
/// mutable mesh cache before that ownership is split across world slots. It is
/// diagnostics only: ordinary frame rendering does not construct it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorDrawResourceSnapshot {
    pub shared_strong_owner_count: usize,
    /// Known retained bytes in the immutable shared atlas. Opaque driver
    /// allocations for pipelines/layouts and figure collection overhead are
    /// intentionally not guessed.
    pub shared_known_retained_bytes: usize,
    /// Counted CPU staging/cache, GPU mesh capacity, and uniform bytes owned by
    /// this one world state.
    pub mutable_state_allocated_bytes: u64,
    pub atlas_size: [u32; 2],
    pub atlas_base_bytes: usize,
    pub figure_count: usize,
    pub direct_pipeline_count: usize,
    pub direct_uniform_payload_bytes: u64,
    pub direct_uniform_slot_bytes: u64,
    pub direct_uniform_slot_count: u32,
    pub direct_uniform_allocated_bytes: u64,
    pub multiview_pipeline_count: usize,
    pub multiview_uniform_allocated_bytes: u64,
    pub placed_pipeline_count: usize,
    pub clipped_placed_pipeline_count: usize,
    pub placed_uniform_allocated_bytes: u64,
    pub placed_multiview_pipeline_count: usize,
    pub clipped_placed_multiview_pipeline_count: usize,
    pub placed_multiview_uniform_allocated_bytes: u64,
    pub mesh: ActorMeshCacheSnapshot,
    pub prepared_shared: PreparedActorSharedSnapshot,
    pub prepared_world: PreparedActorDrawSnapshot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorMeshCacheSnapshot {
    pub cached_actor_count: usize,
    pub rebuild_count: u64,
    pub upload_count: u64,
    pub last_uploaded_bytes: u64,
    pub total_uploaded_bytes: u64,
    pub cpu_vertex_capacity_bytes: usize,
    pub cpu_index_capacity_bytes: usize,
    pub cpu_vertex_staging_capacity_bytes: usize,
    pub cpu_index_staging_capacity_bytes: usize,
    pub gpu_vertex_capacity_bytes: u64,
    pub gpu_index_capacity_bytes: u64,
}

impl ActorSharedResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        atlas: ActorTextureAtlas<'_>,
        actor_figures: Option<&ActorFigureSet>,
    ) -> Result<Arc<Self>> {
        let renderer = ActorRenderer::new(device, color_format);
        let gpu_atlas =
            GpuActorTextureAtlas::new(device, queue, &renderer.texture_bind_group_layout, atlas)?;
        let actor_figures = actor_figures.cloned().unwrap_or_default();
        let prepared =
            PreparedActorSharedResources::new(device, queue, color_format, &actor_figures)?;
        Ok(Arc::new(Self {
            renderer,
            atlas: gpu_atlas,
            texture_layout: atlas.layout,
            atlas_size: [atlas.width.max(1), atlas.height.max(1)],
            actor_figures,
            prepared,
        }))
    }
}

impl ActorDrawResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        atlas: ActorTextureAtlas<'_>,
        actor_figures: Option<&ActorFigureSet>,
    ) -> Result<Self> {
        let shared = ActorSharedResources::new(device, queue, color_format, atlas, actor_figures)?;
        Ok(Self::new_with_shared_resources(device, shared))
    }

    pub fn new_with_shared_resources(
        device: &wgpu::Device,
        shared: Arc<ActorSharedResources>,
    ) -> Self {
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_actor_uniforms",
            UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_bind_group"),
            layout: &shared.renderer.uniform_bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let prepared = PreparedActorDrawResources::new(device, &shared.prepared);
        Self {
            shared,
            uniforms,
            bind_group,
            multiview: RefCell::new(None),
            placed: RefCell::new(None),
            placed_multiview: RefCell::new(None),
            composed_actor_scratch: Vec::new(),
            mesh_cache: ActorMeshCache::new(device),
            prepared,
        }
    }

    pub fn shared_resources(&self) -> Arc<ActorSharedResources> {
        Arc::clone(&self.shared)
    }

    pub fn shares_immutable_resources_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    /// Materialize the opt-in placed topology before a world slot advertises
    /// composed-actor capability. Direct one-world construction remains lazy.
    pub fn ensure_composed_topology(&mut self, device: &wgpu::Device) {
        let renderer = self.shared.renderer.placed_renderer(device);
        if self.placed.borrow().is_none() {
            *self.placed.borrow_mut() = Some(ActorPlacedDrawState::new(
                device,
                &renderer.uniform_bind_group_layout,
            ));
        }
        if device.features().contains(wgpu::Features::MULTIVIEW) {
            let renderer = self
                .shared
                .renderer
                .placed_multiview_renderer(device)
                .expect("MULTIVIEW feature checked before actor topology materialization");
            if self.placed_multiview.borrow().is_none() {
                *self.placed_multiview.borrow_mut() = Some(ActorPlacedMultiviewDrawState::new(
                    device,
                    &renderer.uniform_bind_group_layout,
                ));
            }
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
    ) -> Result<ActorRenderStats> {
        self.render_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            render_options,
            actors,
            SINGLE_VIEW_SLOT,
        )
    }

    /// Prepare renderer-owned actor state once for a frame that may render
    /// multiple views. Callers must pass the same actor slice to
    /// `render_reusing_prepared_in_slot`; a count mismatch refreshes safely.
    pub fn prepare_for_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actors: &[ActorInstance],
    ) {
        self.prepared
            .prepare(device, queue, &self.shared.prepared, actors);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        view_slot: PerViewSlot,
    ) -> Result<ActorRenderStats> {
        self.render_in_slot_with_preparation(
            device,
            queue,
            encoder,
            target,
            render_view,
            render_options,
            actors,
            view_slot,
            true,
        )
    }

    /// Draw another view from actor inputs prepared earlier in the same shared
    /// frame. A count mismatch safely refreshes preparation rather than drawing
    /// stale storage.
    #[allow(clippy::too_many_arguments)]
    pub fn render_reusing_prepared_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        view_slot: PerViewSlot,
    ) -> Result<ActorRenderStats> {
        self.render_in_slot_with_preparation(
            device,
            queue,
            encoder,
            target,
            render_view,
            render_options,
            actors,
            view_slot,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_in_slot_with_preparation(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        view_slot: PerViewSlot,
        refresh_prepared: bool,
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        if refresh_prepared || self.prepared.prepared_input_count() != actors.len() {
            self.prepared
                .prepare(device, queue, &self.shared.prepared, actors);
        }
        let mut stats = ActorRenderStats {
            submitted_actor_count: actors.len(),
            ..ActorRenderStats::default()
        };
        let legacy_actors = self.prepared.legacy_actors();
        if !legacy_actors.is_empty() {
            let depth_view = target
                .depth_view
                .context("actor render pass requires a depth attachment")?;
            if let Some(prepared) = self.mesh_cache.prepare(
                device,
                queue,
                legacy_actors,
                self.shared.texture_layout,
                self.shared.atlas_size,
                &self.shared.actor_figures,
            ) {
                let uniform_offset = self.uniforms.write_slot(
                    queue,
                    view_slot,
                    &uniform_bytes(render_view, render_options),
                );
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_actor_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
                    ..Default::default()
                });
                pass.set_pipeline(&self.shared.renderer.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[uniform_offset]);
                pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..prepared.vertex_byte_len));
                pass.set_index_buffer(
                    prepared.index_buffer.slice(..prepared.index_byte_len),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..prepared.index_count, 0, 0..1);
                stats.drawn_actor_count = legacy_actors.len();
                stats.vertex_count = prepared.vertex_count;
                stats.index_count = prepared.index_count;
            }
        }
        let prepared_stats = self.prepared.render_in_slot(
            queue,
            encoder,
            target,
            render_view,
            render_options,
            None,
            view_slot,
            &self.shared.prepared,
        )?;
        stats.drawn_actor_count = stats
            .drawn_actor_count
            .saturating_add(prepared_stats.drawn_actor_count);
        stats.vertex_count = stats
            .vertex_count
            .saturating_add(prepared_stats.vertex_count);
        stats.index_count = stats.index_count.saturating_add(prepared_stats.index_count);
        Ok(stats)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_composed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        physical_render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        context: WorldCompositionContext,
    ) -> Result<ActorRenderStats> {
        self.render_composed_in_slot(
            device,
            queue,
            encoder,
            target,
            physical_render_view,
            render_options,
            actors,
            context,
            SINGLE_VIEW_SLOT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_composed_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        physical_render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<ActorRenderStats> {
        self.render_composed_in_slot_with_preparation(
            device,
            queue,
            encoder,
            target,
            physical_render_view,
            render_options,
            actors,
            context,
            view_slot,
            true,
        )
    }

    /// Draw another composed view from actor inputs prepared earlier in the
    /// same shared frame.
    #[allow(clippy::too_many_arguments)]
    pub fn render_composed_reusing_prepared_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        physical_render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<ActorRenderStats> {
        self.render_composed_in_slot_with_preparation(
            device,
            queue,
            encoder,
            target,
            physical_render_view,
            render_options,
            actors,
            context,
            view_slot,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_composed_in_slot_with_preparation(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        physical_render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
        refresh_prepared: bool,
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let selection = select_composed_actors(
            &mut self.composed_actor_scratch,
            actors,
            context,
            &[physical_render_view],
        );
        let mut stats = selection.render_stats(actors.len());
        if self.composed_actor_scratch.is_empty() {
            return Ok(stats);
        }
        if refresh_prepared
            || self.prepared.prepared_input_count() != self.composed_actor_scratch.len()
        {
            self.prepared.prepare(
                device,
                queue,
                &self.shared.prepared,
                &self.composed_actor_scratch,
            );
        }
        let legacy_actors = self.prepared.legacy_actors();
        if !legacy_actors.is_empty() {
            let depth_view = target
                .depth_view
                .context("placed actor render pass requires a depth attachment")?;
            if let Some(prepared) = self.mesh_cache.prepare(
                device,
                queue,
                legacy_actors,
                self.shared.texture_layout,
                self.shared.atlas_size,
                &self.shared.actor_figures,
            ) {
                let renderer = self.shared.renderer.placed_renderer(device);
                if self.placed.borrow().is_none() {
                    *self.placed.borrow_mut() = Some(ActorPlacedDrawState::new(
                        device,
                        &renderer.uniform_bind_group_layout,
                    ));
                }
                let placed = self.placed.borrow();
                let placed = placed
                    .as_ref()
                    .expect("placed actor draw state initialized above");
                let uniform_offset = placed.uniforms.write_slot(
                    queue,
                    view_slot,
                    &placed_uniform_bytes(physical_render_view, render_options, context),
                );
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_actor_placed_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
                    ..Default::default()
                });
                pass.set_pipeline(renderer.pipeline(context.clip()));
                pass.set_bind_group(0, &placed.bind_group, &[uniform_offset]);
                pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..prepared.vertex_byte_len));
                pass.set_index_buffer(
                    prepared.index_buffer.slice(..prepared.index_byte_len),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..prepared.index_count, 0, 0..1);
                stats.drawn_actor_count = legacy_actors.len();
                stats.vertex_count = prepared.vertex_count;
                stats.index_count = prepared.index_count;
            }
        }
        let prepared_stats = self.prepared.render_in_slot(
            queue,
            encoder,
            target,
            physical_render_view,
            render_options,
            Some(context),
            view_slot,
            &self.shared.prepared,
        )?;
        stats.drawn_actor_count = stats
            .drawn_actor_count
            .saturating_add(prepared_stats.drawn_actor_count);
        stats.vertex_count = stats
            .vertex_count
            .saturating_add(prepared_stats.vertex_count);
        stats.index_count = stats.index_count.saturating_add(prepared_stats.index_count);
        Ok(stats)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
        actors: &[ActorInstance],
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        self.prepared
            .prepare(device, queue, &self.shared.prepared, actors);
        let mut stats = ActorRenderStats {
            submitted_actor_count: actors.len(),
            ..ActorRenderStats::default()
        };
        let legacy_actors = self.prepared.legacy_actors();
        if !legacy_actors.is_empty() {
            let depth_view = target
                .depth_view
                .context("actor multiview render pass requires a depth attachment")?;
            if let Some(prepared) = self.mesh_cache.prepare(
                device,
                queue,
                legacy_actors,
                self.shared.texture_layout,
                self.shared.atlas_size,
                &self.shared.actor_figures,
            ) {
                let renderer = self.shared.renderer.multiview_renderer(device)?;
                if self.multiview.borrow().is_none() {
                    *self.multiview.borrow_mut() = Some(ActorMultiviewDrawState::new(
                        device,
                        &renderer.uniform_bind_group_layout,
                    ));
                }
                let multiview = self.multiview.borrow();
                let multiview = multiview
                    .as_ref()
                    .expect("actor multiview draw state initialized above");
                multiview.write_uniforms(queue, render_views, render_options);
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_actor_multiview_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
                    ..Default::default()
                });
                pass.set_pipeline(&renderer.pipeline);
                pass.set_bind_group(0, &multiview.bind_group, &[]);
                pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..prepared.vertex_byte_len));
                pass.set_index_buffer(
                    prepared.index_buffer.slice(..prepared.index_byte_len),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..prepared.index_count, 0, 0..1);
                stats.drawn_actor_count = legacy_actors.len();
                stats.vertex_count = prepared.vertex_count;
                stats.index_count = prepared.index_count;
            }
        }
        let prepared_stats = self.prepared.render_multiview(
            queue,
            encoder,
            target,
            render_views,
            render_options,
            None,
            &self.shared.prepared,
        )?;
        stats.drawn_actor_count = stats
            .drawn_actor_count
            .saturating_add(prepared_stats.drawn_actor_count);
        stats.vertex_count = stats
            .vertex_count
            .saturating_add(prepared_stats.vertex_count);
        stats.index_count = stats.index_count.saturating_add(prepared_stats.index_count);
        Ok(stats)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_composed_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        physical_render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
        actors: &[ActorInstance],
        context: WorldCompositionContext,
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let selection = select_composed_actors(
            &mut self.composed_actor_scratch,
            actors,
            context,
            &physical_render_views,
        );
        let mut stats = selection.render_stats(actors.len());
        if self.composed_actor_scratch.is_empty() {
            return Ok(stats);
        }
        self.prepared.prepare(
            device,
            queue,
            &self.shared.prepared,
            &self.composed_actor_scratch,
        );
        let legacy_actors = self.prepared.legacy_actors();
        if !legacy_actors.is_empty() {
            let depth_view = target
                .depth_view
                .context("placed actor multiview render pass requires a depth attachment")?;
            if let Some(prepared) = self.mesh_cache.prepare(
                device,
                queue,
                legacy_actors,
                self.shared.texture_layout,
                self.shared.atlas_size,
                &self.shared.actor_figures,
            ) {
                let renderer = self.shared.renderer.placed_multiview_renderer(device)?;
                if self.placed_multiview.borrow().is_none() {
                    *self.placed_multiview.borrow_mut() = Some(ActorPlacedMultiviewDrawState::new(
                        device,
                        &renderer.uniform_bind_group_layout,
                    ));
                }
                let placed = self.placed_multiview.borrow();
                let placed = placed
                    .as_ref()
                    .expect("placed actor multiview draw state initialized above");
                queue.write_buffer(
                    &placed.uniform_buffer,
                    0,
                    &placed_multiview_uniform_bytes(physical_render_views, render_options, context),
                );
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_actor_placed_multiview_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
                    ..Default::default()
                });
                pass.set_pipeline(renderer.pipeline(context.clip()));
                pass.set_bind_group(0, &placed.bind_group, &[]);
                pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..prepared.vertex_byte_len));
                pass.set_index_buffer(
                    prepared.index_buffer.slice(..prepared.index_byte_len),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..prepared.index_count, 0, 0..1);
                stats.drawn_actor_count = legacy_actors.len();
                stats.vertex_count = prepared.vertex_count;
                stats.index_count = prepared.index_count;
            }
        }
        let prepared_stats = self.prepared.render_multiview(
            queue,
            encoder,
            target,
            physical_render_views,
            render_options,
            Some(context),
            &self.shared.prepared,
        )?;
        stats.drawn_actor_count = stats
            .drawn_actor_count
            .saturating_add(prepared_stats.drawn_actor_count);
        stats.vertex_count = stats
            .vertex_count
            .saturating_add(prepared_stats.vertex_count);
        stats.index_count = stats.index_count.saturating_add(prepared_stats.index_count);
        Ok(stats)
    }

    pub fn resource_snapshot(&self) -> ActorDrawResourceSnapshot {
        let multiview_pipeline_materialized = self.shared.renderer.multiview.get().is_some();
        let multiview_uniform_materialized = self.multiview.borrow().is_some();
        let placed_pipeline_materialized = self.shared.renderer.placed.get().is_some();
        let placed_multiview_pipeline_materialized =
            self.shared.renderer.placed_multiview.get().is_some();
        let placed_uniform_allocated_bytes = self
            .placed
            .borrow()
            .as_ref()
            .map_or(0, |state| state.uniforms.allocated_byte_size());
        let placed_multiview_uniform_allocated_bytes = if self.placed_multiview.borrow().is_some() {
            PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE
        } else {
            0
        };
        let atlas_base_bytes =
            self.shared.atlas_size[0] as usize * self.shared.atlas_size[1] as usize * 4;
        let direct_uniform_allocated_bytes = self.uniforms.allocated_byte_size();
        let multiview_uniform_allocated_bytes = if multiview_uniform_materialized {
            MULTIVIEW_UNIFORM_BYTE_SIZE
        } else {
            0
        };
        let mesh = self.mesh_cache.snapshot();
        let prepared_shared = self.shared.prepared.snapshot();
        let prepared_world = self.prepared.snapshot();
        let prepared_shared_bytes = prepared_shared
            .immutable_vertex_bytes
            .saturating_add(prepared_shared.immutable_index_bytes)
            .saturating_add(prepared_shared.immutable_atlas_bytes)
            as usize;
        ActorDrawResourceSnapshot {
            shared_strong_owner_count: Arc::strong_count(&self.shared),
            shared_known_retained_bytes: atlas_base_bytes.saturating_add(prepared_shared_bytes),
            mutable_state_allocated_bytes: direct_uniform_allocated_bytes
                .saturating_add(multiview_uniform_allocated_bytes)
                .saturating_add(placed_uniform_allocated_bytes)
                .saturating_add(placed_multiview_uniform_allocated_bytes)
                .saturating_add(mesh.cpu_vertex_capacity_bytes as u64)
                .saturating_add(mesh.cpu_index_capacity_bytes as u64)
                .saturating_add(mesh.cpu_vertex_staging_capacity_bytes as u64)
                .saturating_add(mesh.cpu_index_staging_capacity_bytes as u64)
                .saturating_add(mesh.gpu_vertex_capacity_bytes)
                .saturating_add(mesh.gpu_index_capacity_bytes)
                .saturating_add(prepared_world.mutable_known_allocated_bytes),
            atlas_size: self.shared.atlas_size,
            atlas_base_bytes,
            figure_count: self.shared.actor_figures.len(),
            direct_pipeline_count: 1,
            direct_uniform_payload_bytes: self.uniforms.payload_size(),
            direct_uniform_slot_bytes: self.uniforms.slot_size(),
            direct_uniform_slot_count: self.uniforms.slot_count(),
            direct_uniform_allocated_bytes,
            multiview_pipeline_count: usize::from(multiview_pipeline_materialized),
            multiview_uniform_allocated_bytes,
            placed_pipeline_count: usize::from(placed_pipeline_materialized),
            clipped_placed_pipeline_count: usize::from(placed_pipeline_materialized),
            placed_uniform_allocated_bytes,
            placed_multiview_pipeline_count: usize::from(placed_multiview_pipeline_materialized),
            clipped_placed_multiview_pipeline_count: usize::from(
                placed_multiview_pipeline_materialized,
            ),
            placed_multiview_uniform_allocated_bytes,
            mesh,
            prepared_shared,
            prepared_world,
        }
    }
}

struct ActorRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    multiview: OnceLock<ActorMultiviewRenderer>,
    placed: OnceLock<ActorPlacedRenderer>,
    placed_multiview: OnceLock<ActorPlacedMultiviewRenderer>,
}

impl ActorRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::fog::inject_fog_wgsl(include_str!("shaders/entity_actor.wgsl")).into(),
            ),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_actor_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(UNIFORM_BYTE_SIZE),
                },
                count: None,
            }],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_pipeline",
            "fs_main",
            None,
        );

        Self {
            pipeline,
            uniform_bind_group_layout: bind_group_layout,
            texture_bind_group_layout,
            color_format,
            multiview: OnceLock::new(),
            placed: OnceLock::new(),
            placed_multiview: OnceLock::new(),
        }
    }

    fn multiview_renderer(&self, device: &wgpu::Device) -> Result<&ActorMultiviewRenderer> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("actor multiview render requires wgpu MULTIVIEW");
        }
        Ok(self.multiview.get_or_init(|| {
            ActorMultiviewRenderer::new(device, self.color_format, &self.texture_bind_group_layout)
        }))
    }

    fn placed_renderer(&self, device: &wgpu::Device) -> &ActorPlacedRenderer {
        self.placed.get_or_init(|| {
            ActorPlacedRenderer::new(device, self.color_format, &self.texture_bind_group_layout)
        })
    }

    fn placed_multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<&ActorPlacedMultiviewRenderer> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("placed actor multiview render requires wgpu MULTIVIEW");
        }
        Ok(self.placed_multiview.get_or_init(|| {
            ActorPlacedMultiviewRenderer::new(
                device,
                self.color_format,
                &self.texture_bind_group_layout,
            )
        }))
    }
}

struct ActorMultiviewRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl ActorMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::fog::inject_fog_wgsl(include_str!("shaders/entity_actor_multiview.wgsl"))
                    .into(),
            ),
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_multiview_pipeline",
            "fs_main",
            NonZeroU32::new(2),
        );
        Self {
            pipeline,
            uniform_bind_group_layout,
        }
    }
}

struct ActorPlacedRenderer {
    unbounded_pipeline: wgpu::RenderPipeline,
    clipped_pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl ActorPlacedRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_placed_shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::fog::inject_fog_wgsl(include_str!("shaders/entity_actor_placed.wgsl"))
                    .into(),
            ),
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_placed_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(PLACED_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_placed_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let unbounded_pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_placed_pipeline",
            "fs_unbounded",
            None,
        );
        let clipped_pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_clipped_placed_pipeline",
            "fs_half_space",
            None,
        );
        Self {
            unbounded_pipeline,
            clipped_pipeline,
            uniform_bind_group_layout,
        }
    }

    fn pipeline(&self, clip: CompositionClip) -> &wgpu::RenderPipeline {
        match clip {
            CompositionClip::Unbounded => &self.unbounded_pipeline,
            CompositionClip::HalfSpace(_) => &self.clipped_pipeline,
        }
    }
}

struct ActorPlacedMultiviewRenderer {
    unbounded_pipeline: wgpu::RenderPipeline,
    clipped_pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl ActorPlacedMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_placed_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::fog::inject_fog_wgsl(include_str!(
                    "shaders/entity_actor_placed_multiview.wgsl"
                ))
                .into(),
            ),
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_placed_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_placed_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let multiview = NonZeroU32::new(2);
        let unbounded_pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_placed_multiview_pipeline",
            "fs_unbounded",
            multiview,
        );
        let clipped_pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_clipped_placed_multiview_pipeline",
            "fs_half_space",
            multiview,
        );
        Self {
            unbounded_pipeline,
            clipped_pipeline,
            uniform_bind_group_layout,
        }
    }

    fn pipeline(&self, clip: CompositionClip) -> &wgpu::RenderPipeline {
        match clip {
            CompositionClip::Unbounded => &self.unbounded_pipeline,
            CompositionClip::HalfSpace(_) => &self.clipped_pipeline,
        }
    }
}

struct ActorPlacedDrawState {
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
}

impl ActorPlacedDrawState {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_actor_placed_uniforms",
            PLACED_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_placed_bind_group"),
            layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        Self {
            uniforms,
            bind_group,
        }
    }
}

struct ActorPlacedMultiviewDrawState {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ActorPlacedMultiviewDrawState {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_actor_placed_multiview_uniforms"),
            size: PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_placed_multiview_bind_group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        Self {
            uniform_buffer,
            bind_group,
        }
    }
}

struct ActorMultiviewDrawState {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ActorMultiviewDrawState {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_actor_multiview_uniforms"),
            size: MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_multiview_uniform_bind_group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        Self {
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &multiview_uniform_bytes(render_views, render_options),
        );
    }
}

fn create_actor_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    fragment_entry_point: &'static str,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: ACTOR_VERTEX_BYTE_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        offset: 12,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 20,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: 36,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Uint32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry_point),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

struct GpuActorTextureAtlas {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

impl GpuActorTextureAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: ActorTextureAtlas<'_>,
    ) -> Result<Self> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            bail!(
                "actor texture atlas has {} bytes; expected {expected_len} for {}x{} RGBA",
                atlas.rgba.len(),
                width,
                height
            );
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_actor_texture_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_actor_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_texture_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ActorMesh {
    vertices: Vec<ActorVertex>,
    indices: Vec<u32>,
}

#[derive(Default)]
struct ActorMeshBuildScratch {
    sampled_transforms: Vec<CompiledFigureTransform>,
    content_cache: Vec<Option<Mat4>>,
    content_matrices: Vec<Mat4>,
}

struct ActorMeshCache {
    actors: Vec<ActorInstance>,
    mesh: ActorMesh,
    scratch: ActorMeshBuildScratch,
    vertex_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    vertex_buffer_size: wgpu::BufferAddress,
    index_buffer_size: wgpu::BufferAddress,
    uploaded: bool,
    rebuild_count: u64,
    upload_count: u64,
    last_uploaded_bytes: u64,
    total_uploaded_bytes: u64,
}

struct PreparedActorBuffers<'a> {
    vertex_buffer: &'a wgpu::Buffer,
    index_buffer: &'a wgpu::Buffer,
    vertex_byte_len: wgpu::BufferAddress,
    index_byte_len: wgpu::BufferAddress,
    vertex_count: u32,
    index_count: u32,
}

impl ActorMeshCache {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            actors: Vec::new(),
            mesh: ActorMesh {
                vertices: Vec::with_capacity(ACTOR_MESH_MIN_VERTEX_CAPACITY),
                indices: Vec::with_capacity(ACTOR_MESH_MIN_INDEX_CAPACITY),
            },
            scratch: ActorMeshBuildScratch::default(),
            vertex_bytes: Vec::with_capacity(ACTOR_VERTEX_BUFFER_MIN_BYTE_SIZE as usize),
            index_bytes: Vec::with_capacity(ACTOR_INDEX_BUFFER_MIN_BYTE_SIZE as usize),
            vertex_buffer: Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_actor_vertices"),
                size: actor_buffer_capacity(ACTOR_VERTEX_BUFFER_MIN_BYTE_SIZE, 0),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })),
            index_buffer: Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_actor_indices"),
                size: actor_buffer_capacity(ACTOR_INDEX_BUFFER_MIN_BYTE_SIZE, 0),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })),
            vertex_buffer_size: actor_buffer_capacity(ACTOR_VERTEX_BUFFER_MIN_BYTE_SIZE, 0),
            index_buffer_size: actor_buffer_capacity(ACTOR_INDEX_BUFFER_MIN_BYTE_SIZE, 0),
            uploaded: false,
            rebuild_count: 0,
            upload_count: 0,
            last_uploaded_bytes: 0,
            total_uploaded_bytes: 0,
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actors: &[ActorInstance],
        texture_layout: ActorTextureLayout,
        atlas_size: [u32; 2],
        actor_figures: &ActorFigureSet,
    ) -> Option<PreparedActorBuffers<'_>> {
        if self.actors.as_slice() != actors {
            self.actors.clear();
            self.actors.extend_from_slice(actors);
            rebuild_actor_mesh(
                &mut self.mesh,
                actors,
                texture_layout,
                atlas_size,
                actor_figures,
                &mut self.scratch,
            );
            self.uploaded = false;
            self.rebuild_count = self.rebuild_count.saturating_add(1);
        }
        if self.mesh.vertices.is_empty() || self.mesh.indices.is_empty() {
            return None;
        }

        let vertex_byte_len =
            (self.mesh.vertices.len() * ACTOR_VERTEX_BYTE_LEN) as wgpu::BufferAddress;
        let index_byte_len =
            (self.mesh.indices.len() * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
        self.ensure_buffer_capacity(
            device,
            vertex_byte_len,
            index_byte_len,
            "mclone_actor_vertices",
            "mclone_actor_indices",
        );
        if !self.uploaded {
            write_actor_vertex_bytes(&self.mesh.vertices, &mut self.vertex_bytes);
            write_index_bytes(&self.mesh.indices, &mut self.index_bytes);
            if let Some(buffer) = &self.vertex_buffer {
                queue.write_buffer(buffer, 0, &self.vertex_bytes);
            }
            if let Some(buffer) = &self.index_buffer {
                queue.write_buffer(buffer, 0, &self.index_bytes);
            }
            self.uploaded = true;
            self.upload_count = self.upload_count.saturating_add(1);
            self.last_uploaded_bytes = vertex_byte_len.saturating_add(index_byte_len);
            self.total_uploaded_bytes = self
                .total_uploaded_bytes
                .saturating_add(self.last_uploaded_bytes);
        }

        Some(PreparedActorBuffers {
            vertex_buffer: self
                .vertex_buffer
                .as_ref()
                .expect("actor vertex buffer exists after prepare"),
            index_buffer: self
                .index_buffer
                .as_ref()
                .expect("actor index buffer exists after prepare"),
            vertex_byte_len,
            index_byte_len,
            vertex_count: self.mesh.vertices.len() as u32,
            index_count: self.mesh.indices.len() as u32,
        })
    }

    fn ensure_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        vertex_byte_len: wgpu::BufferAddress,
        index_byte_len: wgpu::BufferAddress,
        vertex_label: &'static str,
        index_label: &'static str,
    ) {
        let required_vertex_size =
            actor_buffer_capacity(vertex_byte_len, ACTOR_VERTEX_BUFFER_MIN_BYTE_SIZE);
        if self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_size < required_vertex_size)
        {
            self.vertex_buffer_size = required_vertex_size;
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(vertex_label),
                size: self.vertex_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.uploaded = false;
        }

        let required_index_size =
            actor_buffer_capacity(index_byte_len, ACTOR_INDEX_BUFFER_MIN_BYTE_SIZE);
        if self
            .index_buffer
            .as_ref()
            .is_none_or(|_| self.index_buffer_size < required_index_size)
        {
            self.index_buffer_size = required_index_size;
            self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(index_label),
                size: self.index_buffer_size,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.uploaded = false;
        }
    }

    fn snapshot(&self) -> ActorMeshCacheSnapshot {
        ActorMeshCacheSnapshot {
            cached_actor_count: self.actors.len(),
            rebuild_count: self.rebuild_count,
            upload_count: self.upload_count,
            last_uploaded_bytes: self.last_uploaded_bytes,
            total_uploaded_bytes: self.total_uploaded_bytes,
            cpu_vertex_capacity_bytes: self.mesh.vertices.capacity() * ACTOR_VERTEX_BYTE_LEN,
            cpu_index_capacity_bytes: self.mesh.indices.capacity() * std::mem::size_of::<u32>(),
            cpu_vertex_staging_capacity_bytes: self.vertex_bytes.capacity(),
            cpu_index_staging_capacity_bytes: self.index_bytes.capacity(),
            gpu_vertex_capacity_bytes: self.vertex_buffer_size,
            gpu_index_capacity_bytes: self.index_buffer_size,
        }
    }
}

fn actor_buffer_capacity(
    byte_len: wgpu::BufferAddress,
    minimum: wgpu::BufferAddress,
) -> wgpu::BufferAddress {
    byte_len
        .max(4)
        .max(minimum)
        .checked_next_power_of_two()
        .unwrap_or_else(|| byte_len.max(4).max(minimum))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorVertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
    packed_light: u32,
}

#[cfg(test)]
fn actor_mesh(
    actors: &[ActorInstance],
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: &ActorFigureSet,
) -> ActorMesh {
    let mut mesh = ActorMesh::default();
    let mut scratch = ActorMeshBuildScratch::default();
    rebuild_actor_mesh(
        &mut mesh,
        actors,
        texture_layout,
        atlas_size,
        actor_figures,
        &mut scratch,
    );
    mesh
}

fn rebuild_actor_mesh(
    mesh: &mut ActorMesh,
    actors: &[ActorInstance],
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: &ActorFigureSet,
    scratch: &mut ActorMeshBuildScratch,
) {
    mesh.vertices.clear();
    mesh.indices.clear();
    if mesh.vertices.capacity() < ACTOR_MESH_MIN_VERTEX_CAPACITY {
        mesh.vertices
            .reserve(ACTOR_MESH_MIN_VERTEX_CAPACITY - mesh.vertices.capacity());
    }
    if mesh.indices.capacity() < ACTOR_MESH_MIN_INDEX_CAPACITY {
        mesh.indices
            .reserve(ACTOR_MESH_MIN_INDEX_CAPACITY - mesh.indices.capacity());
    }
    for actor in actors {
        append_actor(
            mesh,
            *actor,
            texture_layout,
            atlas_size,
            actor_figures,
            scratch,
        );
    }
}

fn append_actor(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: &ActorFigureSet,
    scratch: &mut ActorMeshBuildScratch,
) {
    let first_vertex = mesh.vertices.len();
    match actor.shape {
        ActorInstanceShape::Figure(figure) => append_asset_lab_figure_model(
            mesh,
            actor,
            texture_layout,
            atlas_size,
            actor_figures.get(figure),
            scratch,
        ),
        ActorInstanceShape::Humanoid => {
            append_humanoid_model(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::QuadrupedPlaceholder => {
            append_quadruped_placeholder(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::CowModel => append_cow_model(mesh, actor, texture_layout, atlas_size),
        ActorInstanceShape::DebugCube => append_debug_cube(mesh, actor, texture_layout, atlas_size),
        ActorInstanceShape::ItemEgg => append_item_egg(mesh, actor, texture_layout, atlas_size),
        ActorInstanceShape::MallardNest => {
            append_mallard_nest(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::MallardFeather => {
            append_mallard_feather(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::MallardTrack => {
            append_mallard_track(mesh, actor, texture_layout, atlas_size)
        }
    }
    let opacity = if actor.opacity.is_finite() {
        actor.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    for vertex in &mut mesh.vertices[first_vertex..] {
        vertex.color[3] *= opacity;
    }
}

fn append_asset_lab_figure_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    figure: Option<&ActorFigure>,
    scratch: &mut ActorMeshBuildScratch,
) {
    let Some(figure) = figure else {
        append_humanoid_model(mesh, actor, texture_layout, atlas_size);
        return;
    };

    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let model_scale = actor.height.max(0.1);
    sample_figure_part_transforms_into(
        figure,
        actor.animation,
        actor.chicken_wing_flap_radians,
        &mut scratch.sampled_transforms,
    );
    figure_content_matrices_into(
        figure,
        &scratch.sampled_transforms,
        &mut scratch.content_cache,
        &mut scratch.content_matrices,
    );
    for (part_index, part) in figure.parts.iter().enumerate() {
        let content_matrix = scratch.content_matrices[part_index];
        for cuboid in &part.cuboids {
            if actor.first_person_body_only && !cuboid.first_person_visible {
                continue;
            }
            append_asset_lab_local_box(
                mesh,
                actor,
                figure,
                content_matrix,
                cuboid.min,
                cuboid.max,
                white_uv,
                lab_order_face_colors(cuboid.face_colors),
                model_scale,
            );
        }
        for cuboid in &part.overlay_cuboids {
            if actor.first_person_body_only && !cuboid.first_person_visible {
                continue;
            }
            append_asset_lab_local_box(
                mesh,
                actor,
                figure,
                content_matrix,
                cuboid.min,
                cuboid.max,
                white_uv,
                [cuboid.color; 6],
                model_scale,
            );
        }
    }
}

fn sample_figure_part_transforms_into(
    figure: &ActorFigure,
    animation: Option<ActorAnimation>,
    chicken_wing_flap_radians: Option<f32>,
    transforms: &mut Vec<CompiledFigureTransform>,
) {
    transforms.clear();
    transforms.resize(figure.parts.len(), CompiledFigureTransform::default());
    if let Some(animation) = animation {
        let clip = match animation.clip {
            ActorAnimationClip::Walk => figure.clips.get("walk"),
        };
        if let Some(clip) = clip {
            let time_seconds = clip_time_for_animation_distance(clip, animation.distance);
            for (part_index, keys) in &clip.tracks {
                if *part_index < transforms.len() {
                    transforms[*part_index] = sample_clip_track(keys, time_seconds, clip);
                }
            }
        }
    }
    if let Some(wing_flap) = chicken_wing_flap_radians {
        apply_chicken_wing_flap(figure, transforms, wing_flap);
    }
}

fn apply_chicken_wing_flap(
    figure: &ActorFigure,
    transforms: &mut [CompiledFigureTransform],
    wing_flap_radians: f32,
) {
    if !wing_flap_radians.is_finite() || wing_flap_radians.abs() <= f32::EPSILON {
        return;
    }
    for (part_index, part) in figure.parts.iter().enumerate() {
        let z_delta = match part.name.as_str() {
            "wing_l" => wing_flap_radians,
            "wing_r" => -wing_flap_radians,
            _ => continue,
        };
        if let Some(transform) = transforms.get_mut(part_index) {
            let rot = transform.rot_radians.unwrap_or(Vec3::ZERO);
            transform.rot_radians = Some(rot + Vec3::new(0.0, 0.0, z_delta));
        }
    }
}

fn clip_time_for_animation_distance(clip: &CompiledFigureClip, distance: f32) -> f32 {
    if clip.duration_seconds <= 0.0 {
        return 0.0;
    }
    if let Some(locomotion) = &clip.locomotion
        && locomotion.cycle_distance > 0.0
    {
        return (distance.max(0.0) / locomotion.cycle_distance) * clip.duration_seconds;
    }
    distance.max(0.0)
}

fn sample_clip_track(
    keys: &[crate::asset_lab_figure::CompiledFigureKey],
    time_seconds: f32,
    clip: &CompiledFigureClip,
) -> CompiledFigureTransform {
    let Some(first) = keys.first() else {
        return CompiledFigureTransform::default();
    };
    let local_time = if clip.looped && clip.duration_seconds > 0.0 {
        time_seconds.rem_euclid(clip.duration_seconds)
    } else {
        time_seconds.clamp(0.0, clip.duration_seconds)
    };
    let mut left = first;
    let mut right = keys.last().unwrap_or(first);
    for (index, current) in keys.iter().enumerate() {
        let next = keys.get(index + 1);
        if next.is_none_or(|next| local_time < next.time_seconds) {
            left = current;
            right = next.unwrap_or(current);
            break;
        }
    }
    let alpha = if (right.time_seconds - left.time_seconds).abs() <= f32::EPSILON {
        0.0
    } else {
        ((local_time - left.time_seconds) / (right.time_seconds - left.time_seconds))
            .clamp(0.0, 1.0)
    };
    CompiledFigureTransform {
        at: mix_optional_vec3(left.transform.at, right.transform.at, alpha),
        rot_radians: mix_optional_vec3(
            left.transform.rot_radians,
            right.transform.rot_radians,
            alpha,
        ),
    }
}

fn mix_optional_vec3(left: Option<Vec3>, right: Option<Vec3>, alpha: f32) -> Option<Vec3> {
    match (left, right) {
        (None, None) => None,
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (Some(left), Some(right)) => Some(left + (right - left) * alpha),
    }
}

fn figure_content_matrices_into(
    figure: &ActorFigure,
    transforms: &[CompiledFigureTransform],
    cache: &mut Vec<Option<Mat4>>,
    content_matrices: &mut Vec<Mat4>,
) {
    cache.clear();
    cache.resize(figure.parts.len(), None);
    content_matrices.clear();
    content_matrices.resize(figure.parts.len(), Mat4::IDENTITY);
    for index in 0..figure.parts.len() {
        content_matrices[index] = figure_content_matrix(index, figure, transforms, cache);
    }
}

fn figure_content_matrix(
    index: usize,
    figure: &ActorFigure,
    transforms: &[CompiledFigureTransform],
    cache: &mut [Option<Mat4>],
) -> Mat4 {
    if let Some(matrix) = cache[index] {
        return matrix;
    }
    let part = &figure.parts[index];
    let parent_content = part
        .parent
        .map(|parent| figure_content_matrix(parent, figure, transforms, cache))
        .unwrap_or(Mat4::IDENTITY);
    let transform = transforms.get(index).copied().unwrap_or_default();
    let position = part.base_position + transform.at.unwrap_or(Vec3::ZERO);
    let rotation = part.base_rotation_radians + transform.rot_radians.unwrap_or(Vec3::ZERO);
    let group = parent_content
        * Mat4::from_translation(position)
        * Mat4::from_quat(Quat::from_euler(
            EulerRot::XYZ,
            rotation.x,
            rotation.y,
            rotation.z,
        ));
    let content = group * Mat4::from_translation(-part.pivot);
    cache[index] = Some(content);
    content
}

#[allow(clippy::too_many_arguments)]
fn append_asset_lab_local_box(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    figure: &ActorFigure,
    content_matrix: Mat4,
    min: Vec3,
    max: Vec3,
    uv: [f32; 2],
    face_colors: [[f32; 4]; 6],
    model_scale: f32,
) {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        [[uv; 4]; 6],
        actor.packed_light,
        // lab_point_to_actor_local mirrors Z, which reverses handedness.
        BoxWinding::Reverse,
        |corner_index| {
            let lab_position = content_matrix.transform_point3(corners[corner_index]);
            actor_world_position(
                actor,
                lab_point_to_actor_local(figure, lab_position) * model_scale,
            )
        },
    );
}

fn lab_point_to_actor_local(figure: &ActorFigure, point: Vec3) -> Vec3 {
    let normalized = (point - figure.normalization_origin) * figure.inv_height;
    Vec3::new(normalized.x, normalized.y, -normalized.z)
}

fn lab_order_face_colors(actor_order: [[f32; 4]; 6]) -> [[f32; 4]; 6] {
    [
        actor_order[1],
        actor_order[0],
        actor_order[2],
        actor_order[3],
        actor_order[4],
        actor_order[5],
    ]
}

fn append_humanoid_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let clothing_dark = scale_color(actor.body_color, 0.48);
    let clothing_side = scale_color(actor.body_color, 0.78);
    let clothing_light = scale_color(actor.body_color, 1.10);
    let skin_shadow = scale_color(actor.accent_color, 0.72);
    let skin_side = scale_color(actor.accent_color, 0.88);
    let skin_light = scale_color(actor.accent_color, 1.05);
    let hair = scale_color(actor.body_color, 0.22);
    let eye = [0.04, 0.035, 0.03, 1.0];
    let mouth = [0.36, 0.11, 0.10, 1.0];
    append_box(
        mesh,
        actor,
        Vec3::new(-0.21, 0.0, -0.12),
        Vec3::new(-0.04, 0.76, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            actor.body_color,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.04, 0.0, -0.12),
        Vec3::new(0.21, 0.76, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            actor.body_color,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.30, 0.74, -0.15),
        Vec3::new(0.30, 1.36, 0.15),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.46, 0.62, -0.12),
        Vec3::new(-0.31, 1.30, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.31, 0.62, -0.12),
        Vec3::new(0.46, 1.30, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.45, 0.48, -0.11),
        Vec3::new(-0.32, 0.64, 0.11),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.32, 0.48, -0.11),
        Vec3::new(0.45, 0.64, 0.11),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.24, 1.34, -0.24),
        Vec3::new(0.24, 1.80, 0.24),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.255, 1.66, -0.255),
        Vec3::new(0.255, 1.84, 0.255),
        white_uv,
        [
            scale_color(hair, 0.70),
            hair,
            hair,
            scale_color(hair, 1.12),
            scale_color(hair, 0.92),
            scale_color(hair, 1.18),
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.135, 1.56, 0.236),
        Vec3::new(-0.065, 1.635, 0.258),
        white_uv,
        [eye; 6],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.065, 1.56, 0.236),
        Vec3::new(0.135, 1.635, 0.258),
        white_uv,
        [eye; 6],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.075, 1.455, 0.237),
        Vec3::new(0.075, 1.500, 0.258),
        white_uv,
        [mouth; 6],
    );
}

fn append_quadruped_placeholder(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let width = actor.width.max(0.1);
    let height = actor.height.max(0.1);
    let half_width = width * 0.5;
    let leg_half = (width * 0.12).clamp(0.04, 0.16);
    let body_bottom = height * 0.32;
    let body_top = height * 0.82;
    let body_back = -width * 0.62;
    let body_front = width * 0.46;
    let head_bottom = height * 0.54;
    let head_top = height;
    let head_half = width * 0.28;
    let head_front = body_front + width * 0.38;

    let dark = scale_color(actor.body_color, 0.58);
    let side = scale_color(actor.body_color, 0.78);
    let light = scale_color(actor.body_color, 1.12);
    let accent_side = scale_color(actor.accent_color, 0.86);
    append_box(
        mesh,
        actor,
        Vec3::new(-half_width, body_bottom, body_back),
        Vec3::new(half_width, body_top, body_front),
        white_uv,
        [dark, side, side, side, side, light],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-head_half, head_bottom, body_front),
        Vec3::new(head_half, head_top, head_front),
        white_uv,
        [
            scale_color(actor.accent_color, 0.70),
            accent_side,
            accent_side,
            scale_color(actor.accent_color, 0.92),
            scale_color(actor.accent_color, 0.92),
            actor.accent_color,
        ],
    );
    for x in [-half_width + leg_half * 1.2, half_width - leg_half * 1.2] {
        for z in [body_back + leg_half * 1.2, body_front - leg_half * 1.2] {
            append_box(
                mesh,
                actor,
                Vec3::new(x - leg_half, 0.0, z - leg_half),
                Vec3::new(x + leg_half, body_bottom, z + leg_half),
                white_uv,
                [dark, side, side, side, side, actor.body_color],
            );
        }
    }
}

fn append_debug_cube(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let width = actor.width.max(0.1);
    let height = actor.height.max(0.1);
    let half_width = width * 0.5;
    append_box(
        mesh,
        actor,
        Vec3::new(-half_width, 0.0, -half_width),
        Vec3::new(half_width, height, half_width),
        white_uv,
        [
            scale_color(actor.body_color, 0.56),
            scale_color(actor.body_color, 0.78),
            scale_color(actor.body_color, 0.84),
            scale_color(actor.accent_color, 0.82),
            actor.body_color,
            actor.accent_color,
        ],
    );
}

fn append_item_egg(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let width = actor.width.max(0.1);
    let height = actor.height.max(0.1);
    let half = width * 0.5;
    let lower_half = half * 0.72;
    let upper_half = half * 0.62;
    let shell_dark = scale_color(actor.body_color, 0.70);
    let shell_side = scale_color(actor.body_color, 0.88);
    let shell_light = scale_color(actor.body_color, 1.12);
    let spot = scale_color(actor.accent_color, 0.90);

    append_box(
        mesh,
        actor,
        Vec3::new(-lower_half, 0.0, -lower_half),
        Vec3::new(lower_half, height * 0.28, lower_half),
        white_uv,
        [
            shell_dark,
            shell_side,
            shell_side,
            shell_side,
            shell_side,
            shell_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-half, height * 0.20, -half),
        Vec3::new(half, height * 0.78, half),
        white_uv,
        [
            shell_dark,
            shell_side,
            shell_side,
            scale_color(actor.body_color, 0.96),
            shell_side,
            shell_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-upper_half, height * 0.66, -upper_half),
        Vec3::new(upper_half, height, upper_half),
        white_uv,
        [
            shell_dark,
            shell_side,
            shell_side,
            shell_side,
            spot,
            shell_light,
        ],
    );
}

fn append_mallard_nest(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let half = actor.width.max(0.5) * 0.5;
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    append_box(
        mesh,
        actor,
        Vec3::new(-half, 0.0, -half),
        Vec3::new(half, actor.height.max(0.18), half),
        white_uv,
        [
            scale_color(actor.body_color, 0.55),
            actor.body_color,
            scale_color(actor.body_color, 0.82),
            actor.accent_color,
            actor.body_color,
            scale_color(actor.accent_color, 1.08),
        ],
    );
    let mut egg = actor;
    egg.feet_position.y += actor.height.max(0.18) * 0.55;
    egg.width *= 0.38;
    egg.height *= 0.72;
    append_item_egg(mesh, egg, texture_layout, atlas_size);
}

fn append_mallard_feather(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let width = actor.width.max(0.08);
    let height = actor.height.max(0.24);
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    append_box(
        mesh,
        actor,
        Vec3::new(-width * 0.5, 0.0, -0.025),
        Vec3::new(width * 0.5, height, 0.025),
        white_uv,
        [actor.body_color; 6],
    );
}

fn append_mallard_track(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    for x in [-0.11, 0.11] {
        append_box(
            mesh,
            actor,
            Vec3::new(x - 0.045, 0.002, -0.12),
            Vec3::new(x + 0.045, 0.012, 0.10),
            white_uv,
            [actor.body_color; 6],
        );
        for toe_x in [-0.055, 0.0, 0.055] {
            append_box(
                mesh,
                actor,
                Vec3::new(x + toe_x - 0.018, 0.002, 0.06),
                Vec3::new(x + toe_x + 0.018, 0.012, 0.19),
                white_uv,
                [actor.accent_color; 6],
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorModelPart {
    offset_pixels: [f32; 3],
    rotation_radians: [f32; 3],
    cuboids: &'static [ActorModelCuboid],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorModelCuboid {
    texture_offset: [u16; 2],
    origin_pixels: [f32; 3],
    size_pixels: [f32; 3],
}

const COW_HEAD_CUBOIDS: &[ActorModelCuboid] = &[
    ActorModelCuboid {
        texture_offset: [0, 0],
        origin_pixels: [-4.0, -4.0, -6.0],
        size_pixels: [8.0, 8.0, 6.0],
    },
    ActorModelCuboid {
        texture_offset: [22, 0],
        origin_pixels: [-5.0, -5.0, -4.0],
        size_pixels: [1.0, 3.0, 1.0],
    },
    ActorModelCuboid {
        texture_offset: [22, 0],
        origin_pixels: [4.0, -5.0, -4.0],
        size_pixels: [1.0, 3.0, 1.0],
    },
];

const COW_BODY_CUBOIDS: &[ActorModelCuboid] = &[
    ActorModelCuboid {
        texture_offset: [18, 4],
        origin_pixels: [-6.0, -10.0, -7.0],
        size_pixels: [12.0, 18.0, 10.0],
    },
    ActorModelCuboid {
        texture_offset: [52, 0],
        origin_pixels: [-2.0, 2.0, -8.0],
        size_pixels: [4.0, 6.0, 1.0],
    },
];

const COW_LEG_CUBOIDS: &[ActorModelCuboid] = &[ActorModelCuboid {
    texture_offset: [0, 16],
    origin_pixels: [-2.0, 0.0, -2.0],
    size_pixels: [4.0, 12.0, 4.0],
}];

const COW_MODEL_PARTS: &[ActorModelPart] = &[
    ActorModelPart {
        offset_pixels: [0.0, 4.0, -8.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_HEAD_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [0.0, 5.0, 2.0],
        rotation_radians: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
        cuboids: COW_BODY_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [-4.0, 12.0, 7.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [4.0, 12.0, 7.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [-4.0, 12.0, -6.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [4.0, 12.0, -6.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
];

fn append_cow_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    for part in COW_MODEL_PARTS {
        for cuboid in part.cuboids {
            append_model_cuboid(
                mesh,
                actor,
                part,
                cuboid,
                cow_cuboid_face_colors(*cuboid),
                texture_layout.cow,
                atlas_size,
            );
        }
    }
}

fn append_model_cuboid(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    part: &ActorModelPart,
    cuboid: &ActorModelCuboid,
    face_colors: [[f32; 4]; 6],
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
) {
    let origin = Vec3::from_array(cuboid.origin_pixels);
    let size = Vec3::from_array(cuboid.size_pixels);
    let max = origin + size;
    let corners = [
        Vec3::new(origin.x, origin.y, origin.z),
        Vec3::new(max.x, origin.y, origin.z),
        Vec3::new(max.x, max.y, origin.z),
        Vec3::new(origin.x, max.y, origin.z),
        Vec3::new(origin.x, origin.y, max.z),
        Vec3::new(max.x, origin.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(origin.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        cow_cuboid_face_uvs(*cuboid, texture_region, atlas_size),
        actor.packed_light,
        BoxWinding::Preserve,
        |corner_index| {
            let model_position = transform_model_part_point(corners[corner_index], part);
            actor_world_position(actor, model_pixels_to_actor_local(model_position))
        },
    );
}

#[derive(Clone, Copy)]
enum BoxWinding {
    Preserve,
    Reverse,
}

fn append_transformed_box(
    mesh: &mut ActorMesh,
    face_colors: [[f32; 4]; 6],
    face_uvs: [[[f32; 2]; 4]; 6],
    packed_light: u32,
    winding: BoxWinding,
    mut world_corner: impl FnMut(usize) -> Vec3,
) {
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
    ];

    for (face_index, face) in faces.into_iter().enumerate() {
        let base = mesh.vertices.len() as u32;
        for (vertex_index, corner_index) in face.into_iter().enumerate() {
            mesh.vertices.push(ActorVertex {
                position: world_corner(corner_index).to_array(),
                uv: face_uvs[face_index][vertex_index],
                color: face_colors[face_index],
                packed_light,
            });
        }
        let indices = match winding {
            BoxWinding::Preserve => [base, base + 1, base + 2, base, base + 2, base + 3],
            BoxWinding::Reverse => [base, base + 2, base + 1, base, base + 3, base + 2],
        };
        mesh.indices.extend_from_slice(&indices);
    }
}

fn transform_model_part_point(point_pixels: Vec3, part: &ActorModelPart) -> Vec3 {
    rotate_model_point(point_pixels, part.rotation_radians) + Vec3::from_array(part.offset_pixels)
}

fn rotate_model_point(point: Vec3, rotation_radians: [f32; 3]) -> Vec3 {
    let [x_rot, y_rot, z_rot] = rotation_radians;
    let mut rotated = point;
    if z_rot != 0.0 {
        let (sin, cos) = z_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x * cos - rotated.y * sin,
            rotated.x * sin + rotated.y * cos,
            rotated.z,
        );
    }
    if y_rot != 0.0 {
        let (sin, cos) = y_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x * cos + rotated.z * sin,
            rotated.y,
            -rotated.x * sin + rotated.z * cos,
        );
    }
    if x_rot != 0.0 {
        let (sin, cos) = x_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x,
            rotated.y * cos - rotated.z * sin,
            rotated.y * sin + rotated.z * cos,
        );
    }
    rotated
}

fn model_pixels_to_actor_local(model_position: Vec3) -> Vec3 {
    Vec3::new(
        model_position.x * MODEL_PIXEL_SCALE,
        (MODEL_FEET_Y_PIXELS - model_position.y) * MODEL_PIXEL_SCALE,
        -model_position.z * MODEL_PIXEL_SCALE,
    )
}

fn cow_cuboid_face_colors(cuboid: ActorModelCuboid) -> [[f32; 4]; 6] {
    match cuboid.texture_offset {
        [0, 0] | [18, 4] | [22, 0] | [52, 0] | [0, 16] => [[1.0, 1.0, 1.0, 1.0]; 6],
        _ => shaded_faces([1.0, 1.0, 1.0, 1.0]),
    }
}

fn cow_cuboid_face_uvs(
    cuboid: ActorModelCuboid,
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
) -> [[[f32; 2]; 4]; 6] {
    let [u, v] = [
        cuboid.texture_offset[0] as f32,
        cuboid.texture_offset[1] as f32,
    ];
    let [width, height, depth] = cuboid.size_pixels;
    let west = face_uv_rect(
        texture_region,
        atlas_size,
        u,
        v + depth,
        u + depth,
        v + depth + height,
    );
    let north = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth,
        v + depth,
        u + depth + width,
        v + depth + height,
    );
    let east = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width,
        v + depth,
        u + depth + width + depth,
        v + depth + height,
    );
    let south = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width + depth,
        v + depth,
        u + depth + width + depth + width,
        v + depth + height,
    );
    let down = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth,
        v,
        u + depth + width,
        v + depth,
    );
    let up = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width,
        v,
        u + depth + width + width,
        v + depth,
    );

    // append_transformed_box face order is minZ, maxZ, minX, maxX, minY, maxY.
    [north, south, west, east, down, up]
}

fn face_uv_rect(
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) -> [[f32; 2]; 4] {
    [
        texture_region_uv(texture_region, atlas_size, u0, v0),
        texture_region_uv(texture_region, atlas_size, u0, v1),
        texture_region_uv(texture_region, atlas_size, u1, v1),
        texture_region_uv(texture_region, atlas_size, u1, v0),
    ]
}

fn texture_region_center_uv(region: ActorTextureRegion, atlas_size: [u32; 2]) -> [f32; 2] {
    texture_region_uv(
        region,
        atlas_size,
        0.5 * region.width as f32,
        0.5 * region.height as f32,
    )
}

fn texture_region_uv(
    region: ActorTextureRegion,
    atlas_size: [u32; 2],
    texture_u: f32,
    texture_v: f32,
) -> [f32; 2] {
    [
        (region.x as f32 + texture_u) / atlas_size[0].max(1) as f32,
        (region.y as f32 + texture_v) / atlas_size[1].max(1) as f32,
    ]
}

fn shaded_faces(color: [f32; 4]) -> [[f32; 4]; 6] {
    [
        scale_color(color, 0.58),
        scale_color(color, 0.90),
        scale_color(color, 0.72),
        scale_color(color, 0.82),
        scale_color(color, 0.66),
        scale_color(color, 1.08),
    ]
}

fn append_box(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    min: Vec3,
    max: Vec3,
    uv: [f32; 2],
    face_colors: [[f32; 4]; 6],
) {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        [[uv; 4]; 6],
        actor.packed_light,
        BoxWinding::Preserve,
        |corner_index| actor_world_position(actor, corners[corner_index]),
    );
}

fn actor_world_position(actor: ActorInstance, local: Vec3) -> Vec3 {
    let local = local - actor.rotation_pivot;
    let local = if let Some(orientation) = actor.orientation {
        orientation * local
    } else {
        let (pitch_sin, pitch_cos) = actor.pitch_radians.sin_cos();
        let local = Vec3::new(
            local.x,
            local.y * pitch_cos - local.z * pitch_sin,
            local.y * pitch_sin + local.z * pitch_cos,
        );
        let (yaw_sin, yaw_cos) = actor.yaw_radians.sin_cos();
        Vec3::new(
            local.x * yaw_cos + local.z * yaw_sin,
            local.y,
            -local.x * yaw_sin + local.z * yaw_cos,
        )
    };
    actor.feet_position + actor.rotation_pivot + local
}

fn scale_color(color: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
        color[3],
    ]
}

fn write_actor_vertex_bytes(vertices: &[ActorVertex], bytes: &mut Vec<u8>) {
    let required_len = vertices.len() * ACTOR_VERTEX_BYTE_LEN;
    bytes.clear();
    if bytes.capacity() < required_len {
        bytes.reserve(required_len - bytes.capacity());
    }
    for vertex in vertices {
        for value in vertex
            .position
            .into_iter()
            .chain(vertex.uv)
            .chain(vertex.color)
        {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes.extend_from_slice(&vertex.packed_light.to_ne_bytes());
    }
}

fn write_index_bytes(indices: &[u32], bytes: &mut Vec<u8>) {
    let required_len = std::mem::size_of_val(indices);
    bytes.clear();
    if bytes.capacity() < required_len {
        bytes.reserve(required_len - bytes.capacity());
    }
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
}

fn uniform_bytes(
    render_view: ChunkRenderView,
    render_options: TexturedSectionRenderOptions,
) -> [u8; UNIFORM_BYTE_LEN] {
    let mut bytes = [0_u8; UNIFORM_BYTE_LEN];
    let mut offset = 0;
    for value in render_view.uniform_matrix().into_iter().flatten() {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    for value in [
        if render_options.force_fullbright {
            1.0
        } else {
            0.0
        },
        render_options.sky_darken.clamp(0.0, 1.0),
        render_options.fog.shader_options(),
        render_options.fog.ground_base_y,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    let camera_position = render_view.camera_position.to_array();
    for value in [
        camera_position[0],
        camera_position[1],
        camera_position[2],
        0.0,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    for value in [
        render_options.fog.color[0],
        render_options.fog.color[1],
        render_options.fog.color[2],
        render_options.fog.max_opacity,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    let fog_distances = render_options.fog.shader_distances();
    for value in [fog_distances[0], fog_distances[1], 0.0, 0.0] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    bytes
}

fn multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    render_options: [TexturedSectionRenderOptions; 2],
) -> [u8; MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..UNIFORM_BYTE_LEN].copy_from_slice(&uniform_bytes(render_views[0], render_options[0]));
    bytes[UNIFORM_BYTE_LEN..].copy_from_slice(&uniform_bytes(render_views[1], render_options[1]));
    bytes
}

fn placed_uniform_bytes(
    render_view: ChunkRenderView,
    render_options: TexturedSectionRenderOptions,
    context: WorldCompositionContext,
) -> [u8; PLACED_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; PLACED_UNIFORM_BYTE_LEN];
    bytes[..UNIFORM_BYTE_LEN].copy_from_slice(&uniform_bytes(render_view, render_options));
    let (source_anchor_scale, composition_anchor) = context.placement().shader_values();
    let clip_plane = match context.clip() {
        CompositionClip::Unbounded => [0.0; 4],
        CompositionClip::HalfSpace(half_space) => [
            half_space.normal().x,
            half_space.normal().y,
            half_space.normal().z,
            half_space.offset(),
        ],
    };
    for (index, value) in source_anchor_scale
        .into_iter()
        .chain(composition_anchor)
        .chain(clip_plane)
        .enumerate()
    {
        let offset = UNIFORM_BYTE_LEN + index * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn placed_multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    render_options: [TexturedSectionRenderOptions; 2],
    context: WorldCompositionContext,
) -> [u8; PLACED_MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; PLACED_MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..PLACED_UNIFORM_BYTE_LEN].copy_from_slice(&placed_uniform_bytes(
        render_views[0],
        render_options[0],
        context,
    ));
    bytes[PLACED_UNIFORM_BYTE_LEN..].copy_from_slice(&placed_uniform_bytes(
        render_views[1],
        render_options[1],
        context,
    ));
    bytes
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ActorCompositionSelection {
    source_rejected_actor_count: usize,
    clip_rejected_actor_count: usize,
    frustum_rejected_actor_count: usize,
}

impl ActorCompositionSelection {
    fn render_stats(self, submitted_actor_count: usize) -> ActorRenderStats {
        ActorRenderStats {
            submitted_actor_count,
            source_rejected_actor_count: self.source_rejected_actor_count,
            clip_rejected_actor_count: self.clip_rejected_actor_count,
            frustum_rejected_actor_count: self.frustum_rejected_actor_count,
            ..ActorRenderStats::default()
        }
    }
}

fn select_composed_actors(
    selected: &mut Vec<ActorInstance>,
    actors: &[ActorInstance],
    context: WorldCompositionContext,
    physical_render_views: &[ChunkRenderView],
) -> ActorCompositionSelection {
    selected.clear();
    let mut report = ActorCompositionSelection::default();
    for actor in actors {
        if context.source_bounds().is_some_and(|bounds| {
            !bounds.contains(Vec3d::new(
                f64::from(actor.feet_position.x),
                f64::from(actor.feet_position.y),
                f64::from(actor.feet_position.z),
            ))
        }) {
            report.source_rejected_actor_count += 1;
            continue;
        }
        let (source_min, source_max) = actor_source_rejection_aabb(*actor);
        let placement = context.placement();
        let composition_min = placement.source_to_composition_f32(source_min);
        let composition_max = placement.source_to_composition_f32(source_max);
        if context
            .clip()
            .rejects_aabb(composition_min, composition_max)
        {
            report.clip_rejected_actor_count += 1;
            continue;
        }
        if !physical_render_views.iter().copied().any(|render_view| {
            render_view_aabb_visible(render_view, composition_min, composition_max)
        }) {
            report.frustum_rejected_actor_count += 1;
            continue;
        }
        selected.push(*actor);
    }
    report
}

fn actor_source_rejection_aabb(actor: ActorInstance) -> (Vec3, Vec3) {
    // A sphere-derived AABB is conservative for yaw, pitch, arbitrary
    // orientation, animated limbs, wings, and cuboid overlays. Exact clipping
    // remains fragment work; CPU rejection must never remove an intersecting
    // actor before mesh preparation.
    let half_width = actor.width.max(0.1) * 0.5;
    let half_height = actor.height.max(0.1) * 0.5;
    let radius = Vec3::new(half_width, half_height, half_width).length();
    let center = actor.feet_position + Vec3::Y * half_height;
    (center - Vec3::splat(radius), center + Vec3::splat(radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_actor_texture_layout() -> ActorTextureLayout {
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
        }
    }

    fn test_actor_texture_atlas_size() -> [u32; 2] {
        [65, 32]
    }

    fn test_player_figure() -> ActorFigure {
        let json = include_str!("../../../../assets/mclone/figures/player.figure.json");
        let asset: mclone_assets::FigureAsset = serde_json::from_str(json).unwrap();
        crate::asset_lab_figure::compile_figure_asset(&asset).unwrap()
    }

    fn test_player_figures() -> ActorFigureSet {
        ActorFigureSet::new([(default_player_figure_id(), test_player_figure())])
    }

    fn test_chicken_figure() -> ActorFigure {
        let json = include_str!("../../../../assets/mclone/figures/chicken.figure.json");
        let asset: mclone_assets::FigureAsset = serde_json::from_str(json).unwrap();
        crate::asset_lab_figure::compile_figure_asset(&asset).unwrap()
    }

    fn test_chicken_figures() -> ActorFigureSet {
        ActorFigureSet::new([(mclone_assets::chicken_figure_id(), test_chicken_figure())])
    }

    #[test]
    #[ignore = "GPU ownership characterization; run explicitly on a host with a wgpu adapter"]
    fn actor_resource_snapshot_prices_whole_list_cow_and_player_updates() -> Result<()> {
        let (device, queue) = crate::headless::create_headless_device()?;
        let [width, height] = test_actor_texture_atlas_size();
        let rgba = vec![255; width as usize * height as usize * 4];
        let figures = test_player_figures();
        let mut resources = ActorDrawResources::new(
            &device,
            &queue,
            crate::headless::HEADLESS_FORMAT,
            ActorTextureAtlas {
                width,
                height,
                rgba: &rgba,
                layout: test_actor_texture_layout(),
            },
            Some(&figures),
        )?;

        let initial = resources.resource_snapshot();
        assert_eq!(initial.shared_strong_owner_count, 1);
        assert_eq!(initial.atlas_size, [65, 32]);
        assert_eq!(initial.atlas_base_bytes, 8_320);
        assert_eq!(initial.figure_count, 1);
        assert_eq!(initial.direct_pipeline_count, 1);
        assert_eq!(initial.direct_uniform_payload_bytes, 128);
        assert_eq!(initial.direct_uniform_slot_count, 6);
        assert_eq!(initial.multiview_pipeline_count, 0);
        assert_eq!(initial.multiview_uniform_allocated_bytes, 0);
        assert_eq!(initial.mesh.cpu_vertex_capacity_bytes, 327_680);
        assert_eq!(initial.mesh.cpu_index_capacity_bytes, 49_152);
        assert_eq!(initial.mesh.cpu_vertex_staging_capacity_bytes, 327_680);
        assert_eq!(initial.mesh.cpu_index_staging_capacity_bytes, 49_152);
        assert_eq!(initial.mesh.gpu_vertex_capacity_bytes, 524_288);
        assert_eq!(initial.mesh.gpu_index_capacity_bytes, 65_536);

        let mut actors = vec![
            ActorInstance::cow_model(Vec3::new(1.0, 2.0, 3.0), 0.0, 0.9, 1.4),
            ActorInstance::remote_player(Vec3::new(4.0, 2.0, 3.0), 0.0),
        ];
        let prepare = |resources: &mut ActorDrawResources, actors: &[ActorInstance]| {
            let prepared = resources.mesh_cache.prepare(
                &device,
                &queue,
                actors,
                resources.shared.texture_layout,
                resources.shared.atlas_size,
                &resources.shared.actor_figures,
            );
            assert!(prepared.is_some());
        };

        prepare(&mut resources, &actors);
        let first = resources.resource_snapshot();
        assert_eq!(first.mesh.cached_actor_count, 2);
        assert_eq!(first.mesh.rebuild_count, 1);
        assert_eq!(first.mesh.upload_count, 1);
        assert!(first.mesh.last_uploaded_bytes > 0);

        prepare(&mut resources, &actors);
        let unchanged = resources.resource_snapshot();
        assert_eq!(unchanged.mesh.rebuild_count, 1);
        assert_eq!(unchanged.mesh.upload_count, 1);

        actors[0].feet_position.x += 0.25;
        prepare(&mut resources, &actors);
        let cow_moved = resources.resource_snapshot();
        assert_eq!(cow_moved.mesh.rebuild_count, 2);
        assert_eq!(cow_moved.mesh.upload_count, 2);

        actors[1].feet_position.x += 0.25;
        prepare(&mut resources, &actors);
        let player_moved = resources.resource_snapshot();
        assert_eq!(player_moved.mesh.rebuild_count, 3);
        assert_eq!(player_moved.mesh.upload_count, 3);
        assert_eq!(
            player_moved.mesh.total_uploaded_bytes,
            player_moved.mesh.last_uploaded_bytes * 3
        );
        Ok(())
    }

    #[test]
    #[ignore = "GPU actor-cache isolation; run explicitly on a host with a wgpu adapter"]
    fn compatible_world_actor_states_share_topology_without_cache_thrash() -> Result<()> {
        let (device, queue) = crate::headless::create_headless_device()?;
        let [width, height] = test_actor_texture_atlas_size();
        let rgba = vec![255; width as usize * height as usize * 4];
        let mut world_a = ActorDrawResources::new(
            &device,
            &queue,
            crate::headless::HEADLESS_FORMAT,
            ActorTextureAtlas {
                width,
                height,
                rgba: &rgba,
                layout: test_actor_texture_layout(),
            },
            Some(&test_player_figures()),
        )?;
        let mut world_b =
            ActorDrawResources::new_with_shared_resources(&device, world_a.shared_resources());
        assert!(world_a.shares_immutable_resources_with(&world_b));
        assert_eq!(world_a.resource_snapshot().shared_strong_owner_count, 2);
        assert_eq!(world_b.resource_snapshot().shared_strong_owner_count, 2);
        assert_eq!(world_a.resource_snapshot().atlas_base_bytes, 8_320);
        assert_eq!(world_b.resource_snapshot().atlas_base_bytes, 8_320);

        let actors_a = [ActorInstance::remote_player(Vec3::new(1.0, 2.0, 3.0), 0.0)];
        let actors_b = [ActorInstance::remote_player(Vec3::new(9.0, 2.0, 3.0), 0.0)];
        let prepare = |resources: &mut ActorDrawResources, actors: &[ActorInstance]| {
            assert!(
                resources
                    .mesh_cache
                    .prepare(
                        &device,
                        &queue,
                        actors,
                        resources.shared.texture_layout,
                        resources.shared.atlas_size,
                        &resources.shared.actor_figures,
                    )
                    .is_some()
            );
        };
        prepare(&mut world_a, &actors_a);
        prepare(&mut world_b, &actors_b);
        prepare(&mut world_a, &actors_a);
        assert_eq!(world_a.resource_snapshot().mesh.rebuild_count, 1);
        assert_eq!(world_a.resource_snapshot().mesh.upload_count, 1);
        assert_eq!(world_b.resource_snapshot().mesh.rebuild_count, 1);
        assert_eq!(world_b.resource_snapshot().mesh.upload_count, 1);
        assert_ne!(world_a.mesh_cache.actors, world_b.mesh_cache.actors);
        Ok(())
    }

    fn test_single_box_figure() -> ActorFigure {
        let asset: mclone_assets::FigureAsset = serde_json::from_str(
            r##"{
                "schemaVersion": 1,
                "name": "winding_test",
                "materials": { "body": { "color": "#ffffff" } },
                "textures": {},
                "parts": [{
                    "name": "body",
                    "material": "body",
                    "primitive": { "kind": "box", "size": [1, 1, 1] }
                }],
                "clips": {}
            }"##,
        )
        .unwrap();
        crate::asset_lab_figure::compile_figure_asset(&asset).unwrap()
    }

    #[test]
    fn actor_mesh_emits_asset_lab_player_model() {
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::new(1.0, 2.0, 3.0), 0.0)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(mesh.vertices.len(), 76 * 6 * 4);
        assert_eq!(mesh.indices.len(), 76 * 6 * 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.uv == [0.5 / 65.0, 0.5 / 32.0])
        );

        let bounds = mesh_bounds(&mesh);
        assert!((bounds.min.y - 2.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 3.8).abs() < 1.0e-6);
        assert!(bounds.min.x < 0.56);
        assert!(bounds.max.x > 1.44);
        assert!(bounds.max.z > 3.18);
    }

    #[test]
    fn asset_lab_box_winding_faces_outward_after_handedness_conversion() {
        let figures = ActorFigureSet::new([(default_player_figure_id(), test_single_box_figure())]);
        let mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::ZERO, 0.0)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let bounds = mesh_bounds(&mesh);
        let box_center = (bounds.min + bounds.max) * 0.5;

        assert_eq!(mesh.vertices.len(), 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6);
        for face_index in 0..6 {
            let vertex_start = face_index * 4;
            let index_start = face_index * 6;
            let a = Vec3::from_array(mesh.vertices[mesh.indices[index_start] as usize].position);
            let b =
                Vec3::from_array(mesh.vertices[mesh.indices[index_start + 1] as usize].position);
            let c =
                Vec3::from_array(mesh.vertices[mesh.indices[index_start + 2] as usize].position);
            let face_center = mesh.vertices[vertex_start..vertex_start + 4]
                .iter()
                .map(|vertex| Vec3::from_array(vertex.position))
                .sum::<Vec3>()
                * 0.25;
            let normal = (b - a).cross(c - a);

            assert!(
                normal.dot(face_center - box_center) > 0.0,
                "asset-lab box face {face_index} points inward"
            );
        }
    }

    #[test]
    fn asset_lab_player_model_has_front_face_details() {
        let actor = ActorInstance::remote_player(Vec3::ZERO, 0.0);
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let dark_detail_vertices = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.color == [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
            .count();

        assert_eq!(dark_detail_vertices, 8 * 6 * 4);
        assert!(
            mesh.vertices
                .iter()
                .filter(|vertex| vertex.color == [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
                .all(|vertex| vertex.position[2] > 0.18)
        );
    }

    #[test]
    fn asset_lab_player_walk_animation_changes_vertices() {
        let figures = test_player_figures();
        let static_mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::ZERO, 0.0)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let animated_mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::ZERO, 0.0).with_walk_animation_distance(0.25)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(animated_mesh.vertices.len(), static_mesh.vertices.len());
        assert_eq!(animated_mesh.indices.len(), static_mesh.indices.len());
        assert_ne!(animated_mesh.vertices, static_mesh.vertices);
    }

    #[test]
    fn asset_lab_chicken_wing_flap_changes_vertices() {
        let figures = test_chicken_figures();
        let static_actor = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::chicken_figure_id(),
        )
        .with_dimensions(0.4, 0.7);
        let static_mesh = actor_mesh(
            &[static_actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let flapping_mesh = actor_mesh(
            &[static_actor.with_chicken_wing_flap_radians(Some(0.45))],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(flapping_mesh.vertices.len(), static_mesh.vertices.len());
        assert_eq!(flapping_mesh.indices.len(), static_mesh.indices.len());
        assert_ne!(flapping_mesh.vertices, static_mesh.vertices);
    }

    #[test]
    fn first_person_asset_lab_player_model_hides_head_and_face_details() {
        let actor = ActorInstance::local_player(Vec3::ZERO, 0.0).with_first_person_body_only(true);
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(mesh.vertices.len(), 10 * 6 * 4);
        assert_eq!(mesh.indices.len(), 10 * 6 * 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.color != [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
        );

        let bounds = mesh_bounds(&mesh);
        assert!(bounds.max.y < 1.62);
    }

    #[test]
    fn actor_mesh_still_emits_hardcoded_humanoid_debug_shape() {
        let actor = ActorInstance {
            shape: ActorInstanceShape::Humanoid,
            ..ActorInstance::remote_player(Vec3::ZERO, 0.0)
        };
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 12 * 6 * 4);
        assert_eq!(mesh.indices.len(), 12 * 6 * 6);
    }

    #[test]
    fn actor_mesh_uses_humanoid_fallback_for_unknown_figure_ids() {
        let actor = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::ActorFigureId::from_static("mclone:missing"),
        );
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 12 * 6 * 4);
        assert_eq!(mesh.indices.len(), 12 * 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_quadruped_placeholder() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_placeholder(
                Vec3::new(1.0, 2.0, 3.0),
                0.0,
                0.9,
                1.4,
            )],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 6 * 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_debug_cube() {
        let mesh = actor_mesh(
            &[ActorInstance::debug_cube(
                Vec3::ZERO,
                0.0,
                0.0,
                None,
                1.0,
                1.0,
            )],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_item_egg() {
        let mesh = actor_mesh(
            &[ActorInstance::item_egg(Vec3::ZERO, 0.0, 0.25, 0.25)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 3 * 6 * 4);
        assert_eq!(mesh.indices.len(), 3 * 6 * 6);
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.color != mesh.vertices[0].color)
        );
    }

    #[test]
    fn debug_cube_pitch_rotates_around_center() {
        let actor = ActorInstance::debug_cube(Vec3::ZERO, 0.0, 90.0, None, 1.0, 1.0);
        let bottom_center = actor_world_position(actor, Vec3::new(0.0, 0.0, 0.0));
        let top_center = actor_world_position(actor, Vec3::new(0.0, 1.0, 0.0));

        assert!((bottom_center.y - 0.5).abs() < 1.0e-6);
        assert!((top_center.y - 0.5).abs() < 1.0e-6);
        assert!((bottom_center.z + 0.5).abs() < 1.0e-6);
        assert!((top_center.z - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn debug_cube_quaternion_orientation_rotates_around_center() {
        let actor = ActorInstance::debug_cube(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(Quat::from_rotation_z(90.0_f32.to_radians())),
            1.0,
            1.0,
        );
        let left_center = actor_world_position(actor, Vec3::new(-0.5, 0.5, 0.0));
        let right_center = actor_world_position(actor, Vec3::new(0.5, 0.5, 0.0));

        assert!((left_center.x - 0.0).abs() < 1.0e-6);
        assert!((right_center.x - 0.0).abs() < 1.0e-6);
        assert!((left_center.y - 0.0).abs() < 1.0e-6);
        assert!((right_center.y - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn actor_uniform_serializes_underwater_fog() {
        let render_view = crate::chunk::ChunkCamera {
            eye: [4.0, 5.0, 6.0],
            target: [4.0, 5.0, 5.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default().with_fog(crate::fog::RenderFog::underwater()),
        );

        assert_eq!(bytes.len(), UNIFORM_BYTE_LEN);
        assert_eq!(
            f32::from_ne_bytes(bytes[72..76].try_into().unwrap()).to_bits() & 3,
            crate::fog::RenderFogMode::Linear as u32
        );
        assert_eq!(f32::from_ne_bytes(bytes[80..84].try_into().unwrap()), 4.0);
        assert_eq!(f32::from_ne_bytes(bytes[84..88].try_into().unwrap()), 5.0);
        assert_eq!(f32::from_ne_bytes(bytes[88..92].try_into().unwrap()), 6.0);
        assert_eq!(
            f32::from_ne_bytes(bytes[96..100].try_into().unwrap()),
            5.0 / 255.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[112..116].try_into().unwrap()),
            -8.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[116..120].try_into().unwrap()),
            96.0
        );
    }

    #[test]
    fn actor_multiview_uniform_serializes_distinct_left_right_views() {
        let left = crate::chunk::ChunkCamera {
            eye: [1.0, 2.0, 3.0],
            target: [1.0, 2.0, 2.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let right = crate::chunk::ChunkCamera {
            eye: [4.0, 5.0, 6.0],
            target: [4.0, 5.0, 5.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 75.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let options = [
            TexturedSectionRenderOptions::default(),
            TexturedSectionRenderOptions::default().with_fog(crate::fog::RenderFog::underwater()),
        ];

        let bytes = multiview_uniform_bytes([left, right], options);

        assert_eq!(bytes.len(), MULTIVIEW_UNIFORM_BYTE_LEN);
        assert_eq!(
            &bytes[..UNIFORM_BYTE_LEN],
            uniform_bytes(left, options[0]).as_slice()
        );
        assert_eq!(
            &bytes[UNIFORM_BYTE_LEN..],
            uniform_bytes(right, options[1]).as_slice()
        );
        assert_ne!(&bytes[..UNIFORM_BYTE_LEN], &bytes[UNIFORM_BYTE_LEN..]);
    }

    #[test]
    fn placed_actor_uniforms_append_rebased_placement_and_clip_plane() {
        use crate::placement::{CompositionHalfSpace, WorldPlacement};

        let render_view = crate::chunk::ChunkCamera {
            eye: [1.0, 2.0, 8.0],
            target: [0.0, 1.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 60.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let placement = WorldPlacement::new(
            Vec3d::new(1_000.0, 64.0, -2_000.0),
            Vec3d::new(4.0, 0.5, -3.0),
            0.125,
        )
        .unwrap();
        let clip = CompositionHalfSpace::new(Vec3::X, -2.0).unwrap();
        let context =
            WorldCompositionContext::new(placement, None, CompositionClip::HalfSpace(clip));
        let bytes = placed_uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default(),
            context,
        );

        assert_eq!(bytes.len(), PLACED_UNIFORM_BYTE_LEN);
        assert_eq!(
            &bytes[..UNIFORM_BYTE_LEN],
            uniform_bytes(render_view, TexturedSectionRenderOptions::default()).as_slice()
        );
        let appended = bytes[UNIFORM_BYTE_LEN..]
            .chunks_exact(4)
            .map(|value| f32::from_ne_bytes(value.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            appended,
            vec![
                1_000.0, 64.0, -2_000.0, 0.125, 4.0, 0.5, -3.0, 0.0, 1.0, 0.0, 0.0, -2.0,
            ]
        );
    }

    #[test]
    fn composed_actor_selection_preserves_order_and_rejects_before_mesh_build() {
        use crate::placement::{CompositionHalfSpace, WorldPlacement, WorldSourceBounds};

        let render_view = crate::chunk::ChunkCamera {
            eye: [0.0, 3.0, 18.0],
            target: [0.0, 2.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 55.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 100.0,
        }
        .render_view(960, 640);
        let placement =
            WorldPlacement::new(Vec3d::new(1_000.0, 64.0, 1_000.0), Vec3d::ZERO, 0.5).unwrap();
        let bounds = WorldSourceBounds::new(
            Vec3d::new(900.0, 0.0, 900.0),
            Vec3d::new(1_100.0, 400.0, 1_100.0),
        )
        .unwrap();
        let context = WorldCompositionContext::new(
            placement,
            Some(bounds),
            CompositionClip::HalfSpace(CompositionHalfSpace::new(-Vec3::X, 0.0).unwrap()),
        );
        let visible_first =
            ActorInstance::item_egg(Vec3::new(998.0, 64.0, 1_000.0), 0.0, 0.25, 0.25);
        let source_rejected =
            ActorInstance::item_egg(Vec3::new(1_200.0, 64.0, 1_000.0), 0.0, 0.25, 0.25);
        let visible_second = ActorInstance::remote_player(Vec3::new(1_000.0, 64.0, 1_000.0), 0.0);
        let clip_rejected =
            ActorInstance::debug_cube(Vec3::new(1_010.0, 64.0, 1_000.0), 0.0, 0.0, None, 1.0, 1.0);
        let frustum_rejected =
            ActorInstance::debug_cube(Vec3::new(995.0, 300.0, 1_000.0), 0.0, 0.0, None, 1.0, 1.0);
        let actors = [
            visible_first,
            source_rejected,
            visible_second,
            clip_rejected,
            frustum_rejected,
        ];
        let mut selected = Vec::new();
        let report = select_composed_actors(&mut selected, &actors, context, &[render_view]);

        assert_eq!(selected, vec![visible_first, visible_second]);
        assert_eq!(report.source_rejected_actor_count, 1);
        assert_eq!(report.clip_rejected_actor_count, 1);
        assert_eq!(report.frustum_rejected_actor_count, 1);
    }

    #[test]
    fn cow_model_uses_vanilla_cuboid_parts() {
        assert_eq!(COW_MODEL_PARTS.len(), 6);
        assert_eq!(COW_HEAD_CUBOIDS.len(), 3);
        assert_eq!(COW_BODY_CUBOIDS.len(), 2);
        assert_eq!(COW_LEG_CUBOIDS.len(), 1);
        assert_eq!(COW_MODEL_PARTS[0].offset_pixels, [0.0, 4.0, -8.0]);
        assert_eq!(
            COW_MODEL_PARTS[1].rotation_radians,
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0]
        );
        assert_eq!(COW_BODY_CUBOIDS[0].texture_offset, [18, 4]);
        assert_eq!(COW_BODY_CUBOIDS[1].texture_offset, [52, 0]);
    }

    #[test]
    fn actor_mesh_emits_cow_model_at_feet_position() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_model(Vec3::ZERO, 0.0, 0.9, 1.4)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 9 * 6 * 4);
        assert_eq!(mesh.indices.len(), 9 * 6 * 6);

        let bounds = mesh_bounds(&mesh);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 25.0 / 16.0).abs() < 1.0e-6);
        assert!(bounds.max.z > 0.85);
        assert!(bounds.min.z < -0.60);
    }

    #[test]
    fn cow_model_uses_cow_texture_region() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_model(Vec3::ZERO, 0.0, 0.9, 1.4)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );
        let white_uv = [0.5 / 65.0, 0.5 / 32.0];

        assert!(mesh.vertices.iter().any(|vertex| vertex.uv != white_uv));
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.uv[0] >= 1.0 / 65.0)
        );
        assert!(mesh.vertices.iter().all(|vertex| vertex.uv[0] <= 1.0));
    }

    #[test]
    fn remote_player_yaw_uses_java_sign_convention() {
        let actor = ActorInstance::remote_player(Vec3::ZERO, -90.0);
        let forward = actor_world_position(actor, Vec3::new(0.0, 0.0, 1.0));

        assert!((forward.x - 1.0).abs() < 1.0e-6);
        assert!(forward.y.abs() < 1.0e-6);
        assert!(forward.z.abs() < 1.0e-6);
    }

    #[test]
    fn actor_world_position_places_geometry_at_feet_position() {
        let actor = ActorInstance::remote_player(Vec3::new(10.0, 64.0, -4.0), 0.0);
        let world = actor_world_position(actor, Vec3::new(0.0, 1.8, 0.0));

        assert_eq!(world, Vec3::new(10.0, 65.8, -4.0));
    }

    #[test]
    fn legacy_actor_mesh_carries_whole_instance_opacity() {
        let actor =
            ActorInstance::debug_cube(Vec3::ZERO, 0.0, 0.0, None, 1.0, 1.0).with_opacity(0.35);
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert!(!mesh.vertices.is_empty());
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| (vertex.color[3] - 0.35).abs() < 1.0e-6)
        );
        assert_eq!(ActorInstance::remote_player(Vec3::ZERO, 0.0).opacity, 1.0);
        assert_eq!(
            ActorInstance::remote_player(Vec3::ZERO, 0.0)
                .with_opacity(2.0)
                .opacity,
            1.0
        );
    }

    #[test]
    fn legacy_actor_dither_shaders_validate_for_all_view_paths() {
        for (source, capabilities) in [
            (
                include_str!("shaders/entity_actor.wgsl"),
                naga::valid::Capabilities::empty(),
            ),
            (
                include_str!("shaders/entity_actor_placed.wgsl"),
                naga::valid::Capabilities::empty(),
            ),
            (
                include_str!("shaders/entity_actor_multiview.wgsl"),
                naga::valid::Capabilities::MULTIVIEW,
            ),
            (
                include_str!("shaders/entity_actor_placed_multiview.wgsl"),
                naga::valid::Capabilities::MULTIVIEW,
            ),
        ] {
            let source = crate::fog::inject_fog_wgsl(source);
            let module = naga::front::wgsl::parse_str(&source).expect("actor WGSL parses");
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
                .validate(&module)
                .expect("actor WGSL validates");
            assert!(source.contains("coverage_hash"));
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Bounds {
        min: Vec3,
        max: Vec3,
    }

    fn mesh_bounds(mesh: &ActorMesh) -> Bounds {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &mesh.vertices {
            let position = Vec3::from_array(vertex.position);
            min = min.min(position);
            max = max.max(position);
        }
        Bounds { min, max }
    }
}
