//! Locks that the focused sun shadow reaches the screen (`docs/shadow-policy.md`): an occluder above
//! a ground receiver darkens a patch of that ground, and disabling shadows (`strength = 0`, the
//! capability fallback) is a true no-op. Runs on the headless adapter; skips if none is available.

mod common;
use common::{identity, luma_255};

use game_core::TankId;
use renderer_api::{
    MaterialHandle, MeshHandle, RenderFrame, RenderObject, SceneLighting, SceneVertex,
    VehicleMeshAsset, VehicleVertex, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping shadow render frame test: {error}");
            None
        }
    }
}

fn ground_at(cx: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.45, 0.28];
    let v = |x: f32, z: f32| SceneVertex::new([cx + x, 0.0, z], n, c);
    // Wound so the up-facing ground presents its front (CCW) face to a camera looking down on it.
    (vec![v(-4.0, -4.0), v(4.0, -4.0), v(4.0, 4.0), v(-4.0, 4.0)], vec![0, 2, 1, 0, 3, 2])
}

fn ground() -> (Vec<SceneVertex>, Vec<u32>) {
    ground_at(0.0)
}

/// A small horizontal quad floating 1.2 m above the ground — the shadow caster. It is double-sided
/// (both windings) so a back face survives the shadow pass's front-face cull (which real closed tank
/// hulls satisfy naturally) and this flat plate still casts.
fn occluder() -> VehicleMeshAsset {
    let v = |x: f32, z: f32| VehicleVertex {
        tangent: [1.0, 0.0, 0.0, 1.0],
        ..VehicleVertex::new([x, 1.2, z], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 1.0)
    };
    VehicleMeshAsset::new(
        vec![v(-0.9, -0.9), v(0.9, -0.9), v(0.9, 0.9), v(-0.9, 0.9)],
        vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2],
    )
}

/// A horizontal scene-vertex plate floating 1.2 m above the ground, part of the *static* world
/// buffer (terrain + baked buildings share this format). It stands in for a building/tree that must
/// cast even when no vehicle is on the field.
fn scene_occluder_at(cx: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let n = [0.0, 1.0, 0.0];
    let c = [0.5, 0.5, 0.5];
    let v = |x: f32, z: f32| SceneVertex::new([cx + x, 1.2, z], n, c);
    (vec![v(-0.9, -0.9), v(0.9, -0.9), v(0.9, 0.9), v(-0.9, 0.9)], vec![0, 2, 1, 0, 3, 2])
}

fn scene_occluder() -> (Vec<SceneVertex>, Vec<u32>) {
    scene_occluder_at(0.0)
}

#[test]
fn the_sun_shadow_darkens_the_ground_under_an_occluder() {
    let Some(ctx) = headless() else {
        return;
    };
    let (gv, gi) = ground();
    let mesh = MeshHandle(7);
    // Sun low from the right, camera from the front-above: the caster's shadow stretches to the
    // left, clear of the caster in screen space, so the darkened ground is actually visible.
    let camera = renderer_api::Camera {
        eye: [0.0, 4.0, 6.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 40.0);
    let frame = RenderFrame {
        camera,
        objects: vec![RenderObject {
            tank_id: Some(TankId(1)),
            mesh,
            material: MaterialHandle(0),
            transform: identity(),
            tint: [0.6, 0.7, 0.5],
            dither: 0.0,
        }],
        armor_damage: Vec::new(),
    };

    let render = |shadows: bool| {
        let target = OffscreenTarget::new(&ctx, 96, 96).expect("target");
        let mut r = SceneRenderer::for_offscreen(&ctx, &gv, &gi).expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        // A low sun from the right throws a long shadow to the left, onto ground the camera sees.
        lighting.key_direction = [1.0, 0.6, 0.0];
        r.scene_lighting = lighting;
        r.shadow_focus = Some([0.0, 0.0, 0.0]);
        r.set_shadows_enabled(shadows);
        r.register_vehicle_mesh(&ctx, mesh, &occluder());
        r.set_vehicle_render_frame(&ctx, &frame);
        r.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    let lit = render(false);
    let shadowed = render(true);

    // The only difference is the shadow, which strictly darkens the shaded ground and never
    // brightens anything.
    let darkened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(a) - luma_255(b) > 18.0)
        .count();
    let brightened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(b) - luma_255(a) > 18.0)
        .count();

    assert!(
        darkened > 15,
        "the sun shadow must darken a ground patch under the occluder ({darkened} px)"
    );
    assert!(brightened < 5, "shadows only darken; got {brightened} brightened px");
}

