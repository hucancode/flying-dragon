const PI = 3.14159;
const PATH_LEN = 2000.0;
const SPEED = 0.07;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
};
struct VertexOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) world_position: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};
struct Light {
    position: vec3f,
    radius: f32,
    color: vec4f,
};

@group(0) @binding(0)
var<uniform> world: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> rotation: mat4x4<f32>;
@group(0) @binding(2)
var<storage> combined_transform_map: array<mat4x4<f32>>;
@group(0) @binding(3)
var<uniform> time: f32;
@group(0) @binding(4)
var<uniform> combined_transform_map_length: u32;
@group(1) @binding(0)
var<uniform> view_proj: mat4x4<f32>;
@group(1) @binding(1)
var<storage> lights: array<Light>;
@group(1) @binding(2)
var<uniform> light_count: u32;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var result: VertexOutput;
    let n = combined_transform_map_length;
    let u = (input.position.x + time*SPEED)/PATH_LEN*f32(n)+f32(n);
    let u_low = u32(floor(u))%n;
    let u_high = u32(ceil(u))%n;
    let k = fract(u) + step(fract(u), 0.0);
    let combined_low = combined_transform_map[u_low];
    let combined_high = combined_transform_map[u_high];
    let pos = vec4(0.0, input.position.yz, 1.0);
    let transformed_low = combined_low * pos;
    let transformed_high = combined_high * pos;
    result.world_position = world * mix(transformed_low, transformed_high, k);
    result.position = view_proj * result.world_position;
    let normal_low = combined_low * vec4(input.normal.xyz, 0.0);
    let normal_high = combined_high * vec4(input.normal.xyz, 0.0);
    result.normal = rotation * mix(normal_low, normal_high, k);
    result.color = input.color;
    return result;
}

// @vertex
fn vs_main_circle(input: VertexInput) -> VertexOutput {
    let RADIUS = 60.0 - input.position.z;
    var result: VertexOutput;
    var polar_pos = input.position.x/RADIUS*PI*0.5 + time*SPEED/PI/2;
    var x = cos(polar_pos) * RADIUS;
    var dy = sin(polar_pos) * RADIUS;
    var final_pos = vec4f(x, input.position.y + dy, input.position.z, input.position.w);
    result.color = input.color;
    result.world_position = world * final_pos;
    result.position = view_proj * result.world_position;
    result.normal = rotation * input.normal;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var light_color = vec3(0.0);
    let ambient = vec3(0.02, 0.02, 0.01);
    for (var i = 0u; i < light_count; i++) {
        let pos = lights[i].position;
        let r = lights[i].radius;
        let c = lights[i].color.rgb;
        let world_to_light = pos - vertex.world_position.xyz;
        let dist = clamp(length(world_to_light), 0.0, r);
        let radiance = 1.0 - clamp(dist/r, 0.0, 1.0);
        let strength = max(dot(vertex.normal.xyz, normalize(world_to_light)), 0.0);
        light_color += c * radiance * strength * lights[i].color.a;
    }
    var color = vertex.color.rgb * light_color + ambient;
    return vec4(color, vertex.color.a);
}
