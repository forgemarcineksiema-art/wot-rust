use game_core::TeamId;
use net::TankSnapshot;
use sim::{FixedTimestep, SimulationState};

use super::*;

/// The point of sharing one drive step: the locally predicted hull must track the authoritative
/// server tick-for-tick — pose *and* aim dispersion — so the player never sees an aim circle (or a
/// position) the server is not simulating.
#[test]
fn predictor_matches_the_server_pose_and_dispersion_tick_for_tick() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t54_1951();
    let step = FixedTimestep::from_hz(60);

    let mut server = SimulationState::new();
    let tank_id = server.spawn_tank(TeamId(1), spec.clone(), Vec3::new(10.0, 0.0, 10.0));
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&TankSnapshot::from(server.tank(tank_id).expect("spawned tank")));

    // Drive, steer, and traverse at once so movement, aiming, and bloom all exercise the step.
    let command = TankCommand {
        throttle: 1.0,
        steer: 0.3,
        turret_yaw_delta: 0.7,
        gun_pitch_delta: 0.4,
        brake: 0.0,
        fire: false,
        select_ammo: None,
    };

    for tick in 0..60 {
        server.apply_commands_on_terrain(&[(tank_id, command)], step, &flat);
        predictor.step(command, &flat, &[], &[], &[], None, step.dt_seconds());

        let tank = server.tank(tank_id).expect("tank");
        assert!(
            (predictor.position() - tank.position).length() < 1.0e-2,
            "tick {tick}: predicted {:?} vs server {:?}",
            predictor.position(),
            tank.position
        );
        assert!((predictor.yaw() - tank.yaw_rad).abs() < 1.0e-3, "tick {tick}: yaw");
        assert!(
            (predictor.turret_yaw() - tank.turret_yaw_rad).abs() < 1.0e-4,
            "tick {tick}: turret"
        );
        assert!((predictor.gun_pitch() - tank.gun_pitch_rad).abs() < 1.0e-4, "tick {tick}: pitch");
        assert!(
            (predictor.aim_dispersion_mrad() - tank.aim_dispersion_mrad).abs() < 1.0e-3,
            "tick {tick}: dispersion {} vs {}",
            predictor.aim_dispersion_mrad(),
            tank.aim_dispersion_mrad
        );
    }
}

/// Vertical dynamics run the same shared code: launching off a ledge, the ballistic arc, and the
/// landing must match the server tick-for-tick — including the hull height mid-flight.
#[test]
fn predictor_matches_the_server_through_a_launch_flight_and_landing() {
    // A 4 m plateau ending at z = 24 on a 1 m grid, then flat ground: tall enough for a real
    // multi-tick flight, small enough to keep the run inside the map.
    let mut samples = Vec::with_capacity(61 * 61);
    for z in 0..61 {
        for x in 0..61 {
            let _ = x;
            samples.push(if z < 24 { 4.0 } else { 0.0 });
        }
    }
    let map = HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed");
    let spec = TankSpec::t54_1951();
    let step = FixedTimestep::from_hz(60);

    let mut server = SimulationState::new();
    let tank_id = server.spawn_tank(TeamId(1), spec.clone(), Vec3::new(30.0, 4.0, 10.0));
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&TankSnapshot::from(server.tank(tank_id).expect("spawned tank")));

    let command = TankCommand::drive(1.0, 0.0);
    let mut flew = false;
    for tick in 0..600 {
        server.apply_commands_on_terrain(&[(tank_id, command)], step, &map);
        predictor.step(command, &map, &[], &[], &[], None, step.dt_seconds());

        let tank = server.tank(tank_id).expect("tank");
        assert!(
            (predictor.position() - tank.position).length() < 1.0e-3,
            "tick {tick}: predicted {:?} vs server {:?}",
            predictor.position(),
            tank.position
        );
        // The hull is airborne once its height is off both plateau and ground level.
        if tank.position.y > 0.05 && tank.position.y < 3.95 {
            flew = true;
        }
    }
    assert!(flew, "the run must include a real flight, not just the plateau and the floor");
    let tank = server.tank(tank_id).expect("tank");
    assert!(tank.position.y < 0.05, "the run must end landed on the low ground");
}