#[test]
fn the_static_world_casts_a_shadow_with_no_vehicles_present() {
    // The whole world now casts, not just the fleet (docs/shadow-policy.md): a raised piece of the
    // static scene buffer — a building/tree — must darken the ground beneath it even with zero
    // vehicles registered. This is exactly the case the old vehicle-only pass bailed on.
    let Some(ctx) = headless() else {
        return;
    };
    let (mut gv, mut gi) = ground();
    let (ov, oi) = scene_occluder();
    let base = gv.len() as u32;
    gv.extend(ov);
    gi.extend(oi.into_iter().map(|i| i + base));

    let camera = renderer_api::Camera {
        eye: [0.0, 4.0, 6.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 40.0);

    let render = |shadows: bool| {
        let target = OffscreenTarget::new(&ctx, 96, 96).expect("target");
        let mut r = SceneRenderer::for_offscreen(&ctx, &gv, &gi).expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        lighting.key_direction = [1.0, 0.6, 0.0];
        r.scene_lighting = lighting;
        r.shadow_focus = Some([0.0, 0.0, 0.0]);
        r.set_shadows_enabled(shadows);
        // No vehicle registered, no vehicle frame — the static world is the only caster.
        r.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    let lit = render(false);
    let shadowed = render(true);
    let darkened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(a) - luma_255(b) > 18.0)
        .count();
    let brightened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(b) - luma_255(a) > 18.0)
        .count();

    assert!(
        darkened > 15,
        "the static world must cast a ground shadow with no vehicles ({darkened} px)"
    );
    assert!(brightened < 5, "shadows only darken; got {brightened} brightened px");
}

#[test]
fn the_far_cascade_darkens_ground_200m_past_the_shadow_focus() {
    // The capability the cascades add (docs/shadow-policy.md): a static occluder ~200 m from the
    // shadow focus — dead ground for the old single 64 m box — still darkens the ground beneath
    // it, because the far cascade's 288 m half-box covers it. Impossible before this pass existed.
    let Some(ctx) = headless() else {
        return;
    };
    let far_x = 200.0;
    let (mut gv, mut gi) = ground_at(far_x);
    let (ov, oi) = scene_occluder_at(far_x);
    let base = gv.len() as u32;
    gv.extend(ov);
    gi.extend(oi.into_iter().map(|i| i + base));

    // The camera stands by the far patch so the darkening is visible on screen; the shadow focus
    // stays pinned at the origin, so the near box's coverage ends ~136 m short of the receiver.
    let camera = renderer_api::Camera {
        eye: [far_x, 4.0, 6.0],
        target: [far_x, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 40.0);

    let render = |shadows: bool| {
        let target = OffscreenTarget::new(&ctx, 96, 96).expect("target");
        // Feature test: explicit discrete quality (F3 folds this machine's shadow maps).
        let mut r = SceneRenderer::for_offscreen_with_quality(
            &ctx,
            &gv,
            &gi,
            renderer_api::LightingQuality::rich(),
        )
        .expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        lighting.key_direction = [1.0, 0.6, 0.0];
        r.scene_lighting = lighting;
        r.shadow_focus = Some([0.0, 0.0, 0.0]);
        r.set_shadows_enabled(shadows);
        r.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    let lit = render(false);
    let shadowed = render(true);
    let darkened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(a) - luma_255(b) > 18.0)
        .count();
    let brightened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma_255(b) - luma_255(a) > 18.0)
        .count();

    assert!(
        darkened > 15,
        "the far cascade must darken ground 200 m past the shadow focus ({darkened} px)"
    );
    assert!(brightened < 5, "shadows only darken; got {brightened} brightened px");
}

