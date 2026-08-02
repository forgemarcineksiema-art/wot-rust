//! THE lock for "impact marks touch the tank": a battery of real shell traces from many
//! directions, each turned into the `DamageEvent` the server would send, must produce a decal
//! whose seat lies on the target's VISUAL mesh — within 1.5 cm — not floating on the inset
//! collision volume. If this regresses, marks are hovering off the armor again.

use game_core::math::HullPose;
use game_core::{ArmorZone, DamageCause, DamageEvent, TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use sim::{SegmentImpact, ShellTraceWorld, TraceTank, segment_impact};
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{MeshContactIndex, SubmeshKind};

use crate::fx::decal_from_damage_event;
use crate::vehicle::asset_catalog::VehicleContactIndex;
use crate::vehicle::variation::DecalFrame;

const KIND: VehicleKind = VehicleKind::BENCHMARK;

/// The target snapshot: a T-54 at the origin, level, turret and gun neutral — so its hull frame is
/// the identity and its authoring coordinates are world coordinates, matching the trace tank.
fn target_snapshot() -> TankSnapshot {
    let spec = KIND.spec();
    TankSnapshot {
        tank_id: TankId(9),
        team: TeamId(2),
        vehicle: KIND,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 1.0,
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

fn contact_index() -> VehicleContactIndex {
    let baked = authoritative_baked_vehicle(KIND).expect("bake T-54");
    let turret_ring = baked.mounts().turret_ring.translation;
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh");
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret submesh");
    let gun = baked.submesh(SubmeshKind::Gun).expect("gun submesh");
    let trunnion = baked.mounts().gun_trunnion.translation;
    VehicleContactIndex {
        hull: MeshContactIndex::from_mesh(&hull.mesh, Vec3::ZERO),
        turret: MeshContactIndex::from_mesh(&turret.mesh, turret_ring),
        gun: MeshContactIndex::from_mesh(&gun.mesh, trunnion),
    }
}

/// The real trace of one straight shot, as the `DamageEvent` the server emits.
fn trace_event(from: Vec3, to: Vec3, spec: &TankSpec) -> Option<DamageEvent> {
    let tank = TraceTank::from_spec(TankId(9), Vec3::ZERO, HullPose::level(0.0), 0.0, spec);
    let tanks = [tank];
    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &tanks,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    };
    match segment_impact(from, to, to - from, &world) {
        Some(SegmentImpact::Tank { zone, hit_position, plate_normal, facing, .. }) => {
            Some(DamageEvent {
                source: TankId(1),
                target: TankId(9),
                hit_position,
                damage_hp: 100,
                penetrated: true,
                cause: DamageCause::Shell,
                armor_zone: zone,
                armor_facing: facing,
                plate_normal,
                shell_direction: (to - from).normalize(),
                ..Default::default()
            })
        }
        _ => None,
    }
}

/// Distance from a seated decal to the vehicle's visual mesh, in the decal's own frame.
fn distance_to_visual_mesh(
    decal_frame: DecalFrame,
    local: Vec3,
    contact: &VehicleContactIndex,
) -> f32 {
    let index = match decal_frame {
        DecalFrame::Hull => &contact.hull,
        DecalFrame::Turret => &contact.turret,
        DecalFrame::Mantlet => &contact.gun,
    };
    index
        .nearest_point(local, 1.0)
        .map(|surface| surface.position.distance(local))
        .unwrap_or(f32::INFINITY)
}

#[test]
fn every_real_hit_seats_its_decal_on_the_visual_mesh() {
    let spec = KIND.spec();
    let contact = contact_index();
    let snapshot = target_snapshot();

    let mut seated = 0;
    let mut worst = 0.0_f32;
    // Sweep the tank from all around at hull and turret heights, flat and gently plunging.
    for azimuth_step in 0..24 {
        let azimuth = azimuth_step as f32 / 24.0 * std::f32::consts::TAU;
        let (sin, cos) = azimuth.sin_cos();
        for &(height, drop) in &[(0.9_f32, 0.0_f32), (1.7, 0.0), (1.3, 0.35)] {
            let outward = Vec3::new(sin, drop, cos).normalize();
            let aim = Vec3::new(0.0, height, 0.0);
            let from = aim + outward * 9.0;
            let Some(event) = trace_event(from, aim, &spec) else {
                continue;
            };
            let decal = decal_from_damage_event(&event, &snapshot, Some(&contact))
                .expect("a real hit resolves to a decal");
            let local = Vec3::from_array(decal.local_position);
            let distance = distance_to_visual_mesh(decal.frame, local, &contact);
            assert!(
                distance <= 0.015,
                "decal seats {distance:.4} m off the visual mesh (zone {:?}, frame {:?})",
                event.armor_zone,
                decal.frame
            );
            worst = worst.max(distance);
            seated += 1;
        }
    }
    assert!(seated > 30, "the sweep should land many real hits, got {seated}");
    println!("seated {seated} decals, worst {worst:.4} m off the mesh");
}

/// A grazing event whose shell line misses the inset mesh must still resolve to a finite decal
/// through the nearest-point snap or the transmitted-normal fallback — never a NaN, never a panic.
#[test]
fn a_grazing_miss_still_resolves_to_a_finite_decal() {
    let contact = contact_index();
    let snapshot = target_snapshot();
    // A hand-built event a metre out in front with a shell heading that never meets the mesh.
    let event = DamageEvent {
        source: TankId(1),
        target: TankId(9),
        hit_position: Vec3::new(0.0, 1.2, 4.0),
        damage_hp: 50,
        penetrated: false,
        ricocheted: true,
        cause: DamageCause::Shell,
        armor_zone: ArmorZone::UpperGlacis,
        plate_normal: Vec3::new(0.0, 0.3, 0.95).normalize(),
        shell_direction: Vec3::new(1.0, 0.0, 0.0),
        ..Default::default()
    };
    let decal = decal_from_damage_event(&event, &snapshot, Some(&contact))
        .expect("a finite decal even on a grazing miss");
    assert!(Vec3::from_array(decal.local_position).is_finite());
    assert!(Vec3::from_array(decal.local_normal).is_finite());
}
