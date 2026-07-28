use game_core::{TankId, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

use super::live_cover::LiveCoverCache;

fn object(
    id: &str,
    kind: StaticCoverKind,
    center: [f32; 3],
    half_extents_m: [f32; 3],
) -> StaticCoverObject {
    StaticCoverObject { id: id.to_string(), name: id.to_string(), kind, center, half_extents_m }
}

fn player_snapshot(position: [f32; 3]) -> TankSnapshot {
    let spec = VehicleKind::T54_1951.spec();
    TankSnapshot {
        tank_id: TankId(1),
        team: TeamId(1),
        vehicle: VehicleKind::T54_1951,
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
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    }
}

#[test]
fn born_ruins_seed_blocking_and_camera_before_the_first_snapshot() {
    let authored = vec![
        object("whole", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
        object("tenement_ruin", StaticCoverKind::CityBuilding, [30.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
        object("wall_ruin", StaticCoverKind::StoneWall, [60.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
    ];

    let cache = LiveCoverCache::from_born_phases(&authored);

    assert_eq!(cache.phase_bytes(), [0, 1, 2]);
    assert_eq!(
        cache.blocking().iter().map(|cover| cover.id.as_str()).collect::<Vec<_>>(),
        ["whole", "tenement_ruin"],
        "a born wall breach is absent while a born building ruin keeps a mound"
    );
    assert!(cache.blocking()[1].half_extents_m[1] < authored[1].half_extents_m[1]);
    assert_eq!(cache.camera_obstacles().len(), cache.blocking().len());
    for (cover, obstacle) in cache.blocking().iter().zip(cache.camera_obstacles()) {
        assert_eq!(obstacle.center, cover.center);
        assert_eq!(obstacle.half_extents, cover.half_extents_m);
    }
    assert!(
        LiveCoverCache::from_replicated(&authored, &[0, 1]).is_none(),
        "an incomplete startup snapshot must not resurrect a born ruin"
    );
}

#[test]
fn prediction_stops_on_intact_and_rubble_cover_but_passes_gone_cover() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let authored =
        [object("block", StaticCoverKind::CityBuilding, [10.0, 3.0, 30.0], [5.0, 3.0, 4.0])];
    let predicted_z = |phase| {
        let cache = LiveCoverCache::from_replicated(&authored, &[phase]).unwrap();
        let spec = VehicleKind::T54_1951.spec();
        let mut predictor = crate::predict::LocalPredictor::new(&spec);
        predictor.sync_to(&player_snapshot([10.0, 0.0, 10.0]));
        for _ in 0..240 {
            predictor.step(
                sim::TankCommand::drive(1.0, 0.0),
                &flat,
                cache.blocking(),
                &[],
                &[],
                1.0 / 60.0,
            );
        }
        predictor.position().z
    };

    let intact_z = predicted_z(0);
    let rubble_z = predicted_z(1);
    let gone_z = predicted_z(2);

    assert!(intact_z < 25.0, "intact building stops prediction at z={intact_z}");
    assert!(rubble_z < 25.0, "the low mound still stops a hull at z={rubble_z}");
    assert!(
        gone_z > intact_z + 10.0,
        "gone cover opens the route instead of reconciling later ({gone_z} vs {intact_z})"
    );
}

#[test]
fn both_reticle_traces_clear_the_low_rubble_and_gone_phases() {
    let flat = HeightMap::flat(80, 80, 5.0, -50.0).unwrap();
    let authored =
        [object("tenement", StaticCoverKind::CityBuilding, [40.0, 5.0, 75.0], [4.0, 5.0, 2.0])];
    let intact = LiveCoverCache::from_replicated(&authored, &[0]).unwrap();
    let rubble = LiveCoverCache::from_replicated(&authored, &[1]).unwrap();
    let gone = LiveCoverCache::from_replicated(&authored, &[2]).unwrap();
    let muzzle = Vec3::new(40.0, 3.0, 40.0);
    let aim = Vec3::new(40.0, 3.0, 140.0);
    let sight_hit = |cache: &LiveCoverCache| {
        crate::aim::aim_point_with_sweep(
            &flat,
            cache.blocking(),
            None,
            &[],
            TankId(1),
            TeamId(1),
            muzzle,
            Vec3::Z,
        )
    };

    assert!(sight_hit(&intact).z < 80.0);
    assert!(sight_hit(&rubble).z > 1000.0, "the sight ray passes over the low mound");
    assert!(sight_hit(&gone).z > 1000.0, "gone cover is absent from the sight ray");

    let spec = VehicleKind::T54_1951.spec();
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);
    let status = |cache: &LiveCoverCache| {
        crate::hud::reticle::reticle_report(crate::hud::reticle::ReticleFeedbackQuery {
            heightmap: &flat,
            cover: cache.blocking(),
            water: None,
            tanks: &[],
            player_spec: &spec,
            owner: TankId(1),
            owner_team: TeamId(1),
            muzzle,
            aim,
            gun_direction: game_core::math::gun_direction(0.0, pitch),
            muzzle_velocity_mps: 895.0,
            drag_per_s: 0.09,
        })
        .feedback
        .status
    };

    assert_eq!(status(&intact), crate::hud::reticle::ReticleStatus::Blocked);
    assert_eq!(status(&rubble), crate::hud::reticle::ReticleStatus::Clear);
    assert_eq!(status(&gone), crate::hud::reticle::ReticleStatus::Clear);
}
