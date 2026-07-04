//! Silhouette-quality gates: checks that catch a *bad-looking* model even when its bounding-box
//! proportions are fine. The Forge ratio tests only measure the overall box (length/width/height),
//! so they happily pass a turret shaped like an onion, a mantlet stuck on as a detached ball, or a
//! hull that reads as a wide slab. These gates encode specific, measurable shape defects and must
//! fail on a model that reads wrong.
//!
//! NOTE: geometry gates are necessarily blunt — the authoritative "does it look like the real
//! vehicle" check is the rendered review-image set (front/side/top/oblique) reviewed by a human.
//! These tests catch the worst measurable sins so the benchmark stops rubber-stamping rough blobs.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_geometry::{GeometryMesh, MaterialRole, MeshBounds, SubmeshKind, bake_vehicle};

const SOVIET_LEGACY_PAIR: [VehicleKind; 2] = [VehicleKind::T54_1951, VehicleKind::T55A];

fn band_half_width(mesh: &GeometryMesh, y_lo: f32, y_hi: f32) -> f32 {
    mesh.vertices()
        .iter()
        .filter(|v| v.position.y >= y_lo && v.position.y <= y_hi)
        .map(|v| v.position.x.abs())
        .fold(0.0_f32, f32::max)
}

fn submesh_bounds(kind: VehicleKind, sub: SubmeshKind) -> MeshBounds {
    bake_vehicle(kind).unwrap().submesh(sub).unwrap().mesh.bounds().unwrap()
}

fn material_max_z(mesh: &GeometryMesh, material: MaterialRole) -> Option<f32> {
    mesh.vertices()
        .iter()
        .filter(|v| v.material == material)
        .map(|v| v.position.z)
        .fold(None, |acc, z| Some(acc.map_or(z, |a: f32| a.max(z))))
}

/// The cast Soviet turret must read as a flat "frying-pan" dome, not a pointed onion: its roof must
/// keep a real fraction of the shoulder beam. (T-54 benchmark plus legacy-compatible T-55A.)
#[test]
fn soviet_cast_turret_is_a_dome_not_an_onion() {
    for kind in SOVIET_LEGACY_PAIR {
        let baked = bake_vehicle(kind).unwrap();
        let turret = &baked.submesh(SubmeshKind::Turret).unwrap().mesh;
        let b = turret.bounds().unwrap();
        let shoulder = band_half_width(turret, b.min.y + 0.10, b.min.y + 0.40);
        let roof = band_half_width(turret, b.max.y - 0.30, b.max.y);
        let ratio = roof / shoulder.max(0.01);
        assert!(
            ratio >= 0.60,
            "{kind:?} turret roof/shoulder width = {ratio:.2} (roof {roof:.2} / shoulder \
             {shoulder:.2}) — reads as a pointed onion, not a T-54 frying-pan dome"
        );
    }
}

/// The turret must carry a real commander's cupola standing proud of the roof, not a flat pimple —
/// a dead giveaway that the turret is an unfinished blob.
#[test]
fn soviet_turret_has_a_real_cupola_drum() {
    for kind in SOVIET_LEGACY_PAIR {
        let turret_top = submesh_bounds(kind, SubmeshKind::Turret).max.y;
        let roof_y = VehicleBlueprint::for_vehicle(kind).unwrap().turret.roof_y;
        let crown = turret_top - roof_y;
        assert!(
            crown >= 0.12,
            "{kind:?} cupola crowns the roof by only {crown:.2} (top {turret_top:.2} vs roof \
             {roof_y:.2}) — reads as a flat pimple, not a commander's cupola"
        );
    }
}

/// The mantlet must sit *in* the turret front, not poke ahead of it as a separate ball. The gun's
/// cast mantlet front must not protrude past the turret shell front by more than a small margin.
#[test]
fn soviet_gun_mantlet_is_integrated_not_a_detached_ball() {
    for kind in SOVIET_LEGACY_PAIR {
        let baked = bake_vehicle(kind).unwrap();
        let turret_front = baked.submesh(SubmeshKind::Turret).unwrap().mesh.bounds().unwrap().max.z;
        let gun = &baked.submesh(SubmeshKind::Gun).unwrap().mesh;
        let mantlet_front =
            material_max_z(gun, MaterialRole::CastArmor).expect("gun has a cast mantlet");
        assert!(
            mantlet_front <= turret_front + 0.06,
            "{kind:?} mantlet front z={mantlet_front:.2} pokes past turret front z={turret_front:.2} \
             — reads as a detached ball on the turret nose"
        );
    }
}

/// The hull must read as a long Soviet medium, not a wide slab: length/width >= 1.80 (the real
/// T-54 is ~1.83).
#[test]
fn soviet_hull_reads_long_not_a_wide_slab() {
    for kind in SOVIET_LEGACY_PAIR {
        let hull = submesh_bounds(kind, SubmeshKind::Hull);
        let length = hull.max.z - hull.min.z;
        let width = hull.max.x - hull.min.x;
        let ratio = length / width.max(0.01);
        assert!(
            ratio >= 1.80,
            "{kind:?} hull length/width = {ratio:.2} (len {length:.2} / width {width:.2}) — too \
             wide, reads as a slab not a low Soviet medium"
        );
    }
}

/// The running gear must fill the lower hull side, not float as a thin strip over a dark recess.
/// The track + road-wheel band must span a real fraction of the hull height.
#[test]
fn soviet_running_gear_fills_the_lower_hull_side() {
    for kind in SOVIET_LEGACY_PAIR {
        let baked = bake_vehicle(kind).unwrap();
        let hull = &baked.submesh(SubmeshKind::Hull).unwrap().mesh;
        let hull_bounds = hull.bounds().unwrap();
        let hull_height = hull_bounds.max.y - hull_bounds.min.y;
        let gear: Vec<f32> = hull
            .vertices()
            .iter()
            .filter(|v| matches!(v.material, MaterialRole::TrackMetal | MaterialRole::Rubber))
            .map(|v| v.position.y)
            .collect();
        let lo = gear.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = gear.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let coverage = (hi - lo) / hull_height.max(0.01);
        // On the documented 1:1 proportions the track band spans ~0.95 of the 1.75 hull side
        // (≈0.54); anything much thinner reads as a hull floating on toy tracks.
        assert!(
            coverage >= 0.50,
            "{kind:?} running gear covers {coverage:.2} of hull height (y {lo:.2}..{hi:.2}) — too \
             thin, the hull reads as floating on small tracks"
        );
    }
}
