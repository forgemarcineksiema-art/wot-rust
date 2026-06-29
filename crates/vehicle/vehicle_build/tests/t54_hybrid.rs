//! Locking tests for the hybrid T-54 description (`vehicle_build::t54`). They live here as an
//! integration tree so the production module stays within the reviewability budget; every one drives
//! only the crate's public API (`t54_description`, `t54_from_modules`, `MEDIUM_LOD0_TRI_BUDGET`).

use game_core::{MountFrames, VehicleKind};
use glam::Vec3;
use vehicle_build::{MEDIUM_LOD0_TRI_BUDGET, t54_description, t54_from_modules};
use vehicle_geometry::{MaterialRole, RunningGearKinematics, SmoothingGroup, SubmeshKind};

#[test]
fn the_blueprint_is_the_sole_source_of_hull_dimensions() {
    // The generated hull is a pure function of the blueprint's HullVisual — no parallel constant
    // lives in the generator. Its extents track the blueprint block, and perturbing the
    // blueprint copy moves the geometry with it.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.hybrid().unwrap();
    let to_bounds = |hull: &game_core::HullVisual| {
        solid::t54_hull_solid(
            hull,
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
        )
        .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
        .expect("hull solid is valid")
        .bounds()
        .expect("non-empty hull")
    };

    let plate = to_bounds(&v.hull);
    assert!((plate.max.x - v.hull.half_width).abs() < 0.05, "hull width tracks the blueprint");
    assert!((plate.min.y - v.hull.belly_y).abs() < 0.05, "hull belly tracks the blueprint");
    assert!((plate.max.y - v.hull.roof_y).abs() < 0.05, "hull roof tracks the blueprint");

    let mut wide = v.hull;
    wide.half_width *= 2.0;
    let wide_plate = to_bounds(&wide);
    assert!(
        wide_plate.max.x > plate.max.x * 1.8,
        "doubling the blueprint half-width widens the generated hull (no parallel constant)"
    );
}

#[test]
fn glacis_geometry_slope_matches_the_armour_blueprint() {
    // The CAD glacis plate is built from the blueprint armour facet — the single source — so the
    // *built geometry* carries that angle in the armour convention: a glacis face normal sits
    // `slope` degrees above horizontal (atan2(n.y, n.z)) — what you see is what you shoot.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has a blueprint");
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let on_slope = hull.vertices().iter().any(|v| {
        let n = v.normal;
        n.y > 0.2 && n.z > 0.2 && (n.y.atan2(n.z).to_degrees() - bp.armor.hull_front.0).abs() < 2.0
    });
    assert!(on_slope, "a glacis face normal carries the armour slope angle");
}

#[test]
fn every_hull_facet_carries_its_blueprint_armour_angle() {
    // Coherence extended past the glacis: the sloped sides and rear plates carry hull_side and
    // hull_rear in the same convention — what you see is what you shoot, on every facet.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let baked = t54_description().build();
    let ns: Vec<Vec3> = baked
        .submesh(SubmeshKind::Hull)
        .unwrap()
        .mesh
        .vertices()
        .iter()
        .map(|v| v.normal)
        .collect();
    let near = |found: f32, target: f32| (found - target).abs() < 2.0;
    let glacis =
        ns.iter().any(|n| n.z > 0.2 && near(n.y.atan2(n.z).to_degrees(), bp.armor.hull_front.0));
    let side =
        ns.iter().any(|n| n.x > 0.5 && near(n.y.atan2(n.x).to_degrees(), bp.armor.hull_side.0));
    let rear =
        ns.iter().any(|n| n.z < -0.5 && near(n.y.atan2(-n.z).to_degrees(), bp.armor.hull_rear.0));
    assert!(
        glacis && side && rear,
        "glacis={glacis} side={side} rear={rear} must each match blueprint"
    );
}

