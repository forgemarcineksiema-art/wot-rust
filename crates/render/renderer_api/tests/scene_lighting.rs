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

fn all_profiles() -> [(&'static str, SceneLighting); 8] {
    [
        ("battlefield_default", SceneLighting::battlefield_default()),
        ("bystra_clear_afternoon", SceneLighting::bystra_clear_afternoon()),
        ("bystra_rain", SceneLighting::bystra_rain()),
        ("bystra_dawn_fog", SceneLighting::bystra_dawn_fog()),
        ("prokhorovka_golden_evening", SceneLighting::prokhorovka_golden_evening()),
        ("prokhorovka_overcast", SceneLighting::prokhorovka_overcast()),
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
fn every_outdoor_profile_keeps_the_hemispheric_ambient_real() {
    for (name, l) in all_profiles() {
        assert_ne!(
            l.ambient_rgb, l.ground_ambient_rgb,
            "{name}: sky and ground ambient must differ, or the hemisphere is a flat constant"
        );
    }
}

#[test]
fn the_golden_evening_sun_is_genuinely_low_and_warm() {
    let l = SceneLighting::prokhorovka_golden_evening();
    // A real golden hour: the NORMALIZED sun elevation sits in the long-shadow band. This is the
    // look the far shadow cascade exists to sell — a near-noon sun here would waste it.
    let d = l.key_direction;
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let elevation = d[1] / len;
    assert!(
        (0.12..=0.35).contains(&elevation),
        "golden-hour sun elevation must rake long shadows, got {elevation}"
    );
    // Amber light: red over green over blue, decisively.
    let [r, g, b] = l.key_rgb;
    assert!(r > g && g > b && r > b * 1.8, "the evening key must be warm amber: {:?}", l.key_rgb);
    // The overcast sibling is the opposite pole: high flat sun, near-neutral cool key.
    let o = SceneLighting::prokhorovka_overcast();
    assert!(o.key_direction[1] > o.key_direction[0].abs(), "overcast light comes from the lid");
    assert_eq!(o.cloud_shadow_strength, 0.0, "no cloud patches under a full lid");
    assert!(o.cloud_coverage_bias > 0.3, "the overcast profile must actually close the lid");
}

#[test]
fn the_prokhorovka_variants_are_three_different_days() {
    let noon = SceneLighting::battlefield_default();
    let evening = SceneLighting::prokhorovka_golden_evening();
    let overcast = SceneLighting::prokhorovka_overcast();
    let signature = |l: &SceneLighting| (l.key_direction, l.key_rgb, l.sky_horizon_rgb);
    assert_ne!(signature(&noon), signature(&evening));
    assert_ne!(signature(&noon), signature(&overcast));
    assert_ne!(signature(&evening), signature(&overcast));
}

#[test]
fn each_sky_look_owns_a_distinct_cloud_layer() {
    // Rain reads as an overcast lid: much thicker coverage than the clear looks, near-full
    // opacity — and NO terrain cloud shade (the lid itself is the shadow, moving patches under a
    // sunless sky would look wrong).
    let rain = SceneLighting::bystra_rain();
    let clear = SceneLighting::bystra_clear_afternoon();
    let dawn = SceneLighting::bystra_dawn_fog();
    assert!(
        rain.cloud_coverage_bias > clear.cloud_coverage_bias + 0.2,
        "rain must push the coverage into a lid: {} vs {}",
        rain.cloud_coverage_bias,
        clear.cloud_coverage_bias
    );
    assert_eq!(rain.cloud_shadow_strength, 0.0, "no cloud shade under an overcast lid");
    // The three Bystra looks are genuinely different skies, not one cloud layer recoloured.
    let params = |l: &SceneLighting| {
        (l.cloud_coverage_bias, l.cloud_scale, l.cloud_opacity, l.cloud_shadow_strength)
    };
    assert_ne!(params(&rain), params(&clear));
    assert_ne!(params(&clear), params(&dawn));
    assert_ne!(params(&rain), params(&dawn));
    // Interiors carry no sky layer at all.
    for interior in [SceneLighting::garage_studio(), SceneLighting::garage_workshop()] {
        assert_eq!(interior.cloud_opacity, 0.0);
        assert_eq!(interior.cloud_shadow_strength, 0.0);
        assert_eq!(interior.fog_sun_scatter, 0.0);
    }
}

#[test]
fn sun_scatter_and_clouds_never_touch_the_fog_amount() {
    // The 400 m spotting-fairness bound rests on fog_factor; the sky phase adds COLOUR only.
    // Wildly different scatter/cloud settings must leave the fog amount bit-identical.
    let base = SceneLighting::battlefield_default();
    let mut wild = base;
    wild.fog_sun_scatter = 1.0;
    wild.cloud_coverage_bias = 0.5;
    wild.cloud_opacity = 1.0;
    wild.cloud_shadow_strength = 1.0;
    for (distance, height) in [(100.0, 0.0), (400.0, 0.0), (600.0, 40.0), (900.0, 5.0)] {
        assert_eq!(
            base.fog_factor(distance, height),
            wild.fog_factor(distance, height),
            "fog amount must not depend on the sky colour params"
        );
    }
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
