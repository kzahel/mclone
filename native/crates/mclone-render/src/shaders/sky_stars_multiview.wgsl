struct Uniforms {
    view_projections: array<mat4x4<f32>, 2>,
    parameters: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct StarInstance {
    @location(0) equatorial_size_brightness: vec4<f32>,
    @location(1) color_class: f32,
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
        case 3u: { return vec2<f32>(-1.0, 1.0); }
        case 4u: { return vec2<f32>(1.0, -1.0); }
        default: { return vec2<f32>(-1.0, -1.0); }
    }
}

@vertex
fn vs_main(
    input: StarInstance,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    let latitude = uniforms.parameters.x;
    let sidereal_angle = uniforms.parameters.y;
    let right_ascension = input.equatorial_size_brightness.x * 6.28318530718;
    let declination = input.equatorial_size_brightness.y;
    let hour_angle = sidereal_angle - right_ascension;
    let east = -cos(declination) * sin(hour_angle);
    let north = cos(latitude) * sin(declination)
        - sin(latitude) * cos(declination) * cos(hour_angle);
    let up_component = sin(latitude) * sin(declination)
        + cos(latitude) * cos(declination) * cos(hour_angle);
    let direction = normalize(vec3<f32>(east, up_component, -north));
    var reference_up = vec3<f32>(0.0, 1.0, 0.0);
    if abs(direction.y) > 0.95 {
        reference_up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let right = normalize(cross(direction, reference_up));
    let quad_up = normalize(cross(right, direction));
    let corner = quad_corner(vertex_index);
    let half_size = 100.0
        * tan(radians(input.equatorial_size_brightness.z) * 0.5);
    let position = direction * 100.0 + (right * corner.x + quad_up * corner.y) * half_size;
    let horizon = smoothstep(-0.035, 0.02, direction.y);
    let brightness = input.equatorial_size_brightness.w;
    var color = vec3<f32>(0.84, 0.90, 1.0);
    if input.color_class < 0.5 {
        color = vec3<f32>(1.0, 0.91, 0.78);
    } else if input.color_class < 1.5 {
        color = vec3<f32>(0.92, 0.95, 1.0);
    }
    var output: VertexOutput;
    output.position = uniforms.view_projections[u32(view_index)] * vec4<f32>(position, 1.0);
    output.color = color;
    output.opacity = brightness * uniforms.parameters.z * horizon;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.opacity <= 0.001 {
        discard;
    }
    return vec4<f32>(input.color, input.opacity);
}
