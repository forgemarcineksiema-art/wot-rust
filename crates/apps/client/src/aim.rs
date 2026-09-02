//! Mouse input owns a desired sight ray: independent yaw/pitch that drives the camera target.
//! The gun then solves toward the world point under that sight ray. The shot path remains
//! authoritative server/sim state; this module only provides client-side aim prediction.

use game_core::math::{
    HullPose, gun_direction, gun_direction_world, world_direction_to_turret, wrap_angle,
};
use game_core::{TankId, TeamId};
use glam::Vec3;
use net::TankSnapshot;
use sim::{ShellTraceWorld, segment_impact};
use terrain::{HeightMap, StaticCoverObject};

// Sight rays must out-range the map: Prokhorovka targets 1000 m of playable space, and a sniper
// duel across the long diagonal aims well past 600 m.
const AIM_MAX_RANGE_M: f32 = 1200.0;
const GUN_PITCH_SOLVER_MIN_RAD: f32 = -0.5;
const GUN_PITCH_SOLVER_MAX_RAD: f32 = 0.8;
const GUN_PITCH_SOLVER_STEPS: usize = 18;

/// The player's sight ray as a **world-space** direction: yaw is the world azimuth, pitch is the
/// world elevation of the line the crosshair points along (NOT a hull-relative gun angle). The gun
/// then solves — hull-aware — toward the world point under this ray, and the app clamps the pitch
/// to what the gun can actually reach on the current hull (`clamp_desired_aim_to_gun_reach`). This
/// separation is what makes the sniper view correct on slopes: the sight looks where you point in
/// the world, independent of hull tilt, while the gun carries the tilt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DesiredAim {
    yaw_rad: f32,
    pitch_rad: f32,
}

/// Safety bound on the world sight elevation (~46°). The real limit is the per-frame gun-reach
/// clamp; this only stops the accumulated pitch from running away past anything sane.
const VIEW_PITCH_LIMIT_RAD: f32 = 0.8;

impl DesiredAim {
    pub(crate) fn new(yaw_rad: f32, pitch_rad: f32) -> Self {
        Self {
            yaw_rad: wrap_angle(yaw_rad),
            pitch_rad: pitch_rad.clamp(-VIEW_PITCH_LIMIT_RAD, VIEW_PITCH_LIMIT_RAD),
        }
    }

    pub(crate) fn yaw_rad(self) -> f32 {
        self.yaw_rad
    }

    pub(crate) fn pitch_rad(self) -> f32 {
        self.pitch_rad
    }

    pub(crate) fn set_yaw(&mut self, yaw_rad: f32) {
        self.yaw_rad = wrap_angle(yaw_rad);
    }

    /// Set the world sight elevation directly (used by the gun-reach clamp), kept in the safety bound.
    pub(crate) fn set_pitch(&mut self, pitch_rad: f32) {
        self.pitch_rad = pitch_rad.clamp(-VIEW_PITCH_LIMIT_RAD, VIEW_PITCH_LIMIT_RAD);
    }

    pub(crate) fn apply_pitch_delta(&mut self, delta_rad: f32) {
        self.set_pitch(self.pitch_rad + delta_rad);
    }
}

impl Default for DesiredAim {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// March the screen-center ray (`eye` + t·`forward`) to the first terrain hit, returning the
/// world aim point. Off-map samples count as open sky, so a ray that clears the map aims at
/// its far end rather than snapping short.
#[cfg(test)]
pub(crate) fn aim_point(heightmap: &HeightMap, eye: Vec3, forward: Vec3) -> Vec3 {
    const STEP_M: f32 = 1.0;
    let mut previous = eye;
    let mut previous_clearance = clearance(heightmap, eye);
    let mut travelled = STEP_M;
    while travelled <= AIM_MAX_RANGE_M {
        let point = eye + forward * travelled;
        let clearance = clearance(heightmap, point);
        if previous_clearance > 0.0 && clearance <= 0.0 {
            let frac = previous_clearance / (previous_clearance - clearance);
            return previous.lerp(point, frac.clamp(0.0, 1.0));
        }
        previous = point;
        previous_clearance = clearance;
        travelled += STEP_M;
    }
    eye + forward * AIM_MAX_RANGE_M
}

/// Where the sight ray ends, and whether it ended on anything.
///
/// A ray that clears the map runs out at [`AIM_MAX_RANGE_M`] and the sight aims at that point in
/// mid-air — which is correct for laying the gun, and a lie for any readout: the distance to it
/// is the ray's own length, not a target's.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SightPoint {
    pub point: Vec3,
    /// `false` for open sky: the ray met terrain, cover, water nor hull inside its reach.
    pub on_surface: bool,
}

