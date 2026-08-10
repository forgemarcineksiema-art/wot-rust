//! K2: the procedural mechanic — an articulated silhouette built from oriented primitives
//! (procedural doctrine: no imports, no character pipeline), walking a fixed round between
//! the welding bay and the workbench. He exists because the hall's work exists: the arc in
//! the second bay burns on a cycle (K1) and the bench holds the tools — a man moving between
//! those two REAL sites is the Valve rule, not a filler NPC.
//!
//! Contract with the frame:
//! - Deterministic: the pose is a pure function of the scene clock, so the goldens' frozen
//!   review second locks one exact mid-stride figure, in game and golden alike.
//! - He NEVER enters the hero's close orbit: the walk line keeps a measured margin outside
//!   the human-scale radius around the turntable, locked by test.
//! - Kill-switch: K2 is research with its own decision gate — a negative look verdict ships
//!   [`MECHANIC_ENABLED`] `= false` and the hall reverts to implied presence (K1) without
//!   surgery.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;

use super::hangar::{Finish, finish};
use super::tank_mesh::push_oriented_box;

/// The K2 decision gate. The mechanic is judged on the review render at 10+ metres in
/// half-light; this constant IS the verdict switch, so a "no" ships as one flipped bool.
pub const MECHANIC_ENABLED: bool = true;

/// The walk line, floor plan: from the welding bay's screen, past the stores lane, to the
/// workbench wall. Every leg of it stays outside the turntable's human-scale radius (8 m,
/// the A2 dressing ring) — the mechanic works the hall's edge, never the hero's light.
const PATH: [[f32; 2]; 3] = [[-5.8, 13.8], [2.0, 11.0], [8.0, 7.5]];

/// An unhurried working walk, and the stand-and-work dwell at each end of the line.
const WALK_SPEED_M_S: f32 = 1.05;
const DWELL_S: f32 = 6.0;
/// One full gait cycle (both feet) per stride pair — drives leg/arm phase from distance
/// travelled, so the feet never skate: the ground the legs claim is the ground covered.
const STRIDE_M: f32 = 0.72;

/// Overalls: work-blue duck cloth, dark enough to live in the hall's half-light.
const OVERALLS: [f32; 3] = [0.135, 0.145, 0.165];
/// Leather boots, near-black.
const BOOTS: [f32; 3] = [0.085, 0.082, 0.080];
/// Weathered skin — the only warm note, and a small one.
const SKIN: [f32; 3] = [0.335, 0.265, 0.215];
/// The flat cap.
const CAP: [f32; 3] = [0.105, 0.11, 0.12];

fn path_length() -> f32 {
    PATH.windows(2)
        .map(|w| {
            let [a, b] = [w[0], w[1]];
            ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
        })
        .sum()
}

/// Where along the line (arc-length metres) the mechanic stands at `seconds`, plus the walk
/// direction sign (+1 outbound, -1 returning) and whether he is mid-walk or dwelling.
fn walk_state(seconds: f32) -> (f32, f32, bool) {
    let length = path_length();
    let leg_s = length / WALK_SPEED_M_S;
    let period = 2.0 * (leg_s + DWELL_S);
    let t = seconds.rem_euclid(period);
    if t < DWELL_S {
        (0.0, 1.0, false)
    } else if t < DWELL_S + leg_s {
        ((t - DWELL_S) * WALK_SPEED_M_S, 1.0, true)
    } else if t < 2.0 * DWELL_S + leg_s {
        (length, -1.0, false)
    } else {
        (length - (t - 2.0 * DWELL_S - leg_s) * WALK_SPEED_M_S, -1.0, true)
    }
}

/// Position on the floor and the facing direction at `distance` metres along the line.
fn path_point(distance: f32, sign: f32) -> (Vec3, Vec3) {
    let mut remaining = distance.clamp(0.0, path_length());
    for w in PATH.windows(2) {
        let a = Vec3::new(w[0][0], 0.0, w[0][1]);
        let b = Vec3::new(w[1][0], 0.0, w[1][1]);
        let seg = (b - a).length();
        if remaining <= seg {
            let dir = (b - a) / seg;
            return (a + dir * remaining, dir * sign);
        }
        remaining -= seg;
    }
    let a = Vec3::new(PATH[PATH.len() - 2][0], 0.0, PATH[PATH.len() - 2][1]);
    let b = Vec3::new(PATH[PATH.len() - 1][0], 0.0, PATH[PATH.len() - 1][1]);
    (b, (b - a).normalize() * sign)
}

