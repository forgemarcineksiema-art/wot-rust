//! PBR vehicle render-object assembly: turning a cached [`VehicleAssetCatalog`] entry plus a pose
//! into the per-submesh and running-gear render objects. Split from `asset_catalog` to keep each
//! module small.

use game_core::{ModuleSlot, TankId};
use glam::Mat4;
use net::TankSnapshot;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::{GearDynamics, RunningGearKinematics};

use super::asset_catalog::VehicleAssetCatalog;
use super::pose::VehiclePose;
use super::running_gear_objects::gear_render_objects;
use super::variation::VehicleVariation;

/// Presentation-only barrel droop for a knocked-out gun: the elevation gear is dead, so the
/// barrel sags off the aim. ~4°. Gameplay/aiming is untouched — the server still owns the gun.
const GUN_DEAD_DROOP_RAD: f32 = 0.075;
/// A wreck's gun hangs further, all support gone. ~7°.
const WRECK_GUN_DROOP_RAD: f32 = 0.12;

pub fn tank_vehicle_render_objects(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_with_variation(
        catalog,
        snapshot,
        hull_color,
        &VehicleVariation::from_snapshot(snapshot),
    )
}

/// As [`tank_vehicle_render_objects`], but with an explicit runtime variation state (camo, dirt,
/// snow, broken tracks, destroyed modules). The base render path uses the snapshot-derived
/// variation; callers carrying richer per-tank cosmetic state pass it here.
pub fn tank_vehicle_render_objects_with_variation(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_with_tracks(catalog, snapshot, hull_color, variation, 0.0, 0.0)
}

/// As [`tank_vehicle_render_objects_with_variation`], but also instances the animatable running gear
/// (spinning wheels, scrolling track links) from the per-side track distance the presentation world
/// accumulates. Vehicles without blueprint gear ignore the distances and render unchanged.
pub fn tank_vehicle_render_objects_with_tracks(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
    track_left_m: f32,
    track_right_m: f32,
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_posed(
        catalog,
        snapshot,
        hull_color,
        variation,
        track_left_m,
        track_right_m,
        [0.0; 3],
        GearDynamics {
            left_break_t: snapshot.track_break_t[0],
            right_break_t: snapshot.track_break_t[1],
            ..Default::default()
        },
        // Every caller of this wrapper is looking AT the vehicle — the garage, the probes, the
        // review harnesses. The battle path calls the posed form below with the real range.
        vehicle_geometry::GearDetail::Near,
    )
}

/// As [`tank_vehicle_render_objects_with_tracks`], with the sprung-hull attitude
/// `[pitch, roll, heave]` from the presentation world folded into the hull frame.
#[allow(clippy::too_many_arguments)]
pub fn tank_vehicle_render_objects_posed(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
    track_left_m: f32,
    track_right_m: f32,
    attitude: [f32; 3],
    gear_dynamics: GearDynamics<'_>,
    gear_detail: vehicle_geometry::GearDetail,
) -> Vec<RenderObject> {
    catalog.integrate_one_damage_mesh();
    // The interior's Damaged/Burning variants key the per-instance skin: which module slots are
    // hurt (scorched paint) or gone (charred black), straight from the replicated module state.
    // `spec_ref`: this runs per tank per FRAME, and it reads one array.
    let full_hp = snapshot.vehicle.spec_ref().module_health.hit_points_by_slot();
    let mut damaged_modules = 0_u8;
    for (index, live) in snapshot.module_hit_points.iter().enumerate() {
        if game_core::module_condition(*live, full_hp[index]) == game_core::ModuleCondition::Damaged
        {
            damaged_modules |= 1 << index;
        }
    }
    let destroyed_modules = snapshot.destroyed_modules_mask;
    let entry = catalog.vehicle_entry(snapshot.vehicle).expect("vehicle must have baked geometry");
    let hull_mesh = catalog
        .damaged_frame_mesh(
            snapshot.vehicle,
            snapshot.tank_id,
            game_core::ArmorFrame::Hull,
            &snapshot.armor_breaches,
            damaged_modules,
            destroyed_modules,
        )
        .unwrap_or(entry.hull);
    let turret_mesh = catalog
        .damaged_frame_mesh(
            snapshot.vehicle,
            snapshot.tank_id,
            game_core::ArmorFrame::Turret,
            &snapshot.armor_breaches,
            damaged_modules,
            destroyed_modules,
        )
        .unwrap_or(entry.turret);
    let gun_mesh = catalog
        .damaged_frame_mesh(
            snapshot.vehicle,
            snapshot.tank_id,
            game_core::ArmorFrame::Mantlet,
            &snapshot.armor_breaches,
            damaged_modules,
            destroyed_modules,
        )
        .unwrap_or(entry.gun);
    let pose = VehiclePose::new_with_attitude(
        snapshot.vehicle,
        glam::Vec3::from_array(snapshot.position),
        snapshot.yaw_rad,
        snapshot.turret_yaw_rad,
        snapshot.gun_pitch_rad,
        attitude,
    );
    let hull_transform =
        Mat4::from_translation(pose.hull_translation()) * Mat4::from_mat3(pose.hull_basis());
    let turret_transform =
        Mat4::from_translation(pose.turret_translation()) * Mat4::from_mat3(pose.turret_basis());
    let gun_transform =
        Mat4::from_translation(pose.gun_translation()) * Mat4::from_mat3(pose.gun_basis());

    // The cosmetic overlays (camo/dirt/snow) recolour the whole vehicle. A LIVE tank never
    // recolours with damage — wounds speak locally (hit decals, smoke), not through a palette
    // swap. Only a knocked-out hull takes the whole-vehicle burnt-out tint.
    let surface = variation.surface_tint(hull_color);
    let tint =
        if snapshot.hit_points == 0 { VehicleVariation::wreck_tint(surface) } else { surface };

    // A dead gun (or a whole wreck) sags off the aim: the barrel is authored around the trunnion,
    // so a rotation about the gun frame's local X pitches it about the trunnion like a real droop.
    let gun_transform = match gun_droop_rad(snapshot, variation) {
        droop if droop > 1.0e-4 => gun_transform * Mat4::from_rotation_x(droop),
        _ => gun_transform,
    };

    let mut objects = vec![
        vehicle_render_object(snapshot.tank_id, hull_mesh, entry.material, hull_transform, tint),
        vehicle_render_object(
            snapshot.tank_id,
            turret_mesh,
            entry.material,
            turret_transform,
            tint,
        ),
        vehicle_render_object(snapshot.tank_id, gun_mesh, entry.material, gun_transform, tint),
    ];

    // Running gear rides the hull and tints with the suspension; the wheels spin and the links
    // scroll by the per-side track distance.
    if let Some(handles) = entry.running_gear
        && let Some(kin) = RunningGearKinematics::for_vehicle(snapshot.vehicle)
    {
        let track_left_m = if variation.left_track_broken() { 0.0 } else { track_left_m };
        let track_right_m = if variation.right_track_broken() { 0.0 } else { track_right_m };
        objects.extend(gear_render_objects(
            snapshot.tank_id,
            handles,
            entry.material,
            hull_transform,
            &kin,
            track_left_m,
            track_right_m,
            gear_dynamics,
            tint,
            gear_detail,
        ));
    }

    objects
}

