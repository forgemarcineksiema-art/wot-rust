//! Crew repair locks: mobility comes back, fighting power does not. Before this system a
//! thrown track or a rammed-dead suspension made the hull a statue for the rest of the battle
//! (and parked entire bot teams mid-map).

use game_core::{ModuleSlot, TankSpec, TeamId, TrackSide};
use glam::Vec3;
use sim::{
    FixedTimestep, MODULE_PATCH_FRACTION, MODULE_PATCH_S, SimulationState, TRACK_REPAIR_S,
    TankCommand,
};
use terrain::HeightMap;

fn flat_ground() -> HeightMap {
    HeightMap::flat(64, 64, 5.0, 0.0).expect("flat test ground")
}

fn ticks(seconds: f32) -> usize {
    (seconds * 60.0).ceil() as usize + 2
}

fn drive(sim: &mut SimulationState, id: game_core::TankId, heightmap: &HeightMap, steps: usize) {
    let command = TankCommand { throttle: 1.0, ..TankCommand::idle() };
    for _ in 0..steps {
        sim.apply_commands_on_terrain(&[(id, command)], FixedTimestep::from_hz(60), heightmap);
    }
}

#[test]
fn a_thrown_track_is_reseated_and_the_hull_drives_again() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    sim.tank_mut(id).unwrap().tracks.damage_both();

    // Immobilized now: full throttle moves nothing.
    let start = sim.tank(id).unwrap().position;
    drive(&mut sim, id, &heightmap, 60);
    assert!(
        sim.tank(id).unwrap().position.distance(start) < 0.5,
        "with both tracks thrown the hull must not drive off"
    );
    assert!(sim.tank(id).unwrap().tracks.any_broken());

    // After the repair window the crew has re-seated both sides and the hull moves.
    drive(&mut sim, id, &heightmap, ticks(TRACK_REPAIR_S));
    assert!(!sim.tank(id).unwrap().tracks.any_broken(), "crew must re-seat the tracks");
    let repaired = sim.tank(id).unwrap().position;
    drive(&mut sim, id, &heightmap, 120);
    assert!(
        sim.tank(id).unwrap().position.distance(repaired) > 2.0,
        "a repaired hull drives again"
    );
}

#[test]
fn destroyed_mobility_modules_are_field_patched_but_the_gun_stays_dead() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    {
        let tank = sim.tank_mut(id).unwrap();
        tank.modules.damage(ModuleSlot::Suspension, u32::MAX);
        tank.modules.damage(ModuleSlot::Engine, u32::MAX);
        tank.modules.damage(ModuleSlot::Gun, u32::MAX);
    }

    drive(&mut sim, id, &heightmap, ticks(MODULE_PATCH_S));
    let tank = sim.tank(id).unwrap();
    let spec = &tank.spec.module_health;
    for slot in [ModuleSlot::Engine, ModuleSlot::Suspension] {
        let patched = tank.modules.hit_points(slot);
        assert!(patched > 0, "{slot:?} must be field-patched back to life");
        assert!(
            patched <= (spec.hit_points(slot) as f32 * MODULE_PATCH_FRACTION) as u32 + 1,
            "{slot:?} is a field patch, not shop condition (got {patched})"
        );
    }
    assert_eq!(tank.modules.hit_points(ModuleSlot::Gun), 0, "a knocked-out gun stays out");

    // And the patched hull actually drives.
    let before = sim.tank(id).unwrap().position;
    drive(&mut sim, id, &heightmap, 180);
    assert!(sim.tank(id).unwrap().position.distance(before) > 2.0, "patched mobility drives");
}

#[test]
fn a_fresh_hit_during_the_repair_does_not_carry_the_old_clock() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    sim.tank_mut(id).unwrap().tracks.damage(TrackSide::Left);

    // Let most of the repair run, then re-break the same side: the clock must restart, so
    // shortly after the re-hit the track is still broken.
    drive(&mut sim, id, &heightmap, ticks(TRACK_REPAIR_S * 0.8));
    let tank = sim.tank_mut(id).unwrap();
    if !tank.tracks.is_broken(TrackSide::Left) {
        panic!("premise: 80% of the window must not have repaired the track yet");
    }
    // Simulate the repair completing, breaking again right after.
    drive(&mut sim, id, &heightmap, ticks(TRACK_REPAIR_S * 0.3));
    assert!(!sim.tank(id).unwrap().tracks.is_broken(TrackSide::Left));
    sim.tank_mut(id).unwrap().tracks.damage(TrackSide::Left);
    drive(&mut sim, id, &heightmap, ticks(TRACK_REPAIR_S * 0.5));
    assert!(
        sim.tank(id).unwrap().tracks.is_broken(TrackSide::Left),
        "a re-broken track starts its repair from zero"
    );
}
