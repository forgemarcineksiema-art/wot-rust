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
        ammo_counts: spec.ammo.counts,
        selected_ammo: spec.ammo.initial_selected,
        spotted_by_teams_mask,
    }
}

fn shell(owner: u64) -> ShellSnapshot {
    ShellSnapshot {
        owner: TankId(owner),
        position: [owner as f32, 1.5, 0.0],
        velocity_mps: [0.0, 0.0, 900.0],
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
