use game_core::{ModuleSlot, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn replacing_tank_creates_new_vehicle_id_and_resets_live_state() {
    let mut state = SimulationState::new();
    let old_id = state.spawn_tank(TeamId(7), TankSpec::t54_1951(), Vec3::new(12.0, 0.0, 34.0));
    {
        let tank = state.tank_mut(old_id).expect("old tank");
        tank.yaw_rad = 0.75;
        tank.turret_yaw_rad = 0.4;
        tank.gun_pitch_rad = 0.2;
        tank.velocity_mps = Vec3::new(0.0, 0.0, 9.0);
        tank.hit_points = 12;
    }
    state.apply_commands(
        &[(old_id, TankCommand { fire: true, ..TankCommand::idle() })],
        FixedTimestep::from_hz(60),
    );
    assert!(state.shells().iter().any(|shell| shell.owner == old_id));
    let preserved_position = state.tank(old_id).expect("old tank").position;
    state.tank_mut(old_id).expect("old tank").modules.damage(ModuleSlot::Gun, u32::MAX);
    state.tank_mut(old_id).expect("old tank").reload_remaining_s = 4.0;

    let new_id = state
        .replace_tank_with_spec(old_id, TankSpec::jagdtiger())
        .expect("replacement should return a new id");

    assert_ne!(old_id, new_id);
    assert!(state.tank(old_id).is_none(), "old player tank should be removed");
    assert!(
        state.shells().iter().all(|shell| shell.owner != old_id),
        "old owned shells should be cleared on prototype vehicle swap"
    );

    let replacement = state.tank(new_id).expect("replacement tank");
    assert_eq!(replacement.team, TeamId(7));
    assert_eq!(replacement.spec.kind, VehicleKind::Jagdtiger);
    assert_eq!(replacement.position, preserved_position);
    assert_eq!(replacement.yaw_rad, 0.75);
    assert_eq!(replacement.turret_yaw_rad, 0.0);
    assert_eq!(replacement.gun_pitch_rad, 0.0);
    assert_eq!(replacement.velocity_mps, Vec3::ZERO);
    assert_eq!(replacement.hit_points, replacement.spec.hit_points);
    assert_eq!(replacement.reload_remaining_s, 0.0);
    assert_eq!(replacement.modules, replacement.spec.module_health);
}
