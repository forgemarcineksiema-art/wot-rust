use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudeSpec, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder,
    ProfilePoint, RevolveSpec, SmoothingGroup,
};

#[path = "kernel/support.rs"]
mod support;

use support::{
    all_faces_point_outward, assert_smoothed_to_face_average, has_multi_normal_position,
};

#[test]
fn revolve_creates_finite_indexed_geometry_with_unit_normals() {
    let mesh = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![ProfilePoint::new(0.12, 0.0), ProfilePoint::new(0.12, 2.0)],
            axis: Axis::Z,
            segments: 12,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(1),
        })
        .build();

    assert_eq!(mesh.triangle_count(), 24);
    assert_eq!(mesh.indices().len(), mesh.triangle_count() * 3);
    assert!(mesh.vertices().iter().all(|vertex| vertex.position.is_finite()));
    assert!(mesh.vertices().iter().all(|vertex| vertex.normal.is_normalized()));

    let bounds = mesh.bounds().expect("revolved barrel should have bounds");
    assert!(bounds.min.z >= -0.001);
    assert!(bounds.max.z <= 2.001);
    assert!(bounds.min.x <= -0.119 && bounds.max.x >= 0.119);
}

#[test]
fn y_axis_revolve_winds_all_faces_outward() {
    let mesh = MeshBuilder::new()
        .capped_revolve_at(
            Vec3::new(0.0, 0.0, 0.25),
            RevolveSpec {
                profile: vec![ProfilePoint::new(0.75, 1.30), ProfilePoint::new(0.95, 1.52)],
                axis: Axis::Y,
                segments: 16,
                material: MaterialRole::CastArmor,
                smoothing: SmoothingGroup(2),
            },
        )
        .build();

    assert!(all_faces_point_outward(&mesh), "Axis::Y revolve must be outward-wound");
}

#[test]
fn chamfered_prism_keeps_bounds_while_adding_shape_detail() {
    let mesh = MeshBuilder::new()
        .chamfered_prism(
            Vec3::ZERO,
            Vec3::new(1.0, 0.5, 2.0),
            0.12,
            MaterialRole::RolledArmor,
            SmoothingGroup::hard_edges(),
        )
        .build();

    assert!(mesh.vertex_count() > 24, "chamfering should be richer than a plain box");
    assert!(mesh.triangle_count() > 12, "chamfering should be richer than a plain box");
    assert!(mesh.vertices().iter().all(|vertex| vertex.position.is_finite()));
    assert!(mesh.vertices().iter().all(|vertex| vertex.normal.is_normalized()));

    let bounds = mesh.bounds().expect("chamfered prism should have bounds");
    assert!((bounds.min.x + 1.0).abs() < 0.001);
    assert!((bounds.max.x - 1.0).abs() < 0.001);
    assert!((bounds.min.y + 0.5).abs() < 0.001);
    assert!((bounds.max.y - 0.5).abs() < 0.001);
    assert!((bounds.min.z + 2.0).abs() < 0.001);
    assert!((bounds.max.z - 2.0).abs() < 0.001);
}

#[test]
fn extrude_sweeps_a_sloped_section_with_outward_faces() {
    let section =
        vec![Vec2::new(-2.0, 0.2), Vec2::new(2.0, 0.2), Vec2::new(1.2, 1.0), Vec2::new(-2.0, 1.0)];
    let mesh = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: section.clone(),
                axis: Axis::X,
                half_depth: 1.3,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
            },
        )
        .build();

    assert!(mesh.vertices().iter().all(|v| v.position.is_finite()));
    assert!(mesh.vertices().iter().all(|v| v.normal.is_normalized()));

    let bounds = mesh.bounds().expect("extruded prism should have bounds");
    assert!((bounds.min.x + 1.3).abs() < 1.0e-4, "swept to -half_depth on x");
    assert!((bounds.max.x - 1.3).abs() < 1.0e-4, "swept to +half_depth on x");
    assert!((bounds.min.z + 2.0).abs() < 1.0e-4);
    assert!((bounds.max.z - 2.0).abs() < 1.0e-4);
    assert!((bounds.min.y - 0.2).abs() < 1.0e-4);
    assert!((bounds.max.y - 1.0).abs() < 1.0e-4);

    assert!(all_faces_point_outward(&mesh), "ccw section should produce outward faces");

    let reversed: Vec<Vec2> = section.into_iter().rev().collect();
    let mirror = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: reversed,
                axis: Axis::X,
                half_depth: 1.3,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
            },
        )
        .build();
    assert!(all_faces_point_outward(&mirror), "cw section should also produce outward faces");
    assert_eq!(mesh.triangle_count(), mirror.triangle_count());
}

