//! The seam the sight is judged on: **what the sniper eye reaches, the gun must be able to
//! reach** — measured end to end on shipped map terrain, and ratcheted.
//!
//! Every other test beside this one hands the reticle a synthetic heightmap and asks whether it
//! answers correctly. Not one of them could see the defect reported from the game on 2026-08-07:
//! a plainly visible T-54 at 320 m, the crosshair on its hull, the gray BLOCKED form, and the
//! shell in the dirt. That defect does not live inside the reticle — the reticle was right every
//! time. It lives between two ORIGINS. The sniper eye sits above the gun axis, and a shell fired
//! at 320 m leaves the muzzle about 2 mrad above the line to its target, so for the near half of
//! the flight the eye looks OVER folds of ground the shell flies INTO. Nothing in the picture
//! says so, because the fold is under the sight line by centimetres.
//!
//! So this file measures the seam itself — camera eye, sight sweep, firing solution, and the
//! authoritative trace, in the order the game runs them — over thousands of placements on real
//! terrain, and locks the result as an upper bound. A change that tightens the sight passes with
//! room; one that quietly reopens the gap turns red.
//!
//! Population, per map: pairs of settled T-54 hulls 260..400 m apart, drawn from a fixed LCG, both
//! on dry land, the crosshair on the target's turret. Kept: the pairs whose sight ray reaches the
//! enemy hull and whose firing solution is inside the gun's arc — i.e. every shot the player is
//! entitled to believe in.

use game_core::math::HullPose;
use game_core::{MountFrames, TankId, TeamId, VehicleKind};
use glam::Vec3;
use terrain::MapId;

use super::tests::tank_snapshot;

/// Placements drawn per map. Enough that a single sample moving cannot flip the ratchet, cheap
/// enough to live in the workspace run (both maps together are well under a second of work after
/// the two map compiles).
const PLACEMENTS: usize = 30_000;

#[derive(Debug, Default, Clone, Copy)]
struct Seam {
    /// Placements whose sight ray reached the enemy hull inside the gun's arc.
    believable: usize,
    /// ...of which the traced shell does not reach that hull.
    refused: usize,
    /// ...of which the eye ALSO reaches every point of the target's silhouette, belly to roof —
    /// the target looks completely open and the shot is refused anyway. This is the count that
    /// has no honest defence: every other refusal at least cuts the tank in the picture.
    refused_while_uncut: usize,
}

