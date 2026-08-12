//! The T-54's external kit: the fender stowage line (fuel tanks and bins on the shelves over the
//! tracks), the arched mudguards over the end wheels, the glacis splash board, the turret
//! handrails and the stowed tow cables. All are `PartLod::Detail` visual parts derived from the
//! blueprint's [`VisualDetail`]; none adds a gameplay dimension. The kit is what makes the
//! narrow-box hull read as a T-54: the tracks stay exposed and the shelves above them carry the
//! visual mass.

use game_core::{CompleteVisual, FenderVisual, TrackShape};
use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::t54_details::detail_plate;

/// Every kit part: fender stowage, arched mudguards, splash board, turret rails, tow cables.
pub fn t54_kit_parts(
    v: CompleteVisual<'_>,
    glacis_deg: f32,
    track: &TrackShape,
    stern: crate::t54_kit_lines::SternLine,
) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    parts.extend(fender_stowage(v.fender));
    parts.extend(mudguard_arches(v.fender, track));
    // Line-work: splash board, turret rails, tow cables, the unditching beam, the travel lock.
    parts.extend(crate::t54_kit_lines::t54_line_kit_parts(v, glacis_deg, stern));
    parts
}

/// The mudguard arches over the end wheels, with their hanging flaps — ONE swept sheet per side
/// and end.
///
/// The old construction had the vertical logic of the fender line BACKWARDS: a flat shelf ran
/// the full hull length (so its lip had to clear the loop's HIGHEST links, up on the end
/// wraps), and plain slabs angled DOWN over the ends. Every reference shows the opposite: the
/// flat shelf sits LOW between the wraps — the crest links nearly brush it — and over each end
/// wheel the sheet KICKS UP into an arched guard (the three-view reads the bow guard's band at
/// 1.17-1.24) before falling to the hanging flap. The arch is what buys the low shelf.
///
/// Paths are authored around the blueprint's own end wheels (`end_front` / `end_z`), so moving
/// an axle carries its guard along.
fn mudguard_arches(fender: &FenderVisual, track: &TrackShape) -> Vec<VehiclePart> {
    let shelf_top = fender.center_y + fender.half.y;
    let (idler_z, _) = track.end_front.unwrap_or((track.end_z, track.end_radius));
    // (key, sign toward the end, end-wheel axle |z|, ribbed flap?)
    let ends =
        [("mudguard_bow", 1.0_f32, idler_z, false), ("mudguard_tail", -1.0, track.end_z, true)];
    let mut parts = Vec::new();
    let mut index = 0u16;
    for (name, sign, axle_z, ribbed) in ends {
        // The wave, in the y-z side plane: off the shelf, up over the wrap, down the outboard
        // face, then the near-vertical flap. z magnitudes run outboard; `sign` mirrors for the
        // tail. Clearance over the wrap's link line is asserted by
        // `the_mudguards_arch_over_the_end_wheels`.
        let profile: [(f32, f32); 7] = [
            (axle_z - 0.24, shelf_top),
            (axle_z - 0.10, shelf_top + 0.115),
            (axle_z + 0.03, shelf_top + 0.165),
            (axle_z + 0.18, shelf_top + 0.115),
            (axle_z + 0.33, shelf_top - 0.075),
            (axle_z + 0.40, shelf_top - 0.185),
            (axle_z + 0.42, shelf_top - 0.305),
        ];
        for side in [fender.side_x, -fender.side_x] {
            let path: Vec<Vec3> =
                profile.iter().map(|&(z, y)| Vec3::new(side, y, sign * z)).collect();
            let section = SweepSection {
                points: vec![
                    Vec2::new(-fender.half.x, -0.005),
                    Vec2::new(fender.half.x, -0.005),
                    Vec2::new(fender.half.x, 0.005),
                    Vec2::new(-fender.half.x, 0.005),
                ],
                closed: true,
            };
            let mesh = try_sweep(&SweepSpec {
                path: &SweepPath { points: path, closed: false },
                section: &section,
                frame_mode: SweepFrameMode::FixedUp(Vec3::X),
                caps: SweepCaps::Both,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
                section_scale: None,
            })
            .expect("a mudguard arch is a valid sweep");
            parts.push(VehiclePart {
                key: PartKey::indexed(name, index),
                submesh: SubmeshKind::Hull,
                material: MaterialRole::RolledArmor,
                smoothing: SmoothingGroup::hard_edges(),
                shape: PartShape::Mesh(mesh),
                lod: PartLod::Detail,
                generator: GeneratorKind::Sweep,
            });
            // The tail flap carries its pressed stiffening ribs (reference rear view); the bow
            // guard is smooth.
            if ribbed {
                let (top, bottom) = (profile[5], profile[6]);
                for (r, rib) in solid::t54_flap_ribs(
                    side,
                    Vec2::new(sign * top.0, top.1),
                    Vec2::new(sign * bottom.0, bottom.1),
                    fender.half.x,
                )
                .into_iter()
                .enumerate()
                {
                    parts.push(detail_plate(
                        PartKey::indexed("fender_flap_rib", index * 3 + r as u16),
                        SubmeshKind::Hull,
                        MaterialRole::RolledArmor,
                        rib,
                    ));
                }
            }
            index += 1;
        }
    }
    parts
}