/// A reflex corner would silently fan self-overlapping caps, so the kernel must refuse concave
/// sections at bake time instead of shipping broken geometry.
#[test]
#[should_panic(expected = "concave")]
fn extrude_rejects_concave_sections() {
    let section = vec![
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 0.2), // reflex corner poking into the polygon
        Vec2::new(-1.0, 1.0),
    ];
    MeshBuilder::new().extrude(
        Vec3::ZERO,
        ExtrudeSpec {
            section,
            axis: Axis::X,
            half_depth: 0.5,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
        },
    );
}

/// Collinear runs are not reflex corners; a section with a midpoint on an edge must still sweep.
#[test]
fn extrude_accepts_collinear_section_runs() {
    let section = vec![
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, 0.0), // collinear midpoint
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ];
    let mesh = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: 0.5,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
            },
        )
        .build();
    assert!(mesh.triangle_count() > 0);
    assert!(all_faces_point_outward(&mesh));
}

/// A loft connects varying cross-sections into a tapered solid: the back is wide and tall, the
/// bow narrow and low, like a hull plan. The skin must be finite, unit-normalled, outward-wound,
/// and bounded by the extreme sections.
#[test]
fn loft_skins_varying_sections_into_a_tapered_solid() {
    let wide =
        vec![Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0)];
    let narrow =
        vec![Vec2::new(-0.6, 0.0), Vec2::new(0.6, 0.0), Vec2::new(0.6, 0.7), Vec2::new(-0.6, 0.7)];
    let mesh = MeshBuilder::new()
        .loft(
            Vec3::ZERO,
            LoftSpec {
                sections: vec![LoftSection::new(-2.0, wide), LoftSection::new(2.0, narrow)],
                axis: Axis::Z,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
                cap_ends: true,
            },
        )
        .build();

    assert!(mesh.vertices().iter().all(|v| v.position.is_finite()));
    assert!(mesh.vertices().iter().all(|v| v.normal.is_normalized()));
    assert!(all_faces_point_outward(&mesh), "a convex loft must be outward-wound");

    let bounds = mesh.bounds().expect("loft should have bounds");
    assert!((bounds.min.z + 2.0).abs() < 1.0e-4 && (bounds.max.z - 2.0).abs() < 1.0e-4);
    assert!((bounds.min.x + 1.0).abs() < 1.0e-4 && (bounds.max.x - 1.0).abs() < 1.0e-4);
    assert!((bounds.min.y - 0.0).abs() < 1.0e-4 && (bounds.max.y - 1.0).abs() < 1.0e-4);

    // Side walls (4 edges × 1 gap × 2 tris) + two 4-tri caps.
    assert_eq!(mesh.triangle_count(), 16);
}

/// Rings must connect 1:1, so mismatched point counts are a bake-time error, not silent garbage.
#[test]
#[should_panic(expected = "same point count")]
fn loft_rejects_mismatched_section_point_counts() {
    MeshBuilder::new().loft(
        Vec3::ZERO,
        LoftSpec {
            sections: vec![
                LoftSection::new(
                    0.0,
                    vec![Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
                ),
                LoftSection::new(
                    1.0,
                    vec![
                        Vec2::new(-1.0, 0.0),
                        Vec2::new(1.0, 0.0),
                        Vec2::new(1.0, 1.0),
                        Vec2::new(-1.0, 1.0),
                    ],
                ),
            ],
            axis: Axis::Z,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            cap_ends: true,
        },
    );
}

/// Like extrude, a loft only sweeps convex sections; a reflex corner is refused at bake time.
#[test]
#[should_panic(expected = "concave")]
fn loft_rejects_concave_sections() {
    let concave = vec![
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 0.2),
        Vec2::new(-1.0, 1.0),
    ];
    MeshBuilder::new().loft(
        Vec3::ZERO,
        LoftSpec {
            sections: vec![LoftSection::new(-1.0, concave.clone()), LoftSection::new(1.0, concave)],
            axis: Axis::Z,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            cap_ends: true,
        },
    );
}

