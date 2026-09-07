use game_core::{MountFrames, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

/// S14 (the one program, block 5): the flight is a NUMBER, not "y went down". The code's
/// linear drag stands over the GDD's "no air drag" (GDD reconciliation row 31, 2026-09-07):
/// `c = 0.0130 / sectional density` — the D-10T's BR-412 (15.7 kg over a 100 mm bore) sheds
/// 0.0828 m/s per metre — and the arc is the shared `integrate_shell_step` at the tick rate.
/// A T-54 firing flat from the ground: at 1000 m the shell has flown 1.173 s, dropped 6.64 m
/// under its departure line (semi-implicit Euler at 60 Hz: the closed-form 6.54 m plus half a
/// tick of gravity per tick), and arrives at 812 m/s — within 2 m/s of the closed form the
/// reticle and the armour math read (`speed_mps_at_distance`). One physics for the arc you
/// see and the number the HUD promises.
#[test]
fn the_br412_flies_one_thousand_metres_by_the_numbers() {
    let mut state = SimulationState::new();
    // A firing shelf: the hull on a 0 m plateau up to z = 130 m, the range beyond it 20 m
    // lower, so a 6.6 m drop meets no ground and no Z7 skip; the map long enough to hold the
    // whole kilometre (320 cells × 4 m).
    let width = 320;
    let samples = (0..width * width)
        .map(|index| if ((index / width) as f32) * 4.0 < 130.0 { 0.0 } else { -20.0 })
        .collect();
    let terrain = HeightMap::new(width, width, 4.0, samples).expect("a shelf over a low range");
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    let step = FixedTimestep::from_hz(60);
    let dt = 1.0 / 60.0;

    state.apply_commands_on_terrain(
        &[(id, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );
    let shell = *state.shells().first().expect("a shell was fired");
    assert_eq!(shell.shell.round, Some(game_core::RoundId::Br412), "the D-10T's stock round");
    let spec = shell.shell;
    // The departure line: the fire tick already integrated one step (drag, then gravity, then
    // the move), so undo it to read the muzzle velocity and the point it left from. The gun
    // is level to the milliradian (the hull's settle on the ground is the only tilt).
    let departure = shell.position - shell.velocity_mps * dt;
    let muzzle_velocity = (shell.velocity_mps + Vec3::Y * game_core::math::SHELL_GRAVITY_MPS2 * dt)
        / (1.0 - spec.drag_per_s() * dt);
    let line = muzzle_velocity.normalize();
    assert!(line.y.abs() < 2.0e-3, "the gun is level at spawn, got a {} rad line", line.y);
    assert!(
        (muzzle_velocity.length() - spec.muzzle_velocity_mps).abs() < 0.1,
        "the round leaves at its muzzle velocity: {}",
        muzzle_velocity.length()
    );

    // Walk the flight tick by tick and interpolate the kilometre crossing.
    let mut previous = (shell.age_seconds, shell.position, shell.velocity_mps);
    let crossing = loop {
        state.apply_commands_on_terrain(&[(id, TankCommand::idle())], step, &terrain);
        let shell = state.shells().first().expect("the shell flies the whole kilometre");
        let range = |p: Vec3| ((p.x - departure.x).powi(2) + (p.z - departure.z).powi(2)).sqrt();
        let current = (shell.age_seconds, shell.position, shell.velocity_mps);
        if range(current.1) >= 1000.0 {
            let a = (1000.0 - range(previous.1)) / (range(current.1) - range(previous.1));
            break (
                previous.0 + a * (current.0 - previous.0),
                previous.1 + (current.1 - previous.1) * a,
                previous.2 + (current.2 - previous.2) * a,
            );
        }
        previous = current;
    };
    let (time_of_flight, position, velocity) = crossing;
    // The drop is measured under the departure LINE at the kilometre.
    let line_y_at_range =
        departure.y + line.y / (line.x * line.x + line.z * line.z).sqrt() * 1000.0;
    let drop = line_y_at_range - position.y;
    assert!((time_of_flight - 1.173).abs() < 0.01, "time of flight {time_of_flight} s");
    assert!((drop - 6.64).abs() < 0.1, "drop {drop} m under the departure line");
    let integrated = velocity.length();
    let closed = spec.speed_mps_at_distance(1000.0);
    assert!(
        (integrated - closed).abs() < 2.0,
        "the flown speed {integrated} m/s vs the closed form {closed} m/s"
    );
    assert!((closed - 812.2).abs() < 1.0, "the closed form itself: {closed}");
}

/// The shell must leave the *visible* muzzle: pitched about the trunnion, not swung about the
/// hull centre. The expected position derives the pivot chain with explicit trigonometry, and the
/// old hull-centre approximation is asserted to be measurably wrong at this elevation so the test
/// cannot silently accept it again.
#[test]
fn shell_spawns_at_the_visible_muzzle_when_elevated() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(50.0, 0.0, 50.0));
    let terrain = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: 1.0, ..TankCommand::idle() };
        state.apply_commands_on_terrain(&[(id, command)], step, &terrain);
    }
    state.apply_commands_on_terrain(
        &[(id, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );

    let tank = state.tank(id).expect("tank");
    let pitch = tank.gun_pitch_rad;
    assert!(pitch > 0.25, "gun should be elevated for this test, got {pitch}");
    let shell = state.shells().first().expect("a shell was fired");

    // The fire tick already integrated the shell one step (semi-implicit Euler: gravity, then
    // move), so the spawn point is exactly `position - velocity * dt`.
    let dt = 1.0 / 60.0;
    assert!((shell.age_seconds - dt).abs() < 1.0e-6, "shell should be one tick old");
    let spawn = shell.position - shell.velocity_mps * dt;

    let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
    let ring = mounts.turret_ring.translation;
    let trunnion = mounts.gun_trunnion.translation;
    let barrel = mounts.muzzle.translation.z - trunnion.z;
    let pitched =
        Vec3::new(0.0, trunnion.y + barrel * pitch.sin(), trunnion.z + barrel * pitch.cos());
    let yaw_about = |point: Vec3, pivot: Vec3, yaw: f32| {
        let rel = point - pivot;
        pivot
            + Vec3::new(
                rel.x * yaw.cos() + rel.z * yaw.sin(),
                rel.y,
                rel.z * yaw.cos() - rel.x * yaw.sin(),
            )
    };
    let traversed = yaw_about(pitched, ring, tank.turret_yaw_rad);
    let expected = tank.position + yaw_about(traversed, Vec3::ZERO, tank.yaw_rad);

    assert!(
        (spawn - expected).length() < 5.0e-3,
        "shell spawned at {spawn:?}, visible muzzle is at {expected:?}"
    );

    // The retired hull-centre approximation drifts by sin(pitch)·trunnion.z — assert the spawn no
    // longer matches it.
    let muzzle_mount = mounts.muzzle.translation;
    let old_direction = game_core::math::gun_direction(tank.yaw_rad + tank.turret_yaw_rad, pitch);
    let old = tank.position + Vec3::Y * muzzle_mount.y + old_direction * muzzle_mount.z;
    assert!((spawn - old).length() > 0.05, "spawn still matches the old hull-centre pivot");
}

#[test]
fn gun_elevation_clamps_to_its_arc() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: 1.0, ..TankCommand::idle() };
        state.apply_commands(&[(id, command)], step);
    }
    // The arc is per-vehicle now: assert against the tank's OWN limits rather than the
    // fleet-wide pair this used to hard-code.
    let (min_pitch, max_pitch) = state.tank(id).expect("tank").spec.gun_pitch_limits_rad();
    let elevation = state.tank(id).expect("tank").gun_pitch_rad;
    assert!(
        (elevation - max_pitch).abs() < 0.01,
        "elevation should saturate at this gun's own maximum {max_pitch}, got {elevation}"
    );

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() };
        state.apply_commands(&[(id, command)], step);
    }
    let depression = state.tank(id).expect("tank").gun_pitch_rad;
    assert!(
        (depression - min_pitch).abs() < 0.01,
        "depression should saturate at this gun's own minimum {min_pitch}, got {depression}"
    );
}
