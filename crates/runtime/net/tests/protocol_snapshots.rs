use game_core::{
    DamageCause, DamageEvent, ImpactSurface, MatchWeather, ModuleSlot, ShellImpact, TankId, TeamId,
    VehicleKind, WeatherVariant,
};
use glam::Vec3;
use net::{
    AuthoritativeMotion, ClientInputCommand, ClientVehicleSelection, PROTOCOL_VERSION,
    ProtocolMessage, ShellSnapshot, Snapshot, SnapshotDelivery, TankSnapshot, decode_message,
    encode_message,
};
use sim::TankCommand;
use terrain::MapId;

const SESSION_ID: u64 = 0x1020_3040_5060_7080;

#[test]
fn input_command_wire_snapshot_v34_is_stable() {
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

    assert_eq!(PROTOCOL_VERSION, 49);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "input_command_v33"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn vehicle_selection_wire_snapshot_v49_is_stable() {
    let message = ProtocolMessage::VehicleSelection(ClientVehicleSelection {
        session_id: 77,
        client_tick: 11,
        requested_vehicle: VehicleKind::PantherII,
    });

    let bytes = encode_message(&message).expect("vehicle selection should encode");

    assert_eq!(PROTOCOL_VERSION, 49);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "vehicle_selection_v49"));
    assert_eq!(decode_message(&bytes).expect("message should decode"), message);
}

#[test]
fn tank_snapshot_wire_v34_is_stable() {
    // Locks the v19 raw payload layout; transport framing is covered separately.
    let message = ProtocolMessage::Snapshot(tank_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 49);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_tank_v39"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

/// v39's whole point, stated as a wire property rather than a size.
///
/// Perforations were re-sent in full for every tank in every snapshot, which grew a battle's
/// payload monotonically with the shooting until the snapshot no longer fit one transport
/// message. They are permanent, append-only state, so they moved to the reliable lane. This
/// locks the two halves of that: a populated `armor_breaches` costs the snapshot NOTHING, and it
/// does not come back out — a client that expects the wire to carry it gets an empty set, which
/// is the loudest way to say "fill this from the lane".
#[test]
fn armor_breaches_are_client_local_and_never_cross_the_wire() {
    let bare = ProtocolMessage::Snapshot(combat_snapshot_message());
    let mut dressed = combat_snapshot_message();
    for tank in &mut dressed.tanks {
        tank.armor_breaches = sample_breach_set();
    }
    assert!(
        !sample_breach_set().breaches().is_empty(),
        "the fixture must actually carry perforations"
    );

    let bare_bytes = encode_message(&bare).expect("bare snapshot encodes");
    let dressed_bytes =
        encode_message(&ProtocolMessage::Snapshot(dressed)).expect("dressed snapshot encodes");

    assert_eq!(bare_bytes, dressed_bytes, "perforations must add no bytes to a snapshot");

    let ProtocolMessage::Snapshot(decoded) =
        decode_message(&dressed_bytes).expect("snapshot decodes")
    else {
        panic!("expected a snapshot");
    };
    for tank in &decoded.tanks {
        assert_eq!(
            tank.armor_breaches,
            Default::default(),
            "the wire hands back an empty set; the reliable lane fills it"
        );
    }
}

#[test]
fn combat_snapshot_wire_v34_is_stable() {
    let message = ProtocolMessage::Snapshot(combat_snapshot_message());

    let bytes = encode_message(&message).expect("snapshot should encode");

    assert_eq!(PROTOCOL_VERSION, 49);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_combat_v39"));
    assert_eq!(decode_message(&bytes).expect("snapshot should decode"), message);
}

#[test]
fn server_hello_wire_snapshot_v38_is_stable() {
    let message = ProtocolMessage::ServerHello {
        session_id: SESSION_ID,
        protocol_version: PROTOCOL_VERSION,
        map_id: MapId::ProkhorovkaHill252_2,
        // v23: lock one of the appended time-of-day variants into the fixture, so the new
        // discriminants cannot silently shift on the wire.
        weather: MatchWeather::new(WeatherVariant::GoldenEvening, 0x0123_4567_89AB_CDEF),
        map_content_hash: 0xFEED_FACE_CAFE_0001,
    };

    let bytes = encode_message(&message).expect("server hello should encode");

    assert_eq!(PROTOCOL_VERSION, 49);
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "server_hello_v39"));
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
            fuel_fire: true,
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    }
}

