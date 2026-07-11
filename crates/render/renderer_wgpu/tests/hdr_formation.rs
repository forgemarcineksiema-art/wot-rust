//! The HDR half of the image-formation lock (`docs/art-direction-policy.md` rule 7): the world
//! renders LINEAR HDR into the internal Rgba16Float chain, so genuinely hot sources (the sun
//! disc, a raking key) must reach the post pass with values ABOVE 1.0 — energy an 8-bit
//! framebuffer would have clipped. This is the property bloom (A4) will feed on. Runs on the
//! headless adapter; skips if none is available.

use renderer_api::{Camera, SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping hdr formation test: {error}");
            None
        }
    }
}

/// Decode one IEEE 754 half-precision float.
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as u32;
    let frac = (bits & 0x3ff) as u32;
    let f = match (exp, frac) {
        (0, 0) => sign << 31,
        (0, _) => {
            // Subnormal: renormalize.
            let mut e = 127 - 15 + 1;
            let mut m = frac;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            (sign << 31) | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
        (0x1f, 0) => (sign << 31) | 0x7f80_0000,
        (0x1f, _) => (sign << 31) | 0x7fc0_0000,
        _ => (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13),
    };
    f32::from_bits(f)
}

#[test]
fn hot_radiance_survives_to_the_post_pass_above_one() {
    let Some(ctx) = headless_context() else {
        return;
    };

    // A bare ground plane under the battle profile, camera looking INTO the low sun so the sky
    // pass puts the disc (radiance far past 1.0 pre-curve) in frame.
    let vertices = vec![
        SceneVertex::new([-50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([-50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
    ];
    let indices = vec![0u32, 2, 1, 0, 3, 2];

    let target = OffscreenTarget::new(&ctx, 128, 72).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices).expect("renderer");
    let lighting = SceneLighting::battlefield_default();
    let sun = lighting.key_direction;
    renderer.scene_lighting = lighting;

    let eye = [0.0f32, 2.0, 0.0];
    let look = [eye[0] + sun[0] * 100.0, eye[1] + sun[1] * 100.0, eye[2] + sun[2] * 100.0];
    let camera = Camera { eye, target: look, vertical_fov_degrees: 55.0 };
    let view_proj = view_projection_matrix(&camera, 128.0 / 72.0, 0.1, 2000.0);
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");

    let hdr = renderer.read_hdr_rgba16(&ctx).expect("hdr chain exists after a render");
    let mut max_channel = f32::NEG_INFINITY;
    for texel in hdr.chunks_exact(8) {
        for c in 0..3 {
            let bits = u16::from_le_bytes([texel[c * 2], texel[c * 2 + 1]]);
            let v = f16_to_f32(bits);
            assert!(v.is_finite(), "HDR chain must hold finite radiance, got {v}");
            max_channel = max_channel.max(v);
        }
    }
    assert!(
        max_channel > 1.0,
        "the sun in frame must reach the post pass above 1.0 — an 8-bit chain would have \
         clipped it; got max {max_channel}"
    );

    // And the OUTPUT is display range: the post pass tone-maps everything into [0, 1] with the
    // brightest pixels still bright (the disc reads white, not grey).
    let out = target.read_rgba8(&ctx).expect("readback");
    let max_out = out.chunks_exact(4).map(|p| p[0].max(p[1]).max(p[2])).max().unwrap_or(0);
    assert!(max_out >= 240, "the sun must still read hot after the curve: {max_out}");
}
