//! Visual-only factory detailing for the hybrid T-54, assembled as `PartLod::Detail` parts so it
//! appears only at the close-up LOD0 and is dropped from LOD1/LOD2 (which keep the silhouette,
//! mount-critical parts and a readable track band). Clean factory build: crisp manufactured greeble
//! — an engine-deck grille, the exhaust cover, turret periscopes, fender lips and a restrained
//! glacis/deck weld bead — and deliberately no mud, rust, battle damage, decals or weathering. Every
//! piece reads its dimensions from the blueprint's [`VisualDetail`]; none invents a tank dimension.

use game_core::DetailVisual;
use game_core::roundness::round_segments;
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
        // A lid, not a disc. `revolve::drum` gives a flat-topped puck, and that is what every
        // hatch on this vehicle wore: the collar, hinge and handle around it did all the work of
        // saying "hatch" while the cover itself said "coin". A real armoured lid is PRESSED — it
        // domes toward its centre so a round strikes it off-normal, and it steps at the rim where
        // it seats on its coaming.
        //
        // Four stations rather than two: the rim step, the seating shoulder, the dome's shoulder
        // and its crown.
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::Y,
                // The crown sits at exactly the height the flat puck's top did. Doming a lid
                // must not raise the vehicle: the honesty doctrine says the collision box IS the
                // visual footprint, and `hitbox_fit` proved it by failing at 2.58 against 2.53
                // when the dome was allowed to grow. The press lives INSIDE the height it had.
                &[
                    (-half_height, 0.0),
                    (-half_height, radius),
                    (half_height * 0.30, radius),
                    (half_height * 0.52, radius * 0.95),
                    (half_height * 0.85, radius * 0.70),
                    (half_height, 0.0),
                ],
                round_segments(radius),
                MaterialRole::RolledArmor,
                SmoothingGroup(2),
            ),
            center,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    }
}

/// Where each hatch's hardware meets the metal: the LOCAL surface height under every lid, owned
/// by the caller who builds that surface (the cupola drum, the dome roof, the hull roof).
pub struct HatchSeats {
    /// Top of the commander's cupola drum.
    pub cupola: f32,
    /// The hull roof plane the driver's hatch is cut into.
    pub driver: f32,
    /// The dome's roof plate — the loader's hatch stands inside the top station's footprint.
    pub loader: f32,
}

/// The semantic drum fittings: the commander's cupola hatch and the loader's hatch ride the turret
/// (so they traverse); the driver's hatch and the glacis headlight ride the hull. Each is its own
/// part, not anonymous greeble.
pub fn t54_fitting_parts(f: &FittingsVisual, seats: &HatchSeats) -> Vec<VehiclePart> {
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
        seats.cupola,
    ));
    parts.extend(hatch_hardware(
        "driver_hatch",
        SubmeshKind::Hull,
        f.driver_hatch_center,
        f.driver_hatch_radius,
        f.driver_hatch_half_height,
        HandlePlacement::Crown,
        seats.driver,
    ));
    parts.extend(hatch_hardware(
        "loader_hatch",
        SubmeshKind::Turret,
        f.loader_hatch_center,
        f.loader_hatch_radius,
        f.loader_hatch_half_height,
        HandlePlacement::Crown,
        seats.loader,
    ));
    parts
}

