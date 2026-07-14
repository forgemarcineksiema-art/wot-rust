//! Workshop props for the garage hangar: an overhead crane, stacks of spare road wheels and track
//! links, a workbench, fuel barrels, and oil stains on the floor. All procedural boxes and
//! cylinders, kept outside the turntable and the camera's closest orbit so nothing clips the hero
//! vehicle or the lens.

use glam::Vec3;
use renderer_api::SceneVertex;

use super::hangar::{HALF, WALL_HEIGHT, push_cylinder, slab, slab_finished};

const STEEL: [f32; 3] = [0.24, 0.25, 0.27];
const DARK_STEEL: [f32; 3] = [0.16, 0.17, 0.18];
const RUBBER: [f32; 3] = [0.11, 0.11, 0.12];
const WHEEL_HUB: [f32; 3] = [0.30, 0.28, 0.24];
const WOOD: [f32; 3] = [0.34, 0.26, 0.17];
const BARREL: [f32; 3] = [0.30, 0.34, 0.30];
// A darkened-concrete stain, not a void: against the hero-lit floor a near-black disc read as a
// hole/pit. Kept a clear step below the concrete (0.26) so it still registers as oil, but close
// enough that it stains the floor rather than cutting through it.
const OIL: [f32; 3] = [0.20, 0.19, 0.165];

/// Append every workshop prop to the hangar mesh. Wall-side props sit near the shell so the bay
/// floor around the turntable stays clear for the hero vehicle.
pub(super) fn push_props(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    // Wall-side props are anchored a fixed offset in from the shell, so they follow the wall at any
    // hall size instead of floating mid-floor.
    let wall = HALF - 1.7;
    let corner = HALF - 3.0;
    overhead_crane(v, i);
    wheel_stack(v, i, -corner, -corner);
    wheel_stack(v, i, -corner - 0.7, -corner);
    track_link_pile(v, i, corner, -corner);
    workbench(v, i, wall, 6.0);
    tool_board(v, i, 6.0);
    crate_pallet(v, i, corner, corner);
    barrels(v, i, -wall);
    oil_stains(v, i, wall);
    second_bay(v, i, 10.5, 9.5);
    stores_zone(v, i, -(HALF - 2.5), 10.5);
    extinguishers(v, i);
}

/// A gantry crane spanning the bay just under the trusses, with a hoist block hanging over the
/// turntable — the workshop's signature overhead silhouette.
fn overhead_crane(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let beam_y = WALL_HEIGHT - 2.5;
    let span = HALF - 1.5;
    // Main box girder across the bay (x-spanning), plus a lighter cross rail it rides on.
    slab(v, i, [0.0, beam_y, -1.6], [span, 0.28, 0.32], STEEL);
    slab(v, i, [0.0, beam_y + 0.34, -1.6], [span, 0.06, 0.5], DARK_STEEL);
    // Trolley + hoist block, parked out past the turntable so the orbit stays clear (the hook
    // hangs below the camera's headroom — locked by `nothing_invades_the_orbit_or_the_drive_lane`).
    slab(v, i, [6.4, beam_y - 0.2, -1.6], [0.5, 0.2, 0.5], DARK_STEEL);
    slab(v, i, [6.4, beam_y - 1.1, -1.6], [0.14, 0.7, 0.14], DARK_STEEL); // cable run
    slab(v, i, [6.4, beam_y - 1.9, -1.6], [0.28, 0.22, 0.28], STEEL); // hook block
}

/// A stack of spare road wheels lying flat: rubber tyre discs with a lighter hub, tapering up.
fn wheel_stack(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    for layer in 0..4 {
        let y = 0.16 + layer as f32 * 0.30;
        push_cylinder(v, i, Vec3::new(x, y, z), 0.62, 0.24, 20, RUBBER);
        push_cylinder(v, i, Vec3::new(x, y + 0.24, z), 0.22, 0.03, 14, WHEEL_HUB);
    }
}

/// A low heap of track links: staggered dark-steel blocks.
fn track_link_pile(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    for row in 0..3 {
        for col in 0..4 {
            let jitter = ((row * 4 + col) as f32 * 0.37).sin() * 0.05;
            slab(
                v,
                i,
                [x + col as f32 * 0.26 - 0.4, 0.09 + row as f32 * 0.16, z + jitter],
                [0.12, 0.08, 0.20],
                DARK_STEEL,
            );
        }
    }
}

