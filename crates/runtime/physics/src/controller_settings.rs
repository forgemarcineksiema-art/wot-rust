use game_core::math::GRAVITY_MPS2;
use game_core::{SteeringKind, SuspensionKind, TankSpec, VehicleBlueprint, stock_stability};

use crate::hull_attitude::HullSpring;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankControllerSettings {
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    /// The sprung hull's springs (Inny Poziom G7), derived from the vehicle by
    /// [`hull_spring_for_spec`]: natural frequency from the suspension's static deflection,
    /// damping from its family, weight transfer from the centre of mass, wheelbase and gauge.
    #[serde(default)]
    pub hull_spring: HullSpring,
    /// Specific effective drive power (m²/s³): engine power × drivetrain efficiency × the arcade
    /// scale, over mass. Thrust follows `P/v` — huge at a crawl (where track grip caps it), thin
    /// near top speed — so acceleration tapers the way tonnes behind an engine actually do.
    pub drive_power_mps3: f32,
    /// Speed floor for the `P/v` thrust curve, so a standing start has a finite (grip-capped)
    /// launch force instead of a division blow-up.
    pub min_force_speed_mps: f32,
    /// Rolling resistance (m/s²): the constant drag of tracks over ground.
    pub rolling_resist_mps2: f32,
    /// Quadratic drag coefficient, solved per vehicle so thrust and resistance balance exactly at
    /// the spec's top speed — vmax is an emergent equilibrium, not a clamp.
    pub drag_quadratic: f32,
    pub brake_deceleration_mps2: f32,
    pub turn_rate_rad_s: f32,
    pub ground_probe_length_m: f32,
    /// Coast deceleration with the throttle released (engine braking + rolling), before the
    /// quadratic term. A tank rolls out over many hull lengths, not a few metres.
    pub idle_drag_mps2: f32,
    /// Maximum uphill grade (rise/run) the tank can climb; steeper faces stall it. In the
    /// rigid-body model this is no longer a hard `if`: it is the longitudinal grip coefficient
    /// (`longitudinal_grip_mu`), so a face steeper than `tan(theta) = grade` cannot be held by
    /// track traction and the hull stalls (or creeps back) on its own.
    pub max_climb_grade: f32,
    /// Angular acceleration toward the commanded yaw rate (rad/s^2). This is the hull's rotational
    /// inertia knob: a finite ramp is why the hull no longer snaps to a new heading the instant
    /// the steer key is tapped. Heavier hulls spool slower.
    pub yaw_accel_rad_s2: f32,
    /// Longitudinal track grip coefficient (dimensionless mu). With gravity `g`, the tracks can
    /// deliver at most `mu * g * traction * cos(theta)` of forward thrust, which is what makes
    /// gradeability emergent rather than a clamp. Kept equal to `max_climb_grade`.
    pub longitudinal_grip_mu: f32,
    /// Lateral track grip coefficient (dimensionless mu). Sideways friction saturates at
    /// `mu * g * traction`; below it the hull tracks its nose, above it (a hard turn at speed, or
    /// a steep/low-traction face) it breaks loose and slides. High by default so the hull "grips"
    /// like WoT and only drifts in genuinely hard turns or on poor ground.
    pub lateral_grip_mu: f32,
    /// How strongly a turn bleeds forward speed (skid-steer scrub): decel = k * |yaw_rate| * |v|.
    pub turn_scrub: f32,
    /// Static (parked) track-lock grip coefficient. A stopped, undriven hull holds any slope up to
    /// `static_grip_mu * traction` (rise/run) without creeping or sliding — the locked-track hold.
    /// Higher than the kinetic grips (mu_s > mu_k) so the hull "sticks" and only breaks loose on a
    /// genuinely too-steep or too-slick face.
    pub static_grip_mu: f32,
    /// Below this planar speed an undriven hull is eligible to grab and lock (static hold). Above it
    /// the kinetic slide model runs, so a moving hull only grabs once it has nearly stopped.
    pub static_hold_speed_mps: f32,
    /// What the hull turns ABOUT when it pivots — a fact about its gearbox, not a balance knob.
    /// See [`game_core::SteeringKind`].
    pub steering: SteeringKind,
    /// Half the track gauge — the arm the hull swings on when its gearbox can only brake a belt
    /// rather than reverse it. Zero for a hull that counter-rotates, because it swings on nothing.
    pub pivot_arm_m: f32,
    /// Grade past which a face is a hard wall (cliff / railway embankment): the tracks find no
    /// drive and incoming momentum digs the nose in. Between `max_climb_grade` and this, steep faces
    /// are momentum-climbable — the grip slips but does not vanish, so a committed run-up scrabbles
    /// the hull a bounded way up a hump.
    pub momentum_climb_ceiling: f32,
    /// What each belt can still put on the ground, `[left, right]`, as a fraction of nominal:
    /// 1.0 healthy, 0.0 a thrown track or a belt with no grip under it. Both 1.0 is the whole
    /// fleet's normal state and costs nothing — see [`BeltDrive`]. `serde(default)` keeps every
    /// pre-belt fixture loading with two whole tracks.
    #[serde(default = "BeltDrive::healthy")]
    pub belts: BeltDrive,
}

