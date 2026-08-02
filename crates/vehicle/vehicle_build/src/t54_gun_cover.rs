//! The canvas cover over the T-54's gun window.
//!
//! What a viewer sees on a T-54 obr. 1949/1951 front is not a mantlet at all: it is a wide,
//! roughly RECTANGULAR canvas panel, fastened by a perimeter strip over the window cut between
//! the turret's cheeks, bulging where the hidden mount presses it, gathering through radial folds
//! into a short sleeve the barrel walks out of. The armour's narrow ~0.40 m aperture sits UNDER
//! the fabric, invisible.
//!
//! The first build drew a round tapered boot in a round pocket — three round signals stacked, and
//! the front read as the old ball mantlet shrunk and swallowed. The player's verdict, and
//! correct. A cover is a rectangle of fabric; this module builds one:
//!
//!   * the PANEL — a rounded-rectangle section swept out of the window: edge buried at the
//!     window floor, a hem swell, a rounded border, then the FLAT FRONT FACE (a mattress,
//!     not a funnel);
//!   * the SLEEVE — the short round throat piercing that face out to the strap on the tube;
//!   * the FRAME — the perimeter fastening strip (steel, its own part).
//!
//! Deliberately ABSENT: fold ridges. Tried twice — thin tubes on the flat face read as four
//! stamped chevrons from one path, as claw-mark slashes from the other. Real creases are broad,
//! low undulations this kernel cannot fake with tubes; clean canvas is the honest render.

use game_core::GunVisual;
use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// Samples around the sleeve. Twelve keeps the throat round at strap distance.
const SLEEVE_SEGMENTS: usize = 10;
/// Corner radius of the panel's rounded rectangle, at frame scale.
const CORNER_R: f32 = 0.055;
/// Points per corner arc.
const CORNER_PTS: usize = 3;

/// The panel's stations: `(z, scale)` along the barrel axis, trunnion-relative. The frame edge
/// sits just proud of the window floor (buried behind the fastening strip), the hem swells, and
/// the border rounds off toward the FLAT FRONT FACE — a cover is a mattress, not a funnel. The
/// first build ran the taper all the way to 0.30 and the front read as a pyramid of diagonals.
const PANEL_STATIONS: [(f32, f32); 4] =
    [(-0.010, 1.0), (0.035, 1.015), (0.080, 0.97), (0.118, 0.80)];

/// A rounded-rectangle ring of points, counter-clockwise, no repeated vertex: each corner arc
/// samples `[base, base + 90)`, so quadrant boundaries appear exactly once.
fn rounded_rect_ring(half_x: f32, half_y: f32) -> Vec<Vec2> {
    let (cx, cy) = (half_x - CORNER_R, half_y - CORNER_R);
    let mut points = Vec::with_capacity(4 * CORNER_PTS);
    for corner in 0..4 {
        let base = corner as f32 * std::f32::consts::FRAC_PI_2;
        for k in 0..CORNER_PTS {
            let a = base + k as f32 / CORNER_PTS as f32 * std::f32::consts::FRAC_PI_2;
            let mut p = Vec2::new(a.cos(), a.sin()) * CORNER_R;
            p.x += if p.x >= 0.0 { cx } else { -cx };
            p.y += if p.y >= 0.0 { cy } else { -cy };
            points.push(p);
        }
    }
    points
}

/// The canvas: panel + sleeve + folds, one fabric mesh in vehicle-local space.
pub fn t54_mantlet_cover(trunnion: Vec3, gun: &GunVisual) -> GeometryMesh {
    let (fx, fy) = gun.cover_frame_half;
    let mut pieces: Vec<GeometryMesh> = Vec::new();

    // --- the panel -------------------------------------------------------------------------
    let panel_points: Vec<Vec3> =
        PANEL_STATIONS.iter().map(|&(z, _)| trunnion + Vec3::new(0.0, 0.0, z)).collect();
    let panel_scales: Vec<f32> = PANEL_STATIONS.iter().map(|&(_, s)| s).collect();
    // The sweep's FixedUp(Y) frame on a +Z path maps the section's X axis onto world Y —
    // measured, not assumed (the first build came out portrait). The ring is built with the
    // HEIGHT half first so the wide side lands across the vehicle.
    let section = SweepSection { points: rounded_rect_ring(fy, fx), closed: true };
    let path = SweepPath { points: panel_points, closed: false };
    pieces.push(
        try_sweep(&SweepSpec {
            path: &path,
            section: &section,
            frame_mode: SweepFrameMode::FixedUp(Vec3::Y),
            // The END cap is the point: the flat canvas front face the sleeve pierces.
            // The start cap is buried inside the window.
            caps: SweepCaps::Both,
            material: MaterialRole::Canvas,
            smoothing: SmoothingGroup(6),
            section_scale: Some(&panel_scales),
        })
        .expect("the T-54 cover panel is a valid sweep"),
    );

    // --- the sleeve ------------------------------------------------------------------------
    let stations = gun.mantlet_cover;
    let span = stations[stations.len() - 1].0 - stations[0].0;
    let sleeve_points: Vec<Vec3> = stations
        .iter()
        .map(|&(z, _)| {
            let t = if span.abs() > 1.0e-6 { (z - stations[0].0) / span } else { 0.0 };
            let sag = gun.mantlet_cover_sag * (t * std::f32::consts::PI).sin();
            trunnion + Vec3::new(0.0, -sag, z)
        })
        .collect();
    let sleeve_scales: Vec<f32> = stations.iter().map(|&(_, radius)| radius).collect();
    let sleeve_section = SweepSection {
        points: (0..SLEEVE_SEGMENTS)
            .map(|index| {
                let angle = index as f32 / SLEEVE_SEGMENTS as f32 * std::f32::consts::TAU;
                Vec2::new(angle.cos(), angle.sin())
            })
            .collect(),
        closed: true,
    };
    let sleeve_path = SweepPath { points: sleeve_points, closed: false };
    pieces.push(
        try_sweep(&SweepSpec {
            path: &sleeve_path,
            section: &sleeve_section,
            frame_mode: SweepFrameMode::FixedUp(Vec3::Y),
            caps: SweepCaps::Open,
            material: MaterialRole::Canvas,
            smoothing: SmoothingGroup(6),
            section_scale: Some(&sleeve_scales),
        })
        .expect("the T-54 cover sleeve is a valid sweep"),
    );

    revolve::merge(&pieces).weld_and_smooth()
}