/// One pass of the seam over a shipped map.
fn measure(map: MapId) -> Seam {
    let battlefield = map_forge::battlefield(map);
    let heightmap = &battlefield.heightmap;
    let cover = &battlefield.static_cover;
    let kind = VehicleKind::T54_1951;
    let spec = kind.spec();
    let mounts = MountFrames::for_vehicle(kind);
    let shell = spec.gun.shell;
    let muzzle_velocity = shell.muzzle_velocity_mps;
    let drag = shell.drag_per_s();
    let limits = spec.gun_pitch_limits_rad();
    let ground = |x: f32, z: f32| heightmap.sample_height(x, z);

    // A hull settled on the ground it stands on: pitch and roll from the terrain under its own
    // footprint, so the muzzle rides the slope exactly like a parked tank's does.
    let settled = |x: f32, z: f32, yaw: f32| -> Option<HullPose> {
        let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let (half_length, half_width) = (3.0_f32, 1.6_f32);
        let front = ground(x + forward.x * half_length, z + forward.z * half_length)?;
        let rear = ground(x - forward.x * half_length, z - forward.z * half_length)?;
        let port = ground(x - right.x * half_width, z - right.z * half_width)?;
        let starboard = ground(x + right.x * half_width, z + right.z * half_width)?;
        Some(HullPose {
            yaw_rad: yaw,
            pitch_rad: ((front - rear) / (2.0 * half_length)).atan(),
            roll_rad: ((starboard - port) / (2.0 * half_width)).atan(),
        })
    };

    let extent = battlefield.size_m;
    let mut seed = 0x1234_5678_u64;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as f32) / ((1u64 << 31) as f32)
    };
    let mut seam = Seam::default();

    for _ in 0..PLACEMENTS {
        let shooter_x = 60.0 + next() * (extent[0] - 120.0);
        let shooter_z = 60.0 + next() * (extent[1] - 120.0);
        let bearing = next() * std::f32::consts::TAU;
        let range = 260.0 + next() * 140.0;
        let target_x = shooter_x + bearing.sin() * range;
        let target_z = shooter_z + bearing.cos() * range;
        let (Some(shooter_y), Some(target_y)) =
            (ground(shooter_x, shooter_z), ground(target_x, target_z))
        else {
            continue;
        };
        // Neither hull in the water: a drowning tank is not a shot anybody is owed.
        if let Some(water) = battlefield.water
            && (water.depth_over(shooter_y) > 0.0 || water.depth_over(target_y) > 0.0)
        {
            continue;
        }
        let Some(shooter_hull) = settled(shooter_x, shooter_z, bearing) else { continue };
        let Some(target_hull) = settled(target_x, target_z, bearing + std::f32::consts::FRAC_PI_2)
        else {
            continue;
        };

        let shooter_position = Vec3::new(shooter_x, shooter_y, shooter_z);
        let target_position = Vec3::new(target_x, target_y, target_z);
        let mut target = tank_snapshot(TankId(2), target_position.to_array());
        target.yaw_rad = target_hull.yaw_rad;
        target.hull_pitch_rad = target_hull.pitch_rad;
        target.hull_roll_rad = target_hull.roll_rad;
        let tanks = [target];

        // The game's own sight ray: from the sniper eye, along the crosshair.
        let eye = crate::camera::sniper_eye_from_base(kind, shooter_position, shooter_hull);
        let aim_at = target_position + Vec3::Y * 1.6;
        let forward = (aim_at - eye).normalize_or_zero();
        let sets = crate::hud::reticle_sweep::trace_sets(&tanks, TankId(1), TeamId(1));
        // Two probes, two questions. The sight sweep carries the SHELL's body, exactly as
        // `ClientApp::sight_point` does — it asks what this round would meet. The silhouette
        // probe below carries none, because it asks what the picture SHOWS, and light has no
        // calibre. Sharing one radius between them would make the instrument agree with itself
        // by construction and see nothing.
        let world = sim::ShellTraceWorld {
            projectile_radius_m: shell.collision_radius_m(),
            tanks: &sets.targets,
            blockers: &sets.blockers,
            heightmap: Some(heightmap),
            cover,
            water: battlefield.water,
        };
        let eye_world = sim::ShellTraceWorld { projectile_radius_m: 0.0, ..world };
        let sight = sim::segment_impact(eye, eye + forward * 1200.0, forward, &world);
        let Some(sim::SegmentImpact::Tank { .. }) = sight else { continue };
        let sight_point = sight.expect("matched above").point();

        // The shot that sight point asks for. The muzzle moves with the elevation the solution
        // wants, so it is resolved the way the app resolves it: solve, re-seat the muzzle, solve.
        let mut gun_pitch = 0.0_f32;
        let mut muzzle = Vec3::ZERO;
        let mut solution = None;
        for _ in 0..3 {
            muzzle = game_core::math::muzzle_world_position_scaled(
                &mounts,
                shooter_position,
                shooter_hull,
                0.0,
                gun_pitch,
                1.0,
            );
            solution = crate::aim::firing_solution(
                muzzle,
                sight_point,
                shooter_hull,
                limits,
                muzzle_velocity,
                drag,
            );
            let Some(solved) = solution else { break };
            gun_pitch = solved.gun_pitch_rad;
        }
        // Out of arc is an honest refusal with its own signal (the gun visibly cannot get there),
        // and it is a vehicle/map question, not a sight one.
        let Some(solution) = solution.filter(|solution| solution.in_arc) else { continue };
        seam.believable += 1;

        let outcome = crate::hud::reticle_sweep::reticle_trace(
            crate::hud::reticle_sweep::ReticleTraceQuery {
                heightmap,
                cover,
                water: battlefield.water,
                tanks: &tanks,
                owner: TankId(1),
                owner_team: TeamId(1),
                muzzle,
                gun_direction: solution.world_direction,
                muzzle_velocity_mps: muzzle_velocity,
                projectile_radius_m: shell.collision_radius_m(),
                drag_per_s: drag,
            },
        );
        if matches!(outcome, sim::TraceOutcome::Tank { .. }) {
            continue;
        }
        seam.refused += 1;

        // Does the picture cut the tank anywhere? Five points up the silhouette: belly, tracks,
        // hull top, turret, roof.
        let uncut = [0.1_f32, 0.6, 1.1, 1.7, 2.2].iter().all(|height| {
            let point = target_position + Vec3::Y * *height;
            let along = (point - eye).normalize_or_zero();
            matches!(
                sim::segment_impact(eye, point + along * 0.5, along, &eye_world),
                Some(sim::SegmentImpact::Tank { .. })
            )
        });
        if uncut {
            seam.refused_while_uncut += 1;
        }
    }
    seam
}