/// What each track belt can still deliver. Steering in a tracked vehicle is not a yaw command; it
/// is a SPEED DIFFERENCE BETWEEN THE BELTS, and `omega = (v_left - v_right) / gauge` falls out of
/// it. So a belt that cannot drive cannot contribute its half of a turn — and, crucially, cannot be
/// made to. With one belt thrown, the working belt walks the hull around the dead one and there is
/// no input that straightens it: going straight would mean matching the belts, which means stopping.
///
/// Both belts healthy leaves every number in the drive untouched (the window below is unbounded),
/// so the fleet's mobility table and every replay stay bit-identical.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BeltDrive {
    pub left: f32,
    pub right: f32,
}

/// How much of a PINNED pivot a dead belt actually imposes.
///
/// The two ends of the range are both real damage states. A belt jammed solid is a pin: the hull
/// swings on it and the turn radius is pure geometry, `2 * half_gauge` — 2.6 m for a T-54, a
/// pirouette. A belt that is simply GONE imposes nothing, and the ratio of rolling drag to the grip
/// cap puts that at a 42 m radius, which is barely a penalty at all. Measured, both.
///
/// A thrown track is neither. The belt is off the sprocket but bunched under the running gear, and
/// the road wheels ride down into it — far more drag than free rolling, far less than a pin.
///
/// **This constant is a modelling choice inside that band, not a derivation, and it was chosen for
/// what it does to play.** At 0.25 the forced radius is ~10.6 m: a 90° swing takes 17 m of arc and
/// about 2.7 seconds, so a crippled hull can still lurch into cover that is roughly abeam but can
/// never retreat down a road. And because the forced rate scales with SPEED, stopping cancels it
/// outright — a one-track tank can always plant itself and fight with the turret. Damage takes the
/// player's line away, not their agency.
const DEAD_BELT_PIVOT_BITE: f32 = 0.25;

impl BeltDrive {
    /// Both belts whole — the state every tank is in until something breaks it.
    pub const HEALTHY: Self = Self { left: 1.0, right: 1.0 };

    /// `HEALTHY` as a function, for `serde(default)`.
    pub fn healthy() -> Self {
        Self::HEALTHY
    }

    pub fn new(left: f32, right: f32) -> Self {
        Self { left: left.clamp(0.0, 1.0), right: right.clamp(0.0, 1.0) }
    }

    /// The yaw rate the belts FORCE at this speed, signed in the hull's convention (+ = turning
    /// right, because a right turn is the left belt outrunning the right one).
    ///
    /// A weak belt cannot match its partner, so the difference — and the turn — is not optional.
    /// The magnitude is the shortfall carried over the pivot arm: a fully dead belt gives
    /// `v / half_gauge`, which is exactly the working belt walking the hull round the dead one.
    /// Equal belts give zero, whatever their absolute grip, because equal belts do not turn a hull.
    pub fn forced_yaw_rate(&self, forward_speed_mps: f32, half_gauge_m: f32) -> f32 {
        if half_gauge_m <= f32::EPSILON {
            return 0.0;
        }
        // + shortfall on the right = the left belt outruns it = a forced RIGHT turn.
        DEAD_BELT_PIVOT_BITE * (self.left - self.right) * forward_speed_mps / (2.0 * half_gauge_m)
    }

    /// Share of nominal drive the pair can still transmit: the mean of the two belts.
    pub fn drive_fraction(&self) -> f32 {
        (self.left + self.right) * 0.5
    }
}

