//! THE MATERIAL FLOOR: a surface treatment that cannot be seen is not a surface treatment.
//!
//! This vehicle stack has had per-role PBR since long before this file: albedo, **normal**,
//! AO/roughness/metalness and cavity maps, synthesized per material family, sampled either through
//! authored UVs or triplanar projection, cached into Forge artifacts, and loaded by the client.
//! Everything worked. And every tank in the fleet read as one flat plastic tone, because the
//! amplitudes were set to nothing: `normal_jitter` 3/255 on rolled armour, 5/255 on cast — a 1.2%
//! perturbation, with the two armour families separated by two parts in 255.
//!
//! That is the failure this file exists to make impossible to repeat, and it is worth naming
//! precisely: **nothing was broken.** No test failed, no budget was exceeded, no gate went red.
//! A knob was turned down and the result looked like a modelling problem for months.
//!
//! So the assertions below are a FLOOR, in the sense `docs/vehicle-geometry-policy.md` gives the
//! word: not "this must not grow" but "this must be enough to see". They are deliberately far
//! below the shipping values — a floor is not a target, and re-tuning the look must stay free.
//!
//! Measured 2026-08-08, before (x1) and after (x4) the calibration, mean absolute deviation in
//! 0..=255 units:
//!
//! | family | normal tilt | albedo grain | cavity span |
//! |---|---|---|---|
//! | RolledArmor | 1.00 -> 3.24 | 1.17 -> 3.67 | 6 -> 24 |
//! | CastArmor | 1.51 -> 5.23 | 2.36 -> 8.56 | 3 -> 12 |
//! | BarrelSteel | 0.67 -> 2.22 | 0.75 -> 2.52 | 3 -> 12 |
//! | TrackMetal | 1.71 -> 6.23 | 1.77 -> 6.35 | 16 -> 64 |
//! | Rubber | 0.67 -> 2.22 | 1.06 -> 3.44 | 8 -> 32 |
//!
//! The roughness assertion below was deliberately absent when this file was written — the channel
//! had span 0 in every family, and asserting it then would have painted the defect green by
//! lowering the bar to what already shipped. It is here now because the defect is fixed, which is
//! the order these two things have to happen in.

use vehicle_forge::{DefaultMaterialFamily, default_material_families};

/// Mean absolute deviation of a channel from `flat`, in 0..=255 units.
fn mean_deviation(rgba: &[u8], channel: usize, flat: u8) -> f32 {
    let mut total = 0.0_f64;
    let mut count = 0.0_f64;
    for pixel in rgba.chunks_exact(4) {
        total += (f32::from(pixel[channel]) - f32::from(flat)).abs() as f64;
        count += 1.0;
    }
    (total / count.max(1.0)) as f32
}

/// The tangent-space normal map's X/Y are the perturbation; 128 is "no tilt at all".
fn normal_tilt(family: &DefaultMaterialFamily) -> f32 {
    let rgba = family.normal().rgba();
    mean_deviation(rgba, 0, 128).max(mean_deviation(rgba, 1, 128))
}

/// A normal map that never leaves flat is a flat surface with extra memory cost. The shipping
/// values sit far above this; the floor only forbids the invisible.
const MIN_NORMAL_TILT: f32 = 1.8;

#[test]
fn every_material_role_perturbs_its_normal_enough_to_be_seen() {
    for family in default_material_families() {
        let tilt = normal_tilt(family);
        assert!(
            tilt >= MIN_NORMAL_TILT,
            "{:?}: mean normal tilt {tilt:.2}/255 — below the floor of {MIN_NORMAL_TILT}. A normal \
             map this flat renders as no normal map at all, which is exactly how this fleet spent \
             months looking like plastic.",
            family.family()
        );
    }
}

/// Cast steel and rolled plate are the same paint and must stay so; what separates them is the
/// SURFACE — a sand-cast turret undulates where a rolled plate is smooth. They were 2/255 apart.
const MIN_CAST_VS_ROLLED_SEPARATION: f32 = 1.2;

#[test]
fn cast_armour_and_rolled_plate_do_not_wear_the_same_surface() {
    let families = default_material_families();
    let cast = families
        .iter()
        .find(|f| format!("{:?}", f.family()).contains("Cast"))
        .expect("a cast armour family");
    let rolled = families
        .iter()
        .find(|f| format!("{:?}", f.family()).contains("Rolled"))
        .expect("a rolled armour family");

    let separation = (normal_tilt(cast) - normal_tilt(rolled)).abs();
    assert!(
        separation >= MIN_CAST_VS_ROLLED_SEPARATION,
        "cast and rolled armour differ by {separation:.2}/255 of normal tilt — under the floor of \
         {MIN_CAST_VS_ROLLED_SEPARATION}. A casting that answers light exactly like a plate is the \
         single most expensive thing a tank can get wrong up close, because the silhouette is \
         already right and only the surface can tell them apart."
    );
}

/// Cavity darkens the crevices the normal map implies. Flat cavity wastes the lane.
#[test]
fn every_material_role_carries_a_cavity_signal() {
    for family in default_material_families() {
        let rgba = family.cavity().rgba();
        let (min, max) =
            rgba.chunks_exact(4).fold((255_u8, 0_u8), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
        assert!(
            max - min >= 8,
            "{:?}: cavity spans only {} levels — the lane is uploaded and says nothing.",
            family.family(),
            max - min
        );
    }
}

/// A surface's FINISH has to vary, or every part answers the sun identically.
///
/// This channel shipped constant: span 0 in every family. `vehicle.wgsl` reads it
/// (`rough_base = role_roughness * (0.55 + ao_rough.g)`), so it was multiplying by the same number
/// everywhere — a pitted casting and a polished track contact face reflecting alike no matter how
/// much normal or cavity detail either carried.
const MIN_ROUGHNESS_SPAN: u8 = 8;

#[test]
fn every_material_role_varies_its_finish_across_the_surface() {
    for family in default_material_families() {
        let rgba = family.ao_roughness_metalness().rgba();
        // Green is the roughness channel of the packed ao/roughness/metalness map.
        let (min, max) =
            rgba.chunks_exact(4).fold((255_u8, 0_u8), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
        assert!(
            max - min >= MIN_ROUGHNESS_SPAN,
            "{:?}: roughness spans {} levels — under the floor of {MIN_ROUGHNESS_SPAN}. A constant              finish is one gloss for the whole vehicle, which is what this channel did for months.",
            family.family(),
            max - min
        );
    }
}
