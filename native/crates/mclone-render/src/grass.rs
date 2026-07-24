use std::cell::{Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use mclone_core::{BlockPos, CHUNK_WIDTH, HorizontalTopology, Vec3d};
use mclone_mesh::{GrassPatch, RenderSectionKey, TexturedRenderSectionMesh};

use crate::chunk::DEPTH_FORMAT;

pub(crate) const STATIC_GRASS_BLADE_COUNT: u32 = 8;
const GRASS_VERTICES_PER_BLADE: u32 = 12;
const GRASS_PATCH_MIN_CAPACITY: u32 = 4_096;
const GRASS_PIPELINE_VARIANT_COUNT: usize = 6;
const GRASS_FRAME_UNIFORM_SIZE: u64 = 32;
const GRASS_INTERACTION_UNIFORM_SIZE: u64 = 32;
const GRASS_INTERACTION_FIELD_SIZE: u32 = 128;
const GRASS_INTERACTION_CELL_SIZE: f64 = 0.5;
const GRASS_INTERACTION_FIELD_BYTE_LEN: usize =
    GRASS_INTERACTION_FIELD_SIZE as usize * GRASS_INTERACTION_FIELD_SIZE as usize * 4;
const GRASS_INTERACTION_FIELD_POOL_SIZE: usize = 4;
pub const MAX_GRASS_INTERACTORS: usize = 24;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GrassDrawStats {
    pub resident_patch_count: u32,
    pub drawn_patch_count: u32,
    pub estimated_blade_count: u32,
    pub draw_calls: usize,
    pub resident_bytes: u64,
    pub near_patch_count: u32,
    pub middle_patch_count: u32,
    pub far_patch_count: u32,
    pub interaction_field_count: u32,
    pub interaction_active_cell_count: u32,
    pub interaction_stamp_count: u32,
    pub interaction_recenter_count: u32,
    pub interaction_reset_count: u32,
    pub interaction_uploaded_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GrassUploadStats {
    pub uploaded_patch_count: u32,
    pub removed_patch_count: u32,
    pub uploaded_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrassInteractor {
    pub identity: GrassInteractorIdentity,
    pub feet_position: [f32; 3],
    pub footprint_radius: f32,
}

impl GrassInteractor {
    pub fn new(
        identity: GrassInteractorIdentity,
        feet_position: [f32; 3],
        footprint_radius: f32,
    ) -> Option<Self> {
        (feet_position.into_iter().all(f32::is_finite) && footprint_radius.is_finite()).then_some(
            Self {
                identity,
                feet_position,
                footprint_radius: footprint_radius.clamp(0.2, 2.0),
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrassInteractorIdentity {
    #[default]
    LocalPlayer,
    RemotePlayer(u64),
    Entity(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassInteractorSet {
    entries: [GrassInteractor; MAX_GRASS_INTERACTORS],
    len: u8,
}

impl Default for GrassInteractorSet {
    fn default() -> Self {
        Self {
            entries: [GrassInteractor::default(); MAX_GRASS_INTERACTORS],
            len: 0,
        }
    }
}

impl GrassInteractorSet {
    pub fn push(&mut self, interactor: GrassInteractor) {
        if let Some(existing) = self
            .entries
            .get_mut(..usize::from(self.len))
            .and_then(|entries| {
                entries
                    .iter_mut()
                    .find(|entry| entry.identity == interactor.identity)
            })
        {
            *existing = interactor;
            return;
        }
        let index = usize::from(self.len);
        if index < MAX_GRASS_INTERACTORS {
            self.entries[index] = interactor;
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = GrassInteractor> + '_ {
        self.entries[..usize::from(self.len)].iter().copied()
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrassQuality {
    #[default]
    Off,
    Sparse,
    Lush,
    Ultra,
}

impl GrassQuality {
    pub const fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    const fn profile(self) -> Option<GrassQualityProfile> {
        match self {
            Self::Off => None,
            Self::Sparse => Some(GrassQualityProfile {
                radius_blocks: 64.0,
                near_end_blocks: 24.0,
                middle_end_blocks: 48.0,
                near_blades: 2,
                middle_blades: 1,
                far_blades: 1,
                wind_amplitude: 0.10,
                interaction_enabled: false,
            }),
            Self::Lush => Some(GrassQualityProfile {
                radius_blocks: 128.0,
                near_end_blocks: 48.0,
                middle_end_blocks: 96.0,
                near_blades: 6,
                middle_blades: 4,
                far_blades: 2,
                wind_amplitude: 0.14,
                interaction_enabled: true,
            }),
            Self::Ultra => Some(GrassQualityProfile {
                radius_blocks: 192.0,
                near_end_blocks: 64.0,
                middle_end_blocks: 128.0,
                near_blades: 8,
                middle_blades: 6,
                far_blades: 3,
                wind_amplitude: 0.17,
                interaction_enabled: true,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GrassQualityProfile {
    radius_blocks: f32,
    near_end_blocks: f32,
    middle_end_blocks: f32,
    near_blades: u32,
    middle_blades: u32,
    far_blades: u32,
    wind_amplitude: f32,
    interaction_enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum GrassLodTier {
    Near,
    Middle,
    Far,
    #[default]
    Hidden,
}

#[derive(Default)]
struct GrassObserverLodHistory {
    quality: GrassQuality,
    tiers: BTreeMap<RenderSectionKey, GrassLodTier>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GrassPipelineVariant {
    Direct,
    DirectMultiview,
    Placed,
    PlacedMultiview,
    ClippedPlaced,
    ClippedPlacedMultiview,
}

impl GrassPipelineVariant {
    const fn index(self) -> usize {
        match self {
            Self::Direct => 0,
            Self::DirectMultiview => 1,
            Self::Placed => 2,
            Self::PlacedMultiview => 3,
            Self::ClippedPlaced => 4,
            Self::ClippedPlacedMultiview => 5,
        }
    }

    const fn multiview(self) -> Option<NonZeroU32> {
        match self {
            Self::DirectMultiview | Self::PlacedMultiview | Self::ClippedPlacedMultiview => {
                NonZeroU32::new(2)
            }
            Self::Direct | Self::Placed | Self::ClippedPlaced => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Direct => "mclone_grass_direct_pipeline",
            Self::DirectMultiview => "mclone_grass_direct_multiview_pipeline",
            Self::Placed => "mclone_grass_placed_pipeline",
            Self::PlacedMultiview => "mclone_grass_placed_multiview_pipeline",
            Self::ClippedPlaced => "mclone_grass_clipped_placed_pipeline",
            Self::ClippedPlacedMultiview => "mclone_grass_clipped_placed_multiview_pipeline",
        }
    }
}

/// Lazily materialized immutable grass pipelines shared by compatible world
/// draw stores. The default Off path owns this empty cache but compiles no grass
/// shader and allocates no grass GPU buffer.
pub(crate) struct GrassPipelineCache {
    color_format: wgpu::TextureFormat,
    frame_layout: OnceLock<wgpu::BindGroupLayout>,
    interaction_layout: OnceLock<wgpu::BindGroupLayout>,
    pipelines: RefCell<[Option<wgpu::RenderPipeline>; GRASS_PIPELINE_VARIANT_COUNT]>,
}

impl GrassPipelineCache {
    pub(crate) fn new(color_format: wgpu::TextureFormat) -> Self {
        Self {
            color_format,
            frame_layout: OnceLock::new(),
            interaction_layout: OnceLock::new(),
            pipelines: RefCell::new(std::array::from_fn(|_| None)),
        }
    }

    pub(crate) fn frame_layout<'a>(&'a self, device: &wgpu::Device) -> &'a wgpu::BindGroupLayout {
        self.frame_layout
            .get_or_init(|| create_grass_frame_layout(device))
    }

    pub(crate) fn interaction_layout<'a>(
        &'a self,
        device: &wgpu::Device,
    ) -> &'a wgpu::BindGroupLayout {
        self.interaction_layout
            .get_or_init(|| create_grass_interaction_layout(device))
    }

    pub(crate) fn pipeline<'a>(
        &'a self,
        device: &wgpu::Device,
        variant: GrassPipelineVariant,
        uniform_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Ref<'a, wgpu::RenderPipeline> {
        let index = variant.index();
        if self.pipelines.borrow()[index].is_none() {
            let frame_layout = self.frame_layout(device);
            let interaction_layout = self.interaction_layout(device);
            let pipeline = create_grass_pipeline(
                device,
                self.color_format,
                variant,
                uniform_layout,
                texture_layout,
                frame_layout,
                interaction_layout,
            );
            self.pipelines.borrow_mut()[index] = Some(pipeline);
        }
        Ref::map(self.pipelines.borrow(), move |pipelines| {
            pipelines[index]
                .as_ref()
                .expect("grass pipeline initialized above")
        })
    }

    pub(crate) fn cached_pipeline(
        &self,
        variant: GrassPipelineVariant,
    ) -> Ref<'_, wgpu::RenderPipeline> {
        let index = variant.index();
        Ref::map(self.pipelines.borrow(), move |pipelines| {
            pipelines[index]
                .as_ref()
                .expect("grass pipeline must be materialized before drawing")
        })
    }
}

fn create_grass_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    variant: GrassPipelineVariant,
    uniform_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
    frame_layout: &wgpu::BindGroupLayout,
    interaction_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let source = grass_shader_source(variant);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(variant.label()),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(variant.label()),
        bind_group_layouts: &[
            uniform_layout,
            texture_layout,
            frame_layout,
            interaction_layout,
        ],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(variant.label()),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: GrassPatch::BYTE_SIZE as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Sint32x3,
                    },
                    wgpu::VertexAttribute {
                        offset: 12,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Uint32,
                    },
                    wgpu::VertexAttribute {
                        offset: 16,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Uint32,
                    },
                    wgpu::VertexAttribute {
                        offset: 20,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Uint32,
                    },
                    wgpu::VertexAttribute {
                        offset: 24,
                        shader_location: 4,
                        format: wgpu::VertexFormat::Uint32,
                    },
                    wgpu::VertexAttribute {
                        offset: 28,
                        shader_location: 5,
                        format: wgpu::VertexFormat::Uint32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
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
        multiview: variant.multiview(),
        cache: None,
    })
}

fn create_grass_frame_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mclone_grass_frame_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(GRASS_FRAME_UNIFORM_SIZE),
            },
            count: None,
        }],
    })
}

fn create_grass_interaction_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mclone_grass_interaction_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(GRASS_INTERACTION_UNIFORM_SIZE),
                },
                count: None,
            },
        ],
    })
}

