//! T-54 gun parts built from the revolve generator: the main gun barrel and the moving cast mantlet.
//! Every dimension is read from the vehicle blueprint's [`GunVisual`] — the single source — rather
//! than held here, so the geometry cannot drift from the blueprint.

use game_core::GunVisual;
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::{revolve, translate};

/// The barrel side profile from breech (0) to muzzle (`length`).
///
/// A gun is a BORE with steel wrapped round it. This profile used to be written the other way
/// about — the hole was `muzzle_radius * 0.55`, a fraction of the outside — and the result was a
/// muzzle whose face was 76% flat steel with a shallow dimple in the middle. Three quarters of
/// the thing you look at when a tank points its gun at you was the wrong shape.
///
/// Now the bore is the gun's calibre (`GunVisual::bore_radius`), the wall is whatever
/// `muzzle_radius` leaves around it, and the recess runs deep enough to read as a hole rather
/// than a dent: at least one and a half bore diameters, which is past the point where a viewer
/// can see the bottom of it at any normal angle.
fn barrel_profile(length: f32, gun: &GunVisual) -> Vec<(f32, f32)> {
    let r = gun.barrel_radius;
    let collar_r = gun.muzzle_radius * 1.06;
    let collar_len = (length * 0.06).clamp(0.06, 0.26);
    // The hole is the calibre. Nothing derives it from the outside of the tube.
    let bore = gun.bore_radius;
    // Deep enough to be a hole: 1.5 bore diameters of dark before anything stops the eye.
    let recess = (bore * 3.0).clamp(0.10, 0.30);
    vec![
        (0.0, 0.0),
        (0.0, r * 1.18),
        (length * 0.10, r * 1.04),
        (length * 0.42, r * 0.98),
        (length * 0.45, r * 0.90),
        (length - collar_len - gun.muzzle_taper, r * 0.82),
        (length - collar_len, collar_r),
        (length, collar_r),
        // The muzzle FACE: a thin ring of steel between the collar and the bore. On a D-10T that
        // ring is 10-13 mm wide, and it is the whole reason the muzzle reads as an opening.
        (length, bore),
        // A straight run of bore at FULL diameter — this is the dark the eye falls into, and it
        // has to be the whole depth, not a cone starting at the face.
        (length - recess, bore),
        // Only then the funnel that closes the tube out of sight.
        (length - recess - bore * 0.8, 0.0),
    ]
}

/// The main gun barrel along +Z from a fixed breech station. `length` is the breech-to-muzzle
/// span — driven by the installed gun module, not a post-bake scale.
pub fn gun_barrel(length: f32, gun: &GunVisual) -> GeometryMesh {
    let breech = 1.0;
    let profile: Vec<(f32, f32)> =
        barrel_profile(length.max(0.5), gun).into_iter().map(|(z, r)| (breech + z, r)).collect();
    revolve(Vec3::Z, &profile, gun.barrel_segments, MaterialRole::BarrelSteel, SmoothingGroup(4))
}

/// A barrel mounted between the authoritative trunnion and muzzle frames in vehicle-local space.
pub fn gun_barrel_between(trunnion: Vec3, muzzle: Vec3, gun: &GunVisual) -> GeometryMesh {
    let length = (muzzle.z - trunnion.z).max(0.5);
    translate(
        &revolve(
            Vec3::Z,
            &barrel_profile(length, gun),
            gun.barrel_segments,
            MaterialRole::BarrelSteel,
            SmoothingGroup(4),
        ),
        trunnion,
    )
}

/// The moving cast mantlet mask, seated at the trunnion: a profile revolved about Z then squashed to
/// a wide flat oval (so it reads as a mask, not a round ball) by the blueprint's mantlet scale.
pub fn moving_mantlet(trunnion: Vec3, gun: &GunVisual) -> GeometryMesh {
    let base = revolve(
        Vec3::Z,
        &gun.mantlet_profile,
        gun.mantlet_segments,
        MaterialRole::CastArmor,
        SmoothingGroup(2),
    );
    let s = gun.mantlet_scale;
    let vertices = base
        .vertices()
        .iter()
        .map(|v| GeometryVertex {
            position: trunnion
                + Vec3::new(v.position.x * s.x, v.position.y * s.y, v.position.z * s.z),
            normal: Vec3::new(v.normal.x / s.x, v.normal.y / s.y, v.normal.z / s.z)
                .normalize_or_zero(),
            ..*v
        })
        .collect();
    GeometryMesh::new(vertices, base.indices().to_vec()).weld_and_smooth()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gun() -> GunVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .gun
    }

    #[test]
    fn the_barrel_reaches_forward_past_the_turret() {
        let b = gun_barrel(3.6, &gun()).bounds().expect("non-empty");
        assert!(b.max.z > 4.5, "muzzle reaches forward: {:.2}", b.max.z);
        assert!(b.max.x < 0.12 && b.max.y < 0.12, "barrel stays slim");
    }

    #[test]
    fn mounted_barrel_uses_the_authoritative_frames() {
        let trunnion = Vec3::new(0.0, 1.70, 1.10);
        let muzzle = Vec3::new(0.0, 1.70, 5.20);
        let barrel = gun_barrel_between(trunnion, muzzle, &gun());
        let b = barrel.bounds().expect("non-empty");
        assert!(b.min.y < trunnion.y && b.max.y > trunnion.y, "barrel surrounds trunnion height");
        assert!((b.max.z - muzzle.z).abs() < 1.0e-4, "muzzle reaches its authoritative frame");
    }

    #[test]
    fn a_longer_gun_module_makes_a_longer_barrel() {
        let short = gun_barrel(3.0, &gun()).bounds().unwrap().max.z;
        let long = gun_barrel(4.5, &gun()).bounds().unwrap().max.z;
        assert!(long > short + 1.0, "barrel length tracks the module: {long:.2} vs {short:.2}");
    }

    #[test]
    fn the_mantlet_is_wider_than_it_is_tall() {
        let m = moving_mantlet(Vec3::new(0.0, 1.70, 1.10), &gun()).bounds().expect("non-empty");
        assert!((m.max.x - m.min.x) > (m.max.y - m.min.y), "the trunnion bosses spread it wide");
    }

    /// K2. A cast mantlet is a body. This test used to demand 0.50 m of depth — the length of the
    /// external sleeve the T-54 does not have — and said nothing at all about the two open rims
    /// that sleeve left standing in mid-air.
    #[test]
    fn the_mantlet_is_a_closed_body_not_an_open_sleeve() {
        let mesh = moving_mantlet(Vec3::new(0.0, 1.70, 1.10), &gun());
        let report = mesh.quality_report(vehicle_geometry::OPEN_OR_CLOSED_MESH);
        assert_eq!(report.boundary_edges, 0, "a cast body has no open rims");
        assert_eq!(report.non_manifold_edges, 0, "and no seams meeting three ways");
        let m = mesh.bounds().expect("non-empty");
        let depth = m.max.z - m.min.z;
        assert!(
            (0.15..=0.40).contains(&depth),
            "an internal mantlet is compact, got {depth:.3} m deep"
        );
    }
}
