use game_core::{
    DamageCause, DamageEvent, ImpactSurface, ShellImpact, TankId, TeamId, VehicleKind,
};
use glam::Vec3;
use net::{ShellSnapshot, Snapshot, TankSnapshot};

#[test]
fn snapshot_filter_keeps_allies_wrecks_and_spotted_enemies_only() {
    let snapshot = Snapshot {
        server_tick: 12,
        tanks: vec![
            tank(1, 1, 1_000, TeamId(1).spotting_bit()),
            tank(2, 1, 900, TeamId(1).spotting_bit()),
            tank(3, 2, 800, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
            tank(4, 2, 700, TeamId(2).spotting_bit()),
            tank(5, 2, 0, u8::MAX),
        ],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };

    let filtered = snapshot.filtered_for_viewer(TankId(1));

    assert_eq!(tank_ids(&filtered), vec![1, 2, 3, 5]);
}

/// Shells and impacts are world events: a tracer in the air and the dirt a near-miss throws are
/// visible to everyone standing there, whatever the spotting state of the gun that fired. Locked
/// here on the worst case — an UNSPOTTED shooter (tank 3 is filtered out of the viewer's tank
/// list) whose round and impact still replicate, so incoming fire is never silent and a shell
/// never vanishes mid-flight when its owner's spotted hold expires.
#[test]
fn snapshot_filter_replicates_every_shell_and_impact_even_from_hidden_owners() {
    let snapshot = Snapshot {
        server_tick: 12,
        tanks: vec![
            tank(1, 1, 1_000, TeamId(1).spotting_bit()),
            tank(2, 2, 800, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
            tank(3, 2, 800, TeamId(2).spotting_bit()),
        ],
        shells: vec![shell(1), shell(2), shell(3)],
        damage_events: Vec::new(),
        shell_impacts: vec![impact(1), impact(2), impact(3)],
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };

    let filtered = snapshot.filtered_for_viewer(TankId(1));

    assert_eq!(tank_ids(&filtered), vec![1, 2], "the hidden shooter's TANK stays filtered");
    assert_eq!(
        filtered.shells.iter().map(|shell| shell.owner.0).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "every shell in the air replicates, including the hidden shooter's"
    );
    assert_eq!(
        filtered.shell_impacts.iter().map(|impact| impact.owner.0).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "every impact replicates - a near-miss from an unspotted gun still throws dirt"
    );
}

#[test]
fn snapshot_filter_keeps_visible_and_player_combat_events() {
    let snapshot = Snapshot {
        server_tick: 12,
        tanks: vec![
            tank(1, 1, 1_000, TeamId(1).spotting_bit()),
            tank(2, 2, 800, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
            tank(3, 2, 800, TeamId(2).spotting_bit()),
            tank(4, 2, 800, TeamId(2).spotting_bit()),
        ],
        shells: Vec::new(),
        damage_events: vec![
            event(1, 3), // player blind-hit feedback is deliberately retained.
            event(3, 1), // damage taken by player is retained even from a hidden source.
            event(2, 1), // both visible to the viewer.
            event(3, 4), // hidden-vs-hidden combat must not leak.
        ],
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };

    let filtered = snapshot.filtered_for_viewer(TankId(1));

    let pairs = filtered
        .damage_events
        .iter()
        .map(|event| (event.source.0, event.target.0))
        .collect::<Vec<_>>();
    assert_eq!(pairs, vec![(1, 3), (3, 1), (2, 1)]);
}

#[test]
fn snapshot_filter_keeps_detached_turret_wrecks_the_viewer_can_see() {
    let snapshot = Snapshot {
        server_tick: 12,
        tanks: vec![
            tank(1, 1, 1_000, TeamId(1).spotting_bit()),
            tank(5, 2, 0, u8::MAX), // a visible enemy wreck
            tank(6, 2, 0, 0),       // an unspotted enemy wreck — but wrecks are always visible
        ],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        // Both wrecks lost their turret; the viewer sees both (the hit_points == 0 rule).
        detached_turrets: vec![TankId(5), TankId(6)],
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };

    let filtered = snapshot.filtered_for_viewer(TankId(1));

    assert_eq!(filtered.detached_turrets, vec![TankId(5), TankId(6)]);
}

fn tank(id: u64, team: u16, hit_points: u32, spotted_by_teams_mask: u8) -> TankSnapshot {
    let spec = VehicleKind::T54_1951.spec();
    TankSnapshot {
        tank_id: TankId(id),
        team: TeamId(team),
        vehicle: spec.kind,
        position: [id as f32, 0.0, 0.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: spec.ammo.counts,
        selected_ammo: spec.ammo.initial_selected,
        spotted_by_teams_mask,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    }
}

fn shell(owner: u64) -> ShellSnapshot {
    ShellSnapshot {
        owner: TankId(owner),
        position: [owner as f32, 1.5, 0.0],
        velocity_mps: [0.0, 0.0, 900.0],
        ..Default::default()
    }
}

fn impact(owner: u64) -> ShellImpact {
    ShellImpact {
        owner: TankId(owner),
        position: Vec3::new(owner as f32, 0.0, 10.0),
        surface: ImpactSurface::Terrain,
        ..ShellImpact::default()
    }
}

fn event(source: u64, target: u64) -> DamageEvent {
    DamageEvent {
        source: TankId(source),
        target: TankId(target),
        hit_position: Vec3::new(target as f32, 1.0, 0.0),
        damage_hp: 100,
        penetrated: true,
        cause: DamageCause::Shell,
        ..Default::default()
    }
}

fn tank_ids(snapshot: &Snapshot) -> Vec<u64> {
    snapshot.tanks.iter().map(|tank| tank.tank_id.0).collect()
}

/// N4 fairness: beyond 300 m an enemy's EXACT pool is intel the crew could not have — the bar
/// reports 20% steps, rounded UP (never a fake kill reading), while teammates, wrecks and
/// close-range enemies stay exact.
#[test]
fn distant_enemy_hp_is_quantized_never_exact() {
    let viewer = tank(1, 1, 1000, 0b01);
    let mut close_enemy = tank(2, 2, 743, 0b01);
    close_enemy.position = [100.0, 0.0, 0.0];
    let mut far_enemy = tank(3, 2, 743, 0b01);
    far_enemy.position = [500.0, 0.0, 0.0];
    let mut ally = tank(4, 1, 743, 0b01);
    ally.position = [500.0, 0.0, 0.0];
    let snapshot = net::Snapshot {
        server_tick: 9,
        tanks: vec![viewer, close_enemy, far_enemy, ally],
        ..Default::default()
    };
    let cut = snapshot.filtered_for_viewer(game_core::TankId(1));
    let hp = |id: u64| cut.tanks.iter().find(|t| t.tank_id.0 == id).expect("visible").hit_points;
    assert_eq!(hp(2), 743, "a close enemy shows its true pool");
    assert_eq!(hp(4), 743, "an ally is always exact");
    let full = cut.tanks[2].vehicle.spec().hit_points as f32;
    let far = hp(3);
    assert_ne!(far, 743, "a distant enemy's exact pool is hidden");
    assert!(far >= 743, "quantization rounds UP, never toward a fake kill");
    let step = (full * 0.20).max(1.0);
    assert!(
        (far as f32 / step).fract().abs() < 1.0e-3,
        "the reading sits on a 20% step, got {far}"
    );
}

/// N4: a dead viewer keeps its TEAM's vision — spectating your own side is not a wallhack, and
/// the death screen must not blink the battle away.
#[test]
fn a_dead_viewer_keeps_team_vision() {
    let mut viewer = tank(1, 1, 0, 0b11);
    viewer.position = [0.0, 0.0, 0.0];
    let spotted_enemy = tank(2, 2, 900, 0b01);
    let hidden_enemy = tank(3, 2, 900, 0b10);
    let snapshot = net::Snapshot {
        server_tick: 9,
        tanks: vec![viewer, spotted_enemy, hidden_enemy],
        ..Default::default()
    };
    let cut = snapshot.filtered_for_viewer(game_core::TankId(1));
    assert!(cut.tanks.iter().any(|t| t.tank_id.0 == 2), "team-spotted enemies stay visible");
    assert!(!cut.tanks.iter().any(|t| t.tank_id.0 == 3), "unspotted enemies stay hidden");
}
