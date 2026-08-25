use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudePolygonSpec, ExtrudeSpec, GeometryMesh, LoftSection, LoftSpec, MaterialRole,
    MeshBuilder, OPEN_OR_CLOSED_MESH, ProfilePoint, RevolveSpec, SmoothingGroup,
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
    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("revolved barrel is a clean mesh");

    let bounds = mesh.bounds().expect("revolved barrel should have bounds");
    assert!(bounds.min.z >= -0.001);
    assert!(bounds.max.z <= 2.001);
    assert!(bounds.min.x <= -0.119 && bounds.max.x >= 0.119);
}

#[test]
fn sloped_revolve_profile_normals_follow_the_surface_not_just_the_radius() {
    // A road-wheel dish is a shallow cone around X. Its outward normal must carry a strong
    // +X component; a purely radial normal makes the face light as alternating bright/dark
    // sectors even though it is aimed toward the viewer.
    let mesh = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![ProfilePoint::new(0.20, 0.50), ProfilePoint::new(1.00, 0.10)],
            axis: Axis::X,
            segments: 24,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(5),
        })
        .build();

    assert!(
        mesh.vertices().iter().all(|vertex| vertex.normal.x > 0.35),
        "a forward-facing dish needs profile-aware normals with a positive axial component"
    );
    for tri in mesh.indices().chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let face = (mesh.vertices()[b].position - mesh.vertices()[a].position)
            .cross(mesh.vertices()[c].position - mesh.vertices()[a].position)
            .normalize_or_zero();
        for index in [a, b, c] {
            assert!(
                mesh.vertices()[index].normal.dot(face) > 0.95,
                "revolve vertex normal must agree with its geometric face"
            );
        }
    }
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

/// A profile is allowed to run BACKWARDS along its axis, and the caps must follow it. The gun's
/// bore funnel does exactly this — it recedes from the muzzle face into the tube — and a cap
/// normal taken from the axis alone then faces into the solid at both ends. Both fans wind
/// inward; `weld_and_smooth` then fuses their rims with the wall (a smooth group leaves the
/// normal out of the weld key) and every rim edge is traversed twice the same way. That shipped
/// on seven of eight guns, so the descending profile gets a lock of its own here, at the kernel,
/// rather than only at the fleet gate that caught it.
#[test]
fn a_revolve_profile_that_runs_backwards_still_caps_outward() {
    let descending = MeshBuilder::new()
        .capped_revolve_at(
            Vec3::ZERO,
            RevolveSpec {
                profile: vec![ProfilePoint::new(0.40, 1.00), ProfilePoint::new(0.18, 0.20)],
                axis: Axis::Z,
                segments: 16,
                material: MaterialRole::BarrelSteel,
                smoothing: SmoothingGroup(3),
            },
        )
        .build()
        .weld_and_smooth();

    let report = descending.quality_report(OPEN_OR_CLOSED_MESH);
    assert_eq!(
        report.inconsistent_winding_edges, 0,
        "a descending profile must cap the same way an ascending one does"
    );
    assert!(all_faces_point_outward(&descending), "descending revolve must be outward-wound");
}

/// A CLOSED profile — one that goes out, along, back in and returns — is how every ring in the
/// project is made: the steel rim under a road wheel tyre, the tyre itself, the return roller,
/// the turret-ring collar, the drums the hatches are cut from.
///
/// Such a profile has a band that faces the AXIS rather than away from it: the ring's inner wall.
/// The kernel used to orient each band on its own by asking whether it pointed away from the
/// axis, so that one band came out flipped relative to its neighbours and its seams were wound
/// two different ways at once. It cost 22 broken edges per ring and it shipped on every wheel in
/// the fleet, because the mesh-quality gate only ever ran over the BAKED submeshes and the gear
/// lives outside the bake.
///
/// The lathe is now wound once and oriented once, so a ring is a ring from both sides.
#[test]
fn a_closed_ring_profile_is_wound_one_way_all_the_way_round() {
    let ring = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.26, -0.16),
                ProfilePoint::new(0.36, -0.16),
                ProfilePoint::new(0.36, 0.16),
                ProfilePoint::new(0.26, 0.16),
                ProfilePoint::new(0.26, -0.16),
            ],
            axis: Axis::X,
            segments: 24,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(5),
        })
        .build();

    let report = ring.quality_report(OPEN_OR_CLOSED_MESH);
    assert_eq!(
        report.inconsistent_winding_edges, 0,
        "the ring's inner wall must agree with its outer wall — {} edges are wound both ways",
        report.inconsistent_winding_edges
    );
    assert_eq!(report.non_manifold_edges, 0, "a closed ring is a manifold shell");
    assert!(
        report.signed_volume > 0.0,
        "the shell must enclose its metal, not turn it inside out (signed volume {})",
        report.signed_volume
    );
}

