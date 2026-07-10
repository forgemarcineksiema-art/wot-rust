//! Locks the load-bearing invariants of the atmosphere lighting profile (`docs/atmosphere-policy.md`
//! phase 1): the battle look has a live rim and a real hemispheric ambient, and the ambient blend
//! is grounded — up-facing surfaces read the sky, down-facing surfaces the warmer ground bounce.

use renderer_api::SceneLighting;

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// CPU mirror of the shaders' `hemi_ambient(n)`: blend ground->sky by the normal's up fraction.
/// This documents and locks the *model* the WGSL implements, so a degenerate profile (flat ambient)
/// or a flipped blend is caught here rather than only by eye.
fn hemi_ambient(l: &SceneLighting, normal_up: f32) -> [f32; 3] {
    let t = (normal_up * 0.5 + 0.5).clamp(0.0, 1.0);
    let mix = |g: f32, s: f32| g + (s - g) * t;
    [
        mix(l.ground_ambient_rgb[0], l.ambient_rgb[0]),
        mix(l.ground_ambient_rgb[1], l.ambient_rgb[1]),
        mix(l.ground_ambient_rgb[2], l.ambient_rgb[2]),
    ]
}

#[test]
fn battle_profile_has_a_live_rim_and_a_raking_side_sun() {
    let l = SceneLighting::battlefield_default();
    // The rim was black (silhouette flat against the sky); phase 1 turns it on.
    assert!(luminance(l.rim_rgb) > 0.05, "battle rim must be live: {:?}", l.rim_rgb);
    // The sun rakes from the side (strong horizontal component) rather than sitting near-overhead,
    // so it sculpts the sides of a low hull. Old key was [0.45, 0.82, 0.35] (mostly up).
    assert!(
        l.key_direction[0].abs() > l.key_direction[1],
        "sun should rake from the side, not top-down: {:?}",
        l.key_direction
    );
}

#[test]
fn hemispheric_ambient_is_real_not_a_flat_constant() {
    for l in [SceneLighting::battlefield_default(), SceneLighting::garage_studio()] {
        assert_ne!(
            l.ambient_rgb, l.ground_ambient_rgb,
            "sky and ground ambient must differ, or the hemisphere is a flat constant"
        );
        // The sky (upper) ambient is the cooler/brighter of the two; the ground bounce is warmer
        // and dimmer. A grounded look needs the ground darker than the sky.
        assert!(
            luminance(l.ambient_rgb) > luminance(l.ground_ambient_rgb),
            "sky ambient should out-lume the ground bounce"
        );
    }
}

#[test]
fn ambient_blend_grounds_up_faces_to_sky_and_down_faces_to_ground() {
    let l = SceneLighting::battlefield_default();
    let up = hemi_ambient(&l, 1.0);
    let down = hemi_ambient(&l, -1.0);
    let flat = hemi_ambient(&l, 0.0);
    assert_eq!(up, l.ambient_rgb, "a fully up-facing surface takes the sky ambient");
    assert_eq!(down, l.ground_ambient_rgb, "a fully down-facing surface takes the ground bounce");
    // A horizontal-facing surface sits between the two hemispheres.
    assert!(luminance(down) < luminance(flat) && luminance(flat) < luminance(up));
}

#[test]
fn battle_profile_has_aerial_perspective_and_the_interior_does_not() {
    let battle = SceneLighting::battlefield_default();
    // Phase 2: the battlefield fades distant surfaces into the horizon haze for 1000 m depth.
    assert!(
        battle.fog_density > 0.0,
        "battle needs aerial-perspective fog: {}",
        battle.fog_density
    );
    // The horizon (also the fog colour) must differ from the zenith, or the gradient sky and the
    // fade-to-sky are degenerate flat colours.
    assert_ne!(
        battle.sky_horizon_rgb, battle.sky_zenith_rgb,
        "gradient sky must have a distinct horizon and zenith"
    );
    // The horizon is the paler, hazier end of the sky the fog fades toward.
    assert!(
        luminance(battle.sky_horizon_rgb) > luminance(battle.sky_zenith_rgb),
        "horizon haze should out-lume the deeper zenith"
    );
    // Interior profiles must not apply aerial perspective (there is no open air/horizon).
    for interior in [SceneLighting::garage_studio(), SceneLighting::garage_workshop()] {
        assert_eq!(interior.fog_density, 0.0, "garage interiors carry no distance fog");
    }
}

