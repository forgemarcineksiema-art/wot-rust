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
    // NEGATIVE sharpness is the sun-shaft tag (Hala 3.0 E1): same falloff at |sharpness|,
    // plus a slow drifting-density modulation down the blade — dust moving through a beam,
    // on the tick-domain clock. Battle particles are always positive and take the exact
    // arithmetic they always took.
    let radial = clamp((1.0 - dot(input.uv, input.uv)) * abs(input.sharpness), 0.0, 1.0);
    var fade = radial * radial;
    if (input.sharpness < 0.0) {
        // A shaft is a BEAM, not a puff: soft across its width, nearly full-strength down
        // its length (gentle caps at the glazing and the floor), with the drifting-density
        // modulation breathing along it on the tick-domain clock.
        let across = clamp((1.0 - input.uv.x * input.uv.x) * abs(input.sharpness), 0.0, 1.0);
        let along = 0.55 + 0.45 * (1.0 - input.uv.y * input.uv.y);
        let t = camera.time_params.x;
        let drift = 0.66
            + 0.22 * sin(input.uv.y * 9.0 - t * 0.35 + input.uv.x * 3.0)
            + 0.12 * sin(input.uv.y * 23.0 + t * 0.21 + input.uv.x * 7.0);
        fade = across * across * along * clamp(drift, 0.0, 1.0);
    }
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
