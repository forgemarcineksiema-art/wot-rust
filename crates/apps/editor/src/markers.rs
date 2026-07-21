//! The editor's 3D annotations, drawn through the dynamic-mesh slot: report problems as
//! pylons you can fly to, spawn zones as team rings, strategic points as posts, and the
//! cursor probe as a disc. Matte painterly boxes — the same visual language as the world,
//! never a floating gizmo from another game.

use glam::Vec3;
use map_forge::{MapReport, Severity};
use renderer_api::SceneVertex;
use terrain::BattlefieldMap;

const ERROR_RED: [f32; 3] = [0.72, 0.20, 0.14];
const WARNING_AMBER: [f32; 3] = [0.82, 0.58, 0.16];
const SELECTED_LIFT: f32 = 1.35;
const TEAM_COLORS: [[f32; 3]; 2] = [[0.30, 0.50, 0.28], [0.30, 0.40, 0.55]];
const POINT_WHITE: [f32; 3] = [0.78, 0.78, 0.72];
const PROBE_AMBER: [f32; 3] = [0.95, 0.65, 0.15];

/// Every report entry that knows WHERE it is — the jump-to-problem list, in report order.
pub fn problem_positions(report: &MapReport) -> Vec<([f32; 3], Severity)> {
    report.entries.iter().filter_map(|entry| entry.at.map(|at| (at, entry.severity))).collect()
}

/// The static annotation mesh for a compiled map: problem pylons (the `selected` one burns
/// brighter), spawn rings, strategic-point posts. Rebuilt per reload, not per frame.
pub fn map_markers(
    map: &BattlefieldMap,
    report: &MapReport,
    selected: Option<usize>,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (index, (at, severity)) in problem_positions(report).into_iter().enumerate() {
        let base = match severity {
            Severity::Error => ERROR_RED,
            Severity::Warning => WARNING_AMBER,
        };
        let lift = if selected == Some(index) { SELECTED_LIFT } else { 1.0 };
        let color = base.map(|c| (c * lift).min(1.0));
        let ground = map.heightmap.sample_height(at[0], at[2]).unwrap_or(at[1]);
        let height = if selected == Some(index) { 9.0 } else { 6.0 };
        push_marker_box(
            &mut vertices,
            &mut indices,
            Vec3::new(at[0], ground + height * 0.5, at[2]),
            Vec3::new(0.35, height * 0.5, 0.35),
            color,
        );
    }
    for zone in &map.spawn_zones {
        let color = TEAM_COLORS[(zone.team as usize + 1) % 2];
        push_ground_ring(
            &mut vertices,
            &mut indices,
            map,
            [zone.center[0], zone.center[2]],
            zone.radius_m,
            1.4,
            color,
        );
    }
    for point in &map.strategic_points {
        push_marker_box(
            &mut vertices,
            &mut indices,
            Vec3::new(point.position[0], point.position[1] + 1.6, point.position[2]),
            Vec3::new(0.4, 1.6, 0.4),
            POINT_WHITE,
        );
        push_ground_ring(
            &mut vertices,
            &mut indices,
            map,
            [point.position[0], point.position[2]],
            point.radius_m,
            0.35,
            POINT_WHITE.map(|c| c * 0.55),
        );
    }
    (vertices, indices)
}

/// The cursor probe: a flat amber disc seated on the ground — the M4 brush cursor's seed.
pub fn probe_marker(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    at: Vec3,
    radius_m: f32,
) {
    const SEGMENTS: u32 = 20;
    let center = at + Vec3::Y * 0.12;
    let base = vertices.len() as u32;
    vertices.push(SceneVertex::new(center.to_array(), [0.0, 1.0, 0.0], PROBE_AMBER));
    for segment in 0..=SEGMENTS {
        let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let rim = center + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius_m;
        vertices.push(SceneVertex::new(rim.to_array(), [0.0, 1.0, 0.0], PROBE_AMBER));
    }
    for segment in 0..SEGMENTS {
        indices.extend_from_slice(&[base, base + 2 + segment, base + 1 + segment]);
    }
}

/// A flat ribbon ring seated on the terrain (each segment samples its own ground height, so
/// the ring drapes over slopes instead of clipping into them).
fn push_ground_ring(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    map: &BattlefieldMap,
    center: [f32; 2],
    radius_m: f32,
    width_m: f32,
    color: [f32; 3],
) {
    const SEGMENTS: u32 = 40;
    let ground = |x: f32, z: f32| map.heightmap.sample_height(x, z).unwrap_or(0.0) + 0.25;
    let base = vertices.len() as u32;
    for segment in 0..SEGMENTS {
        let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for radius in [radius_m - width_m * 0.5, radius_m + width_m * 0.5] {
            let x = center[0] + cos * radius;
            let z = center[1] + sin * radius;
            vertices.push(SceneVertex::new([x, ground(x, z), z], [0.0, 1.0, 0.0], color));
        }
    }
    for segment in 0..SEGMENTS {
        let a = base + segment * 2;
        let b = base + ((segment + 1) % SEGMENTS) * 2;
        indices.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
    }
}

/// An axis-aligned matte box with per-face normals (the painterly read of world objects).
fn push_marker_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    color: [f32; 3],
) {
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ];
    for (normal, u, v) in faces {
        let base = vertices.len() as u32;
        let face_center = center + normal * (half * normal.abs()).dot(normal.abs());
        let eu = u * (half * u.abs()).dot(u.abs());
        let ev = v * (half * v.abs()).dot(v.abs());
        for corner in [
            face_center - eu - ev,
            face_center + eu - ev,
            face_center + eu + ev,
            face_center - eu + ev,
        ] {
            vertices.push(SceneVertex::new(corner.to_array(), normal.to_array(), color));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compiled_map_grows_markers_for_spawns_and_points_and_they_are_well_formed() {
        let document = crate::EditorDocument::new_scratch();
        let compiled = document.recompile();
        let (vertices, indices) = map_markers(&compiled.battlefield, &compiled.report, None);
        assert!(!vertices.is_empty(), "two spawns must ring the scratch plain");
        assert!(indices.len().is_multiple_of(3));
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));

        let mut probe_vertices = Vec::new();
        let mut probe_indices = Vec::new();
        probe_marker(&mut probe_vertices, &mut probe_indices, Vec3::new(150.0, 5.0, 150.0), 1.5);
        assert!(probe_indices.len().is_multiple_of(3));
        // The disc floats just over its ground seat, never under it.
        assert!(probe_vertices.iter().all(|vertex| vertex.position[1] > 5.0));
    }
}