/// Rate in parts per ten thousand, as integer arithmetic — no float comparison to drift with the
/// last digit of a heightmap sample.
///
/// Per MILLE is the scale this started on and it has already stopped resolving: Bystra now refuses
/// 6 shots in 9 849, which rounds to zero there. A ceiling that reads zero cannot be tightened and
/// cannot say by how much it was missed.
fn per_ten_thousand(part: usize, whole: usize) -> usize {
    part * 10_000 / whole.max(1)
}

/// The sight may not offer a shot the gun cannot take — and above all, it may not offer one while
/// the target stands completely open in the picture.
///
/// The bounds below are MEASURED, not chosen: today's numbers with roughly half again as headroom,
/// so they answer "did this change reopen the gap?" rather than "is this number pretty?". They are
/// tight enough that undoing either wave turns them red on its own — reverting the optic height
/// alone puts Bystra at 38 against a ceiling of 10. Raising one is a deliberate decision that
/// comes with a fresh measurement, never a way to get a run green.
#[test]
fn what_the_sniper_eye_reaches_the_gun_can_reach() {
    for (map, refused_ceiling, uncut_ceiling) in
        [(MapId::BystraValley, 10, 4), (MapId::ProkhorovkaHill252_2, 40, 12)]
    {
        let seam = measure(map);
        assert!(
            seam.believable > 5_000,
            "{map:?}: the population must be large enough to mean something, got {}",
            seam.believable
        );
        let refused = per_ten_thousand(seam.refused, seam.believable);
        let uncut = per_ten_thousand(seam.refused_while_uncut, seam.believable);
        // Printed, not just asserted: re-measuring after a terrain or ballistics change is the
        // point of this instrument, and `cargo test -- --nocapture` is how the ceilings above
        // were taken.
        eprintln!("{map:?}: {seam:?}  refused={refused}/10000  uncut={uncut}/10000");
        assert!(
            refused <= refused_ceiling,
            "{map:?}: {}/{} sight-reachable hulls the gun cannot reach = {refused} per ten \
             thousand, over the {refused_ceiling} ceiling",
            seam.refused,
            seam.believable,
        );
        assert!(
            uncut <= uncut_ceiling,
            "{map:?}: {}/{} refusals with the target NOT cut anywhere in the picture = {uncut} \
             per ten thousand, over the {uncut_ceiling} ceiling — the sight is refusing a shot \
             the player has no way to know is refused",
            seam.refused_while_uncut,
            seam.believable,
        );
    }
}