/// A workbench against the wall: a wooden top on steel legs.
fn workbench(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    slab(v, i, [x, 0.92, z], [0.5, 0.06, 1.8], WOOD);
    for dz in [-1.6_f32, 1.6] {
        for dx in [-0.42_f32, 0.42] {
            slab(v, i, [x + dx, 0.45, z + dz], [0.05, 0.45, 0.05], STEEL);
        }
    }
}

/// A short row of fuel barrels standing against the left wall (`wall_x`).
fn barrels(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, wall_x: f32) {
    for (n, z) in [-3.4_f32, -2.6, -1.8].into_iter().enumerate() {
        let x = wall_x + (n as f32 * 0.05);
        push_cylinder(v, i, Vec3::new(x, 0.0, z), 0.34, 0.92, 18, BARREL);
        // Two rim rings read the barrel as steel, not a plain can.
        push_cylinder(v, i, Vec3::new(x, 0.28, z), 0.36, 0.04, 18, DARK_STEEL);
        push_cylinder(v, i, Vec3::new(x, 0.62, z), 0.36, 0.04, 18, DARK_STEEL);
    }
}

/// A pegboard tool wall behind the workbench: a timber board with a few hung-steel silhouettes.
/// One glance says "workshop"; scattered floor discs never did.
fn tool_board(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, z: f32) {
    let x = HALF - 0.30;
    slab(v, i, [x, 1.7, z], [0.05, 0.65, 1.5], WOOD);
    // Hung tools: two wrench bars, a hammer head, a coiled line block.
    slab(v, i, [x - 0.07, 1.9, z - 0.9], [0.02, 0.30, 0.05], STEEL);
    slab(v, i, [x - 0.07, 1.8, z - 0.45], [0.02, 0.38, 0.05], DARK_STEEL);
    slab(v, i, [x - 0.07, 1.95, z + 0.2], [0.02, 0.10, 0.22], STEEL);
    slab(v, i, [x - 0.07, 1.65, z + 0.85], [0.02, 0.18, 0.18], DARK_STEEL);
}

/// A pallet of ammunition crates in the corner: drab boxes on skid timbers, stacked two-high.
fn crate_pallet(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
    const CRATE_LID: [f32; 3] = [0.26, 0.28, 0.21];
    for dz in [-0.4_f32, 0.5] {
        slab(v, i, [x, 0.07, z + dz], [0.85, 0.05, 0.16], WOOD);
    }
    for (dx, dz, dy) in [(-0.4_f32, 0.0_f32, 0.35_f32), (0.45, 0.05, 0.35), (0.0, 0.0, 0.95)] {
        slab(v, i, [x + dx, dy, z + dz], [0.42, 0.24, 0.55], CRATE);
        slab(v, i, [x + dx, dy + 0.26, z + dz], [0.44, 0.02, 0.57], CRATE_LID);
    }
}

/// Two oil stains on the concrete by the workbench — where the work happens, not scattered
/// across the bay like manhole covers — plus one under the second bay's engine stand.
fn oil_stains(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, wall_x: f32) {
    for (x, z, r) in [(wall_x - 1.6, 4.9_f32, 0.55_f32), (wall_x - 2.4, 6.6, 0.4)] {
        push_cylinder(v, i, Vec3::new(x, 0.006, z), r, 0.004, 22, OIL);
    }
}

/// The second, occupied-looking maintenance bay: parking strips, an A-frame engine gantry with
/// a block slung under it on a chain, jack stands, a tool trolley and its own stain — evidence
/// the hall services a COMPANY, not one museum piece.
fn second_bay(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const MARKING: [f32; 3] = [0.62, 0.55, 0.20];
    for dx in [-1.6_f32, 1.6] {
        slab(v, i, [x + dx, 0.004, z], [0.12, 0.005, 3.0], MARKING);
    }
    // A-frame gantry: two leg pairs and the top beam.
    for dz in [-1.3_f32, 1.3] {
        for dx in [-0.8_f32, 0.8] {
            slab(v, i, [x + dx, 1.55, z + dz], [0.07, 1.55, 0.07], STEEL);
        }
        slab(v, i, [x, 3.05, z + dz], [0.9, 0.07, 0.07], STEEL);
    }
    slab_finished(v, i, [x, 3.16, z], [0.09, 0.09, 1.45], STEEL, 0.25);
    // The chain and the engine block it holds over the stand.
    slab(v, i, [x, 2.35, z], [0.02, 0.72, 0.02], DARK_STEEL);
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
    slab_finished(v, i, [x + 2.1, 0.55, z + 1.7], [0.45, 0.33, 0.3], STEEL, 0.2);
    for (dx, dz) in [(-0.35_f32, -0.22_f32), (0.35, -0.22), (-0.35, 0.22), (0.35, 0.22)] {
        slab(v, i, [x + 2.1 + dx, 0.11, z + 1.7 + dz], [0.05, 0.11, 0.05], RUBBER);
    }
    slab(v, i, [x + 2.6, 1.0, z + 1.7], [0.03, 0.14, 0.28], DARK_STEEL);
    push_cylinder(v, i, Vec3::new(x - 0.4, 0.006, z + 0.9), 0.45, 0.004, 20, OIL);
}

