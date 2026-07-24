use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::ops::Range;

use anyhow::{Context, Result, bail};
use mclone_core::{HorizontalTopology, Vec3d};
use mclone_mesh::{GrassPatch, RenderSectionKey, TexturedRenderSectionMesh};

use crate::chunk::DEPTH_FORMAT;

pub(crate) const STATIC_GRASS_BLADE_COUNT: u32 = 8;
const GRASS_VERTICES_PER_BLADE: u32 = 6;
const GRASS_PATCH_MIN_CAPACITY: u32 = 4_096;
const GRASS_PIPELINE_VARIANT_COUNT: usize = 6;

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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GrassUploadStats {
    pub uploaded_patch_count: u32,
    pub removed_patch_count: u32,
    pub uploaded_bytes: u64,
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
            }),
            Self::Lush => Some(GrassQualityProfile {
                radius_blocks: 128.0,
                near_end_blocks: 48.0,
                middle_end_blocks: 96.0,
                near_blades: 6,
                middle_blades: 4,
                far_blades: 2,
            }),
            Self::Ultra => Some(GrassQualityProfile {
                radius_blocks: 192.0,
                near_end_blocks: 64.0,
                middle_end_blocks: 128.0,
                near_blades: 8,
                middle_blades: 6,
                far_blades: 3,
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
    pipelines: RefCell<[Option<wgpu::RenderPipeline>; GRASS_PIPELINE_VARIANT_COUNT]>,
}

impl GrassPipelineCache {
    pub(crate) fn new(color_format: wgpu::TextureFormat) -> Self {
        Self {
            color_format,
            pipelines: RefCell::new(std::array::from_fn(|_| None)),
        }
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
            let pipeline = create_grass_pipeline(
                device,
                self.color_format,
                variant,
                uniform_layout,
                texture_layout,
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
) -> wgpu::RenderPipeline {
    let source = grass_shader_source(variant);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(variant.label()),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(variant.label()),
        bind_group_layouts: &[uniform_layout, texture_layout],
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

#[derive(Default)]
pub(crate) struct GrassPatchDrawResources {
    arena: Option<GrassPatchArena>,
    sections: BTreeMap<RenderSectionKey, Range<u32>>,
    lod_histories: RefCell<BTreeMap<u32, GrassObserverLodHistory>>,
}

impl GrassPatchDrawResources {
    pub(crate) fn apply_section_updates(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sections: &[TexturedRenderSectionMesh],
        removed: impl IntoIterator<Item = RenderSectionKey>,
    ) -> Result<GrassUploadStats> {
        let mut stats = GrassUploadStats::default();
        for key in removed {
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
        }
        if self.sections.is_empty() {
            self.arena = None;
            self.lod_histories.get_mut().clear();
        }
        Ok(stats)
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
        let Some(profile) = quality.profile() else {
            return stats;
        };
        let mut histories = self.lod_histories.borrow_mut();
        let history = histories.entry(observer_id).or_default();
        if history.quality != quality {
            history.quality = quality;
            history.tiers.clear();
        }
        pass.set_pipeline(pipeline);
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
