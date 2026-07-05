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
