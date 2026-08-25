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

/// The EXACT outcome of a deterministic replay, not a band. The sim carries no RNG — the honest-tank
/// doctrine is the whole point — so one shooter, one target, one command stream resolve to one
/// result every time. A floor/ceiling (`hit_points <= 1250`, `events >= 1`) let a real regression
/// hide inside its slack: a penetration that dealt 300 instead of 320, or one that stopped throwing
/// its second event, both still satisfied the band. These are pinned like a golden — a deliberate
/// balance change re-blesses them; drift fails them.
#[derive(Debug, Deserialize)]
struct ReplayExpected {
    target_hit_points_full: u32,
    target_hit_points: u32,
    damage_events: usize,
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
    assert_eq!(
        target.spec.hit_points, replay.expected.target_hit_points_full,
        "the target's full pool changed — the shot is measured against a different tank now"
    );
    assert_eq!(
        target.hit_points,
        replay.expected.target_hit_points,
        "the penetration's damage drifted from the pinned deterministic outcome \
         (dealt {}, fixture expects {})",
        replay.expected.target_hit_points_full - target.hit_points,
        replay.expected.target_hit_points_full - replay.expected.target_hit_points
    );
    assert_eq!(
        total_damage_events, replay.expected.damage_events,
        "the damage-event count drifted from the pinned deterministic outcome"
    );
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
