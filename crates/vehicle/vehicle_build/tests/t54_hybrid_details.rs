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
    let v = bp.complete_visual().unwrap();
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
    assert!(f.driver_hatch_center.x > 0.0, "driver hatch sits on the left (port, +X)");
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let driver_apex = lid_apex(hull, f.driver_hatch_center, f.driver_hatch_radius);
    assert!(
        driver_apex >= f.driver_hatch_center.y + f.driver_hatch_half_height - 0.01,
        "driver hatch lid rises above the hull roof, apex {driver_apex:.2}"
    );

    // Loader's hatch: a raised round lid on the turret roof, loader (right) side — it rides the
    // turret submesh so it traverses with the vehicle.
    assert!(f.loader_hatch_center.x < 0.0, "loader hatch sits on the loader (starboard, -X) side");
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
            && v.position.x > 0.0
    });
    assert!(
        driver_raked,
        "driver periscopes must read as raked prism heads on the hull roof, left (port, +X)"
    );
}

#[test]
fn t54_fenders_are_segmented_with_support_brackets() {
    use std::collections::HashSet;
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;

    // Segmented fender: the outer fender top now carries many distinct z-edges (one per section
    // boundary); a single continuous slab would expose only the two end edges.
    // The fender rides its measured 1.35 sheet plane (2026-08-12, both projections), so the
    // outer edge lives above 1.3 while staying below the 1.58 roof.
    let mut z_bands: HashSet<i32> = HashSet::new();
    for v in hull.vertices() {
        if v.material == MaterialRole::RolledArmor && v.position.x > 1.6 && v.position.y > 1.3 {
            z_bands.insert((v.position.z / 0.05).round() as i32);
        }
    }
    assert!(
        z_bands.len() >= 6,
        "fender splits into bolted sections, got {} z-bands",
        z_bands.len()
    );

    // Support gussets hang just below the fender shelf at the HULL side, on both fenders —
    // triangulating against the wall like the real supports. With the shelf at its measured
    // 1.35 sheet plane the gusset band sits at ~1.30-1.35, far clear of the links below.
    let has_bracket = |sign: f32| {
        hull.vertices().iter().any(|v| {
            v.material == MaterialRole::TrackMetal
                && v.position.x * sign > 1.00
                && v.position.x * sign < 1.20
                && v.position.y > 1.28
                && v.position.y < 1.36
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
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
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

/// Tight variant for the doubly-curved face: a ±15 mm x-window so a sample on the shelf cannot
/// swallow the window wall or a cheek column standing 35 mm away. Non-finite means "no ring
/// column here" — callers scan a small range instead of trusting one x.
fn face_z_tight(mesh: &vehicle_geometry::GeometryMesh, y: f32, x: f32) -> f32 {
    mesh.vertices()
        .iter()
        .filter(|v| {
            (v.position.y - y).abs() < 0.035
                && (v.position.x.abs() - x).abs() < 0.015
                && v.position.z > 0.6
        })
        .map(|v| v.position.z)
        .fold(f32::NEG_INFINITY, f32::max)
}

/// Best finite tight sample over a scanned x-range (the loft's azimuth columns are discrete, so
/// any single x can fall between rings).
fn face_z_scan(mesh: &vehicle_geometry::GeometryMesh, y: f32, x0: f32, x1: f32) -> f32 {
    let mut best = f32::NEG_INFINITY;
    let mut x = x0;
    while x <= x1 {
        best = best.max(face_z_tight(mesh, y, x));
        x += 0.01;
    }
    best
}

/// The casting's front surface at the gun's height, as a function of how far out from the gun
/// axis you look — the instrument every aperture assertion below reads.
fn face_z_at(mesh: &vehicle_geometry::GeometryMesh, y: f32, x: f32, dy: f32) -> f32 {
    mesh.vertices()
        .iter()
        .filter(|v| {
            (v.position.y - y - dy).abs() < 0.035
                && (v.position.x.abs() - x).abs() < 0.035
                && v.position.z > 0.6
        })
        .map(|v| v.position.z)
        .fold(f32::NEG_INFINITY, f32::max)
}

/// M4. The T-54 obr. 1951 has a NARROW APERTURE with the mantlet behind it. The external
/// "pig's head" ball is a WoT-ism the dossier rules out by name, and the model carried one:
/// a 0.64 m mask reaching 0.40 m past the trunnion, out through the casting and into the air,
/// filling a metre-wide soft dish that had been dished out to receive it.
///
/// Measured the way a person with a tape measure would: walk out from the gun axis across the
/// turret face and find where the metal steps back forward. Not compared against the blueprint's
/// own bump function — that would only prove the blueprint equals itself.
#[test]
fn the_turret_face_carries_a_narrow_gun_aperture() {
    let description = t54_description();
    let casting = description
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("the turret is one lofted casting")
        .mesh();
    let gun_y = description.mounts.gun_trunnion.translation.y;

    let floor = face_z_at(&casting, gun_y, 0.0, 0.0);
    let outside = casting.bounds().expect("casting bounds").max.z;
    let depth = outside - floor;
    assert!(
        depth >= 0.13,
        "the aperture must be a cavity cut through the wall, got {depth:.3} m deep"
    );

    // Half depth is the edge: the width a tape measure reports. Vertically the dome curls
    // away above the window FASTER than the recess is deep (a_f drops 1.15 -> 1.04 across the
    // window band), so no global plane ever crosses — the height walk uses a LOCAL threshold:
    // 0.07 above the floor sits inside the aperture's top wall rise and below the curl.
    let edge = floor + depth * 0.5;
    let cross = |vertical: bool, threshold: f32| {
        let mut last = 0.0;
        for step in 1..=40 {
            let offset = step as f32 * 0.01;
            let z = if vertical {
                face_z_at(&casting, gun_y, 0.0, offset)
            } else {
                face_z_at(&casting, gun_y, offset, 0.0)
            };
            if z.is_finite() {
                if z >= threshold {
                    return offset;
                }
                last = offset;
            }
        }
        last
    };
    let width = 2.0 * cross(false, edge);
    // Vertically no single number survives the dome's own curl (above the window the whole
    // casting sits behind the floor level in z), so the lock is three physical bounds on the
    // TRULY deep band (z within 30 mm of the floor, y capped at the gun's travel envelope):
    // the cut clears the gun's travel both ways and never reaches the ring seat.
    let deep: Vec<f32> = casting
        .vertices()
        .iter()
        .filter(|v| {
            v.position.x.abs() < 0.06
                && (v.position.y - gun_y).abs() < 0.25
                && v.position.z > 0.6
                && v.position.z <= floor + 0.03
        })
        .map(|v| v.position.y)
        .collect();
    let deep_lo = deep.iter().copied().fold(f32::INFINITY, f32::min);
    let deep_hi = deep.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        deep_lo <= gun_y - 0.10 && deep_hi >= gun_y + 0.10,
        "the aperture clears the tube and its travel, got deep band {deep_lo:.3}..{deep_hi:.3}"
    );
    assert!(
        deep_lo >= 1.60,
        "and it never cuts the ring seat at 1.58, got its floor down to {deep_lo:.3}"
    );
    assert!(
        (width - 0.40).abs() <= 0.06,
        "the documented aperture is ~0.40 m across, measured {width:.3}"
    );

    // And the recess is the WINDOW: the 0.40 m aperture plus ~50 mm of wall each side — the
    // MEASURED ~0.50 m pillow of the references (module 2), not the first build's 0.85 m
    // letterbox (an eyeballed number that lived in this very assert). There is no broad shelf
    // on the real casting; the reveal between aperture and window wall IS the wall thickness.
    // Measured here as the recess's outer width near face level: walk out from the centre and
    // find where the surface climbs back to within 25 mm of the plate around the window.
    let plate = face_z_scan(&casting, gun_y, 0.30, 0.42);
    assert!(plate.is_finite(), "the face plate flanks the window");
    let mut window_half = 0.10;
    for step in 0..=30 {
        let offset = 0.10 + step as f32 * 0.01;
        let z = face_z_tight(&casting, gun_y, offset);
        if z.is_finite() {
            if z >= plate - 0.025 {
                break;
            }
            window_half = offset;
        }
    }
    let window_width = 2.0 * window_half;
    assert!(
        (0.42..=0.60).contains(&window_width),
        "the window is the measured ~0.50 m pillow, got {window_width:.3}"
    );
}

/// M4 + K2. The mantlet is INSIDE: its face never reaches the casting around it, and it is wider
/// than the hole, which is the whole mechanical point of an internal mount — it cannot be driven
/// back through the aperture by a hit.
///
/// And it is a BODY. It used to be a sleeve open at both ends — no station reached radius zero —
/// so its rims stood in mid-air where nothing met them, and the mesh contract the gun submesh is
/// held to (`OPEN_OR_CLOSED`) had no opinion about it.
#[test]
fn the_mantlet_is_an_internal_closed_body_wider_than_its_aperture() {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let mantlet = revolve::moving_mantlet(trunnion, v.gun);
    let bounds = mantlet.bounds().expect("mantlet bounds");
    let casting = vehicle_build::t54_turret_loft(v.turret_loft);
    let reach = casting.bounds().expect("casting bounds").max.z;

    assert!(
        bounds.max.z < reach - 0.08,
        "an internal mantlet stays behind the casting: face {:.3} vs casting {reach:.3}",
        bounds.max.z
    );
    let floor = face_z_at(&casting, trunnion.y, 0.0, 0.0);
    assert!(
        bounds.max.z > floor,
        "its face still shows through the aperture: face {:.3} vs pocket floor {floor:.3}",
        bounds.max.z
    );

    let half_width = bounds.max.x.max(-bounds.min.x);
    assert!(
        half_width > 0.22,
        "the mantlet must be wider than the 0.40 m aperture it sits behind, got half-width {half_width:.3}"
    );

    let report = mantlet.quality_report(vehicle_geometry::OPEN_OR_CLOSED_MESH);
    assert_eq!(
        report.boundary_edges, 0,
        "a cast mantlet is a closed body, not a tube with two open rims"
    );
    assert_eq!(report.non_manifold_edges, 0, "and a manifold one");
}

/// K2 → the window rebuild. With the mantlet inside, what closes the hole on a T-54 is a wide
/// rectangular canvas PANEL clamped into the shallow window by a perimeter strip, gathering into
/// a short sleeve on the tube. The first build's boot semantics ("rear ring swallowed at its own
/// radius") described a round plug — a panel's hem legitimately swells PROUD over its frame, so
/// what is locked here is the panel's own grammar: hem rooted at the window shelf, fabric closing
/// the deep aperture and wrapping past the metal it hides, sleeve gripping the tube.
#[test]
fn the_canvas_cover_seals_the_aperture_to_the_barrel() {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let casting = vehicle_build::t54_turret_loft(v.turret_loft);
    let baked = t54_description().build();
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let cover: Vec<Vec3> = gun
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::Canvas)
        .map(|vertex| vertex.position)
        .collect();
    assert!(!cover.is_empty(), "the gun mount carries a canvas cover");

    let radius = |p: &Vec3| p.x.hypot(p.y - trunnion.y);
    // The hem roots at the window shelf: rearmost fabric within a fastening strip of the shelf,
    // and swallowed past the cheeks' own front — a hem floating at face depth would read as a
    // pasted-on decal, not a fastened cover.
    let hem = cover.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let shelf = face_z_at(&casting, trunnion.y, 0.18, 0.0);
    let outside = casting.bounds().expect("casting bounds").max.z;
    assert!(
        hem < shelf + 0.06,
        "the hem must root at the window shelf, got z {hem:.3} against a shelf at {shelf:.3}"
    );
    assert!(
        hem < outside - 0.04,
        "the hem must sit INSIDE the window, got z {hem:.3} against a face at {outside:.3}"
    );

    // The fabric spans the window it is fastened over — panel width, not boot width.
    let half_span = cover.iter().map(|p| p.x.abs()).fold(0.0_f32, f32::max);
    assert!(
        (0.20..=0.27).contains(&half_span),
        "the panel spans the measured ~0.50 m window, got half-span {half_span:.3}"
    );

    // The sleeve grips the tube: the barrel is 0.098 there, so no daylight around it.
    let front = cover.iter().max_by(|a, b| a.z.total_cmp(&b.z)).unwrap();
    assert!(
        radius(front) < 0.115,
        "the cover must clamp the tube, not hang off it: ring radius {:.3}",
        radius(front)
    );

    // In between it stands AHEAD of the pocket, which is what closing the hole means.
    let pocket_floor = face_z_at(&casting, trunnion.y, 0.0, 0.0);
    let over_pocket =
        cover.iter().filter(|p| radius(p) < 0.19).map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        over_pocket > pocket_floor + 0.05,
        "the cover must close the aperture, not lie in it: fabric at {over_pocket:.3}, floor at {pocket_floor:.3}"
    );

    // And the metal it exists to hide stays hidden: the fabric reaches past the mantlet's face.
    let mantlet = revolve::moving_mantlet(trunnion, v.gun);
    let mantlet_face = mantlet.bounds().expect("mantlet bounds").max.z;
    assert!(
        front.z > mantlet_face + 0.05,
        "the fabric must wrap past the metal: sleeve ends {:.3}, mantlet face {mantlet_face:.3}",
        front.z
    );
}

/// The fastening frame is the crisp rectangular outline the eye reads first, and it only reads
/// true if it actually clamps THIS panel into THIS window: strip just outside the fabric hem on
/// both axes, buried inside the window's depth, inside the window's walls.
#[test]
fn the_cover_frame_matches_the_window() {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
    let trunnion = MountFrames::for_vehicle(VehicleKind::T54_1951).gun_trunnion.translation;
    let casting = vehicle_build::t54_turret_loft(v.turret_loft);
    let description = t54_description();
    let frame = description
        .parts
        .iter()
        .find(|part| part.key.name == "gun_mantlet_frame")
        .expect("the cover is fastened by a perimeter strip")
        .mesh();
    let bounds = frame.bounds().expect("frame bounds");

    let (fx, fy) = v.gun.canvas.expect("the benchmark wears its canvas").frame_half;
    let clearance_x = bounds.max.x - fx;
    let clearance_y = bounds.max.y - trunnion.y - fy;
    assert!(
        (0.005..=0.05).contains(&clearance_x) && (0.005..=0.05).contains(&clearance_y),
        "the strip clamps just outside the hem on both axes, got +{clearance_x:.3} / +{clearance_y:.3}"
    );

    let outside = casting.bounds().expect("casting bounds").max.z;
    assert!(
        bounds.max.z < outside - 0.02,
        "the strip is buried in the window, not proud of the face: z {:.3} vs {outside:.3}",
        bounds.max.z
    );

    // Inside the window's walls: the shelf the strip clamps into must sit visibly DEEPER than
    // the wall-and-cheek band just outside the strip — a strip wider than its window would sit
    // on the wall and read as trim glued to the cheek.
    let inside_strip = face_z_scan(&casting, trunnion.y, 0.14, 0.20);
    let outside_strip = face_z_scan(&casting, trunnion.y, bounds.max.x + 0.02, bounds.max.x + 0.10);
    assert!(
        inside_strip.is_finite() && outside_strip.is_finite(),
        "both sides of the strip's edge must be sampled, got {inside_strip:.3} / {outside_strip:.3}"
    );
    assert!(
        outside_strip > inside_strip + 0.015,
        "the wall must rise outside the strip over the shelf inside it, got {inside_strip:.3} -> {outside_strip:.3}"
    );
}

#[test]
fn t54_fenders_carry_the_reference_stowage_line() {
    // The references' top view: fuel tanks and bins line the FENDER shelves over the exposed
    // tracks — the kit that carries the visual mass beside the narrow hull box. Locked so the
    // fenders never regress to bare shelves. Everything stays inside the track span and well
    // below the hull roof.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
    let fender_top = v.fender.center_y + v.fender.half.y;
    let outer = bp.track.outer_x;
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let description = t54_description();

    for side in [1.0_f32, -1.0] {
        let stowage: Vec<_> = hull
            .vertices()
            .iter()
            .filter(|vert| vert.position.x * side > bp.hull.half_width)
            .filter(|vert| {
                vert.position.y > fender_top + 0.03 && vert.position.y < bp.hull.deck_y + 0.04
            })
            .collect();
        assert!(
            stowage.len() >= 8,
            "fender (side {side}) must carry stowage above its shelf, got {} vertices",
            stowage.len()
        );
        // The lids close the silhouette AT the roof line: both projections of the reference
        // sheet draw box tops and roof as ONE line. A stowage line ending a hand below the
        // deck is the "boxes on the ground" read the 2026-08-12 review called out. Measured
        // on the BOX bodies (`PartShape::Plates`) — the lid furniture (hinge knuckles, grab
        // handles) rides a knuckle higher and is not the line the silhouette reads.
        let lid_line = description
            .parts
            .iter()
            .filter(|part| matches!(part.key.name, "stowage_bin" | "fuel_tank"))
            .filter(|part| matches!(part.shape, vehicle_build::PartShape::Plates(_)))
            .filter_map(|part| part.mesh().bounds())
            .filter(|b| (b.min.x + b.max.x) * 0.5 * side > 0.0)
            .map(|b| b.max.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (bp.hull.deck_y - 0.03..=bp.hull.deck_y + 0.01).contains(&lid_line),
            "stowage lids (side {side}) must sit flush with the {:.2} roof line, got {lid_line:.3}",
            bp.hull.deck_y
        );
        let (mut zs_min, mut zs_max) = (f32::INFINITY, f32::NEG_INFINITY);
        // The band's widest honest reach is the FENDER LINE (side_x + half.x = the documented
        // 3.27 m over the fenders): the mudguard arches span exactly that, like the shelf
        // whose run they continue. Stowage proper stays inside the track band below it.
        let fender_edge = bp.complete_visual().expect("visual").fender.side_x
            + bp.complete_visual().expect("visual").fender.half.x;
        for vert in &stowage {
            assert!(
                vert.position.x.abs() <= fender_edge.max(outer) + 1.0e-3,
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

/// M7. The fender asymmetry is a RECOGNITION feature, not decoration: the right shelf carries two
/// flat external fuel tanks with stowage fore and aft; the left carries three stowage bins and the
/// exhaust. The model had three tanks on the right, which is a later fit.
///
/// Counted from the part graph, because that is where the vehicle says what each box IS. Counting
/// boxes in the merged mesh would only tell you how many boxes there are.
#[test]
fn t54_fenders_are_asymmetric_the_way_the_references_are() {
    let description = t54_description();
    // Count STOWAGE, not parts. A bin is now several parts sharing one key — the box, its lid
    // seam, its hinge and its latch — because the construction floor reads a key's parts as the
    // device they make up. The box is the one carrying the volume, so it is the one counted:
    // `PartShape::Plates` is the box, everything else riding the name is furniture on it.
    let count = |name: &str, side: f32| {
        description
            .parts
            .iter()
            .filter(|part| part.key.name == name)
            .filter(|part| matches!(part.shape, vehicle_build::PartShape::Plates(_)))
            .filter(|part| {
                part.mesh().bounds().is_some_and(|b| (b.min.x + b.max.x) * 0.5 * side > 0.0)
            })
            .count()
    };
    assert_eq!(
        count("fuel_tank", -1.0),
        2,
        "the right (starboard, -X) shelf carries two flat fuel tanks"
    );
    assert_eq!(count("fuel_tank", 1.0), 0, "the left shelf carries none");
    assert_eq!(
        count("stowage_bin", 1.0),
        3,
        "the left (port, +X) shelf carries three stowage bins"
    );
    assert!(
        count("stowage_bin", -1.0) >= 2,
        "and the right carries stowage fore and aft of its tanks"
    );
}

/// M7. The fixed course machine gun. obr. 1951 has an SG-43 laid in the glacis, fired by the
/// driver — and the driver sits LEFT, so the gun is right of centre. The model had a bare plate.
#[test]
fn t54_carries_a_course_machine_gun_port_right_of_centre() {
    let description = t54_description();
    let port = description
        .parts
        .iter()
        .find(|part| part.key.name == "course_mg_port")
        .expect("the glacis carries the course MG's port");
    let bounds = port.mesh().bounds().expect("port bounds");
    let center_x = (bounds.min.x + bounds.max.x) * 0.5;
    assert!(
        center_x < -0.15,
        "the gun is right of centre (starboard, -X), opposite the driver: x {center_x:.3}"
    );

    let driver = description
        .parts
        .iter()
        .find(|part| part.key.name == "driver_hatch")
        .expect("driver's hatch");
    let driver_x = driver.mesh().bounds().expect("hatch bounds");
    assert!(
        (driver_x.min.x + driver_x.max.x) * 0.5 > 0.0,
        "and the driver is on the other side of it"
    );

    // It reaches THROUGH the plate: a port is an aperture with a boss round it, not a stud.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.complete_visual().unwrap();
    let glacis = bp.armor.hull_front.0.to_radians();
    let plate_z = (v.hull.glacis_offset - glacis.sin() * 1.15) / glacis.cos();
    assert!(bounds.max.z > plate_z, "the jacket stands proud of the glacis");
    assert!(bounds.min.z < plate_z, "and the boss is rooted behind it");
}

/// M7, at the documented size. The tank smoke canister is the BDSh-5: a 650 mm drum 450 mm
/// across, 45-50 kg, carried in pairs on the lower rear plate. The first build drew 220 mm drums
/// sized by the collision box instead of by a source — the compromise this test used to LOCK, by
/// demanding the drums stay inside the footprint. Now it locks the documented girth, and the
/// honest consequence: drums that size necessarily reach past the hitbox, by the same catalogued
/// exception the main gun holds (`hitbox_fit::HITBOX_EXCEPTIONS`).
#[test]
fn t54_carries_two_smoke_canisters_on_the_rear_plate() {
    let description = t54_description();
    let canisters: Vec<_> =
        description.parts.iter().filter(|part| part.key.name == "smoke_canister").collect();
    assert_eq!(canisters.len(), 2, "obr. 1951 carries two BDSh-5 canisters");

    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let mut sides = 0.0_f32;
    for canister in &canisters {
        let b = canister.mesh().bounds().expect("canister bounds");
        let cx = (b.min.x + b.max.x) * 0.5;
        sides += cx.signum();
        // The documented drum, measured off the mesh. The DIAMETER is measured radially — the
        // first draft read the bounding box, and a bounding box measures a 14-gon across its
        // FLATS (439 mm for a 450 mm circle): the instrument-measures-the-wrong-thing defect,
        // in miniature, inside the very pass that was hunting it.
        let cy = (b.min.y + b.max.y) * 0.5;
        let cz = (b.min.z + b.max.z) * 0.5;
        let mesh = canister.mesh();
        let radius = mesh
            .vertices()
            .iter()
            .map(|v| (v.position.y - cy).hypot(v.position.z - cz))
            .fold(0.0_f32, f32::max);
        let length = b.max.x - b.min.x;
        assert!(
            (radius * 2.0 - 0.45).abs() <= 0.005,
            "a BDSh-5 is 450 mm across, got {:.3}",
            radius * 2.0
        );
        assert!((length - 0.65).abs() <= 0.01, "and 650 mm long, got {length:.3}");
        assert!(
            b.max.z < -bp.hull.half_len + 0.10,
            "it hangs on the rear plate, not inside the hull: {:.3}",
            b.max.z
        );
        assert!(
            b.max.x.abs().max(b.min.x.abs()) <= bp.hull.half_width,
            "and stays inside the hull's own width"
        );
        // HIGH on the upper plate, under the deck lip — the documented seat (the first pass
        // left them lying at the tail's foot) — and clear of the log stowed LOW below them.
        assert!(b.max.y < bp.hull.deck_y, "tucked under the deck lip: top {:.3}", b.max.y);
        assert!(b.min.y > 1.00, "clear of the beam stowed below: bottom {:.3}", b.min.y);
        // A 50 kg drum is strapped, not glued.
        let strap = description
            .parts
            .iter()
            .find(|p| {
                p.key.name == "smoke_canister_strap" && p.key.instance == canister.key.instance
            })
            .expect("each drum carries its quick-release straps");
        assert!(strap.mesh().triangle_count() > 50, "real straps, not a decal");
    }
    assert_eq!(sides, 0.0, "one canister each side");
}

/// M10. There is no gun travel lock. One was drawn on the rear deck with no citation behind it,
/// and the dossier says obr. 1951 carried none. A fitting nobody can cite is a fitting the
/// vehicle does not have — and the way to keep it gone is to say so in a test.
#[test]
fn t54_has_no_external_gun_travel_lock() {
    let description = t54_description();
    let found: Vec<&str> = description
        .parts
        .iter()
        .map(|part| part.key.name)
        .filter(|name| name.contains("travel_lock"))
        .collect();
    assert!(found.is_empty(), "obr. 1951 has no travel lock, found {found:?}");
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

    // `dshk_ring`, not `dshk_mount`: the gun turns on the loader hatch's ring now (PR-26b),
    // so the RING is the part that meets the casting and the pintle stands on the ring.
    for key in ["cupola", "loader_hatch", "dshk_ring", "turret_periscope"] {
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

/// M4, the other way round. This test used to REQUIRE a cast mask standing wide and proud of the
/// turret face — it locked the very defect the dossier names: "the 'pig's head' external ball is
/// a WoT-ism, not obr. 1951". A test that demands the wrong shape is not a weaker test than one
/// that demands the right shape; it is the defect, written down twice.
///
/// What it asserts now is the truth about this vehicle: outside the casting there is the tube and
/// the canvas over the hole, and no cast armour at all.
#[test]
fn no_cast_armour_of_the_gun_mount_stands_proud_of_the_turret_face() {
    let description = t54_description();
    let casting = description
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("the turret is one lofted casting")
        .mesh();
    let reach = casting.bounds().expect("casting bounds").max.z;

    let baked = description.build();
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let proud: Vec<f32> = gun
        .vertices()
        .iter()
        .filter(|v| v.material == MaterialRole::CastArmor && v.position.z > reach)
        .map(|v| v.position.z)
        .collect();
    assert!(
        proud.is_empty(),
        "an internal mantlet puts no cast armour ahead of the casting at {reach:.3}, found {} vertices (the furthest at z {:.3})",
        proud.len(),
        proud.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    );
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
