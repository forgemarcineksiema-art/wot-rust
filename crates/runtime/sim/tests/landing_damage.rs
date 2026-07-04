//! Locking tests for fall damage: a hard drop off a high face charges the hull and its
//! suspension through a self-inflicted `DamageCause::Impact` event; a small hop is free.

use game_core::{DamageCause, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

/// 61x61 @ 1 m plateau of the given height ending at z = 20, then flat ground.
fn drop_map(plateau_m: f32) -> HeightMap {
    let mut samples = Vec::with_capacity(61 * 61);
    for z in 0..61 {
        for x in 0..61 {
            let _ = x;
            samples.push(if (z as f32) < 20.0 { plateau_m } else { 0.0 });
        }
    }
    HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed")
}

/// Drive one tank off the plateau and return the first Impact event, if any tick emitted one.
fn drive_off(
    plateau_m: f32,
) -> (SimulationState, game_core::TankId, Option<game_core::DamageEvent>) {
    let map = drop_map(plateau_m);
    let mut state = SimulationState::new();
    let id =
        state.spawn_tank(TeamId(0), TankSpec::medium_test_tank(), Vec3::new(30.0, plateau_m, 8.0));
    let timestep = FixedTimestep::from_hz(60);
    for _ in 0..900 {
        state.apply_commands_on_terrain(&[(id, TankCommand::drive(1.0, 0.0))], timestep, &map);
        let impact =
            state.damage_events().iter().find(|event| event.cause == DamageCause::Impact).copied();
        if impact.is_some() {
            return (state, id, impact);
        }
    }
    (state, id, None)
}

#[test]
fn a_hard_drop_charges_the_hull_and_suspension_with_an_impact_event() {
    let (state, id, event) = drive_off(8.0);
    let event = event.expect("an 8 m drop must emit fall damage");
    assert_eq!(event.source, id, "fall damage is self-inflicted");
    assert_eq!(event.target, id);
    assert_eq!(event.module, Some(ModuleSlot::Suspension));
    assert!(event.damage_hp > 0);

    let tank = state.tank(id).expect("the tank survives the drop");
    let spec = TankSpec::medium_test_tank();
    assert!(tank.hit_points < spec.hit_points, "the hull pays hit points");
    assert!(
        tank.modules.hit_points(ModuleSlot::Suspension)
            < spec.module_health.hit_points(ModuleSlot::Suspension),
        "the suspension pays double"
    );
}

#[test]
fn a_small_hop_lands_free() {
    let (state, id, event) = drive_off(0.8);
    assert!(event.is_none(), "a 0.8 m drop is inside the safe landing envelope: {event:?}");
    let tank = state.tank(id).expect("tank");
    assert_eq!(tank.hit_points, TankSpec::medium_test_tank().hit_points);
}