/// How far the displayed barrel droops below the aim: a knocked-out gun sags, a wreck sags more.
/// A live tank with a healthy gun returns zero (no droop). Positive pitches the muzzle down about
/// the trunnion; `+Z` is forward, so `Rx(+droop)` lowers it (see the trunnion-pivoted gun mesh).
fn gun_droop_rad(snapshot: &TankSnapshot, variation: &VehicleVariation) -> f32 {
    if snapshot.hit_points == 0 {
        WRECK_GUN_DROOP_RAD
    } else if variation.module_destroyed(ModuleSlot::Gun) {
        GUN_DEAD_DROOP_RAD
    } else {
        0.0
    }
}

fn vehicle_render_object(
    tank_id: TankId,
    mesh: MeshHandle,
    material: MaterialHandle,
    transform: Mat4,
    tint: [f32; 3],
) -> RenderObject {
    RenderObject {
        tank_id: Some(tank_id),
        mesh,
        material,
        transform: transform.to_cols_array_2d(),
        tint,
    }
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;
    use glam::Vec3;

    use super::*;

    fn snapshot(kind: VehicleKind, destroyed_modules_mask: u8, hit_points: u32) -> TankSnapshot {
        let spec = kind.spec();
        TankSnapshot {
            tank_id: TankId(1),
            team: game_core::TeamId(1),
            vehicle: kind,
            position: [0.0, 0.0, 0.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask,
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

    /// World height of the muzzle end of the posed gun submesh (index 2). The gun mesh is authored
    /// around the trunnion, so a point a couple of metres down its local +Z axis rides the barrel.
    fn muzzle_height(objects: &[RenderObject]) -> f32 {
        let gun = Mat4::from_cols_array_2d(&objects[crate::VEHICLE_GUN_OBJECT].transform);
        gun.transform_point3(Vec3::new(0.0, 0.0, 2.5)).y
    }

    #[test]
    fn a_dead_gun_and_a_wreck_droop_the_barrel_below_the_live_aim() {
        let kind = VehicleKind::T54_1951;
        let mut catalog = VehicleAssetCatalog::default();
        let color = [0.34, 0.42, 0.30];

        let healthy = tank_vehicle_render_objects(&mut catalog, &snapshot(kind, 0, 1500), color);
        let gun_bit = ModuleSlot::Gun.destroyed_mask_bit();
        let gun_dead =
            tank_vehicle_render_objects(&mut catalog, &snapshot(kind, gun_bit, 1500), color);
        let wreck = tank_vehicle_render_objects(&mut catalog, &snapshot(kind, gun_bit, 0), color);

        let (live, dead, dead_wreck) =
            (muzzle_height(&healthy), muzzle_height(&gun_dead), muzzle_height(&wreck));
        assert!(dead < live - 0.05, "a dead gun visibly droops: {dead} vs live {live}");
        assert!(
            dead_wreck < dead - 0.02,
            "a wreck's gun hangs further than a merely-dead gun: {dead_wreck} vs {dead}"
        );
    }

    #[test]
    fn a_healthy_live_gun_does_not_droop() {
        let kind = VehicleKind::T54_1951;
        let mut catalog = VehicleAssetCatalog::default();
        let objects =
            tank_vehicle_render_objects(&mut catalog, &snapshot(kind, 0, 1500), [0.3, 0.4, 0.3]);
        // Index 2 is the gun; with a healthy gun its transform carries no droop rotation, so the
        // muzzle stays at (or above) the trunnion height for a level gun.
        assert_eq!(gun_droop_rad(&snapshot(kind, 0, 1500), &VehicleVariation::default()), 0.0);
        assert!(muzzle_height(&objects).is_finite());
    }
}
