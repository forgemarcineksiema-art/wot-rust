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
    /// The top run as a support polyline (rear→front): the upper convex hull of the two
    /// end-wrap tops and the top-run carriers (return rollers where the layout has them,
    /// road-wheel tops otherwise). Between hull points the run SAGS, and the sag is clamped
    /// from below by the carriers under it — so the belt visibly RESTS on the wheels
    /// (T-54/Tiger), hangs between rollers (IS-3/Centurion), and is lifted by wheels that
    /// stand proud of the wrap chord (T-34-85). A tank's track carries its wheels; a taut
    /// straight ribbon was the model-logic audit's defect #7.
    top: Vec<TopSegment>,
    top_len: f32,
}

/// One straight-chord segment of the top-run polyline, with its sag and its clamp floor.
pub(crate) struct TopSegment {
    pub z0: f32,
    pub y0: f32,
    pub z1: f32,
    pub y1: f32,
    pub len: f32,
    /// Sag amplitude for this span (already clamped against `floor`).
    pub sag: f32,
    /// The carrier level under this span (`-inf` when nothing rides below it).
    pub floor: f32,
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
        let wheel_ground = kin.cy - kin.wheel_radius - LINK_SEAT;
        let raised = kin.end_cy - r > wheel_ground + 1.0e-3 && kin.end_cz > kin.half_run + 1.0e-3;
        let (y_bot, bottom_end_z, ramp_dir, theta_start, ramp_len, arc_len);
        if raised {
            // Ramped: the ground run extends just past the last road wheel, then climbs the
            // external tangent from that point onto the raised end wrap.
            y_bot = wheel_ground;
            bottom_end_z = kin.half_run + kin.wheel_radius * 0.25;
            let p = (-bottom_end_z, y_bot);
            let c = (-kin.end_cz, kin.end_cy);
            let d = ((c.0 - p.0), (c.1 - p.1));
            let dist = (d.0 * d.0 + d.1 * d.1).sqrt().max(r + 1.0e-3);
            ramp_len = (dist * dist - r * r).sqrt();
            // Tangent leaving the point on the wrap's ground side: the line's direction angle
            // is the centre bearing rotated outward by asin(r / dist).
            let dir_angle = d.1.atan2(d.0) + (r / dist).asin();
            ramp_dir = (dir_angle.cos(), dir_angle.sin());
            let t = (p.0 + ramp_dir.0 * ramp_len, p.1 + ramp_dir.1 * ramp_len);
            theta_start = (t.1 - c.1).atan2(t.0 - c.0);
            // Wrap from the tangent point, through the rear (-PI), up to the top (+PI/2).
            arc_len = r * (theta_start + 1.5 * PI);
        } else {
            // Stadium: full semicircular end wraps on the axle line at the wheel span.
            y_bot = kin.end_cy - r;
            bottom_end_z = kin.end_cz;
            ramp_dir = (-1.0, 0.0);
            theta_start = -PI / 2.0;
            ramp_len = 0.0;
            arc_len = PI * r;
        }

        // --- The top run: supports, upper convex hull, then clamped sag (drape). ---
        let wrap_top = kin.end_cy + r;
        // The carriers under the top run: return rollers where the layout has them, road-wheel
        // tops otherwise. Their tops wear the same LINK_SEAT the ground run does.
        let carrier_y = if kin.roller_zs.is_empty() {
            kin.cy + kin.wheel_radius + LINK_SEAT
        } else {
            kin.roller_y + kin.roller_radius + LINK_SEAT
        };
        let carrier_zs: &[f32] =
            if kin.roller_zs.is_empty() { &kin.wheel_zs } else { &kin.roller_zs };
        // Tension model (v27 read preserved): `top_sag` is the slack allowance `s`. The belt
        // can drop at most `reach = 2.2*s` below the wrap chord — a driven track (small s)
        // floats ABOVE the carriers; at rest and under braking it reaches them and RESTS,
        // then each span between contacts hangs by `s * span / 1.8`.
        let reach = 2.2 * top_sag;
        // The wrap tops share one height, so the wrap chord is flat at `wrap_top`.
        let chord_y = |_z: f32| -> f32 { wrap_top };
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(carrier_zs.len() + 2);
        pts.push((-kin.end_cz, wrap_top));
        for &z in carrier_zs {
            let touches = chord_y(z) - reach <= carrier_y + 1.0e-4;
            if touches && z > -kin.end_cz + 1.0e-3 && z < kin.end_cz - 1.0e-3 {
                pts.push((z, carrier_y));
            }
        }
        pts.push((kin.end_cz, wrap_top));
        // The touch test above already decided which carriers are path nodes: a floating
        // (driven-tight) belt has no carrier nodes and hangs as one span; a resting belt
        // strings wrap → carrier → carrier → wrap, and the spans between contacts sag.
        let hull = pts;
        // Segments, each with its sag clamped from below by the carriers riding under the
        // span: the belt hangs until it lands on them - "the track rests on the wheels".
        let mut top = Vec::with_capacity(hull.len() - 1);
        let mut top_len = 0.0;
        for pair in hull.windows(2) {
            let (z0, y0) = pair[0];
            let (z1, y1) = pair[1];
            let span = z1 - z0;
            let len = (span * span + (y1 - y0) * (y1 - y0)).sqrt().max(1.0e-4);
            let floor = carrier_zs
                .iter()
                .filter(|&&z| z > z0 + 1.0e-3 && z < z1 - 1.0e-3)
                .map(|_| carrier_y)
                .fold(f32::NEG_INFINITY, f32::max);
            // Span sag: slack spread over the span, never past the tension reach, and never
            // through a carrier riding under the span. SHORT spans stay dead straight: between
            // closely-spaced carriers (the Tiger family's interleaved stations sit ~0.5 m
            // apart, about one link pitch) a sinus dip aliases against the link spacing into
            // a jagged, drunken top run — on the real vehicles tension flattens those spans
            // completely, and the sag lives only in the long gaps (wrap→first wheel, the IS
            // family's roller bays).
            let desired = if span >= 0.85 { (top_sag * span / 1.8).min(reach) } else { 0.0 };
            let mid = 0.5 * (y0 + y1);
            let sag = if floor.is_finite() { desired.min((mid - floor).max(0.0)) } else { desired };
            top.push(TopSegment { z0, y0, z1, y1, len, sag, floor });
            top_len += len;
        }

