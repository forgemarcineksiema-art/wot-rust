//! The T-54 as a hybrid parametric description: hull front from exact CAD plates, cast turret from
//! the SDF, round parts revolved. This module is now an *adapter*: every dimension comes from the
//! vehicle blueprint's [`VisualDetail`](game_core::VisualDetail) and the installed module loadout —
//! it holds no geometry constants of its own. The single dimension that drives the visible glacis is
//! the same armour facet the penetration model reads, so "what you see is what you shoot" by
//! construction.

use game_core::roundness::round_segments;
use game_core::{VehicleBlueprint, VehicleKind, VehicleModules};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::description::VehicleDescription;
use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// LOD0 triangle budget for a detail-tier medium tank — a deliberate per-class budget that replaces
/// the spike's tight micro-cap.
///
/// Raised 22,000 -> 26,000 (measured 2026-08-08, the Wave 1 construction pass). The hybrid moves
/// from 21,508 to 24,069: the periscopes became devices, every stowage bin and fuel tank got the
/// lid seam, hinge and latch that make a box a bin, and the exhaust stopped being a toolbox and
/// got louvres and a dark outlet. 26,000 gives the measured shape 8% headroom.
///
/// Affordable, and the frame says so rather than taste. Every part added here is
/// `PartLod::Detail`, so it exists at LOD0 and is DROPPED at LOD1 and below — the cost lands only
/// on vehicles close enough to read it. At `scene_pass`'s measured ~20 ns/triangle
/// (measured 2026-08-08 on the MX330 min spec, at the shipped 1x MSAA) the worst case is a 7v7
/// where every tank is near: +2,561 triangles each, ~0.72 ms against 2.99 ms of p95 headroom.
/// A realistic battle has one or two vehicles at that range, not fourteen.
pub const MEDIUM_LOD0_TRI_BUDGET: usize = 26_000;

/// Build the hybrid T-54 from the stock loadout (CAD hull plates + SDF cast turret + revolved parts).
pub fn t54_description() -> VehicleDescription {
    t54_from_modules(&VehicleKind::T54_1951.default_loadout())
}

/// Build the hybrid T-54 from an explicit, possibly LIVE blueprint (a Studio `--blueprint-file`
/// override) at the stock loadout. The same assembly the authoritative bake uses — the fast
/// loop must never show an author a mesh the game does not ship.
pub fn t54_description_from_blueprint(bp: &VehicleBlueprint) -> VehicleDescription {
    t54_from_modules_with_blueprint(&VehicleKind::T54_1951.default_loadout(), bp)
}

/// Build the hybrid T-54 from an explicit module loadout. The installed gun drives the barrel
/// geometry, so swapping the gun rebuilds the barrel — visual modularity, not a post-bake scale.
/// All shape dimensions are read from the blueprint; only the gun length comes from the loadout.
pub fn t54_from_modules(modules: &VehicleModules) -> VehicleDescription {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    t54_from_modules_with_blueprint(modules, &bp)
}