#[test]
fn the_description_builds_hull_and_turret_within_budget() {
    let baked = t54_description().build();
    assert!(baked.submesh(SubmeshKind::Hull).is_some(), "hull submesh present");
    assert!(baked.submesh(SubmeshKind::Turret).is_some(), "turret submesh present");
    let tris: usize = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
    assert!(
        (250..MEDIUM_LOD0_TRI_BUDGET).contains(&tris),
        "hybrid T-54 LOD0 {tris} tris outside [250, {MEDIUM_LOD0_TRI_BUDGET})"
    );
}

#[test]
fn the_barrel_is_keyed_to_the_installed_gun_module() {
    // The muzzle z tracks the GunModule's barrel length — geometry from the module, not a scale.
    let gun_z = t54_description()
        .build()
        .submesh(SubmeshKind::Gun)
        .expect("gun submesh")
        .mesh
        .bounds()
        .expect("non-empty")
        .max
        .z;
    assert!(
        (gun_z - MountFrames::for_vehicle(VehicleKind::T54_1951).muzzle.translation.z).abs()
            < 1.0e-4,
        "muzzle {gun_z:.2} matches its authoritative mount"
    );
}

#[test]
fn gun_submesh_uses_the_authoritative_mount_frames() {
    let baked = t54_description().build();
    let bounds = baked.submesh(SubmeshKind::Gun).unwrap().mesh.bounds().unwrap();
    let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
    assert!(bounds.min.y < mounts.gun_trunnion.translation.y);
    assert!(bounds.max.y > mounts.gun_trunnion.translation.y);
    assert!((bounds.max.z - mounts.muzzle.translation.z).abs() < 1.0e-4);
}

