//! T-54 gun parts built from the revolve generator: the main gun barrel and the moving cast mantlet.
//! Every dimension is read from the vehicle blueprint's [`GunVisual`] — the single source — rather
//! than held here, so the geometry cannot drift from the blueprint.

use game_core::GunVisual;
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::{revolve, translate};

/// The barrel side profile from breech (0) to muzzle (`length`): a thick root sleeve behind the
/// mask, the heavier rear tube half, the D-10 family's mid-tube STEP down to the slim front half,
/// a stepped muzzle collar, and a RECESSED bore — the muzzle ends as a steel ring with the dark
/// bore tube set back inside it, never a solid capped rod.
fn barrel_profile(length: f32, gun: &GunVisual) -> Vec<(f32, f32)> {
    let r = gun.barrel_radius;
    let collar_r = gun.muzzle_radius * 1.13;
    let collar_len = (length * 0.06).clamp(0.06, 0.26);
    let bore = gun.muzzle_radius * 0.55;
    let recess = (length * 0.035).clamp(0.04, 0.15);
    vec![
        (0.0, 0.0),
        (0.0, r * 1.18),
        (length * 0.10, r * 1.04),
        (length * 0.42, r * 0.98),
        (length * 0.45, r * 0.90),
        (length - collar_len - gun.muzzle_taper, r * 0.82),
        (length - collar_len, collar_r),
        (length, collar_r),
        (length, bore),
        (length - recess, bore * 0.95),
        (length - recess, 0.0),
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
    fn the_moving_mantlet_is_a_wide_flat_oval() {
        let m = moving_mantlet(Vec3::new(0.0, 1.70, 1.10), &gun()).bounds().expect("non-empty");
        assert!((m.max.x - m.min.x) > (m.max.y - m.min.y), "mantlet reads wider than tall");
    }

    #[test]
    fn the_t54_mantlet_has_a_deep_stepped_mask_profile() {
        let m = moving_mantlet(Vec3::new(0.0, 1.70, 1.10), &gun()).bounds().expect("non-empty");
        assert!(
            m.max.z - m.min.z >= 0.50,
            "T-54 mantlet needs a substantial stepped depth, got {:.3}",
            m.max.z - m.min.z
        );
    }
}
