struct StereoUniforms {
    view_projections: array<mat4x4<f32>, 2>,
};

@group(0) @binding(0)
var<uniform> uniforms: StereoUniforms;

@group(1) @binding(0)
var sun_texture: texture_2d<f32>;

@group(1) @binding(1)
var sun_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opacity: f32,
    @location(3) mode: f32,
    @location(4) phase: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
    @location(2) mode: f32,
    @location(3) phase: f32,
};

@vertex
fn vs_main(
    input: VertexInput,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projections[u32(view_index)] * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.opacity = input.opacity;
    output.mode = input.mode;
    output.phase = input.phase;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.opacity <= 0.001 {
        discard;
    }
    let sampled = textureSample(sun_texture, sun_sampler, input.uv);
    if input.mode < 0.5 {
        return vec4<f32>(sampled.rgb, sampled.a * input.opacity);
    }
    if input.mode < 1.5 {
        return vec4<f32>(vec3<f32>(1.0, 0.965, 0.745), input.opacity);
    }
    if input.mode < 2.5 {
        let centered = input.uv * 2.0 - vec2<f32>(1.0);
        let radius = length(centered);
        let halo = (1.0 - smoothstep(0.08, 1.0, radius)) * 0.16;
        return vec4<f32>(vec3<f32>(1.0, 0.72, 0.30), halo * input.opacity);
    }
    if input.mode < 3.5 {
        let pixel_uv = (floor(input.uv * 16.0) + vec2<f32>(0.5)) / 16.0;
        let centered = pixel_uv * 2.0 - vec2<f32>(1.0);
        let illumination = (1.0 - cos(input.phase * 6.28318530718)) * 0.5;
        let threshold = 1.0 - illumination * 2.0;
        let bend = (1.0 - centered.y * centered.y) * 0.12
            * sin(input.phase * 6.28318530718);
        var lit = centered.x >= threshold + bend;
        if input.phase >= 0.5 {
            lit = centered.x <= -threshold + bend;
        }
        if !lit {
            discard;
        }
        return vec4<f32>(vec3<f32>(0.82, 0.86, 0.92), input.opacity);
    }
    let frame = u32(round(input.phase)) % 8u;
    let atlas_cell = vec2<f32>(f32(frame % 4u), f32(frame / 4u));
    let atlas_uv = (atlas_cell + input.uv) / vec2<f32>(4.0, 2.0);
    let moon = textureSample(sun_texture, sun_sampler, atlas_uv);
    return vec4<f32>(moon.rgb, moon.a * input.opacity);
}