/// Non-empty combat snapshot used by the v18 fixture (and its generator): shells in flight, a
/// damage event, and an absorbed-shell impact.
#[test]
fn input_batch_disconnect_and_ack_wire_v38_are_stable() {
    let batch = ProtocolMessage::InputBatch {
        session_id: SESSION_ID,
        commands: vec![sample_input_command(), sample_input_command()],
    };
    let bytes = net::encode_frame(&batch).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "input_batch_v39"));

    let goodbye =
        ProtocolMessage::Disconnect { session_id: SESSION_ID, reason: net::DisconnectReason::Quit };
    let bytes = net::encode_frame(&goodbye).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "disconnect_v39"));

    let ack = ProtocolMessage::InputAck { session_id: SESSION_ID, last_processed_input_seq: 41 };
    let bytes = net::encode_frame(&ack).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "input_ack_v39"));
}

#[test]
fn snapshot_delivery_wire_v38_is_stable() {
    let delivery = SnapshotDelivery {
        session_id: SESSION_ID,
        snapshot: combat_snapshot_message(),
        last_processed_input_seq: Some(41),
        local_motion: AuthoritativeMotion {
            velocity_mps: [1.0, -0.5, 7.25],
            hull_yaw_velocity_rad_s: 0.33,
        },
    };
    let message = ProtocolMessage::SnapshotDelivery(delivery);
    let bytes = net::encode_frame(&message).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "snapshot_delivery_v39"));
    assert_eq!(net::decode_frame(&bytes).expect("decode"), message);
}

#[test]
fn reliable_combat_event_lane_wire_v38_is_stable() {
    let batch = ProtocolMessage::CombatEventBatch {
        session_id: SESSION_ID,
        events: vec![
            net::SequencedCombatEvent {
                delivery_seq: 12,
                event: net::CombatEvent::Damage(combat_snapshot_message().damage_events[0]),
            },
            net::SequencedCombatEvent {
                delivery_seq: 13,
                event: net::CombatEvent::ShellImpact(combat_snapshot_message().shell_impacts[0]),
            },
        ],
    };
    let bytes = net::encode_frame(&batch).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "combat_event_batch_v39"));
    assert_eq!(net::decode_frame(&bytes).expect("decode"), batch);

    let ack = ProtocolMessage::CombatEventAck { session_id: SESSION_ID, last_received_seq: 13 };
    let bytes = net::encode_frame(&ack).expect("encode");
    assert_eq!(hex(&bytes), wire_fixture(&bytes, "combat_event_ack_v39"));
    assert_eq!(net::decode_frame(&bytes).expect("decode"), ack);
}

/// The transport ships a snapshot in datagrams; a battle-worn 14-tank snapshot (live breaches on
/// every tank) must stay a HANDFUL of fragments. This is the size budget from the network plan:
/// if it ever exceeds four fragments, delta compression / interest management graduates from
/// plan B to a scheduled package.
#[test]
fn a_battle_worn_snapshot_fits_the_fragment_budget() {
    let mut snapshot = combat_snapshot_message();
    let worn = snapshot.tanks[0].clone();
    while snapshot.tanks.len() < 14 {
        let mut tank = worn.clone();
        tank.tank_id = game_core::TankId(100 + snapshot.tanks.len() as u64);
        snapshot.tanks.push(tank);
    }
    let delivery = SnapshotDelivery {
        session_id: SESSION_ID,
        snapshot,
        last_processed_input_seq: Some(99),
        local_motion: AuthoritativeMotion::default(),
    };
    let bytes = net::encode_frame(&ProtocolMessage::SnapshotDelivery(delivery)).expect("encode");
    let fragments = net::transport::fragment_message(1, &bytes).expect("fits the hard cap");
    assert!(
        fragments.len() <= 4,
        "a worn 14-tank snapshot must fit 4 fragments, took {} ({} bytes)",
        fragments.len(),
        bytes.len()
    );
}

