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
pub const MEDIUM_LOD0_TRI_BUDGET: usize = 22_000;

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
        shape: PartShape::Mesh(revolve::t54_running_gear(&v.running_gear, v.track_belt.axle_y)),
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

    let f = &v.fittings;
    let mut parts =
        vec![lower_tub, upper_hull, gear, tracks, track_ends, track_links, turret, barrel];
    // Semantic drum fittings as their own parts (not anonymous greeble): the commander's cupola
    // hatch and the driver's/loader's hatches (all raised round lids), plus the glacis headlight.
    parts.extend(crate::t54_details::t54_fitting_parts(f));
    // The engine deck reads as bolted panels, not one slab — its split plates carry the silhouette.
    for panel in solid::t54_engine_deck_panels(&v.deck) {
        parts.push(VehiclePart {
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(panel),
            lod: PartLod::Silhouette,
        });
    }
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
    // Clean factory greeble (grille, exhaust cover, periscopes, fender lips, weld bead) — all at the
    // Detail tier, so the close-up LOD0 carries it and the lower LODs keep only the silhouette.
    parts.extend(crate::t54_details::t54_detail_parts(v));
    // Swing-arm brackets mounting each road wheel to the hull's lower tub side (suspension cue).
    parts.extend(crate::t54_details::t54_suspension_parts(v, bp.hull.lower_half_width));

    VehicleDescription { kind, parts, mounts }
}
