//! Locks the damage-event â†’ battle-scar pipeline: a replicated shell strike on a tank records a
//! decal in that tank's local frame (rendered on the hull by the FX pass), non-shell damage does
//! not, and a strike on a tank missing from the snapshot is dropped rather than misplaced.

use game_core::{ArmorZone, DamageCause, DamageEvent, TankId};
use net::{Snapshot, TankSnapshot};

use crate::vehicle::variation::DecalKind;

use super::ClientApp;

#[test]
fn a_shell_strike_records_a_scar_on_the_target_in_its_local_frame() {
    let mut app = ClientApp::new();
    let player = app.player_tank;
    let target_id = TankId(77);
    let target_pos = glam::Vec3::new(20.0, 0.0, 30.0);
    let mut snapshot = snapshot_at(player, 3);
    snapshot.tanks.push(tank_at(target_id, target_pos.to_array()));
    // A penetrating hit on the target's flank, 1 m off its hull centreline.
    let hit = target_pos + glam::Vec3::new(1.05, 1.0, 0.5);
    snapshot.damage_events.push(DamageEvent {
        source: player,
        target: target_id,
        hit_position: hit,
        damage_hp: 120,
        penetrated: true,
        cause: DamageCause::Shell,
        armor_zone: ArmorZone::HullSide,
        ..Default::default()
    });

    app.accept_and_sync(snapshot);

    let scars = app.tank_scars.get(&target_id).expect("target carries a scar");
    assert_eq!(scars.decals().len(), 1);
    let decal = &scars.decals()[0];
    assert_eq!(decal.kind, DecalKind::Penetration, "a pen records the permanent hole");
    let local = glam::Vec3::from_array(decal.local_position);
    assert!(
        (local - glam::Vec3::new(1.05, 1.0, 0.5)).length() < 1.0e-3,
        "the scar lives in the target's local frame, got {local}"
    );

    // And it reaches the frame's FX batch as an on-hull quad near the world hit point.
    let mut vertices = Vec::new();
    app.append_scar_quads(&mut vertices);
    assert!(!vertices.is_empty(), "scars render through the FX pass");
    assert!(
        vertices
            .iter()
            .any(|vertex| { (glam::Vec3::from_array(vertex.position) - hit).length() < 0.5 }),
        "the scar quad sits on the hit point"
    );
}

#[test]
fn ram_damage_and_orphan_events_record_no_scar() {
    let mut app = ClientApp::new();
    let player = app.player_tank;
    let target_id = TankId(77);
    let mut snapshot = snapshot_at(player, 3);
    snapshot.tanks.push(tank_at(target_id, [20.0, 0.0, 30.0]));
    snapshot.damage_events.push(DamageEvent {
        source: player,
        target: target_id,
        hit_position: glam::Vec3::new(20.5, 1.0, 30.0),
        damage_hp: 40,
        penetrated: true,
        cause: DamageCause::Ram,
        ..Default::default()
    });
    // A shell event whose target is not in the snapshot (despawned same tick).
    snapshot.damage_events.push(DamageEvent {
        source: player,
        target: TankId(4242),
        hit_position: glam::Vec3::new(9.0, 1.0, 9.0),
        damage_hp: 90,
        penetrated: true,
        cause: DamageCause::Shell,
        ..Default::default()
    });

    app.accept_and_sync(snapshot);

    assert!(app.tank_scars.get(&target_id).is_none_or(|scars| scars.decals().is_empty()));
    assert!(!app.tank_scars.contains_key(&TankId(4242)));
}

fn tank_at(tank_id: TankId, position: [f32; 3]) -> TankSnapshot {
    let vehicle = game_core::VehicleKind::T54_1951;
    let spec = vehicle.spec();
    TankSnapshot {
        tank_id,
        team: game_core::TeamId(2),
        vehicle,
        position,
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    }
}

fn snapshot_at(tank_id: TankId, server_tick: u64) -> Snapshot {
    Snapshot {
        server_tick,
        tanks: vec![tank_at(tank_id, [0.0, 0.0, 0.0])],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
    }
}