/// The fender stowage line from the references' top view: a forward bin, the flat external fuel
/// tanks and a rear bin on the right shelf; toolboxes fore and aft of the exhaust cover (placed by
/// `t54_details`) on the left. Chamfered pressings, tops well below the roof, outer faces just
/// inside the track span.
fn fender_stowage(fender: &FenderVisual) -> Vec<VehiclePart> {
    let base = fender.center_y + fender.half.y + 0.005;
    // (side, key, z centre, half length, half height)
    // The asymmetry IS the recognition feature (register M7): the RIGHT shelf carries two flat
    // rectangular external fuel tanks with stowage fore and aft of them; the LEFT carries three
    // stowage bins and the exhaust cover (placed by `t54_details`). The model had three tanks on
    // the right, which is a later fit — obr. 1951 is two.
    let boxes: [(f32, &str, f32, f32, f32); 8] = [
        (1.0, "stowage_bin", 2.15, 0.28, 0.15),
        (1.0, "fuel_tank", 1.05, 0.42, 0.13),
        (1.0, "fuel_tank", -0.15, 0.42, 0.13),
        (1.0, "stowage_bin", -1.35, 0.40, 0.14),
        (1.0, "stowage_bin", -2.35, 0.28, 0.14),
        (-1.0, "stowage_bin", 1.95, 0.40, 0.15),
        (-1.0, "stowage_bin", 0.55, 0.35, 0.14),
        (-1.0, "stowage_bin", -2.30, 0.35, 0.14),
    ];
    let mut parts = Vec::new();
    for (i, &(side, key, z, half_z, half_y)) in boxes.iter().enumerate() {
        // Centred on the SHELF, not on a copy of where the shelf used to be. This was a
        // literal 1.34 beside a fender that has since moved with the documented track gauge.
        let center = Vec3::new(side * fender.side_x, base + half_y, z);
        let half = Vec3::new(0.27, half_y, half_z);
        parts.push(detail_plate(
            PartKey::indexed(key, i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::chamfered_box(center, half, 0.035),
        ));
        // The flat external fuel tanks carry the references' pressed X-stiffened lids: two raised
        // diagonal ribs across the top face.
        if key == "fuel_tank" {
            parts.push(tank_lid_ribs(i as u16, center, half));
        }
        parts.push(lid_furniture(key, i as u16, side, center, half));
    }
    parts
}

/// What makes a box a BIN: the seam its lid closes on, the hinge it swings about, and the latch
/// that holds it shut on a moving tank.
///
/// Every stowage bin and fuel tank on this vehicle was a single `chamfered_box` and nothing else —
/// 44 triangles, fourteen planes, indistinguishable from a block of soap at deck distance. The
/// fuel tanks proved the technique in the same file: their pressed X of two `weld_bead` diagonals
/// reads clearly from the deck camera. The bins simply never got the same treatment.
///
/// Hinges go INBOARD (a lid opens away from the track, or the crewman opening it is standing on
/// the running gear) and the latch outboard, where a hand reaches it from the ground.
fn lid_furniture(
    key: &'static str,
    instance: u16,
    side: f32,
    center: Vec3,
    half: Vec3,
) -> VehiclePart {
    let top = center.y + half.y;
    let inboard = center.x - side * half.x;
    let outboard = center.x + side * half.x;
    // The lid line: a bead run around the top face, inset so it reads as the cover's edge rather
    // than as the box's own corner.
    let (ix, iz) = (half.x - 0.022, half.z - 0.022);
    // A LOOP, and it has to be built as one: `weld_bead` hard-codes `closed: false` and caps both
    // ends, so handing it a ring path lands two end caps on top of each other — eight non-manifold
    // edges, which is exactly what the kernel contract test caught when this was a `weld_bead`.
    // `casting_seam_loop` exists for precisely this, and a lid line IS a closed seam.
    let corners = [
        Vec3::new(center.x - ix, top + 0.004, center.z - iz),
        Vec3::new(center.x + ix, top + 0.004, center.z - iz),
        Vec3::new(center.x + ix, top + 0.004, center.z + iz),
        Vec3::new(center.x - ix, top + 0.004, center.z + iz),
    ];
    let mut pieces = vec![detail::casting_seam_loop(&corners)];

    // The hinge along the inboard lid edge, and a latch on the outboard one.
    pieces.push(detail::hinge(
        Vec3::new(inboard + side * 0.014, top + 0.006, center.z),
        Vec3::Z,
        (half.z * 1.4).min(0.34),
        0.011,
        0.030,
        3,
    ));
    pieces.push(detail::grab_handle(
        Vec3::new(outboard - side * 0.030, top + 0.002, center.z - 0.055),
        Vec3::new(outboard - side * 0.030, top + 0.002, center.z + 0.055),
        Vec3::Y,
        0.026,
    ));

    VehiclePart {
        // Offset well clear of the box instances (0..8) so the furniture keeps the bin's NAME —
        // the construction floor reads a key's parts as their union — without colliding with the
        // box's own identity, which anchors and the manifest key off.
        key: PartKey::indexed(key, instance + 64),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::BarrelSteel,
        smoothing: vehicle_geometry::SmoothingGroup(7),
        shape: PartShape::Mesh(revolve::merge(&pieces).weld_and_smooth()),
        lod: PartLod::Detail,
        // `detail` has no GeneratorKind of its own; Revolve is the closest honest answer for
        // beads, knuckles and a rail.
        generator: GeneratorKind::Revolve,
    }
}

/// The pressed X on one fuel-tank lid: two thin raised ribs along the top-face diagonals.
fn tank_lid_ribs(instance: u16, center: Vec3, half: Vec3) -> VehiclePart {
    let top = center.y + half.y + 0.004;
    let (dx, dz) = (half.x - 0.07, half.z - 0.07);
    let a = detail::weld_bead(
        &[
            Vec3::new(center.x - dx, top, center.z - dz),
            Vec3::new(center.x + dx, top, center.z + dz),
        ],
        0.014,
    );
    let b = detail::weld_bead(
        &[
            Vec3::new(center.x - dx, top, center.z + dz),
            Vec3::new(center.x + dx, top, center.z - dz),
        ],
        0.014,
    );
    VehiclePart {
        key: PartKey::indexed("tank_lid_ribs", instance),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: vehicle_geometry::SmoothingGroup(7),
        shape: PartShape::Mesh(revolve::merge(&[a, b])),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    }
}
