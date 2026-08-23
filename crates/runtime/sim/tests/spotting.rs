//! LOS spotting v1: masks, range, terrain and cover occlusion, wreck visibility, and the
//! fixed-cadence recompute hook in the sim tick.

use game_core::{TeamId, VehicleKind};
use glam::Vec3;
use sim::{
    FixedTimestep, SPOTTED_HOLD_TICKS, SPOTTING_INTERVAL_TICKS, SimulationState, VIEW_RANGE_M,
    line_of_sight,
};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

const TEAM_1_BIT: u8 = 1 << 0;
const TEAM_2_BIT: u8 = 1 << 1;

fn tree_line(center: [f32; 3], half: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject {
        id: "cover".into(),
        name: "tree line".into(),
        kind: StaticCoverKind::TreeLine,
        center,
        half_extents_m: half,
    }
}

/// Spawn two enemies, tick once (tick 0 recomputes), return their `(team1, team2)` masks.
fn duel_masks(separation_m: f32) -> (u8, u8) {
    let mut state = SimulationState::new();
    let a = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let b = state.spawn_tank(
        TeamId(2),
        VehicleKind::T54_1951.spec(),
        Vec3::new(0.0, 0.0, separation_m),
    );
    state.apply_commands(&[], FixedTimestep::from_hz(60));
    (state.tank(a).unwrap().spotted_mask, state.tank(b).unwrap().spotted_mask)
}

#[test]
fn enemies_in_the_open_spot_each_other() {
    let (mask_a, mask_b) = duel_masks(60.0);
    // Each carries its own team bit plus the enemy team that now sees it.
    assert_eq!(mask_a, TEAM_1_BIT | TEAM_2_BIT);
    assert_eq!(mask_b, TEAM_1_BIT | TEAM_2_BIT);
}

#[test]
fn refresh_spotting_seeds_masks_before_the_first_sim_tick() {
    let mut state = SimulationState::new();
    let a = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let b = state.spawn_tank(TeamId(2), VehicleKind::T54_1951.spec(), Vec3::new(0.0, 0.0, 60.0));

    state.refresh_spotting(None, &[]);

    assert_eq!(state.tank(a).unwrap().spotted_mask, TEAM_1_BIT | TEAM_2_BIT);
    assert_eq!(state.tank(b).unwrap().spotted_mask, TEAM_1_BIT | TEAM_2_BIT);
    assert_eq!(state.tick(), 0, "refreshing visibility must not advance simulation time");
}

#[test]
fn beyond_view_range_only_own_team_sees() {
    let (mask_a, mask_b) = duel_masks(VIEW_RANGE_M + 50.0);
    assert_eq!(mask_a, TEAM_1_BIT);
    assert_eq!(mask_b, TEAM_2_BIT);
}

#[test]
fn cover_between_observers_blocks_the_sight_line() {
    let eye = Vec3::new(0.0, 2.0, 0.0);
    let target = Vec3::new(0.0, 1.0, 60.0);
    let wall = [tree_line([0.0, 1.5, 30.0], [6.0, 3.0, 2.0])];

    assert!(line_of_sight(None, &[], eye, target), "open field is a clear line");
    assert!(!line_of_sight(None, &wall, eye, target), "tree line on the line blocks it");
    // A tree line off to the side leaves the line clear.
    let aside = [tree_line([40.0, 1.5, 30.0], [6.0, 3.0, 2.0])];
    assert!(line_of_sight(None, &aside, eye, target));
}

#[test]
fn a_ridge_between_observers_blocks_the_sight_line() {
    // 100 m x 100 m grid, flat except a tall band at z == 30 m that rises above any sight line.
    let (w, cell) = (11usize, 10.0f32);
    let mut samples = vec![0.0f32; w * w];
    for x in 0..w {
        samples[3 * w + x] = 30.0;
    }
    let heightmap = HeightMap::new(w, w, cell, samples).unwrap();
    let eye = Vec3::new(50.0, 2.0, 10.0);
    let target = Vec3::new(50.0, 1.0, 90.0);

    assert!(!line_of_sight(Some(&heightmap), &[], eye, target), "ridge occludes");
    // Flat ground at the same spots is clear.
    let flat = HeightMap::flat(w, w, cell, 0.0).unwrap();
    assert!(line_of_sight(Some(&flat), &[], eye, target));
}

