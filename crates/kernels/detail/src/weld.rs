//! Welded-path detail: beads, handle rails and casting seams. All three are a small convex section
//! swept along a polyline with a minimal-twist (parallel-transport) frame, so they hug arbitrary
//! spatial runs without the roll a naive Frenet frame would introduce.

use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// A regular `sides`-gon of the given `radius`, used as the swept cross-section.
fn polygon(radius: f32, sides: usize) -> Vec<Vec2> {
    let sides = sides.max(3);
    (0..sides)
        .map(|i| {
            let a = i as f32 / sides as f32 * std::f32::consts::TAU;
            Vec2::new(radius * a.cos(), radius * a.sin())
        })
        .collect()
}

/// Sweep a round section of `radius` along `path`, capping the ends. The smoothing group controls
/// whether the tube reads as a soft bead or a faceted bar.
fn tube_along(
    path: &[Vec3],
    radius: f32,
    sides: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let path = SweepPath { points: path.to_vec(), closed: false };
    let section = SweepSection { points: polygon(radius, sides), closed: true };
    let spec = SweepSpec {
        path: &path,
        section: &section,
        frame_mode: SweepFrameMode::ParallelTransport,
        caps: SweepCaps::Both,
        material,
        smoothing,
    };
    try_sweep(&spec).expect("a non-degenerate path with a convex section sweeps cleanly")
}

/// A continuous weld bead along `path` — a thin soft tube of cast-steel, the seam where two plates
/// are joined. Smooth-shaded so it catches light as a rounded fillet.
pub fn weld_bead(path: &[Vec3], radius: f32) -> GeometryMesh {
    tube_along(path, radius, 6, MaterialRole::CastArmor, SmoothingGroup(7))
}

/// A grab handle / tow rail along `path` — a faceted steel bar standing proud of the surface.
pub fn handle_rail(path: &[Vec3], radius: f32) -> GeometryMesh {
    tube_along(path, radius, 8, MaterialRole::BarrelSteel, SmoothingGroup::hard_edges())
}

/// A casting seam along `path` — the raised mould line left on a cast turret. A very thin soft bead.
pub fn casting_seam(path: &[Vec3]) -> GeometryMesh {
    tube_along(path, 0.008, 5, MaterialRole::CastArmor, SmoothingGroup(7))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> Vec<Vec3> {
        vec![Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.0, 0.2, 0.0)]
    }

    #[test]
    fn a_weld_bead_follows_its_path_and_is_watertight() {
        let mesh = weld_bead(&path(), 0.02);
        let b = mesh.bounds().expect("bead bounds");
        assert!(b.max.x > 0.9, "bead reaches the path end");
        assert!(b.max.y > 0.2, "bead rises with the path");
        mesh.validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH)
            .expect("a smooth capped tube is a closed manifold");
    }

    #[test]
    fn a_handle_rail_stands_proud_and_is_clean() {
        let mesh = handle_rail(&[Vec3::ZERO, Vec3::new(0.3, 0.0, 0.0)], 0.015);
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean handle rail");
        assert!(mesh.bounds().unwrap().max.x > 0.29);
    }

    #[test]
    fn a_casting_seam_is_a_thin_clean_bead() {
        let mesh = casting_seam(&path());
        let b = mesh.bounds().expect("seam bounds");
        assert!(b.max.z < 0.02 && b.min.z > -0.02, "the seam is thin across the path");
        mesh.validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH).expect("clean casting seam");
    }
}
