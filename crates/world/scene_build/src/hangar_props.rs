//! Workshop props for the garage hangar: an overhead crane, stacks of spare road wheels and track
//! links, a workbench, fuel barrels, and oil stains on the floor. All procedural boxes and
//! cylinders, kept outside the turntable and the camera's closest orbit so nothing clips the hero
//! vehicle or the lens.

use glam::Vec3;
use renderer_api::SceneVertex;

use super::hangar::{Finish, HALF_X, HALF_Z, SLAB, finish, push_cylinder, slab};
use super::hangar_gallery::CRANE_GIRDER_Y;

const STEEL: [f32; 3] = [0.24, 0.25, 0.27];
const DARK_STEEL: [f32; 3] = [0.16, 0.17, 0.18];
/// C2 palette: the equipment olive — issued kit (the tool cabinet) wears the drab its crates
/// already wear, not shop-steel grey.
const OLIVE: [f32; 3] = [0.275, 0.30, 0.225];
/// C2 palette: rust on handled machined steel — the link plates hands have turned over.
const RUST: [f32; 3] = [0.295, 0.195, 0.14];
const RUBBER: [f32; 3] = [0.11, 0.11, 0.12];
/// The wear-and-grime ring where a standing prop meets the concrete (`push_contact_ring`).
const CONTACT: [f32; 3] = [0.185, 0.182, 0.176];
const WHEEL_HUB: [f32; 3] = [0.30, 0.28, 0.24];
const WOOD: [f32; 3] = [0.34, 0.26, 0.17];
const BARREL: [f32; 3] = [0.30, 0.34, 0.30];
// A darkened-concrete stain, not a void: against the hero-lit floor a near-black disc read as a
// hole/pit. Kept a clear step below the concrete (0.26) so it still registers as oil, but close
// enough that it stains the floor rather than cutting through it.
const OIL: [f32; 3] = [0.20, 0.19, 0.165];

/// Append every workshop prop to the hangar mesh. Wall-side props sit near the shell so the bay
/// floor around the turntable stays clear for the hero vehicle; the working zones stretch down
/// the nave's long axis (gate → hero station → second bay → stores), the A3 workflow line.
pub(super) fn push_props(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    // Wall-side props are anchored a fixed offset in from the shell, so they follow the wall at any
    // hall size instead of floating mid-floor.
    let wall = HALF_X - 1.7;
    let gate_corner_x = HALF_X - 3.0;
    let gate_corner_z = HALF_Z - 3.0;
    overhead_crane(v, i);
    wheel_stack(v, i, -gate_corner_x, -gate_corner_z);
    wheel_stack(v, i, -gate_corner_x - 0.7, -gate_corner_z);
    track_link_pile(v, i, gate_corner_x, -gate_corner_z);
    // A3 slot-frame compositions. The gun framing looks down the lane at the ajar gate:
    // ammunition racks flank the lane where the shells come IN (gate) before they go to the
    // rack by the stores. The suspension framing's low pass ends on the flow wall: running
    // gear stock — the same fleet wheels and links the gate corner holds — staged where the
    // gear WORK happens, beside the station.
    ammo_rack(v, i, -4.2, -(HALF_Z - 4.5));
    ammo_rack(v, i, 4.2, -(HALF_Z - 4.0));
    wheel_stack(v, i, -(HALF_X - 1.5), 0.9);
    track_link_pile(v, i, -(HALF_X - 1.6), -0.7);
    workbench(v, i, wall, 6.0);
    tool_board(v, i, 6.0);
    crate_pallet(v, i, gate_corner_x, gate_corner_z);
    barrels(v, i, -wall);
    oil_stains(v, i, wall);
    // Across the nave from the hero camera's rest position — the first A1 render had it at
    // (+5.5, 14.5), which put its gantry legs INSIDE the hero framing's eye.
    second_bay(v, i, -5.5, 14.5);
    stores_zone(v, i, -(HALF_X - 2.5), 17.0);
    extinguishers(v, i);
    station_dressing(v, i);
}

