// Unlit battle-FX pass: world-space quads (muzzle flash, smoke, dirt, sparks, tracers) that the
// client already built in world space, drawn with depth TEST against the lit scene but no depth
// write, blended premultiplied. Colors are authored premultiplied: alpha 0 with non-zero RGB is
// pure additive glow, full premultiplied color is ordinary transparency.
//
// The uniform declares only the leading `view_proj` of the shared scene camera buffer; binding a
// buffer larger than the declared struct is valid, and FX needs none of the lighting fields.

struct FxCamera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: FxCamera;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) sharpness: f32,
    @location(3) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) sharpness: f32,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.sharpness = input.sharpness;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Radial falloff: uv spans [-1, 1] across the quad, so 1 - |uv|^2 is 1 at the center and 0
    // at the inscribed ellipse edge. `sharpness` steepens the edge (1.0 = the soft gaussian-ish
    // particle look; 6.0 reads as a stamped hard disc). Squared for the tight core; colors are
    // premultiplied so one multiply fades RGB and A together without shifting hue.
    let radial = clamp((1.0 - dot(input.uv, input.uv)) * input.sharpness, 0.0, 1.0);
    let fade = radial * radial;
    if (fade <= 0.001) {
        discard;
    }
    return input.color * fade;
}
