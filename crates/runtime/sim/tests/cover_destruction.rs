//! Locks destructible static cover (protocol v21): HE brings structures down, a hull flattens a
//! hedgerow it drives through, and — the honesty payoff — the shell trace and spotting LOS follow
//! the state, so a shot passes (and a target is seen) exactly where cover was cleared.

use std::f32::consts::PI;

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{CoverPhase, FixedTimestep, SimulationState, TankCommand};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

const HE_SLOT: u8 = 2;

fn flat_field() -> HeightMap {
    HeightMap::flat(96, 96, 4.0, 0.0).expect("flat terrain")
}

fn cover(id: &str, kind: StaticCoverKind, center: [f32; 3], half: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject { id: id.into(), name: id.into(), kind, center, half_extents_m: half }
}

fn fire_once(
    state: &mut SimulationState,
    shooter: TankId,
    terrain: &HeightMap,
    cover: &[StaticCoverObject],
) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_battlefield(
        &[(shooter, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        terrain,
        cover,
    );
    for _ in 0..40 {
        if state.shells().is_empty() {
            break;
        }
        state.apply_commands_on_battlefield(&[], step, terrain, cover);
    }
}

#[test]
fn a_high_explosive_round_brings_a_building_down() {
    let terrain = flat_field();
    let barn = [cover("barn", StaticCoverKind::FarmBuilding, [0.0, 1.5, 27.0], [4.0, 2.5, 1.5])];

    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let _target =
        state.spawn_tank_with_yaw(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0), PI);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.selected_ammo = HE_SLOT; // HE fells structures
        shooter.ammo_counts[HE_SLOT as usize] = 5; // guarantee HE rounds on hand
    }
    // Cover states are lazily sized on the first battlefield tick, so `full` is the kind's max.
    let full = StaticCoverKind::FarmBuilding.max_health().unwrap();

    fire_once(&mut state, shooter, &terrain, &barn);

    let after = state.cover_states()[0];
    assert!(after.health < full, "the HE round chipped the building: {} < {full}", after.health);
    // 600 hp barn, 300 per HE hit: one round leaves it standing but wounded.
    assert_eq!(after.phase, CoverPhase::Intact, "one HE round does not yet collapse a barn");
}

#[test]
fn a_hull_driving_through_a_hedgerow_flattens_it_and_takes_a_nick() {
    let terrain = flat_field();
    let hedge = [cover("hedge", StaticCoverKind::TreeLine, [0.0, 1.0, 20.0], [10.0, 1.0, 0.6])];

    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(0.0, 0.0, 10.0));
    let full_hp = state.tank(tank).expect("tank").hit_points;
    let step = FixedTimestep::from_hz(60);
    for _ in 0..300 {
        state.apply_commands_on_battlefield(
            &[(tank, TankCommand::drive(1.0, 0.0))],
            step,
            &terrain,
            &hedge,
        );
    }

    assert_eq!(state.cover_states()[0].phase, CoverPhase::Gone, "the hull flattened the hedge");
    let tank = state.tank(tank).expect("tank");
    assert!(tank.hit_points < full_hp, "bulldozing is not free — the hull took a nick");
    assert!(tank.position.z > 20.0, "and the hull drove on THROUGH where the hedge stood");
}

#[test]
fn a_shell_flies_where_a_crushed_hedgerow_used_to_block_it() {
    let terrain = flat_field();
    // A hedge straddling the shot line at z = 27, blocking a duel between 0 and 55.
    let hedge = [cover("hedge", StaticCoverKind::TreeLine, [0.0, 1.5, 27.0], [6.0, 1.5, 0.6])];

    // First: with the hedge intact, the shot is absorbed short of the target.
    let mut blocked = SimulationState::new();
    let shooter = blocked.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let _target =
        blocked.spawn_tank_with_yaw(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0), PI);
    blocked.tank_mut(shooter).unwrap().aim_dispersion_mrad = 0.0;
    blocked.tank_mut(shooter).unwrap().spec.gun.dispersion_mrad = 0.0;
    fire_once(&mut blocked, shooter, &terrain, &hedge);
    assert!(blocked.damage_events().is_empty(), "the intact hedge absorbs the shell");

    // Now crush the hedge with a hull, then fire the same shot: it flies clean to the target.
    let mut open = SimulationState::new();
    let shooter = open.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target =
        open.spawn_tank_with_yaw(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0), PI);
    let crusher = open.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(0.0, 0.0, 20.0));
    open.tank_mut(shooter).unwrap().aim_dispersion_mrad = 0.0;
    open.tank_mut(shooter).unwrap().spec.gun.dispersion_mrad = 0.0;
    let step = FixedTimestep::from_hz(60);
    for _ in 0..240 {
        open.apply_commands_on_battlefield(
            &[(crusher, TankCommand::drive(1.0, 0.0))],
            step,
            &terrain,
            &hedge,
        );
        if open.cover_states()[0].phase == CoverPhase::Gone {
            break;
        }
    }
    // Move the crusher clear of the shot line, then fire.
    for _ in 0..240 {
        open.apply_commands_on_battlefield(
            &[(crusher, TankCommand::drive(1.0, 1.0))],
            step,
            &terrain,
            &hedge,
        );
    }
    fire_once(&mut open, shooter, &terrain, &hedge);
    let hit = open.damage_events().iter().any(|event| event.target == target);
    assert!(hit, "with the hedge gone the shell reaches the enemy");
}

