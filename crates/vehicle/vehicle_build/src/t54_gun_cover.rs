//! The canvas dust cover over the T-54's gun embrasure.
//!
//! A vehicle whose mantlet is INSIDE the turret has a hole in its turret face. Something closes
//! that hole against rain, dust and the eye, and on a T-54 that something is a proofed canvas
//! boot clamped to the aperture frame at one end and to the gun tube at the other. It is the part
//! of this gun mount a viewer actually sees — the internal mantlet behind it is, by construction,
//! not visible from outside.
//!
//! Built with [`sweep`] rather than a revolve on purpose: fabric hangs. A revolve about the barrel
//! axis can only make a body of revolution, and a body of revolution is exactly the thing a boot
//! stretched between two clamps is not. The sweep's path droops between them and its
//! `section_scale` does the flare — the case that parameter was added for.

use game_core::GunVisual;
use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// Section samples around the boot. Sixteen is enough that the fabric reads round at the clamp
/// without the facets showing at the flare.
const COVER_SEGMENTS: usize = 16;

/// The canvas boot between the embrasure frame and the barrel clamp, in vehicle-local space.
///
/// `trunnion` is the gun's authoritative trunnion frame; the blueprint's cover stations are
/// trunnion-relative `(z, radius)` on the barrel axis.
pub fn t54_mantlet_cover(trunnion: Vec3, gun: &GunVisual) -> GeometryMesh {
    let stations = gun.mantlet_cover;
    let span = stations[stations.len() - 1].0 - stations[0].0;

    // The droop: zero at both clamps, deepest in between. A half-sine rather than a parabola so
    // the fabric leaves each clamp along the axis instead of kinking away from it.
    let points: Vec<Vec3> = stations
        .iter()
        .map(|&(z, _)| {
            let t = if span.abs() > 1.0e-6 { (z - stations[0].0) / span } else { 0.0 };
            let sag = gun.mantlet_cover_sag * (t * std::f32::consts::PI).sin();
            trunnion + Vec3::new(0.0, -sag, z)
        })
        .collect();
    let scales: Vec<f32> = stations.iter().map(|&(_, radius)| radius).collect();

    let section = SweepSection {
        points: (0..COVER_SEGMENTS)
            .map(|index| {
                let angle = index as f32 / COVER_SEGMENTS as f32 * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                Vec2::new(cos, sin)
            })
            .collect(),
        closed: true,
    };
    let path = SweepPath { points, closed: false };

    try_sweep(&SweepSpec {
        path: &path,
        section: &section,
        // The boot is a planar, gently drooping run: pinning the section's up axis to world Y
        // keeps it from rolling, which parallel transport has no reason not to do.
        frame_mode: SweepFrameMode::FixedUp(Vec3::Y),
        // Both ends are SEATED, not open to view: the rear station is buried inside the casting
        // and the front one grips the tube. Capping them would put lids inside solid geometry.
        caps: SweepCaps::Open,
        material: MaterialRole::Canvas,
        smoothing: SmoothingGroup(6),
        section_scale: Some(&scales),
    })
    // The T-54 cover stations are static, validated authoring data locked by the gun tests; an
    // error here means the blueprint regressed, not bad runtime input.
    .expect("the T-54 mantlet cover blueprint is a valid sweep")
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{VehicleBlueprint, VehicleKind};

    fn gun() -> GunVisual {
        VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap().gun
    }

    #[test]
    fn the_cover_flares_from_the_barrel_clamp_out_to_the_embrasure_frame() {
        let trunnion = Vec3::new(0.0, 1.78, 1.15);
        let cover = t54_mantlet_cover(trunnion, &gun());
        let radius_near = |z: f32| {
            cover
                .vertices()
                .iter()
                .filter(|v| (v.position.z - z).abs() < 0.02)
                .map(|v| v.position.x.hypot(v.position.y - trunnion.y))
                .fold(0.0_f32, f32::max)
        };
        let frame = radius_near(trunnion.z + gun().mantlet_cover[0].0);
        let clamp = radius_near(trunnion.z + gun().mantlet_cover[3].0);
        assert!(
            frame > clamp * 2.0,
            "the boot must open out from the tube to the frame, got {frame:.3} vs {clamp:.3}"
        );
    }

    #[test]
    fn the_cover_hangs_instead_of_running_straight() {
        let trunnion = Vec3::new(0.0, 1.78, 1.15);
        let cover = t54_mantlet_cover(trunnion, &gun());
        // Sampled AT the stations, with a window wide enough to catch a whole ring. The sweep
        // puts geometry only where the path has points, and it tilts each section with the path's
        // tangent — so a sagging boot's rings are not planar in z, and a midspan sample taken
        // between two stations finds nothing at all.
        let axis_at = |z: f32| {
            let (low, high) = cover
                .vertices()
                .iter()
                .filter(|v| (v.position.z - z).abs() < 0.045 && v.position.x.abs() < 0.02)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v.position.y), hi.max(v.position.y))
                });
            assert!(low.is_finite(), "the boot has fabric at z {z:.3}");
            (low + high) * 0.5
        };
        let stations = gun().mantlet_cover;
        let clamp = axis_at(trunnion.z + stations[0].0);
        let hanging = axis_at(trunnion.z + stations[2].0);
        assert!(
            (clamp - trunnion.y).abs() < 1.0e-3,
            "the boot leaves its frame clamp on the gun axis, got {clamp:.4}"
        );
        assert!(
            hanging < trunnion.y - 0.004,
            "canvas hangs: the free span at {hanging:.4} must sit below the axis {:.4}",
            trunnion.y
        );
    }
}
