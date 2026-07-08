use game_core::{TankId, TeamId, TrackDamageMask, VehicleKind};
use net::{
    ClientInputCommand, ProtocolMessage, ShellSnapshot, Snapshot, TankSnapshot, decode_message,
    encode_message,
};
use sim::TankCommand;

#[test]
fn input_command_round_trips_through_binary_protocol() {
    let message = ProtocolMessage::Input(ClientInputCommand {
        client_tick: 7,
        tank_id: TankId(42),
        command: TankCommand::drive(1.0, 0.25),
    });

    let bytes = encode_message(&message).expect("message should encode");
    let decoded = decode_message(&bytes).expect("message should decode");

    assert_eq!(decoded, message);
}

#[test]
fn decode_rejects_trailing_bytes() {
    let mut bytes = encode_message(&ProtocolMessage::Ping { client_time_us: 7 }).unwrap();
    bytes.push(0xAB);

    assert!(decode_message(&bytes).is_err(), "a trailing byte must not be silently dropped");
}

#[test]
fn decode_rejects_truncated_and_garbage_input() {
    let bytes = encode_message(&ProtocolMessage::Ping { client_time_us: 7 }).unwrap();
    assert!(decode_message(&bytes[..bytes.len() - 1]).is_err(), "truncated input must error");

    // Out-of-range enum discriminant / empty input must error, never panic.
    assert!(decode_message(&[0xFF, 0xFF, 0xFF, 0xFF]).is_err());
    assert!(decode_message(&[]).is_err());
}

#[test]
fn snapshot_round_trips_track_damage_mask() {
    let tank = TankSnapshot {
        tank_id: TankId(42),
        team: TeamId(1),
        vehicle: VehicleKind::T54_1951,
        position: [1.0, 2.0, 3.0],
        yaw_rad: 0.25,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.5,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: -0.04,
        hit_points: 800,
        reload_remaining_s: 1.25,
        aim_dispersion_mrad: 3.5,
        module_hit_points: VehicleKind::T54_1951.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: TrackDamageMask::LEFT.bits(),
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    };
    let message = ProtocolMessage::Snapshot(Snapshot {
        server_tick: 99,
        tanks: vec![tank],
        shells: vec![ShellSnapshot {
            owner: TankId(42),
            position: [0.0, 1.0, 2.0],
            velocity_mps: [0.0, 0.0, 900.0],
        }],
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
    });

    let bytes = encode_message(&message).expect("snapshot should encode");
    let decoded = decode_message(&bytes).expect("snapshot should decode");

    let ProtocolMessage::Snapshot(snapshot) = decoded else {
        panic!("expected snapshot");
    };
    assert_eq!(snapshot.tanks[0].track_damage_mask, TrackDamageMask::LEFT.bits());
}
