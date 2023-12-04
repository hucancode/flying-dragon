const MAX_LIGHT = 10;
const PI = 3.14159;
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
    @location(0) position: vec3<f32>,
    @location(1) radius: f32,
    @location(2) color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> world: mat4x4<f32>;
@group(0) @binding(1)
var<uniform> rotation: mat4x4<f32>;
@group(0) @binding(2)
var displacement_map: texture_2d<f32>;
@group(0) @binding(3)
var displacement_sampler: sampler;
@group(0) @binding(4)
var<uniform> displacement_offset: f32;
@group(1) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let LEN = 250.0;
    var result: VertexOutput;
    let dt = 0.002;
    var u0 = (input.position.x + displacement_offset)/LEN - dt;
    var u1 = (input.position.x + displacement_offset)/LEN;
    var u2 = (input.position.x + displacement_offset)/LEN + dt;
    var p0 = textureSampleLevel(displacement_map, displacement_sampler, vec2(u0,0.0), 0.0);
    var p1 = textureSampleLevel(displacement_map, displacement_sampler, vec2(u1,0.0), 0.0);
    var p2 = textureSampleLevel(displacement_map, displacement_sampler, vec2(u2,0.0), 0.0);
    // basis calculation are messed up now
    var tangent = normalize(p2 - p1);
    var prev_tangent = normalize(p1 - p0);
    var normal = normalize(prev_tangent * tangent);
    if(length(normal) < 0.01 || true) { 
        normal = vec4(0.0,0.0,1.0,0.0);
    }
    var binormal = normalize(tangent * normal);
    if(length(binormal) < 0.01 || true) {
        binormal = vec4(0.0,1.0,0.0,0.0);
    }
    var x = p1 * LEN;
    var y = input.position.y * binormal;
    var z = input.position.z * normal;
    var position = x + y + z;
    position.w = 1.0;
    result.color = input.color;
    result.world_position = world * position;
    result.position = view_proj * result.world_position;
    result.normal = rotation * input.normal;
    return result;
}

@vertex
fn vs_main_circle(input: VertexInput) -> VertexOutput {
    let RADIUS = 60.0 - input.position.z;
    var result: VertexOutput;
    var polar_pos = input.position.x/RADIUS * PI * 0.5 + displacement_offset;
    var x = cos(polar_pos) * RADIUS;
    var dz = sin(polar_pos) * RADIUS;
    var final_pos = vec4f(x, input.position.y, input.position.z + dz, input.position.w);
    result.color = input.color;
    result.world_position = world * final_pos;
    result.position = view_proj * result.world_position;
    result.normal = rotation * input.normal;
    return result;
}

@group(1) @binding(1)
var<uniform> lights: array<Light, MAX_LIGHT>;
@group(1) @binding(2)
var<uniform> light_count: u32;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec3(0.0);
    for (var i = 0u; i < light_count; i++) {
        let world_to_light = lights[i].position - vertex.world_position.xyz;
        let dist = clamp(length(world_to_light), 0.0, lights[i].radius);
        let radiance = lights[i].color.rgb * (1.0 - dist / lights[i].radius);
        let strength = max(dot(vertex.normal.xyz, normalize(world_to_light)), 0.0);
        color += vertex.color.rgb * radiance * strength;
    }
    return vec4(color, vertex.color.a);
}
