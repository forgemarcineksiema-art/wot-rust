//! Part-aware reduction: importance scales the clustering grid (silhouette/mount-critical held
//! finer, detail coarser), and a post-reduction audit proves a tier never expands the silhouette,
//! drops a required group to nothing, or moves a mount frame.

use game_core::{MountFrames, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{
    BakedVehicle, GeometryMesh, GeometryVertex, LodAuditError, MaterialRole, PartImportance,
    SmoothingGroup, Submesh, SubmeshKind, audit_reduction, reduce_mesh,
};

/// A lat-long sphere of `radius` at `center` — a dense smooth shell with plenty of triangles to
/// cluster. Pole degeneracy is irrelevant here (we measure triangle counts and bounds, not manifold).
fn sphere(center: Vec3, radius: f32) -> GeometryMesh {
    let (stacks, slices) = (16_usize, 24_usize);
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for i in 0..=stacks {
        let phi = i as f32 / stacks as f32 * std::f32::consts::PI;
        for j in 0..=slices {
            let theta = j as f32 / slices as f32 * std::f32::consts::TAU;
            let n = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
            v.push(GeometryVertex::new(
                center + n * radius,
                n,
                MaterialRole::CastArmor,
                SmoothingGroup(4),
            ));
        }
    }
    let w = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = (i * w + j) as u32;
            idx.extend_from_slice(&[a, a + w as u32, a + 1, a + 1, a + w as u32, a + w as u32 + 1]);
        }
    }
    GeometryMesh::new(v, idx).weld_and_smooth()
}

fn baked(hull: GeometryMesh, turret: GeometryMesh, gun: GeometryMesh) -> BakedVehicle {
    BakedVehicle::new(
        VehicleKind::PrototypeMedium,
        vec![
            Submesh { kind: SubmeshKind::Hull, mesh: hull },
            Submesh { kind: SubmeshKind::Turret, mesh: turret },
            Submesh { kind: SubmeshKind::Gun, mesh: gun },
        ],
        MountFrames::for_vehicle(VehicleKind::PrototypeMedium),
    )
}

fn reduced_like(base: &BakedVehicle, cell: impl Fn(SubmeshKind) -> f32) -> BakedVehicle {
    let submeshes = base
        .submeshes()
        .iter()
        .map(|s| Submesh { kind: s.kind, mesh: reduce_mesh(&s.mesh, cell(s.kind)) })
        .collect();
    BakedVehicle::new(base.kind(), submeshes, *base.mounts())
}

#[test]
fn silhouette_and_mount_critical_cluster_finer_than_detail() {
    assert!(PartImportance::Silhouette.cell_scale() < 1.0, "silhouette held to a finer grid");
    assert!(
        PartImportance::MountCritical.cell_scale() < 1.0,
        "mount-critical held to a finer grid"
    );
    assert!(PartImportance::Detail.cell_scale() > 1.0, "detail clustered more aggressively");
    assert_eq!(
        PartImportance::Silhouette.cell_scale(),
        PartImportance::MountCritical.cell_scale(),
        "silhouette and mount-critical share the fine scale",
    );
}

#[test]
fn a_finer_cell_keeps_at_least_as_many_triangles() {
    let mesh = sphere(Vec3::ZERO, 1.0);
    let fine = reduce_mesh(&mesh, 0.10 * PartImportance::Silhouette.cell_scale());
    let coarse = reduce_mesh(&mesh, 0.10 * PartImportance::Detail.cell_scale());
    assert!(
        fine.triangle_count() >= coarse.triangle_count(),
        "finer grid {} should retain at least as many triangles as coarser {}",
        fine.triangle_count(),
        coarse.triangle_count(),
    );
    assert!(
        coarse.triangle_count() < mesh.triangle_count(),
        "reduction actually removed triangles"
    );
}

#[test]
fn a_well_formed_reduction_passes_the_audit() {
    let base = baked(
        sphere(Vec3::ZERO, 1.2),
        sphere(Vec3::new(0.0, 1.4, 0.0), 0.7),
        sphere(Vec3::new(0.0, 1.4, 2.0), 0.2),
    );
    let reduced = reduced_like(&base, |_| 0.08);
    audit_reduction(&base, &reduced)
        .expect("a centroid reduction stays inside LOD0 and keeps groups");
}

#[test]
fn the_audit_catches_a_collapsed_required_group() {
    let base = baked(
        sphere(Vec3::ZERO, 1.2),
        sphere(Vec3::new(0.0, 1.4, 0.0), 0.7),
        sphere(Vec3::new(0.0, 1.4, 2.0), 0.2),
    );
    // A cell far larger than the gun collapses it to nothing.
    let reduced = reduced_like(&base, |kind| if kind == SubmeshKind::Gun { 100.0 } else { 0.08 });
    assert_eq!(
        audit_reduction(&base, &reduced),
        Err(LodAuditError::RequiredPartCollapsed { kind: SubmeshKind::Gun }),
    );
}

#[test]
fn the_audit_catches_moved_mount_frames() {
    let base = baked(
        sphere(Vec3::ZERO, 1.2),
        sphere(Vec3::new(0.0, 1.4, 0.0), 0.7),
        sphere(Vec3::new(0.0, 1.4, 2.0), 0.2),
    );
    let mut reduced = reduced_like(&base, |_| 0.08);
    let mut mounts = *reduced.mounts();
    mounts.turret_ring.translation += Vec3::new(0.0, 0.5, 0.0);
    reduced = BakedVehicle::new(reduced.kind(), reduced.submeshes().to_vec(), mounts);
    assert_eq!(audit_reduction(&base, &reduced), Err(LodAuditError::MountFramesMoved));
}

#[test]
fn the_audit_catches_bounds_expansion() {
    let base = baked(
        sphere(Vec3::ZERO, 1.0),
        sphere(Vec3::new(0.0, 1.4, 0.0), 0.7),
        sphere(Vec3::new(0.0, 1.4, 2.0), 0.2),
    );
    // Swap in a hull that pokes outside the LOD0 hull bounds.
    let bigger = Submesh { kind: SubmeshKind::Hull, mesh: sphere(Vec3::ZERO, 1.5) };
    let mut subs = base.submeshes().to_vec();
    subs[0] = bigger;
    let reduced = BakedVehicle::new(base.kind(), subs, *base.mounts());
    assert!(matches!(
        audit_reduction(&base, &reduced),
        Err(LodAuditError::BoundsExpanded { kind: SubmeshKind::Hull, .. }),
    ));
}
