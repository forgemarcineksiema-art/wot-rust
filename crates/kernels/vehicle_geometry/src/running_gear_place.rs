//! Placement of animatable running-gear unit meshes along each per-side track path, with the
//! presentation-side suspension dynamics: per-wheel vertical travel over terrain and a top-run
//! sag that tightens under drive and slackens when braking.

use glam::{Mat4, Vec3};

use crate::running_gear::{GearPart, GearPlacement, RunningGearKinematics};
use crate::running_gear_belt::BeltPath;

/// Presentation dynamics for one placement pass. Defaults are the rest pose, so static consumers
/// (bakes, tests, the garage) render unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct GearDynamics<'a> {
    /// Per-road-wheel vertical travel (m), in `wheel_zs` order; missing entries read as 0.
    pub left_travel: &'a [f32],
    pub right_travel: &'a [f32],
    /// Multiplies the top-run sag. `1.0` = rest; drive pulls it toward ~0.5, braking toward ~1.5.
    pub sag_scale: f32,
}

impl GearDynamics<'_> {
    fn sag(&self, kin: &RunningGearKinematics) -> f32 {
        let scale = if self.sag_scale <= 0.0 { 1.0 } else { self.sag_scale };
        kin.top_sag_m * scale
    }
}

/// Place and orient every moving part for both tracks at the given per-side phase (rest pose).
pub fn running_gear_placements(
    kin: &RunningGearKinematics,
    left_phase_m: f32,
    right_phase_m: f32,
) -> Vec<GearPlacement> {
    running_gear_placements_dynamic(kin, left_phase_m, right_phase_m, GearDynamics::default())
}

/// As [`running_gear_placements`], with live suspension dynamics.
pub fn running_gear_placements_dynamic(
    kin: &RunningGearKinematics,
    left_phase_m: f32,
    right_phase_m: f32,
    dynamics: GearDynamics<'_>,
) -> Vec<GearPlacement> {
    let mut placements = Vec::new();
    for (side_sign, phase, travel) in [
        (1.0_f32, right_phase_m, dynamics.right_travel),
        (-1.0_f32, left_phase_m, dynamics.left_travel),
    ] {
        place_side(kin, side_sign, phase, travel, dynamics.sag(kin), &mut placements);
    }
    placements
}

/// Vertical travel for the wheel at `index` (0 = rest for anything the caller did not provide).
/// The upward range is generous (a real T-54 bogie travels ~0.2 m): wheels that cap out on a
/// bump leave the belt buried in the terrain.
fn travel_at(travel: &[f32], index: usize) -> f32 {
    travel.get(index).copied().unwrap_or(0.0).clamp(-0.08, 0.20)
}

/// Bottom-run travel under `z`: the shoe follows the wheels it spans, interpolated between the
/// two neighbouring road wheels and easing to the rest line past the outermost ones.
fn bottom_travel(kin: &RunningGearKinematics, travel: &[f32], z: f32) -> f32 {
    if travel.is_empty() || kin.wheel_zs.is_empty() {
        return 0.0;
    }
    let zs = &kin.wheel_zs;
    if z <= zs[0] {
        let fade = ((zs[0] - z) / 0.6).clamp(0.0, 1.0);
        return travel_at(travel, 0) * (1.0 - fade);
    }
    if z >= zs[zs.len() - 1] {
        let fade = ((z - zs[zs.len() - 1]) / 0.6).clamp(0.0, 1.0);
        return travel_at(travel, zs.len() - 1) * (1.0 - fade);
    }
    for i in 0..zs.len() - 1 {
        if z >= zs[i] && z <= zs[i + 1] {
            let t = (z - zs[i]) / (zs[i + 1] - zs[i]).max(1.0e-4);
            return travel_at(travel, i) * (1.0 - t) + travel_at(travel, i + 1) * t;
        }
    }
    0.0
}

fn place_side(
    kin: &RunningGearKinematics,
    side_sign: f32,
    phase: f32,
    travel: &[f32],
    sag: f32,
    out: &mut Vec<GearPlacement>,
) {
    let wheel_spin = phase / kin.wheel_radius.max(0.05);
    for (index, &z) in kin.wheel_zs.iter().enumerate() {
        let y = kin.cy + travel_at(travel, index);
        out.push(GearPlacement {
            part: GearPart::RoadWheel,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, y, z))
                * Mat4::from_rotation_x(wheel_spin),
        });
    }

    let end_spin = phase / kin.end_radius.max(0.05);
    for (part, z) in [(GearPart::Sprocket, -kin.end_cz), (GearPart::Idler, kin.end_cz)] {
        out.push(GearPlacement {
            part,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.end_cy, z))
                * Mat4::from_rotation_x(end_spin),
        });
    }

    let path = BeltPath::with_sag(kin, sag);
    let bottom_len = path.bottom_run_len();
    let count = kin.link_count();
    let length = path.length();
    let mut link_phase = phase.rem_euclid(length);
    if link_phase < 1.0e-4 || link_phase > length - 1.0e-4 {
        link_phase = 0.0;
    }
    for i in 0..count {
        let mut s = (link_phase + (i as f32 / count as f32) * length).rem_euclid(length);
        if s > length - 1.0e-4 {
            s = 0.0;
        }
        let sample = path.sample(s);
        // The ground run conforms to the wheels riding over terrain; wraps and ramps stay rigid.
        let dy = if s < bottom_len { bottom_travel(kin, travel, sample.z) } else { 0.0 };
        out.push(GearPlacement {
            part: GearPart::Link,
            transform: Mat4::from_translation(Vec3::new(
                side_sign * kin.link_x,
                sample.y + dy,
                sample.z,
            )) * Mat4::from_rotation_x(sample.rot_x),
        });
    }
}
