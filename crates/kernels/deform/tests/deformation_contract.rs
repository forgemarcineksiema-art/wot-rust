//! The bake-only deformation contract: deformation touches *only* a visual mesh, stays inside its
//! configured tolerance, reproduces across bakes, and leaves finite normals — so gameplay truth
//! (hitbox, armour facets, mount frames, anchors) it never even receives cannot drift.

use deform::{DeformationKind, DeformationSpec, deform};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

/// A small smooth cylinder shell standing in for a cast surface patch.
fn shell() -> GeometryMesh {
    let (rings, slices) = (6_usize, 20_usize);
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for i in 0..=rings {
        let y = i as f32 / rings as f32 - 0.5;
        for j in 0..=slices {
            let a = j as f32 / slices as f32 * std::f32::consts::TAU;
            let n = Vec3::new(a.cos(), 0.0, a.sin());
            v.push(GeometryVertex::new(
                Vec3::new(a.cos(), y, a.sin()),
                n,
                MaterialRole::CastArmor,
                SmoothingGroup(3),
            ));
        }
    }
    let w = slices + 1;
    for i in 0..rings {
        for j in 0..slices {
            let p = (i * w + j) as u32;
            idx.extend_from_slice(&[p, p + w as u32, p + 1, p + 1, p + w as u32, p + w as u32 + 1]);
        }
    }
    GeometryMesh::new(v, idx).weld_and_smooth()
}

fn wear_spec(seed: u64) -> DeformationSpec {
    DeformationSpec {
        kind: DeformationKind::SurfaceWear,
        center: Vec3::ZERO,
        radius: 3.0,
        amplitude: 0.05,
        seed,
    }
}

#[test]
fn a_bake_reproduces_byte_for_byte() {
    let base = shell();
    let first = deform(&base, &wear_spec(99), 0.02);
    for _ in 0..6 {
        let again = deform(&base, &wear_spec(99), 0.02);
        assert_eq!(first.vertices().len(), again.vertices().len());
        for (a, b) in first.vertices().iter().zip(again.vertices()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
        }
    }
}

#[test]
fn different_seeds_give_different_surfaces() {
    let base = shell();
    let a = deform(&base, &wear_spec(1), 0.02);
    let b = deform(&base, &wear_spec(2), 0.02);
    let differs = a.vertices().iter().zip(b.vertices()).any(|(p, q)| p.position != q.position);
    assert!(differs, "the seed must change the bake");
}

#[test]
fn displacement_never_escapes_the_tolerance_volume() {
    let base = shell();
    let tolerance = 0.015;
    let out = deform(&base, &wear_spec(5), tolerance);
    for (before, after) in base.vertices().iter().zip(out.vertices()) {
        assert!((after.position - before.position).length() <= tolerance + 1.0e-5);
    }
}

#[test]
fn amplitude_above_tolerance_is_still_clamped_to_tolerance() {
    // Even a reckless amplitude cannot push past the configured visual tolerance.
    let base = shell();
    let spec = DeformationSpec { amplitude: 100.0, ..wear_spec(3) };
    let tolerance = 0.01;
    let out = deform(&base, &spec, tolerance);
    for (before, after) in base.vertices().iter().zip(out.vertices()) {
        assert!((after.position - before.position).length() <= tolerance + 1.0e-5);
    }
}

#[test]
fn the_deformed_surface_keeps_finite_normals() {
    let out = deform(&shell(), &wear_spec(8), 0.02);
    for v in out.vertices() {
        assert!(v.normal.is_finite());
    }
}
