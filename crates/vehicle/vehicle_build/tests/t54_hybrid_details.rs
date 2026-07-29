//! Locking tests for the hybrid T-54's exterior detailing and mantlet bedding (`vehicle_build::t54`).
//! Split from `t54_hybrid.rs` so each integration file stays within the reviewability budget; every
//! test drives only the crate's public API. This file owns the greeble contract — hatches,
//! periscopes, fenders, transmission covers, the static/runtime running-gear split, and the moving
//! mantlet's bedding into the lofted turret embrasure.

use game_core::{MountFrames, VehicleKind};
use glam::Vec3;
use vehicle_build::t54_description;
use vehicle_geometry::{MaterialRole, SubmeshKind};

#[test]
fn t54_carries_driver_and_loader_hatches() {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.hybrid().unwrap();
    let f = &v.fittings;
    let baked = t54_description().build();

    // The highest point of any lid-local vertex (within the lid radius in the roof plane) — the
    // raised hatch cover, distinct from the flat surface it sits on.
    let lid_apex = |mesh: &vehicle_geometry::GeometryMesh, center: Vec3, radius: f32| {
        mesh.vertices()
            .iter()
            .filter(|vert| {
                let (dx, dz) = (vert.position.x - center.x, vert.position.z - center.z);
                (dx * dx + dz * dz).sqrt() <= radius + 0.01
            })
            .map(|vert| vert.position.y)
            .fold(f32::NEG_INFINITY, f32::max)
    };

    // Driver's hatch: a raised round lid on the hull roof, forward and to the left of centre.
    assert!(f.driver_hatch_center.z > 0.5, "driver hatch sits forward on the hull");
    assert!(f.driver_hatch_center.x < 0.0, "driver hatch sits on the left");
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let driver_apex = lid_apex(hull, f.driver_hatch_center, f.driver_hatch_radius);
    assert!(
        driver_apex >= f.driver_hatch_center.y + f.driver_hatch_half_height - 0.01,
        "driver hatch lid rises above the hull roof, apex {driver_apex:.2}"
    );

    // Loader's hatch: a raised round lid on the turret roof, loader (right) side — it rides the
    // turret submesh so it traverses with the vehicle.
    assert!(f.loader_hatch_center.x > 0.0, "loader hatch sits on the loader (right) side");
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret").mesh;
    let loader_apex = lid_apex(turret, f.loader_hatch_center, f.loader_hatch_radius);
    assert!(
        loader_apex >= f.loader_hatch_center.y + f.loader_hatch_half_height - 0.01,
        "loader hatch lid rises above the turret roof, apex {loader_apex:.2}"
    );

    // Visual-only: both lids sit inside the gameplay collision hitbox (no overhang, no growth).
    let hb = bp.hitbox();
    for c in [f.driver_hatch_center, f.loader_hatch_center] {
        assert!(c.x.abs() <= hb.half_width_m && c.z.abs() <= hb.half_length_m);
        assert!((c.y - hb.center_y_m).abs() <= hb.half_height_m);
    }
}

#[test]
fn t54_periscopes_are_raked_prism_heads_on_turret_and_hull() {
    // The defect this locks: periscopes modelled as plain boxes have only axis-aligned faces. Each
    // must carry a forward-and-up raked prism face (~45 deg, so n.z > 0.6 distinguishes it from the
    // 60 deg glacis at n.z = 0.5). Filter to RolledArmor so the cast dome's smooth normals are out.
    let baked = t54_description().build();
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret").mesh;
    let turret_raked = turret
        .vertices()
        .iter()
        .any(|v| v.material == MaterialRole::RolledArmor && v.normal.y > 0.5 && v.normal.z > 0.6);
    assert!(turret_raked, "turret periscopes must read as raked prism heads");

    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let driver_raked = hull.vertices().iter().any(|v| {
        v.material == MaterialRole::RolledArmor
            && v.normal.x.abs() < 0.15
            && v.normal.y > 0.5
            && v.normal.z > 0.6
            && v.position.z > 1.2
            && v.position.x < 0.0
    });
    assert!(
        driver_raked,
        "driver periscopes must read as raked prism heads on the hull roof, left"
    );
}

