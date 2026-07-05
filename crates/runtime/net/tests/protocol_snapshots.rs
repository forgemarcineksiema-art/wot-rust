use game_core::{
    DamageCause, DamageEvent, ImpactSurface, ModuleSlot, ShellImpact, TankId, TeamId, VehicleKind,
};
use glam::Vec3;
use net::{
    ClientInputCommand, ClientVehicleSelection, PROTOCOL_VERSION, ProtocolMessage, ShellSnapshot,
    Snapshot, TankSnapshot, decode_message, encode_message,
};
use sim::TankCommand;

#[test]
fn input_command_wire_snapshot_v16_is_stable() {
    let message = ProtocolMessage::Input(ClientInputCommand {
        client_tick: 7,
        tank_id: TankId(42),
        command: TankCommand {
            throttle: 1.0,
            steer: 0.25,
            brake: 0.75,
            turret_yaw_delta: 0.5,
            gun_pitch_delta: 0.125,
            fire: false,
            select_ammo: Some(1),
        },
    });

    let bytes = encode_message(&message).expect("message should encode");

    assert_eq!(PROTOCOL_VERSION, 16);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "input_command_v16"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn vehicle_selection_wire_snapshot_v16_is_stable() {
    let message = ProtocolMessage::VehicleSelection(ClientVehicleSelection {
        client_tick: 11,
        requested_vehicle: VehicleKind::PantherII,
    });

    let bytes = encode_message(&message).expect("vehicle selection should encode");

    assert_eq!(PROTOCOL_VERSION, 16);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "vehicle_selection_v16"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn tank_snapshot_wire_v16_is_stable() {
    // Locks the v16 raw payload layout; transport framing is covered separately.
    let message = ProtocolMessage::Snapshot(tank_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 16);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_tank_v16"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

#[test]
fn combat_snapshot_wire_v16_is_stable() {
    let message = ProtocolMessage::Snapshot(combat_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 16);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_combat_v16"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

/// Baseline tank snapshot used by the v16 fixture (and its generator).
pub fn tank_snapshot_message() -> Snapshot {
    Snapshot {
        server_tick: 5,
        tanks: vec![TankSnapshot {
            tank_id: TankId(7),
            team: TeamId(2),
            vehicle: VehicleKind::Jagdtiger,
            position: [1.0, 2.0, 3.0],
            yaw_rad: 0.5,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.25,
            turret_yaw_velocity_rad_s: 0.12,
            gun_pitch_rad: 0.1,
            hit_points: 1_500,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 5.5,
            module_hit_points: VehicleKind::Jagdtiger.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 1 << 3,
            track_damage_mask: 0,
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
    }
}

/// Non-empty combat snapshot used by the v16 fixture (and its generator): shells in flight, a
/// damage event, and an absorbed-shell impact.
pub fn combat_snapshot_message() -> Snapshot {
    Snapshot {
        server_tick: 9,
        tanks: vec![TankSnapshot {
            tank_id: TankId(7),
            team: TeamId(1),
            vehicle: VehicleKind::TigerII,
            position: [3.0, 0.5, 12.0],
            yaw_rad: 0.2,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: -0.1,
            turret_yaw_velocity_rad_s: -0.08,
            gun_pitch_rad: 0.05,
            hit_points: 2_050,
            reload_remaining_s: 1.5,
            aim_dispersion_mrad: 7.25,
            module_hit_points: VehicleKind::TigerII.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 1 << 3,
            track_damage_mask: 0,
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
        }],
        shells: vec![ShellSnapshot {
            owner: TankId(7),
            position: [0.0, 1.5, 12.0],
            velocity_mps: [0.0, 0.0, 900.0],
        }],
        damage_events: vec![DamageEvent {
            source: TankId(7),
            target: TankId(8),
            hit_position: Vec3::new(0.0, 1.2, 55.0),
            damage_hp: 320,
            penetrated: true,
            cause: DamageCause::Shell,
            module: Some(ModuleSlot::Gun),
            ..Default::default()
        }],
        shell_impacts: vec![ShellImpact {
            owner: TankId(7),
            position: Vec3::new(2.0, 0.1, 80.0),
            surface: ImpactSurface::Hull,
        }],
    }
}

/// The golden wire fixture for `name`. Set `REGEN_WIRE_FIXTURES=1` while running these tests to
/// rewrite the fixtures after a deliberate protocol bump, then rerun clean to verify.
fn wire_fixture(bytes: &[u8], name: &str) -> String {
    let path = format!("{}/tests/snapshots/{name}.hex", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("REGEN_WIRE_FIXTURES").is_some() {
        std::fs::write(&path, hex(bytes)).expect("fixture should be writable");
    }
    std::fs::read_to_string(&path).expect("wire fixture should exist").trim().to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join("")
}
