//! The T-54 as a hybrid parametric description: hull front from exact CAD plates, cast turret from
//! the SDF, round parts revolved. This module is now an *adapter*: every dimension comes from the
//! vehicle blueprint's [`HybridVisual`](game_core::HybridVisual) and the installed module loadout —
//! it holds no geometry constants of its own. The single dimension that drives the visible glacis is
//! the same armour facet the penetration model reads, so "what you see is what you shoot" by
//! construction.

use game_core::{VehicleBlueprint, VehicleKind, VehicleModules};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::description::VehicleDescription;
use crate::part::{PartLod, PartShape, VehiclePart};

/// LOD0 triangle budget for a detail-tier medium tank — a deliberate per-class budget that replaces
/// the spike's tight micro-cap. The fully-detailed hybrid T-54 (multi-slope hull, running gear with
/// hubs, tracks, cast turret, barrel, fenders, deck) lands ~11.6k; the headroom leaves room for
/// loadout variants and detail without drifting toward HD-era counts. Tier per LOD/class later.
pub const MEDIUM_LOD0_TRI_BUDGET: usize = 14_000;

/// Build the hybrid T-54 from the stock loadout (CAD hull plates + SDF cast turret + revolved parts).
pub fn t54_description() -> VehicleDescription {
    t54_from_modules(&VehicleKind::T54_1951.default_loadout())
}

/// Build the hybrid T-54 from an explicit module loadout. The installed gun drives the barrel
/// geometry, so swapping the gun rebuilds the barrel — visual modularity, not a post-bake scale.
/// All shape dimensions are read from the blueprint; only the gun length comes from the loadout.
pub fn t54_from_modules(modules: &VehicleModules) -> VehicleDescription {
    let kind = VehicleKind::T54_1951;
    let bp = VehicleBlueprint::for_vehicle(kind).expect("T-54 blueprint");
    let v = bp.hybrid().expect("T-54 carries hybrid visual data");

    // The hull is decomposed into its real T-54 plates: a narrow lower tub and the wide upper hull
    // that overhangs it as the sponson. The two-plate front (upper glacis over the tucked nose) and
    // the sloped sides/rear each carry their blueprint armour angle.
    let lower_tub = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::t54_lower_tub(
            &bp.hull,
            &v.hull_plates,
            bp.armor.hull_rear.0,
        )),
        lod: PartLod::Silhouette,
    };
    let upper_hull = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::t54_upper_hull(
            &bp.hull,
            &v.hull_plates,
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
        )),
        lod: PartLod::Silhouette,
    };

    let gear = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::Rubber,
        smoothing: SmoothingGroup(5),
        shape: PartShape::Mesh(revolve::t54_running_gear(&v.running_gear)),
        lod: PartLod::Silhouette,
    };

    let tracks = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_tracks(&v.track_belt)),
        lod: PartLod::Silhouette,
    };

    // The track ends read as distinct mechanisms: a smooth front idler and a faceted rear sprocket.
    let track_ends = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_track_ends(&v.running_gear, &v.track_belt)),
        lod: PartLod::Detail,
    };

    // Link cues along the ground run so the belt reads as tracked links, not a smooth band.
    let track_links = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_track_link_cues(&v.track_belt)),
        lod: PartLod::Detail,
    };

    let (turret_sdf, min, max) = sdf_mesh::t54_turret(&v.turret);
    let turret = VehiclePart {
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Cast { sdf: turret_sdf, min, max, budget: v.turret.budget },
        lod: PartLod::MountCritical,
    };

    // Barrel geometry is driven by the installed gun module — not a post-bake scale of a fixed mesh
    // (the old `barrel_scale` hack). Swap the gun and the barrel is rebuilt at the module's length.
    let mounts = bp.mount_frames();
    let stock_length = kind.default_loadout().gun.barrel_length_m();
    let muzzle = mounts.muzzle.translation
        + Vec3::Z * ((modules.gun.barrel_length_m() - stock_length) * v.gun.module_delta_scale);
    let trunnion = mounts.gun_trunnion.translation;
    let barrel = VehiclePart {
        submesh: SubmeshKind::Gun,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::merge(&[
            revolve::moving_mantlet(trunnion, &v.gun),
            revolve::gun_barrel_between(trunnion, muzzle, &v.gun),
        ])),
        lod: PartLod::MountCritical,
    };

    let deck = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::t54_engine_deck(&v.deck)),
        lod: PartLod::Silhouette,
    };

    // Semantic fittings. The cupola hatch rides the turret (so it traverses); the headlight and the
    // front tow hooks ride the hull. Each is its own part, not anonymous greeble.
    let f = &v.fittings;
    let cupola_hatch = VehiclePart {
        submesh: SubmeshKind::Turret,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            f.cupola_hatch_center,
            f.cupola_hatch_radius,
            f.cupola_hatch_half_height,
            16,
            MaterialRole::RolledArmor,
            SmoothingGroup(2),
        )),
        lod: PartLod::Detail,
    };
    let headlight = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            f.headlight_center,
            f.headlight_radius,
            f.headlight_half_height,
            12,
            MaterialRole::RolledArmor,
            SmoothingGroup(2),
        )),
        lod: PartLod::Detail,
    };

    let mut parts = vec![
        lower_tub,
        upper_hull,
        gear,
        tracks,
        track_ends,
        track_links,
        turret,
        barrel,
        deck,
        cupola_hatch,
        headlight,
    ];
    for side in [f.tow_hook_center.x, -f.tow_hook_center.x] {
        let center = Vec3::new(side, f.tow_hook_center.y, f.tow_hook_center.z);
        parts.push(VehiclePart {
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(solid::ConvexSolid::box_at(center, f.tow_hook_half)),
            lod: PartLod::Detail,
        });
    }
    for side in [v.fender.side_x, -v.fender.side_x] {
        parts.push(VehiclePart {
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(solid::t54_fender(side, &v.fender)),
            lod: PartLod::Detail,
        });
    }

    VehicleDescription { kind, parts, mounts }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::MountFrames;

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
            n.y > 0.2
                && n.z > 0.2
                && (n.y.atan2(n.z).to_degrees() - bp.armor.hull_front.0).abs() < 2.0
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
        let glacis = ns
            .iter()
            .any(|n| n.z > 0.2 && near(n.y.atan2(n.z).to_degrees(), bp.armor.hull_front.0));
        let side =
            ns.iter().any(|n| n.x > 0.5 && near(n.y.atan2(n.x).to_degrees(), bp.armor.hull_side.0));
        let rear = ns
            .iter()
            .any(|n| n.z < -0.5 && near(n.y.atan2(-n.z).to_degrees(), bp.armor.hull_rear.0));
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
        assert!(
            has(MaterialRole::Rubber) && has(MaterialRole::TrackMetal),
            "hull carries running gear (tyres + track), not a bare box"
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
}
