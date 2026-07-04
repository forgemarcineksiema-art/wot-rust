//! The closed belt path of the animatable running gear: the loop the shoe links ride, with the
//! legacy stadium wrap and the T-54's ramped raised-end layout resolved in one model. Split from
//! `running_gear_geom` to keep each module small.

use std::f32::consts::PI;

use crate::running_gear::RunningGearKinematics;

/// A sampled point on the belt loop: position in the side plane and the link rotation about X.
pub(crate) struct BeltSample {
    pub y: f32,
    pub z: f32,
    /// Rotation about X that aligns a link's local +Z with the belt tangent.
    pub rot_x: f32,
}

/// How far below a wheel rim the ground-run link *centre* line rides. The shoe plate reaches
/// ~0.030 toward the wheel, so this leaves the plate PRESSED lightly into the tire's curve (a
/// loaded track, not a floating ribbon) while the guide horns ride inside the tire's centre
/// groove; the surfaces stay curved-vs-flat, so nothing is coplanar enough to z-fight.
pub(crate) const LINK_SEAT: f32 = 0.02;

/// Radius the shoe-link centre line wraps the end wheels at. The sprocket/idler SPIN must use
/// this (not the bare wheel radius) so tooth surfaces and links move together.
pub(crate) fn wrap_radius(kin: &RunningGearKinematics) -> f32 {
    kin.end_radius.max(0.05) + LINK_SEAT
}

/// The resolved belt path in the side plane. Two layouts share one loop model:
///
/// * **Stadium** (legacy): the end wheels sit on the wheel axle line at the wheel span, so the
///   ramps degenerate to zero and each end wrap is a full semicircle — the historical loop.
/// * **Ramped** (T-54): the idler/sprocket axles sit beyond the road wheels and higher than the
///   axle line (`end_cz > half_run`, raised `end_cy`), so the ground run continues past the last
///   wheel and climbs a straight tangent ramp onto each end wrap, as the references show.
pub(crate) struct BeltPath {
    y_bot: f32,
    y_top: f32,
    /// |z| where the bottom run ends and the ramp begins.
    bottom_end_z: f32,
    /// End-wrap circle: |z| of centre, centre height, radius.
    end_cz: f32,
    end_cy: f32,
    r: f32,
    /// Ramp tangent direction for the rear side (unit `(dz, dy)`, pointing rear-and-up).
    ramp_dir: (f32, f32),
    /// Angle (about the rear end circle) where the ramp meets the wrap; `-PI/2` when degenerate.
    theta_start: f32,
    ramp_len: f32,
    arc_len: f32,
    top_sag: f32,
}

impl BeltPath {
    pub(crate) fn new(kin: &RunningGearKinematics) -> Self {
        Self::with_sag(kin, kin.top_sag_m)
    }

    /// As [`Self::new`], with an explicit top-run sag: the render scales it with drive state
    /// (a driven track pulls its top run tight; braking or coasting lets it hang).
    pub(crate) fn with_sag(kin: &RunningGearKinematics, top_sag: f32) -> Self {
        Self::build(kin, top_sag.clamp(0.0, 0.12))
    }


    fn build(kin: &RunningGearKinematics, top_sag: f32) -> Self {
        // Links wrap OUTSIDE the end-wheel tread by the same seat as the ground run: a wrap
        // radius equal to the wheel radius buries half a shoe in the idler tire and shimmers.
        let r = wrap_radius(kin);
        let y_top = kin.end_cy + r;
        let wheel_ground = kin.cy - kin.wheel_radius - LINK_SEAT;
        let raised = kin.end_cy - r > wheel_ground + 1.0e-3 && kin.end_cz > kin.half_run + 1.0e-3;
        if !raised {
            // Stadium: full semicircular end wraps on the axle line at the wheel span.
            return Self {
                y_bot: kin.end_cy - r,
                y_top,
                bottom_end_z: kin.end_cz,
                end_cz: kin.end_cz,
                end_cy: kin.end_cy,
                r,
                ramp_dir: (-1.0, 0.0),
                theta_start: -PI / 2.0,
                ramp_len: 0.0,
                arc_len: PI * r,
                top_sag,
            };
        }

        // Ramped: the ground run extends just past the last road wheel, then climbs the external
        // tangent from that point onto the raised end wrap.
        let y_bot = wheel_ground;
        let bottom_end_z = kin.half_run + kin.wheel_radius * 0.25;
        let p = (-bottom_end_z, y_bot);
        let c = (-kin.end_cz, kin.end_cy);
        let d = ((c.0 - p.0), (c.1 - p.1));
        let dist = (d.0 * d.0 + d.1 * d.1).sqrt().max(r + 1.0e-3);
        let ramp_len = (dist * dist - r * r).sqrt();
        // Tangent leaving the point on the wrap's ground side: the line's direction angle is the
        // centre bearing rotated outward by asin(r / dist).
        let dir_angle = d.1.atan2(d.0) + (r / dist).asin();
        let ramp_dir = (dir_angle.cos(), dir_angle.sin());
        let t = (p.0 + ramp_dir.0 * ramp_len, p.1 + ramp_dir.1 * ramp_len);
        let theta_start = (t.1 - c.1).atan2(t.0 - c.0);
        // Wrap from the tangent point, through the rear (-PI), up to the top (-3·PI/2 ≡ +PI/2).
        let arc_len = r * (theta_start + 1.5 * PI);
        Self {
            y_bot,
            y_top,
            bottom_end_z,
            end_cz: kin.end_cz,
            end_cy: kin.end_cy,
            r,
            ramp_dir,
            theta_start,
            ramp_len,
            arc_len,
            top_sag,
        }
    }

