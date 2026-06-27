//! The bake-time contact-cavity pass: a deterministic, order-independent darkening of
//! `surface_shade` from semantic [`CavityBand`]s. These tests lock the primitive the vehicle layer
//! drives — band cores darken, the open surface stays clean, and the result clamps and reproduces.

use glam::{Vec2, Vec3};
use vehicle_geometry::{CavityBand, GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

fn vert(position: Vec3) -> GeometryVertex {
    GeometryVertex::new(position, Vec3::Y, MaterialRole::CastArmor, SmoothingGroup(2))
}

fn mesh(points: &[Vec3]) -> GeometryMesh {
    let vertices: Vec<_> = points.iter().map(|p| vert(*p)).collect();
    // Topology is irrelevant to the shade pass; a single fan of indices keeps the mesh well-formed.
    let indices = (0..points.len() as u32).collect();
    GeometryMesh::new(vertices, indices)
}

fn shade_at(mesh: &GeometryMesh, i: usize) -> f32 {
    mesh.vertices()[i].surface_shade
}

#[test]
fn a_band_darkens_its_core_and_leaves_the_open_surface_clean() {
    let inside = Vec3::new(0.0, 1.30, 0.0);
    let outside = Vec3::new(0.0, 2.50, 0.0);
    let band = CavityBand {
        center: Vec3::new(0.0, 1.30, 0.0),
        half_extents: Vec3::new(1.0, 0.05, 1.0),
        falloff: 0.14,
        amount: 0.40,
    };

    let baked = mesh(&[inside, outside]).with_contact_cavity(&[band]);

    assert!((shade_at(&baked, 0) - 0.60).abs() < 1.0e-5, "core darkens to 1 - amount");
    assert_eq!(shade_at(&baked, 1), 1.0, "the open surface is untouched");
}

#[test]
fn the_falloff_fades_smoothly_to_the_open_surface() {
    let band =
        CavityBand { center: Vec3::ZERO, half_extents: Vec3::ZERO, falloff: 1.0, amount: 0.50 };
    // Half a metre out of a 1 m falloff is half weight: shade *= 1 - 0.5 * 0.5.
    let baked = mesh(&[Vec3::new(0.5, 0.0, 0.0)]).with_contact_cavity(&[band]);
    assert!((shade_at(&baked, 0) - 0.75).abs() < 1.0e-5);
}

#[test]
fn overlapping_bands_multiply_and_stay_in_range() {
    let band = CavityBand {
        center: Vec3::ZERO,
        half_extents: Vec3::splat(0.1),
        falloff: 0.1,
        amount: 0.9,
    };
    // Three heavy overlapping bands must never drive shade below zero or above one.
    let baked = mesh(&[Vec3::ZERO]).with_contact_cavity(&[band, band, band]);
    let s = shade_at(&baked, 0);
    assert!((0.0..=1.0).contains(&s), "shade stays in 0..=1, got {s}");
    assert!(s < 0.01, "three 0.9 bands stack to near-black, got {s}");
}

#[test]
fn the_pass_is_deterministic() {
    let band = CavityBand {
        center: Vec3::new(0.1, 0.2, 0.3),
        half_extents: Vec3::new(0.4, 0.1, 0.5),
        falloff: 0.12,
        amount: 0.35,
    };
    let points: Vec<Vec3> = (0..64).map(|i| Vec3::splat(i as f32 * 0.05)).collect();
    let a = mesh(&points).with_contact_cavity(&[band]);
    let b = mesh(&points).with_contact_cavity(&[band]);
    let shades =
        |m: &GeometryMesh| m.vertices().iter().map(|v| v.surface_shade).collect::<Vec<_>>();
    assert_eq!(shades(&a), shades(&b), "the cavity bake must reproduce byte-for-byte");
}

#[test]
fn parametric_uv_vertices_keep_their_chart_through_the_pass() {
    // The pass touches only shade — UV0 and mapping mode are preserved.
    let v = vert(Vec3::ZERO).with_uv0(Vec2::new(0.3, 0.7));
    let baked = GeometryMesh::new(vec![v], vec![0]).with_contact_cavity(&[CavityBand {
        center: Vec3::ZERO,
        half_extents: Vec3::splat(0.5),
        falloff: 0.1,
        amount: 0.5,
    }]);
    assert_eq!(baked.vertices()[0].uv0, Vec2::new(0.3, 0.7));
}