fn sample_input_command() -> ClientInputCommand {
    ClientInputCommand {
        client_tick: 9,
        tank_id: TankId(42),
        command: TankCommand {
            throttle: 0.5,
            steer: -0.25,
            brake: 0.0,
            turret_yaw_delta: 0.1,
            gun_pitch_delta: 0.0,
            fire: true,
            select_ammo: None,
        },
    }
}

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
            // v39: client-local presentation state, deliberately absent from the wire. A
            // populated value here would not survive the round trip — which is exactly what
            // `armor_breaches_are_client_local_and_never_cross_the_wire` locks below.
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
        }],
        shells: vec![ShellSnapshot {
            shell_id: game_core::ShellId::from_shot(TankId(7), 3),
            owner: Some(TankId(7)),
            position: [0.0, 1.5, 12.0],
            velocity_mps: [0.0, 0.0, 900.0],
            shell_type: game_core::ShellType::Apcr,
            caliber_mm: 88.0,
            drag_per_s: 0.21,
            age_seconds: 0.35,
            // v47: the concrete round's identity — lock a non-default variant on the wire.
            round: Some(game_core::RoundId::Pzgr40_43),
        }],
        damage_events: vec![DamageEvent {
            event_id: game_core::BattleEventId(42),
            occurred_tick: 8,
            source: TankId(7),
            target: TankId(8),
            shell_id: Some(game_core::ShellId::from_shot(TankId(7), 3)),
            target_destroyed: true,
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
            // v47: the named round on the event, so the killfeed's word is locked on the wire.
            round: Some(game_core::RoundId::Pzgr40_43),
            ..Default::default()
        }],
        shell_impacts: vec![ShellImpact {
            event_id: game_core::BattleEventId(43),
            occurred_tick: 8,
            shell_id: game_core::ShellId::from_shot(TankId(7), 3),
            owner: Some(TankId(7)),
            position: Vec3::new(2.0, 0.1, 80.0),
            surface: ImpactSurface::Hull,
            // v17: the wire says WHAT died here — lock a non-default variant into the fixture.
            shell_type: game_core::ShellType::HighExplosive,
            direction: Vec3::new(0.6, -0.64, 0.48),
            caliber_mm: 122.0,
        }],
        // v20: a decapitated wreck on the wire, so the new detached-turret list is locked.
        detached_turrets: vec![TankId(8)],
        // v21: a rubble mound + a cleared object, so the new cover-state bytes are locked.
        cover_states: vec![1, 2],
        // v31: one quantized high-explosive crater, so the deformation ledger is locked on
        // the wire.
        craters: vec![terrain::CraterRecord {
            x_q: 404,
            z_q: 808,
            radius_q: 22,
            depth_q: 15,
            kind: terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        }],
        // v33: one HE bite on a wall face, so the cover-wound layout is locked on the wire.
        cover_scars: vec![terrain::CoverScar {
            cover: 3,
            face: 2,
            u_q: 120,
            v_q: 40,
            radius_q: 20,
            kind: terrain::COVER_SCAR_KIND_HIGH_EXPLOSIVE,
        }],
        // v41: the shot as an event, so the muzzle-flash cue no longer has to be inferred from a
        // reload clock jumping between snapshots.
        shots_fired: vec![game_core::ShotFired {
            shooter: TankId(7),
            shell_id: game_core::ShellId::from_shot(TankId(7), 0),
        }],
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

/// A run that regenerates the goldens must be RED, never green.
///
/// `REGEN_WIRE_FIXTURES=1` rewrites every `.hex` fixture to match whatever bytes the code emits
/// THIS run — so every `*_is_stable` assertion above then compares the fresh bytes against the
/// fixture it just wrote from those same bytes, and cannot fail. A regen run that reported green
/// would silently bless a protocol change: the whole point of these goldens, inverted. This guard
/// fails whenever the variable is set, so the intended workflow is the only green path — regen
/// (this test RED, fixtures rewritten), then rerun WITHOUT the variable (this test green, protocol
/// actually verified against the committed fixtures).
#[test]
fn a_regeneration_run_is_never_green() {
    assert!(
        std::env::var_os("REGEN_WIRE_FIXTURES").is_none(),
        "REGEN_WIRE_FIXTURES is set: the wire goldens were just rewritten from this run's own \
         bytes, so every stability test here is a tautology. Rerun the suite WITHOUT the variable \
         to verify the protocol against the committed fixtures."
    );
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
