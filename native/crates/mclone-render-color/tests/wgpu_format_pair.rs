#![cfg(not(target_arch = "wasm32"))]

use std::sync::mpsc;

use mclone_render_color::{
    RenderColorProfile, color_transform_wgpu, inject_target_color_transform_wgsl,
};

const DISPLAY_SWATCH: [u8; 4] = [28, 122, 176, 255];
const DISPLAY_CLEAR: [u8; 4] = [6, 9, 14, 255];

#[test]
#[ignore = "GPU format-pair acceptance fixture; run explicitly on a host with a wgpu adapter"]
fn vanilla_display_values_match_on_unorm_and_srgb_targets() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("format-pair fixture needs a wgpu adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("mclone_render_color_format_pair_device"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("format-pair fixture needs a wgpu device");

    let unorm = render_fixture(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
    let srgb = render_fixture(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);

    assert_pixel_near(&unorm[0..4], DISPLAY_SWATCH);
    assert_pixel_near(&unorm[4..8], DISPLAY_CLEAR);
    assert_pixel_near(&srgb[0..4], DISPLAY_SWATCH);
    assert_pixel_near(&srgb[4..8], DISPLAY_CLEAR);
    for (unorm, srgb) in unorm[0..8].iter().zip(&srgb[0..8]) {
        assert!(
            unorm.abs_diff(*srgb) <= 1,
            "format pair differs: UNORM={unorm}, sRGB={srgb}"
        );
    }
}

fn render_fixture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    let transform = RenderColorProfile::Vanilla.target_color_transform(format);
    let shader_source = inject_target_color_transform_wgsl(
        r#"
// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__
const output_transform: f32 = __MCLONE_TARGET_COLOR_TRANSFORM__;

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

@fragment
fn fragment_main() -> @location(0) vec4<f32> {
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(0.11, 0.48, 0.69, 1.0),
        output_transform,
    );
}
"#,
        transform,
    )
    .unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mclone_render_color_format_pair_shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mclone_render_color_format_pair_pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_render_color_format_pair_texture"),
        size: wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_render_color_format_pair_readback"),
        size: u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let clear = color_transform_wgpu(
        wgpu::Color {
            r: 0.025,
            g: 0.035,
            b: 0.055,
            a: 1.0,
        },
        transform,
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_render_color_format_pair_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_render_color_format_pair_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_scissor_rect(0, 0, 1, 1);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap();
    });
    device.poll(wgpu::PollType::Wait).unwrap();
    receiver.recv().unwrap().unwrap();
    let bytes = slice.get_mapped_range()[..8].to_vec();
    buffer.unmap();
    bytes
}

fn assert_pixel_near(actual: &[u8], expected: [u8; 4]) {
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 1,
            "pixel differs: actual={actual}, expected={expected}"
        );
    }
}
