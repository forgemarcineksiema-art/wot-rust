use engine::{PresentationWorld, TankMotion};
use game_core::{TankId, TeamId, TrackDamageMask, VehicleKind};
use net::TankSnapshot;

/// A tank synced with zero tick-domain motion — the track/attitude cues that read pose deltas
/// (not motion) are exercised through position changes alone.
fn still(snapshot: TankSnapshot) -> (TankSnapshot, TankMotion) {
    (snapshot, TankMotion::default())
}

fn snapshot(id: u64, position: [f32; 3], hit_points: u32) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(id),
        team: TeamId(1),
        vehicle: VehicleKind::T54_1951,
        position,
        yaw_rad: 0.1,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.2,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.3,
        hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 2.9,
        module_hit_points: VehicleKind::T54_1951.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
    }
}

#[test]
fn sync_spawns_an_entity_per_tank_and_extracts_its_pose() {
    let mut world = PresentationWorld::default();

    world.sync_tanks(&[
        still(snapshot(1, [1.0, 0.0, 2.0], 900)),
        still(snapshot(2, [3.0, 0.0, 4.0], 500)),
    ]);

    assert_eq!(world.tank_count(), 2);
    let tanks = world.presentation_tanks();
    assert_eq!(tanks.len(), 2);
    // Ordered by id for deterministic rendering.
    assert_eq!(tanks[0].id, TankId(1));
    assert_eq!(tanks[0].translation, [1.0, 0.0, 2.0]);
    assert_eq!(tanks[0].hit_points, 900);
    assert_eq!(tanks[0].turret_yaw_rad, 0.2);
    assert_eq!(tanks[1].id, TankId(2));
    assert_eq!(tanks[1].hit_points, 500);
}

/// History this test exists to not repeat: `sync_tanks` carried `fuel_fire` into the component
/// correctly, and the projection back out hardcoded `false` on the line directly below the correct
/// `engine_fire` mapping. The sim set the flag on a holed tank (`combat.rs`, a `FuelTank` component
/// struck), the wire replicated it, the component held it — and it died one line before the two
/// consumers that were already waiting for it: the burning-audio voice and the flame/smoke column
/// (`app/audio_link.rs`, `app/battle_scars.rs`, both reading `engine_fire || fuel_fire`).
///
/// A whole replicated damage state was invisible in game because of one literal. Assert BOTH flags
/// so the next one cannot be dropped quietly either.
#[test]
fn both_fire_flags_survive_the_projection_out_of_the_presentation_world() {
    let mut world = PresentationWorld::default();

    let mut engine_burning = snapshot(1, [0.0, 0.0, 0.0], 900);
    engine_burning.engine_fire = true;
    let mut fuel_burning = snapshot(2, [5.0, 0.0, 0.0], 900);
    fuel_burning.fuel_fire = true;
    let mut both_burning = snapshot(3, [10.0, 0.0, 0.0], 900);
    both_burning.engine_fire = true;
    both_burning.fuel_fire = true;

    world.sync_tanks(&[still(engine_burning), still(fuel_burning), still(both_burning)]);

    let tanks = world.presentation_tanks();
    assert!(tanks[0].engine_fire, "an engine fire must reach the presentation world");
    assert!(!tanks[0].fuel_fire, "an engine fire is not a fuel fire");
    assert!(tanks[1].fuel_fire, "a holed fuel tank must reach the presentation world");
    assert!(!tanks[1].engine_fire, "a fuel fire is not an engine fire");
    assert!(tanks[2].engine_fire && tanks[2].fuel_fire, "a tank can burn on both counts at once");
}

#[test]
fn re_syncing_a_tank_updates_in_place_without_respawning() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
    let first = world.presentation_tanks();

    world.sync_tanks(&[still(snapshot(1, [10.0, 0.0, 0.0], 300))]);
    let second = world.presentation_tanks();

    // Same single entity, moved and damaged — not a duplicate.
    assert_eq!(world.tank_count(), 1);
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].translation, [10.0, 0.0, 0.0]);
    assert_eq!(second[0].hit_points, 300);
}

#[test]
fn a_tank_absent_from_the_next_sync_is_despawned() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(snapshot(1, [0.0; 3], 900)), still(snapshot(2, [0.0; 3], 900))]);
    assert_eq!(world.tank_count(), 2);

    world.sync_tanks(&[still(snapshot(1, [0.0; 3], 900))]);

    assert_eq!(world.tank_count(), 1);
    let tanks = world.presentation_tanks();
    assert_eq!(tanks.len(), 1);
    assert_eq!(tanks[0].id, TankId(1));
}

fn posed(position: [f32; 3], yaw_rad: f32) -> TankSnapshot {
    TankSnapshot { yaw_rad, ..snapshot(1, position, 900) }
}