#[test]
fn t54_fenders_are_segmented_with_support_brackets() {
    use std::collections::HashSet;
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;

    // Segmented fender: the outer fender top now carries many distinct z-edges (one per section
    // boundary); a single continuous slab would expose only the two end edges.
    // The fender now rides over the track (top ≈1.03), not up at the roof, so the outer top sits
    // above 1.0 while staying well below the 1.20 roof.
    let mut z_bands: HashSet<i32> = HashSet::new();
    for v in hull.vertices() {
        if v.material == MaterialRole::RolledArmor && v.position.x > 1.6 && v.position.y > 1.0 {
            z_bands.insert((v.position.z / 0.05).round() as i32);
        }
    }
    assert!(
        z_bands.len() >= 6,
        "fender splits into bolted sections, got {} z-bands",
        z_bands.len()
    );

    // Support gussets hang just below the fender shelf (above the moving top track run), on both
    // fenders over the exposed track band.
    let has_bracket = |sign: f32| {
        hull.vertices().iter().any(|v| {
            v.material == MaterialRole::TrackMetal
                && v.position.x * sign > 1.15
                && v.position.y > 0.99
                && v.position.y < 1.11
        })
    };
    assert!(has_bracket(1.0) && has_bracket(-1.0), "fender support brackets hang below both sides");
}

#[test]
fn t54_static_bake_leaves_moving_running_gear_to_the_runtime() {
    let desc = t54_description();
    let part_names: std::collections::BTreeSet<_> =
        desc.parts.iter().map(|part| part.key.name).collect();

    assert!(
        !part_names.contains("tracks"),
        "full track belts are runtime running gear; baking them into the hull duplicates the moving links"
    );
    for moving in ["running_gear", "track_ends", "track_links"] {
        assert!(
            !part_names.contains(moving),
            "{moving} is animated by the runtime running-gear instancer and must not be baked into the static hull"
        );
    }
}

#[test]
fn t54_hull_carries_rear_transmission_covers() {
    let v =
        *game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap();
    let deck_top = v.deck.center.y + v.deck.half.y;
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    // Raised RolledArmor plates standing proud of the rear deck, one on each side of the centreline.
    let has_cover = |sign: f32| {
        hull.vertices().iter().any(|vert| {
            vert.material == MaterialRole::RolledArmor
                && vert.position.x * sign > 0.9
                && vert.position.y > deck_top + 0.02
                && vert.position.z < -0.8
        })
    };
    assert!(
        has_cover(1.0) && has_cover(-1.0),
        "transmission covers stand proud of both deck sides"
    );
}

#[test]
fn the_mantlet_beds_into_the_lofted_turret_embrasure() {
    // The moving mantlet beds into the lofted turret's front embrasure recess: its rear sits at or
    // behind the recessed front surface (not floating wholly proud) while its face protrudes to
    // cover the opening. With the lofted shell the embrasure is an open dish rather than a deep cast
    // socket, so this bedding + coverage is what closes the gun-to-turret seam.
    let v =
        *game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let mantlet_mesh = revolve::moving_mantlet(trunnion, &v.gun);
    let mantlet = mantlet_mesh.bounds().expect("mantlet");
    let turret = vehicle_build::t54_turret_loft(&v.turret_loft);

    // Front-most turret surface along the gun centreline at gun height — the lip the mantlet covers.
    let front_z = turret
        .vertices()
        .iter()
        .filter(|vx| vx.position.x.abs() < 0.15 && (vx.position.y - trunnion.y).abs() < 0.18)
        .map(|vx| vx.position.z)
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        mantlet.min.z <= front_z + 0.03,
        "the mantlet rear must bed into the embrasure, not float in front (rear {:.2}, front {front_z:.2})",
        mantlet.min.z
    );
    let rear_shoulder_z = mantlet_mesh
        .vertices()
        .iter()
        .filter(|vx| vx.position.x.abs() >= 0.18 || (vx.position.y - trunnion.y).abs() >= 0.12)
        .map(|vx| vx.position.z)
        .fold(f32::INFINITY, f32::min);
    assert!(
        rear_shoulder_z <= front_z - 0.04,
        "the mantlet shoulder must tuck into the turret, not leave an air gap (shoulder {rear_shoulder_z:.2}, front {front_z:.2})"
    );
    assert!(
        mantlet.max.z > front_z + 0.20,
        "the mantlet face must protrude to cover the embrasure opening (face {:.2}, front {front_z:.2})",
        mantlet.max.z
    );
}

