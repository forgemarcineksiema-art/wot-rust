//! The garage hangar: a single static interior scene the owned tank is parked in, replacing the
//! battlefield while the garage is open. Modelled the beta-WoT way — an enclosed industrial shed
//! (concrete floor, metal walls, dark roof with bright skylight strips) on a turntable spot. The
//! room is built from *solid slabs* surrounding the play volume: each slab's inner surface is an
//! ordinary outward-facing face, so back-face culling keeps exactly the walls seen from inside.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;

use crate::tank_mesh::push_oriented_box;

/// Half-width/half-depth of the hangar interior, metres (room is `2 * HALF` on a side).
const HALF: f32 = 13.0;
/// Interior height from floor (y=0) to ceiling, metres.
const WALL_HEIGHT: f32 = 8.0;
/// Slab thickness for floor/walls/ceiling, metres.
const SLAB: f32 = 0.15;
/// Top surface of the turntable the tank rests on, metres above the floor.
pub const TURNTABLE_TOP_M: f32 = 0.12;
/// Radius of the turntable disc, metres.
const TURNTABLE_RADIUS_M: f32 = 5.2;

const CONCRETE: [f32; 3] = [0.34, 0.34, 0.36];
const METAL: [f32; 3] = [0.26, 0.27, 0.30];
const ROOF: [f32; 3] = [0.15, 0.15, 0.17];
const TURNTABLE: [f32; 3] = [0.44, 0.42, 0.39];
const SKYLIGHT: [f32; 3] = [0.72, 0.74, 0.80];

/// Pivot the garage orbit camera looks at: roughly the centre of a parked tank.
pub fn hangar_camera_pivot() -> Vec3 {
    Vec3::new(0.0, TURNTABLE_TOP_M + 1.5, 0.0)
}

/// Build the static hangar mesh. The tank is parked at the origin on top of the turntable
/// (`TURNTABLE_TOP_M`), so place the parked vehicle's `position.y` at that height.
pub fn hangar_scene_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let id = Mat3::IDENTITY;

    // Floor slab — top surface flush with y = 0.
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(0.0, -SLAB, 0.0),
        Vec3::new(HALF, SLAB, HALF),
        id,
        CONCRETE,
    );
    // Ceiling slab — bottom surface flush with y = WALL_HEIGHT.
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(0.0, WALL_HEIGHT + SLAB, 0.0),
        Vec3::new(HALF, SLAB, HALF),
        id,
        ROOF,
    );
    // Four wall slabs sitting just outside the interior; their inner faces look into the room.
    let half_h = WALL_HEIGHT / 2.0;
    let mid_y = half_h;
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(0.0, mid_y, -HALF),
        Vec3::new(HALF, half_h, SLAB),
        id,
        METAL,
    );
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(0.0, mid_y, HALF),
        Vec3::new(HALF, half_h, SLAB),
        id,
        METAL,
    );
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-HALF, mid_y, 0.0),
        Vec3::new(SLAB, half_h, HALF),
        id,
        METAL,
    );
    push_oriented_box(
        &mut vertices,
        &mut indices,
        Vec3::new(HALF, mid_y, 0.0),
        Vec3::new(SLAB, half_h, HALF),
        id,
        METAL,
    );

    // Bright skylight strips under the roof so the dark ceiling reads as lit from above.
    for x in [-6.0_f32, 0.0, 6.0] {
        push_oriented_box(
            &mut vertices,
            &mut indices,
            Vec3::new(x, WALL_HEIGHT - 0.05, 0.0),
            Vec3::new(1.6, 0.03, HALF - 2.0),
            id,
            SKYLIGHT,
        );
    }

    // The turntable the tank parks on.
    push_cylinder(
        &mut vertices,
        &mut indices,
        Vec3::new(0.0, 0.0, 0.0),
        TURNTABLE_RADIUS_M,
        TURNTABLE_TOP_M,
        48,
        TURNTABLE,
    );

    (vertices, indices)
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

    // Top cap as a triangle fan around the centre.
    let center_index = vertices.len() as u32;
    vertices.push(SceneVertex::new([base_center.x, top_y, base_center.z], up, color));
    let rim_start = vertices.len() as u32;
    for s in 0..segments {
        let theta = s as f32 / segments as f32 * std::f32::consts::TAU;
        let (sin, cos) = theta.sin_cos();
        let x = base_center.x + radius * cos;
        let z = base_center.z + radius * sin;
        vertices.push(SceneVertex::new([x, top_y, z], up, color));
    }
    for s in 0..segments {
        let a = rim_start + s;
        let b = rim_start + (s + 1) % segments;
        indices.extend_from_slice(&[center_index, b, a]);
    }

    // Side ring: one outward quad per segment.
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
        // Geometry must exist on every side of the parked tank and below/above it, or the room
        // would not read as an enclosed shed around the vehicle.
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