/// `projectile_radius_m` is the BODY the sight probes the world with, and it must be the loaded
/// shell's — the same radius the trace beside it flies.
///
/// It used to be zero: the ray asked "what would a needle hit along this line" while the trace
/// asked "what would THIS SHELL hit", and the two disagreed by exactly one radius. Measured on
/// Bystra, that difference WAS the defect for **35 of the 36** refusals where the target is not
/// cut anywhere in the picture (register I3): the needle threaded a barn roof by about 7 cm, the
/// shell grazed it, and the crosshair sat on a tank at 327 m while the round died on masonry at
/// 120 m. With the shell's own body the sweep stops ON that roof — the readout says 120 M and the
/// player is visibly aiming at a barn, which is a picture that explains itself.
#[expect(clippy::too_many_arguments)]
pub(crate) fn aim_point_with_sweep(
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    water: terrain::WaterView<'_>,
    tanks: &[TankSnapshot],
    owner: TankId,
    owner_team: TeamId,
    eye: Vec3,
    forward: Vec3,
    projectile_radius_m: f32,
) -> SightPoint {
    let sets = crate::hud::reticle_sweep::trace_sets(tanks, owner, owner_team);
    let world = ShellTraceWorld {
        projectile_radius_m,
        tanks: &sets.targets,
        blockers: &sets.blockers,
        heightmap: Some(heightmap),
        cover,
        water,
    };
    // ONE nearest-impact query over the whole sight ray. The old outer 1 m march re-tested
    // every tank OBB and cover slab per metre (~30k tests per sweep, and this sweep runs every
    // fixed tick AND every frame); `segment_impact` already resolves the nearest hit exactly on
    // a segment of any length — the slab/OBB tests are analytic, and the terrain sweep steps
    // internally. Same answer, three orders of magnitude fewer intersection tests.
    // Straight sight ray, so `forward` stands in for the shell velocity; the aim point only
    // needs the impact location, not the (unused) tank impact angle.
    match segment_impact(eye, eye + forward * AIM_MAX_RANGE_M, forward, &world) {
        Some(impact) => SightPoint { point: impact.point(), on_surface: true },
        None => SightPoint { point: eye + forward * AIM_MAX_RANGE_M, on_surface: false },
    }
}

/// The shot this gun will actually take toward a sight point: the ballistic arc solved in world
/// space, folded through the hull pose, and clamped to the gun's own elevation limits.
///
/// ONE definition of "the shot I am asking for", shared by the gun commands (which chase it) and
/// by the reticle (which previews it). They used to derive it separately, and the reticle's copy
/// compared a WORLD elevation against HULL-relative limits — so a nose-down hull on a ridge, the
/// whole point of hull-down, read as "out of arc" while the gun was happily on target.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FiringSolution {
    /// World direction the shell leaves along once the gun has settled.
    pub world_direction: Vec3,
    /// Hull-relative gun elevation after the arc clamp.
    pub gun_pitch_rad: f32,
    /// Hull-relative turret bearing that puts the muzzle *through* the sight point.
    pub turret_yaw_rad: f32,
    /// Whether the arc could reach the ballistic solution at all — false means the clamp bit,
    /// and no amount of waiting will put a shell on that point from this hull.
    pub in_arc: bool,
    /// WHICH end of the arc bit, when one did (Inny Poziom A3). A gun that cannot depress onto
    /// a target below the crest and a gun that cannot elevate onto a roof are two different
    /// lessons for the player, and both used to wear the same broken form as a wall. A casemate
    /// whose hull line does not pass through the sight point reports `Traverse` (A4): the shot
    /// leaves along the hull line, and whether THAT arrives is what the trace then judges.
    pub arc_limit: Option<ArcLimit>,
}

/// The end of the gun arc a clamped solution ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArcLimit {
    /// The sight point sits below what this hull, on this slope, can depress to.
    Depression,
    /// The sight point sits above what the gun can elevate to.
    Elevation,
    /// The sight point sits off the bearing a fixed casemate can point at — the hull line.
    Traverse,
}

