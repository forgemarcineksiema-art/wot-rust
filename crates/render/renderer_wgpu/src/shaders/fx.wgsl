// Battle-FX pass: world-space quads (muzzle flash, smoke, dirt, sparks, tracers, ground
// marks) that the client already built in world space, drawn with depth TEST against the
// lit scene but no depth write, blended premultiplied. Colors are authored premultiplied:
// alpha 0 with non-zero RGB is pure additive glow, full premultiplied color is ordinary
// transparency.
//
// Teren F3 (slice 1): PHYSICAL media breathe the air — a rut, a smoke plume, a dust sheet
// recede into the same haze the world does, through the shared `apply_fog` (rule 4:
// atmosphere is depth). PURE-ADDITIVE glow stays unfogged on purpose: a tracer's read at
// range is a gameplay promise, not a lighting choice. Sun/cloud shade on ground marks is
// the recorded slice 2 — it needs the shadow group in this pipeline's layout.
// Composed after camera_common.wgsl and lighting_common.wgsl.

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
    @location(3) world_pos: vec3<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.sharpness = input.sharpness;
    out.color = input.color;
    out.world_pos = input.position;
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
    var color = input.color;
    // Physical media only (alpha carries coverage): un-premultiply, run the ONE fog
    // implementation, re-premultiply. Additive glow (alpha ~ 0) passes through.
    if (color.a > 0.011) {
        let straight = color.rgb / color.a;
        color = vec4<f32>(apply_fog(straight, input.world_pos) * color.a, color.a);
    }
    return color * fade;
}