/// The human scale around the hero station (A2): the mechanic's tool cabinet rolled up to the
/// work spot, a pair of jack stands staged at the plate's edge, the air hose coiled where its
/// wall line drops, and a bucket under it. Every piece has its reason — this is the station's
/// working perimeter, not prop-bin filler (locked by `the_station_wears_human_scale`).
fn station_dressing(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    // Tool cabinet on casters, drawers facing the vehicle: rolled here for THIS job.
    let cabinet = v.len();
    let (cx, cz) = (7.0_f32, 2.2_f32);
    for (dx, dz) in [(-0.22_f32, -0.18_f32), (0.22, -0.18), (-0.22, 0.18), (0.22, 0.18)] {
        slab(v, i, [cx + dx, 0.03, cz + dz], [0.03, 0.03, 0.03], DARK_STEEL);
    }
    // Issued kit wears the equipment olive (C2), not shop-steel grey.
    slab(v, i, [cx, 0.58, cz], [0.30, 0.52, 0.26], OLIVE);
    for drawer_y in [0.35_f32, 0.62, 0.89] {
        slab(v, i, [cx - 0.32, drawer_y, cz], [0.02, 0.10, 0.22], DARK_STEEL);
    }
    // An enamelled cabinet answers the worklamps like the extinguisher bottle does, one step
    // duller — painted pressing, not mill plate.
    finish(&mut v[cabinet..], Finish { gloss: 0.28, ..Finish::PAINTED_STEEL });

    // Jack stands staged at the plate's west edge, ready for the next lift.
    let stands = v.len();
    for stand_z in [-2.0_f32, -0.9] {
        for (collar_y, half_w) in [(0.12_f32, 0.26_f32), (0.36, 0.18), (0.58, 0.10)] {
            slab(v, i, [-6.5, collar_y, stand_z], [half_w, 0.12, half_w], STEEL);
        }
    }
    finish(&mut v[stands..], Finish::MACHINED_STEEL);

    // The air hose: coiled flat where its wall line comes down by the bench, the run back to
    // the wall still attached — it is plumbing, not decor.
    let hose = v.len();
    push_cylinder(v, i, Vec3::new(7.8, 0.0, 3.2), 0.36, 0.05, 20, RUBBER);
    push_cylinder(v, i, Vec3::new(7.8, 0.05, 3.2), 0.27, 0.04, 20, RUBBER);
    let run_half = (HALF_X - SLAB - 7.8) / 2.0;
    slab(v, i, [7.8 + run_half, 0.028, 3.2], [run_half, 0.028, 0.035], RUBBER);
    finish(&mut v[hose..], Finish::RUBBER);

    // The bucket by the stands, on its own wear ring: it has stood there through many jobs.
    push_contact_ring(v, i, -6.3, 3.0, 0.24);
    let bucket = v.len();
    push_cylinder(v, i, Vec3::new(-6.3, 0.0, 3.0), 0.16, 0.32, 14, STEEL);
    finish(&mut v[bucket..], Finish { gloss: 0.30, ..Finish::MACHINED_STEEL });
}

/// A gantry crane spanning the nave just under the truss chords, riding the wall rails
/// (`hangar_gallery::push_crane_rails`), with a hoist block parked out past the turntable —
/// the workshop's signature overhead silhouette.
fn overhead_crane(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    let beam_y = CRANE_GIRDER_Y;
    let span = HALF_X - 1.5;
    // Main box girder across the bay (x-spanning), plus a lighter cross rail it rides on.
    slab(v, i, [0.0, beam_y, -1.6], [span, 0.28, 0.32], STEEL);
    slab(v, i, [0.0, beam_y + 0.34, -1.6], [span, 0.06, 0.5], DARK_STEEL);
    // Trolley + hoist block over the vehicle (A3: "the hook over the turret" — the crane is
    // mid-job, not parked in a corner). Everything hangs ABOVE the orbit column's 7.4 m
    // ceiling, so the shot stays clear while the silhouette reads in every high framing.
    slab(v, i, [1.5, beam_y - 0.2, -1.6], [0.5, 0.2, 0.5], DARK_STEEL);
    // The cable and hook SWAY (E2) — they hang free, and the hall has a draft (the gate is
    // ajar); the trolley above them is bolted to its rail and does not.
    let hanging = v.len();
    slab(v, i, [1.5, beam_y - 0.48, -1.6], [0.03, 0.08, 0.03], DARK_STEEL); // cable run
    slab(v, i, [1.5, beam_y - 0.68, -1.6], [0.15, 0.15, 0.15], STEEL); // hook block
    super::hangar::set_sway(&mut v[hanging..], 0.03);
    finish(&mut v[start..], Finish::MACHINED_STEEL);
}

