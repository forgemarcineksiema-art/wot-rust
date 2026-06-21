//! Visual-only factory detailing for the hybrid T-54, assembled as `PartLod::Detail` parts so it
//! appears only at the close-up LOD0 and is dropped from LOD1/LOD2 (which keep the silhouette,
//! mount-critical parts and a readable track band). Clean factory build: crisp manufactured greeble
//! — an engine-deck grille, the exhaust cover, turret periscopes, fender lips and a restrained
//! glacis/deck weld bead — and deliberately no mud, rust, battle damage, decals or weathering. Every
//! piece reads its dimensions from the blueprint's [`HybridVisual`]; none invents a tank dimension.

use game_core::{FittingsVisual, GunVisual, HybridVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{PartLod, PartShape, VehiclePart};

/// A raised round lid/drum fitting (hatch lid or headlight), as its own `Detail` part.
fn drum_fitting(submesh: SubmeshKind, center: Vec3, radius: f32, half_height: f32) -> VehiclePart {
    VehiclePart {
        submesh,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            center,
            radius,
            half_height,
            16,
            MaterialRole::RolledArmor,
            SmoothingGroup(2),
        )),
        lod: PartLod::Detail,
    }
}

/// The semantic drum fittings: the commander's cupola hatch and the loader's hatch ride the turret
/// (so they traverse); the driver's hatch and the glacis headlight ride the hull. Each is its own
/// part, not anonymous greeble.
pub fn t54_fitting_parts(f: &FittingsVisual) -> Vec<VehiclePart> {
    vec![
        drum_fitting(
            SubmeshKind::Turret,
            f.cupola_hatch_center,
            f.cupola_hatch_radius,
            f.cupola_hatch_half_height,
        ),
        drum_fitting(
            SubmeshKind::Hull,
            f.driver_hatch_center,
            f.driver_hatch_radius,
            f.driver_hatch_half_height,
        ),
        drum_fitting(
            SubmeshKind::Turret,
            f.loader_hatch_center,
            f.loader_hatch_radius,
            f.loader_hatch_half_height,
        ),
        drum_fitting(
            SubmeshKind::Hull,
            f.headlight_center,
            f.headlight_radius,
            f.headlight_half_height,
        ),
    ]
}

fn detail_plate(
    submesh: SubmeshKind,
    material: MaterialRole,
    solid: solid::ConvexSolid,
) -> VehiclePart {
    VehiclePart {
        submesh,
        material,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid),
        lod: PartLod::Detail,
    }
}

/// Every factory detail part for the T-54, all at `PartLod::Detail`.
pub fn t54_detail_parts(v: &HybridVisual) -> Vec<VehiclePart> {
    let d = &v.detail;
    let mut parts = Vec::new();

    // Engine-deck grille (frame + slats) and the left-fender exhaust cover ride the hull.
    for solid in solid::t54_deck_grille(d) {
        parts.push(detail_plate(SubmeshKind::Hull, MaterialRole::TrackMetal, solid));
    }
    parts.push(detail_plate(
        SubmeshKind::Hull,
        MaterialRole::TrackMetal,
        solid::t54_exhaust_housing(d),
    ));

    // Fender lips on both outer fender edges, plus the support brackets hanging below each fender.
    for side in [v.fender.side_x, -v.fender.side_x] {
        parts.push(detail_plate(
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::t54_fender_lip(side, &v.fender, d),
        ));
        for bracket in solid::t54_fender_brackets(side, &v.fender, v.hull.half_width) {
            parts.push(detail_plate(SubmeshKind::Hull, MaterialRole::TrackMetal, bracket));
        }
    }

    // Turret-roof periscopes (gunner + loader side), riding the turret so they traverse with it.
    // Each is a raked prism head (forward-looking glass), not a plain block.
    for side in [d.periscope_center.x, -d.periscope_center.x] {
        let center = Vec3::new(side, d.periscope_center.y, d.periscope_center.z);
        parts.push(detail_plate(
            SubmeshKind::Turret,
            MaterialRole::RolledArmor,
            solid::t54_periscope(center, d.periscope_half),
        ));
    }

    // The driver's two forward vision periscopes on the hull roof, flanking and just ahead of the
    // driver's hatch. Derived from the hatch position (no new blueprint dimension) and clear of the
    // hatch lid; same raked prism head as the turret periscopes.
    let dh = v.fittings.driver_hatch_center;
    let driver_peri_half = Vec3::new(0.055, 0.05, 0.05);
    for dx in [-0.26_f32, 0.26] {
        let center = Vec3::new(dh.x + dx, dh.y, dh.z + 0.08);
        parts.push(detail_plate(
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::t54_periscope(center, driver_peri_half),
        ));
    }

    // Loader-side DShK: a compact pedestal and a distinct steel barrel mounted on the turret roof.
    // It is a real turret part so it traverses with the vehicle; it is not stowage or a texture cue.
    parts.push(VehiclePart {
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            d.dshk_mount_center,
            0.065,
            0.10,
            10,
            MaterialRole::TrackMetal,
            SmoothingGroup::hard_edges(),
        )),
        lod: PartLod::Detail,
    });
    let dshk = GunVisual {
        barrel_radius: 0.025,
        muzzle_radius: 0.030,
        muzzle_taper: 0.05,
        barrel_segments: 10,
        ..v.gun
    };
    parts.push(VehiclePart {
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::gun_barrel_between(
            d.dshk_mount_center,
            d.dshk_mount_center + Vec3::Z * d.dshk_barrel_length,
            &dshk,
        )),
        lod: PartLod::Detail,
    });

    // A restrained weld bead along the front edge of the engine deck (a crisp cast/plate seam).
    let bead_center =
        Vec3::new(0.0, v.deck.center.y + v.deck.half.y, v.deck.center.z + v.deck.half.z);
    let bead_half =
        Vec3::new(v.deck.half.x * 0.85, d.weld_seam_half_thickness, d.weld_seam_half_thickness);
    parts.push(detail_plate(
        SubmeshKind::Hull,
        MaterialRole::RolledArmor,
        solid::ConvexSolid::box_at(bead_center, bead_half),
    ));

    parts
}

/// A visual swing-arm bracket per road wheel, bridging the hull's lower tub side to the wheel hub at
/// axle height. Without it the road wheels read as floating on a bare axle line; this is the link
/// that mounts them to the hull. `lower_half_width` is the hull tub half-width (the pivot side).
pub fn t54_suspension_parts(v: &HybridVisual, lower_half_width: f32) -> Vec<VehiclePart> {
    let gear = &v.running_gear;
    let axle_y = v.track_belt.axle_y;
    let wheel_inner = gear.side_x - gear.wheel_half_width;
    let arm_cx = 0.5 * (lower_half_width + wheel_inner);
    let arm_hx = (0.5 * (wheel_inner - lower_half_width)).max(0.04);
    let mut parts = Vec::new();
    for z in revolve::road_wheel_stations(gear) {
        for side in [1.0_f32, -1.0] {
            let center = Vec3::new(side * arm_cx, axle_y, z);
            parts.push(detail_plate(
                SubmeshKind::Hull,
                MaterialRole::TrackMetal,
                solid::ConvexSolid::box_at(center, Vec3::new(arm_hx, 0.05, 0.10)),
            ));
        }
    }
    parts
}
