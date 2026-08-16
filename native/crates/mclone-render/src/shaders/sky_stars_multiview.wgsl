struct Uniforms {
    view_projections: array<mat4x4<f32>, 2>,
    celestial_rotation: vec4<f32>,
    visibility: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct StarInstance {
    @location(0) equatorial_basis: vec4<f32>,
    @location(1) presentation: vec4<f32>,
    @location(2) roll_cos: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) opacity: f32,
};

fn quad_corner(vertex_index: u32) -> vec2<f32> {
    switch vertex_index {
        case 0u: { return vec2<f32>(-1.0, 1.0); }
        case 1u: { return vec2<f32>(1.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, -1.0); }
        default: { return vec2<f32>(-1.0, -1.0); }
    }
}

@vertex
fn vs_main(
    input: StarInstance,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    let sin_ra = input.equatorial_basis.x;
    let cos_ra = input.equatorial_basis.y;
    let sin_dec = input.equatorial_basis.z;
    let cos_dec = input.equatorial_basis.w;
    let cos_sidereal = uniforms.celestial_rotation.x;
    let sin_sidereal = uniforms.celestial_rotation.y;
    let cos_latitude = uniforms.celestial_rotation.z;
    let sin_latitude = uniforms.celestial_rotation.w;
    let sin_hour = sin_sidereal * cos_ra - cos_sidereal * sin_ra;
    let cos_hour = cos_sidereal * cos_ra + sin_sidereal * sin_ra;
    let east = -cos_dec * sin_hour;
    let north = cos_latitude * sin_dec - sin_latitude * cos_dec * cos_hour;
    let up_component = sin_latitude * sin_dec + cos_latitude * cos_dec * cos_hour;
    let direction = normalize(vec3<f32>(east, up_component, -north));
    let horizon = smoothstep(-0.035, 0.02, direction.y);
    let brightness = input.presentation.y;
    let opacity = brightness * uniforms.visibility.x * horizon;
    if opacity <= 0.001 {
        var culled: VertexOutput;
        culled.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        culled.color = vec3<f32>(0.0);
        culled.opacity = 0.0;
        return culled;
    }
    var reference_up = vec3<f32>(0.0, 1.0, 0.0);
    if abs(direction.y) > 0.95 {
        reference_up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let right = normalize(cross(direction, reference_up));
    let quad_up = normalize(cross(right, direction));
    let corner = quad_corner(vertex_index);
    let roll_sin = input.presentation.w;
    let roll_cos = input.roll_cos;
    let rolled_corner = vec2<f32>(
        corner.x * roll_cos - corner.y * roll_sin,
        corner.y * roll_cos + corner.x * roll_sin,
    );
    let half_size = input.presentation.x;
    let position = direction * 100.0
        + (right * rolled_corner.x + quad_up * rolled_corner.y) * half_size;
    var color = vec3<f32>(0.84, 0.90, 1.0);
    if input.presentation.z < 0.5 {
        color = vec3<f32>(1.0, 0.91, 0.78);
    } else if input.presentation.z < 1.5 {
        color = vec3<f32>(0.92, 0.95, 1.0);
    }
    var output: VertexOutput;
    output.position = uniforms.view_projections[u32(view_index)] * vec4<f32>(position, 1.0);
    output.color = color;
    output.opacity = opacity;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.opacity <= 0.001 {
        discard;
    }
    return vec4<f32>(input.color, input.opacity);
}
