//! The garage hangar: a single static interior scene the owned tank is parked in, replacing the
//! battlefield while the garage is open. Modelled the beta-WoT way — an enclosed industrial shed
//! (concrete floor, ribbed metal walls, roof trusses, a bright doorway in back, parking-bay floor
//! markings, a turntable spot, and a soft ground shadow). The room is built from *solid slabs*
//! surrounding the play volume: each slab's inner surface is an ordinary outward-facing face, so
//! back-face culling keeps exactly the walls seen from inside. Colours run warm; the renderer's
//! per-scene tint (set in `garage_render`) pushes the whole scene amber to match the reference.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;

use crate::tank_mesh::push_oriented_box;

const HALF: f32 = 13.0;
const WALL_HEIGHT: f32 = 8.0;
const SLAB: f32 = 0.15;
/// Top surface of the turntable the tank rests on, metres above the floor.
pub const TURNTABLE_TOP_M: f32 = 0.12;
const TURNTABLE_RADIUS_M: f32 = 5.2;

const CONCRETE: [f32; 3] = [0.30, 0.27, 0.24];
const METAL: [f32; 3] = [0.26, 0.24, 0.22];
const RIB: [f32; 3] = [0.33, 0.30, 0.26];
const ROOF: [f32; 3] = [0.14, 0.12, 0.11];
const TRUSS: [f32; 3] = [0.20, 0.18, 0.16];
const TURNTABLE: [f32; 3] = [0.42, 0.37, 0.31];
const SKYLIGHT: [f32; 3] = [1.0, 0.92, 0.72];
const DOORWAY: [f32; 3] = [1.0, 0.95, 0.82];
const MARKING: [f32; 3] = [0.74, 0.58, 0.20];
const SHADOW: [f32; 3] = [0.05, 0.04, 0.03];

/// Pivot the garage orbit camera looks at: roughly the centre of a parked tank.
pub fn hangar_camera_pivot() -> Vec3 {
    Vec3::new(0.0, TURNTABLE_TOP_M + 1.3, 0.0)
}

/// Build the static hangar mesh. The tank is parked at the origin on top of the turntable
/// (`TURNTABLE_TOP_M`), so place the parked vehicle's `position.y` at that height.
pub fn hangar_scene_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let h = WALL_HEIGHT / 2.0;

    // Shell: floor, ceiling, four walls (inner faces look into the room).
    slab(&mut v, &mut i, [0.0, -SLAB, 0.0], [HALF, SLAB, HALF], CONCRETE);
    slab(&mut v, &mut i, [0.0, WALL_HEIGHT + SLAB, 0.0], [HALF, SLAB, HALF], ROOF);
    slab(&mut v, &mut i, [0.0, h, -HALF], [HALF, h, SLAB], METAL);
    slab(&mut v, &mut i, [0.0, h, HALF], [HALF, h, SLAB], METAL);
    slab(&mut v, &mut i, [-HALF, h, 0.0], [SLAB, h, HALF], METAL);
    slab(&mut v, &mut i, [HALF, h, 0.0], [SLAB, h, HALF], METAL);

    // Vertical wall ribs (pilasters) proud of the side and back walls.
    for z in [-9.0_f32, -4.5, 0.0, 4.5, 9.0] {
        slab(&mut v, &mut i, [-(HALF - 0.2), h, z], [0.12, h - 0.4, 0.35], RIB);
        slab(&mut v, &mut i, [HALF - 0.2, h, z], [0.12, h - 0.4, 0.35], RIB);
    }
    for x in [-9.0_f32, -4.5, 4.5, 9.0] {
        slab(&mut v, &mut i, [x, h, -(HALF - 0.2)], [0.35, h - 0.4, 0.12], RIB);
    }

    // Roof trusses spanning the bay, and bright skylight strips below them.
    for z in [-8.0_f32, -4.0, 0.0, 4.0, 8.0] {
        slab(&mut v, &mut i, [0.0, WALL_HEIGHT - 0.3, z], [HALF - 0.5, 0.12, 0.18], TRUSS);
    }
    for x in [-6.0_f32, 0.0, 6.0] {
        slab(&mut v, &mut i, [x, WALL_HEIGHT - 0.05, 0.0], [1.4, 0.03, HALF - 3.0], SKYLIGHT);
    }

    // A bright doorway in the back wall: daylight pouring in behind the tank.
    slab(&mut v, &mut i, [0.0, 3.6, -(HALF - 0.28)], [5.0, 2.4, 0.05], DOORWAY);

    // Parking-bay markings flanking the turntable, flush with the floor.
    for x in [-6.2_f32, 6.2] {
        slab(&mut v, &mut i, [x, 0.004, 0.0], [0.14, 0.005, HALF - 2.0], MARKING);
    }

    // Turntable + a soft shadow disc grounding the tank on it.
    push_cylinder(&mut v, &mut i, Vec3::ZERO, TURNTABLE_RADIUS_M, TURNTABLE_TOP_M, 48, TURNTABLE);
    push_cylinder(
        &mut v,
        &mut i,
        Vec3::new(0.0, TURNTABLE_TOP_M + 0.002, 0.0),
        3.6,
        0.002,
        40,
        SHADOW,
    );

    (v, i)
}

