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
use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

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
        key: PartKey::new("lower_tub"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::t54_lower_tub(
            &bp.hull,
            &v.hull_plates,
            bp.armor.hull_rear.0,
        )),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Solid,
    };
    let upper_hull = VehiclePart {
        key: PartKey::new("upper_hull"),
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
        generator: GeneratorKind::Solid,
    };

    let gear = VehiclePart {
        key: PartKey::new("running_gear"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::Rubber,
        smoothing: SmoothingGroup(5),
        shape: PartShape::Mesh(revolve::t54_running_gear(&v.running_gear, v.track_belt.axle_y)),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Revolve,
    };

    let tracks = VehiclePart {
        key: PartKey::new("tracks"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_tracks(&v.track_belt)),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Sweep,
    };

    // The track ends read as distinct mechanisms: a smooth front idler and a faceted rear sprocket.
    let track_ends = VehiclePart {
        key: PartKey::new("track_ends"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_track_ends(&v.running_gear, &v.track_belt)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    };

    // Link cues along the ground run so the belt reads as tracked links, not a smooth band.
    let track_links = VehiclePart {
        key: PartKey::new("track_links"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::t54_track_link_cues(&v.track_belt)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    };

    // The cast turret is a LOFTED shell (replacing the metaball SDF composition): a controlled,
    // designed surface that reads as one casting from every angle. The cupola is no longer blended
    // into the shell, so it rides as its own cast drum part on the roof.
    let turret = VehiclePart {
        key: PartKey::new("turret_shell"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Mesh(crate::t54_turret_loft::t54_turret_loft(&v.turret_loft)),
        lod: PartLod::MountCritical,
        generator: GeneratorKind::CastLoft,
    };
    let cupola = VehiclePart {
        key: PartKey::new("cupola"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Mesh(revolve::drum(
            v.turret_loft.cupola_center,
            v.turret_loft.cupola_radius,
            v.turret_loft.cupola_half_height,
            20,
            MaterialRole::CastArmor,
            SmoothingGroup(2),
        )),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Revolve,
    };

    // Barrel geometry is driven by the installed gun module — not a post-bake scale of a fixed mesh
    // (the old `barrel_scale` hack). Swap the gun and the barrel is rebuilt at the module's length.
    let mounts = bp.mount_frames();
    let stock_length = kind.default_loadout().gun.barrel_length_m();
    let muzzle = mounts.muzzle.translation
        + Vec3::Z * ((modules.gun.barrel_length_m() - stock_length) * v.gun.module_delta_scale);
    let trunnion = mounts.gun_trunnion.translation;
    let barrel = VehiclePart {
        key: PartKey::new("gun_barrel"),
        submesh: SubmeshKind::Gun,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::merge(&[
            revolve::moving_mantlet(trunnion, &v.gun),
            revolve::gun_barrel_between(trunnion, muzzle, &v.gun),
        ])),
        lod: PartLod::MountCritical,
        generator: GeneratorKind::Revolve,
    };

    let f = &v.fittings;
    let mut parts =
        vec![lower_tub, upper_hull, gear, tracks, track_ends, track_links, turret, cupola, barrel];
    // Semantic drum fittings as their own parts (not anonymous greeble): the commander's cupola
    // hatch and the driver's/loader's hatches (all raised round lids), plus the glacis headlight.
    parts.extend(crate::t54_details::t54_fitting_parts(f));
    // The engine deck reads as bolted panels, not one slab — its split plates carry the silhouette.
    for (i, panel) in solid::t54_engine_deck_panels(&v.deck).into_iter().enumerate() {
        parts.push(VehiclePart {
            key: PartKey::indexed("engine_deck_panel", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(panel),
            lod: PartLod::Silhouette,
            generator: GeneratorKind::Solid,
        });
    }
    for (i, side) in [f.tow_hook_center.x, -f.tow_hook_center.x].into_iter().enumerate() {
        let center = Vec3::new(side, f.tow_hook_center.y, f.tow_hook_center.z);
        parts.push(VehiclePart {
            key: PartKey::indexed("tow_hook", i as u16),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Plates(solid::ConvexSolid::box_at(center, f.tow_hook_half)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Solid,
        });
    }
    // Fenders split into bolted sections along each run, rather than one continuous slab.
    let mut fender_segment = 0u16;
    for side in [v.fender.side_x, -v.fender.side_x] {
        for segment in solid::t54_fender_segments(side, &v.fender) {
            parts.push(VehiclePart {
                key: PartKey::indexed("fender_segment", fender_segment),
                submesh: SubmeshKind::Hull,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
                shape: PartShape::Plates(segment),
                lod: PartLod::Detail,
                generator: GeneratorKind::Solid,
            });
            fender_segment += 1;
        }
    }
    // Clean factory greeble (grille, exhaust cover, periscopes, fender lips, weld bead) — all at the
    // Detail tier, so the close-up LOD0 carries it and the lower LODs keep only the silhouette.
    parts.extend(crate::t54_details::t54_detail_parts(v));
    // Swing-arm brackets mounting each road wheel to the hull's lower tub side (suspension cue).
    parts.extend(crate::t54_chassis::t54_suspension_parts(v, bp.hull.lower_half_width));
    // Hull plate articulation: the glacis-to-roof weld seam and the rear transmission covers.
    parts.extend(crate::t54_chassis::t54_hull_plate_parts(v, bp.armor.hull_front.0));

    VehicleDescription { kind, parts, mounts }
}