/// Where the belts actually are: the contact footprint's centre line, from the blueprint the
/// running gear is placed by.
///
/// This helper used to read the hitbox half-width — the same wrong number the code under test was
/// reading, so the assertion below compared a wrong gauge against a wrong gauge and passed. On a
/// T-54 that is 1.75 m against a real 1.32 m centre line: a third too wide, and it made the inner
/// and outer belts disagree by a third too much through every turn. An instrument calibrated
/// against the thing it is meant to judge cannot report the judgement.
fn half_gauge() -> f32 {
    game_core::ContactFootprint::for_vehicle(VehicleKind::T54_1951).half_gauge_x
}

#[test]
fn driving_forward_advances_both_tracks_equally() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(posed([0.0, 0.0, 0.0], 0.0))]); // seed
    world.sync_tanks(&[still(posed([0.0, 0.0, 5.0], 0.0))]); // +Z is forward at yaw 0

    let tank = world.presentation_tanks().remove(0);
    assert!((tank.track_left_m - 5.0).abs() < 1.0e-4, "left {}", tank.track_left_m);
    assert!((tank.track_right_m - 5.0).abs() < 1.0e-4, "right {}", tank.track_right_m);
}

#[test]
fn pivoting_in_place_runs_the_tracks_in_opposite_directions() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(posed([0.0, 0.0, 0.0], 0.0))]); // seed
    world.sync_tanks(&[still(posed([0.0, 0.0, 0.0], 0.5))]); // rotate, no translation

    let tank = world.presentation_tanks().remove(0);
    assert!(tank.track_left_m < -1.0e-3, "left should run back, got {}", tank.track_left_m);
    assert!(tank.track_right_m > 1.0e-3, "right should run forward, got {}", tank.track_right_m);
    assert!((tank.track_left_m + tank.track_right_m).abs() < 1.0e-4, "a pure pivot is symmetric");
    assert!((tank.track_right_m - 0.5 * half_gauge()).abs() < 1.0e-4);
}

#[test]
fn reversing_runs_both_tracks_backward() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(posed([0.0, 0.0, 0.0], 0.0))]); // seed
    world.sync_tanks(&[still(posed([0.0, 0.0, -3.0], 0.0))]); // backward

    let tank = world.presentation_tanks().remove(0);
    assert!((tank.track_left_m + 3.0).abs() < 1.0e-4, "left {}", tank.track_left_m);
    assert!((tank.track_right_m + 3.0).abs() < 1.0e-4, "right {}", tank.track_right_m);
}

#[test]
fn broken_track_side_stops_accumulating_while_healthy_side_moves() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[still(posed([0.0, 0.0, 0.0], 0.0))]);
    world.sync_tanks(&[still(TankSnapshot {
        track_damage_mask: TrackDamageMask::LEFT.bits(),
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        ..posed([0.0, 0.0, 5.0], 0.0)
    })]);

    let tank = world.presentation_tanks().remove(0);
    assert_eq!(tank.track_left_m, 0.0);
    assert!((tank.track_right_m - 5.0).abs() < 1.0e-4, "right {}", tank.track_right_m);
}

#[test]
fn advance_time_accumulates_ticks_and_elapsed_seconds() {
    let mut world = PresentationWorld::default();

    world.advance_time(0.05);
    world.advance_time(0.025);

    let time = world.time();
    assert_eq!(time.tick, 2);
    assert!((time.delta_seconds - 0.025).abs() < 1.0e-6);
    assert!((time.elapsed_seconds - 0.075).abs() < 1.0e-6);
}

#[test]
fn a_stall_frame_is_clamped_so_the_presentation_never_lurches_through_it() {
    let mut world = PresentationWorld::default();

    // A debugger pause / OS suspend hands the render clock a 3-second "frame".
    world.advance_time(3.0);

    let time = world.time();
    assert!(
        time.delta_seconds <= 0.1,
        "a stall must not land as one huge presentation step, got {}",
        time.delta_seconds
    );
}

#[test]
fn a_thrown_left_track_seats_the_hull_toward_the_dead_side() {
    let mut world = PresentationWorld::default();
    let broken = still(TankSnapshot {
        track_damage_mask: TrackDamageMask::LEFT.bits(),
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        ..snapshot(1, [0.0, 0.0, 0.0], 900)
    });
    // Let the presentation spring ease the lean in over a second of frames.
    for _ in 0..60 {
        world.advance_time(1.0 / 60.0);
        world.sync_tanks(std::slice::from_ref(&broken));
    }
    let tank = world.presentation_tanks().remove(0);
    assert!(
        tank.attitude_roll_rad < -0.02,
        "left-broken hull must lean left (+roll is right side up), got {}",
        tank.attitude_roll_rad
    );
}