/// An axis-aligned solid box (every face winds CCW outward for back-face culling).
fn slab(vertices: &mut Vec<SceneVertex>, indices: &mut Vec<u32>, center: [f32; 3], half: [f32; 3], color: [f32; 3]) {
    push_oriented_box(vertices, indices, Vec3::from_array(center), Vec3::from_array(half), Mat3::IDENTITY, color);
}

/// A low cylinder resting on the floor: a top cap (normal +Y) plus an outward-facing side ring.
/// The bottom is omitted — it sits on the floor slab and is never seen.
fn push_cylinder(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    base_center: Vec3,
    radius: f32,
    height: f32,
    segments: u32,
    color: [f32; 3],
) {
    let top_y = base_center.y + height;
    let up = [0.0, 1.0, 0.0];

    let center_index = vertices.len() as u32;
    vertices.push(SceneVertex::new([base_center.x, top_y, base_center.z], up, color));
    let rim_start = vertices.len() as u32;
    for s in 0..segments {
        let theta = s as f32 / segments as f32 * std::f32::consts::TAU;
        let (sin, cos) = theta.sin_cos();
        vertices.push(SceneVertex::new(
            [base_center.x + radius * cos, top_y, base_center.z + radius * sin],
            up,
            color,
        ));
    }
    for s in 0..segments {
        let a = rim_start + s;
        let b = rim_start + (s + 1) % segments;
        indices.extend_from_slice(&[center_index, b, a]);
    }

    for s in 0..segments {
        let t0 = s as f32 / segments as f32 * std::f32::consts::TAU;
        let t1 = (s + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();
        let p_top_0 = [base_center.x + radius * c0, top_y, base_center.z + radius * s0];
        let p_top_1 = [base_center.x + radius * c1, top_y, base_center.z + radius * s1];
        let p_bot_0 = [base_center.x + radius * c0, base_center.y, base_center.z + radius * s0];
        let p_bot_1 = [base_center.x + radius * c1, base_center.y, base_center.z + radius * s1];
        let n0 = [c0, 0.0, s0];
        let n1 = [c1, 0.0, s1];
        let base = vertices.len() as u32;
        vertices.push(SceneVertex::new(p_bot_0, n0, color));
        vertices.push(SceneVertex::new(p_bot_1, n1, color));
        vertices.push(SceneVertex::new(p_top_1, n1, color));
        vertices.push(SceneVertex::new(p_top_0, n0, color));
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hangar_mesh_is_nonempty_and_indices_are_in_range() {
        let (vertices, indices) = hangar_scene_mesh();
        assert!(!vertices.is_empty() && !indices.is_empty());
        assert_eq!(indices.len() % 3, 0, "triangle list");
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
    }

    #[test]
    fn hangar_encloses_the_parked_tank() {
        let (vertices, _) = hangar_scene_mesh();
        let pivot = hangar_camera_pivot();
        let any = |pred: fn(&[f32; 3]) -> bool| vertices.iter().any(|v| pred(&v.position));
        assert!(any(|p| p[0] < -HALF + 1.0), "left wall");
        assert!(any(|p| p[0] > HALF - 1.0), "right wall");
        assert!(any(|p| p[2] < -HALF + 1.0), "back wall");
        assert!(any(|p| p[2] > HALF - 1.0), "front wall");
        assert!(any(|p| p[1] <= 0.0), "floor at or below the tank");
        assert!(any(|p| p[1] >= WALL_HEIGHT - 0.5), "ceiling above the tank");
        assert!(pivot.y > 0.0 && pivot.y < WALL_HEIGHT, "camera pivot sits inside the room");
    }
}