/// Spare road wheels are FLEET parts, not prop-bin filler: the T-54's road wheel is 0.81 m
/// across (`docs/vehicles/t-54.md`), and a spare on the workshop floor is the same wheel that
/// hangs on the tank five metres away. The old stack was ⌀1.24 m — subliminally wrong next to
/// every hull in the hall. Locked by `spare_wheels_are_fleet_wheels`.
const SPARE_WHEEL_RADIUS_M: f32 = 0.405;
const SPARE_WHEEL_THICKNESS_M: f32 = 0.25;

/// A stack of spare road wheels lying flat: rubber tyre discs with a lighter hub, tapering up.
fn wheel_stack(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    push_contact_ring(v, i, x, z, SPARE_WHEEL_RADIUS_M + 0.08);
    for layer in 0..4 {
        let y = 0.02 + layer as f32 * (SPARE_WHEEL_THICKNESS_M + 0.01);
        let tyre = v.len();
        push_cylinder(
            v,
            i,
            Vec3::new(x, y, z),
            SPARE_WHEEL_RADIUS_M,
            SPARE_WHEEL_THICKNESS_M,
            20,
            RUBBER,
        );
        finish(&mut v[tyre..], Finish::RUBBER);
        let hub = v.len();
        push_cylinder(
            v,
            i,
            Vec3::new(x, y + SPARE_WHEEL_THICKNESS_M, z),
            0.15,
            0.03,
            14,
            WHEEL_HUB,
        );
        finish(&mut v[hub..], Finish::MACHINED_STEEL);
    }
}

/// A low heap of track links: staggered dark-steel blocks in the fleet's LINK dimensions —
/// a T-54 link is 0.58 m wide at a ~0.14 m pitch and less than a decimetre thick, so a pile
/// of them is a low, WIDE stack of plates, not a crate of bricks.
fn track_link_pile(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    let start = v.len();
    for row in 0..4 {
        for col in 0..4 {
            let jitter = ((row * 4 + col) as f32 * 0.37).sin() * 0.05;
            // Every fifth plate wears rust (C2): links come off vehicles weathered, and a
            // pile of them is never one uniform steel.
            let plate = if (row * 4 + col) % 5 == 3 { RUST } else { DARK_STEEL };
            slab(
                v,
                i,
                [x + col as f32 * 0.16 - 0.24, 0.045 + row as f32 * 0.09, z + jitter],
                [0.07, 0.045, 0.29],
                plate,
            );
        }
    }
    // Worn link steel: polished by the ground it ran on, not painted.
    finish(&mut v[start..], Finish::MACHINED_STEEL);
}

/// An ammunition rack flanking the drive lane by the gate (A3, the gun framing's background):
/// a steel A-stand with two sloped shelf rails and drab shell crates seated on them — the
/// stage where rounds come off the truck before they reach the stores rack.
fn ammo_rack(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    let rack = v.len();
    for dz in [-0.9_f32, 0.9] {
        for dx in [-0.45_f32, 0.45] {
            slab(v, i, [x + dx, 0.65, z + dz], [0.05, 0.65, 0.05], STEEL);
        }
    }
    for shelf_y in [0.42_f32, 0.98] {
        slab(v, i, [x, shelf_y, z], [0.55, 0.035, 1.05], STEEL);
    }
    finish(&mut v[rack..], Finish::MACHINED_STEEL);
    let stock = v.len();
    const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
    for (dy, dz, hh) in [(0.42_f32, -0.5_f32, 0.16_f32), (0.42, 0.45, 0.16), (0.98, -0.1, 0.14)] {
        slab(v, i, [x, dy + 0.035 + hh, z + dz], [0.5, hh, 0.42], CRATE);
    }
    finish(&mut v[stock..], Finish::TIMBER);
}