/// `None` when the sight point sits on the muzzle itself: there is no bearing to solve, so the
/// caller keeps its own fallback (hold the traverse / fly the current barrel).
pub(crate) fn firing_solution(
    muzzle: Vec3,
    aim: Vec3,
    hull: HullPose,
    gun_pitch_limits_rad: (f32, f32),
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    // A fixed casemate cannot traverse: the sim forces its turret yaw to the hull line
    // (`TankSpec::effective_turret_yaw_rad`), so the solution must fly THAT bearing (A4).
    fixed_casemate: bool,
) -> Option<FiringSolution> {
    let delta = aim - muzzle;
    if delta.x.abs() <= 1.0e-4 && delta.z.abs() <= 1.0e-4 {
        return None;
    }
    let world_pitch = gun_pitch_to_hit(muzzle, aim, muzzle_velocity_mps, drag_per_s);
    let world_direction = gun_direction(delta.x.atan2(delta.z), world_pitch);
    let (solved_yaw, solved_pitch) = world_direction_to_turret(hull, world_direction);
    // The bearing the gun can actually take. A turret takes the solved one; a casemate takes
    // the hull line, and the shell leaves along it whatever the sight asked for — the trace
    // downstream then judges whether that shot still arrives (a sight point a hair off the
    // line does; one 30° off does not).
    let turret_yaw_rad = if fixed_casemate { 0.0 } else { solved_yaw };
    let yaw_clamped = (turret_yaw_rad - solved_yaw).abs() > 1.0e-6;
    let (min_pitch, max_pitch) = gun_pitch_limits_rad;
    let gun_pitch_rad = solved_pitch.clamp(min_pitch, max_pitch);
    let in_arc = (gun_pitch_rad - solved_pitch).abs() <= 1.0e-6;
    let arc_limit = if !in_arc {
        Some(if solved_pitch < min_pitch { ArcLimit::Depression } else { ArcLimit::Elevation })
    } else if yaw_clamped {
        Some(ArcLimit::Traverse)
    } else {
        None
    };
    Some(FiringSolution {
        world_direction: gun_direction_world(hull, turret_yaw_rad, gun_pitch_rad),
        gun_pitch_rad,
        turret_yaw_rad,
        in_arc,
        arc_limit,
    })
}