/// The fully-explicit assembly: loadout + blueprint both injected. Every other constructor is a
/// convenience wrapper over this one, so the embedded and live paths cannot drift apart.
pub fn t54_from_modules_with_blueprint(
    modules: &VehicleModules,
    bp: &VehicleBlueprint,
) -> VehicleDescription {
    let kind = VehicleKind::T54_1951;
    let v = bp.complete_visual().expect("T-54 carries hybrid visual data");

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
            v.hull_plates,
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
            v.hull_plates,
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
        )),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Solid,
    };

    // The cast turret is a LOFTED shell (replacing the metaball SDF composition): a controlled,
    // designed surface that reads as one casting from every angle. The cupola is no longer blended
    // into the shell, so it rides as its own cast drum part on the roof.
    let turret = VehiclePart {
        key: PartKey::new("turret_shell"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Mesh(crate::t54_turret_loft::t54_turret_loft(v.turret_loft)),
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
            round_segments(v.turret_loft.cupola_radius),
            MaterialRole::CastArmor,
            SmoothingGroup(2),
        )),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Revolve,
    };

    // Barrel geometry is driven by the installed gun module — not a post-bake scale of a fixed mesh
    // (the old `barrel_scale` hack). Swap the gun and the barrel is rebuilt at the module's length.
    let mut mounts = bp.mount_frames();
    let stock_length = kind.default_loadout().gun.barrel_length_m();
    let muzzle = mounts.muzzle.translation
        + Vec3::Z * ((modules.gun.barrel_length_m() - stock_length) * v.gun.module_delta_scale);
    mounts.muzzle.translation = muzzle;
    let trunnion = mounts.gun_trunnion.translation;
    // The mantlet is its own CAST part, distinct from the steel barrel — merging them under one
    // material made the mount read as bare gun steel. On this vehicle it sits INSIDE the turret:
    // what the outside world sees of the gun mount is the aperture, the canvas boot and the tube.
    let mantlet = VehiclePart {
        key: PartKey::new("gun_mantlet"),
        submesh: SubmeshKind::Gun,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
        shape: PartShape::Mesh(revolve::moving_mantlet(trunnion, v.gun)),
        lod: PartLod::MountCritical,
        generator: GeneratorKind::Revolve,
    };
    // The canvas cover over the gun window. It moves with the gun, so it belongs to the same
    // submesh as the barrel and mantlet, and it is neither armour nor steel.
    let mantlet_cover = VehiclePart {
        key: PartKey::new("gun_mantlet_cover"),
        submesh: SubmeshKind::Gun,
        material: MaterialRole::Canvas,
        smoothing: SmoothingGroup(6),
        shape: PartShape::Mesh(crate::t54_gun_cover::t54_mantlet_cover(
            trunnion,
            v.gun.canvas.as_ref().expect("the benchmark wears its canvas"),
        )),
        lod: PartLod::MountCritical,
        generator: GeneratorKind::Sweep,
    };
    // The fastening strip round the window's perimeter: bolted to the CASTING, so it rides the
    // turret, not the gun — the fabric moves under it.
    let mantlet_frame = VehiclePart {
        key: PartKey::new("gun_mantlet_frame"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(3),
        shape: PartShape::Mesh(crate::t54_gun_cover::t54_mantlet_frame(
            trunnion,
            v.gun.canvas.as_ref().expect("the benchmark wears its canvas"),
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    };
    let barrel = VehiclePart {
        key: PartKey::new("gun_barrel"),
        submesh: SubmeshKind::Gun,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::gun_barrel_between(trunnion, muzzle, v.gun)),
        lod: PartLod::MountCritical,
        generator: GeneratorKind::Revolve,
    };

    let f = &v.fittings;
    let mut parts =
        vec![lower_tub, upper_hull, turret, cupola, mantlet, mantlet_cover, mantlet_frame, barrel];
    // Semantic drum fittings as their own parts (not anonymous greeble): the commander's cupola
    // hatch and the driver's/loader's hatches (all raised round lids), plus the glacis headlight.
    parts.extend(crate::t54_details::t54_fitting_parts(f));
    // The engine deck reads as bolted panels, not one slab — its split plates carry the silhouette.
    let deck_top = v.deck.center.y + v.deck.half.y;
    for (i, panel) in solid::t54_engine_deck_panels(v.deck).into_iter().enumerate() {
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
    // The bolts that hold those panels down. `detail::bolt_head` has existed since the detail
    // kernel was written and had never been called: the deck read as split plates with nothing
    // holding them, which is the one thing a bolted panel is FOR. Two rows per panel, along the
    // seams, because that is where a fastener goes.
    parts.extend(crate::t54_details::t54_deck_panel_bolts(v.deck, deck_top));
    // A tow hook is a HOOK: a bracket welded to the nose plate, a curved throat a shackle drops
    // into, and a catch across the mouth so it cannot drop back out. These were 240 x 220 x 200
    // bricks — a cast block with no curvature, no throat and no catch, which is a boss, not a
    // hook (register K9).
    for (i, side) in [f.tow_hook_center.x, -f.tow_hook_center.x].into_iter().enumerate() {
        let center = Vec3::new(side, f.tow_hook_center.y, f.tow_hook_center.z);
        parts.extend(crate::t54_details::t54_tow_hook(i as u16, center, f.tow_hook_half));
    }
    // Fenders split into bolted sections along each run, rather than one continuous slab — and each
    // section is one folded pressing, plate and return lip together, so a seam runs to the fold.
    parts.extend(crate::t54_fender::t54_fender_parts(v.fender, v.detail));
    // Clean factory greeble (grille, exhaust cover, periscopes, fender lips, weld bead) — all at the
    // Detail tier, so the close-up LOD0 carries it and the lower LODs keep only the silhouette.
    parts.extend(crate::t54_details::t54_detail_parts(v));
    // The external kit: fender stowage and sloping fender ends, the glacis splash board, turret
    // handrails and stowed tow cables — the reference dressing that makes the narrow hull read.
    parts.extend(crate::t54_kit::t54_kit_parts(v, bp.armor.hull_front.0));
    // Hull plate articulation: the glacis-to-roof weld seam and the rear transmission covers.
    parts.extend(crate::t54_chassis::t54_hull_plate_parts(v, bp.armor.hull_front.0));
    // Damage-ready interior: major visible assemblies are generated from the authoritative
    // DamageLayout, with close-view mechanical detail layered around those exact transforms.
    parts.extend(crate::t54_interior::t54_interior_parts());
    // The turret-ring collar: a short cast drum sealing the slot between the dome's
    // overhanging skirt and the hull roof. Without it the seat is OPEN air — from a low bow
    // angle the rasterizer culls the skirt's underside and you look straight into the lit
    // fighting compartment (the "plate with three holes": primer wall + D-10 round noses;
    // model-logic audit #10). Turret group, so the seal holds at every traverse angle.
    parts.push(VehiclePart {
        key: PartKey::new("turret_ring_collar"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            // From the deck plane UP into the skirt only: dipping below ring_plane_y would
            // inflate the turret submesh's height and with it the silhouette ratio gate.
            Vec3::new(0.0, v.turret.ring_plane_y + 0.07, 0.0),
            // Wide enough to fill the whole deck-aperture annulus under the skirt: with the
            // narrow 0.93 collar a ring of open sky remained between it and the aperture
            // edge, and a low bow camera looked past the dome silhouette straight into the
            // lit basket (the "plate": primer wall + D-10 noses through the slot).
            v.turret.ring_radius + 0.13,
            0.07,
            28,
            MaterialRole::CastArmor,
            SmoothingGroup::hard_edges(),
        )),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Revolve,
    });

    // Bake-time ambient contact: darken the turret-ring seam, mantlet seat, running-gear recess and
    // glacis weld into `surface_shade` after merge (the cast turret and welded hull no longer read
    // flat). Derived from the same blueprint `v` that drives the geometry.
    let surface_bake = crate::surface_bake::t54_surface_bake(v, &bp.track, muzzle);
    VehicleDescription { kind, parts, mounts, surface_bake }
}
