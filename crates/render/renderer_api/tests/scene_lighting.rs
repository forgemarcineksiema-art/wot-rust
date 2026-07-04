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
