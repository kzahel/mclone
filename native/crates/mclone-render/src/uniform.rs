use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerViewSlot(u32);

impl PerViewSlot {
    pub const SINGLE: Self = Self(0);
    pub const LEFT_EYE: Self = Self(0);
    pub const RIGHT_EYE: Self = Self(1);

    pub const fn index(self) -> u32 {
        self.0
    }
}

pub const SINGLE_VIEW_SLOT: PerViewSlot = PerViewSlot::SINGLE;
pub const LEFT_EYE_VIEW_SLOT: PerViewSlot = PerViewSlot::LEFT_EYE;
pub const RIGHT_EYE_VIEW_SLOT: PerViewSlot = PerViewSlot::RIGHT_EYE;
pub const STEREO_VIEW_SLOT_COUNT: u32 = 2;

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
}
