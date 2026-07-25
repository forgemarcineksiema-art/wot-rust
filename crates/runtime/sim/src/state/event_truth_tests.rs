use game_core::{BattleEventId, ShellId, ShellSpec, TankSpec, TeamId};
use glam::Vec3;
use terrain::HeightMap;

use super::SimulationState;
use crate::{FixedTimestep, ShellState};

fn shell(
    id: ShellId,
    owner: game_core::TankId,
    position: Vec3,
    velocity_mps: Vec3,
    shell: ShellSpec,
) -> ShellState {
    ShellState {
        id,
        owner,
        position,
        velocity_mps,
        shell,
        age_seconds: 0.0,
        traveled_m: 0.0,
        max_age_seconds: 5.0,
        ricocheted_once: false,
        last_penetrated_target: None,
    }
}

#[test]
fn lethal_attribution_and_event_ids_survive_snapshot_windows() {
    let mut state = SimulationState::new();
    let first_attacker =
        state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(50.0, 0.0, 0.0));
    let second_attacker =
        state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(-50.0, 0.0, 0.0));
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::ZERO);
    state.tank_mut(target).expect("target").hit_points = 150;

    let shot = ShellSpec::heat(100.0, 600.0, 1_000.0, 100);
    let first_shell_id = ShellId(101);
    let lethal_shell_id = ShellId(202);
    state.shells.extend([
        shell(
            first_shell_id,
            first_attacker,
            Vec3::new(0.0, 0.9, -5.0),
            Vec3::new(0.0, 0.0, 600.0),
            shot,
        ),
        shell(
            lethal_shell_id,
            second_attacker,
            Vec3::new(0.0, 0.9, -5.0),
            Vec3::new(0.0, 0.0, 600.0),
            shot,
        ),
    ]);

    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[], step);

    let events = state.damage_events();
    assert_eq!(events.len(), 2, "both same-tick attackers must own a damage event");
    assert_eq!(events[0].source, first_attacker);
    assert_eq!(events[0].shell_id, Some(first_shell_id));
    assert!(!events[0].target_destroyed, "the wounding shot must not inherit the later kill");
    assert_eq!(events[1].source, second_attacker);
    assert_eq!(events[1].shell_id, Some(lethal_shell_id));
    assert!(events[1].target_destroyed, "only the alive-to-dead transition owns the kill");
    assert_eq!(
        events.iter().map(|event| event.event_id).collect::<Vec<_>>(),
        [BattleEventId(1), BattleEventId(2)]
    );
    assert!(events.iter().all(|event| event.occurred_tick == 0));

    let impact_shell_id = ShellId(303);
    state.shells.push(shell(
        impact_shell_id,
        first_attacker,
        Vec3::new(20.0, 0.1, 20.0),
        Vec3::new(0.0, -30.0, 0.0),
        ShellSpec::armor_piercing(100.0, 600.0, 200.0, 100),
    ));
    let ground = HeightMap::flat(64, 64, 1.0, 0.0).expect("flat test ground");
    state.apply_commands_on_terrain(&[], step, &ground);

    let impact = state.shell_impacts().first().copied().expect("ground impact");
    assert_eq!(impact.event_id, BattleEventId(3));
    assert_eq!(impact.occurred_tick, 1);
    assert_eq!(impact.shell_id, impact_shell_id);

    let encoded = serde_json::to_string(&state).expect("serialize simulation");
    let mut restored: SimulationState = serde_json::from_str(&encoded).expect("restore simulation");
    assert_eq!(
        restored.shell_impacts().first().map(|event| event.event_id),
        Some(BattleEventId(3)),
        "an event keeps its identity while snapshots repeat or state is restored"
    );

    let next_shell_id = ShellId(404);
    restored.shells.push(shell(
        next_shell_id,
        first_attacker,
        Vec3::new(22.0, 0.1, 22.0),
        Vec3::new(0.0, -30.0, 0.0),
        ShellSpec::armor_piercing(100.0, 600.0, 200.0, 100),
    ));
    restored.apply_commands_on_terrain(&[], step, &ground);
    let next = restored.shell_impacts().first().expect("next ground impact");
    assert_eq!(next.event_id, BattleEventId(4));
    assert_eq!(next.occurred_tick, 2);
    assert_eq!(next.shell_id, next_shell_id);
}