/// A workbench against the wall: a wooden top on steel legs.
fn workbench(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    let top = v.len();
    slab(v, i, [x, 0.92, z], [0.5, 0.06, 1.8], WOOD);
    // The bench top is sawn board and takes the world's plank treatment — the same one a barn
    // wall wears. A workshop bench being rendered as untreated fill was the plainest instance
    // of the hall having no materials at all.
    finish(&mut v[top..], Finish::TIMBER);
    let legs = v.len();
    for dz in [-1.6_f32, 1.6] {
        for dx in [-0.42_f32, 0.42] {
            slab(v, i, [x + dx, 0.45, z + dz], [0.05, 0.45, 0.05], STEEL);
        }
    }
    finish(&mut v[legs..], Finish::MACHINED_STEEL);
}

/// A standard 200-litre steel drum: ⌀0.57 m × 0.88 m tall, its two rolling hoops sitting
/// FLUSH on the shell (6 mm proud) at the real third-points. The old drums were ⌀0.68 with
/// hoops overhanging the shell by 2 cm — from the hero framing those unlit hoop undersides
/// read as dark ellipses floating around the can. One helper for every drum in the hall, so
/// a barrel cannot drift back into a caricature in one corner only.
fn push_drum(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const DRUM_RADIUS_M: f32 = 0.29;
    const DRUM_HEIGHT_M: f32 = 0.88;
    push_contact_ring(v, i, x, z, DRUM_RADIUS_M + 0.08);
    let can = v.len();
    push_cylinder(v, i, Vec3::new(x, 0.0, z), DRUM_RADIUS_M, DRUM_HEIGHT_M, 18, BARREL);
    for hoop_y in [0.28_f32, 0.56] {
        push_cylinder(v, i, Vec3::new(x, hoop_y, z), DRUM_RADIUS_M + 0.006, 0.06, 18, DARK_STEEL);
    }
    // A drum is a painted pressing, hoops and all.
    finish(&mut v[can..], Finish::PAINTED_STEEL);
}

/// The dark ring of wear and grime where a prop meets the concrete. Honest grounding — this
/// is dirt that accumulates around anything that stands in a workshop for months, not a faked
/// shadow (the sun draws the real shadows now); it keeps a prop's foot from reading as a
/// mathematically clean seam.
fn push_contact_ring(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32, radius: f32) {
    let start = v.len();
    push_cylinder(v, i, Vec3::new(x, 0.005, z), radius, 0.004, 18, CONTACT);
    // Grime lying ON the slab: it is the floor's material, darker.
    finish(&mut v[start..], Finish::CONCRETE);
}

/// A short row of fuel barrels standing against the left wall (`wall_x`).
fn barrels(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, wall_x: f32) {
    for (n, z) in [-3.4_f32, -2.6, -1.8].into_iter().enumerate() {
        let x = wall_x + (n as f32 * 0.05);
        push_drum(v, i, x, z);
    }
}

/// A pegboard tool wall behind the workbench: a timber board with a few hung-steel silhouettes.
/// One glance says "workshop"; scattered floor discs never did.
fn tool_board(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, z: f32) {
    let x = HALF_X - 0.30;
    let board = v.len();
    slab(v, i, [x, 1.7, z], [0.05, 0.65, 1.5], WOOD);
    finish(&mut v[board..], Finish::TIMBER);
    let tools = v.len();
    // Hung tools: two wrench bars, a hammer head, a coiled line block.
    slab(v, i, [x - 0.07, 1.9, z - 0.9], [0.02, 0.30, 0.05], STEEL);
    slab(v, i, [x - 0.07, 1.8, z - 0.45], [0.02, 0.38, 0.05], DARK_STEEL);
    slab(v, i, [x - 0.07, 1.95, z + 0.2], [0.02, 0.10, 0.22], STEEL);
    slab(v, i, [x - 0.07, 1.65, z + 0.85], [0.02, 0.18, 0.18], DARK_STEEL);
    finish(&mut v[tools..], Finish::MACHINED_STEEL);
}

