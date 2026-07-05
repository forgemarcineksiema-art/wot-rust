//! The garage hangar: a single static interior scene the owned tank is parked in, replacing the
//! battlefield while the garage is open. Modelled as a working repair shop — a cool concrete floor,
//! ribbed metal walls with near-black upper reaches, roof trusses under bright skylight strips, a
//! cold-daylight doorway in back, a turntable spot, and workshop props (`hangar_props`: crane,
//! wheel/track stacks, workbench, barrels, oil stains). The room is built from *solid slabs*
//! surrounding the play volume: each slab's inner surface is an ordinary outward-facing face, so
//! back-face culling keeps exactly the walls seen from inside. The hero vehicle throws a real
//! contact shadow on the turntable from the workshop sun key — no faked shadow disc.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;

use crate::tank_mesh::push_oriented_box;

const HALF: f32 = 13.0;
const WALL_HEIGHT: f32 = 8.0;
const SLAB: f32 = 0.15;
/// Top surface of the turntable the tank rests on, metres above the floor.
pub const TURNTABLE_TOP_M: f32 = 0.12;
const TURNTABLE_RADIUS_M: f32 = 5.2;

// Cooler, more neutral workshop palette than the old amber shed: bare concrete and gunmetal, with
// the upper walls falling to near-black so the lit panels and the hero vehicle carry the frame.
const CONCRETE: [f32; 3] = [0.26, 0.26, 0.27];
const METAL: [f32; 3] = [0.20, 0.21, 0.23];
const UPPER_WALL: [f32; 3] = [0.10, 0.10, 0.12];
const RIB: [f32; 3] = [0.27, 0.28, 0.30];
const ROOF: [f32; 3] = [0.09, 0.09, 0.10];
const TRUSS: [f32; 3] = [0.15, 0.15, 0.16];
const TURNTABLE: [f32; 3] = [0.33, 0.33, 0.34];
/// Skylight strips run hot so the tone curve blooms them into daylight pouring through the roof.
const SKYLIGHT: [f32; 3] = [1.5, 1.5, 1.35];
/// Cold daylight in the back doorway, opposing the warm key.
const DOORWAY: [f32; 3] = [0.86, 0.94, 1.05];
const MARKING: [f32; 3] = [0.62, 0.55, 0.20];

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

    // Shell: floor, ceiling, four walls. The lower walls are gunmetal; a near-black upper band
    // above the doorway line lets the roof fall into shadow so the lit bay reads as the subject.
    slab(&mut v, &mut i, [0.0, -SLAB, 0.0], [HALF, SLAB, HALF], CONCRETE);
    slab(&mut v, &mut i, [0.0, WALL_HEIGHT + SLAB, 0.0], [HALF, SLAB, HALF], ROOF);
    for (cz, hz) in [(-HALF, SLAB), (HALF, SLAB)] {
        slab(&mut v, &mut i, [0.0, 2.4, cz], [HALF, 2.4, hz], METAL);
        slab(&mut v, &mut i, [0.0, 5.8, cz], [HALF, 1.6, hz], UPPER_WALL);
    }
    for (cx, hx) in [(-HALF, SLAB), (HALF, SLAB)] {
        slab(&mut v, &mut i, [cx, 2.4, 0.0], [hx, 2.4, HALF], METAL);
        slab(&mut v, &mut i, [cx, 5.8, 0.0], [hx, 1.6, HALF], UPPER_WALL);
    }

    // Vertical wall ribs (pilasters) proud of the side and back walls.
    for z in [-9.0_f32, -4.5, 0.0, 4.5, 9.0] {
        slab(&mut v, &mut i, [-(HALF - 0.2), h, z], [0.12, h - 0.4, 0.35], RIB);
        slab(&mut v, &mut i, [HALF - 0.2, h, z], [0.12, h - 0.4, 0.35], RIB);
    }
    for x in [-9.0_f32, -4.5, 4.5, 9.0] {
        slab(&mut v, &mut i, [x, h, -(HALF - 0.2)], [0.35, h - 0.4, 0.12], RIB);
    }

    // Roof trusses spanning the bay, backlit by bright skylight strips above them so the trusses
    // read as dark bars against daylight.
    for z in [-8.0_f32, -4.0, 0.0, 4.0, 8.0] {
        slab(&mut v, &mut i, [0.0, WALL_HEIGHT - 0.3, z], [HALF - 0.5, 0.12, 0.18], TRUSS);
    }
    for x in [-6.0_f32, 0.0, 6.0] {
        slab(&mut v, &mut i, [x, WALL_HEIGHT - 0.02, 0.0], [1.4, 0.03, HALF - 3.0], SKYLIGHT);
    }

    // Cold daylight in the back doorway behind the tank.
    slab(&mut v, &mut i, [0.0, 3.6, -(HALF - 0.28)], [5.0, 2.4, 0.05], DOORWAY);

    // Parking-bay markings flanking the turntable, flush with the floor.
    for x in [-6.2_f32, 6.2] {
        slab(&mut v, &mut i, [x, 0.004, 0.0], [0.14, 0.005, HALF - 2.0], MARKING);
    }

    // Turntable (no faked shadow disc — the hero vehicle casts a real contact shadow here).
    push_cylinder(&mut v, &mut i, Vec3::ZERO, TURNTABLE_RADIUS_M, TURNTABLE_TOP_M, 48, TURNTABLE);

    super::hangar_props::push_props(&mut v, &mut i);

    (v, i)
}

/// An axis-aligned solid box (every face winds CCW outward for back-face culling). Shared with
/// `hangar_props`.
pub(super) fn slab(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: [f32; 3],
    half: [f32; 3],
    color: [f32; 3],
) {
    push_oriented_box(
        vertices,
        indices,
        Vec3::from_array(center),
        Vec3::from_array(half),
        Mat3::IDENTITY,
        color,
    );
}

/// A low cylinder resting on the floor: a top cap (normal +Y) plus an outward-facing side ring.
/// The bottom is omitted — it sits on the floor slab and is never seen. Shared with `hangar_props`.
pub(super) fn push_cylinder(
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

    #[test]
    fn the_skylights_run_hot_and_no_faked_shadow_disc_remains() {
        let (vertices, _) = hangar_scene_mesh();
        // The skylight strips blow past 1.0 so the tone curve blooms them into daylight.
        assert!(
            vertices.iter().any(|v| v.color[0] > 1.2 && v.color[1] > 1.2),
            "skylight strips must run hot"
        );
        // The old near-black shadow disc sat just above the turntable; nothing that dark should
        // hover there now that the vehicle casts a real contact shadow.
        let disc = vertices.iter().any(|v| {
            v.color[0] < 0.08
                && v.position[1] > TURNTABLE_TOP_M
                && v.position[1] < TURNTABLE_TOP_M + 0.02
        });
        assert!(!disc, "the faked shadow disc must be gone");
    }

    #[test]
    fn workshop_props_add_geometry_beyond_the_bare_shell() {
        let (with_props, _) = hangar_scene_mesh();
        // The crane, wheel/track stacks, workbench, barrels and stains dwarf the plain shed shell.
        assert!(with_props.len() > 1500, "the workshop is furnished, got {}", with_props.len());
        // Props sit outside the turntable, clear of the hero vehicle.
        assert!(
            with_props.iter().any(|v| v.position[0].abs() > TURNTABLE_RADIUS_M + 1.0),
            "props stand off the turntable"
        );
    }
}
