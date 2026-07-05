//! Workshop props for the garage hangar: an overhead crane, stacks of spare road wheels and track
//! links, a workbench, fuel barrels, and oil stains on the floor. All procedural boxes and
//! cylinders, kept outside the turntable and the camera's closest orbit so nothing clips the hero
//! vehicle or the lens.

use glam::Vec3;
use renderer_api::SceneVertex;

use super::hangar::{HALF, WALL_HEIGHT, push_cylinder, slab};

const STEEL: [f32; 3] = [0.24, 0.25, 0.27];
const DARK_STEEL: [f32; 3] = [0.16, 0.17, 0.18];
const RUBBER: [f32; 3] = [0.11, 0.11, 0.12];
const WHEEL_HUB: [f32; 3] = [0.30, 0.28, 0.24];
const WOOD: [f32; 3] = [0.34, 0.26, 0.17];
const BARREL: [f32; 3] = [0.30, 0.34, 0.30];
const OIL: [f32; 3] = [0.05, 0.045, 0.04];

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
    barrels(v, i, -wall);
    oil_stains(v, i);
}

/// A gantry crane spanning the bay just under the trusses, with a hoist block hanging over the
/// turntable — the workshop's signature overhead silhouette.
fn overhead_crane(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let beam_y = WALL_HEIGHT - 2.5;
    let span = HALF - 1.5;
    // Main box girder across the bay (x-spanning), plus a lighter cross rail it rides on.
    slab(v, i, [0.0, beam_y, -1.6], [span, 0.28, 0.32], STEEL);
    slab(v, i, [0.0, beam_y + 0.34, -1.6], [span, 0.06, 0.5], DARK_STEEL);
    // Trolley + hoist block, offset off-centre so it does not sit dead over the hero vehicle.
    slab(v, i, [3.2, beam_y - 0.2, -1.6], [0.5, 0.2, 0.5], DARK_STEEL);
    slab(v, i, [3.2, beam_y - 1.1, -1.6], [0.14, 0.7, 0.14], DARK_STEEL); // cable run
    slab(v, i, [3.2, beam_y - 1.9, -1.6], [0.28, 0.22, 0.28], STEEL); // hook block
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

/// Flat oil stains on the concrete just off the turntable, breaking up the clean floor.
fn oil_stains(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    for (x, z, r) in [(6.4_f32, 2.2_f32, 1.1_f32), (-5.8, -3.0, 0.8), (4.2, -5.5, 0.6)] {
        push_cylinder(v, i, Vec3::new(x, 0.006, z), r, 0.004, 22, OIL);
    }
}