/// The fastening FRAME: the perimeter strip that clamps the fabric hem into the window — the
/// crisp rectangular outline a viewer reads before anything else. A closed loop (a frame has no
/// ends), just outside the panel's frame station, against the window walls.
pub fn t54_mantlet_frame(trunnion: Vec3, gun: &GunVisual) -> GeometryMesh {
    let ring = rounded_rect_ring(gun.cover_frame_half.0 + 0.010, gun.cover_frame_half.1 + 0.010);
    let points: Vec<Vec3> =
        ring.into_iter().map(|p| trunnion + Vec3::new(p.x, p.y, -0.006)).collect();
    let path = SweepPath { points, closed: true };
    let section = SweepSection {
        points: (0..5)
            .map(|index| {
                let angle = index as f32 / 5.0 * std::f32::consts::TAU;
                Vec2::new(angle.cos(), angle.sin()) * 0.012
            })
            .collect(),
        closed: true,
    };
    try_sweep(&SweepSpec {
        path: &path,
        section: &section,
        // The loop lies in the XY plane, so its up reference is the plane NORMAL — a Y up is
        // parallel to the vertical runs' tangents and degenerates.
        frame_mode: SweepFrameMode::FixedUp(Vec3::Z),
        caps: SweepCaps::Open,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(3),
        section_scale: None,
    })
    .expect("the cover frame is a valid closed sweep")
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{VehicleBlueprint, VehicleKind};

    fn gun() -> GunVisual {
        VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().visual_detail().unwrap().gun
    }

    #[test]
    fn the_cover_is_a_rectangle_wider_than_tall() {
        let trunnion = Vec3::new(0.0, 1.78, 1.15);
        let cover = t54_mantlet_cover(trunnion, &gun());
        let b = cover.bounds().expect("cover bounds");
        let width = b.max.x - b.min.x;
        let height = b.max.y - b.min.y;
        assert!(
            (1.05..=1.6).contains(&(width / height)),
            "a T-54 cover is a compact pillow, wider than tall (measured ~0.45 x 0.37), got {width:.3} x {height:.3}"
        );
    }

    #[test]
    fn the_sleeve_ends_gripping_the_tube() {
        let trunnion = Vec3::new(0.0, 1.78, 1.15);
        let cover = t54_mantlet_cover(trunnion, &gun());
        let far = gun().mantlet_cover[3].0;
        let ring: Vec<f32> = cover
            .vertices()
            .iter()
            .filter(|v| (v.position.z - (trunnion.z + far)).abs() < 0.01)
            .map(|v| v.position.x.hypot(v.position.y - trunnion.y))
            .collect();
        assert!(!ring.is_empty(), "the sleeve reaches its strap station");
        let radius = ring.iter().copied().fold(0.0_f32, f32::max);
        assert!(radius < 0.115, "the strap grips the tube, not the air: {radius:.3}");
    }

    #[test]
    fn the_frame_is_a_closed_loop_outside_the_panel() {
        let trunnion = Vec3::new(0.0, 1.78, 1.15);
        let frame = t54_mantlet_frame(trunnion, &gun());
        let report = frame.quality_report(vehicle_geometry::OPEN_OR_CLOSED_MESH);
        assert_eq!(report.non_manifold_edges, 0, "a ring of strip, cleanly closed");
        let b = frame.bounds().expect("frame bounds");
        let (fx, fy) = gun().cover_frame_half;
        assert!(b.max.x > fx && b.max.y > fy, "the strip clamps OUTSIDE the fabric hem");
    }
}