/// The orientation vote is decided by the surface as a whole, so the minority band follows the
/// majority instead of flipping alone — and a plain outer wall is still wound outward.
#[test]
fn a_lathe_orients_by_its_dominant_face_not_band_by_band() {
    // A deep ring: the inner wall is nearly as large as the outer one, so a per-band guess had
    // the most to disagree about here, and the vote still has to come out facing the world.
    let deep_ring = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.40, -0.05),
                ProfilePoint::new(0.44, -0.05),
                ProfilePoint::new(0.44, 0.05),
                ProfilePoint::new(0.40, 0.05),
                ProfilePoint::new(0.40, -0.05),
            ],
            axis: Axis::Z,
            segments: 20,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(5),
        })
        .build();
    assert_eq!(deep_ring.quality_report(OPEN_OR_CLOSED_MESH).inconsistent_winding_edges, 0);
    assert!(
        deep_ring.quality_report(OPEN_OR_CLOSED_MESH).signed_volume > 0.0,
        "even when the inner wall nearly matches the outer one, the shell faces out"
    );

    // And the simple case the old heuristic did get right must stay right.
    let tube = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![ProfilePoint::new(0.20, -0.5), ProfilePoint::new(0.20, 0.5)],
            axis: Axis::X,
            segments: 18,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(3),
        })
        .build();
    assert!(all_faces_point_outward(&tube), "a plain tube wall still faces away from its axis");
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
    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("chamfered prism is a clean mesh");

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

    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("extruded prism is a clean mesh");

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

#[test]
fn extrude_polygon_triangulates_concave_section_with_hole() {
    let mesh = MeshBuilder::new()
        .extrude_polygon(
            Vec3::ZERO,
            ExtrudePolygonSpec {
                points: vec![
                    Vec2::new(-1.0, -1.0),
                    Vec2::new(1.0, -1.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.25, 0.25),
                    Vec2::new(-1.0, 1.0),
                    Vec2::new(-0.25, -0.25),
                    Vec2::new(0.25, -0.25),
                    Vec2::new(0.25, 0.10),
                    Vec2::new(-0.25, 0.10),
                ],
                hole_indices: vec![5],
                axis: Axis::Y,
                half_depth: 0.1,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
            },
        )
        .build();

    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("earcut extrusion is a clean mesh");
    assert!(mesh.triangle_count() >= 16);
    assert!(mesh.vertices().iter().all(|v| v.position.is_finite()));
    assert!(
        mesh.vertices().iter().any(|v| v.position.x > 0.24 && v.normal.x < -0.8),
        "right hole wall should face into the cutout"
    );
    assert!(
        mesh.vertices().iter().any(|v| v.position.x < -0.24 && v.normal.x > 0.8),
        "left hole wall should face into the cutout"
    );
}

#[test]
fn a_reflex_corner_wall_faces_outward_not_toward_the_centroid() {
    // An L-shaped section is concave. The lower arm's top edge (2,1)->(1,1) has its true outward
    // normal along +section.y (world +Z under Axis::Y), but `edge_mid - centroid` points +section.x
    // (world +X): the reflex corner drags the mean-of-points the wrong way, and because that hint is
    // perpendicular to the wall's real normal the old code left the wall facing INWARD (world -Z).
    let mesh = MeshBuilder::new()
        .extrude_polygon(
            Vec3::ZERO,
            ExtrudePolygonSpec {
                points: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(2.0, 0.0),
                    Vec2::new(2.0, 1.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(1.0, 2.0),
                    Vec2::new(0.0, 2.0),
                ],
                hole_indices: vec![],
                axis: Axis::Y,
                half_depth: 0.5,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
            },
        )
        .build();

    // The lower arm's top wall spans section (2,1)->(1,1) => world x in [1,2] at z=1. Its outward
    // normal must be world +Z (the section's +y), never world -Z (the inward flip the hint caused).
    let faces_out = mesh
        .vertices()
        .iter()
        .any(|v| (v.position - Vec3::new(2.0, 0.5, 1.0)).length() < 1.0e-3 && v.normal.z > 0.8);
    assert!(
        faces_out,
        "the reflex arm's top wall must face outward (+Z), not inward toward the centroid"
    );
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

    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("a capped convex loft is a clean mesh");
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

    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("a chamfered plate is a clean mesh");
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

    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("a clamped plate is still a clean mesh");
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
fn weld_and_smooth_drops_a_triangle_its_merge_collapsed_but_keeps_healthy_ones() {
    // A healthy triangle plus a sub-grid sliver whose short edge (0.1 mm, under the ~0.244 mm weld
    // grid) fuses its two corners into one bucket. The sliver must be DROPPED — a collapsed triangle
    // is a zero-area ghost that renders as a black sliver with a broken tangent — while the healthy
    // triangle survives with all three corners intact.
    let mesh = GeometryMesh::new(
        vec![
            vertex(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
            vertex(Vec3::new(1.0, 0.0, 0.0), Vec3::Z),
            vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Z),
            vertex(Vec3::new(0.0, 0.0, 2.0), Vec3::Z),
            vertex(Vec3::new(0.0001, 0.0, 2.0), Vec3::Z), // 0.1 mm from the previous corner
            vertex(Vec3::new(0.0, 1.0, 2.0), Vec3::Z),
        ],
        vec![0, 1, 2, 3, 4, 5],
    )
    .weld_and_smooth();

    assert_eq!(mesh.triangle_count(), 1, "the collapsed sliver is dropped, the healthy face stays");
    for tri in mesh.indices().chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]];
        assert!(a != b && b != c && a != c, "no surviving triangle may have a repeated index");
    }
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