/// RAMMING A LIVE HULL — the manoeuvre the player reported as vibration: hit someone's flank at
/// speed and the picture buzzes.
///
/// Nothing in the simulation shakes; a flank impact was measured clean at every impact point. The
/// shake was the seam. The server let the local hull shove the victim aside and keep its speed; the
/// predictor pinned the victim IMMOVABLE and MOTIONLESS, so it stopped the local hull dead. Every
/// snapshot then yanked the camera forward to catch up — a 20 Hz sawtooth lasting about a second,
/// which is what buzzing looks like. Free-running divergence peaked at 1.77 m at full throttle.
///
/// The fix is to predict contact by the authority's rules (movable neighbours with mass, moving at
/// the speed `TankMotion` derives from the snapshot pair) and keep only the local half of the
/// answer. Per-snapshot correction, T-54 into a parked flank:
///
/// | throttle | pinned + motionless | authority's rules |
/// |----------|--------------------:|------------------:|
/// | 0.35     |  0.110 m, 9 corrections | 0.078 m, 5 |
/// | 0.70     |  0.227 m, 16+ corrections | 0.123 m, 7 |
/// | 1.00     |  0.283 m, 16+ corrections | 0.207 m, 10 |
///
/// The peak matters less than the tail: the old corrections decayed so slowly they were still
/// 0.15 m sixteen snapshots later. These bounds hold today's numbers so the seam cannot reopen.
#[test]
fn ramming_a_live_hull_does_not_buzz_the_camera() {
    // Per-snapshot correction ceiling, and how many snapshots may carry a visible one.
    const WORST_CORRECTION_M: f32 = 0.24;
    const MAX_CORRECTIONS: usize = 12;

    for (throttle, ceiling) in [(0.35_f32, 0.10_f32), (0.7, 0.16), (1.0, WORST_CORRECTION_M)] {
        let flat = HeightMap::flat(200, 200, 4.0, 0.0).unwrap();
        let spec = TankSpec::t54_1951();
        let step = FixedTimestep::from_hz(60);
        let snapshot_ticks = 3;

        let mut server = SimulationState::new();
        let charger = server.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
        let victim = server.spawn_tank_with_yaw(
            TeamId(2),
            spec.clone(),
            Vec3::new(1.8, 0.0, 70.0),
            std::f32::consts::FRAC_PI_2,
        );
        let mut predictor = LocalPredictor::new(&spec);
        predictor.sync_to(&TankSnapshot::from(server.tank(charger).expect("charger")));

        let drive = TankCommand::drive(throttle, 0.0);
        let go = [(charger, drive), (victim, TankCommand::drive(0.0, 0.0))];
        // The neighbour exactly as `neighbours_for_prediction` builds it: refreshed per snapshot,
        // movable, and moving at the speed the snapshot pair implies.
        let mut pose = (Vec3::new(1.8, 0.0, 70.0), std::f32::consts::FRAC_PI_2);
        let mut velocity = Vec3::ZERO;
        let mut yaw_rate = 0.0_f32;
        let mut worst = 0.0_f32;
        let mut corrections = 0;
        for tick in 0..900 {
            if tick % snapshot_ticks == 0 {
                let seen = server.tank(victim).expect("victim");
                let window = snapshot_ticks as f32 * step.dt_seconds();
                velocity = (seen.position - pose.0) / window;
                yaw_rate = (seen.yaw_rad - pose.1) / window;
                pose = (seen.position, seen.yaw_rad);
            }
            let neighbour = ContactBody {
                id: 2,
                position: pose.0,
                velocity,
                yaw_rad: pose.1,
                yaw_rate_rad_s: yaw_rate,
                footprint: physics::TankFootprint::from_plan(spec.hull_plan()),
                mass_kg: spec.mass_kg,
                movable: true,
            };
            server.apply_commands_on_terrain(&go, step, &flat);
            predictor.step(drive, &flat, &[], &[neighbour], &[], None, step.dt_seconds());

            // What the client really does: reconcile onto authority every snapshot. The gap at
            // that instant is the distance the camera is yanked.
            if tick % snapshot_ticks == snapshot_ticks - 1 {
                let truth = server.tank(charger).expect("charger");
                let jump = (predictor.position() - truth.position).length();
                worst = worst.max(jump);
                if jump > 0.02 {
                    corrections += 1;
                }
                predictor.sync_to(&TankSnapshot::from(truth));
            }
        }
        assert!(
            worst <= ceiling,
            "throttle {throttle}: a flank ram now yanks the camera {worst:.3} m per snapshot, past \
             the {ceiling} m recorded. The predictor and the authority disagree about contact again"
        );
        assert!(
            corrections <= MAX_CORRECTIONS,
            "throttle {throttle}: {corrections} snapshots carry a visible correction, past the \
             {MAX_CORRECTIONS} recorded. A long tail of corrections is exactly what buzzes"
        );
    }
}