#[test]
fn the_mantlet_side_silhouette_has_no_daylight_gap_to_the_turret() {
    let v =
        *game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let mantlet = revolve::moving_mantlet(trunnion, &v.gun);
    let turret = vehicle_build::t54_turret_loft(&v.turret_loft);

    // The mask stands PROUD of the casting (that is the visible pig's head), but through the seal
    // zone around the gun axis its REAR edge must tuck to (or behind) the local turret face —
    // otherwise the side view shows daylight between the mask and the casting. (The oval's extreme
    // top/bottom rims legitimately stand ahead of the receding dome, as on the real embrasure.)
    let mut worst_gap = f32::NEG_INFINITY;
    let mut worst_sample = (0.0, 0.0, 0.0);
    for band in 0..7 {
        let y = trunnion.y - 0.15 + 0.05 * band as f32;
        let mask_rear = mantlet
            .vertices()
            .iter()
            .filter(|vertex| (vertex.position.y - y).abs() <= 0.03)
            .map(|vertex| vertex.position.z)
            .fold(f32::INFINITY, f32::min);
        let turret_front = turret
            .vertices()
            .iter()
            .filter(|vertex| (vertex.position.y - y).abs() <= 0.06)
            .map(|vertex| vertex.position.z)
            .fold(f32::NEG_INFINITY, f32::max);
        if !mask_rear.is_finite() || !turret_front.is_finite() {
            continue;
        }
        let gap = mask_rear - turret_front;
        if gap > worst_gap {
            worst_gap = gap;
            worst_sample = (y, mask_rear, turret_front);
        }
    }

    assert!(
        worst_gap <= 0.04,
        "the mask's rear edge must tuck into the turret face through the seal zone, gap {worst_gap:.3} at y {:.3}, mask rear z {:.3}, turret z {:.3}",
        worst_sample.0,
        worst_sample.1,
        worst_sample.2
    );
}

#[test]
fn t54_fenders_carry_the_reference_stowage_line() {
    // The references' top view: fuel tanks and bins line the FENDER shelves over the exposed
    // tracks — the kit that carries the visual mass beside the narrow hull box. Locked so the
    // fenders never regress to bare shelves. Everything stays inside the track span and well
    // below the hull roof.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.hybrid().unwrap();
    let fender_top = v.fender.center_y + v.fender.half.y;
    let outer = bp.track.outer_x;
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;

    for side in [1.0_f32, -1.0] {
        let stowage: Vec<_> = hull
            .vertices()
            .iter()
            .filter(|vert| vert.position.x * side > bp.hull.half_width)
            .filter(|vert| {
                vert.position.y > fender_top + 0.03 && vert.position.y < bp.hull.deck_y - 0.05
            })
            .collect();
        assert!(
            stowage.len() >= 8,
            "fender (side {side}) must carry stowage above its shelf, got {} vertices",
            stowage.len()
        );
        let (mut zs_min, mut zs_max) = (f32::INFINITY, f32::NEG_INFINITY);
        for vert in &stowage {
            assert!(
                vert.position.x.abs() <= outer + 1.0e-3,
                "stowage must not widen the vehicle: x {}",
                vert.position.x
            );
            zs_min = zs_min.min(vert.position.z);
            zs_max = zs_max.max(vert.position.z);
        }
        assert!(
            zs_max - zs_min > 3.5,
            "stowage should line the fender run, span {:.2}",
            zs_max - zs_min
        );
    }
}

/// The cupola, the loader's hatch ring, the DShK pedestal and the periscopes sit on a CURVED
/// casting, not on a flat roof: each must reach deep enough to meet the local shell surface while
/// still standing proud of it. That is what stops a fitting floating as a drum in mid-air.
///
/// It used to be written as absolute heights — 2.06 here, 2.30 there — measured off a 2.27 m
/// roof. The moment the dome moved (PR-15 raised it to its documented 2.40 and re-shaped it from
/// the S1 stations) those numbers described a casting that no longer exists, and the test failed
/// the correct model. So it MEASURES the dome now, at each fitting's own place on it.
#[test]
fn t54_roof_fittings_root_into_the_curved_dome() {
    let desc = t54_description();
    let casting = desc
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("the turret is one lofted casting")
        .mesh();

    for key in ["cupola", "loader_hatch", "dshk_mount", "turret_periscope"] {
        let part = desc
            .parts
            .iter()
            .find(|part| part.key.name == key)
            .unwrap_or_else(|| panic!("part {key} present"));
        let bounds = part.mesh().bounds().expect("part has bounds");
        let (cx, cz) = ((bounds.min.x + bounds.max.x) * 0.5, (bounds.min.z + bounds.max.z) * 0.5);
        // The casting's surface under this fitting: the top of the metal AT ITS OWN PLACE on
        // the dome. Sampled from the nearest casting vertices in plan — a wide window would
        // reach the crown and report the roof for a fitting that sits out on the slope.
        let mut near: Vec<&vehicle_geometry::GeometryVertex> = casting.vertices().iter().collect();
        near.sort_by(|a, b| {
            let da = (a.position.x - cx).hypot(a.position.z - cz);
            let db = (b.position.x - cx).hypot(b.position.z - cz);
            da.total_cmp(&db)
        });
        let surface = near.iter().take(12).map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
        assert!(surface.is_finite(), "{key} sits over no casting at all");

        assert!(
            bounds.min.y <= surface,
            "{key} base {:.2} floats above the casting under it ({surface:.2})",
            bounds.min.y
        );
        assert!(
            bounds.max.y > surface,
            "{key} top {:.2} is buried in the casting ({surface:.2})",
            bounds.max.y
        );
        // And it stands PROUD, not flush: a fitting level with the roof is a texture.
        assert!(
            bounds.max.y - surface > 0.02,
            "{key} stands only {:.3} m proud of the casting — that reads as paint",
            bounds.max.y - surface
        );
    }
}