/// A MACHINED ARRIS IS A LINE, NOT A BLEND. The lathe shares one vertex per (segment, profile
/// point) and averages its normal over every band that touches it — so a 90-degree corner on a
/// wheel rim, a flange or a drum shades like a fillet, on every lathe in the fleet. (And because
/// a CLOSED profile duplicates its first point, exactly one corner of a rectangular ring is
/// accidentally crisp: three soft corners and one hard one on the same part.)
///
/// The contract: profile corners sharper than the hard-break threshold carry one normal PER
/// BAND — same position, different normals — while gentle turns keep sharing. Zero new
/// triangles; the split is vertex-only.
#[test]
fn a_lathe_profile_corner_sharper_than_the_threshold_is_crisp() {
    // A closed rectangular ring: four 90-degree corners.
    let ring = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.20, -0.05),
                ProfilePoint::new(0.30, -0.05),
                ProfilePoint::new(0.30, 0.05),
                ProfilePoint::new(0.20, 0.05),
                ProfilePoint::new(0.20, -0.05),
            ],
            axis: Axis::X,
            segments: 12,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(5),
        })
        .build();

    // Every corner position must present at least two distinct normals.
    let mut positions: std::collections::BTreeMap<(i32, i32, i32), Vec<glam::Vec3>> =
        std::collections::BTreeMap::new();
    for v in ring.vertices() {
        let key = (
            (v.position.x * 4096.0).round() as i32,
            (v.position.y * 4096.0).round() as i32,
            (v.position.z * 4096.0).round() as i32,
        );
        positions.entry(key).or_default().push(v.normal);
    }
    let mut crisp = 0usize;
    let mut soft_corners = 0usize;
    for normals in positions.values() {
        if normals.len() < 2 {
            continue;
        }
        let spread = normals
            .iter()
            .flat_map(|a| normals.iter().map(move |b| a.dot(*b)))
            .fold(f32::INFINITY, f32::min);
        if spread < 0.7 {
            crisp += 1;
        } else {
            soft_corners += 1;
        }
    }
    // 4 corners x 12 segments = 48 crisp positions. Before the split the welded average left
    // only the accidental seam corner crisp (12 positions).
    assert!(
        crisp >= 48,
        "a rectangular ring has four crisp corners per segment: {crisp} crisp positions found \
         ({soft_corners} corner positions still blended)"
    );

    // And a GENTLE profile stays smooth: a shallow dome must not sprout seams.
    let dome = MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.30, 0.00),
                ProfilePoint::new(0.29, 0.03),
                ProfilePoint::new(0.26, 0.06),
                ProfilePoint::new(0.21, 0.085),
                ProfilePoint::new(0.15, 0.10),
            ],
            axis: Axis::X,
            segments: 12,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(5),
        })
        .build();
    let mut dome_positions: std::collections::BTreeMap<(i32, i32, i32), Vec<glam::Vec3>> =
        std::collections::BTreeMap::new();
    for v in dome.vertices() {
        let key = (
            (v.position.x * 4096.0).round() as i32,
            (v.position.y * 4096.0).round() as i32,
            (v.position.z * 4096.0).round() as i32,
        );
        dome_positions.entry(key).or_default().push(v.normal);
    }
    for normals in dome_positions.values() {
        let spread = normals
            .iter()
            .flat_map(|a| normals.iter().map(move |b| a.dot(*b)))
            .fold(f32::INFINITY, f32::min);
        assert!(
            spread > 0.9,
            "a gentle dome profile must stay smooth — a split there is a seam nobody machined"
        );
    }
}
