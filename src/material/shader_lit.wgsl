const MAX_LIGHT = 10;
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
@group(1) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var result: VertexOutput;
    result.color = input.color;
    result.world_position = world * input.position;
    result.position = view_proj * result.world_position;
    result.normal = rotation * input.normal;
    return result;
}

@group(1) @binding(1)
var<storage> lights: array<Light>;

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
        color += vertex.color.rgb * radiance * strength;
    }
    return vec4(normalize(color), vertex.color.a);
}