        Self {
            y_bot,
            bottom_end_z,
            end_cz: kin.end_cz,
            end_cy: kin.end_cy,
            r,
            ramp_dir,
            theta_start,
            ramp_len,
            arc_len,
            top,
            top_len,
        }
    }

    pub(crate) fn y_bot(&self) -> f32 {
        self.y_bot
    }

    pub(crate) fn bottom_end_z(&self) -> f32 {
        self.bottom_end_z
    }

    /// Rear ramp endpoint on the wrap side, `(z, y)`; the front ramp is its Z-mirror.
    pub(crate) fn rear_ramp_end(&self) -> (f32, f32) {
        (
            -self.bottom_end_z + self.ramp_dir.0 * self.ramp_len,
            self.y_bot + self.ramp_dir.1 * self.ramp_len,
        )
    }

    /// The top run sampled as a dense polyline `(z, y)` (rear→front), `subdiv` points per
    /// segment — the static band extrudes these very chords, so band and links agree.
    pub(crate) fn top_polyline(&self, subdiv: usize) -> Vec<(f32, f32)> {
        let subdiv = subdiv.max(1);
        let mut out = Vec::with_capacity(self.top.len() * subdiv + 1);
        if let Some(first) = self.top.first() {
            out.push((first.z0, first.y0));
        }
        for seg in self.top.iter() {
            for k in 1..=subdiv {
                let u = k as f32 / subdiv as f32;
                let chord_y = seg.y0 + (seg.y1 - seg.y0) * u;
                let hang = seg.sag * (std::f32::consts::PI * u).sin();
                let y = if seg.floor.is_finite() {
                    (chord_y - hang).max(seg.floor)
                } else {
                    chord_y - hang
                };
                out.push((seg.z0 + (seg.z1 - seg.z0) * u, y));
            }
        }
        out
    }

    /// Length of the closed loop (bottom run, two ramps, two end wraps, top run).
    pub(crate) fn length(&self) -> f32 {
        2.0 * self.bottom_end_z + 2.0 * (self.ramp_len + self.arc_len) + self.top_len
    }

    /// Sample the loop at arc length `s` in `[0, length)`. The loop runs: bottom run (front→rear)
    /// → rear ramp → rear wrap → top run (rear→front) → front wrap → front ramp.
    pub(crate) fn sample(&self, s: f32) -> BeltSample {
        let bottom = 2.0 * self.bottom_end_z;
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
        if s < self.top_len {
            // Top run: rear -> front along the support polyline, sagging within each span and
            // landing flat on the carrier floor where the drape reaches it.
            let mut s_left = s;
            for seg in &self.top {
                if s_left > seg.len {
                    s_left -= seg.len;
                    continue;
                }
                let u = (s_left / seg.len).clamp(0.0, 1.0);
                let chord_y = seg.y0 + (seg.y1 - seg.y0) * u;
                let chord_dy_dz = (seg.y1 - seg.y0) / (seg.z1 - seg.z0).max(1.0e-4);
                let hang = seg.sag * (PI * u).sin();
                let hang_dy_dz = seg.sag * PI / (seg.z1 - seg.z0).max(1.0e-4) * (PI * u).cos();
                let (y, dy_dz) = if seg.floor.is_finite() && chord_y - hang <= seg.floor {
                    // Resting on the carriers: flat contact, zero slope.
                    (seg.floor, 0.0)
                } else {
                    (chord_y - hang, chord_dy_dz - hang_dy_dz)
                };
                return BeltSample {
                    y,
                    z: seg.z0 + (seg.z1 - seg.z0) * u,
                    rot_x: tangent_rot(1.0, dy_dz),
                };
            }
            // Numerical tail: fall through to the front wrap start.
        }
        let s = (s - self.top_len).max(0.0);
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
