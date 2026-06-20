//! The T-54 as a hybrid parametric description: hull front from exact CAD plates, cast turret from
//! the SDF. The single dimension that drives the visible glacis is the same one the armour model
//! reads — locked by a test so "what you see is what you shoot" holds by construction.

use game_core::{MountFrames, VehicleKind, VehicleModules};
use glam::Vec3;
use solid::{ConvexSolid, Plane};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, SubmeshKind};

use crate::description::VehicleDescription;
use crate::part::{PartShape, VehiclePart};

/// Converts an optional gun module's length delta into the spike's visual scale.
const MODULE_BARREL_DELTA_SCALE: f32 = 0.65;

/// LOD0 triangle budget for a detail-tier medium tank — a deliberate per-class budget that replaces
/// the spike's tight micro-cap. The fully-detailed hybrid T-54 (multi-slope hull, running gear with
/// hubs, tracks, cast turret, barrel, fenders, deck) lands ~11.6k; the headroom leaves room for
/// loadout variants and detail without drifting toward HD-era counts. Tier per LOD/class later.
pub const MEDIUM_LOD0_TRI_BUDGET: usize = 14_000;

/// The hull as a convex solid whose every plate normal is sloped to the blueprint armour angle in
/// the `game_core::armor` convention (degrees of normal above horizontal): glacis (hull_front),
/// sloped sides (hull_side, top narrower), sloped rear (hull_rear), plus a lower nose bevel. So the
/// visible rake of *every* facet matches the angle the penetration model uses for it.
fn t54_hull_solid() -> ConvexSolid {
    let bp =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let (front, side, rear) = (
        bp.armor.hull_front.0.to_radians(),
        bp.armor.hull_side.0.to_radians(),
        bp.armor.hull_rear.0.to_radians(),
    );
    let (hx, belly, roof, hz) = (1.45_f32, 0.10_f32, 1.20_f32, 2.90_f32);
    let side_off = hx * side.cos() + belly * side.sin();
    ConvexSolid::new(vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -belly),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), roof),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), belly * rear.sin() + hz * rear.cos()),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), 1.85),
        Plane::new(Vec3::new(0.0, -0.5, 1.0), 2.55),
    ])
}

fn t54_moving_mantlet(trunnion: Vec3) -> GeometryMesh {
    let profile = [(-0.18, 0.22), (0.0, 0.34), (0.18, 0.24)];
    let base = revolve::revolve(Vec3::Z, &profile, 20, MaterialRole::CastArmor, SmoothingGroup(2));
    let vertices = base
        .vertices()
        .iter()
        .map(|v| GeometryVertex {
            position: trunnion + Vec3::new(v.position.x * 1.45, v.position.y * 0.72, v.position.z),
            normal: Vec3::new(v.normal.x / 1.45, v.normal.y / 0.72, v.normal.z).normalize_or_zero(),
            ..*v
        })
        .collect();
    GeometryMesh::new(vertices, base.indices().to_vec()).weld_and_smooth()
}

/// Build the hybrid T-54 from the stock loadout (CAD hull plates + SDF cast turret + revolved parts).
pub fn t54_description() -> VehicleDescription {
    t54_from_modules(&VehicleKind::T54_1951.default_loadout())
}

/// Build the hybrid T-54 from an explicit module loadout. The installed gun drives the barrel
/// geometry, so swapping the gun rebuilds the barrel — visual modularity, not a post-bake scale.
pub fn t54_from_modules(modules: &VehicleModules) -> VehicleDescription {
    let kind = VehicleKind::T54_1951;

    let hull = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(t54_hull_solid()),
    };

    let gear = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::Rubber,
        smoothing: SmoothingGroup(5),
        shape: PartShape::Mesh(revolve::t54_running_gear()),
    };

    let tracks = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_tracks()),
    };

    let (turret_sdf, min, max) = sdf_mesh::t54_turret();
    let turret = VehiclePart {
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Cast { sdf: turret_sdf, min, max, budget: 9_000 },
    };

    // Barrel geometry is driven by the installed gun module — not a post-bake scale of a fixed mesh
    // (the old `barrel_scale` hack). Swap the gun and the barrel is rebuilt at the module's length.
    let mounts = MountFrames::for_vehicle(kind);
    let stock_length = kind.default_loadout().gun.barrel_length_m();
    let muzzle = mounts.muzzle.translation
        + Vec3::Z * ((modules.gun.barrel_length_m() - stock_length) * MODULE_BARREL_DELTA_SCALE);
    let trunnion = mounts.gun_trunnion.translation;
    let barrel = VehiclePart {
        submesh: SubmeshKind::Gun,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::merge(&[
            t54_moving_mantlet(trunnion),
            revolve::gun_barrel_between(trunnion, muzzle),
        ])),
    };

    let deck = VehiclePart {
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::t54_engine_deck()),
    };

    let mut parts = vec![hull, gear, tracks, turret, barrel, deck];
    for side in [1.5_f32, -1.5] {
        parts.push(VehiclePart {
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(solid::t54_fender(side)),
        });
    }

    VehicleDescription { kind, parts, mounts }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glacis_geometry_slope_matches_the_armour_blueprint() {
        // The CAD glacis plate is built from `solid::GLACIS_SLOPE_DEG`; the armour facet reads its
        // slope from the blueprint. They must be the same number — coherence by construction.
        let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
            .expect("T-54 has a blueprint");
        assert!(
            (solid::GLACIS_SLOPE_DEG - bp.armor.hull_front.0).abs() < 1.0e-3,
            "visible glacis {} deg vs armour facet {} deg",
            solid::GLACIS_SLOPE_DEG,
            bp.armor.hull_front.0
        );

        // And the *built geometry* carries that angle in the armour convention: a glacis face normal
        // sits `slope` degrees above horizontal (atan2(n.y, n.z)) — what you see is what you shoot.
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
        // Per-LOD budgets (the plan's tiered budgets): the hybrid flows through the existing
        // reduce_vehicle pipeline and each tier lands under its cap, decimating monotonically.
        use vehicle_geometry::{BakedVehicle, LodLevel, reduce_vehicle};
        const LOD1_BUDGET: usize = 4_000;
        const LOD2_BUDGET: usize = 1_200;
        let baked = t54_description().build();
        let total =
            |v: &BakedVehicle| v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum::<usize>();
        let lod0 = total(&baked);
        let lod1 = total(&reduce_vehicle(&baked, LodLevel::Lod1));
        let lod2 = total(&reduce_vehicle(&baked, LodLevel::Lod2));
        assert!(lod0 > lod1 && lod1 > lod2, "tiers decimate monotonically: {lod0} {lod1} {lod2}");
        assert!(lod1 < LOD1_BUDGET, "LOD1 {lod1} within tier budget {LOD1_BUDGET}");
        assert!(lod2 < LOD2_BUDGET, "LOD2 {lod2} within tier budget {LOD2_BUDGET}");
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
