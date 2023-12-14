const MAX_LIGHT = 10;
const PI = 3.14159;
const PATH_SCALE = 40.0;
const PATH_LEN = 300.0;

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

fn sample(uv: vec2f) -> vec4f {
    return textureSampleLevel(displacement_map, displacement_sampler, uv, 0.0);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var result: VertexOutput;
    var u = (input.position.x + displacement_offset)/PATH_LEN;
    var displacement1 = sample(vec2(u,0.0));
    var displacement2 = sample(vec2(u+0.01,0.0));
    var tangent = displacement2 - displacement1;
    var normal = sample(vec2(u,0.5));
    var binormal = sample(vec2(u,1.0));
    tangent = normalize(tangent);
    normal = normalize(normal);
    binormal = normalize(binormal);
    var basis = mat4x4(tangent, normal, binormal, vec4f(0.0,0.0,0.0,1.0));
    var x = displacement1 * PATH_SCALE;
    var yz = vec4f(0.0, input.position.yz, 1.0);
    result.color = input.color;
    result.world_position = world * (x + basis * yz);
    result.position = view_proj * result.world_position;
    result.normal = rotation * normalize(basis * input.normal);
    return result;
}

@vertex
fn vs_main_circle(input: VertexInput) -> VertexOutput {
    let RADIUS = 60.0 - input.position.z;
    let SPEED = 0.02;
    var result: VertexOutput;
    var polar_pos = input.position.x/RADIUS * PI * 0.5 + SPEED*displacement_offset;
    var x = cos(polar_pos) * RADIUS;
    var dy = sin(polar_pos) * RADIUS;
    var final_pos = vec4f(x, input.position.y + dy, input.position.z, input.position.w);
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
