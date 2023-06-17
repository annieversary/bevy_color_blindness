#import bevy_pbr::utils

struct Percentages {
    red: vec3<f32>,
    green: vec3<f32>,
    blue: vec3<f32>,
};

@group(0) @binding(0)
var texture: texture_2d<f32>;

@group(0) @binding(1)
var our_sampler: sampler;

@group(0) @binding(2)
var<uniform> p: Percentages;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // Get screen position with coordinates from 0 to 1
    let uv = in.uv;

    var c = textureSample(texture, our_sampler, uv);

    return vec4<f32>(
        c.r * p.red.x + c.g * p.red.y + c.b * p.red.z,
        c.r * p.green.x + c.g * p.green.y + c.b * p.green.z,
        c.r * p.blue.x + c.g * p.blue.y + c.b * p.blue.z,
        c.a
    );
}
