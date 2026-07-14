use std::num::NonZeroU64;
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerViewSlot(u32);

impl PerViewSlot {
    pub const SINGLE: Self = Self(0);
    pub const LEFT_EYE: Self = Self(0);
    pub const RIGHT_EYE: Self = Self(1);

    pub const fn index(self) -> u32 {
        self.0
    }

    pub const fn view_index(self) -> u32 {
        self.0 % STEREO_VIEW_SLOT_COUNT
    }

    pub const fn is_right_eye(self) -> bool {
        self.view_index() == Self::RIGHT_EYE.view_index()
    }

    pub fn in_uniform_frame(self, frame_index: u32) -> Self {
        assert!(
            frame_index < PER_VIEW_UNIFORM_FRAME_COUNT,
            "uniform frame {frame_index} is outside frame ring count {PER_VIEW_UNIFORM_FRAME_COUNT}"
        );
        Self(frame_index * STEREO_VIEW_SLOT_COUNT + self.view_index())
    }

    pub fn byte_range(self, slot_size: wgpu::BufferAddress) -> Range<wgpu::BufferAddress> {
        assert!(slot_size > 0, "per-view slot byte size must be non-zero");
        let slot_index = self.index();
        assert!(
            slot_index < PER_VIEW_UNIFORM_SLOT_COUNT,
            "per-view slot {slot_index} is outside slot count {PER_VIEW_UNIFORM_SLOT_COUNT}"
        );
        let start = slot_size
            .checked_mul(slot_index as wgpu::BufferAddress)
            .expect("per-view slot byte range start fits in u64");
        let end = start
            .checked_add(slot_size)
            .expect("per-view slot byte range end fits in u64");
        start..end
    }
}

pub const SINGLE_VIEW_SLOT: PerViewSlot = PerViewSlot::SINGLE;
pub const LEFT_EYE_VIEW_SLOT: PerViewSlot = PerViewSlot::LEFT_EYE;
pub const RIGHT_EYE_VIEW_SLOT: PerViewSlot = PerViewSlot::RIGHT_EYE;
pub const STEREO_VIEW_SLOT_COUNT: u32 = 2;
pub const PER_VIEW_UNIFORM_FRAME_COUNT: u32 = 3;
pub const PER_VIEW_UNIFORM_SLOT_COUNT: u32 = STEREO_VIEW_SLOT_COUNT * PER_VIEW_UNIFORM_FRAME_COUNT;

/// Uniform buffer storage for per-view data that may need multiple live copies
/// inside one GPU submission.
pub struct PerViewUniformBuffer {
    buffer: wgpu::Buffer,
    payload_size: NonZeroU64,
    slot_size: wgpu::BufferAddress,
    slot_count: u32,
}

impl PerViewUniformBuffer {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        payload_size: wgpu::BufferAddress,
        slot_count: u32,
    ) -> Self {
        let payload_size = NonZeroU64::new(payload_size).expect("uniform payload is non-empty");
        assert!(slot_count > 0, "uniform slot count must be non-zero");
        let alignment =
            device.limits().min_uniform_buffer_offset_alignment.max(1) as wgpu::BufferAddress;
        let slot_size = align_to(payload_size.get(), alignment);
        let buffer_size = slot_size
            .checked_mul(slot_count as wgpu::BufferAddress)
            .expect("uniform buffer size fits in u64");
        assert!(
            slot_size <= u32::MAX as wgpu::BufferAddress,
            "uniform dynamic offset fits in u32"
        );
        assert!(
            buffer_size <= u32::MAX as wgpu::BufferAddress,
            "uniform buffer dynamic offsets fit in u32"
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: buffer_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            payload_size,
            slot_size,
            slot_count,
        }
    }

    pub fn layout_entry(
        &self,
        binding: u32,
        visibility: wgpu::ShaderStages,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: Some(self.payload_size),
            },
            count: None,
        }
    }

    pub fn bind_group_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &self.buffer,
                offset: 0,
                size: Some(self.payload_size),
            }),
        }
    }

    pub fn write_slot(&self, queue: &wgpu::Queue, slot: PerViewSlot, bytes: &[u8]) -> u32 {
        let slot_index = slot.index();
        assert!(
            slot_index < self.slot_count,
            "uniform slot {slot_index} is outside slot count {}",
            self.slot_count
        );
        assert!(
            bytes.len() as wgpu::BufferAddress <= self.payload_size.get(),
            "uniform write of {} bytes exceeds payload size {}",
            bytes.len(),
            self.payload_size
        );
        let offset = self.slot_offset(slot_index);
        queue.write_buffer(&self.buffer, offset as wgpu::BufferAddress, bytes);
        offset
    }

    pub const fn payload_size(&self) -> wgpu::BufferAddress {
        self.payload_size.get()
    }

    pub const fn slot_size(&self) -> wgpu::BufferAddress {
        self.slot_size
    }

    pub const fn slot_count(&self) -> u32 {
        self.slot_count
    }

    pub const fn allocated_byte_size(&self) -> wgpu::BufferAddress {
        self.slot_size * self.slot_count as wgpu::BufferAddress
    }

    fn slot_offset(&self, slot: u32) -> u32 {
        (self.slot_size * slot as wgpu::BufferAddress) as u32
    }
}

fn align_to(value: wgpu::BufferAddress, alignment: wgpu::BufferAddress) -> wgpu::BufferAddress {
    debug_assert!(alignment > 0);
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_to_keeps_aligned_values() {
        assert_eq!(align_to(64, 256), 256);
        assert_eq!(align_to(256, 256), 256);
        assert_eq!(align_to(257, 256), 512);
    }

    #[test]
    fn per_view_slot_maps_view_into_uniform_frame() {
        let left = LEFT_EYE_VIEW_SLOT.in_uniform_frame(2);
        let right = RIGHT_EYE_VIEW_SLOT.in_uniform_frame(2);
        assert_eq!(left.index(), 4);
        assert_eq!(right.index(), 5);
        assert_eq!(left.view_index(), 0);
        assert_eq!(right.view_index(), 1);
        assert!(!left.is_right_eye());
        assert!(right.is_right_eye());
    }

    #[test]
    fn per_view_slot_byte_range_uses_raw_slot_index() {
        let right = RIGHT_EYE_VIEW_SLOT.in_uniform_frame(1);
        assert_eq!(right.index(), 3);
        assert_eq!(right.byte_range(64), 192..256);
    }

    #[test]
    #[should_panic(expected = "outside frame ring count")]
    fn per_view_slot_rejects_out_of_range_uniform_frame() {
        let _ = LEFT_EYE_VIEW_SLOT.in_uniform_frame(PER_VIEW_UNIFORM_FRAME_COUNT);
    }
}
