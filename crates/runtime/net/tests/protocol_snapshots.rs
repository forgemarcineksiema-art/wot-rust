use game_core::{
    DamageCause, DamageEvent, ImpactSurface, ModuleSlot, ShellImpact, TankId, TeamId, VehicleKind,
    WeatherVariant,
};
use glam::Vec3;
use net::{
    ClientInputCommand, ClientVehicleSelection, PROTOCOL_VERSION, ProtocolMessage, ShellSnapshot,
    Snapshot, TankSnapshot, decode_message, encode_message,
};
use sim::TankCommand;
use terrain::MapId;

#[test]
fn input_command_wire_snapshot_v26_is_stable() {
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

    assert_eq!(PROTOCOL_VERSION, 26);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "input_command_v26"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn vehicle_selection_wire_snapshot_v26_is_stable() {
    let message = ProtocolMessage::VehicleSelection(ClientVehicleSelection {
        client_tick: 11,
        requested_vehicle: VehicleKind::PantherII,
    });

    let bytes = encode_message(&message).expect("vehicle selection should encode");

    assert_eq!(PROTOCOL_VERSION, 26);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "vehicle_selection_v26"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn tank_snapshot_wire_v26_is_stable() {
    // Locks the v19 raw payload layout; transport framing is covered separately.
    let message = ProtocolMessage::Snapshot(tank_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 26);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_tank_v26"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

#[test]
fn combat_snapshot_wire_v26_is_stable() {
    let message = ProtocolMessage::Snapshot(combat_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 26);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_combat_v26"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

#[test]
fn server_hello_wire_snapshot_v26_is_stable() {
    let message = ProtocolMessage::ServerHello {
        protocol_version: PROTOCOL_VERSION,
        map_id: MapId::ProkhorovkaHill252_2,
        // v23: lock one of the appended time-of-day variants into the fixture, so the new
        // discriminants cannot silently shift on the wire.
        weather_variant: WeatherVariant::GoldenEvening,
    };

    let bytes = encode_message(&message).expect("server hello should encode");

    assert_eq!(PROTOCOL_VERSION, 26);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "server_hello_v26"));
    assert_eq!(decode_message(&bytes).expect("server hello should decode"), message);
}

/// Baseline tank snapshot used by the v18 fixture (and its generator).
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
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
    }
}

/// Non-empty combat snapshot used by the v18 fixture (and its generator): shells in flight, a
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
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: sample_breach_set(),
            track_break_t: [None, None],
            engine_fire: false,
        }],
        shells: vec![ShellSnapshot {
            shell_id: game_core::ShellId::from_shot(TankId(7), 3),
            owner: TankId(7),
            position: [0.0, 1.5, 12.0],
            velocity_mps: [0.0, 0.0, 900.0],
            shell_type: game_core::ShellType::Apcr,
            caliber_mm: 88.0,
            drag_per_s: 0.21,
            age_seconds: 0.35,
        }],
        damage_events: vec![DamageEvent {
            source: TankId(7),
            target: TankId(8),
            hit_position: Vec3::new(0.0, 1.2, 55.0),
            damage_hp: 320,
            penetrated: true,
            cause: DamageCause::Shell,
            module: Some(ModuleSlot::Gun),
            // v19: lock a non-default struck-plate normal and shell heading into the fixture so
            // the new wire fields cannot silently regress.
            plate_normal: Vec3::new(0.0, 0.0, -1.0),
            shell_direction: Vec3::new(0.0, 0.0, 1.0),
            // v22: a thrown-left-track hit, so the new track-feedback field is locked on the wire.
            track_hit: Some(game_core::TrackHit { side: game_core::TrackSide::Left, broke: true }),
            ..Default::default()
        }],
        shell_impacts: vec![ShellImpact {
            owner: TankId(7),
            position: Vec3::new(2.0, 0.1, 80.0),
            surface: ImpactSurface::Hull,
            // v17: the wire says WHAT died here â€” lock a non-default variant into the fixture.
            shell_type: game_core::ShellType::HighExplosive,
        }],
        // v20: a decapitated wreck on the wire, so the new detached-turret list is locked.
        detached_turrets: vec![TankId(8)],
        // v21: a rubble mound + a cleared object, so the new cover-state bytes are locked.
        cover_states: vec![1, 2],
    }
}

fn sample_breach_set() -> game_core::ArmorBreachSet {
    let mut set = game_core::ArmorBreachSet::default();
    let frame = game_core::ArmorFrame::Turret;
    let zone = game_core::ArmorZone::TurretFront;
    let lobe = game_core::ApertureLobe {
        entry_local: Vec3::new(0.2, 1.8, 0.9),
        exit_local: Vec3::new(0.2, 1.78, 0.68),
        entry_normal_local: Vec3::new(0.0, 0.1, 0.995).normalize(),
        exit_normal_local: Vec3::new(0.0, -0.1, -0.995).normalize(),
        direction_local: Vec3::NEG_Z,
        thickness_m: 0.22,
        outer: game_core::BreachContour::new(0.07, 0.052, 0.4, 0.12),
        inner: game_core::BreachContour::new(0.105, 0.08, 0.57, 0.16),
        fracture_seed: 0x1234_5678_9abc_def0,
    };
    set.add(game_core::ArmorBreach::new(
        game_core::ArmorBreachDescriptor {
            breach_id: 0x1020_3040_5060_7080,
            surface: game_core::ArmorSurfaceId::new(frame, zone),
            frame,
            zone,
            material: game_core::ArmorMaterial::CastSteel,
            face: game_core::BreachFace::Ingress,
            shell_type: game_core::ShellType::Apcr,
            created_tick: 321,
            impact_angle_degrees: 28.0,
            impact_energy_kj: 1_420.0,
            projectile_diameter_m: 0.1,
            residual_penetration_mm: 96.0,
        },
        lobe,
    ));
    set
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