    /// Length of the closed loop (bottom run, two ramps, two end wraps, top run).
    pub(crate) fn length(&self) -> f32 {
        2.0 * (self.bottom_end_z + self.end_cz) + 2.0 * (self.ramp_len + self.arc_len)
    }

    /// Sample the loop at arc length `s` in `[0, length)`. The loop runs: bottom run (front→rear)
    /// → rear ramp → rear wrap → top run (rear→front) → front wrap → front ramp.
    pub(crate) fn sample(&self, s: f32) -> BeltSample {
        let bottom = 2.0 * self.bottom_end_z;
        let top = 2.0 * self.end_cz;
        let (dzr, dyr) = self.ramp_dir;

        if s < bottom {
            // Bottom run: front -> rear, tangent toward -Z.
            return BeltSample {
                y: self.y_bot,
                z: self.bottom_end_z - s,
                rot_x: tangent_rot(-1.0, 0.0),
            };
        }
        let s = s - bottom;
        if s < self.ramp_len {
            // Rear ramp: climbing rear-and-up from the ground run onto the wrap.
            return BeltSample {
                y: self.y_bot + dyr * s,
                z: -self.bottom_end_z + dzr * s,
                rot_x: tangent_rot(dzr, dyr),
            };
        }
        let s = s - self.ramp_len;
        if s < self.arc_len {
            // Rear wrap around (-end_cz, end_cy): tangent point -> rear -> top. Theta decreases
            // with s (dtheta/ds = -1/r), so the unit tangent is (sin theta, -cos theta).
            let theta = self.theta_start - s / self.r;
            return BeltSample {
                y: self.end_cy + self.r * theta.sin(),
                z: -self.end_cz + self.r * theta.cos(),
                rot_x: tangent_rot(theta.sin(), -theta.cos()),
            };
        }
        let s = s - self.arc_len;
        if s < top {
            // Top run: rear -> front, tangent toward +Z, sagging between the end wraps.
            let u = (s / top.max(0.001)).clamp(0.0, 1.0);
            let sag = self.top_sag * (PI * u).sin();
            let dy_dz = -self.top_sag * PI / top.max(0.001) * (PI * u).cos();
            return BeltSample { y: self.y_top - sag, z: -self.end_cz + s, rot_x: tangent_rot(1.0, dy_dz) };
        }
        let s = s - top;
        if s < self.arc_len {
            // Front wrap around (+end_cz, end_cy): top -> front -> tangent point (mirror of rear).
            let theta = PI / 2.0 - s / self.r;
            return BeltSample {
                y: self.end_cy + self.r * theta.sin(),
                z: self.end_cz + self.r * theta.cos(),
                rot_x: tangent_rot(theta.sin(), -theta.cos()),
            };
        }
        let s = s - self.arc_len;
        // Front ramp: descending rearward-and-down from the idler underside onto the ground run
        // (the mirror of the rear ramp; travel keeps the rear ramp's dz and inverts its dy).
        BeltSample {
            y: self.y_bot + dyr * (self.ramp_len - s),
            z: self.bottom_end_z - dzr * (self.ramp_len - s),
            rot_x: tangent_rot(dzr, -dyr),
        }
    }
}

/// Rotation about X mapping a link's local +Z onto the belt tangent `(dz, dy)`. A rotation of `θ`
/// about X sends local +Z to world `(y, z) = (-sin θ, cos θ)`, so to land on the tangent `(dy, dz)`
/// the angle is `atan2(-dy, dz)` — negating `dy` here keeps the shoe following the belt the right way
/// up around the end wraps and along the sagging top run (an un-negated `dy` flips it, splaying the
/// guide horns outward on the bends).
fn tangent_rot(dz: f32, dy: f32) -> f32 {
    (-dy).atan2(dz)
}

