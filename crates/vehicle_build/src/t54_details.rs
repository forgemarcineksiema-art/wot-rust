//! Visual-only factory detailing for the hybrid T-54, assembled as `PartLod::Detail` parts so it
//! appears only at the close-up LOD0 and is dropped from LOD1/LOD2 (which keep the silhouette,
//! mount-critical parts and a readable track band). Clean factory build: crisp manufactured greeble
//! — an engine-deck grille, the exhaust cover, turret periscopes, fender lips and a restrained
//! glacis/deck weld bead — and deliberately no mud, rust, battle damage, decals or weathering. Every
//! piece reads its dimensions from the blueprint's [`HybridVisual`]; none invents a tank dimension.

use game_core::{GunVisual, HybridVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{PartLod, PartShape, VehiclePart};

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

    // Fender lips on both outer fender edges.
    for side in [v.fender.side_x, -v.fender.side_x] {
        parts.push(detail_plate(
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::t54_fender_lip(side, &v.fender, d),
        ));
    }

    // Turret-roof periscopes (commander + loader side), riding the turret so they traverse with it.
    for side in [d.periscope_center.x, -d.periscope_center.x] {
        let center = Vec3::new(side, d.periscope_center.y, d.periscope_center.z);
        parts.push(detail_plate(
            SubmeshKind::Turret,
            MaterialRole::RolledArmor,
            solid::ConvexSolid::box_at(center, d.periscope_half),
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