/// THE PCF KERNEL MUST STRADDLE THE FRAGMENT, not sit off to one side of it.
///
/// The reduced kernel — the one the SHIPPED canonical profile runs — used to tap `i, j` over
/// `{0, 1}`, so its four samples had their centroid half a texel up and right of the fragment in
/// the light's UV space, and dragged every near-cascade shadow edge that way. The 3×3 kernel
/// kept for dev captures was centred all along, as is the far cascade's own 2×2.
///
/// Proved by MIRRORING the scene rather than by comparing the two tiers. Comparing tiers cannot
/// work: they differ in kernel WIDTH as well as centring, and a display transform this nonlinear
/// does not preserve a profile's centroid under a change of blur width — measured, the two
/// effects came out the same size (a ~3 px width artefact against a ~5 px bias), so no tolerance
/// could separate them. Mirroring holds the kernel, its width and the whole nonlinearity fixed
/// and flips only the world: a centred kernel then renders one picture and its exact mirror,
/// while an off-centre one cannot, because its bias lives in light-space UV and does not mirror
/// with the scene. That also DOUBLES the signal — the two shadows move opposite ways.
#[test]
fn the_pcf_kernel_is_centred_on_the_fragment() {
    let Some(ctx) = headless() else {
        return;
    };
    // A WIDE receiver: at a 25 cm texel the 3×3 kernel's penumbra reaches 0.75 m past the hard
    // edge, and on the ±4 m quad the other tests use it ran off the end. A clipped penumbra is
    // not a symmetric blur, and an asymmetric one moves the very centroid this test reads.
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.45, 0.28];
    let g = |x: f32, z: f32| SceneVertex::new([x, 0.0, z], n, c);
    let mut gv = vec![g(-20.0, -20.0), g(20.0, -20.0), g(20.0, 20.0), g(-20.0, 20.0)];
    let mut gi = vec![0, 2, 1, 0, 3, 2];
    let (ov, oi) = scene_occluder();
    let base = gv.len() as u32;
    gv.extend(ov);
    gi.extend(oi.into_iter().map(|i| i + base));

    // Overhead, with a low sun that throws the shadow entirely clear of the plate casting it, so
    // the whole shadow is visible and unforeshortened. The scene (ground and occluder alike) is
    // symmetric about the x = 0 plane, which is what makes the mirrored render a valid twin.
    const SIZE: u32 = 256;
    // Deliberately COARSE: the defect is half a TEXEL, so it is only measurable when a texel is
    // bigger than a pixel. A 256 m half-box over the shipped 2048² map makes one texel 25 cm,
    // which at this framing (~17.5 px/m) is ~4.4 px of row disagreement for an off-centre kernel.
    // Coarser than this does not work: the normal offset scales with the texel, and at 50 cm it
    // pushes the receiver so far along its normal that a caster 1.2 m up stops casting at all.
    const FOCUS_RADIUS_M: f32 = 256.0;
    /// Measured on this scene: 0.01 px with the kernel centred, 4.39 px with it off-centre — the
    /// full texel the two mirrored shadows are supposed to be separated by. The tolerance sits
    /// between them with room for a different rasteriser on either side.
    const ROW_TOLERANCE_PX: f32 = 2.0;
    /// Measured on this scene: the fixed ±1-diagonal lattice leaves 42.0% of the penumbra as
    /// flat plateau (the printed weave), the rotated cross 8.4%. The tolerance sits between.
    const WEAVE_FLAT_TOLERANCE: f32 = 0.20;

    let render = |mirror: bool, shadows: bool| {
        let sign = if mirror { -1.0 } else { 1.0 };
        let camera = renderer_api::Camera {
            eye: [sign * -3.0, 14.0, 0.6],
            target: [sign * -3.0, 0.0, 0.0],
            vertical_fov_degrees: 55.0,
        };
        let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 60.0);
        let target = OffscreenTarget::new(&ctx, SIZE, SIZE).expect("target");
        let mut r = SceneRenderer::for_offscreen_with_quality(
            &ctx,
            &gv,
            &gi,
            renderer_api::LightingQuality::canonical(),
        )
        .expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        lighting.key_direction = [sign * 1.0, 0.35, 0.0];
        // The indirect rig is mirrored too. It never touches the shadow the test reads (that is
        // differenced away), but leaving it fixed would tilt the darkening weights across the
        // frame and nudge a centroid for reasons unrelated to the kernel.
        lighting.fill_direction[0] *= sign;
        lighting.rim_direction[0] *= sign;
        r.scene_lighting = lighting;
        r.shadow_focus = Some([0.0, 0.0, 0.0]);
        r.shadow_focus_radius_m = Some(FOCUS_RADIUS_M);
        r.set_shadows_enabled(shadows);
        r.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    // Read along the frame's VERTICAL axis (world Z), which is the axis that exposes the bias.
    //
    // The kernel's offset splits into two components. The one along the light's `v` axis lies in
    // the world XZ plane the sun tilts in, so it mirrors with the scene and cancels — measuring
    // columns finds nothing, which is exactly what the first version of this test reported.
    // The one along the light's `u` axis points along world Z, and `u` FLIPS when the sun does
    // while the camera's Z does not: the twins then displace their shadows in opposite
    // directions along Z, and the row centroid sees a full texel of disagreement.
    let centroid_row = |mirror: bool| -> f32 {
        let lit = render(mirror, false);
        let shadowed = render(mirror, true);
        let (mut weighted, mut total) = (0.0f64, 0.0f64);
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let p = (y * SIZE as usize + x) * 4;
                let drop = (luma_255(&lit[p..p + 4]) - luma_255(&shadowed[p..p + 4])) as f64;
                if drop > 3.0 {
                    weighted += drop * y as f64;
                    total += drop;
                }
            }
        }
        assert!(total > 0.0, "the occluder must cast a visible shadow (mirror = {mirror})");
        (weighted / total) as f32
    };

    assert!(
        !renderer_api::LightingQuality::canonical()
            .shader_detail
            .has(renderer_api::ShaderDetailMask::PCF_WIDE),
        "premise: the shipped profile runs the REDUCED kernel — this test guards that one"
    );

    // THE PENUMBRA MUST NOT PRINT A PATTERN. A four-tap kernel wide enough to be soft samples
    // a sparse lattice, and if that lattice is FIXED it prints: the ±1-diagonal variant shipped
    // briefly and every penumbra came back wearing the same woven-cloth weave — plateaus
    // wherever no tap support crossed a depth edge. The rotated cross dithers those plateaus
    // into per-pixel grain.
    //
    // Measured as the share of transition-band pixels (10–90% of the local drop) that are FLAT
    // against all four neighbours. A plateau is flat in every direction by definition; a clean
    // gradient is not (across the penumbra the drop changes several luma steps per pixel); grain
    // is not (neighbours differ pseudo-randomly). So the share is orientation-proof — a first
    // draft measured plateau RUN LENGTHS per row and drowned in lines running parallel to the
    // shadow edge, which sit in the band for dozens of near-identical pixels with any kernel.
    {
        let lit = render(false, false);
        let shadowed = render(false, true);
        let size = SIZE as usize;
        let drop_at = |x: usize, y: usize| -> f32 {
            let p = (y * size + x) * 4;
            luma_255(&lit[p..p + 4]) - luma_255(&shadowed[p..p + 4])
        };
        let max_drop = (0..size * size).map(|i| drop_at(i % size, i / size)).fold(0.0f32, f32::max);
        assert!(max_drop > 30.0, "the scene must cast a strong shadow, got drop {max_drop}");
        let (band_lo, band_hi) = (max_drop * 0.1, max_drop * 0.9);
        let (mut band, mut flat) = (0u32, 0u32);
        for y in 1..size - 1 {
            for x in 1..size - 1 {
                let d = drop_at(x, y);
                if d <= band_lo || d >= band_hi {
                    continue;
                }
                band += 1;
                let still = |dx: isize, dy: isize| {
                    (drop_at((x as isize + dx) as usize, (y as isize + dy) as usize) - d).abs()
                        < 1.5
                };
                if still(1, 0) && still(-1, 0) && still(0, 1) && still(0, -1) {
                    flat += 1;
                }
            }
        }
        assert!(band > 100, "too few penumbra pixels to judge ({band})");
        let share = flat as f32 / band as f32;
        eprintln!(
            "PCF weave: {:.1}% of {band} penumbra pixels are flat plateaus \
             (tolerance {:.0}%; the fixed diagonal lattice measured 42.0%, the rotated \
             cross 8.4%)",
            share * 100.0,
            WEAVE_FLAT_TOLERANCE * 100.0
        );
        assert!(
            share <= WEAVE_FLAT_TOLERANCE,
            "{:.1}% of the penumbra is flat plateau — a fixed sparse kernel is printing its \
             lattice into the shadow edge again",
            share * 100.0
        );
    }

    let straight = centroid_row(false);
    let mirrored = centroid_row(true);
    // Always reported: the separation is the evidence, and a number in the log is what lets the
    // next person re-tune the tolerance instead of guessing at it.
    eprintln!(
        "PCF centring: rows {straight:.2} / {mirrored:.2}, disagreement {:.2} px \
         (tolerance {ROW_TOLERANCE_PX} px; an off-centre kernel measures ~4.4)",
        (straight - mirrored).abs()
    );
    assert!(
        (straight - mirrored).abs() < ROW_TOLERANCE_PX,
        "the twins put the shadow at different depths: row {straight:.2} vs {mirrored:.2} — the \
         PCF kernel is biased in light-space UV instead of centred on the fragment"
    );
}