#[test]
fn t54_turret_has_a_nested_inward_facing_armor_skin() {
    let desc = t54_description();
    let outer = desc
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("outer turret shell")
        .mesh();
    let inner = desc
        .parts
        .iter()
        .find(|part| part.key.name == "turret_inner_skin")
        .expect("inner turret skin")
        .mesh();
    let outer_bounds = outer.bounds().expect("outer bounds");
    let inner_bounds = inner.bounds().expect("inner bounds");
    assert!(inner_bounds.min.x > outer_bounds.min.x + 0.05);
    assert!(inner_bounds.max.x < outer_bounds.max.x - 0.05);
    assert!(inner.vertices().iter().all(|vertex| {
        vertex.material == MaterialRole::InteriorPrimer && vertex.normal.is_finite()
    }));

    let side_normals: Vec<_> = inner
        .vertices()
        .iter()
        .filter(|vertex| Vec3::new(vertex.position.x, 0.0, vertex.position.z).length() > 0.55)
        .map(|vertex| {
            vertex.normal.dot(Vec3::new(vertex.position.x, 0.0, vertex.position.z).normalize())
        })
        .collect();
    assert!(
        side_normals.iter().copied().sum::<f32>() / (side_normals.len() as f32) < -0.45,
        "inner turret normals must face the fighting compartment"
    );
}

#[test]
fn t54_muzzle_ends_in_a_recessed_bore_not_a_capped_rod() {
    let baked = t54_description().build();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let muzzle_z = gun.bounds().expect("gun bounds").max.z;
    let radial = |v: &vehicle_geometry::GeometryVertex| {
        let (dx, dy) = (v.position.x, v.position.y - trunnion.y);
        (dx * dx + dy * dy).sqrt()
    };

    // The muzzle face is a RING: no vertex on the end plane sits near the axis.
    for v in gun.vertices().iter().filter(|v| v.position.z >= muzzle_z - 1.0e-3) {
        assert!(
            radial(v) >= 0.035,
            "the muzzle end must be an open ring, found solid cap at radial {:.3}",
            radial(v)
        );
    }
    // The dark bore duct sits recessed behind the rim.
    let bore = gun.vertices().iter().any(|v| {
        v.position.z < muzzle_z - 0.02 && v.position.z > muzzle_z - 0.20 && radial(v) < 0.06
    });
    assert!(bore, "a recessed bore duct must sit behind the muzzle ring");
}

/// The pig's-head mask must be a CAST part visibly wrapping the barrel root ahead of the
/// casting — not a bare steel tube glued to the dome.
///
/// "Ahead of the casting" used to be written as `z > 1.05`, a number taken off the turret the
/// model had. When the casting's front moved (PR-15: S1 puts it 1.016 m forward of the ring, and
/// the mask bedded back with it) that number described a face that no longer exists. It measures
/// the casting now.
#[test]
fn t54_mask_stands_proud_as_cast_armor_on_the_turret_face() {
    let description = t54_description();
    let casting = description
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("the turret is one lofted casting")
        .mesh();
    let trunnion_y = description.mounts.gun_trunnion.translation.y;
    // The casting's face at the gun's own height — the embrasure the mask beds into.
    let face_z = casting
        .vertices()
        .iter()
        .filter(|v| (v.position.y - trunnion_y).abs() < 0.10 && v.position.x.abs() < 0.30)
        .map(|v| v.position.z)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(face_z.is_finite(), "the casting has a face at the gun height");

    let baked = description.build();
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let proud = gun.vertices().iter().any(|v| {
        v.material == MaterialRole::CastArmor && v.position.z > face_z && v.position.x.abs() > 0.30
    });
    assert!(proud, "the cast mask must stand wide and proud of the turret face at {face_z:.2}");
}

#[test]
fn t54_fender_ends_slope_over_idler_and_sprocket() {
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    for sign in [1.0_f32, -1.0] {
        let slope = hull.vertices().iter().any(|v| {
            v.material == MaterialRole::RolledArmor
                && v.position.z * sign > 2.78
                && v.position.y < 1.00
                && v.position.x.abs() > 1.05
        });
        assert!(
            slope,
            "the fender end (z sign {sign}) must drop over the track end as the references show"
        );
    }
}