/// A pallet of ammunition crates in the corner: drab boxes on skid timbers, stacked two-high.
fn crate_pallet(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    let start = v.len();
    const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
    const CRATE_LID: [f32; 3] = [0.26, 0.28, 0.21];
    for dz in [-0.4_f32, 0.5] {
        slab(v, i, [x, 0.07, z + dz], [0.85, 0.05, 0.16], WOOD);
    }
    for (dx, dz, dy) in [(-0.4_f32, 0.0_f32, 0.35_f32), (0.45, 0.05, 0.35), (0.0, 0.0, 0.95)] {
        slab(v, i, [x + dx, dy, z + dz], [0.42, 0.24, 0.55], CRATE);
        slab(v, i, [x + dx, dy + 0.26, z + dz], [0.44, 0.02, 0.57], CRATE_LID);
    }
    // Skids, boxes and lids: nailed board, drab-painted. Board all the way down.
    finish(&mut v[start..], Finish::TIMBER);
}

/// Two oil stains on the concrete by the workbench — where the work happens, not scattered
/// across the bay like manhole covers — plus one under the second bay's engine stand.
fn oil_stains(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, wall_x: f32) {
    let start = v.len();
    for (x, z, r) in [(wall_x - 1.6, 4.9_f32, 0.55_f32), (wall_x - 2.4, 6.6, 0.4)] {
        push_cylinder(v, i, Vec3::new(x, 0.006, z), r, 0.004, 22, OIL);
    }
    // Oil soaked INTO the slab: still the slab.
    finish(&mut v[start..], Finish::CONCRETE);
}

/// The second, occupied-looking maintenance bay: parking strips, an A-frame engine gantry with
/// a block slung under it on a chain, jack stands, a tool trolley and its own stain — evidence
/// the hall services a COMPANY, not one museum piece.
fn second_bay(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const MARKING: [f32; 3] = [0.62, 0.55, 0.20];
    let marks = v.len();
    for dx in [-1.6_f32, 1.6] {
        slab(v, i, [x + dx, 0.004, z], [0.12, 0.005, 3.0], MARKING);
    }
    finish(&mut v[marks..], Finish::PAINT_MARK);
    let rig = v.len();
    // A-frame gantry: two leg pairs and the top beam.
    for dz in [-1.3_f32, 1.3] {
        for dx in [-0.8_f32, 0.8] {
            slab(v, i, [x + dx, 1.55, z + dz], [0.07, 1.55, 0.07], STEEL);
        }
        slab(v, i, [x, 3.05, z + dz], [0.9, 0.07, 0.07], STEEL);
    }
    slab(v, i, [x, 3.16, z], [0.09, 0.09, 1.45], STEEL);
    // The chain hangs SLACK — the block already rests on its stand — so the chain sways
    // with the hall's draft (E2) while the block it lowered stays put.
    let chain = v.len();
    slab(v, i, [x, 2.35, z], [0.02, 0.72, 0.02], DARK_STEEL);
    super::hangar::set_sway(&mut v[chain..], 0.02);
    for dx in [-0.35_f32, 0.35] {
        slab(v, i, [x + dx, 0.35, z], [0.05, 0.35, 0.05], DARK_STEEL);
    }
    slab(v, i, [x, 1.05, z], [0.55, 0.35, 0.8], DARK_STEEL);
    for dx in [-0.28_f32, 0.28] {
        slab(v, i, [x + dx, 1.48, z], [0.16, 0.08, 0.7], WHEEL_HUB);
    }
    // Jack stands: three shrinking collars each.
    for stand_x in [x - 1.3, x + 1.3] {
        for (collar_y, half_w) in [(0.12_f32, 0.26_f32), (0.36, 0.18), (0.58, 0.10)] {
            slab(v, i, [stand_x, collar_y, z - 1.9], [half_w, 0.12, half_w], STEEL);
        }
    }
    // Tool trolley on stub wheels, with a push bar.
    slab(v, i, [x + 2.1, 0.55, z + 1.7], [0.45, 0.33, 0.3], STEEL);
    finish(&mut v[rig..], Finish::MACHINED_STEEL);
    let tyres = v.len();
    for (dx, dz) in [(-0.35_f32, -0.22_f32), (0.35, -0.22), (-0.35, 0.22), (0.35, 0.22)] {
        slab(v, i, [x + 2.1 + dx, 0.11, z + 1.7 + dz], [0.05, 0.11, 0.05], RUBBER);
    }
    finish(&mut v[tyres..], Finish::RUBBER);
    let bar = v.len();
    slab(v, i, [x + 2.6, 1.0, z + 1.7], [0.03, 0.14, 0.28], DARK_STEEL);
    finish(&mut v[bar..], Finish::MACHINED_STEEL);
    let stain = v.len();
    push_cylinder(v, i, Vec3::new(x - 0.4, 0.006, z + 0.9), 0.45, 0.004, 20, OIL);
    finish(&mut v[stain..], Finish::CONCRETE);
}