fn grass_shader_source(variant: GrassPipelineVariant) -> String {
    match variant {
        GrassPipelineVariant::Direct => include_str!("shaders/grass.wgsl").to_owned(),
        GrassPipelineVariant::DirectMultiview => {
            include_str!("shaders/grass_multiview.wgsl").to_owned()
        }
        GrassPipelineVariant::Placed => include_str!("shaders/grass_placed.wgsl").to_owned(),
        GrassPipelineVariant::PlacedMultiview => {
            include_str!("shaders/grass_placed_multiview.wgsl").to_owned()
        }
        GrassPipelineVariant::ClippedPlaced => clipped_placed_grass_shader_source(),
        GrassPipelineVariant::ClippedPlacedMultiview => {
            clipped_placed_multiview_grass_shader_source()
        }
    }
}

fn clipped_placed_grass_shader_source() -> String {
    let source = include_str!("shaders/grass_placed.wgsl");
    let source = source.replacen(
        "    composition_anchor: vec4<f32>,\n};",
        "    composition_anchor: vec4<f32>,\n    clip_plane: vec4<f32>,\n};",
        1,
    );
    let source = source.replacen(
        "fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n",
        concat!(
            "fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n",
            "    if (dot(uniforms.clip_plane.xyz, input.composition_position) ",
            "+ uniforms.clip_plane.w < 0.0) {\n",
            "        discard;\n",
            "    }\n",
        ),
        1,
    );
    assert_eq!(source.matches("clip_plane: vec4<f32>").count(), 1);
    assert_eq!(source.matches("uniforms.clip_plane").count(), 2);
    source
}

fn clipped_placed_multiview_grass_shader_source() -> String {
    let source = include_str!("shaders/grass_placed_multiview.wgsl");
    let source = source.replacen(
        "    composition_anchor: vec4<f32>,\n};",
        "    composition_anchor: vec4<f32>,\n    clip_plane: vec4<f32>,\n};",
        1,
    );
    let source = source.replacen(
        "    let color = apply_color_profile(\n",
        concat!(
            "    if (dot(uniforms.clip_plane.xyz, input.composition_position) ",
            "+ uniforms.clip_plane.w < 0.0) {\n",
            "        discard;\n",
            "    }\n",
            "    let color = apply_color_profile(\n",
        ),
        1,
    );
    assert_eq!(source.matches("clip_plane: vec4<f32>").count(), 1);
    assert_eq!(source.matches("uniforms.clip_plane").count(), 2);
    source
}