#[test]
fn moving_mantlet_is_part_of_the_gun_submesh() {
    let gun = t54_description().build().submesh(SubmeshKind::Gun).unwrap().mesh.clone();
    let cast: Vec<_> =
        gun.vertices().iter().filter(|v| v.material == MaterialRole::CastArmor).collect();
    assert!(!cast.is_empty(), "gun submesh needs a moving cast mantlet");
    let min_x = cast.iter().map(|v| v.position.x).fold(f32::INFINITY, f32::min);
    let max_x = cast.iter().map(|v| v.position.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = cast.iter().map(|v| v.position.y).fold(f32::INFINITY, f32::min);
    let max_y = cast.iter().map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
    assert!(max_x - min_x > max_y - min_y, "mantlet is a wide oval");
}

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

    // Support brackets hang below the fender at the hull side, on both fenders.
    let has_bracket = |sign: f32| {
        hull.vertices().iter().any(|v| {
            v.material == MaterialRole::TrackMetal
                && v.position.x * sign > 1.35
                && v.position.y > 0.80
                && v.position.y < 0.98
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
    let mantlet = revolve::moving_mantlet(trunnion, &v.gun).bounds().expect("mantlet");
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
    assert!(
        mantlet.max.z > front_z + 0.20,
        "the mantlet face must protrude to cover the embrasure opening (face {:.2}, front {front_z:.2})",
        mantlet.max.z
    );
}

#[test]
fn the_hybrid_t54_silhouette_reads_right() {
    // Locks the hybrid's visual reads against regression (cf. vehicle_geometry silhouette gates).
    let baked = t54_description().build();
    let turret = baked.submesh(SubmeshKind::Turret).unwrap().mesh.bounds().unwrap();
    assert!(
        (turret.max.x - turret.min.x) > (turret.max.y - turret.min.y),
        "turret reads as a flat cast dome (wider than tall)"
    );
    let hull = &baked.submesh(SubmeshKind::Hull).unwrap().mesh;
    let has = |m: MaterialRole| hull.vertices().iter().any(|v| v.material == m);
    assert!(has(MaterialRole::TrackMetal), "hull carries the static track belt, not a bare box");
    assert!(
        !has(MaterialRole::Rubber),
        "rubber road wheels are runtime-animated gear, not static hull geometry"
    );
    assert!(
        RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).is_some(),
        "runtime running gear supplies the animated rubber wheels"
    );
    let b = hull.bounds().unwrap();
    assert!((b.max.z - b.min.z) > (b.max.x - b.min.x), "hull reads longer than wide");
}

#[test]
fn the_hybrid_bake_is_deterministic() {
    // Same description, same bytes — every generator (CAD, SDF, revolve) is deterministic.
    assert_eq!(
        t54_description().build().deterministic_hash(),
        t54_description().build().deterministic_hash()
    );
}

#[test]
fn the_hybrid_reduces_through_lod_tiers_within_tiered_budgets() {
    // Per-LOD budgets (the plan's tiered budgets): each tier first drops the parts its policy
    // excludes (build_lod) then decimates (reduce_vehicle), landing under its cap monotonically.
    use vehicle_geometry::{BakedVehicle, LodLevel, reduce_vehicle};
    const LOD1_BUDGET: usize = 4_000;
    const LOD2_BUDGET: usize = 1_200;
    let d = t54_description();
    let total =
        |v: &BakedVehicle| v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum::<usize>();
    let lod0 = total(&d.build_lod(LodLevel::Lod0));
    let lod1 = total(&reduce_vehicle(&d.build_lod(LodLevel::Lod1), LodLevel::Lod1));
    let lod2 = total(&reduce_vehicle(&d.build_lod(LodLevel::Lod2), LodLevel::Lod2));
    assert!(lod0 > lod1 && lod1 > lod2, "tiers decimate monotonically: {lod0} {lod1} {lod2}");
    assert!(lod1 < LOD1_BUDGET, "LOD1 {lod1} within tier budget {LOD1_BUDGET}");
    assert!(lod2 < LOD2_BUDGET, "LOD2 {lod2} within tier budget {LOD2_BUDGET}");
}

#[test]
fn higher_lods_drop_detail_parts_but_keep_silhouette_and_mounts() {
    // Per-part LOD policy: LOD1 drops the detail fittings and track links (so it has fewer raw
    // triangles than LOD0 before any decimation), while the turret and gun mount parts survive.
    use vehicle_geometry::{BakedVehicle, LodLevel};
    let d = t54_description();
    let total =
        |v: &BakedVehicle| v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum::<usize>();
    let lod0 = d.build_lod(LodLevel::Lod0);
    let lod1 = d.build_lod(LodLevel::Lod1);
    assert!(total(&lod1) < total(&lod0), "LOD1 drops detail parts before decimation");
    assert!(
        lod1.submesh(SubmeshKind::Turret).is_some() && lod1.submesh(SubmeshKind::Gun).is_some(),
        "mount-bearing turret and gun survive into LOD1"
    );
}

#[test]
fn swapping_the_gun_module_changes_the_barrel_geometry() {
    // Visual modularity: a different gun module rebuilds a different barrel, end to end.
    let kind = VehicleKind::T54_1951;
    let muzzle_z = |gun: game_core::GunModule| {
        let mut loadout = kind.default_loadout();
        loadout.gun = gun;
        t54_from_modules(&loadout)
            .build()
            .submesh(SubmeshKind::Gun)
            .unwrap()
            .mesh
            .bounds()
            .unwrap()
            .max
            .z
    };
    let opts = kind.gun_options();
    let short = opts
        .iter()
        .cloned()
        .reduce(|a, b| if a.barrel_length_m() <= b.barrel_length_m() { a } else { b })
        .unwrap();
    let long = opts
        .iter()
        .cloned()
        .reduce(|a, b| if a.barrel_length_m() >= b.barrel_length_m() { a } else { b })
        .unwrap();
    assert!(long.barrel_length_m() > short.barrel_length_m(), "the T-54 has two gun lengths");
    assert!(muzzle_z(long) > muzzle_z(short), "the longer gun module makes a longer barrel");
}
