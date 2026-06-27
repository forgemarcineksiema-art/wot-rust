//! The shell contract: an open manifold sheet thickens into a watertight thin solid (or twin
//! sheets), and unsuitable input is a typed error.

use glam::Vec3;
use shell::{ShellDirection, ShellError, ShellRimPolicy, ShellSpec, try_shell};
use vehicle_geometry::{
    CLOSED_SMOOTH_MESH, GeometryMesh, GeometryVertex, MaterialRole, OPEN_OR_CLOSED_MESH,
    SmoothingGroup,
};

/// A flat 3x3 grid sheet in the XZ plane (normal +Y), with shared indices — open and manifold.
fn grid_sheet() -> GeometryMesh {
    let mut verts = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            verts.push(GeometryVertex::new(
                Vec3::new(i as f32, 0.0, j as f32),
                Vec3::Y,
                MaterialRole::RolledArmor,
                SmoothingGroup(1),
            ));
        }
    }
    let mut idx = Vec::new();
    for j in 0..2u32 {
        for i in 0..2u32 {
            let a = j * 3 + i;
            idx.extend_from_slice(&[a, a + 3, a + 4, a, a + 4, a + 1]);
        }
    }
    GeometryMesh::new(verts, idx)
}

fn tetrahedron() -> GeometryMesh {
    let v = |p: Vec3| {
        GeometryVertex::new(p, p.normalize_or_zero(), MaterialRole::RolledArmor, SmoothingGroup(1))
    };
    let verts = vec![v(Vec3::ZERO), v(Vec3::X), v(Vec3::Y), v(Vec3::Z)];
    GeometryMesh::new(verts, vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3])
}

#[test]
fn bridging_an_open_sheet_makes_a_closed_thin_solid() {
    let sheet = grid_sheet();
    let solid = try_shell(&ShellSpec {
        surface: &sheet,
        thickness: 0.1,
        direction: ShellDirection::NegativeNormal,
        rim_policy: ShellRimPolicy::Bridge,
    })
    .expect("shell a sheet");
    solid.validate_quality(CLOSED_SMOOTH_MESH).expect("bridged shell is a closed manifold");
    let b = solid.bounds().expect("bounds");
    // The back face sits 0.1 below the original +Y surface.
    assert!((b.max.y - 0.0).abs() < 1.0e-4 && (b.min.y + 0.1).abs() < 1.0e-4);
}

#[test]
fn an_open_rim_policy_leaves_twin_sheets() {
    let sheet = grid_sheet();
    let twin = try_shell(&ShellSpec {
        surface: &sheet,
        thickness: 0.1,
        direction: ShellDirection::NegativeNormal,
        rim_policy: ShellRimPolicy::Open,
    })
    .expect("twin sheets");
    let report = twin.quality_report(OPEN_OR_CLOSED_MESH);
    assert!(report.boundary_edges > 0, "an open shell keeps its rim open");
}

#[test]
fn the_outer_surface_is_preserved_exactly() {
    let sheet = grid_sheet();
    let solid = try_shell(&ShellSpec {
        surface: &sheet,
        thickness: 0.1,
        direction: ShellDirection::NegativeNormal,
        rim_policy: ShellRimPolicy::Bridge,
    })
    .expect("shell");
    // Every original front vertex position still appears in the shelled solid.
    for v in sheet.vertices() {
        assert!(
            solid.vertices().iter().any(|s| (s.position - v.position).length() < 1.0e-5),
            "outer armour surface vertex {:?} was moved",
            v.position
        );
    }
}

#[test]
fn a_non_positive_thickness_is_rejected() {
    let sheet = grid_sheet();
    let r = try_shell(&ShellSpec {
        surface: &sheet,
        thickness: 0.0,
        direction: ShellDirection::AlongVertexNormals,
        rim_policy: ShellRimPolicy::Bridge,
    });
    assert_eq!(r, Err(ShellError::NonPositiveThickness));
}

#[test]
fn a_closed_surface_is_rejected() {
    let closed = tetrahedron();
    let r = try_shell(&ShellSpec {
        surface: &closed,
        thickness: 0.1,
        direction: ShellDirection::AlongVertexNormals,
        rim_policy: ShellRimPolicy::Bridge,
    });
    assert_eq!(r, Err(ShellError::ClosedInput));
}

#[test]
fn a_non_manifold_surface_is_rejected() {
    // Three triangles sharing the edge (0,1): non-manifold.
    let v = |p: Vec3| GeometryVertex::new(p, Vec3::Y, MaterialRole::RolledArmor, SmoothingGroup(1));
    let verts = vec![
        v(Vec3::new(0.0, 0.0, 0.0)),
        v(Vec3::new(1.0, 0.0, 0.0)),
        v(Vec3::new(0.0, 0.0, 1.0)),
        v(Vec3::new(0.0, 0.0, -1.0)),
        v(Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mesh = GeometryMesh::new(verts, vec![0, 1, 2, 0, 1, 3, 0, 1, 4]);
    let r = try_shell(&ShellSpec {
        surface: &mesh,
        thickness: 0.1,
        direction: ShellDirection::AlongVertexNormals,
        rim_policy: ShellRimPolicy::Bridge,
    });
    assert_eq!(r, Err(ShellError::NonManifoldInput));
}