#[derive(Debug)]
struct GrassRangeAllocator {
    capacity: u32,
    free: BTreeMap<u32, u32>,
}

impl GrassRangeAllocator {
    fn new(capacity: u32) -> Self {
        Self {
            capacity,
            free: BTreeMap::from([(0, capacity)]),
        }
    }

    fn allocate(&mut self, count: u32) -> Option<Range<u32>> {
        if count == 0 {
            return Some(0..0);
        }
        let (start, available) = self
            .free
            .iter()
            .find(|(_, available)| **available >= count)
            .map(|(start, available)| (*start, *available))?;
        self.free.remove(&start);
        if available > count {
            self.free.insert(start + count, available - count);
        }
        Some(start..start + count)
    }

    fn release(&mut self, range: Range<u32>) {
        if range.is_empty() {
            return;
        }
        let mut start = range.start;
        let mut end = range.end;
        if let Some((previous_start, previous_count)) = self
            .free
            .range(..=start)
            .next_back()
            .map(|(start, count)| (*start, *count))
            && previous_start + previous_count == start
        {
            self.free.remove(&previous_start);
            start = previous_start;
        }
        if let Some((next_start, next_count)) = self
            .free
            .range(start..)
            .next()
            .map(|(start, count)| (*start, *count))
            && end == next_start
        {
            self.free.remove(&next_start);
            end = next_start + next_count;
        }
        self.free.insert(start, end - start);
    }

    fn extend(&mut self, capacity: u32) {
        let previous = self.capacity;
        self.capacity = capacity;
        self.release(previous..capacity);
    }
}

struct GrassPatchArena {
    buffer: wgpu::Buffer,
    ranges: GrassRangeAllocator,
    maximum_capacity: u32,
}

impl GrassPatchArena {
    fn new(device: &wgpu::Device, required: u32) -> Result<Self> {
        let maximum_capacity =
            u32::try_from(device.limits().max_buffer_size / GrassPatch::BYTE_SIZE as u64)
                .unwrap_or(u32::MAX);
        let requested = required.max(GRASS_PATCH_MIN_CAPACITY);
        if requested > maximum_capacity {
            bail!(
                "grass patch arena request {requested} exceeds adapter capacity {maximum_capacity}"
            );
        }
        let capacity = requested
            .checked_next_power_of_two()
            .unwrap_or(maximum_capacity)
            .min(maximum_capacity);
        Ok(Self {
            buffer: create_grass_patch_buffer(device, capacity),
            ranges: GrassRangeAllocator::new(capacity),
            maximum_capacity,
        })
    }

    fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: u32,
    ) -> Result<Range<u32>> {
        if let Some(range) = self.ranges.allocate(count) {
            return Ok(range);
        }
        let minimum = self
            .ranges
            .capacity
            .checked_add(count)
            .context("grass patch arena element count overflow")?;
        if minimum > self.maximum_capacity {
            bail!(
                "grass patch arena exhausted at {} patches (requested {count})",
                self.ranges.capacity
            );
        }
        let capacity = self
            .ranges
            .capacity
            .saturating_mul(2)
            .max(minimum)
            .checked_next_power_of_two()
            .unwrap_or(self.maximum_capacity)
            .min(self.maximum_capacity);
        let new_buffer = create_grass_patch_buffer(device, capacity);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_grass_patch_arena_grow"),
        });
        encoder.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &new_buffer,
            0,
            u64::from(self.ranges.capacity) * GrassPatch::BYTE_SIZE as u64,
        );
        queue.submit(Some(encoder.finish()));
        self.buffer = new_buffer;
        self.ranges.extend(capacity);
        self.ranges
            .allocate(count)
            .context("grown grass patch arena must fit requested range")
    }
}

fn create_grass_patch_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_grass_patch_arena"),
        size: u64::from(capacity) * GrassPatch::BYTE_SIZE as u64,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

struct GrassFrameResources {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    empty_interaction: GrassInteractionGpu,
}

impl GrassFrameResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_layout: &wgpu::BindGroupLayout,
        interaction_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_grass_frame_uniform"),
            size: GRASS_FRAME_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_grass_frame_bind_group"),
            layout: frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let empty_interaction = GrassInteractionGpu::new(
            device,
            interaction_layout,
            1,
            "mclone_grass_empty_interaction",
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &empty_interaction.texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            &[0, 0, 0, 0],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        queue.write_buffer(
            &empty_interaction.uniform_buffer,
            0,
            &grass_interaction_uniform_bytes([0.0, 0.0], HorizontalTopology::UNBOUNDED, false),
        );
        Self {
            buffer,
            bind_group,
            empty_interaction,
        }
    }
}

struct GrassInteractionGpu {
    texture: wgpu::Texture,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl GrassInteractionGpu {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        size: u32,
        label: &'static str,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_grass_interaction_uniform"),
            size: GRASS_INTERACTION_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_grass_interaction_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            texture,
            uniform_buffer,
            bind_group,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct GrassInteractionSnapshot {
    active_cell_count: u32,
    stamp_count: u32,
    recenter_count: u32,
    reset_count: u32,
    uploaded_bytes: u64,
}

struct GrassInteractionField {
    gpu: GrassInteractionGpu,
    pixels: Vec<u8>,
    origin_cells: [i64; 2],
    center: Option<Vec3d>,
    topology: HorizontalTopology,
    last_time_seconds: Option<f32>,
    interactor_positions: BTreeMap<GrassInteractorIdentity, Vec3d>,
    snapshot: GrassInteractionSnapshot,
}

impl GrassInteractionField {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            gpu: GrassInteractionGpu::new(
                device,
                layout,
                GRASS_INTERACTION_FIELD_SIZE,
                "mclone_grass_interaction_field",
            ),
            pixels: vec![0; GRASS_INTERACTION_FIELD_BYTE_LEN],
            origin_cells: [0, 0],
            center: None,
            topology: HorizontalTopology::UNBOUNDED,
            last_time_seconds: None,
            interactor_positions: BTreeMap::new(),
            snapshot: GrassInteractionSnapshot::default(),
        }
    }