/// The rack's shelf CENTRE heights and half-thickness — the one place they live. The crates
/// compute their seat from these, so a shelf cannot move without its stock following (locked
/// by `rack_crates_sit_on_their_shelves`; they used to float 19–27 cm above the steel).
const RACK_SHELF_YS: [f32; 3] = [0.5, 1.4, 2.3];
const RACK_SHELF_HALF_H: f32 = 0.03;
/// Each crate on the stores rack as `(shelf index, z offset, x jog, half-depth, half-height)`.
/// The x jog breaks the stamped all-flush face without letting a crate overhang an upright.
const RACK_CRATES: [(usize, f32, f32, f32, f32); 7] = [
    (0, -1.4, -0.05, 0.5, 0.25),
    (0, -0.3, 0.04, 0.42, 0.19),
    (0, 0.9, 0.0, 0.55, 0.27),
    (1, -0.9, 0.06, 0.48, 0.23),
    (1, 0.4, -0.04, 0.38, 0.19),
    (2, -0.2, 0.02, 0.52, 0.23),
    (2, 1.2, -0.06, 0.4, 0.19),
];

/// The stores zone against the left wall: a shelving rack with crates, a tarped mound, and a
/// reserve barrel row — the quartermaster's corner.
fn stores_zone(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
    const TARP: [f32; 3] = [0.24, 0.26, 0.23];
    // Rack: four uprights ending just proud of the top shelf (a post sticking a metre into the
    // air above the last shelf read as scaffolding), three shelves.
    let rack = v.len();
    let post_top = RACK_SHELF_YS[2] + RACK_SHELF_HALF_H + 0.08;
    for dz in [-2.0_f32, 2.0] {
        for dx in [-0.7_f32, 0.7] {
            slab(v, i, [x + dx, post_top / 2.0, z + dz], [0.05, post_top / 2.0, 0.05], STEEL);
        }
    }
    for shelf_y in RACK_SHELF_YS {
        slab(v, i, [x, shelf_y, z], [0.75, RACK_SHELF_HALF_H, 2.1], STEEL);
    }
    finish(&mut v[rack..], Finish::MACHINED_STEEL);
    let stock = v.len();
    // Crates on the shelves, varied so the rack reads stocked, not stamped; every crate SEATS
    // on its shelf's top face.
    for (shelf, dz, dx, hw, hh) in RACK_CRATES {
        let seat = RACK_SHELF_YS[shelf] + RACK_SHELF_HALF_H;
        slab(v, i, [x + dx, seat + hh, z + dz], [0.55, hh, hw], CRATE);
    }
    finish(&mut v[stock..], Finish::TIMBER);
    // The tarped mound beside the rack. Proofed duck cloth: it takes the PLASTER treatment —
    // fine grain over half-metre blotches is what a folded tarpaulin reads as, and inventing a
    // twelfth role to say the same thing would be a role for one prop.
    let tarp = v.len();
    for (layer_y, hx, hz) in [(0.35_f32, 1.05_f32, 0.8_f32), (0.85, 0.85, 0.62), (1.2, 0.6, 0.45)] {
        slab(v, i, [x + 0.3, layer_y, z + 3.6], [hx, 0.36, hz], TARP);
    }
    finish(&mut v[tarp..], Finish::CANVAS);
    // Reserve barrels — the same honest drum as by the gate wall.
    for (n, dz) in [-3.2_f32, -3.9].into_iter().enumerate() {
        let bx = x - 0.9 + n as f32 * 0.15;
        push_drum(v, i, bx, z + dz);
    }
}

