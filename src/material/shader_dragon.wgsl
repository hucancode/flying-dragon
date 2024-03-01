const MAX_LIGHT = 10;
const PI = 3.14159;
const PATH_LEN = 400.0;
const SPEED = 0.05;

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
    position_and_radius: vec4f,
    color: vec4f,
};

@group(0) @binding(0)
var<uniform> world: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> rotation: mat4x4<f32>;
@group(0) @binding(2)
var<storage> displacement_map: array<mat4x4<f32>>;
@group(0) @binding(3)
var<storage> rotation_offset_map: array<mat4x4<f32>>;
@group(0) @binding(4)
var<uniform> time: f32;
@group(1) @binding(0)
var<uniform> view_proj: mat4x4<f32>;
@group(1) @binding(1)
var<storage> lights: array<Light>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var result: VertexOutput;
    let n = arrayLength(&displacement_map);
    let u = (input.position.x + time*SPEED)/PATH_LEN*f32(n) + f32(n);
    let u_low = u32(floor(u))%n;
    let u_high = u32(ceil(u))%n;
    let k = fract(u);
    let translation_low = displacement_map[u_low];
    let translation_high = displacement_map[u_high];
    let rotation_low = rotation_offset_map[u_low];
    let rotation_high = rotation_offset_map[u_high];
    let pos = vec4f(0.0, input.position.y, input.position.z, input.position.w);
    result.world_position = world * mix(translation_low * rotation_low * pos, translation_high * rotation_high * pos, k);
    result.position = view_proj * result.world_position;
    result.normal = rotation * mix(rotation_low * input.normal, rotation_high * input.normal, k);
    result.color = input.color;
    return result;
}

@vertex
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
    var color = vec3(0.0);
    let n = arrayLength(&lights);
    for (var i = 0u; i < n; i++) {
        let pos = lights[i].position_and_radius.xyz;
        let r = lights[i].position_and_radius.w;
        let c = lights[i].color.rgb;
        let world_to_light = pos - vertex.world_position.xyz;
        let dist = clamp(length(world_to_light), 0.0, r);
        let radiance = 1.0 - clamp(dist/r, 0.0, 1.0);
        let strength = max(dot(vertex.normal.xyz, normalize(world_to_light)), 0.0);
        color += vertex.color.rgb * c * radiance * strength;
    }
    return vec4(color, vertex.color.a);
}