/// Gun elevation (radians, + = up) that puts the muzzle on `aim` using the same fixed-step
/// ballistic integration (drag included) as the authoritative shell trace.
pub(crate) fn gun_pitch_to_hit(
    muzzle: Vec3,
    aim: Vec3,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
) -> f32 {
    let delta = aim - muzzle;
    let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
    if horizontal < 0.5 {
        return 0.0;
    }
    if muzzle_velocity_mps <= 0.0 {
        return delta.y.atan2(horizontal);
    }
    let mut low = GUN_PITCH_SOLVER_MIN_RAD;
    let mut high = GUN_PITCH_SOLVER_MAX_RAD;
    for _ in 0..GUN_PITCH_SOLVER_STEPS {
        let mid = (low + high) * 0.5;
        if shell_height_at_horizontal(muzzle, mid, muzzle_velocity_mps, drag_per_s, horizontal)
            < aim.y
        {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) * 0.5
}

fn shell_height_at_horizontal(
    muzzle: Vec3,
    pitch_rad: f32,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    horizontal: f32,
) -> f32 {
    let dt_seconds = crate::hud::reticle_sweep::tick_dt_seconds();
    let mut position = muzzle;
    let mut velocity = gun_direction(0.0, pitch_rad) * muzzle_velocity_mps;
    let mut previous_range = 0.0;
    for _ in 0..(sim::SHELL_MAX_AGE_SECONDS / dt_seconds).ceil() as usize {
        let previous = position;
        game_core::math::integrate_shell_step(&mut velocity, drag_per_s, dt_seconds);
        position += velocity * dt_seconds;
        let range = ((position.x - muzzle.x).powi(2) + (position.z - muzzle.z).powi(2)).sqrt();
        if range >= horizontal {
            let frac = (horizontal - previous_range) / (range - previous_range);
            return previous.y + (position.y - previous.y) * frac.clamp(0.0, 1.0);
        }
        previous_range = range;
    }
    position.y
}

#[cfg(test)]
fn clearance(heightmap: &HeightMap, point: Vec3) -> f32 {
    heightmap.sample_height(point.x, point.z).map_or(f32::MAX, |ground| point.y - ground)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_flat_ground_ahead_of_the_camera() {
        let flat = HeightMap::flat(64, 64, 5.0, 0.0).unwrap();
        let eye = Vec3::new(100.0, 10.0, 100.0);
        let forward = Vec3::new(0.0, -1.0, 1.0).normalize();
        let hit = aim_point(&flat, eye, forward);
        // Descends 10 m over 10 m of +Z before reaching ground level.
        assert!(hit.y.abs() < 0.5, "hit near ground, y = {}", hit.y);
        assert!((hit.z - 110.0).abs() < 1.5, "hit ~10 m ahead, z = {}", hit.z);
    }

    #[test]
    fn open_sky_ray_aims_at_max_range() {
        let flat = HeightMap::flat(64, 64, 5.0, 0.0).unwrap();
        let eye = Vec3::new(100.0, 10.0, 100.0);
        let hit = aim_point(&flat, eye, Vec3::Y); // straight up never meets ground
        assert!((hit.y - (10.0 + AIM_MAX_RANGE_M)).abs() < 1.0e-3);
    }

    #[test]
    fn elevation_rises_for_higher_targets() {
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let level = gun_pitch_to_hit(muzzle, Vec3::new(0.0, 1.0, 100.0), 895.0, 0.09);
        assert!(level.abs() < 0.02, "level shot ~ 0, got {level}");
        let raised = gun_pitch_to_hit(muzzle, Vec3::new(0.0, 21.0, 100.0), 895.0, 0.09);
        assert!(raised > 0.15, "20 m up over 100 m elevates the gun, got {raised}");
    }

    #[test]
    fn elevation_matches_authoritative_shell_step_for_slow_long_shot() {
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let aim = Vec3::new(0.0, 1.0, 500.0);
        let pitch = gun_pitch_to_hit(muzzle, aim, 500.0, 0.09);
        let shell_y = shell_height_at_horizontal(muzzle, pitch, 500.0, 0.09, 500.0);
        assert!(
            (shell_y - aim.y).abs() < 0.02,
            "pitch {pitch} should put the simulated shell at target height {}, got {shell_y}",
            aim.y
        );
    }

    /// The single-segment sweep is an optimization, not a behavior change: on a scene with
    /// terrain relief, cover, and a tank, it must return the same aim point as the old 1 m
    /// stepped march (inlined here as the oracle) for rays that hit each obstacle class — and
    /// for the open-sky miss.
    #[test]
    fn the_single_segment_sweep_matches_the_stepped_oracle() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let heightmap = &battlefield.heightmap;
        let cover = &battlefield.static_cover;
        let spec = game_core::VehicleKind::T54_1951.spec();
        let tank = net::TankSnapshot {
            tank_id: TankId(2),
            team: TeamId(2),
            vehicle: game_core::VehicleKind::T54_1951,
            position: [500.0, heightmap.sample_height(500.0, 460.0).unwrap(), 460.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: spec.hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
        };
        let tanks = [tank];
        let eye_ground = heightmap.sample_height(500.0, 400.0).unwrap();

        let oracle = |eye: Vec3, forward: Vec3| -> Vec3 {
            let sets = crate::hud::reticle_sweep::trace_sets(&tanks, TankId(1), TeamId(1));
            let world = ShellTraceWorld {
                projectile_radius_m: 0.0,
                tanks: &sets.targets,
                blockers: &sets.blockers,
                heightmap: Some(heightmap),
                cover,
                water: terrain::WaterView::DRY,
            };
            let mut previous = eye;
            let mut travelled = 1.0_f32;
            while travelled <= AIM_MAX_RANGE_M {
                let point = eye + forward * travelled;
                if let Some(impact) = segment_impact(previous, point, forward, &world) {
                    return impact.point();
                }
                previous = point;
                travelled += 1.0;
            }
            eye + forward * AIM_MAX_RANGE_M
        };

        let rays = [
            // Into the enemy hull 60 m ahead.
            (Vec3::new(500.0, eye_ground + 2.0, 400.0), Vec3::new(0.0, -0.005, 1.0).normalize()),
            // Down into the terrain.
            (Vec3::new(500.0, eye_ground + 2.0, 400.0), Vec3::new(0.3, -0.15, 1.0).normalize()),
            // Along the map into cover (the Oktyabrskiy barns sit near mid-map).
            (Vec3::new(430.0, eye_ground + 2.5, 470.0), Vec3::new(1.0, 0.0, 0.0).normalize()),
            // Open sky.
            (Vec3::new(500.0, eye_ground + 2.0, 400.0), Vec3::new(0.0, 0.5, 1.0).normalize()),
        ];
        for (eye, forward) in rays {
            let fast = aim_point_with_sweep(
                heightmap,
                cover,
                terrain::WaterView::DRY,
                &tanks,
                TankId(1),
                TeamId(1),
                eye,
                forward,
                0.0,
            );
            let slow = oracle(eye, forward);
            assert!(
                fast.point.distance(slow) < 1.0e-3,
                "sweep diverged from the stepped oracle: {:?} vs {slow:?} (eye {eye:?})",
                fast.point
            );
        }
    }

    /// The sweep reports whether the sight ray landed on anything. A ray into the sky runs out
    /// at its own maximum reach — the point is right for laying the gun, and the distance to it
    /// is a fact about the ray, not about the battlefield, so the readouts must not print it.
    #[test]
    fn the_sweep_reports_open_sky_apart_from_a_real_target() {
        let flat = HeightMap::flat(64, 64, 5.0, 0.0).unwrap();
        let eye = Vec3::new(100.0, 10.0, 100.0);
        let sweep = |forward: Vec3| {
            aim_point_with_sweep(
                &flat,
                &[],
                terrain::WaterView::DRY,
                &[],
                TankId(1),
                TeamId(1),
                eye,
                forward,
                0.0,
            )
        };

        let ground = sweep(Vec3::new(0.0, -1.0, 1.0).normalize());
        assert!(ground.on_surface, "a ray into the ground has a target");

        let sky = sweep(Vec3::new(0.0, 0.5, 1.0).normalize());
        assert!(!sky.on_surface, "a ray over the horizon has none");
        assert!(
            (sky.point - eye).length() > AIM_MAX_RANGE_M - 1.0,
            "and it ends at the ray's own reach"
        );
    }

    /// The sight probes the world with the SHELL'S body, not with a needle.
    ///
    /// Zero radius here against the trace's calibre radius beside it was the whole of register I3:
    /// on Bystra it produced 35 of the 36 refusals where the target stands completely open in the
    /// picture. The ray threaded a barn roof by centimetres, the round grazed it, and the sight
    /// showed a clean shot at a tank three hundred metres past the masonry that ate it.
    ///
    /// The scene below is that graze, minimised: a slab whose top sits just under the ray. The
    /// needle passes; the shell does not. Both halves are asserted, so this turns red if the
    /// radius stops being wired through — not merely if the geometry moves.
    #[test]
    fn the_sight_ray_stops_on_what_the_shell_cannot_clear() {
        let flat = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
        let eye = Vec3::new(40.0, 2.0, 40.0);
        let forward = Vec3::Z;
        // Roof at 2.0 - 0.03: three centimetres under a level ray, inside a 100 mm shell's radius.
        let barn = StaticCoverObject {
            id: "barn".to_string(),
            name: "barn".to_string(),
            kind: terrain::StaticCoverKind::FarmBuilding,
            center: [40.0, 0.985, 80.0],
            half_extents_m: [6.0, 0.985, 4.0],
        };
        let sweep = |radius| {
            aim_point_with_sweep(
                &flat,
                std::slice::from_ref(&barn),
                terrain::WaterView::DRY,
                &[],
                TankId(1),
                TeamId(1),
                eye,
                forward,
                radius,
            )
        };

        let needle = sweep(0.0);
        assert!(
            needle.point.z > 200.0,
            "the scene must be a genuine graze: a needle threads it, landing at {:?}",
            needle.point
        );

        let shell = game_core::VehicleKind::T54_1951.spec().gun.shell;
        assert!(shell.collision_radius_m() > 0.03, "the T-54's round is wider than the gap");
        let round = sweep(shell.collision_radius_m());
        assert!(round.on_surface, "the shell's body meets the roof");
        assert!(
            (round.point.z - 76.0).abs() < 1.0,
            "and the crosshair stops ON the barn instead of a target beyond it: {:?}",
            round.point
        );
    }

    #[test]
    fn drag_demands_more_elevation_at_range() {
        // The same shot solved in still air and in drag: the draggy shell arrives slower and
        // drops longer, so the solver must hold the gun higher — one integration for the arc
        // you fly and the arc you solve.
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let aim = Vec3::new(0.0, 1.0, 800.0);
        let vacuum = gun_pitch_to_hit(muzzle, aim, 500.0, 0.0);
        let dragged = gun_pitch_to_hit(muzzle, aim, 500.0, 0.21);
        assert!(dragged > vacuum + 1.0e-3, "drag must raise the solution: {dragged} vs {vacuum}");
    }
}
