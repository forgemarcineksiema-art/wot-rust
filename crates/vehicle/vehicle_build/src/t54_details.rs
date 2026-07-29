//! Visual-only factory detailing for the hybrid T-54, assembled as `PartLod::Detail` parts so it
//! appears only at the close-up LOD0 and is dropped from LOD1/LOD2 (which keep the silhouette,
//! mount-critical parts and a readable track band). Clean factory build: crisp manufactured greeble
//! — an engine-deck grille, the exhaust cover, turret periscopes, fender lips and a restrained
//! glacis/deck weld bead — and deliberately no mud, rust, battle damage, decals or weathering. Every
//! piece reads its dimensions from the blueprint's [`HybridVisual`]; none invents a tank dimension.

use game_core::{BoxVisual, FittingsVisual, HybridVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// A raised round lid/drum fitting (hatch lid or headlight), as its own `Detail` part.
fn drum_fitting(
    key: PartKey,
    submesh: SubmeshKind,
    center: Vec3,
    radius: f32,
    half_height: f32,
) -> VehiclePart {
    VehiclePart {
        key,
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
        generator: GeneratorKind::Revolve,
    }
}

/// The semantic drum fittings: the commander's cupola hatch and the loader's hatch ride the turret
/// (so they traverse); the driver's hatch and the glacis headlight ride the hull. Each is its own
/// part, not anonymous greeble.
pub fn t54_fitting_parts(f: &FittingsVisual) -> Vec<VehiclePart> {
    let mut parts = vec![
        drum_fitting(
            PartKey::new("cupola_hatch"),
            SubmeshKind::Turret,
            f.cupola_hatch_center,
            f.cupola_hatch_radius,
            f.cupola_hatch_half_height,
        ),
        drum_fitting(
            PartKey::new("driver_hatch"),
            SubmeshKind::Hull,
            f.driver_hatch_center,
            f.driver_hatch_radius,
            f.driver_hatch_half_height,
        ),
        drum_fitting(
            PartKey::new("loader_hatch"),
            SubmeshKind::Turret,
            f.loader_hatch_center,
            f.loader_hatch_radius,
            f.loader_hatch_half_height,
        ),
    ];
    parts.extend(t54_headlight(f));
    parts
}

/// The glacis headlight: a drum whose axis points FORWARD, with a glass lens in its face, a stalk
/// down to the fender and a guard hoop over it.
///
/// It was a `drum_fitting`, and `revolve::drum` revolves about **Y** — so the lamp was a vertical
/// puck with its flat faces up and down. The one part of this vehicle whose whole job is to point
/// somewhere was lying on its side, shining at the sky. Register K8.
fn t54_headlight(f: &FittingsVisual) -> Vec<VehiclePart> {
    let c = f.headlight_center;
    let r = f.headlight_radius;
    let depth = f.headlight_half_height;
    // (z, radius) along +Z: the body, then the rim, then the lens seat.
    let body = [
        (-depth, 0.0_f32),
        (-depth, r * 0.62),
        (-depth * 0.35, r),
        (depth * 0.8, r),
        (depth, r * 0.94),
        (depth, 0.0),
    ];
    let lens = [
        (depth - 0.004, 0.0_f32),
        (depth - 0.004, r * 0.88),
        (depth + 0.012, r * 0.80),
        (depth + 0.012, 0.0),
    ];

    let revolved = |profile: &[(f32, f32)], material: MaterialRole, smoothing: SmoothingGroup| {
        PartShape::Mesh(revolve::translate(
            &revolve::revolve(Vec3::Z, profile, 16, material, smoothing),
            c,
        ))
    };

    vec![
        VehiclePart {
            key: PartKey::new("headlight"),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup(2),
            shape: revolved(&body, MaterialRole::RolledArmor, SmoothingGroup(2)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        },
        // The LENS. Its own part and its own material: a lamp whose glass is rendered as the steel
        // drum behind it is a disc, and a viewer reads it as one.
        VehiclePart {
            key: PartKey::new("headlight_lens"),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::Glass,
            smoothing: SmoothingGroup(2),
            shape: revolved(&lens, MaterialRole::Glass, SmoothingGroup(2)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        },
        // The stalk it stands on. A lamp bolted to nothing floats.
        detail_plate(
            PartKey::new("headlight_bracket"),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::chamfered_box(
                Vec3::new(c.x, c.y - r - 0.045, c.z - depth * 0.3),
                Vec3::new(0.028, r * 0.55, 0.028),
                0.008,
            ),
        ),
        // And the hoop that keeps branches off the glass.
        VehiclePart {
            key: PartKey::new("headlight_guard"),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::handle_rail(
                &(0..7)
                    .map(|i| {
                        let angle = std::f32::consts::PI * (0.15 + 0.70 * i as f32 / 6.0);
                        let (sin, cos) = angle.sin_cos();
                        Vec3::new(c.x + cos * (r + 0.022), c.y + sin * (r + 0.022), c.z + depth)
                    })
                    .collect::<Vec<_>>(),
                0.012,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        },
    ]
}

/// The bolt rings that hold the engine-deck panels down, laid along the panel seams.
///
/// A bolt is a small thing and there are a lot of them, so they merge into ONE part rather than
/// forty: the graph is a description of the vehicle's assemblies, and "the deck fasteners" is one
/// assembly. Fine detail only — they drop at the first LOD step, where they stop being resolvable
/// anyway.
pub(crate) fn t54_deck_panel_bolts(deck: &BoxVisual, deck_top: f32) -> Vec<VehiclePart> {
    const PER_ROW: usize = 9;
    let mut heads = Vec::with_capacity(PER_ROW * 4);
    for row in 0..2 {
        // The two long seams, inboard of the deck edges.
        let x = deck.half.x * if row == 0 { -0.86 } else { 0.86 };
        for i in 0..PER_ROW {
            let t = (i as f32 + 0.5) / PER_ROW as f32;
            let z = deck.center.z - deck.half.z + t * 2.0 * deck.half.z;
            heads.push(detail::bolt_head(Vec3::new(x, deck_top, z), Vec3::Y, 0.016, 0.010));
        }
    }
    vec![VehiclePart {
        key: PartKey::new("engine_deck_bolts"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::merge(&heads).weld_and_smooth()),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }]
}

pub(crate) fn detail_plate(
    key: PartKey,
    submesh: SubmeshKind,
    material: MaterialRole,
    solid: solid::ConvexSolid,
) -> VehiclePart {
    VehiclePart {
        key,
        submesh,
        material,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid),
        lod: PartLod::Detail,
        generator: GeneratorKind::Solid,
    }
}

/// Every factory detail part for the T-54, all at `PartLod::Detail`.
pub fn t54_detail_parts(v: &HybridVisual) -> Vec<VehiclePart> {
    let d = &v.detail;
    let mut parts = Vec::new();

    // Engine-deck grille (well + frame + slats) and the left-fender exhaust cover ride the hull. The
    // well under the slats sits in shadow (the "engine_grille" surface bake) so it reads as a dark
    // cooling intake, not slats on the bright deck.
    let deck_top = v.deck.center.y + v.deck.half.y;
    for (i, solid) in solid::t54_deck_grille(d, deck_top).into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("deck_grille", i as u16),
            SubmeshKind::Hull,
            MaterialRole::TrackMetal,
            solid,
        ));
    }
    parts.push(detail_plate(
        PartKey::new("exhaust_cover"),
        SubmeshKind::Hull,
        MaterialRole::TrackMetal,
        solid::t54_exhaust_housing(d),
    ));

    // Fender lips on both outer fender edges, plus the support brackets hanging below each fender.
    let mut bracket_n = 0u16;
    for (i, side) in [v.fender.side_x, -v.fender.side_x].into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("fender_lip", i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::t54_fender_lip(side, &v.fender, d),
        ));
        for bracket in solid::t54_fender_brackets(side, &v.fender) {
            parts.push(detail_plate(
                PartKey::indexed("fender_bracket", bracket_n),
                SubmeshKind::Hull,
                MaterialRole::TrackMetal,
                bracket,
            ));
            bracket_n += 1;
        }
    }

    // Turret-roof periscopes (gunner + loader side), riding the turret so they traverse with it.
    // Each is a raked prism head (forward-looking glass), not a plain block.
    for (i, side) in [d.periscope_center.x, -d.periscope_center.x].into_iter().enumerate() {
        let center = Vec3::new(side, d.periscope_center.y, d.periscope_center.z);
        parts.push(detail_plate(
            PartKey::indexed("turret_periscope", i as u16),
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
    for (i, dx) in [-0.26_f32, 0.26].into_iter().enumerate() {
        let center = Vec3::new(dh.x + dx, dh.y, dh.z + 0.08);
        parts.push(detail_plate(
            PartKey::indexed("driver_periscope", i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::t54_periscope(center, driver_peri_half),
        ));
    }

    // Loader-side DShK (pedestal, receiver, ammo can, stepped barrel).
    parts.extend(crate::t54_dshk::t54_dshk_parts(v));

    // A restrained weld bead along the front edge of the engine deck (a crisp cast/plate seam).
    let bead_center =
        Vec3::new(0.0, v.deck.center.y + v.deck.half.y, v.deck.center.z + v.deck.half.z);
    let bead_half =
        Vec3::new(v.deck.half.x * 0.85, d.weld_seam_half_thickness, d.weld_seam_half_thickness);
    parts.push(detail_plate(
        PartKey::new("deck_weld_bead"),
        SubmeshKind::Hull,
        MaterialRole::RolledArmor,
        solid::ConvexSolid::box_at(bead_center, bead_half),
    ));

    parts
}
