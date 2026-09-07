//! Locks the HE burst: `explosive_radius_m` is finally consumed. A non-penetrating HE hit
//! bursts on the armor and throws blast damage at everything inside its radius — attenuated by
//! distance and soaked by the victim's thinnest external plate. Allies are protected exactly
//! like direct fire; out-of-radius vehicles feel nothing.

use std::f32::consts::PI;

use game_core::{DamageCause, ShellSpec, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

#[test]
fn a_non_pen_he_burst_splashes_the_tank_beside_the_impact() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    // A big HE round: hopeless against the Tiger II's plate, generous 4 m blast.
    state.tank_mut(shooter).expect("shooter").spec.gun.shell =
        ShellSpec::high_explosive(122.0, 515.0, 38.0, 410, 4.0);
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.007;

    let wall = state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 40.0));
    state.tank_mut(wall).expect("wall").yaw_rad = PI;
    // A medium parked right beside the burst, an ally of the shooter equally close, and an
    // enemy far outside the radius.
    let bystander = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(-3.4, 0.0, 40.0));
    let ally = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(3.4, 0.0, 40.0));
    let far = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(-14.0, 0.0, 40.0));
    let bystander_hp = state.tank(bystander).expect("bystander").hit_points;
    let ally_hp = state.tank(ally).expect("ally").hit_points;
    let far_hp = state.tank(far).expect("far").hit_points;

    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..240 {
        if !state.damage_events().is_empty() {
            break;
        }
        state.apply_commands(&[], step);
    }

    let events = state.damage_events();
    let direct = events
        .iter()
        .find(|event| event.target == wall && event.cause == DamageCause::Shell)
        .expect("the HE round bursts on the Tiger II");
    assert!(!direct.penetrated, "38 mm of HE penetration cannot open a Tiger II");
    // S15: the burst on a Tiger II glacis (150 mm at 50 degrees, 233 mm through) does nothing
    // to the tank it hit (0.5 * 410 - 1.3 * 233 < 0); the whole bill is the blast around it.
    assert_eq!(direct.damage_hp, 0, "the surface burst finds too much steel under it");

    let splash = events
        .iter()
        .find(|event| event.target == bystander && event.cause == DamageCause::Splash)
        .expect("the blast reaches the tank beside the impact");
    assert!(splash.damage_hp > 0);
    assert!(!splash.penetrated);
    assert!(
        state.tank(bystander).expect("bystander").hit_points < bystander_hp,
        "blast damage lands on the bystander's hit points"
    );

    assert!(
        events.iter().all(|event| event.target != ally),
        "allies are protected from splash exactly like direct fire"
    );
    assert_eq!(state.tank(ally).expect("ally").hit_points, ally_hp);
    assert!(
        events.iter().all(|event| event.target != far),
        "a tank outside the blast radius feels nothing"
    );
    assert_eq!(state.tank(far).expect("far").hit_points, far_hp);
}

/// S23: a wreck is steel to the blast. A dead T-54 parked between the burst and a bystander
/// shields it; the same bystander with the lane empty takes the wave.
#[test]
fn a_wreck_between_the_burst_and_a_bystander_shields_it() {
    let run = |with_wreck: bool| {
        let mut state = SimulationState::new();
        let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
        // A big blast, so the far bystander is well inside the radius either way.
        state.tank_mut(shooter).expect("shooter").spec.gun.shell =
            ShellSpec::high_explosive(152.0, 400.0, 40.0, 600, 12.0);
        state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.007;
        let wall =
            state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 40.0));
        state.tank_mut(wall).expect("wall").yaw_rad = PI;
        let bystander =
            state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(-8.5, 0.0, 36.0));
        let bystander_hp = state.tank(bystander).expect("bystander").hit_points;
        if with_wreck {
            let wreck =
                state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(-4.0, 0.0, 36.0));
            state.tank_mut(wreck).expect("wreck").hit_points = 0;
        }
        let step = FixedTimestep::from_hz(60);
        state.apply_commands(&[(shooter, fire_command())], step);
        for _ in 0..240 {
            if !state.damage_events().is_empty() {
                break;
            }
            state.apply_commands(&[], step);
        }
        let events = state.damage_events();
        assert!(
            events.iter().any(|event| event.target == wall && event.cause == DamageCause::Shell),
            "the round bursts on the Tiger II"
        );
        let splashed = events
            .iter()
            .any(|event| event.target == bystander && event.cause == DamageCause::Splash);
        (splashed, bystander_hp - state.tank(bystander).expect("bystander").hit_points)
    };

    let (splashed, lost) = run(false);
    assert!(splashed && lost > 0, "with the lane empty the bystander takes the wave: {lost}");
    let (splashed, lost) = run(true);
    assert!(!splashed && lost == 0, "behind a wreck the bystander takes nothing: {lost}");
}

/// A burst BESIDE a hull throws the band on the side it came from.
///
/// Until 2026-08-02 splash was hit points and nothing else: `shell_splash.rs` mentioned no module,
/// no track and no crew, so HE was a track weapon only when it landed ON the hull — the case a
/// player is least likely to be aiming for when they load it against a flanker. The chunk had been
/// waiting the whole time: `track_hit_damage`'s HE branch is documented as "splash / non-pen" and
/// only the non-pen half ever called it.
#[test]
fn a_burst_beside_a_hull_throws_the_band_it_came_from() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    state.tank_mut(shooter).expect("shooter").spec.gun.shell =
        ShellSpec::high_explosive(122.0, 515.0, 38.0, 410, 4.0);
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.007;

    let wall = state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 40.0));
    state.tank_mut(wall).expect("wall").yaw_rad = PI;
    // Parked to the burst's LEFT, so the blast arrives on its own RIGHT flank.
    let bystander = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(-3.4, 0.0, 40.0));
    let before = state.tank(bystander).expect("bystander").tracks;

    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..240 {
        if !state.damage_events().is_empty() {
            break;
        }
        state.apply_commands(&[], step);
    }

    let splash = state
        .damage_events()
        .iter()
        .find(|event| event.target == bystander && event.cause == DamageCause::Splash)
        .copied()
        .expect("the blast reaches the tank beside the impact");

    let hit = splash.track_hit.expect("a burst at track height throws links, not only hit points");
    assert_eq!(
        hit.side,
        game_core::TrackSide::Right,
        "the blast came from the bystander's right, so that is the band it chipped"
    );
    assert_eq!(
        splash.module,
        Some(game_core::ModuleSlot::Suspension),
        "the running gear is the one module a burst reaches without penetrating"
    );
    let after = state.tank(bystander).expect("bystander").tracks;
    assert!(
        after.hp(game_core::TrackSide::Right) < before.hp(game_core::TrackSide::Right),
        "the struck band actually lost pool"
    );
    assert_eq!(
        after.hp(game_core::TrackSide::Left),
        before.hp(game_core::TrackSide::Left),
        "the far band is untouched — a blast has a side"
    );
}