/// A plate is a box chamfered on every edge: full extent on each axis, a beveled perimeter, and no
/// sharp box corners left — a cut-steel armour plate rather than a raw cuboid.
#[test]
fn plate_box_bevels_every_edge_into_a_cut_plate() {
    let half = Vec3::new(1.0, 0.1, 1.5);
    let mesh = MeshBuilder::new()
        .plate_box(Vec3::ZERO, half, 0.05, MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
        .build();

    assert!(mesh.vertices().iter().all(|v| v.position.is_finite()));
    assert!(mesh.vertices().iter().all(|v| v.normal.is_normalized()));
    assert!(all_faces_point_outward(&mesh), "a chamfered plate is convex and outward-wound");

    // 6 face quads + 12 edge chamfers + 8 corner triangles.
    assert_eq!(mesh.triangle_count(), 6 * 2 + 12 * 2 + 8);

    let bounds = mesh.bounds().expect("plate bounds");
    assert!((bounds.min - (-half)).length() < 1.0e-4, "plate reaches -half on every axis");
    assert!((bounds.max - half).length() < 1.0e-4, "plate reaches +half on every axis");

    let sharp_corner = mesh.vertices().iter().any(|v| {
        (v.position.x.abs() - half.x).abs() < 1.0e-4
            && (v.position.y.abs() - half.y).abs() < 1.0e-4
            && (v.position.z.abs() - half.z).abs() < 1.0e-4
    });
    assert!(!sharp_corner, "the bevel must replace every sharp box corner");
}

/// An over-large bevel is clamped, not panicked: the plate still closes into a valid convex solid.
#[test]
fn plate_box_clamps_an_oversized_bevel() {
    let mesh = MeshBuilder::new()
        .plate_box(
            Vec3::ZERO,
            Vec3::splat(0.5),
            5.0,
            MaterialRole::RolledArmor,
            SmoothingGroup::hard_edges(),
        )
        .build();

    assert!(mesh.vertices().iter().all(|v| v.position.is_finite() && v.normal.is_normalized()));
    assert!(all_faces_point_outward(&mesh));
}

/// `weld_and_smooth` must fuse coincident vertices that share a *smooth* group and replace their
/// normals with the face average, while leaving hard-edge seams as distinct per-face normals so
/// welded plates keep their crisp edges.
#[test]
fn weld_and_smooth_averages_cast_normals_and_preserves_hard_seams() {
    // A chamfered prism duplicates every corner across the faces meeting there — a compact
    // stand-in for a cast turret cheek.
    let prism = |material, smoothing| {
        MeshBuilder::new()
            .chamfered_prism(Vec3::ZERO, Vec3::new(1.0, 0.6, 1.4), 0.3, material, smoothing)
            .build()
    };

    let raw_cast = prism(MaterialRole::CastArmor, SmoothingGroup(2));
    let cast = raw_cast.clone().weld_and_smooth();

    assert!(
        cast.vertex_count() < raw_cast.vertex_count(),
        "welding a cast prism should remove duplicate corner vertices"
    );
    assert!(cast.vertices().iter().all(|vertex| vertex.normal.is_normalized()));
    assert_smoothed_to_face_average(&raw_cast, &cast);

    let hard = prism(MaterialRole::RolledArmor, SmoothingGroup::hard_edges()).weld_and_smooth();
    assert!(
        has_multi_normal_position(&hard),
        "hard edges must keep distinct per-face normals at shared corners"
    );
}

#[test]
fn weld_and_smooth_recomputes_smooth_normals_from_indexed_faces() {
    let bad_normal = Vec3::Y;
    let mesh = GeometryMesh::new(
        vec![
            vertex(Vec3::new(0.0, 0.0, 0.0), bad_normal),
            vertex(Vec3::new(1.0, 0.0, 0.0), bad_normal),
            vertex(Vec3::new(0.0, 1.0, 0.0), bad_normal),
        ],
        vec![0, 1, 2],
    )
    .weld_and_smooth();

    let expected = Vec3::Z;
    assert!(mesh.vertices().iter().all(|v| (v.normal - expected).length() < 1.0e-5));
}

fn vertex(position: Vec3, normal: Vec3) -> vehicle_geometry::GeometryVertex {
    vehicle_geometry::GeometryVertex::new(
        position,
        normal,
        MaterialRole::CastArmor,
        SmoothingGroup(2),
    )
}