/// Maximum uphill grade (rise/run) a tank can climb. Steeper faces -- like the railway
/// embankment -- stall it.
///
/// The number itself lives in `game_core::mobility`, because the map contract's drive graph has
/// to refuse exactly the ground this refuses. Two hand-written copies meant a map could wall off
/// a slope the hull drives, and nobody would see the pair.
const DEFAULT_MAX_CLIMB_GRADE: f32 = game_core::MAX_CLIMB_GRADE;

/// Lateral grip coefficient. 0.95 keeps the hull on its line through normal turns and only lets
/// it break loose near the top of the speed/turn-rate envelope or on low-traction ground (the
/// terrain `traction` scales this down). Raise toward "on rails", lower toward "ice".
const DEFAULT_LATERAL_GRIP_MU: f32 = 0.95;

/// Drivetrain efficiency times the arcade condensation factor. Real tanks take 25+ seconds to
/// top out; the game wants the same *shape* (hard launch, grinding top end) compressed to roughly
/// half the wall-clock, so effective power is scaled up rather than the curve flattened.
const DRIVE_POWER_FACTOR: f32 = 0.75 * 2.2;

/// Rolling resistance of tracks on firm ground, m/s².
const ROLLING_RESIST_MPS2: f32 = 0.45;

/// Static track-lock grip. 0.9 (~42°) comfortably holds every drivable slope and most embankment
/// faces, so a stopped hull stays where you park it. Higher than the kinetic grips (mu_s > mu_k).
const STATIC_GRIP_MU: f32 = 0.9;

/// A hull slower than this (m/s) and off the throttle grabs and locks onto the slope.
const STATIC_HOLD_SPEED_MPS: f32 = 0.7;

/// Grade above which a face is a hard wall (cliff / embankment); 0.6–0.68 is the momentum-climb
/// band (~31° steady gradeability up to ~34° with a run-up). Set under the railway embankment's
/// ~0.71 head-on probe grade, so a straight charge is walled — but hitting the same face at an angle
/// drops the *forward* component of the grade below this, so a diagonal run-up can still scale it.
/// That angle-of-attack climb is emergent (forward_slope is the grade along the heading), and is
/// exactly the "clever, fair" steep climb: skill and commitment, not a straight bump-over.
const MOMENTUM_CLIMB_CEILING: f32 = 0.68;

impl TankControllerSettings {
    pub fn arcade_default() -> Self {
        Self::from_spec(&TankSpec::medium_test_tank())
    }

    pub fn from_spec(spec: &TankSpec) -> Self {
        let drive_power_mps3 =
            spec.engine_power_kw * 1000.0 * DRIVE_POWER_FACTOR / spec.mass_kg.max(1.0);
        let vmax = spec.max_forward_speed_mps.max(1.0);
        // Solve the quadratic drag so thrust == resistance exactly at the spec top speed.
        let drag_quadratic =
            ((drive_power_mps3 / vmax - ROLLING_RESIST_MPS2) / (vmax * vmax)).max(1.0e-4);

        // Heavier hulls take longer to reach their commanded yaw rate, so the rotation reads as
        // weight rather than an instant snap. spool ~0.25 s (light) to ~0.7 s (heavy).
        let yaw_spool_s = (spec.mass_kg / 120_000.0).clamp(0.25, 0.7);
        let yaw_accel_rad_s2 = (spec.turn_rate_rad_s / yaw_spool_s).max(0.1);

        Self {
            max_forward_speed_mps: spec.max_forward_speed_mps,
            max_reverse_speed_mps: spec.max_reverse_speed_mps,
            drive_power_mps3,
            min_force_speed_mps: 2.2,
            rolling_resist_mps2: ROLLING_RESIST_MPS2,
            drag_quadratic,
            // Tracks brake hard, but not "granite wall" hard: ~0.5 g reads as tonnes digging in.
            brake_deceleration_mps2: 4.8,
            turn_rate_rad_s: spec.turn_rate_rad_s,
            ground_probe_length_m: 3.0,
            idle_drag_mps2: 1.3,
            max_climb_grade: DEFAULT_MAX_CLIMB_GRADE,
            yaw_accel_rad_s2,
            longitudinal_grip_mu: DEFAULT_MAX_CLIMB_GRADE,
            lateral_grip_mu: DEFAULT_LATERAL_GRIP_MU,
            turn_scrub: 0.25,
            static_grip_mu: STATIC_GRIP_MU,
            static_hold_speed_mps: STATIC_HOLD_SPEED_MPS,
            momentum_climb_ceiling: MOMENTUM_CLIMB_CEILING,
            steering: SteeringKind::for_vehicle(spec.kind),
            pivot_arm_m: spec.contact_footprint().half_gauge_x,
            belts: BeltDrive::HEALTHY,
            hull_spring: hull_spring_for_spec(spec),
        }
    }
}