/// The 10 Hz boolean LOS recompute strobes a target dancing on a ridge or a bush corner — model
/// pop, minimap blink, and the shooter's ballistic aim point flipping between hull and terrain.
/// Locked here: after the fresh line breaks, the spot HOLDS for `SPOTTED_HOLD_TICKS`, then drops
/// on the next recompute past expiry — one clean spot-then-fade cycle instead of a strobe.
#[test]
fn a_spotted_target_holds_for_the_grace_window_after_los_breaks() {
    let timestep = FixedTimestep::from_hz(60);
    let flat = HeightMap::flat(32, 32, 10.0, 0.0).unwrap();
    let mut state = SimulationState::new();
    let _observer = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let target =
        state.spawn_tank(TeamId(2), VehicleKind::T54_1951.spec(), Vec3::new(0.0, 0.0, 60.0));

    // Tick 0 recompute in the open: freshly spotted.
    state.apply_commands_on_battlefield(&[], timestep, &flat, &[]);
    assert_ne!(state.tank(target).unwrap().spotted_mask & TEAM_1_BIT, 0, "fresh spot");

    // A wall now blocks the line. Within the hold window the target must stay lit...
    let wall = [tree_line([0.0, 5.0, 30.0], [40.0, 10.0, 2.0])];
    for _ in 0..SPOTTED_HOLD_TICKS {
        state.apply_commands_on_battlefield(&[], timestep, &flat, &wall);
    }
    assert_ne!(
        state.tank(target).unwrap().spotted_mask & TEAM_1_BIT,
        0,
        "the hold keeps the target lit after LOS breaks"
    );

    // ...and the first recompute past expiry drops it.
    for _ in 0..2 * SPOTTING_INTERVAL_TICKS {
        state.apply_commands_on_battlefield(&[], timestep, &flat, &wall);
    }
    assert_eq!(
        state.tank(target).unwrap().spotted_mask & TEAM_1_BIT,
        0,
        "the hold expires instead of lasting forever"
    );

    // Re-acquiring the line re-lights the target immediately on the next recompute.
    for _ in 0..SPOTTING_INTERVAL_TICKS {
        state.apply_commands_on_battlefield(&[], timestep, &flat, &[]);
    }
    assert_ne!(state.tank(target).unwrap().spotted_mask & TEAM_1_BIT, 0, "re-spot works");
}

#[test]
fn a_wreck_is_visible_to_every_team() {
    let mut state = SimulationState::new();
    let a = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    // Far enough that a living tank would not be spotted, so the wreck rule is what shows through.
    let b = state.spawn_tank(
        TeamId(2),
        VehicleKind::T54_1951.spec(),
        Vec3::new(0.0, 0.0, VIEW_RANGE_M + 100.0),
    );
    if let Some(wreck) = state.tank_mut(b) {
        wreck.hit_points = 0;
    }
    state.apply_commands(&[], FixedTimestep::from_hz(60));

    assert_eq!(state.tank(b).unwrap().spotted_mask, u8::MAX, "a wreck is public");
    // The living observer is still only seen by its own team.
    assert_eq!(state.tank(a).unwrap().spotted_mask, TEAM_1_BIT);
}

/// Optics live on the observer's spec: the T-54 (440 m) sees a hull the T-34-85 (400 m) cannot.
#[test]
fn view_range_follows_the_observers_vehicle() {
    let t54 = VehicleKind::T54_1951.spec();
    let t34 = VehicleKind::T34_85.spec();
    assert_eq!(t54.view_range_m(), 440.0);
    assert_eq!(t34.view_range_m(), 400.0);

    // A duel at 420 m: the T-54 sees the T-34-85; the T-34-85 optics fall short.
    let mut state = SimulationState::new();
    let t54_id = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let t34_id =
        state.spawn_tank(TeamId(2), VehicleKind::T34_85.spec(), Vec3::new(0.0, 0.0, 420.0));
    // Both hulls have JUST FIRED: concealment is off, so this test measures optics alone.
    state.tank_mut(t54_id).unwrap().last_shot_tick = Some(0);
    state.tank_mut(t34_id).unwrap().last_shot_tick = Some(0);
    state.apply_commands(&[], FixedTimestep::from_hz(60));
    assert_eq!(
        state.tank(t34_id).unwrap().spotted_mask,
        TEAM_1_BIT | TEAM_2_BIT,
        "the 440 m T-54 optics light the T-34-85 at 420 m"
    );
    assert_eq!(
        state.tank(t54_id).unwrap().spotted_mask,
        TEAM_1_BIT,
        "the 400 m T-34-85 optics fall short at 420 m"
    );
}