/// A limb segment: an oriented square-section box spanning joint to joint.
fn limb(
    v: &mut Vec<SceneVertex>,
    i: &mut Vec<u32>,
    from: Vec3,
    to: Vec3,
    half_w: f32,
    color: [f32; 3],
) {
    let axis = to - from;
    let length = axis.length().max(1.0e-4);
    let fwd = axis / length;
    let side = if fwd.dot(Vec3::Y).abs() > 0.99 { Vec3::X } else { fwd.cross(Vec3::Y).normalize() };
    let up = side.cross(fwd);
    push_oriented_box(
        v,
        i,
        (from + to) * 0.5,
        Vec3::new(half_w, half_w, length * 0.5),
        Mat3::from_cols(side, up, fwd),
        color,
    );
}

/// The mechanic at a moment on the presentation clock. Empty when the kill-switch is off.
pub fn mechanic_at(seconds: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    if !MECHANIC_ENABLED {
        return (Vec::new(), Vec::new());
    }
    let (dist, sign, walking) = walk_state(seconds);
    let (foot, fwd) = path_point(dist, sign);
    let right = fwd.cross(Vec3::Y).normalize();
    let up = Vec3::Y;

    // Gait phase from ground covered — feet never skate. Dwelling collapses the swing to a
    // stand with a slow breath.
    let phase = dist / STRIDE_M * std::f32::consts::TAU;
    let swing = if walking { 1.0 } else { 0.0 };
    let bob = if walking { 0.028 * (2.0 * phase).cos() } else { 0.012 * (seconds * 1.4).sin() };

    let mut v = Vec::new();
    let mut i = Vec::new();
    let basis = Mat3::from_cols(right, up, fwd);
    let pelvis = foot + up * (0.94 + bob);

    // Skeleton first: joints from the gait, so the cloth pass and the extremity pass agree.
    // Legs: hip pitch swings the thigh, the shank flexes at the knee through the swing leg's
    // recovery; each side half a cycle apart. Arms swing opposite their legs with a constant
    // working bend at the elbow.
    let mut ankles = Vec::with_capacity(2);
    let mut legs = Vec::with_capacity(2);
    for (side_sign, leg_phase) in [(1.0, phase), (-1.0, phase + std::f32::consts::PI)] {
        let hip = pelvis + right * (0.10 * side_sign);
        let hip_pitch = 0.42 * leg_phase.sin() * swing;
        let knee_flex = 0.85 * (leg_phase - 0.6).sin().max(0.0) * swing;
        let thigh_dir = -up * hip_pitch.cos() + fwd * hip_pitch.sin();
        let knee = hip + thigh_dir * 0.44;
        let shank_pitch = hip_pitch - knee_flex;
        let shank_dir = -up * shank_pitch.cos() + fwd * shank_pitch.sin();
        let ankle = knee + shank_dir * 0.44;
        legs.push((hip, knee, ankle));
        ankles.push(ankle);
    }
    let mut wrists = Vec::with_capacity(2);
    let mut arms = Vec::with_capacity(2);
    for (side_sign, arm_phase) in [(1.0, phase + std::f32::consts::PI), (-1.0, phase)] {
        let shoulder = pelvis + right * (0.225 * side_sign) + up * 0.52;
        let arm_pitch = 0.30 * arm_phase.sin() * swing;
        let upper_dir = -up * arm_pitch.cos() + fwd * arm_pitch.sin();
        let elbow = shoulder + upper_dir * 0.30;
        let fore_pitch = arm_pitch + 0.38;
        let fore_dir = -up * fore_pitch.cos() + fwd * fore_pitch.sin();
        let wrist = elbow + fore_dir * 0.28;
        arms.push((shoulder, elbow, wrist));
        wrists.push((wrist, fore_dir));
    }

    // The cloth pass: everything the overalls cover, stamped CANVAS in one range.
    let cloth_start = v.len();
    for &(hip, knee, ankle) in &legs {
        limb(&mut v, &mut i, hip, knee, 0.062, OVERALLS);
        limb(&mut v, &mut i, knee, ankle, 0.052, OVERALLS);
    }
    // Torso with a working man's slight forward lean.
    push_oriented_box(
        &mut v,
        &mut i,
        pelvis + up * 0.33,
        Vec3::new(0.165, 0.275, 0.105),
        Mat3::from_axis_angle(right, 0.07) * basis,
        OVERALLS,
    );
    for &(shoulder, elbow, wrist) in &arms {
        limb(&mut v, &mut i, shoulder, elbow, 0.048, OVERALLS);
        limb(&mut v, &mut i, elbow, wrist, 0.042, OVERALLS);
    }
    finish(&mut v[cloth_start..], Finish::CANVAS);

    // Extremities, each with its own material after the cloth stamp cannot reach them.
    let boots_start = v.len();
    for ankle in ankles {
        push_oriented_box(
            &mut v,
            &mut i,
            ankle + fwd * 0.055 + up * 0.02,
            Vec3::new(0.055, 0.05, 0.125),
            basis,
            BOOTS,
        );
    }
    finish(&mut v[boots_start..], Finish::RUBBER);

    let skin_start = v.len();
    for (wrist, fore_dir) in wrists {
        push_oriented_box(
            &mut v,
            &mut i,
            wrist + fore_dir * 0.045,
            Vec3::new(0.035, 0.045, 0.035),
            basis,
            SKIN,
        );
    }
    let head = pelvis + up * 0.70;
    push_oriented_box(&mut v, &mut i, head, Vec3::new(0.082, 0.10, 0.088), basis, SKIN);
    finish(&mut v[skin_start..], Finish { gloss: 0.05, ..Finish::CANVAS });

    let cap_start = v.len();
    push_oriented_box(
        &mut v,
        &mut i,
        head + up * 0.105 + fwd * 0.012,
        Vec3::new(0.092, 0.022, 0.105),
        basis,
        CAP,
    );
    finish(&mut v[cap_start..], Finish::CANVAS);

    (v, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// K2 CONTRACT: the mechanic works the hall's edge — every sampled second of his round
    /// keeps every vertex outside the turntable's 8 m human-scale ring, on the floor (no
    /// levitation, no basement), at human height, and the pose is a pure function of the
    /// clock. The hero's close orbit never meets him.
    #[test]
    // The constant IS the subject: the kill-switch state is a shipping decision, and this
    // lock makes flipping it a deliberate, test-visible act (camera.rs precedent).
    #[allow(clippy::assertions_on_constants)]
    fn the_mechanic_keeps_out_of_the_heros_light() {
        assert!(MECHANIC_ENABLED, "K2 ships behind its decision gate; flip deliberately");
        for step in 0..240 {
            let seconds = step as f32 * 0.25;
            let (v, _) = mechanic_at(seconds);
            assert!(!v.is_empty());
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            for vertex in &v {
                let [x, y, z] = vertex.position;
                let r = (x * x + z * z).sqrt();
                assert!(r > 8.0, "the mechanic entered the hero ring: r={r:.2} at t={seconds}");
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
            assert!(min_y > -0.05 && min_y < 0.30, "feet near the floor: {min_y}");
            assert!(max_y > 1.55 && max_y < 1.95, "human height: {max_y}");
        }
        let (a, ai) = mechanic_at(31.7);
        let (b, bi) = mechanic_at(31.7);
        assert!(a == b && ai == bi, "the mechanic is a pure function of the clock");
    }

    /// The round genuinely travels between its two work sites (welding bay ↔ workbench):
    /// over one period the figure's floor position spans the full walk line, and while
    /// walking the feet claim the ground the path covers (stride phase from distance).
    #[test]
    fn the_round_connects_the_two_work_sites() {
        let length = path_length();
        assert!(length > 12.0, "a real round, not a shuffle: {length}");
        let (start, _) = path_point(0.0, 1.0);
        let (end, _) = path_point(length, 1.0);
        assert!((start - Vec3::new(-5.8, 0.0, 13.8)).length() < 1.0e-4);
        assert!((end - Vec3::new(8.0, 0.0, 7.5)).length() < 1.0e-4);
        // The dwell ends stand still; mid-walk moves.
        let (d0, _, w0) = walk_state(1.0);
        assert!(!w0 && d0 == 0.0, "dwelling at the welding end first");
        let (d1, _, w1) = walk_state(DWELL_S + 3.0);
        assert!(w1 && (d1 - 3.0 * WALK_SPEED_M_S).abs() < 1.0e-4);
    }
}
