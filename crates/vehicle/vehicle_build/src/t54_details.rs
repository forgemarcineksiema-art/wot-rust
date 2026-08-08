//! Visual-only factory detailing for the hybrid T-54, assembled as `PartLod::Detail` parts so it
//! appears only at the close-up LOD0 and is dropped from LOD1/LOD2 (which keep the silhouette,
//! mount-critical parts and a readable track band). Clean factory build: crisp manufactured greeble
//! — an engine-deck grille, the exhaust cover, turret periscopes, fender lips and a restrained
//! glacis/deck weld bead — and deliberately no mud, rust, battle damage, decals or weathering. Every
//! piece reads its dimensions from the blueprint's [`VisualDetail`]; none invents a tank dimension.

use game_core::{BoxVisual, CompleteVisual, FittingsVisual};
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
    // The commander's lid gets a RIM handle rather than a crown one. The cupola is the tallest
    // thing on the tank and its lid already sits at the top of the silhouette; a bar stacked on
    // top of that stands the vehicle above its own collision box, and the doctrine is that the
    // box IS the footprint. It is also where the handle is: you pull this lid round, not up.
    parts.extend(hatch_hardware(
        "cupola_hatch",
        SubmeshKind::Turret,
        f.cupola_hatch_center,
        f.cupola_hatch_radius,
        f.cupola_hatch_half_height,
        HandlePlacement::Rim,
    ));
    parts.extend(hatch_hardware(
        "driver_hatch",
        SubmeshKind::Hull,
        f.driver_hatch_center,
        f.driver_hatch_radius,
        f.driver_hatch_half_height,
        HandlePlacement::Crown,
    ));
    parts.extend(hatch_hardware(
        "loader_hatch",
        SubmeshKind::Turret,
        f.loader_hatch_center,
        f.loader_hatch_radius,
        f.loader_hatch_half_height,
        HandlePlacement::Crown,
    ));
    parts
}

/// The hardware that makes a lid a lid: the collar it seats on, the hinge it swings about and the
/// handle a crewman pulls.
///
/// Every hatch on this vehicle was one `revolve::drum` puck — and so was every hatch on every
/// other vehicle in the fleet, because `grep hinge` over the whole repository returned nothing.
/// A cover with no coaming under it, no hinge behind it and no handle on it is a disc painted on
/// the roof; the step between the collar and the cover is most of what says otherwise.
/// Where a hatch's grab handle goes: across the crown of the cover, or out at its rim.
#[derive(Clone, Copy, PartialEq)]
enum HandlePlacement {
    Crown,
    Rim,
}