/// Fire extinguishers on wall plates — the hall's ONLY saturated red, by the gate and by the
/// workbench (locked by `the_extinguishers_are_the_only_saturated_red`).
fn extinguishers(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    const EXTINGUISHER: [f32; 3] = [0.55, 0.08, 0.06];
    for (x, z, along_x) in [(-5.6, -(HALF_Z - 0.45), false), (HALF_X - 0.45, 3.8, true)] {
        let plate_offset = 0.14;
        let (px, pz) = if along_x { (x + plate_offset, z) } else { (x, z - plate_offset) };
        let plate_half = if along_x { [0.02, 0.24, 0.14] } else { [0.14, 0.24, 0.02] };
        let start = v.len();
        slab(v, i, [px, 1.15, pz], plate_half, DARK_STEEL);
        push_cylinder(v, i, Vec3::new(x, 0.72, z), 0.11, 0.55, 14, EXTINGUISHER);
        slab(v, i, [x, 1.34, z], [0.045, 0.07, 0.045], DARK_STEEL);
        // Enamelled pressure bottle on its bracket: the glossiest paint in the hall, which is
        // half of why the eye finds the one red accent in a grey room.
        finish(&mut v[start..], Finish { gloss: 0.45, ..Finish::PAINTED_STEEL });
    }
}

#[cfg(test)]
mod tests {
    use super::super::hangar::hangar_scene_mesh;
    use super::*;

    /// Corner-shade multiplies all channels by one factor in `[0.8, 1.0]`, so hue identity
    /// survives the bake (mirror of the helper in `hangar::tests`).
    fn is_shade_of(color: [f32; 3], base: [f32; 3]) -> bool {
        let k = color[0] / base[0];
        (0.79..=1.001).contains(&k) && (0..3).all(|i| (color[i] - base[i] * k).abs() < 1.0e-4)
    }

    /// The five drums the hall stands (three along the left wall, two in stores), by the
    /// centres the builders use. A duplicated list, on purpose: moving a drum without moving
    /// its lock is exactly the drift this test exists to catch.
    const DRUMS: [(f32, f32); 5] =
        [(-9.3, -3.4), (-9.25, -2.6), (-9.2, -1.8), (-9.4, 13.8), (-9.25, 13.1)];

    #[test]
    fn spare_wheels_are_fleet_wheels() {
        // The T-54 road wheel is 0.81 m across; a spare on the floor is the SAME part that
        // hangs on the hull five metres away. The old ⌀1.24 m stack read subliminally wrong
        // against every tank in the hall.
        let (vertices, _) = hangar_scene_mesh();
        // Two stacks stand side by side; a vertex belongs to the NEARER axis, so the max of
        // that min-distance is exactly one wheel's radius.
        let stacks = [(-(HALF_X - 3.0), -(HALF_Z - 3.0)), (-(HALF_X - 3.0) - 0.7, -(HALF_Z - 3.0))];
        let radius = vertices
            .iter()
            .filter(|v| is_shade_of(v.color, RUBBER))
            .map(|v| {
                stacks
                    .iter()
                    .map(|(cx, cz)| {
                        let (dx, dz) = (v.position[0] - cx, v.position[2] - cz);
                        (dx * dx + dz * dz).sqrt()
                    })
                    .fold(f32::MAX, f32::min)
            })
            .filter(|r| *r < 1.0)
            .fold(0.0f32, f32::max);
        assert!(
            (radius - SPARE_WHEEL_RADIUS_M).abs() < 0.02,
            "the spare wheel is the fleet's 0.81 m wheel, measured radius {radius}"
        );
    }

