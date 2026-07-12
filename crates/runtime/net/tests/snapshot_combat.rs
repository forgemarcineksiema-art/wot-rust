use game_core::{DamageCause, DamageEvent, ModuleSlot, TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use net::{ProtocolMessage, ShellSnapshot, Snapshot, TankSnapshot, decode_message, encode_message};
use sim::SimulationState;

#[test]
fn snapshots_carry_projectiles_and_damage_events_through_the_wire() {
    let snapshot = Snapshot {
        server_tick: 9,
        tanks: vec![TankSnapshot {
            tank_id: TankId(1),
            team: TeamId(1),
            vehicle: VehicleKind::TigerII,
            position: [3.0, 0.5, 12.0],
            yaw_rad: 0.2,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: -0.1,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.05,
            hit_points: 2_050,
            reload_remaining_s: 1.5,
            aim_dispersion_mrad: 6.25,
            module_hit_points: VehicleKind::TigerII.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 1 << 3,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: [24, 10, 6],
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
        }],
        shells: vec![ShellSnapshot {
            owner: TankId(1),
            position: [0.0, 1.5, 12.0],
            velocity_mps: [0.0, 0.0, 900.0],
            ..Default::default()
        }],
        damage_events: vec![DamageEvent {
            source: TankId(1),
            target: TankId(2),
            hit_position: Vec3::new(0.0, 1.2, 55.0),
            damage_hp: 320,
            penetrated: true,
            cause: DamageCause::Shell,
            module: Some(ModuleSlot::Gun),
            ..Default::default()
        }],
        shell_impacts: vec![game_core::ShellImpact {
            owner: TankId(1),
            position: Vec3::new(4.0, 0.1, 70.0),
            surface: game_core::ImpactSurface::Cover,
            ..Default::default()
        }],
        detached_turrets: vec![TankId(2)],
        cover_states: vec![1],
    };
    let message = ProtocolMessage::Snapshot(snapshot);

    // Real wire roundtrip, not just asserting fields we set one line above.
    let bytes = encode_message(&message).expect("snapshot should encode");
    let decoded = decode_message(&bytes).expect("snapshot should decode");

    assert_eq!(decoded, message);
    let ProtocolMessage::Snapshot(round) = decoded else { panic!("expected a Snapshot") };
    assert_eq!(round.tanks[0].vehicle, VehicleKind::TigerII);
    assert_eq!(round.tanks[0].aim_dispersion_mrad, 6.25);
    assert_eq!(
        round.tanks[0].module_hit_points,
        VehicleKind::TigerII.spec().module_health.hit_points_by_slot()
    );
    assert_eq!(round.tanks[0].destroyed_modules_mask, 1 << 3);
    assert_eq!(round.shells.len(), 1);
    assert_eq!(round.damage_events[0].damage_hp, 320);
    assert_eq!(round.damage_events[0].cause, DamageCause::Shell);
    assert_eq!(round.damage_events[0].module, Some(ModuleSlot::Gun));
    assert_eq!(round.tanks[0].team, TeamId(1));
    assert_eq!(round.shell_impacts.len(), 1);
    assert_eq!(round.shell_impacts[0].surface, game_core::ImpactSurface::Cover);
}

#[test]
fn tank_snapshot_replicates_team_and_shell_impacts_from_sim_state() {
    let mut state = SimulationState::new();
    state.spawn_tank(TeamId(7), TankSpec::t55a(), Vec3::ZERO);

    let snapshot = Snapshot::from(&state);

    assert_eq!(snapshot.tanks[0].team, TeamId(7));
    assert!(snapshot.shell_impacts.is_empty(), "no shells fired, no impacts");
}

#[test]
fn shell_snapshot_carries_stable_identity_and_flight_parameters_from_sim() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    state.apply_commands(
        &[(tank, sim::TankCommand { fire: true, ..sim::TankCommand::idle() })],
        sim::FixedTimestep::from_hz(60),
    );

    let authoritative = state.shells().first().expect("shot remains in flight");
    let snapshot = Snapshot::from(&state);
    let replicated = snapshot.shells.first().expect("shot is replicated");

    assert_eq!(replicated.shell_id, authoritative.id);
    assert_ne!(replicated.shell_id, game_core::ShellId::default());
    assert_eq!(replicated.shell_type, authoritative.shell.shell_type);
    assert_eq!(replicated.caliber_mm, authoritative.shell.caliber_mm);
    assert_eq!(replicated.drag_per_s, authoritative.shell.drag_per_s());
    assert_eq!(replicated.age_seconds, authoritative.age_seconds);
}

#[test]
fn tank_snapshot_replicates_destroyed_module_mask_from_sim_state() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::tiger_i_ausf_e(), Vec3::ZERO);
    let modules = &mut state.tank_mut(tank).expect("tank").modules;
    modules.damage(ModuleSlot::Engine, u32::MAX);
    modules.damage(ModuleSlot::Gun, u32::MAX);

    let snapshot = Snapshot::from(&state);

    let expected_mask = (1 << 0) | (1 << 3);
    assert_eq!(snapshot.tanks[0].destroyed_modules_mask, expected_mask);
}

#[test]
fn tank_snapshot_replicates_live_module_hit_points_from_sim_state() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let modules = &mut state.tank_mut(tank).expect("tank").modules;
    modules.damage(ModuleSlot::Gun, 17);
    modules.damage(ModuleSlot::AmmoRack, 31);

    let snapshot = Snapshot::from(&state);
    let spec = TankSpec::t55a();

    assert_eq!(
        snapshot.tanks[0].module_hit_points,
        [
            spec.module_health.hit_points(ModuleSlot::Engine),
            spec.module_health.hit_points(ModuleSlot::Suspension),
            spec.module_health.hit_points(ModuleSlot::Turret),
            spec.module_health.hit_points(ModuleSlot::Gun) - 17,
            spec.module_health.hit_points(ModuleSlot::AmmoRack) - 31,
            spec.module_health.hit_points(ModuleSlot::Radio),
        ]
    );
}

#[test]
fn tank_snapshot_replicates_authoritative_aim_dispersion() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    state.tank_mut(tank).expect("tank").aim_dispersion_mrad = 9.5;

    let snapshot = Snapshot::from(&state);

    assert_eq!(snapshot.tanks[0].aim_dispersion_mrad, 9.5);
}

#[test]
fn tank_snapshot_replicates_authoritative_turret_yaw_velocity() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    state.tank_mut(tank).expect("tank").turret_yaw_velocity_rad_s = 0.31;

    let snapshot = Snapshot::from(&state);

    assert_eq!(snapshot.tanks[0].turret_yaw_velocity_rad_s, 0.31);
}
