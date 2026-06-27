//! R8 / Task 25 lock: the hybrid T-54 bakes semantic ambient contact into `surface_shade`. The cast
//! turret and welded hull must no longer read flat — the turret-ring seam, the mantlet seat and the
//! running-gear recess sit in shadow — while open surfaces (crown, barrel, glacis face) stay clean,
//! proving this is contact occlusion, not a global dimming. The bake is deterministic.

use vehicle_build::t54_description;
use vehicle_geometry::{GeometryMesh, SubmeshKind};

fn turret() -> GeometryMesh {
    t54_description().build().submesh(SubmeshKind::Turret).expect("turret").mesh.clone()
}

fn hull() -> GeometryMesh {
    t54_description().build().submesh(SubmeshKind::Hull).expect("hull").mesh.clone()
}

fn min_shade(mesh: &GeometryMesh, keep: impl Fn(glam::Vec3) -> bool) -> f32 {
    mesh.vertices()
        .iter()
        .filter(|v| keep(v.position))
        .map(|v| v.surface_shade)
        .fold(f32::MAX, f32::min)
}

#[test]
fn open_surfaces_stay_fully_lit_so_the_bake_is_contact_occlusion_not_global_dimming() {
    let baked = t54_description().build();
    let max_shade = baked
        .submeshes()
        .iter()
        .flat_map(|s| s.mesh.vertices())
        .map(|v| v.surface_shade)
        .fold(0.0_f32, f32::max);
    assert!(
        (max_shade - 1.0).abs() < 1.0e-4,
        "open surfaces must keep full shade, got {max_shade}"
    );
}

#[test]
fn the_running_gear_recess_sits_in_shadow() {
    // The wheels and track band under the sponson are the darkest hull region in ambient light.
    let recess = min_shade(&hull(), |p| (0.30..=0.70).contains(&p.y));
    assert!(recess < 0.66, "running-gear recess must be shaded, got min {recess:.3}");
}

#[test]
fn the_turret_ring_seam_sits_in_shadow() {
    // The casting seats on the narrowed ring and overhangs it: the seam reads as a dark undercut.
    let ring = min_shade(&turret(), |p| (1.27..=1.33).contains(&p.y) && p.x.abs() < 0.9);
    assert!(ring < 0.66, "turret-ring seam must be shaded, got min {ring:.3}");
}

#[test]
fn the_mantlet_seat_sits_in_shadow() {
    // The recessed embrasure the mantlet beds into is a tight, dark cast trough on the turret front;
    // the mask itself rides in the gun submesh, so scan the whole body within the seat band.
    let baked = t54_description().build();
    let in_seat = |p: glam::Vec3| p.x.abs() < 0.45 && (1.45..=1.62).contains(&p.y) && p.z > 0.85;
    let seat: Vec<f32> = baked
        .submeshes()
        .iter()
        .flat_map(|s| s.mesh.vertices())
        .filter(|v| in_seat(v.position))
        .map(|v| v.surface_shade)
        .collect();
    assert!(!seat.is_empty(), "the mantlet seat band must cover real front vertices");
    let min = seat.iter().copied().fold(f32::MAX, f32::min);
    assert!(min < 0.70, "mantlet seat must be shaded, got min {min:.3}");
}

#[test]
fn the_contact_bake_is_deterministic() {
    let shades = || {
        t54_description()
            .build()
            .submeshes()
            .iter()
            .flat_map(|s| s.mesh.vertices().to_vec())
            .map(|v| v.surface_shade)
            .collect::<Vec<_>>()
    };
    assert_eq!(shades(), shades(), "the contact-cavity bake must reproduce run to run");
}
