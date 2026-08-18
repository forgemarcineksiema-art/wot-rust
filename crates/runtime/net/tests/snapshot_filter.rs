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

/// Shells and impacts are world events that ALWAYS replicate (a tracer in the air and the dirt a
/// near-miss throws are visible to everyone standing there) — but their OWNER is intel (v44).
/// Locked here on the worst case: an UNSPOTTED shooter (tank 3, filtered out of the viewer's
/// tank list) whose round and impact still ride through, so incoming fire is never silent — yet
/// carry NO owner, so back-integrating the tracer cannot name the tank that fired it. A visible
/// shooter (tank 2) and the viewer's own (tank 1) keep their owner. A shot from an unspotted gun
/// is dropped entirely — it was never drawable (no pose) and its shooter+shell_id pairing was
/// the sharpest leak of all.
#[test]
fn a_hidden_shooters_shell_replicates_but_carries_no_identity() {
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
        shots_fired: vec![
            game_core::ShotFired { shooter: TankId(1), shell_id: game_core::ShellId(1) },
            game_core::ShotFired { shooter: TankId(2), shell_id: game_core::ShellId(2) },
            game_core::ShotFired { shooter: TankId(3), shell_id: game_core::ShellId(3) },
        ],
    };

    let filtered = snapshot.filtered_for_viewer(TankId(1));

    assert_eq!(tank_ids(&filtered), vec![1, 2], "the hidden shooter's TANK stays filtered");
    // Every shell still in the air — the tracer is a world event, always replicated.
    assert_eq!(filtered.shells.len(), 3, "every shell replicates, hidden shooter included");
    assert_eq!(
        filtered.shells.iter().map(|shell| shell.owner).collect::<Vec<_>>(),
        vec![Some(TankId(1)), Some(TankId(2)), None],
        "the viewer's own and the spotted shooter keep their owner; the hidden one is anonymized"
    );
    assert_eq!(
        filtered.shell_impacts.len(),
        3,
        "every impact replicates - the dirt is world state"
    );
    assert_eq!(
        filtered.shell_impacts.iter().map(|impact| impact.owner).collect::<Vec<_>>(),
        vec![Some(TankId(1)), Some(TankId(2)), None],
        "impacts anonymize their owner exactly like shells"
    );
    // The muzzle-flash event carries shooter+shell_id, the sharpest pairing: it survives only for
    // shooters the viewer may know. The hidden shooter's shot is gone; its tracer still flies.
    assert_eq!(
        filtered.shots_fired.iter().map(|shot| shot.shooter).collect::<Vec<_>>(),
        vec![TankId(1), TankId(2)],
        "an unspotted gun's shot is dropped; a known gun's shot rides through"
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

/// A bounce gives the shooter no honest channel to what its back-face fragments did inside
/// (Amunicja 3.0 A3): on a NON-penetrating event the crew mask reaches only the target's own
/// team. The v46 penetration callout is untouched — the shooter saw the hole.
#[test]
fn a_bounce_does_not_tell_the_shooter_whom_it_wounded() {
    let mut spall = event(1, 2); // shooter team 1, target team 2, both visible
    spall.penetrated = false;
    spall.crew_hits_mask = game_core::CrewRole::Gunner.mask_bit();
    let mut pen = event(1, 2);
    pen.penetrated = true;
    pen.crew_hits_mask = game_core::CrewRole::Loader.mask_bit();

    let snapshot = Snapshot {
        server_tick: 31,
        tanks: vec![
            tank(1, 1, 1_000, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
            tank(2, 2, 900, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
            tank(3, 2, 900, TeamId(1).spotting_bit() | TeamId(2).spotting_bit()),
        ],
        shells: Vec::new(),
        damage_events: vec![spall, pen],
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };

    let shooter_view = snapshot.filtered_for_viewer(TankId(1));
    assert_eq!(
        shooter_view.damage_events[0].crew_hits_mask, 0,
        "the bounce's spall wound is the target crew's own business"
    );
    assert_eq!(
        shooter_view.damage_events[1].crew_hits_mask,
        game_core::CrewRole::Loader.mask_bit(),
        "the penetration callout stays — the shooter saw the hole"
    );

    let target_view = snapshot.filtered_for_viewer(TankId(2));
    assert_eq!(
        target_view.damage_events[0].crew_hits_mask,
        game_core::CrewRole::Gunner.mask_bit(),
        "the victim reads exactly who went down"
    );
    let target_teammate_view = snapshot.filtered_for_viewer(TankId(3));
    assert_eq!(
        target_teammate_view.damage_events[0].crew_hits_mask,
        game_core::CrewRole::Gunner.mask_bit(),
        "crew state is team intel, spall wounds included"
    );
}

/// Crew wounds are interior state exactly like the rack fuze (v46): the team reads who is down
/// and the bandage countdown; an enemy sees a whole crew, and a downed RADIO OPERATOR silences
/// the viewer's own team intel the same way a destroyed radio module does.
#[test]
fn crew_state_is_team_private_and_a_downed_operator_silences_the_net() {
    let mut wounded_teammate = tank(2, 1, 900, TeamId(1).spotting_bit());
    wounded_teammate.crew_unconscious_mask = game_core::CrewRole::Gunner.mask_bit();
    wounded_teammate.crew_weakened_mask = game_core::CrewRole::Driver.mask_bit();
    wounded_teammate.crew_down_remaining_s[game_core::CrewRole::Gunner.wire_index()] = Some(9.5);
    let mut wounded_enemy = tank(5, 2, 900, u8::MAX);
    wounded_enemy.crew_unconscious_mask = game_core::CrewRole::Loader.mask_bit();
    wounded_enemy.crew_down_remaining_s[game_core::CrewRole::Loader.wire_index()] = Some(3.0);

    let snapshot = Snapshot {
        server_tick: 30,
        tanks: vec![tank(1, 1, 1_000, TeamId(1).spotting_bit()), wounded_teammate, wounded_enemy],
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
    let teammate = filtered.tanks.iter().find(|t| t.tank_id == TankId(2)).expect("teammate");
    assert_eq!(teammate.crew_unconscious_mask, game_core::CrewRole::Gunner.mask_bit());
    assert_eq!(teammate.crew_weakened_mask, game_core::CrewRole::Driver.mask_bit());
    assert_eq!(
        teammate.crew_down_remaining_s[game_core::CrewRole::Gunner.wire_index()],
        Some(9.5),
        "the team reads the bandage countdown"
    );
    let enemy = filtered.tanks.iter().find(|t| t.tank_id == TankId(5)).expect("enemy");
    assert_eq!(enemy.crew_unconscious_mask, 0, "an enemy crew reads whole");
    assert_eq!(enemy.crew_weakened_mask, 0);
    assert!(enemy.crew_down_remaining_s.iter().all(Option::is_none));

    // The radio net needs the man as much as the set: with the viewer's own operator down,
    // team-shared spotting collapses to the crew's own eyes (here: nothing), exactly like a
    // destroyed radio module. The spotted-by-team enemy vanishes from the filtered view.
    let mut deaf_viewer = snapshot.clone();
    deaf_viewer.tanks[0].crew_unconscious_mask = game_core::CrewRole::RadioOperator.mask_bit();
    let deaf = deaf_viewer.filtered_for_viewer_with_observers(TankId(1), &[], 0);
    assert!(
        deaf.tanks.iter().all(|t| t.tank_id != TankId(5)),
        "a downed radio operator carries no team intel"
    );
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
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
    }
}

fn shell(owner: u64) -> ShellSnapshot {
    ShellSnapshot {
        shell_id: game_core::ShellId(owner),
        owner: Some(TankId(owner)),
        position: [owner as f32, 1.5, 0.0],
        velocity_mps: [0.0, 0.0, 900.0],
        ..Default::default()
    }
}

fn impact(owner: u64) -> ShellImpact {
    ShellImpact {
        owner: Some(TankId(owner)),
        position: Vec3::new(owner as f32, 0.0, 10.0),
        surface: ImpactSurface::Terrain,
        shell_id: game_core::ShellId(owner),
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

/// v43: a cooking rack is INTERIOR state. The crew and their team read the fuze — that is the
/// decision window the whole mechanic is about — and an enemy learns of it only when it
/// resolves. What never reaches the wire cannot be read out of it.
#[test]
fn snapshot_filter_conceals_an_enemy_rack_fuze_and_keeps_a_teammates() {
    let mut ally = tank(2, 1, 900, 0b11);
    ally.rack_fire_remaining_s = Some(6.5);
    let mut enemy = tank(3, 2, 900, 0b11);
    enemy.rack_fire_remaining_s = Some(4.0);
    let snapshot = Snapshot {
        server_tick: 7,
        tanks: vec![tank(1, 1, 1000, 0b11), ally, enemy],
        ..Default::default()
    };
    let cut = snapshot.filtered_for_viewer(game_core::TankId(1));
    let by_id = |id: u64| cut.tanks.iter().find(|t| t.tank_id.0 == id).expect("visible");
    assert_eq!(by_id(2).rack_fire_remaining_s, Some(6.5), "a teammate's fuze rides the radio net");
    assert_eq!(
        by_id(3).rack_fire_remaining_s,
        None,
        "an enemy's interior stays private until it resolves"
    );
}
