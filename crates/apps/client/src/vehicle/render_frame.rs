use engine::PresentationTank;
use game_core::TankId;
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{ArmorApertureRender, ArmorDamageInstance, RenderFrame, RenderObject};
use terrain::HeightMap;
use vehicle_geometry::{GearDynamics, RunningGearKinematics};

use super::asset_render::tank_vehicle_render_objects_posed;
use super::variation::VehicleVariation;
use crate::{VehicleAssetCatalog, VehicleMeshCatalog, tank_render_objects};

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleRenderFrame {
    pub objects: Vec<RenderObject>,
    pub armor_damage: Vec<ArmorDamageInstance>,
}

pub fn split_vehicle_render_frame(
    catalog: &mut VehicleMeshCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    for tank in tanks {
        let is_player = tank.id == player_tank;
        let hull_color = if is_player { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        let snapshot = render_snapshot(&tank);
        let mut tank_objects = tank_render_objects(catalog, &snapshot, hull_color);
        // The player's installed gun may have a longer/shorter barrel than the baked stock mesh;
        // stretch its gun submesh (index 2: [hull, turret, gun]) along the barrel axis to match the
        // muzzle the sim fires from. Enemies are always stock (scale 1.0).
        scale_player_gun(&mut tank_objects, is_player, player_gun_scale);
        objects.append(&mut tank_objects);
    }
    VehicleRenderFrame { objects, armor_damage: Vec::new() }
}

pub fn split_pbr_vehicle_render_frame(
    catalog: &mut VehicleAssetCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
) -> VehicleRenderFrame {
    split_pbr_vehicle_render_frame_on_terrain(catalog, tanks, player_tank, player_gun_scale, None)
}

/// As [`split_pbr_vehicle_render_frame`], with the local heightmap so each tank's road wheels can
/// ride the ground under them (per-wheel suspension travel) and its track tension can follow the
/// hull's drive state.
pub fn split_pbr_vehicle_render_frame_on_terrain(
    catalog: &mut VehicleAssetCatalog,
    tanks: Vec<PresentationTank>,
    player_tank: TankId,
    player_gun_scale: f32,
    terrain: Option<&HeightMap>,
) -> VehicleRenderFrame {
    let mut objects = Vec::new();
    let mut armor_damage = Vec::new();
    for tank in tanks {
        let is_player = tank.id == player_tank;
        let hull_color = if is_player { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        let snapshot = render_snapshot(&tank);
        if let Some(damage) = armor_damage_instance(&snapshot) {
            armor_damage.push(damage);
        }
        let variation = VehicleVariation::from_snapshot(&snapshot);
        let (left_travel, right_travel, wheel_count) = wheel_travel(&tank, terrain);
        // A driven track pulls its top run tight; braking or coasting lets it hang. The gain is
        // gentle: the P/v launch hits ~8 m/sÂ˛, and a sag that slams to its clamp on a throttle
        // tap reads as the track convulsing rather than tensioning. A hard landing (the sprung
        // hull dips below the replicated height) throws extra slack into both runs for a beat,
        // and a THROWN track loses its tension entirely â€” that side hangs deep and dead.
        let landing_slack = (-tank.attitude_heave_m).max(0.0) * 2.5;
        let sag_scale = (1.0 - tank.accel_long_mps2 * 0.05 + landing_slack).clamp(0.72, 1.5);
        let damage = game_core::TrackDamageMask::from_bits(tank.track_damage_mask);
        let side_sag = |broken: bool| if broken { 2.2 } else { sag_scale };
        let dynamics = GearDynamics {
            left_travel: &left_travel[..wheel_count],
            right_travel: &right_travel[..wheel_count],
            left_sag_scale: side_sag(damage.is_broken(game_core::TrackSide::Left)),
            right_sag_scale: side_sag(damage.is_broken(game_core::TrackSide::Right)),
            left_break_t: tank.track_break_t[0],
            right_break_t: tank.track_break_t[1],
        };
        let mut tank_objects = tank_vehicle_render_objects_posed(
            catalog,
            &snapshot,
            hull_color,
            &variation,
            tank.track_left_m,
            tank.track_right_m,
            [tank.attitude_pitch_rad, tank.attitude_roll_rad, tank.attitude_heave_m],
            dynamics,
        );
        recoil_gun(&mut tank_objects, tank.gun_recoil_m);
        scale_player_gun(&mut tank_objects, is_player, player_gun_scale);
        objects.append(&mut tank_objects);
    }
    VehicleRenderFrame { objects, armor_damage }
}

pub fn armor_damage_instance(snapshot: &TankSnapshot) -> Option<ArmorDamageInstance> {
    if snapshot.vehicle != game_core::VehicleKind::T54_1951 {
        return None;
    }
    let pose = super::pose::VehiclePose::from_snapshot(snapshot);
    let mut apertures = Vec::new();
    for breach in snapshot.armor_breaches.breaches() {
        for lobe in breach.lobes() {
            let (point, basis) = match breach.frame {
                game_core::ArmorFrame::Hull => {
                    (pose.hull_point(lobe.entry_local), pose.hull_basis())
                }
                game_core::ArmorFrame::Turret => {
                    (pose.turret_point(lobe.entry_local), pose.turret_basis())
                }
                game_core::ArmorFrame::Mantlet => {
                    (pose.gun_point(lobe.entry_local), pose.gun_basis())
                }
            };
            let (tangent, _) =
                game_core::armor_surface_basis(lobe.entry_normal_local, lobe.direction_local);
            let phase_a = hash_phase(lobe.fracture_seed);
            let phase_b = hash_phase(lobe.fracture_seed.rotate_left(29));
            apertures.push(ArmorApertureRender {
                center: point.to_array(),
                normal: (basis * lobe.entry_normal_local).normalize_or_zero().to_array(),
                tangent: (basis * tangent).normalize_or_zero().to_array(),
                major_radius_m: lobe.outer.major_radius_m,
                minor_radius_m: lobe.outer.minor_radius_m,
                rotation_rad: lobe.outer.rotation_rad,
                irregularity: lobe.outer.irregularity,
                phase_a,
                phase_b,
                half_depth_m: (lobe.thickness_m + 0.025).clamp(0.04, 0.45),
            });
        }
    }
    (!apertures.is_empty()).then_some(ArmorDamageInstance { tank_id: snapshot.tank_id, apertures })
}

fn hash_phase(seed: u64) -> f32 {
    let mixed = game_core::math::splitmix64(seed);
    ((mixed >> 40) as f32) / ((1_u64 << 24) as f32) * std::f32::consts::TAU
}

/// Slide the gun submesh back along its own barrel axis by the live recoil stroke. Applied
/// BEFORE the player barrel scale so the stroke stays in real meters â€” a long gun recoils the
/// same distance as its stock sibling, it does not stretch the recoil with the mesh.
fn recoil_gun(objects: &mut [RenderObject], recoil_m: f32) {
    if recoil_m > 1.0e-4
        && let Some(gun) = objects.get_mut(2)
    {
        let recoiled = Mat4::from_cols_array_2d(&gun.transform)
            * Mat4::from_translation(Vec3::new(0.0, 0.0, -recoil_m));
        gun.transform = recoiled.to_cols_array_2d();
    }
}

const MAX_ROAD_WHEELS: usize = 8;

/// Per-wheel vertical travel from the terrain under each road wheel: the residual between the
/// ground height at the wheel and the sprung hull's ground plane there. Wheels drop into dips and
/// ride over bumps the hull attitude has already averaged out.
fn wheel_travel(
    tank: &PresentationTank,
    terrain: Option<&HeightMap>,
) -> ([f32; MAX_ROAD_WHEELS], [f32; MAX_ROAD_WHEELS], usize) {
    let mut left = [0.0; MAX_ROAD_WHEELS];
    let mut right = [0.0; MAX_ROAD_WHEELS];
    let (Some(map), Some(kin)) = (terrain, RunningGearKinematics::for_vehicle(tank.vehicle)) else {
        return (left, right, 0);
    };
    let count = kin.wheel_zs.len().min(MAX_ROAD_WHEELS);
    let (sin, cos) = tank.hull_yaw_rad.sin_cos();
    let (bx, bz) = (tank.translation[0], tank.translation[2]);
    let hull_y = tank.translation[1];
    let (pitch, roll) = (tank.attitude_pitch_rad, tank.attitude_roll_rad);
    for (index, &wz) in kin.wheel_zs.iter().take(count).enumerate() {
        for (lane, side) in [(&mut left, -1.0_f32), (&mut right, 1.0)] {
            let lx = side * kin.wheel_x;
            // Hull-local (lx, wz) into the world; the sprung ground plane tilts with the attitude.
            let wx = bx + cos * lx + sin * wz;
            let wzw = bz - sin * lx + cos * wz;
            let plane = hull_y + pitch.tan() * wz + roll.tan() * lx;
            let ground = map.sample_height(wx, wzw).unwrap_or(plane);
            lane[index] = (ground - plane).clamp(-0.08, 0.20);
        }
    }
    (left, right, count)
}

fn scale_player_gun(objects: &mut [RenderObject], is_player: bool, player_gun_scale: f32) {
    if is_player
        && (player_gun_scale - 1.0).abs() > 1.0e-3
        && let Some(gun) = objects.get_mut(2)
    {
        let scaled = Mat4::from_cols_array_2d(&gun.transform)
            * Mat4::from_scale(Vec3::new(1.0, 1.0, player_gun_scale));
        gun.transform = scaled.to_cols_array_2d();
    }
}

/// Adapt a presentation entity into the pose-only `TankSnapshot` the procedural mesh kernels
/// consume. The fields the meshes never read (`reload_remaining_s`, `aim_dispersion_mrad`) are
/// zeroed â€” they belong to the player's HUD path, not vehicle geometry.
pub(crate) fn render_snapshot(tank: &PresentationTank) -> TankSnapshot {
    TankSnapshot {
        tank_id: tank.id,
        team: tank.team,
        vehicle: tank.vehicle,
        position: tank.translation,
        yaw_rad: tank.hull_yaw_rad,
        // The PRESENTATION attitude (authoritative base + weight-transfer spring): the pose
        // built from this snapshot tilts exactly like the sprung hull the player sees.
        hull_pitch_rad: tank.attitude_pitch_rad,
        hull_roll_rad: tank.attitude_roll_rad,
        turret_yaw_rad: tank.turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: tank.gun_pitch_rad,
        hit_points: tank.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: tank.module_hit_points,
        destroyed_modules_mask: tank.destroyed_modules_mask,
        track_damage_mask: tank.track_damage_mask,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        // Ammo is HUD state, not geometry: the mesh kernels never read it (like reload above).
        ammo_counts: [0; game_core::MAX_AMMO_SLOTS],
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: tank.armor_breaches.clone(),
        track_break_t: tank.track_break_t,
        engine_fire: tank.engine_fire,
    }
}

pub fn render_frame_from_objects(objects: Vec<RenderObject>) -> RenderFrame {
    RenderFrame { objects, ..RenderFrame::default() }
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;

    use super::*;

    /// A full 7v7 (14 tanks) of the instance-heaviest playable vehicle must fit the renderer's
    /// vehicle instance buffer. When it does not, `set_vehicle_render_frame` truncates tanks off
    /// the frame — and before it truncated, the whole oversized upload was silently dropped,
    /// freezing every vehicle on screen the moment enough tanks were spotted. This locks the
    /// budget against roster growth and running-gear detail growth alike.
    #[test]
    fn worst_case_7v7_battle_fits_the_vehicle_instance_budget() {
        let per_tank_worst = VehicleKind::PLAYABLE
            .iter()
            .map(|&kind| {
                let gear = RunningGearKinematics::for_vehicle(kind).map_or(0, |kin| {
                    vehicle_geometry::running_gear_placements_dynamic(
                        &kin,
                        0.0,
                        0.0,
                        GearDynamics::default(),
                    )
                    .len()
                });
                3 + gear // hull + turret + gun + every animated gear part
            })
            .max()
            .expect("playable roster is non-empty");

        let battle_worst = 14 * per_tank_worst;
        assert!(
            battle_worst <= renderer_wgpu::vehicle_instance_budget(),
            "a 14-tank battle can submit {battle_worst} vehicle instances but the renderer budget \
             holds {}; grow VEHICLE_INSTANCE_CAPACITY before shipping this roster/gear detail",
            renderer_wgpu::vehicle_instance_budget(),
        );
    }

    #[test]
    fn render_snapshot_carries_every_pose_field_the_meshes_read() {
        let tank = PresentationTank {
            id: TankId(7),
            team: game_core::TeamId(2),
            vehicle: VehicleKind::TigerII,
            translation: [1.0, 2.0, 3.0],
            hull_yaw_rad: 0.4,
            turret_yaw_rad: 0.5,
            gun_pitch_rad: -0.1,
            hit_points: 1200,
            destroyed_modules_mask: 0b101,
            spotted_by_teams_mask: 0,
            module_hit_points: [11, 22, 33, 44, 55, 66],
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        };

        let snapshot = render_snapshot(&tank);

        assert_eq!(snapshot.tank_id, TankId(7));
        assert_eq!(snapshot.vehicle, VehicleKind::TigerII);
        assert_eq!(snapshot.position, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.yaw_rad, 0.4);
        assert_eq!(snapshot.turret_yaw_rad, 0.5);
        assert_eq!(snapshot.gun_pitch_rad, -0.1);
        assert_eq!(snapshot.destroyed_modules_mask, 0b101);
        // Live module HP rides through so partial damage can drive mesh-side cues.
        assert_eq!(snapshot.module_hit_points, [11, 22, 33, 44, 55, 66]);
    }
}
