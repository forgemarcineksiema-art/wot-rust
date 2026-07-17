use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use serde::Deserialize;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[derive(Debug, Deserialize)]
struct CombatReplay {
    tick_rate_hz: u32,
    shooter: ReplayTank,
    target: ReplayTank,
    frames: Vec<TankCommand>,
    expected: ReplayExpected,
}

#[derive(Debug, Deserialize)]
struct ReplayTank {
    team: u16,
    spec: String,
    position: [f32; 3],
    yaw_rad: f32,
}

#[derive(Debug, Deserialize)]
struct ReplayExpected {
    target_hit_points_max: u32,
    damage_events_min: usize,
}

#[test]
fn fire_penetration_replay_is_a_regression_test() {
    let replay: CombatReplay =
        serde_json::from_str(include_str!("replays/fire_penetration_v1.json"))
            .expect("valid combat replay");

    let mut state = SimulationState::new();
    let shooter = spawn_replay_tank(&mut state, &replay.shooter);
    let target = spawn_replay_tank(&mut state, &replay.target);
    let step = FixedTimestep::from_hz(replay.tick_rate_hz);
    let mut total_damage_events = 0;

    for command in replay.frames {
        state.apply_commands(&[(shooter, command)], step);
        total_damage_events += state.damage_events().len();
    }

    let target = state.tank(target).expect("target tank");
    assert!(target.hit_points <= replay.expected.target_hit_points_max);
    assert!(total_damage_events >= replay.expected.damage_events_min);
}

fn spawn_replay_tank(state: &mut SimulationState, tank: &ReplayTank) -> TankId {
    let id = state.spawn_tank(
        TeamId(tank.team),
        spec_by_name(&tank.spec),
        Vec3::from_array(tank.position),
    );
    state.tank_mut(id).expect("spawned tank").yaw_rad = tank.yaw_rad;
    id
}

fn spec_by_name(name: &str) -> TankSpec {
    match name {
        "t54_1951" => TankSpec::t54_1951(),
        "tiger_ii" => TankSpec::tiger_ii_ausf_b(),
        other => panic!("unsupported replay tank spec: {other}"),
    }
}
