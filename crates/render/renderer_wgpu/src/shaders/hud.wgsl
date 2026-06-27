struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    out.uv = input.uv;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Negative uv.x is the "solid" sentinel: bars and crosshairs fill at full coverage and never
    // touch the atlas. Text quads sample the single-channel coverage atlas. textureSampleLevel
    // keeps the sample in uniform-safe control flow under the branch.
    var coverage = 1.0;
    if (input.uv.x >= 0.0) {
        coverage = textureSampleLevel(atlas_tex, atlas_samp, input.uv, 0.0).r;
    }
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