#[test]
fn spotting_opens_once_the_hedge_between_is_shot_away() {
    let terrain = flat_field();
    // A tall hedge wall between an observer and an enemy, blocking the sight line.
    let hedge = [cover("hedge", StaticCoverKind::TreeLine, [0.0, 2.0, 30.0], [12.0, 2.5, 0.8])];

    let mut state = SimulationState::new();
    let observer = state.spawn_tank(TeamId(1), TankSpec::t54_1951().clone(), Vec3::ZERO);
    let enemy =
        state.spawn_tank_with_yaw(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 60.0), PI);
    {
        let observer = state.tank_mut(observer).expect("observer");
        observer.aim_dispersion_mrad = 0.0;
        observer.spec.gun.dispersion_mrad = 0.0;
        observer.selected_ammo = HE_SLOT;
    }
    let team1_bit = TeamId(1).spotting_bit();

    // With the hedge up, the enemy is hidden.
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_battlefield(&[], step, &terrain, &hedge);
    assert_eq!(
        state.tank(enemy).unwrap().spotted_mask & team1_bit,
        0,
        "the hedge hides the enemy behind it"
    );

    // Shoot the hedge away, then the sight line clears and the enemy lights up.
    fire_once(&mut state, observer, &terrain, &hedge);
    assert_eq!(state.cover_states()[0].phase, CoverPhase::Gone, "one HE round clears the hedge");
    for _ in 0..4 {
        state.apply_commands_on_battlefield(&[], step, &terrain, &hedge);
    }
    assert_ne!(
        state.tank(enemy).unwrap().spotted_mask & team1_bit,
        0,
        "with the hedge gone the enemy is spotted"
    );
}

/// Fizyczny Świat P8 (protocol v32): the shell that chips a wall also WOUNDS it — a replicated
/// scar on the struck face, so every client (and a late joiner) dresses the same wall alike.
#[test]
fn an_absorbed_shell_leaves_a_replicated_wound_on_the_struck_face() {
    let terrain = flat_field();
    let barn = [cover("barn", StaticCoverKind::FarmBuilding, [0.0, 1.5, 27.0], [4.0, 2.5, 1.5])];

    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let _target =
        state.spawn_tank_with_yaw(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0), PI);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.selected_ammo = HE_SLOT;
        shooter.ammo_counts[HE_SLOT as usize] = 5;
    }
    fire_once(&mut state, shooter, &terrain, &barn);

    assert_eq!(state.cover_scars().len(), 1, "one absorbed shell, one wound");
    let scar = state.cover_scars()[0];
    assert_eq!(scar.cover, 0);
    assert_eq!(scar.kind, terrain::COVER_SCAR_KIND_HIGH_EXPLOSIVE);
    assert_eq!(scar.face, 3, "fired from -Z, the wound sits on the -Z face");
    assert!(scar.radius_m() > 0.25, "an HE bite is decimetres wide: {}", scar.radius_m());
}

/// The per-cover cap: a wall shelled forever remembers its freshest eight wounds.
#[test]
fn a_wall_remembers_at_most_eight_wounds_and_recycles_the_oldest() {
    let barn = cover("barn", StaticCoverKind::FarmBuilding, [0.0, 1.5, 27.0], [4.0, 2.5, 1.5]);
    let mut ledger = Vec::new();
    for index in 0..12 {
        let impact = game_core::ShellImpact {
            owner: game_core::TankId(1),
            position: glam::Vec3::new(-3.0 + index as f32 * 0.5, 1.5, 25.5),
            surface: game_core::ImpactSurface::Cover,
            shell_type: game_core::ShellType::ArmorPiercing,
            direction: glam::Vec3::Z,
            caliber_mm: 100.0,
        };
        sim::record_cover_scar(&mut ledger, 0, &barn, &impact);
    }
    assert_eq!(ledger.len(), terrain::MAX_COVER_SCARS_PER_COVER);
    // The freshest strike (furthest right) survived; the first (furthest left) weathered away.
    assert!(ledger.iter().all(|scar| scar.u_q != ledger_first_u()), "oldest recycled");
}

fn ledger_first_u() -> u8 {
    // The u of the very first strike above (x = -3.0 on an 8 m face): (-3/4+1)/2*255 = 32.
    32
}
