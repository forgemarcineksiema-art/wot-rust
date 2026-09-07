//! D39: every exterior role keeps a lobe of its own, in a fixed order, and steel mirrors the
//! sky where rubber does not.

use renderer_api::{ExteriorRole, environment_energy, specular_amplitude, surface_roughness};

/// The ladder the shader's `material_params` carries, smooth to rough.
const LADDER: [ExteriorRole; 8] = [
    ExteriorRole::Glass,
    ExteriorRole::BarrelSteel,
    ExteriorRole::TrackMetal,
    ExteriorRole::RolledArmor,
    ExteriorRole::CastArmor,
    ExteriorRole::Rubber,
    ExteriorRole::Timber,
    ExteriorRole::Canvas,
];

#[test]
fn every_exterior_role_keeps_a_lobe_of_its_own_in_a_fixed_order() {
    let mut previous = f32::INFINITY;
    for role in LADDER {
        // At the lane's roughest texel, the finest grain and no dust: the worst case for a lobe.
        // Canvas and timber are matte by authorship (duck cloth, seasoned wood) and may go
        // fully matte on their roughest texels; every steel, rubber and glass role may not.
        let matte_by_authorship = matches!(role, ExteriorRole::Canvas | ExteriorRole::Timber);
        let roughness = surface_roughness(role, 1.0, 0.5, 0.0);
        assert!(
            matte_by_authorship || roughness < 1.0,
            "{role:?} saturates at roughness {roughness}: no lobe, no sky"
        );
        let lobe = specular_amplitude(roughness);
        assert!(matte_by_authorship || lobe > 0.0, "{role:?}: a zero lobe");
        // The order is a strict ladder at the finish itself.
        let at_finish = specular_amplitude(surface_roughness(role, 0.5, 0.5, 0.0));
        assert!(
            at_finish < previous,
            "{role:?}: the ladder is not ordered ({at_finish} >= {previous})"
        );
        previous = at_finish;
    }
}

#[test]
fn worn_track_steel_mirrors_the_sky_where_rubber_does_not() {
    let energy = |role: ExteriorRole| {
        environment_energy(surface_roughness(role, 0.5, 0.5, 0.0), role.metalness())
    };
    let track = energy(ExteriorRole::TrackMetal);
    let rubber = energy(ExteriorRole::Rubber);
    assert!(
        track > 5.0 * rubber,
        "track metal must carry > 5x the sky of rubber: {track:.4} vs {rubber:.4}"
    );
    // Paint stays paint: a rolled plate reflects the sky as a grazing highlight, never like
    // bare steel face-on.
    assert!(energy(ExteriorRole::RolledArmor) < track);
}
