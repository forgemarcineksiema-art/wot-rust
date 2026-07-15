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
    /// Per-side multipliers on the top-run sag. `1.0` (or the `0.0` default) = rest; drive pulls
    /// a side toward ~0.5, braking toward ~1.5, and a THROWN track lets its side hang deep.
    pub left_sag_scale: f32,
    pub right_sag_scale: f32,
    /// `Some` when this side's track is THROWN. The value is the normalized belt-path location
    /// of the break (kept for wire compatibility and future dangle work); presentation-wise a
    /// thrown side draws NO belt at all — the loop is off the wheels, lying on the field as
    /// the shed ribbon. Bare road wheels are the photograph of a de-tracked tank.
    pub left_break_t: Option<f32>,
    pub right_break_t: Option<f32>,
}

impl GearDynamics<'_> {
    fn sag(&self, kin: &RunningGearKinematics, scale: f32) -> f32 {
        let scale = if scale <= 0.0 { 1.0 } else { scale };
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
    for (side_sign, phase, travel, sag_scale, break_t) in [
        (
            1.0_f32,
            right_phase_m,
            dynamics.right_travel,
            dynamics.right_sag_scale,
            dynamics.right_break_t,
        ),
        (
            -1.0_f32,
            left_phase_m,
            dynamics.left_travel,
            dynamics.left_sag_scale,
            dynamics.left_break_t,
        ),
    ] {
        place_side(
            kin,
            side_sign,
            phase,
            travel,
            dynamics.sag(kin, sag_scale),
            break_t,
            &mut placements,
        );
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
    break_t: Option<f32>,
    out: &mut Vec<GearPlacement>,
) {
    let wheel_spin = phase / kin.wheel_radius.max(0.05);
    for (index, &z) in kin.wheel_zs.iter().enumerate() {
        let lift = travel_at(travel, index);
        let y = kin.cy + lift;
        // Schachtellaufwerk: odd-indexed wheels ride the inner row, offset inboard — the Tiger
        // family's interleaved/overlapped look. Zero for the single-file layouts.
        let wheel_x = kin.wheel_x - if index % 2 == 1 { kin.wheel_overlap_dx } else { 0.0 };
        out.push(GearPlacement {
            part: GearPart::RoadWheel,
            transform: Mat4::from_translation(Vec3::new(side_sign * wheel_x, y, z))
                * Mat4::from_rotation_x(wheel_spin),
        });
        out.push(GearPlacement {
            part: GearPart::SwingArm,
            transform: crate::running_gear_arms::swing_arm_transform(kin, side_sign, z, lift),
        });
    }

    // Return rollers carry the taut top run and spin with the belt passing over them.
    let roller_spin = phase / kin.roller_radius.max(0.05);
    for &z in &kin.roller_zs {
        out.push(GearPlacement {
            part: GearPart::ReturnRoller,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.roller_y, z))
                * Mat4::from_rotation_x(roller_spin),
        });
    }

    // End wheels spin at the LINK line's radius (wheel radius + seat), so the sprocket's teeth
    // and the shoes they flank move together instead of creeping past each other.
    let end_spin = phase / crate::running_gear_belt::wrap_radius(kin);
    for (part, z) in [(GearPart::Sprocket, -kin.end_cz), (GearPart::Idler, kin.end_cz)] {
        out.push(GearPlacement {
            part,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.end_cy, z))
                * Mat4::from_rotation_x(end_spin),
        });
    }

    // A thrown track is OFF the wheels: the whole loop lies on the field (the shed ribbon),
    // so the broken side draws no belt at all — bare road wheels, the way every photograph
    // of a de-tracked tank reads. The first cut left a token 5-link gap that a glance never
    // caught; a missing band cannot be missed.
    if break_t.is_some() {
        return;
    }
    let path = BeltPath::with_sag(kin, sag);
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
        // The whole LOWER half of the loop conforms to the wheels riding over terrain — the
        // ground run fully, the ramps through the same faded interpolation — so the belt lifts
        // with the gear instead of kinking at the ramp foot and knifing into a bump. The upper
        // wraps and the top run stay rigid (the fade has reached zero there anyway).
        let dy = if sample.y < kin.end_cy { bottom_travel(kin, travel, sample.z) } else { 0.0 };
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

/// Links of the freshly thrown band still hanging over the drive sprocket (Fizyczny Świat,
/// thrown-track phase 2): when a track breaks, the last stretch of the band often stays draped
/// over the front wrap for a few beats before it slides off. The chain follows the sprocket's
/// wrap arc from the top, leaves it past the front, and hangs down against the hull's lower
/// bow. `slide_m` pulls the whole chain along its own length (the slide-off animation); links
/// that reach the ground line are gone — they have joined the band lying on the field.
pub fn thrown_remnant_placements(
    kin: &RunningGearKinematics,
    side_sign: f32,
    slide_m: f32,
) -> Vec<GearPlacement> {
    const REMNANT_LINKS: usize = 14;
    /// How far around the front wrap the chain stays seated before it hangs free (radians
    /// from the top of the sprocket, toward the front and under).
    const WRAP_EXIT_RAD: f32 = 2.0;
    let radius = crate::running_gear_belt::wrap_radius(kin);
    let pitch = kin.belt_length() / kin.link_count().max(1) as f32;
    let arc_len = radius * WRAP_EXIT_RAD;
    // The band's bottom line: links that slide below it have reached the ground.
    let ground_y = kin.cy - kin.wheel_radius + 0.03;
    let mut placements = Vec::new();
    for index in 0..REMNANT_LINKS {
        let d = index as f32 * pitch + slide_m.max(0.0);
        let (y, z, rot_x) = if d <= arc_len {
            let phi = d / radius;
            (
                kin.end_cy + radius * phi.cos(),
                -kin.end_cz - radius * phi.sin(),
                phi.sin().atan2(-phi.cos()),
            )
        } else {
            let overshoot = d - arc_len;
            let (exit_sin, exit_cos) = WRAP_EXIT_RAD.sin_cos();
            let y_dir = -exit_sin;
            let z_dir = -exit_cos;
            (
                kin.end_cy + radius * exit_cos + y_dir * overshoot,
                -kin.end_cz - radius * exit_sin + z_dir * overshoot,
                (-y_dir).atan2(z_dir),
            )
        };
        if y < ground_y {
            continue; // this link has already joined the band on the field
        }
        placements.push(GearPlacement {
            part: GearPart::Link,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.link_x, y, z))
                * Mat4::from_rotation_x(rot_x),
        });
    }
    placements
}
