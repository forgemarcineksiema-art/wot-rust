// The HUD shader (interface program F2): one pass, one pipeline, five styles.
//
// Every vertex carries a style in its last lane. The two legacy styles reproduce the old pixels
// exactly — SOLID fills at full coverage, GLYPH multiplies by the atlas's coverage — so the look
// goldens do not move by a byte. The three new styles give the interface its material: PLATE is
// a rounded or chamfered plate evaluated as a signed distance from the vertex's local coordinate,
// lit on its bevel from the top-left and tiled with the material sheet; SHEET samples the sheet
// directly (icons, the baked minimap relief); GLASS is a tint with a soft reflection band.
// Nothing here needs a uniform: a plate's units are whatever the emitter put in `local`,
// `extent` and `params`, and the anti-aliasing width comes from screen-space derivatives.
//
// The style values are a CPU/GPU protocol: `renderer_api::hud_style` declares the same numbers
// and `hud_style_values_are_bound_at_both_ends` holds the two ends together. Append-only.

const STYLE_SOLID: u32 = 0u;
const STYLE_GLYPH: u32 = 1u;
const STYLE_PLATE: u32 = 2u;
const STYLE_SHEET: u32 = 3u;
const STYLE_GLASS: u32 = 4u;
// The low byte of `style` is the kind; the byte above it is the sheet tile a plate is cut from.
const STYLE_KIND_MASK: u32 = 255u;
const STYLE_TILE_SHIFT: u32 = 8u;
// The sheet is a square grid of square tiles; a plate repeats its tile every TILE_UNITS of local.
const SHEET_TILES_PER_SIDE: f32 = 4.0;
const TILE_UNITS: f32 = 128.0;
// A tile is a modulation map centred at one half: a neutral tile leaves the plate's colour alone.
const TILE_NEUTRAL_GAIN: f32 = 2.0;
// The bevel's light comes from the top-left of the screen (screen y grows downward).
const BEVEL_LIGHT_DIR: vec2<f32> = vec2<f32>(-0.70710678, -0.70710678);
const BEVEL_STRENGTH: f32 = 0.35;
const GLASS_BAND_STRENGTH: f32 = 0.18;

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) local: vec2<f32>,
    @location(4) extent: vec2<f32>,
    @location(5) params: vec2<f32>,
    @location(6) style: u32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local: vec2<f32>,
    @location(3) @interpolate(flat) extent: vec2<f32>,
    @location(4) @interpolate(flat) params: vec2<f32>,
    @location(5) @interpolate(flat) style: u32,
};

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;
@group(0) @binding(2) var sheet_tex: texture_2d<f32>;
@group(0) @binding(3) var sheet_samp: sampler;

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    out.uv = input.uv;
    out.local = input.local;
    out.extent = input.extent;
    out.params = input.params;
    out.style = input.style;
    return out;
}

// Signed distance to a box of half-extents `half` with rounded corners of `radius`, centred at
// the origin: negative inside, zero on the edge, positive outside.
fn rounded_box(p: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(radius, radius);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Where inside the sheet a plate at `local` samples its tile: the tile's cell, plus the local
// coordinate wrapped every TILE_UNITS so the material repeats seamlessly under the plate.
fn sheet_tile_uv(tile: u32, local: vec2<f32>) -> vec2<f32> {
    let col = f32(tile % 4u);
    let row = f32(tile / 4u);
    let inner = fract(local / TILE_UNITS);
    return (vec2<f32>(col, row) + inner) / SHEET_TILES_PER_SIDE;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let kind = input.style & STYLE_KIND_MASK;
    let tile = input.style >> STYLE_TILE_SHIFT;

    // Derivatives first, before any branch on the (non-uniform) style: a derivative inside
    // non-uniform control flow is undefined, and the plate's edge width is one.
    let centred = input.local - input.extent;
    let distance = rounded_box(centred, input.extent, input.params.x);
    let gradient = vec2<f32>(dpdx(distance), dpdy(distance));
    let aa = max(fwidth(distance), 1e-4);

    // The legacy path, exactly as it was: a negative uv.x is the solid sentinel and fills at
    // full coverage whatever the style says; a glyph multiplies by the atlas coverage.
    // textureSampleLevel keeps every sample in uniform-safe control flow under the branches.
    if (kind == STYLE_SOLID || input.uv.x < 0.0) {
        return vec4<f32>(input.color.rgb, input.color.a);
    }
    if (kind == STYLE_GLYPH) {
        let coverage = textureSampleLevel(atlas_tex, atlas_samp, input.uv, 0.0).r;
        return vec4<f32>(input.color.rgb, input.color.a * coverage);
    }
    if (kind == STYLE_SHEET) {
        let sample = textureSampleLevel(sheet_tex, sheet_samp, input.uv, 0.0);
        return vec4<f32>(input.color.rgb * sample.rgb, input.color.a * sample.a);
    }

    // PLATE and GLASS: the rounded box in the element's own units, anti-aliased over one unit of
    // screen-space distance.
    let coverage = 1.0 - smoothstep(-aa, 0.0, distance);
    if (kind == STYLE_GLASS) {
        // A soft diagonal reflection band, phased by params.y, lightening the tint where it runs.
        let span = max(input.extent.x + input.extent.y, 1e-4);
        let diagonal = (input.local.x + input.local.y) / span * 0.5;
        let t = fract(diagonal + input.params.y);
        let band = smoothstep(0.30, 0.48, t) * (1.0 - smoothstep(0.52, 0.70, t));
        let rgb = mix(input.color.rgb, vec3<f32>(1.0, 1.0, 1.0), band * GLASS_BAND_STRENGTH);
        return vec4<f32>(rgb, input.color.a * coverage);
    }

    // PLATE: the tile modulates the plate's own colour; the bevel band is lit by its outward
    // normal against the fixed top-left light, an inset (negative bevel) flipping which edge
    // catches it, so a pressed control reads as pressed with no second geometry.
    let tile_uv = sheet_tile_uv(tile, input.local);
    let modulation = textureSampleLevel(sheet_tex, sheet_samp, tile_uv, 0.0).rgb * TILE_NEUTRAL_GAIN;
    let bevel_width = abs(input.params.y);
    let edge = smoothstep(-bevel_width, 0.0, distance);
    let normal = normalize(gradient + vec2<f32>(1e-6, 0.0));
    let lit = dot(normal, BEVEL_LIGHT_DIR) * sign(input.params.y);
    let shade = 1.0 + edge * lit * BEVEL_STRENGTH;
    let albedo = clamp(input.color.rgb * modulation * shade, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(albedo, input.color.a * coverage);
}