    fn update(
        &mut self,
        queue: &wgpu::Queue,
        time_seconds: f32,
        observer_position: [f32; 3],
        topology: HorizontalTopology,
        interactors: GrassInteractorSet,
        surface_roots: &BTreeSet<(i32, i32, i32)>,
    ) {
        let raw_observer = Vec3d::new(
            f64::from(observer_position[0]),
            f64::from(observer_position[1]),
            f64::from(observer_position[2]),
        );
        let observer = self.center.map_or(raw_observer, |center| {
            topology.nearest_position_lift(raw_observer, center)
        });
        let new_origin = [
            (observer.x / GRASS_INTERACTION_CELL_SIZE).floor() as i64
                - i64::from(GRASS_INTERACTION_FIELD_SIZE / 2),
            (observer.z / GRASS_INTERACTION_CELL_SIZE).floor() as i64
                - i64::from(GRASS_INTERACTION_FIELD_SIZE / 2),
        ];
        let duplicate_time = self
            .last_time_seconds
            .is_some_and(|last| (last - time_seconds).abs() <= f32::EPSILON);
        if duplicate_time && new_origin == self.origin_cells && topology == self.topology {
            return;
        }

        let mut snapshot = GrassInteractionSnapshot::default();
        if self.center.is_none() || topology != self.topology {
            self.clear();
            self.origin_cells = new_origin;
            snapshot.reset_count = 1;
        } else if new_origin != self.origin_cells {
            let delta = [
                new_origin[0] - self.origin_cells[0],
                new_origin[1] - self.origin_cells[1],
            ];
            if delta[0].unsigned_abs() >= u64::from(GRASS_INTERACTION_FIELD_SIZE / 2)
                || delta[1].unsigned_abs() >= u64::from(GRASS_INTERACTION_FIELD_SIZE / 2)
            {
                self.clear();
                snapshot.reset_count = 1;
            } else {
                self.shift(delta);
                snapshot.recenter_count = 1;
            }
            self.origin_cells = new_origin;
        }

        let dt = self.last_time_seconds.map_or(0.0, |last| {
            (time_seconds - last).rem_euclid(4_096.0).min(0.25)
        });
        if dt > 0.0 {
            self.decay(dt);
        }

        let mut current_identities = BTreeSet::new();
        for interactor in interactors.iter() {
            current_identities.insert(interactor.identity);
            let raw_position = Vec3d::new(
                f64::from(interactor.feet_position[0]),
                f64::from(interactor.feet_position[1]),
                f64::from(interactor.feet_position[2]),
            );
            let current = topology.nearest_position_lift(raw_position, observer);
            let previous = self
                .interactor_positions
                .get(&interactor.identity)
                .copied()
                .map(|previous| topology.nearest_position_lift(previous, current));
            let displacement = previous
                .map(|previous| current.subtract(previous))
                .unwrap_or(Vec3d::ZERO);
            let horizontal_distance =
                (displacement.x * displacement.x + displacement.z * displacement.z).sqrt();
            let sample_count = if horizontal_distance <= 8.0 {
                ((horizontal_distance / GRASS_INTERACTION_CELL_SIZE).ceil() as usize).max(1)
            } else {
                1
            };
            let motion = if horizontal_distance > 1.0e-4 && horizontal_distance <= 8.0 {
                [
                    (displacement.x / horizontal_distance) as f32,
                    (displacement.z / horizontal_distance) as f32,
                ]
            } else {
                [0.0, 0.0]
            };
            for sample in 1..=sample_count {
                let alpha = sample as f64 / sample_count as f64;
                let position =
                    previous.map_or(current, |previous| previous.add(displacement.scale(alpha)));
                if grass_surface_near(
                    surface_roots,
                    topology,
                    position,
                    f64::from(interactor.footprint_radius),
                ) {
                    self.stamp(position, f64::from(interactor.footprint_radius), motion);
                    snapshot.stamp_count += 1;
                }
            }
            self.interactor_positions
                .insert(interactor.identity, current);
        }
        self.interactor_positions
            .retain(|identity, _| current_identities.contains(identity));

        snapshot.active_cell_count = self
            .pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[2] != 0)
            .count() as u32;
        snapshot.uploaded_bytes = GRASS_INTERACTION_FIELD_BYTE_LEN as u64;
        self.center = Some(observer);
        self.topology = topology;
        self.last_time_seconds = Some(time_seconds);
        self.snapshot = snapshot;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.gpu.texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            &self.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(GRASS_INTERACTION_FIELD_SIZE * 4),
                rows_per_image: Some(GRASS_INTERACTION_FIELD_SIZE),
            },
            wgpu::Extent3d {
                width: GRASS_INTERACTION_FIELD_SIZE,
                height: GRASS_INTERACTION_FIELD_SIZE,
                depth_or_array_layers: 1,
            },
        );
        queue.write_buffer(
            &self.gpu.uniform_buffer,
            0,
            &grass_interaction_uniform_bytes(
                [
                    self.origin_cells[0] as f32 * GRASS_INTERACTION_CELL_SIZE as f32,
                    self.origin_cells[1] as f32 * GRASS_INTERACTION_CELL_SIZE as f32,
                ],
                topology,
                true,
            ),
        );
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
        self.interactor_positions.clear();
    }

    fn reset_for_pool(&mut self) {
        self.clear();
        self.origin_cells = [0, 0];
        self.center = None;
        self.topology = HorizontalTopology::UNBOUNDED;
        self.last_time_seconds = None;
        self.snapshot = GrassInteractionSnapshot::default();
    }

    fn shift(&mut self, delta: [i64; 2]) {
        let size = i64::from(GRASS_INTERACTION_FIELD_SIZE);
        let mut shifted = vec![0; GRASS_INTERACTION_FIELD_BYTE_LEN];
        for z in 0..size {
            for x in 0..size {
                let old_x = x + delta[0];
                let old_z = z + delta[1];
                if !(0..size).contains(&old_x) || !(0..size).contains(&old_z) {
                    continue;
                }
                let source = ((old_z * size + old_x) * 4) as usize;
                let target = ((z * size + x) * 4) as usize;
                shifted[target..target + 4].copy_from_slice(&self.pixels[source..source + 4]);
            }
        }
        self.pixels = shifted;
    }

    fn decay(&mut self, dt: f32) {
        let multiplier = (-dt / 0.85).exp();
        for pixel in self.pixels.chunks_exact_mut(4) {
            let strength = (f32::from(pixel[2]) * multiplier).round() as u8;
            pixel[2] = strength;
            if strength < 2 {
                pixel.fill(0);
            }
        }
    }

    fn stamp(&mut self, position: Vec3d, radius: f64, motion: [f32; 2]) {
        let radius = radius.max(GRASS_INTERACTION_CELL_SIZE);
        let min_x = ((position.x - radius) / GRASS_INTERACTION_CELL_SIZE).floor() as i64;
        let max_x = ((position.x + radius) / GRASS_INTERACTION_CELL_SIZE).ceil() as i64;
        let min_z = ((position.z - radius) / GRASS_INTERACTION_CELL_SIZE).floor() as i64;
        let max_z = ((position.z + radius) / GRASS_INTERACTION_CELL_SIZE).ceil() as i64;
        let size = i64::from(GRASS_INTERACTION_FIELD_SIZE);
        for world_z in min_z..=max_z {
            for world_x in min_x..=max_x {
                let x = world_x - self.origin_cells[0];
                let z = world_z - self.origin_cells[1];
                if !(0..size).contains(&x) || !(0..size).contains(&z) {
                    continue;
                }
                let cell_x = (world_x as f64 + 0.5) * GRASS_INTERACTION_CELL_SIZE;
                let cell_z = (world_z as f64 + 0.5) * GRASS_INTERACTION_CELL_SIZE;
                let dx = cell_x - position.x;
                let dz = cell_z - position.z;
                let distance = (dx * dx + dz * dz).sqrt();
                if distance > radius {
                    continue;
                }
                let radial = if distance > 1.0e-5 {
                    [(dx / distance) as f32, (dz / distance) as f32]
                } else {
                    motion
                };
                let mut direction = [
                    radial[0] * 0.72 + motion[0] * 0.58,
                    radial[1] * 0.72 + motion[1] * 0.58,
                ];
                let direction_length =
                    (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
                if direction_length > 1.0e-5 {
                    direction[0] /= direction_length;
                    direction[1] /= direction_length;
                }
                let falloff = (1.0 - distance as f32 / radius as f32).clamp(0.0, 1.0);
                let strength = (0.35 + falloff * 0.65).clamp(0.0, 1.0);
                let offset = ((z * size + x) * 4) as usize;
                let old_strength = f32::from(self.pixels[offset + 2]) / 255.0;
                if strength >= old_strength {
                    self.pixels[offset] = encode_signed_unit(direction[0]);
                    self.pixels[offset + 1] = encode_signed_unit(direction[1]);
                    self.pixels[offset + 2] = (strength * 255.0).round() as u8;
                    self.pixels[offset + 3] = 255;
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct GrassPatchDrawResources {
    arena: Option<GrassPatchArena>,
    frame: Option<GrassFrameResources>,
    sections: BTreeMap<RenderSectionKey, Range<u32>>,
    root_sections: BTreeMap<RenderSectionKey, Vec<[i32; 3]>>,
    surface_roots: BTreeSet<(i32, i32, i32)>,
    lod_histories: RefCell<BTreeMap<u32, GrassObserverLodHistory>>,
    interaction_fields: RefCell<BTreeMap<u32, GrassInteractionField>>,
    interaction_pool: RefCell<Vec<GrassInteractionField>>,
}

impl GrassPatchDrawResources {
    pub(crate) fn apply_section_updates(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_layout: &wgpu::BindGroupLayout,
        interaction_layout: &wgpu::BindGroupLayout,
        sections: &[TexturedRenderSectionMesh],
        removed: impl IntoIterator<Item = RenderSectionKey>,
    ) -> Result<GrassUploadStats> {
        let mut stats = GrassUploadStats::default();
        for key in removed {
            self.remove_surface_roots(key);
            if let Some(range) = self.sections.remove(&key)
                && let Some(arena) = self.arena.as_mut()
            {
                stats.removed_patch_count += range.len() as u32;
                arena.ranges.release(range);
            }
            for history in self.lod_histories.get_mut().values_mut() {
                history.tiers.remove(&key);
            }
        }
        for section in sections {
            self.remove_surface_roots(section.key);
            if let Some(range) = self.sections.remove(&section.key)
                && let Some(arena) = self.arena.as_mut()
            {
                stats.removed_patch_count += range.len() as u32;
                arena.ranges.release(range);
            }
            if section.grass_patches.is_empty() {
                continue;
            }
            let count = section.grass_patches.len() as u32;
            if self.arena.is_none() {
                self.arena = Some(GrassPatchArena::new(device, count)?);
                self.frame = Some(GrassFrameResources::new(
                    device,
                    queue,
                    frame_layout,
                    interaction_layout,
                ));
                self.interaction_pool.get_mut().extend(
                    (0..GRASS_INTERACTION_FIELD_POOL_SIZE)
                        .map(|_| GrassInteractionField::new(device, interaction_layout)),
                );
            }
            let arena = self.arena.as_mut().expect("grass arena created above");
            let range = arena.allocate(device, queue, count)?;
            let bytes = grass_patch_bytes(&section.grass_patches);
            queue.write_buffer(
                &arena.buffer,
                u64::from(range.start) * GrassPatch::BYTE_SIZE as u64,
                &bytes,
            );
            stats.uploaded_patch_count += count;
            stats.uploaded_bytes += bytes.len() as u64;
            self.sections.insert(section.key, range);
            let roots = section
                .grass_patches
                .iter()
                .map(|patch| patch.root)
                .collect::<Vec<_>>();
            self.surface_roots
                .extend(roots.iter().map(|root| (root[0], root[1], root[2])));
            self.root_sections.insert(section.key, roots);
        }
        if self.sections.is_empty() {
            self.arena = None;
            self.frame = None;
            self.lod_histories.get_mut().clear();
            self.interaction_fields.get_mut().clear();
            self.interaction_pool.get_mut().clear();
            self.root_sections.clear();
            self.surface_roots.clear();
        }
        Ok(stats)
    }

    fn remove_surface_roots(&mut self, key: RenderSectionKey) {
        if let Some(roots) = self.root_sections.remove(&key) {
            for root in roots {
                self.surface_roots.remove(&(root[0], root[1], root[2]));
            }
        }
    }

    pub(crate) fn write_frame(
        &self,
        queue: &wgpu::Queue,
        quality: GrassQuality,
        time_seconds: f32,
        observer_id: u32,
        observer_position: [f32; 3],
        topology: HorizontalTopology,
        interactors: GrassInteractorSet,
    ) {
        let (Some(frame), Some(profile)) = (self.frame.as_ref(), quality.profile()) else {
            return;
        };
        queue.write_buffer(
            &frame.buffer,
            0,
            &grass_frame_uniform_bytes(time_seconds, profile.wind_amplitude),
        );
        let mut fields = self.interaction_fields.borrow_mut();
        if !profile.interaction_enabled {
            let mut pool = self.interaction_pool.borrow_mut();
            for mut field in std::mem::take(&mut *fields).into_values() {
                field.reset_for_pool();
                pool.push(field);
            }
            return;
        }
        if !fields.contains_key(&observer_id) {
            let field = self
                .interaction_pool
                .borrow_mut()
                .pop()
                .expect("grass supports at most four simultaneous presentation observers");
            fields.insert(observer_id, field);
        }
        fields
            .get_mut(&observer_id)
            .expect("grass interaction field inserted above")
            .update(
                queue,
                time_seconds,
                observer_position,
                topology,
                interactors,
                &self.surface_roots,
            );
    }

    pub(crate) fn resident_patch_count(&self) -> u32 {
        self.sections.values().map(|range| range.len() as u32).sum()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub(crate) fn draw<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        pipeline: &'pass wgpu::RenderPipeline,
        quality: GrassQuality,
        observer_id: u32,
        observer_position: [f32; 3],
        topology: HorizontalTopology,
        mut visible: impl FnMut(RenderSectionKey) -> bool,
    ) -> GrassDrawStats {
        let resident_patch_count = self.resident_patch_count();
        let mut stats = GrassDrawStats {
            resident_patch_count,
            resident_bytes: u64::from(resident_patch_count) * GrassPatch::BYTE_SIZE as u64,
            ..GrassDrawStats::default()
        };
        let Some(arena) = self.arena.as_ref() else {
            return stats;
        };
        let frame = self
            .frame
            .as_ref()
            .expect("non-empty grass draw must own frame resources");
        let Some(profile) = quality.profile() else {
            return stats;
        };
        if profile.interaction_enabled {
            let snapshot = self
                .interaction_fields
                .borrow()
                .get(&observer_id)
                .map(|field| field.snapshot)
                .unwrap_or_default();
            stats.interaction_field_count = 1;
            stats.interaction_active_cell_count = snapshot.active_cell_count;
            stats.interaction_stamp_count = snapshot.stamp_count;
            stats.interaction_recenter_count = snapshot.recenter_count;
            stats.interaction_reset_count = snapshot.reset_count;
            stats.interaction_uploaded_bytes = snapshot.uploaded_bytes;
        }
        let mut histories = self.lod_histories.borrow_mut();
        let history = histories.entry(observer_id).or_default();
        if history.quality != quality {
            history.quality = quality;
            history.tiers.clear();
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(2, &frame.bind_group, &[]);
        let interaction_fields = self.interaction_fields.borrow();
        let interaction_bind_group = interaction_fields
            .get(&observer_id)
            .map_or(&frame.empty_interaction.bind_group, |field| {
                &field.gpu.bind_group
            });
        pass.set_bind_group(3, interaction_bind_group, &[]);
        pass.set_vertex_buffer(0, arena.buffer.slice(..));
        for (key, range) in &self.sections {
            if !visible(*key) {
                continue;
            }
            let distance = grass_section_distance(*key, observer_position, topology);
            let previous = history.tiers.get(key).copied().unwrap_or_default();
            let tier = grass_lod_tier(profile, distance, previous);
            history.tiers.insert(*key, tier);
            let blade_count = match tier {
                GrassLodTier::Near => {
                    stats.near_patch_count += range.len() as u32;
                    profile.near_blades
                }
                GrassLodTier::Middle => {
                    stats.middle_patch_count += range.len() as u32;
                    profile.middle_blades
                }
                GrassLodTier::Far => {
                    stats.far_patch_count += range.len() as u32;
                    profile.far_blades
                }
                GrassLodTier::Hidden => continue,
            };
            debug_assert!(blade_count <= STATIC_GRASS_BLADE_COUNT);
            let patch_count = range.len() as u32;
            pass.draw(0..blade_count * GRASS_VERTICES_PER_BLADE, range.clone());
            stats.drawn_patch_count += patch_count;
            stats.estimated_blade_count += patch_count * blade_count;
            stats.draw_calls += 1;
        }
        stats
    }
}

fn grass_frame_uniform_bytes(time_seconds: f32, amplitude: f32) -> [u8; 32] {
    let values = [
        time_seconds.rem_euclid(4_096.0),
        0.819_231_9,
        0.573_462_37,
        0.18,
        amplitude,
        0.035,
        0.72,
        0.035,
    ];
    let mut bytes = [0_u8; 32];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn grass_interaction_uniform_bytes(
    origin: [f32; 2],
    topology: HorizontalTopology,
    enabled: bool,
) -> [u8; 32] {
    let values = [
        origin[0],
        origin[1],
        GRASS_INTERACTION_CELL_SIZE as f32,
        if enabled { 1.0 } else { 0.0 },
        topology
            .x
            .period_chunks()
            .map_or(0.0, |period| period as f32 * CHUNK_WIDTH as f32),
        topology
            .z
            .period_chunks()
            .map_or(0.0, |period| period as f32 * CHUNK_WIDTH as f32),
        0.52,
        0.0,
    ];
    let mut bytes = [0_u8; 32];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn encode_signed_unit(value: f32) -> u8 {
    ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0).round() as u8
}

fn grass_surface_near(
    surface_roots: &BTreeSet<(i32, i32, i32)>,
    topology: HorizontalTopology,
    position: Vec3d,
    radius: f64,
) -> bool {
    let min_x = (position.x - radius).floor() as i32;
    let max_x = (position.x + radius).floor() as i32;
    let min_y = (position.y - 0.75).floor() as i32;
    let max_y = (position.y + 0.75).ceil() as i32;
    let min_z = (position.z - radius).floor() as i32;
    let max_z = (position.z + radius).floor() as i32;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let Some(canonical) = topology.canonicalize_block(BlockPos::new(x, y, z)) else {
                    continue;
                };
                if surface_roots.contains(&(canonical.x, canonical.y, canonical.z)) {
                    return true;
                }
            }
        }
    }
    false
}

fn grass_section_distance(
    key: RenderSectionKey,
    observer_position: [f32; 3],
    topology: HorizontalTopology,
) -> f32 {
    let section_center = Vec3d::new(
        f64::from(key.chunk_x) * 16.0 + 8.0,
        f64::from(key.section_y) * 16.0 + 8.0,
        f64::from(key.chunk_z) * 16.0 + 8.0,
    );
    let observer = Vec3d::new(
        f64::from(observer_position[0]),
        f64::from(observer_position[1]),
        f64::from(observer_position[2]),
    );
    let delta = topology.shortest_position_displacement(observer, section_center);
    (delta.x.mul_add(delta.x, delta.z * delta.z) as f32).sqrt()
}

fn grass_lod_tier(
    profile: GrassQualityProfile,
    distance: f32,
    previous: GrassLodTier,
) -> GrassLodTier {
    const HYSTERESIS_BLOCKS: f32 = 4.0;
    match previous {
        GrassLodTier::Near if distance <= profile.near_end_blocks + HYSTERESIS_BLOCKS => {
            GrassLodTier::Near
        }
        GrassLodTier::Middle if distance < profile.near_end_blocks - HYSTERESIS_BLOCKS => {
            GrassLodTier::Near
        }
        GrassLodTier::Middle if distance <= profile.middle_end_blocks + HYSTERESIS_BLOCKS => {
            GrassLodTier::Middle
        }
        GrassLodTier::Far if distance < profile.middle_end_blocks - HYSTERESIS_BLOCKS => {
            if distance < profile.near_end_blocks {
                GrassLodTier::Near
            } else {
                GrassLodTier::Middle
            }
        }
        GrassLodTier::Far if distance <= profile.radius_blocks + HYSTERESIS_BLOCKS => {
            GrassLodTier::Far
        }
        GrassLodTier::Hidden if distance > profile.radius_blocks - HYSTERESIS_BLOCKS => {
            GrassLodTier::Hidden
        }
        _ if distance <= profile.near_end_blocks => GrassLodTier::Near,
        _ if distance <= profile.middle_end_blocks => GrassLodTier::Middle,
        _ if distance <= profile.radius_blocks => GrassLodTier::Far,
        _ => GrassLodTier::Hidden,
    }
}

pub(crate) fn grass_patch_bytes(patches: &[GrassPatch]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(patches.len() * GrassPatch::BYTE_SIZE);
    for patch in patches {
        for value in patch.root {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&patch.packed_tint.to_le_bytes());
        bytes.extend_from_slice(&patch.packed_light.to_le_bytes());
        bytes.extend_from_slice(&patch.seed.to_le_bytes());
        bytes.extend_from_slice(&patch.flags.to_le_bytes());
        bytes.extend_from_slice(&patch.reserved.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    #[test]
    fn packed_grass_patch_bytes_match_the_declared_vertex_abi() {
        let patch = GrassPatch {
            root: [-3, 70, 9],
            packed_tint: 0x0012_3456,
            packed_light: 0x00f0_0070,
            seed: 0x89ab_cdef,
            flags: 7,
            reserved: 11,
        };
        let bytes = grass_patch_bytes(&[patch]);
        assert_eq!(bytes.len(), GrassPatch::BYTE_SIZE);
        assert_eq!(&bytes[0..4], &(-3_i32).to_le_bytes());
        assert_eq!(&bytes[12..16], &patch.packed_tint.to_le_bytes());
        assert_eq!(&bytes[28..32], &patch.reserved.to_le_bytes());
    }

    #[test]
    fn grass_frame_uniforms_are_fixed_width_and_rebase_time() {
        let bytes = grass_frame_uniform_bytes(4_097.5, 0.14);
        assert_eq!(bytes.len(), GRASS_FRAME_UNIFORM_SIZE as usize);
        assert_eq!(f32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1.5);
        assert_eq!(f32::from_le_bytes(bytes[16..20].try_into().unwrap()), 0.14);
    }

    #[test]
    fn grass_interactors_deduplicate_stable_identity_and_bound_capacity() {
        let mut interactors = GrassInteractorSet::default();
        interactors.push(
            GrassInteractor::new(GrassInteractorIdentity::LocalPlayer, [0.0, 1.0, 0.0], 0.6)
                .unwrap(),
        );
        interactors.push(
            GrassInteractor::new(GrassInteractorIdentity::LocalPlayer, [1.0, 1.0, 0.0], 0.7)
                .unwrap(),
        );
        for id in 0..MAX_GRASS_INTERACTORS as u64 + 8 {
            interactors.push(
                GrassInteractor::new(
                    GrassInteractorIdentity::Entity(id),
                    [id as f32, 1.0, 0.0],
                    0.5,
                )
                .unwrap(),
            );
        }
        assert_eq!(interactors.len(), MAX_GRASS_INTERACTORS);
        assert_eq!(
            interactors.iter().next().unwrap().feet_position,
            [1.0, 1.0, 0.0]
        );
        assert!(
            GrassInteractor::new(
                GrassInteractorIdentity::Entity(999),
                [0.0, 1.0, 0.0],
                f32::NAN,
            )
            .is_none()
        );
    }

    #[test]
    fn grass_surface_filter_uses_real_patch_height_and_periodic_identity() {
        let roots = BTreeSet::from([(0, 1, 0)]);
        assert!(grass_surface_near(
            &roots,
            HorizontalTopology::UNBOUNDED,
            Vec3d::new(0.5, 1.0, 0.5),
            0.6,
        ));
        assert!(!grass_surface_near(
            &roots,
            HorizontalTopology::UNBOUNDED,
            Vec3d::new(0.5, 4.0, 0.5),
            0.6,
        ));
        assert!(grass_surface_near(
            &roots,
            HorizontalTopology::cylinder_x(0, 32),
            Vec3d::new(512.25, 1.0, 0.5),
            0.6,
        ));
    }

    #[test]
    fn interaction_uniform_is_one_aligned_row_and_carries_periods() {
        assert_eq!(GRASS_INTERACTION_FIELD_SIZE * 4, 512);
        let bytes = grass_interaction_uniform_bytes(
            [480.0, -32.0],
            HorizontalTopology::cylinder_x(0, 32),
            true,
        );
        assert_eq!(bytes.len(), GRASS_INTERACTION_UNIFORM_SIZE as usize);
        assert_eq!(f32::from_le_bytes(bytes[12..16].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(bytes[16..20].try_into().unwrap()), 512.0);
    }

    #[test]
    fn clipped_shader_variants_keep_the_shared_placement_contract() {
        let mono = clipped_placed_grass_shader_source();
        let multiview = clipped_placed_multiview_grass_shader_source();
        for source in [mono, multiview] {
            assert!(source.contains("source_anchor_scale"));
            assert!(source.contains("composition_anchor"));
            assert!(source.contains("clip_plane"));
            assert!(source.contains("discard"));
        }
    }

    #[test]
    fn every_grass_shader_uses_shared_wind_and_interaction_contracts() {
        for variant in [
            GrassPipelineVariant::Direct,
            GrassPipelineVariant::Placed,
            GrassPipelineVariant::ClippedPlaced,
            GrassPipelineVariant::DirectMultiview,
            GrassPipelineVariant::PlacedMultiview,
            GrassPipelineVariant::ClippedPlacedMultiview,
        ] {
            let source = grass_shader_source(variant);
            assert!(source.contains("@group(2) @binding(0)"));
            assert!(source.contains("@group(3) @binding(0)"));
            assert!(source.contains("@group(3) @binding(1)"));
            assert!(source.contains("grass_frame.shape.y"));
            assert!(source.contains("height_factor * height_factor"));
            assert!(source.contains("let world_sample = root.xz + center;"));
            assert!(source.contains("grass_interaction_bend"));
            assert!(source.contains("grass_interaction_bend(world_sample) * influence"));
        }
    }

    #[test]
    fn grass_quality_profiles_use_hysteretic_lod_bands() {
        let profile = GrassQuality::Lush.profile().unwrap();
        assert_eq!(
            grass_lod_tier(profile, 47.0, GrassLodTier::Hidden),
            GrassLodTier::Near
        );
        assert_eq!(
            grass_lod_tier(profile, 51.0, GrassLodTier::Near),
            GrassLodTier::Near
        );
        assert_eq!(
            grass_lod_tier(profile, 53.0, GrassLodTier::Near),
            GrassLodTier::Middle
        );
        assert_eq!(
            grass_lod_tier(profile, 99.0, GrassLodTier::Middle),
            GrassLodTier::Middle
        );
        assert_eq!(
            grass_lod_tier(profile, 101.0, GrassLodTier::Middle),
            GrassLodTier::Far
        );
        assert_eq!(
            grass_lod_tier(profile, 131.0, GrassLodTier::Far),
            GrassLodTier::Far
        );
        assert_eq!(
            grass_lod_tier(profile, 133.0, GrassLodTier::Far),
            GrassLodTier::Hidden
        );
    }

    #[test]
    fn periodic_grass_lod_distance_uses_the_shortest_seam_lift() {
        let topology = HorizontalTopology::cylinder_x(0, 32);
        let key = RenderSectionKey::new(0, 0, 0);
        let seam_distance = grass_section_distance(key, [511.0, 8.0, 8.0], topology);
        assert_eq!(seam_distance, 9.0);
    }

    #[test]
    #[ignore = "GPU pipeline validation for Tactical 238 static grass"]
    fn all_grass_pipeline_variants_validate_on_gpu() -> Result<()> {
        let (device, _queue) = crate::headless::create_headless_device()?;
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_test_grass_texture_layout"),
            entries: &[],
        });
        let cache = GrassPipelineCache::new(crate::headless::HEADLESS_FORMAT);
        for (variant, uniform_size, dynamic) in [
            (GrassPipelineVariant::Direct, 128, true),
            (GrassPipelineVariant::Placed, 160, true),
            (GrassPipelineVariant::ClippedPlaced, 176, true),
        ] {
            let uniform_layout = test_uniform_layout(&device, uniform_size, dynamic);
            drop(cache.pipeline(&device, variant, &uniform_layout, &texture_layout));
        }
        if device.features().contains(wgpu::Features::MULTIVIEW) {
            for (variant, uniform_size) in [
                (GrassPipelineVariant::DirectMultiview, 256),
                (GrassPipelineVariant::PlacedMultiview, 320),
                (GrassPipelineVariant::ClippedPlacedMultiview, 352),
            ] {
                let uniform_layout = test_uniform_layout(&device, uniform_size, false);
                drop(cache.pipeline(&device, variant, &uniform_layout, &texture_layout));
            }
        }
        Ok(())
    }

    #[test]
    #[ignore = "GPU-backed interaction lifecycle proof for Tactical 238"]
    fn grass_interaction_field_stamps_recovers_recenters_and_wraps() -> Result<()> {
        let (device, queue) = crate::headless::create_headless_device()?;
        let layout = create_grass_interaction_layout(&device);
        let roots = BTreeSet::from([(0, 1, 0)]);
        let mut field = GrassInteractionField::new(&device, &layout);
        let mut interactors = GrassInteractorSet::default();
        interactors.push(
            GrassInteractor::new(GrassInteractorIdentity::LocalPlayer, [0.5, 1.0, 0.5], 0.8)
                .unwrap(),
        );
        field.update(
            &queue,
            0.0,
            [0.5, 2.0, 0.5],
            HorizontalTopology::UNBOUNDED,
            interactors,
            &roots,
        );
        assert!(field.snapshot.active_cell_count > 0);
        assert!(field.snapshot.stamp_count > 0);
        assert_eq!(
            field.snapshot.uploaded_bytes,
            GRASS_INTERACTION_FIELD_BYTE_LEN as u64
        );
        let contact_strength = field
            .pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[2]))
            .sum::<u64>();

        field.update(
            &queue,
            0.25,
            [1.0, 2.0, 0.5],
            HorizontalTopology::UNBOUNDED,
            GrassInteractorSet::default(),
            &roots,
        );
        let recovery_strength = field
            .pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[2]))
            .sum::<u64>();
        assert!(recovery_strength < contact_strength);
        assert_eq!(field.snapshot.recenter_count, 1);

        let mut seam_field = GrassInteractionField::new(&device, &layout);
        let mut seam_interactors = GrassInteractorSet::default();
        seam_interactors.push(
            GrassInteractor::new(GrassInteractorIdentity::Entity(7), [512.25, 1.0, 0.5], 0.8)
                .unwrap(),
        );
        seam_field.update(
            &queue,
            0.0,
            [511.5, 2.0, 0.5],
            HorizontalTopology::cylinder_x(0, 32),
            seam_interactors,
            &roots,
        );
        assert!(seam_field.snapshot.active_cell_count > 0);

        let mut high_field = GrassInteractionField::new(&device, &layout);
        let mut high_interactors = GrassInteractorSet::default();
        high_interactors.push(
            GrassInteractor::new(GrassInteractorIdentity::Entity(8), [0.5, 8.0, 0.5], 0.8).unwrap(),
        );
        high_field.update(
            &queue,
            0.0,
            [0.5, 2.0, 0.5],
            HorizontalTopology::UNBOUNDED,
            high_interactors,
            &roots,
        );
        assert_eq!(high_field.snapshot.active_cell_count, 0);
        Ok(())
    }

    fn test_uniform_layout(
        device: &wgpu::Device,
        size: u64,
        has_dynamic_offset: bool,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_test_grass_uniform_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset,
                    min_binding_size: NonZeroU64::new(size),
                },
                count: None,
            }],
        })
    }
}
