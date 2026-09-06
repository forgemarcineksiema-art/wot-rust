//! Crew repair locks: mobility AND fighting power come back, but wounded. Before this system a
//! thrown track or a rammed-dead suspension made the hull a statue for the rest of the battle
//! (and parked entire bot teams mid-map); a knocked-out gun silently locked out firing for the
//! whole match. Now the crew field-patches engine, suspension, gun and ammo rack back to a
//! wounded state — the turret ring and radio still stay out.

use game_core::{ModuleSlot, TankSpec, TeamId, TrackSeverity, TrackSide};
use glam::Vec3;
use sim::{
    FixedTimestep, MODULE_PATCH_FRACTION, MODULE_PATCH_S, SimulationState, TRACK_REGEN_INTERVAL_S,
    TRACK_REPAIR_S, TankCommand,
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
    sim.tank_mut(id).unwrap().tracks.break_both();

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
fn destroyed_modules_are_field_patched_including_the_gun_and_rack_but_not_the_turret_ring() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    {
        let tank = sim.tank_mut(id).unwrap();
        for slot in [
            ModuleSlot::Suspension,
            ModuleSlot::Engine,
            ModuleSlot::Gun,
            ModuleSlot::AmmoRack,
            ModuleSlot::Turret,
        ] {
            tank.modules.damage(slot, u32::MAX);
        }
    }

    drive(&mut sim, id, &heightmap, ticks(MODULE_PATCH_S));
    let tank = sim.tank(id).unwrap();
    let spec = &tank.spec.module_health;
    // Mobility AND fighting modules come back — wounded, at the field-patch fraction, not full.
    for slot in [ModuleSlot::Engine, ModuleSlot::Suspension, ModuleSlot::Gun, ModuleSlot::AmmoRack]
    {
        let patched = tank.modules.hit_points(slot);
        assert!(patched > 0, "{slot:?} must be field-patched back to life");
        assert!(
            patched <= (spec.hit_points(slot) as f32 * MODULE_PATCH_FRACTION) as u32 + 1,
            "{slot:?} is a field patch, not shop condition (got {patched})"
        );
    }
    // The turret ring is deliberately left out — a jammed ring stays jammed.
    assert_eq!(tank.modules.hit_points(ModuleSlot::Turret), 0, "the turret ring stays knocked out");

    // And the patched hull actually drives.
    let before = sim.tank(id).unwrap().position;
    drive(&mut sim, id, &heightmap, 180);
    assert!(sim.tank(id).unwrap().position.distance(before) > 2.0, "patched mobility drives");
}

#[test]
fn a_field_patched_gun_fires_again_but_reloads_slower_than_a_whole_one() {
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));

    let whole_reload = sim.tank(id).unwrap().full_reload_seconds();
    // Drain the gun to roughly the crew-patch fraction — a wounded but functional breech.
    let full = sim.tank(id).unwrap().spec.module_health.hit_points(ModuleSlot::Gun);
    sim.tank_mut(id).unwrap().modules.damage(ModuleSlot::Gun, full * 3 / 4);

    let tank = sim.tank(id).unwrap();
    assert!(
        tank.modules.is_functional(ModuleSlot::Gun),
        "a wounded (not destroyed) gun must still be able to fire"
    );
    let wounded_reload = tank.full_reload_seconds();
    assert!(
        wounded_reload > whole_reload * 1.2,
        "a wounded gun reloads meaningfully slower ({wounded_reload:.2}s vs whole {whole_reload:.2}s)"
    );
}

#[test]
fn a_damaged_track_regenerates_back_to_full_over_time() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    // A glancing hit that degrades the pool without throwing the track (still rolls).
    sim.tank_mut(id).unwrap().tracks.damage(TrackSide::Left, 60);
    assert_eq!(
        sim.tank(id).unwrap().tracks.severity(TrackSide::Left),
        TrackSeverity::Damaged,
        "premise: the hit degraded but did not throw the track"
    );

    // Over enough regen intervals the crew nurses the pool all the way back to full.
    drive(&mut sim, id, &heightmap, ticks(TRACK_REGEN_INTERVAL_S * 4.0));
    assert_eq!(
        sim.tank(id).unwrap().tracks.severity(TrackSide::Left),
        TrackSeverity::Healthy,
        "a damaged pool regenerates to full on its own"
    );
}

#[test]
fn a_fresh_hit_during_the_repair_does_not_carry_the_old_clock() {
    let heightmap = flat_ground();
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    sim.tank_mut(id).unwrap().tracks.break_side(TrackSide::Left);

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
    sim.tank_mut(id).unwrap().tracks.break_side(TrackSide::Left);
    drive(&mut sim, id, &heightmap, ticks(TRACK_REPAIR_S * 0.5));
    assert!(
        sim.tank(id).unwrap().tracks.is_broken(TrackSide::Left),
        "a re-broken track starts its repair from zero"
    );
}