/// The stores zone against the left wall: a shelving rack with crates, a tarped mound, and a
/// reserve barrel row — the quartermaster's corner.
fn stores_zone(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, x: f32, z: f32) {
    const CRATE: [f32; 3] = [0.295, 0.315, 0.235];
    const TARP: [f32; 3] = [0.24, 0.26, 0.23];
    // Rack: four uprights, three shelves.
    for dz in [-2.0_f32, 2.0] {
        for dx in [-0.7_f32, 0.7] {
            slab(v, i, [x + dx, 1.5, z + dz], [0.05, 1.5, 0.05], STEEL);
        }
    }
    for shelf_y in [0.5_f32, 1.4, 2.3] {
        slab(v, i, [x, shelf_y, z], [0.75, 0.03, 2.1], STEEL);
    }
    // Crates on the shelves, varied so the rack reads stocked, not stamped.
    for (dy, dz, hw, hh) in [
        (0.78_f32, -1.4_f32, 0.5_f32, 0.25_f32),
        (0.72, -0.3, 0.42, 0.19),
        (0.80, 0.9, 0.55, 0.27),
        (1.66, -0.9, 0.48, 0.23),
        (1.62, 0.4, 0.38, 0.19),
        (2.56, -0.2, 0.52, 0.23),
        (2.52, 1.2, 0.4, 0.19),
    ] {
        slab(v, i, [x, dy + hh, z + dz], [0.55, hh, hw], CRATE);
    }
    // The tarped mound beside the rack.
    for (layer_y, hx, hz) in [(0.35_f32, 1.05_f32, 0.8_f32), (0.85, 0.85, 0.62), (1.2, 0.6, 0.45)] {
        slab_finished(v, i, [x + 0.3, layer_y, z + 3.6], [hx, 0.36, hz], TARP, 0.1);
    }
    // Reserve barrels.
    for (n, dz) in [-3.2_f32, -3.9].into_iter().enumerate() {
        let bx = x - 0.9 + n as f32 * 0.15;
        push_cylinder(v, i, Vec3::new(bx, 0.0, z + dz), 0.34, 0.92, 18, BARREL);
        push_cylinder(v, i, Vec3::new(bx, 0.28, z + dz), 0.36, 0.04, 18, DARK_STEEL);
        push_cylinder(v, i, Vec3::new(bx, 0.62, z + dz), 0.36, 0.04, 18, DARK_STEEL);
    }
}

/// Fire extinguishers on wall plates — the hall's ONLY saturated red, by the gate and by the
/// workbench (locked by `the_extinguishers_are_the_only_saturated_red`).
fn extinguishers(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    const EXTINGUISHER: [f32; 3] = [0.55, 0.08, 0.06];
    for (x, z, along_x) in [(-5.6, -(HALF - 0.45), false), (HALF - 0.45, 3.8, true)] {
        let plate_offset = 0.14;
        let (px, pz) = if along_x { (x + plate_offset, z) } else { (x, z - plate_offset) };
        let plate_half = if along_x { [0.02, 0.24, 0.14] } else { [0.14, 0.24, 0.02] };
        slab(v, i, [px, 1.15, pz], plate_half, DARK_STEEL);
        let start = v.len();
        push_cylinder(v, i, Vec3::new(x, 0.72, z), 0.11, 0.55, 14, EXTINGUISHER);
        for vertex in &mut v[start..] {
            vertex.gloss = 0.45;
        }
        slab(v, i, [x, 1.34, z], [0.045, 0.07, 0.045], DARK_STEEL);
    }
}