fn all_profiles() -> [(&'static str, SceneLighting); 6] {
    [
        ("battlefield_default", SceneLighting::battlefield_default()),
        ("bystra_clear_afternoon", SceneLighting::bystra_clear_afternoon()),
        ("bystra_rain", SceneLighting::bystra_rain()),
        ("bystra_dawn_fog", SceneLighting::bystra_dawn_fog()),
        ("garage_studio", SceneLighting::garage_studio()),
        ("garage_workshop", SceneLighting::garage_workshop()),
    ]
}

#[test]
fn every_profile_grades_within_the_sane_display_envelope() {
    // The grade is data now, so a fat-fingered preset (exposure 11.0, black point 0.8) would
    // silently crush the image; these bounds are the envelope the image formation was designed
    // for. Widening them is a deliberate diff, not a tuning accident.
    for (name, l) in all_profiles() {
        assert!((0.5..=2.0).contains(&l.exposure), "{name}: exposure {} out of range", l.exposure);
        assert!(
            (0.0..=0.08).contains(&l.black_point),
            "{name}: black point {} out of range",
            l.black_point
        );
        assert!(
            (0.8..=1.5).contains(&l.saturation),
            "{name}: saturation {} out of range",
            l.saturation
        );
        assert!((0.9..=1.4).contains(&l.contrast), "{name}: contrast {} out of range", l.contrast);
    }
}

#[test]
fn a_neutral_grade_only_applies_the_tone_curve() {
    // With exposure 1 / black 0 / saturation 1 / contrast 1, grade_reference reduces to the bare
    // ACES-lite curve — the identity of the grading stage. Locks that no hidden constant remains
    // in the pipeline now that the old hardcoded 1.18/1.10 moved into the profiles.
    let mut l = SceneLighting::battlefield_default();
    l.exposure = 1.0;
    l.black_point = 0.0;
    l.saturation = 1.0;
    l.contrast = 1.0;
    let aces = |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    for value in [0.0, 0.02, 0.18, 0.5, 1.0, 4.0] {
        let graded = l.grade_reference([value; 3]);
        for channel in graded {
            assert!(
                (channel - aces(value)).abs() < 1.0e-6,
                "neutral grade must be the bare curve at {value}: {channel} vs {}",
                aces(value)
            );
        }
    }
}

#[test]
fn the_battle_profile_pulls_deep_shade_to_true_black() {
    // The point of the black point: the ACES-lite curve lifts near-blacks (aces(0.02) ≈ 0.017,
    // never 0), which is why cast shadows read milky. The battle grade must map a 0.02 HDR input
    // essentially to black — real shade, not grey.
    let battle = SceneLighting::battlefield_default();
    let graded = battle.grade_reference([0.02; 3]);
    for channel in graded {
        assert!(channel < 0.005, "deep shade must grade to black, got {channel}");
    }
    // And it must not crush everything: mid grey stays mid, highlights stay bright.
    let mid = battle.grade_reference([0.18; 3]);
    assert!(mid[1] > 0.2 && mid[1] < 0.6, "mid grey survives the grade: {}", mid[1]);
    let bright = battle.grade_reference([4.0; 3]);
    assert!(bright[1] > 0.85, "highlights stay bright through the grade: {}", bright[1]);
}

#[test]
fn exposure_brightens_monotonically_before_the_curve() {
    let mut l = SceneLighting::battlefield_default();
    l.exposure = 0.8;
    let dim = l.grade_reference([0.3; 3]);
    l.exposure = 1.4;
    let bright = l.grade_reference([0.3; 3]);
    assert!(
        bright[1] > dim[1] + 0.05,
        "higher exposure must brighten the same radiance: {} vs {}",
        bright[1],
        dim[1]
    );
}

#[test]
fn fog_thickens_with_distance_and_thins_with_height() {
    let l = SceneLighting::battlefield_default();
    // No fog at the camera; a distant surface is heavily fogged; density is monotonic in distance.
    assert_eq!(l.fog_factor(0.0, 0.0), 0.0, "no fog at zero distance");
    let near = l.fog_factor(100.0, 0.0);
    let far = l.fog_factor(900.0, 0.0);
    assert!(far > near && near > 0.0, "fog thickens with distance: near={near} far={far}");
    assert!(far <= 1.0, "fog factor saturates at 1: {far}");
    // At the same distance, a surface higher up sits in thinner air and fogs less.
    let low = l.fog_factor(600.0, 0.0);
    let high = l.fog_factor(600.0, 60.0);
    assert!(high < low, "fog thins with height: low={low} high={high}");
    // Density 0 (interior) is fully clear regardless of distance.
    assert_eq!(SceneLighting::garage_studio().fog_factor(900.0, 0.0), 0.0);
}
