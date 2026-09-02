//! Inny Poziom S10: impact FX are seated in the target's frame. Tanks render interpolated one
//! snapshot (50 ms) behind the sim; FX spawned at the WORLD hit point hung in the air up to
//! 0.5 m (≈ 10 px at 300 m) behind a crossing hull, while the breach decal — seated in the hull
//! frame — opened on the plate. Seated FX ride the same interpolated pose the decal does.

use game_core::{ArmorFacing, ArmorZone, DamageCause, DamageEvent, ShellType, TankId, TeamId};
use glam::Vec3;
use net::TankSnapshot;

use super::{FxSystem, decal_from_damage_event, pose_of};

fn target_at(z: f32) -> TankSnapshot {
    let vehicle = game_core::VehicleKind::T54_1951;
    TankSnapshot {
        tank_id: TankId(9),
        team: TeamId(2),
        vehicle,
        position: [0.0, 0.0, z],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 1.0,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
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

/// A hit on the hull's left side, resolved at the SIM pose (the hull at z = 10).
fn side_hit(target: &TankSnapshot) -> DamageEvent {
    DamageEvent {
        source: TankId(1),
        target: target.tank_id,
        hit_position: Vec3::from_array(target.position) + Vec3::new(-1.6, 1.0, 0.3),
        damage_hp: 300,
        penetrated: true,
        cause: DamageCause::Shell,
        shell_type: ShellType::ArmorPiercing,
        plate_normal: Vec3::NEG_X,
        shell_direction: Vec3::X,
        armor_facing: ArmorFacing::HullSide,
        armor_zone: ArmorZone::HullSide,
        ..Default::default()
    }
}

fn centroid(fx: &FxSystem, presented: &TankSnapshot) -> Vec3 {
    let seat_pose = |id: TankId| (id == presented.tank_id).then(|| pose_of(presented));
    let verts = fx.vertices(Vec3::new(0.0, 3.0, -20.0), Vec3::ZERO, &seat_pose);
    assert!(!verts.is_empty(), "the burst draws");
    verts.iter().map(|v| Vec3::from_array(v.position)).sum::<Vec3>() / verts.len() as f32
}

/// At 10 m/s the hull the sim struck is 0.5 m ahead of the hull the client draws. The seated
/// signature's centroid stays within 5 cm of the seated decal after a presented frame; the old
/// world-space spawn hung 0.4 m and more behind the plate it was meant to mark.
#[test]
fn seated_impact_fx_stay_on_the_plate_of_a_crossing_hull() {
    let sim_pose = target_at(10.0);
    let presented = target_at(9.5);
    let event = side_hit(&sim_pose);
    let decal = decal_from_damage_event(&event, &sim_pose, None).expect("a hull decal");
    let decal_world = pose_of(&presented).hull_point(Vec3::from_array(decal.local_position));

    // The signature's entry flash has no velocity of its own: it IS the hit point, so its
    // distance to the decal is the seating error alone, with none of the sparks' spread in it.
    let flash_of = |fx: &FxSystem| {
        *fx.particles.iter().find(|p| p.velocity_mps == Vec3::ZERO).expect("the entry flash")
    };
    let mut seated = FxSystem::default();
    seated.seated(sim_pose.tank_id, &pose_of(&sim_pose), |fx| {
        fx.armor_hit_directed(event.hit_position, true, false, None, 0.3);
    });
    seated.tick(1.0 / 60.0);
    let flash = flash_of(&seated);
    assert_eq!(flash.seat, Some(sim_pose.tank_id), "the flash is seated");
    let seated_gap = (pose_of(&presented).hull_point(flash.position) - decal_world).length();

    let mut loose = FxSystem::default();
    loose.armor_hit_directed(event.hit_position, true, false, None, 0.3);
    loose.tick(1.0 / 60.0);
    let loose_gap = (flash_of(&loose).position - decal_world).length();

    assert!(seated_gap <= 0.05, "seated FX sit on the decal: {seated_gap:.3} m");
    assert!(loose_gap >= 0.4, "the world-space spawn hung behind the hull: {loose_gap:.3} m");
    // And the whole seated burst draws through the presented pose, sparks and smoke included.
    let drawn = centroid(&seated, &presented);
    assert!((drawn - decal_world).length() < 0.5, "the burst is on the hull, not behind it");
}

/// A seat whose hull is gone this frame draws nothing rather than a particle at a stale pose;
/// world-space particles are untouched by the seat lookup.
#[test]
fn a_seat_without_its_hull_draws_nothing_and_world_particles_do_not_care() {
    let sim_pose = target_at(10.0);
    let mut fx = FxSystem::default();
    fx.seated(sim_pose.tank_id, &pose_of(&sim_pose), |fx| {
        fx.impact_burst(Vec3::new(0.0, 1.0, 10.0), game_core::ImpactSurface::Hull)
    });
    fx.impact_burst(Vec3::new(5.0, 1.0, 5.0), game_core::ImpactSurface::Hull);
    let none = fx.vertices(Vec3::new(0.0, 3.0, -20.0), Vec3::ZERO, &|_| None);
    assert_eq!(none.len(), 8 * 6, "only the eight world sparks draw without the hull");
    let seat_pose = |id: TankId| (id == sim_pose.tank_id).then(|| pose_of(&sim_pose));
    let all = fx.vertices(Vec3::new(0.0, 3.0, -20.0), Vec3::ZERO, &seat_pose);
    assert_eq!(all.len(), 16 * 6, "and all sixteen with it");
}