    #[test]
    fn drum_hoops_hug_the_shell() {
        // The rolling hoops of a steel drum sit flush on the shell. The old hoops overhung by
        // 2 cm, and their unlit undersides read as dark ellipses floating around the can.
        let (vertices, _) = hangar_scene_mesh();
        for (cx, cz) in DRUMS {
            let max_r = vertices
                .iter()
                .filter(|v| v.position[1] > 0.05 && v.position[1] < 0.9)
                .map(|v| {
                    let (dx, dz) = (v.position[0] - cx, v.position[2] - cz);
                    (dx * dx + dz * dz).sqrt()
                })
                .filter(|r| *r < 0.40)
                .fold(0.0f32, f32::max);
            assert!(
                max_r <= 0.30,
                "drum at ({cx}, {cz}): nothing on the shell may overhang it, widest {max_r}"
            );
        }
    }

    #[test]
    fn standing_props_meet_the_floor_on_a_wear_ring() {
        // Honest grounding: the dark ring is dirt that accumulates around anything standing
        // on workshop concrete for months — not a faked shadow (the skylight sun draws the
        // real ones). Every drum and wheel stack must sit on one wider than its own foot.
        let (vertices, _) = hangar_scene_mesh();
        let ring_under = |cx: f32, cz: f32, foot: f32| {
            vertices.iter().filter(|v| is_shade_of(v.color, CONTACT) && v.position[1] < 0.02).any(
                |v| {
                    let (dx, dz) = (v.position[0] - cx, v.position[2] - cz);
                    let r = (dx * dx + dz * dz).sqrt();
                    (r - (foot + 0.08)).abs() < 0.03
                },
            )
        };
        for (cx, cz) in DRUMS {
            assert!(ring_under(cx, cz, 0.29), "the drum at ({cx}, {cz}) stands on a wear ring");
        }
        assert!(
            ring_under(-(HALF_X - 3.0), -(HALF_Z - 3.0), SPARE_WHEEL_RADIUS_M),
            "the wheel stack stands on a wear ring"
        );
    }

    /// The stock SITS on the steel: every rack crate's lowest vertex lies on its shelf's top
    /// face. Locks the floating-rack fix — the crates used to hover 19–27 cm above their
    /// shelves because their seats were hand-numbers strangers to the shelf heights.
    #[test]
    fn rack_crates_sit_on_their_shelves() {
        let (vertices, _) = hangar_scene_mesh();
        const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
        let (rack_x, rack_z) = (-(HALF_X - 2.5), 17.0);
        for (shelf, dz, dx, hw, hh) in RACK_CRATES {
            let seat = RACK_SHELF_YS[shelf] + RACK_SHELF_HALF_H;
            let (cx, cz) = (rack_x + dx, rack_z + dz);
            let band_top = seat + 2.0 * hh + 0.02;
            let bottom = vertices
                .iter()
                .filter(|v| {
                    is_shade_of(v.color, CRATE)
                        && (v.position[0] - cx).abs() <= 0.56
                        && (v.position[2] - cz).abs() <= hw + 0.005
                        && (seat - 0.02..band_top).contains(&v.position[1])
                })
                .map(|v| v.position[1])
                .fold(f32::INFINITY, f32::min);
            assert!(
                (bottom - seat).abs() < 0.01,
                "the crate at dz {dz} must sit on shelf {shelf}: bottom {bottom}, shelf top {seat}"
            );
        }
    }

    #[test]
    fn the_track_link_pile_is_a_pile_of_links_not_bricks() {
        // A T-54 link is 0.58 m wide, ~0.14 m at the pitch and under a decimetre thick: the
        // pile reads as low, WIDE plates.
        let (vertices, _) = hangar_scene_mesh();
        let (cx, cz) = (HALF_X - 3.0, -(HALF_Z - 3.0));
        let pile: Vec<_> = vertices
            .iter()
            .filter(|v| {
                is_shade_of(v.color, DARK_STEEL)
                    && (v.position[0] - cx).abs() < 1.0
                    && (v.position[2] - cz).abs() < 1.0
            })
            .collect();
        assert!(!pile.is_empty(), "the link pile exists");
        let z_span = pile.iter().map(|v| v.position[2] - cz).fold(0.0f32, |a, b| a.max(b.abs()));
        let height = pile.iter().map(|v| v.position[1]).fold(0.0f32, f32::max);
        assert!(z_span >= 0.28, "links are track-width plates, got half-span {z_span}");
        assert!(height <= 0.45, "the heap stays low, got {height}");
    }
}