#[test]
fn a_fire_event_throws_the_barrel_back_and_the_spring_returns_it_to_battery() {
    let mut world = PresentationWorld::default();
    world.advance_time(1.0 / 60.0);
    world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);

    world.apply_fire_recoil(TankId(1), 0.0, 1.0);

    // The stroke grows over the first frames, peaks visibly, then returns to battery.
    let mut peak = 0.0_f32;
    let mut settled = 0.0_f32;
    for frame in 0..90 {
        world.advance_time(1.0 / 60.0);
        world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        let recoil = world.presentation_tanks()[0].gun_recoil_m;
        peak = peak.max(recoil);
        if frame == 89 {
            settled = recoil;
        }
    }
    assert!(peak > 0.15, "the stroke must be visible over the fender line, got {peak}");
    assert!(peak < 0.6, "the stroke stays a recoil, not an ejection, got {peak}");
    assert!(settled < 0.02, "back in battery within 1.5 s, got {settled}");
}

#[test]
fn firing_over_the_bow_pitches_the_hull_and_over_the_side_rolls_it() {
    let dt = 1.0 / 60.0;
    let run = |turret_yaw: f32| {
        let mut world = PresentationWorld::default();
        world.advance_time(dt);
        world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        // Let the attitude spring seed and settle level first.
        for _ in 0..120 {
            world.advance_time(dt);
            world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        }
        world.apply_fire_recoil(TankId(1), turret_yaw, 1.0);
        let (mut max_pitch, mut max_roll) = (0.0_f32, 0.0_f32);
        for _ in 0..60 {
            world.advance_time(dt);
            world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
            let tank = world.presentation_tanks()[0].clone();
            max_pitch = max_pitch.max(tank.attitude_pitch_rad.abs());
            max_roll = max_roll.max(tank.attitude_roll_rad.abs());
        }
        (max_pitch, max_roll)
    };

    let (bow_pitch, bow_roll) = run(0.0);
    assert!(bow_pitch > 0.005, "a bow shot visibly rocks the hull in pitch, got {bow_pitch}");
    assert!(bow_roll < bow_pitch * 0.2, "a bow shot barely rolls, got {bow_roll}");

    let (side_pitch, side_roll) = run(std::f32::consts::FRAC_PI_2);
    assert!(side_roll > 0.005, "a side shot visibly rolls the hull, got {side_roll}");
    assert!(side_pitch < side_roll * 0.2, "a side shot barely pitches, got {side_pitch}");
}

#[test]
fn a_fire_event_for_an_unknown_tank_is_dropped_quietly() {
    let mut world = PresentationWorld::default();
    world.apply_fire_recoil(TankId(42), 0.0, 1.0);
    assert_eq!(world.tank_count(), 0);
}

/// Inny Poziom S5: an incoming shell rocks the STRUCK tank's hull on its springs from the
/// side it came in — a frontal strike lifts the nose, a strike on the right side rolls the
/// hull, a heavier round further — and the hull settles level again in one nod. The cue
/// reaches any tank in view, not only the player's.
#[test]
fn an_incoming_hit_rocks_the_struck_hull_from_the_side_it_came_in() {
    let dt = 1.0 / 60.0;
    let rock = |bearing: f32, energy: f32| {
        let mut world = PresentationWorld::default();
        world.advance_time(dt);
        world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        for _ in 0..120 {
            world.advance_time(dt);
            world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        }
        world.apply_hit_impulse(TankId(1), bearing, energy);
        let (mut pitch, mut roll) = (0.0_f32, 0.0_f32);
        for _ in 0..60 {
            world.advance_time(dt);
            world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
            let tank = world.presentation_tanks()[0].clone();
            if tank.attitude_pitch_rad.abs() > pitch.abs() {
                pitch = tank.attitude_pitch_rad;
            }
            if tank.attitude_roll_rad.abs() > roll.abs() {
                roll = tank.attitude_roll_rad;
            }
        }
        for _ in 0..240 {
            world.advance_time(dt);
            world.sync_tanks(&[still(snapshot(1, [0.0, 0.0, 0.0], 900))]);
        }
        let settled = world.presentation_tanks()[0].attitude_pitch_rad;
        (pitch, roll, settled)
    };
    let (front_pitch, front_roll, settled) = rock(0.0, 1.0);
    assert!(front_pitch > 0.004, "a frontal strike lifts the nose: {front_pitch}");
    assert!(front_roll.abs() < front_pitch * 0.2, "and barely rolls: {front_roll}");
    assert!(settled.abs() < 1.0e-3, "the hull settles level again: {settled}");
    let (side_pitch, side_roll, _) = rock(std::f32::consts::FRAC_PI_2, 1.0);
    assert!(side_roll > 0.004, "a strike on the right side rolls the hull: {side_roll}");
    assert!(side_pitch.abs() < side_roll * 0.2, "and barely pitches: {side_pitch}");
    let (heavy_pitch, _, _) = rock(0.0, 1.36);
    assert!(heavy_pitch > front_pitch * 1.2, "a heavier round rocks further: {heavy_pitch}");
    // A tank the world does not know takes no cue and panics nothing.
    let mut world = PresentationWorld::default();
    world.apply_hit_impulse(TankId(9), 0.0, 1.0);
}
