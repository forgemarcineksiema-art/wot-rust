//! Placement of animatable running-gear unit meshes along each per-side track path.

use glam::{Mat4, Vec3};

use crate::running_gear::{GearPart, GearPlacement, RunningGearKinematics};
use crate::running_gear_geom::sample_belt;

/// Place and orient every moving part for both tracks at the given per-side phase.
pub fn running_gear_placements(
    kin: &RunningGearKinematics,
    left_phase_m: f32,
    right_phase_m: f32,
) -> Vec<GearPlacement> {
    let mut placements = Vec::new();
    for (side_sign, phase) in [(1.0_f32, right_phase_m), (-1.0_f32, left_phase_m)] {
        place_side(kin, side_sign, phase, &mut placements);
    }
    placements
}

fn place_side(
    kin: &RunningGearKinematics,
    side_sign: f32,
    phase: f32,
    out: &mut Vec<GearPlacement>,
) {
    let wheel_spin = phase / kin.wheel_radius.max(0.05);
    for &z in &kin.wheel_zs {
        out.push(GearPlacement {
            part: GearPart::RoadWheel,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.cy, z))
                * Mat4::from_rotation_x(wheel_spin),
        });
    }

    let end_spin = phase / kin.end_radius.max(0.05);
    for (part, z) in
        [(GearPart::Sprocket, kin.cz - kin.half_run), (GearPart::Idler, kin.cz + kin.half_run)]
    {
        out.push(GearPlacement {
            part,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.cy, z))
                * Mat4::from_rotation_x(end_spin),
        });
    }

    let count = kin.link_count();
    let length = kin.belt_length();
    let mut link_phase = phase.rem_euclid(length);
    if link_phase < 1.0e-4 || link_phase > length - 1.0e-4 {
        link_phase = 0.0;
    }
    for i in 0..count {
        let mut s = (link_phase + (i as f32 / count as f32) * length).rem_euclid(length);
        if s > length - 1.0e-4 {
            s = 0.0;
        }
        let sample = sample_belt(kin, s);
        out.push(GearPlacement {
            part: GearPart::Link,
            transform: Mat4::from_translation(Vec3::new(
                side_sign * kin.link_x,
                sample.y,
                sample.z,
            )) * Mat4::from_rotation_x(sample.rot_x),
        });
    }
}