/// v29: a destroyed radio drops the crew out of team-share — its sightings no longer light the
/// team mask. Its OWN eyes stay honest through the observer mask, so nothing in plain sight can
/// vanish off its screen once the per-viewer filter unions both.
#[test]
fn a_dead_radio_stops_sharing_but_never_blinds_its_own_eyes() {
    let mut state = SimulationState::new();
    let scout = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let _enemy =
        state.spawn_tank(TeamId(2), VehicleKind::T54_1951.spec(), Vec3::new(0.0, 0.0, 80.0));
    // Kill the scout's radio outright.
    let radio_full = {
        let tank = state.tank(scout).unwrap();
        tank.spec.module_health.hit_points_by_slot()
            [game_core::ModuleSlot::Radio.destroyed_mask_bit().trailing_zeros() as usize]
    };
    state.tank_mut(scout).unwrap().modules.damage(game_core::ModuleSlot::Radio, radio_full);
    state.apply_commands(&[], FixedTimestep::from_hz(60));

    let tanks: Vec<_> = state.tanks().to_vec();
    let team_masks = sim::compute_spotted_masks(&tanks, 0, None, &[]);
    let observer_masks = sim::compute_observer_masks(&tanks, 0, None, &[]);
    // The enemy is index 1; the scout is index 0.
    assert_eq!(
        team_masks[1] & TEAM_1_BIT,
        0,
        "a radio-dead scout must not light targets for its team"
    );
    assert_ne!(
        observer_masks[1] & 1,
        0,
        "the scout's own eyes still see the enemy in front of them"
    );
}

/// THE SPOTTING DECISION (2.6): a stationary tank is seen from 70 % of range.
///
/// One rule, binary and readable — sitting still IS the concealment, with no per-vehicle
/// camouflage percentage to memorise. At 350 m against 440 m optics: a parked hull sits outside
/// the 308 m concealed radius and stays dark; the same hull rolling is lit.
#[test]
fn a_stationary_hull_is_harder_to_find_and_a_rolling_one_is_not() {
    let mut state = SimulationState::new();
    let _observer = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let enemy =
        state.spawn_tank(TeamId(2), VehicleKind::T54_1951.spec(), Vec3::new(0.0, 0.0, 350.0));
    state.apply_commands(&[], FixedTimestep::from_hz(60));
    assert_eq!(
        state.tank(enemy).unwrap().spotted_mask,
        TEAM_2_BIT,
        "a parked, silent hull at 350 m hides inside the 0.7 factor of 440 m optics"
    );

    // The same hull with real planar speed is spotted on the next recompute.
    state.tank_mut(enemy).unwrap().velocity_mps = Vec3::new(0.0, 0.0, -3.0);
    for _ in 0..sim::SPOTTING_INTERVAL_TICKS {
        state.apply_commands(&[], FixedTimestep::from_hz(60));
    }
    assert_eq!(
        state.tank(enemy).unwrap().spotted_mask,
        TEAM_1_BIT | TEAM_2_BIT,
        "a rolling hull at the same range is seen — motion is the whole of the rule"
    );
}

/// THE SPOTTING DECISION (2.6), second rule: firing makes you fully visible for 8 seconds.
///
/// The muzzle flash is the loudest, brightest thing on a battlefield, and the sim records the
/// shot as a fact — so the reveal costs one field read, not a new system.
#[test]
fn firing_reveals_a_parked_hull_and_the_reveal_expires() {
    let mut state = SimulationState::new();
    let _observer = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let enemy =
        state.spawn_tank(TeamId(2), VehicleKind::T54_1951.spec(), Vec3::new(0.0, 0.0, 350.0));
    state.apply_commands(&[], FixedTimestep::from_hz(60));
    assert_eq!(state.tank(enemy).unwrap().spotted_mask, TEAM_2_BIT, "parked and silent: dark");

    // The shot lights it on the next recompute.
    state.tank_mut(enemy).unwrap().last_shot_tick = Some(1);
    for _ in 0..sim::SPOTTING_INTERVAL_TICKS {
        state.apply_commands(&[], FixedTimestep::from_hz(60));
    }
    assert_eq!(
        state.tank(enemy).unwrap().spotted_mask,
        TEAM_1_BIT | TEAM_2_BIT,
        "the gun that just fired is lit"
    );

    // Reveal over, hold over: the parked hull goes dark again.
    for _ in 0..(sim::FIRE_REVEAL_TICKS + sim::SPOTTED_HOLD_TICKS + sim::SPOTTING_INTERVAL_TICKS) {
        state.apply_commands(&[], FixedTimestep::from_hz(60));
    }
    assert_eq!(
        state.tank(enemy).unwrap().spotted_mask,
        TEAM_2_BIT,
        "eight seconds of reveal plus the hold, and the silent hull disappears again"
    );
}

/// The reveal input is written where the shell is born — firing through the real command path
/// stamps `last_shot_tick`, so the rule can never be forgotten at a second wiring site.
#[test]
fn a_real_shot_stamps_the_reveal_clock() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    assert!(state.tank(shooter).unwrap().last_shot_tick.is_none());
    state.apply_commands(
        &[(shooter, sim::TankCommand { fire: true, ..sim::TankCommand::idle() })],
        FixedTimestep::from_hz(60),
    );
    assert!(state.tank(shooter).unwrap().last_shot_tick.is_some(), "the shot stamped the clock");
}