/// Vision blocks around the commander's cupola — the reason the drum exists at all.
///
/// The cupola was a smooth 24-sided cylinder: the tallest fitting on the vehicle, the one thing
/// standing above the roofline at every range, and it carried not a single device a commander
/// could look through. Every reference shows the ring of blocks under the drum's top rim.
///
/// Five around the forward arc, each a device in the periscope pattern: an armoured hood rooted
/// INTO the drum with a GLASS pane lying in its outer face, both sharing one part key so the
/// construction floor judges the device rather than its flattest piece.
pub fn t54_cupola_vision_blocks(center: Vec3, radius: f32, top_y: f32) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    // Just under the top rim, looking out and slightly over the forward half.
    let band_y = top_y - 0.048;
    // Width is bounded by CHORD geometry, not taste: the slit's frame is a flat plate on a
    // curved drum, so its corners reach `sqrt(r² + half_w²)` — at 48 mm of half-width that is
    // ⌀637 against the Locked ⌀624 ±10 tape. 42 mm keeps the corners inside the anchor.
    let (half_w, half_h, half_d) = (0.042, 0.026, 0.036);
    for (i, azimuth) in [-1.55_f32, -0.78, 0.0, 0.78, 1.55].into_iter().enumerate() {
        let (sin, cos) = azimuth.sin_cos();
        // 0 rad faces +Z, the bow; the tangent runs across the face.
        let out = Vec3::new(sin, 0.0, cos);
        let across = Vec3::new(cos, 0.0, -sin);
        // FLUSH, NOT PROUD. The first build stood armoured hoods 22 mm off the drum, and the
        // dimension gate threw it straight back: the cupola's Locked ⌀624 mm is measured as a
        // tape around the armour skin, and the tape caught the hoods at ⌀675. The gate was
        // right on the history too — the obr. 1951 cupola carries its vision devices IN the
        // drum, slits with armoured glass behind them, not pods bolted onto it. So the frame
        // sits 3 mm proud (inside the anchor's ±10 mm) and the GLASS is recessed 6 mm into the
        // opening, which is what makes a slit read as a slit: a dark rectangle in the casting
        // with glass at the bottom of it. The band height is ABSOLUTE — `center` carries the
        // drum's own y, which must not be added twice (the very first build put the ring 2.4 m
        // over the tank).
        let wall = Vec3::new(center.x, band_y, center.z) + out * radius;
        parts.push(VehiclePart {
            key: PartKey::indexed("cupola_vision_block", i as u16),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup::hard_edges(),
            // Outer face AT the drum's nominal radius: the 24-gon's facets sag ~2.7 mm below it
            // between vertices, so the frame still stands off the wall it is set into — and the
            // tape around the casting stays inside its Locked diameter.
            shape: PartShape::Mesh(detail::oriented_plate(
                wall - out * half_d,
                across * half_w,
                Vec3::Y * half_h,
                out * half_d,
                MaterialRole::CastArmor,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Solid,
        });
        // The pane, at the bottom of its opening: recessed behind the frame's face, spanning
        // most of the slit so the dark rectangle has glass in it rather than more steel.
        parts.push(VehiclePart {
            key: PartKey::indexed("cupola_vision_block", 8 + i as u16),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::Glass,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::oriented_plate(
                wall - out * 0.006,
                across * (half_w * 0.80),
                Vec3::Y * (half_h * 0.70),
                out * 0.003,
                MaterialRole::Glass,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Solid,
        });
    }
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
    seat_y: f32,
) -> Vec<VehiclePart> {
    // THE HARDWARE SITS ON THE METAL, NOT ON THE LID'S MATHS. It used to hang everything off
    // `center.y - half_height` — the lid's own base — and a lid is deliberately rooted DEEP into
    // the casting so it cannot levitate. Root the lid 120 mm down and the collar goes down with
    // it: the cupola's coaming sat entirely inside the drum, the loader's coaming AND hinge lay
    // under the dome roof — 412 triangles rendering zero pixels — and the covering test measured
    // the collar against the lid, which is the exact relationship that never breaks. `seat_y` is
    // the LOCAL metal surface (drum top, dome roof, hull roof), passed by the caller who owns
    // that geometry; the visibility lock measures the result against the built meshes.
    let lid_top = center.y + half_height;
    let exposed = (lid_top - seat_y).max(0.02);
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
                // An 8 mm weld bite into the surface, and the collar climbs from there.
                Vec3::new(center.x, seat_y - 0.008, center.z),
                Vec3::Y,
                radius * 1.10,
                (exposed * 0.55).clamp(0.018, 0.045) + 0.008,
                radius * 0.14,
                MaterialRole::RolledArmor,
                round_segments(radius),
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
                Vec3::new(center.x, seat_y + exposed * 0.55, hinge_z),
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
                // The rim handle rides where the lid shows, not the lid's buried midline: at
                // `center.y` half of it sat inside the cupola drum.
                HandlePlacement::Rim => detail::grab_handle(
                    Vec3::new(
                        center.x - radius * 0.40,
                        seat_y + exposed * 0.45,
                        center.z + radius * 0.80,
                    ),
                    Vec3::new(
                        center.x + radius * 0.40,
                        seat_y + exposed * 0.45,
                        center.z + radius * 0.80,
                    ),
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
    //
    // It used to sweep `-cos(a)`, which puts the bulge at +Z and the 108-degree gap at -Z — the
    // mouth facing the plate the hook is welded to, and the catch stranded inside the closed
    // half. A hook a shackle cannot enter is a doughnut. The comment said "leaving the front
    // open" the whole time; the code did the opposite and the test only asked whether three
    // parts existed and whether the catch was ahead of the bracket, which the reversed version
    // satisfied too.
    let radius = half.y * 0.62;
    let throat: Vec<Vec3> = (0..=9)
        .map(|k| {
            // From the top of the mouth, round the BACK, to the bottom — the gap faces the bow.
            let a = std::f32::consts::PI * (0.30 + 1.40 * k as f32 / 9.0);
            let (sin, cos) = a.sin_cos();
            center + Vec3::new(0.0, sin * radius, cos * radius)
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
            // Across the MOUTH. The C now closes toward the plate it is welded to and opens at
            // the bow, so the catch belongs at +Z, spanning the gap. Sitting inside the closed
            // half — which is where it used to be — it stopped nothing from jumping out.
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
        // The two long seams, at the deck EDGES.
        //
        // These used to sit at 0.86 of the half-width, which is x = +-0.817 — inside the
        // transmission covers, which span 0.59..0.91 and stand 30 mm proud while a bolt head is
        // 10 mm. Fourteen of the eighteen were sealed in solid geometry: 1008 triangles rendering
        // the inside of a plate, and the four that showed were the ones nobody aimed for. The
        // covering test asks for a span wider than 0.8 m and a height near the deck, which buried
        // bolts satisfy perfectly.
        //
        // At the edge they do the job they exist for: two plates of the same colour, 30 mm apart
        // in height, are told apart by the line of fasteners along the seam between them.
        let x = deck.half.x * if row == 0 { -0.98 } else { 0.98 };
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

/// The left-fender exhaust: an armoured cowl with LOUVRES on its outboard face and a dark OUTLET
/// at its stern.
///
/// It was `chamfered_box(exhaust_center, exhaust_half, 0.03)` and nothing else — the same
/// primitive as a stowage bin, differing from one only in the chamfer (0.03 against 0.035). On the
/// deck it read as another toolbox, which is what an exhaust with no opening is.
///
/// And this vehicle was BEHIND the eight it is supposed to be the bar for: `soviet_exhaust_ports`
/// gives the recipe fleet short pipes with dark open mouths, the Germans get stacks, the Centurion
/// gets cowls. A good decision applied to eight vehicles and skipped on the ninth.
fn exhaust_cowl(d: &DetailVisual) -> Vec<VehiclePart> {
    let (c, h) = (d.exhaust_center, d.exhaust_half);
    // Outboard is away from the centreline; the cowl sits on the LEFT shelf, so its vented face
    // looks out over the track rather than in at the hull.
    let out = if c.x < 0.0 { -1.0 } else { 1.0 };
    let mut parts = vec![detail_plate(
        PartKey::new("exhaust_cover"),
        SubmeshKind::Hull,
        MaterialRole::TrackMetal,
        solid::t54_exhaust_housing(d),
    )];

    // Louvres across the outboard face: hot air leaves through something.
    parts.push(VehiclePart {
        key: PartKey::indexed("exhaust_cover", 1),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::TrackMetal,
        smoothing: vehicle_geometry::SmoothingGroup(0),
        // Set into the face rather than onto it: the slats stand `depth` proud of where they are
        // centred, and the cowl's outboard face is already at x 1.60 against a track outer edge
        // of 1.61. Louvres are pressed into a panel anyway — they are not fins bolted to one.
        //
        // Width across the face and height up it, which is now what the kernel means by those
        // words. Before, a vertical face got them swapped: 0.675 m of "width" stood UP, so five
        // blades grew out of a 220 mm box, through the fender, and into the moving top run.
        shape: PartShape::Mesh(detail::louvre_slats(
            Vec3::new(c.x + out * (h.x - 0.022), c.y, c.z),
            Vec3::new(out, 0.0, 0.0),
            h.z * 1.5,
            h.y * 1.3,
            0.016,
            5,
            0.5,
        )),
        lod: PartLod::Detail,
        // `detail` has no `GeneratorKind` variant of its own, which is a small proof of the
        // audit's point that this field is an author-typed label rather than a derived fact.
        // `Solid` is the closest honest answer: louvre slats are plate boxes.
        generator: GeneratorKind::Solid,
    });

    // The outlet itself, at the stern face: a rim, and inside it the dark mouth the fleet's own
    // exhaust ports already use — a recessed disc in track steel, which reads as a hole.
    let mouth = Vec3::new(c.x, c.y - h.y * 0.15, c.z - h.z - 0.005);
    let radius = (h.y * 0.62).min(h.x * 0.42);
    parts.push(VehiclePart {
        key: PartKey::indexed("exhaust_cover", 2),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: vehicle_geometry::SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::merge(&[
            detail::coaming(
                mouth,
                Vec3::Z,
                radius,
                0.05,
                0.018,
                MaterialRole::BarrelSteel,
                round_segments(radius),
            ),
            // The mouth: recessed a finger inside the rim so it sits in its own shadow.
            revolve::translate(
                &revolve::revolve(
                    Vec3::Z,
                    &[(0.0, 0.0), (0.0, radius - 0.020), (0.030, radius - 0.020)],
                    round_segments(radius),
                    MaterialRole::TrackMetal,
                    vehicle_geometry::SmoothingGroup(3),
                ),
                mouth + Vec3::new(0.0, 0.0, 0.006),
            ),
        ])),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });
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
    parts.extend(exhaust_cowl(d));

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

    // THE ROOF VENTILATOR. A mushroom-domed extractor sits on the crown behind the hatches, and
    // it is a tell: obr. 1951 has one, the plan view shows it, and the model's roof was bare
    // where it belongs. Every reference — line drawing, museum vehicle, period photograph —
    // reads it from above and in profile.
    //
    // Rooted INTO the casting rather than perched on it. Its seat is taken from the loader
    // hatch's own height so it follows the crown if the dome is ever reshaped, and it stands on
    // the centreline behind both hatches where the fighting compartment's foul air is drawn off.
    {
        let seat = Vec3::new(0.0, v.fittings.loader_hatch_center.y - 0.10, -0.50);
        let radius = 0.132;
        parts.push(VehiclePart {
            key: PartKey::new("turret_ventilator"),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    Vec3::Y,
                    // (offset along the axis, radius): a flat seat, a rolled shoulder and a
                    // domed crown — the pressed cap of an extractor, not a bubble.
                    &[
                        (0.0, 0.0),
                        (0.0, radius * 0.94),
                        (0.055, radius),
                        (0.125, radius * 0.92),
                        (0.165, radius * 0.58),
                        (0.180, 0.0),
                    ],
                    round_segments(radius),
                    MaterialRole::CastArmor,
                    SmoothingGroup(3),
                ),
                seat,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
        // The armoured cap stands on a short collar, so the dome reads as a cover over an opening
        // rather than as a blister moulded into the roof.
        parts.push(VehiclePart {
            key: PartKey::new("turret_ventilator_collar"),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::RolledArmor,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(detail::coaming(
                Vec3::new(seat.x, seat.y - 0.02, seat.z),
                Vec3::Y,
                radius * 0.72,
                0.030,
                0.016,
                MaterialRole::RolledArmor,
                round_segments(radius * 0.72),
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }

    // THE AERIAL. A whip on the turret's left rear quarter, tapering as it rises. It is thin
    // enough to cost nothing and tall enough to be part of the silhouette at any range — the
    // reference render reads it against the sky from every angle, and the model had no antenna
    // anywhere in the fleet.
    {
        let base = Vec3::new(-0.58, v.fittings.loader_hatch_center.y - 0.28, -0.70);
        parts.push(VehiclePart {
            key: PartKey::new("turret_aerial_base"),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::TrackMetal,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    Vec3::Y,
                    &[(0.0, 0.0), (0.0, 0.052), (0.030, 0.048), (0.062, 0.030), (0.062, 0.0)],
                    round_segments(0.052),
                    MaterialRole::TrackMetal,
                    SmoothingGroup(3),
                ),
                base,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
        parts.push(VehiclePart {
            key: PartKey::new("turret_aerial"),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(3),
            shape: PartShape::Mesh(revolve::translate(
                &revolve::revolve(
                    Vec3::Y,
                    // A whip tapers: 14 mm at the ferrule down to 4 mm at the tip, over 1.25 m.
                    &[(0.0, 0.014), (0.30, 0.009), (0.60, 0.004), (0.60, 0.0)],
                    8,
                    MaterialRole::BarrelSteel,
                    SmoothingGroup(3),
                ),
                base + Vec3::new(0.0, 0.055, 0.0),
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
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
