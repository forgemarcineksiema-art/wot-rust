use net::TankSnapshot;
use renderer_api::SceneVertex;

use super::geometry_mesh::append_baked_tank_mesh;

/// Append a fully baked tank mesh in world space.
pub fn append_tank_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) {
    assert!(
        append_baked_tank_mesh(vertices, indices, snapshot, hull_color),
        "{:?} must have complete baked vehicle geometry",
        snapshot.vehicle
    );
}

/// Build a single tank mesh for offscreen examples and geometry smoke tests.
pub fn tank_scene_mesh(snapshot: &TankSnapshot) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_tank_mesh(&mut vertices, &mut indices, snapshot, [0.32, 0.40, 0.30]);
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{TankId, VehicleKind};

    #[test]
    fn builds_finite_baked_geometry_for_every_vehicle() {
        for kind in VehicleKind::ALL {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            append_tank_mesh(&mut vertices, &mut indices, &snapshot(kind, 0.2), [0.4, 0.4, 0.4]);
            assert!(!vertices.is_empty() && !indices.is_empty(), "{kind:?} produced no geometry");
            assert!(
                vertices.iter().all(|v| v.position.iter().all(|c| c.is_finite())),
                "{kind:?} produced non-finite vertices"
            );
        }
    }

    fn snapshot(kind: VehicleKind, turret_yaw_rad: f32) -> TankSnapshot {
        TankSnapshot {
            tank_id: TankId(1),
            team: game_core::TeamId(1),
            vehicle: kind,
            position: [0.0, 0.0, 0.0],
            yaw_rad: 0.3,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.1,
            hit_points: 100,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: kind.spec().gun.dispersion_mrad,
            module_hit_points: kind.spec().module_health.hit_points_by_slot(),
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
            hull_pitch_velocity_rad_s: 0.0,
            hull_roll_velocity_rad_s: 0.0,
        }
    }
}
