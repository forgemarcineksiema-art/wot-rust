//! LOS spotting v1: masks, range, terrain and cover occlusion, wreck visibility, and the
//! fixed-cadence recompute hook in the sim tick.

use game_core::{TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, VIEW_RANGE_M, line_of_sight};
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