/// The hull's springs from the vehicle alone (Inny Poziom G7). The natural frequency is the one
/// law every sprung mass obeys, `ω = √(g / sag)`: the static deflection of the suspension is the
/// blueprint's arm rise (how far the trailing arm's pivot stands above the axle at rest), so a
/// Christie hull on its long-travel coils (0.22 m) wallows near 1.2 Hz where a torsion-bar hull
/// (0.13 m) sits near 1.5 Hz — and a T-54 and a Jagdtiger on the same bars share the frequency,
/// because mass cancels in steel sized for its load. Damping is the family's: torsion bars with
/// shock absorbers on the end stations 0.5, Christie coils with almost none 0.35, Horstmann
/// bogies with their friction 0.55. Weight transfer is the quasi-static tilt of the sprung mass,
/// `θ = 12·a·h_cg / (ω²·L²)`, over the wheelbase for pitch and the gauge for roll, with the
/// centre of mass from the stock stability model.
pub fn hull_spring_for_spec(spec: &TankSpec) -> HullSpring {
    let blueprint = VehicleBlueprint::for_vehicle(spec.kind);
    let (sag_m, family) = blueprint
        .as_ref()
        .map(|bp| (bp.track.arm_rise(), bp.track.suspension))
        .unwrap_or((0.13, SuspensionKind::TorsionBar));
    let omega_rad_s = (GRAVITY_MPS2 / sag_m.max(0.04)).sqrt();
    let zeta = match family {
        SuspensionKind::TorsionBar => 0.5,
        SuspensionKind::Christie => 0.35,
        SuspensionKind::Horstmann => 0.55,
    };
    let com_height_m = stock_stability(spec.kind).map_or(0.9, |s| s.com_height_m);
    let footprint = spec.contact_footprint();
    let wheelbase_m = footprint.wheelbase_m().max(spec.hitbox.half_length_m * 1.2).max(1.0);
    let gauge_m = (2.0 * footprint.half_gauge_x).max(1.0);
    let omega_sq = omega_rad_s * omega_rad_s;
    HullSpring {
        omega_rad_s,
        zeta,
        dive_rad_per_mps2: 12.0 * com_height_m / (omega_sq * wheelbase_m * wheelbase_m),
        lean_rad_per_mps2: 12.0 * com_height_m / (omega_sq * gauge_m * gauge_m),
    }
}

/// Inny Poziom G7, lock (2) — per-vehicle wallow, by what the suspension predicts and nothing
/// else: a Christie hull on its long-travel coils (T-34-85) settles at a lower natural frequency
/// than a torsion-bar hull (T-54) by the square root of their static deflections, a T-54 and a
/// Jagdtiger on the same bars share the frequency — mass cancels in steel sized for its load —
/// and every hull sits in the band tonnes of sprung steel actually ring at (0.8–2 Hz).
#[cfg(test)]
mod hull_spring_locks {
    use game_core::VehicleKind;

    use super::*;

    #[test]
    fn suspension_family_and_travel_set_the_wallow_not_the_mass() {
        let t54 = hull_spring_for_spec(&VehicleKind::T54_1951.spec());
        let t34 = hull_spring_for_spec(&VehicleKind::T34_85.spec());
        let jagdtiger = hull_spring_for_spec(&VehicleKind::Jagdtiger.spec());
        let predicted = (0.13_f32 / 0.22).sqrt();
        let ratio = t34.omega_rad_s / t54.omega_rad_s;
        assert!(
            (ratio - predicted).abs() < 0.05,
            "Christie over torsion bar: {ratio} vs {predicted}"
        );
        assert!(t34.zeta < t54.zeta, "coils with no dampers wallow longer than bars with them");
        assert!((jagdtiger.omega_rad_s - t54.omega_rad_s).abs() < 1.0e-3, "mass cancels");
        for (name, spring) in [("T-54", t54), ("T-34-85", t34), ("Jagdtiger", jagdtiger)] {
            let hz = spring.omega_rad_s / std::f32::consts::TAU;
            assert!((0.8..=2.0).contains(&hz), "{name} rings at {hz} Hz");
            assert!(spring.dive_rad_per_mps2 > 0.0 && spring.lean_rad_per_mps2 > 0.0);
        }
    }
}