fn hatch_hardware(
    name: &'static str,
    submesh: SubmeshKind,
    center: Vec3,
    radius: f32,
    half_height: f32,
    handle: HandlePlacement,
) -> Vec<VehiclePart> {
    let seat = center.y - half_height;
    // The hinge lies BEHIND the lid (toward -Z), which is the way these covers open on a T-54:
    // forward, so the crewman is shielded by the raised cover.
    let hinge_z = center.z - radius * 0.94;
    vec![
        VehiclePart {
            key: PartKey::indexed(name, 100),
            submesh,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(detail::coaming(
                Vec3::new(center.x, seat, center.z),
                Vec3::Y,
                radius * 1.10,
                half_height * 0.85,
                radius * 0.14,
                MaterialRole::RolledArmor,
                20,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        },
        VehiclePart {
            key: PartKey::indexed(name, 101),
            submesh,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(detail::hinge(
                Vec3::new(center.x, center.y + half_height * 0.55, hinge_z),
                Vec3::X,
                radius * 0.80,
                radius * 0.075,
                radius * 0.34,
                3,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        },
        VehiclePart {
            key: PartKey::indexed(name, 102),
            submesh,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(match handle {
                HandlePlacement::Crown => detail::grab_handle(
                    Vec3::new(
                        center.x - radius * 0.45,
                        center.y + half_height,
                        center.z + radius * 0.30,
                    ),
                    Vec3::new(
                        center.x + radius * 0.45,
                        center.y + half_height,
                        center.z + radius * 0.30,
                    ),
                    Vec3::Y,
                    (half_height * 0.45).min(0.05),
                ),
                HandlePlacement::Rim => detail::grab_handle(
                    Vec3::new(center.x - radius * 0.40, center.y, center.z + radius * 0.80),
                    Vec3::new(center.x + radius * 0.40, center.y, center.z + radius * 0.80),
                    Vec3::Z,
                    radius * 0.22,
                ),
            }),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        },
    ]
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
/// A tow hook: the bracket welded to the nose plate, the curved throat a shackle drops into, and
/// the catch across its mouth.
///
/// It was a `ConvexSolid::box_at` — a 240 x 220 x 200 brick with no curvature, no throat and no
/// catch. That is a boss, not a hook: there is nowhere for a shackle to go (register K9).
pub(crate) fn t54_tow_hook(index: u16, center: Vec3, half: Vec3) -> Vec<VehiclePart> {
    // The throat: a C of steel opening FORWARD, swept round so a shackle has somewhere to sit.
    let radius = half.y * 0.62;
    let throat: Vec<Vec3> = (0..=9)
        .map(|k| {
            // From the top of the mouth, round the back, to the bottom — leaving the front open.
            let a = std::f32::consts::PI * (0.30 + 1.40 * k as f32 / 9.0);
            let (sin, cos) = a.sin_cos();
            center + Vec3::new(0.0, sin * radius, -cos * radius)
        })
        .collect();
    vec![
        // The bracket that carries it, welded flat to the plate.
        detail_plate(
            PartKey::indexed("tow_hook", index),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::chamfered_box(
                center - Vec3::Z * (half.z * 0.55),
                Vec3::new(half.x * 0.85, half.y * 0.92, half.z * 0.45),
                0.018,
            ),
        ),
        VehiclePart {
            key: PartKey::indexed("tow_hook_throat", index),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(detail::handle_rail(&throat, half.x * 0.32)),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        },
        // The catch across the mouth: what stops the shackle jumping out over a ditch.
        VehiclePart {
            key: PartKey::indexed("tow_hook_catch", index),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::handle_rail(
                &[
                    center + Vec3::new(0.0, radius * 0.86, radius * 0.50),
                    center + Vec3::new(0.0, -radius * 0.86, radius * 0.50),
                ],
                half.x * 0.16,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        },
    ]
}

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

/// One vision device, built as one: the raked housing, the GLASS prism it looks through, and the
/// armoured cheeks that keep the prism. The head alone is a box with a corner cut — seven planes —
/// which is what the construction floor lists both periscope keys as debt for.
///
/// The prism and the guards carry the same `PartKey` name as the head on purpose: the
/// construction floor counts a key's parts as their UNION, so the device is judged as a device
/// rather than as its flattest piece — a prism really is a six-plane pane and correctly so — and
/// the anchors and the manifest keep naming one periscope rather than four fragments.
fn periscope_parts(
    name: &'static str,
    submesh: SubmeshKind,
    instance: u16,
    center: Vec3,
    half: Vec3,
) -> Vec<VehiclePart> {
    let mut parts = vec![
        detail_plate(
            PartKey::indexed(name, instance * 4),
            submesh,
            MaterialRole::RolledArmor,
            solid::t54_periscope(center, half),
        ),
        detail_plate(
            PartKey::indexed(name, instance * 4 + 1),
            submesh,
            MaterialRole::Glass,
            solid::t54_periscope_prism(center, half),
        ),
    ];
    for (k, guard) in solid::t54_periscope_guards(center, half).into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed(name, instance * 4 + 2 + k as u16),
            submesh,
            MaterialRole::RolledArmor,
            guard,
        ));
    }
    parts
}
/// Every factory detail part for the T-54, all at `PartLod::Detail`.
pub fn t54_detail_parts(v: CompleteVisual<'_>) -> Vec<VehiclePart> {
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

    // The gusset brackets hanging below each fender. The lip is no longer a part: it is the fold of
    // the fender pressing itself (`t54_fender`), which is what a lip actually is.
    let mut bracket_n = 0u16;
    for side in [v.fender.side_x, -v.fender.side_x] {
        for bracket in solid::t54_fender_brackets(side, v.fender) {
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
        parts.extend(periscope_parts(
            "turret_periscope",
            SubmeshKind::Turret,
            i as u16,
            center,
            d.periscope_half,
        ));
    }

    // The driver's two forward vision periscopes on the hull roof, flanking and just ahead of the
    // driver's hatch. Derived from the hatch position (no new blueprint dimension) and clear of the
    // hatch lid; same raked prism head as the turret periscopes.
    let dh = v.fittings.driver_hatch_center;
    let driver_peri_half = Vec3::new(0.055, 0.05, 0.05);
    for (i, dx) in [-0.26_f32, 0.26].into_iter().enumerate() {
        let center = Vec3::new(dh.x + dx, dh.y, dh.z + 0.08);
        parts.extend(periscope_parts(
            "driver_periscope",
            SubmeshKind::Hull,
            i as u16,
            center,
            driver_peri_half,
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
