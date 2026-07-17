use game_core::{MountFrames, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

#[test]
fn shell_falls_under_gravity_after_firing() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(50.0, 0.0, 50.0));
    let terrain = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let step = FixedTimestep::from_hz(60);

    state.apply_commands_on_terrain(
        &[(id, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );
    let launch = state.shells().first().expect("a shell was fired").velocity_mps.y;

    for _ in 0..10 {
        state.apply_commands_on_terrain(&[(id, TankCommand::idle())], step, &terrain);
    }
    let shell = state.shells().first().expect("shell still in flight");
    assert!(shell.velocity_mps.y < launch, "gravity must reduce vertical velocity");
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
    let elevation = state.tank(id).expect("tank").gun_pitch_rad;
    assert!(
        (0.30..=0.40).contains(&elevation),
        "elevation should saturate near max, got {elevation}"
    );

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() };
        state.apply_commands(&[(id, command)], step);
    }
    let depression = state.tank(id).expect("tank").gun_pitch_rad;
    assert!((-0.16..=-0.10).contains(&depression), "depression should saturate, got {depression}");
}
