//! The snapshot's WIRE budget.
//!
//! `transport` caps one protocol message at `MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS` bytes and
//! refuses anything larger — it never truncates. That refusal reaches the host as a `send` error,
//! and a crew past the ceiling loses world state while its input ACKs and reliable combat lane
//! keep flowing: the battle looks frozen rather than disconnected.
//!
//! The snapshot used to grow monotonically with the shooting, because every tank's whole
//! `ArmorBreachSet` rode in every one. Measured at the sim's own `MAX_ARMOR_BREACHES`: 31 695 B
//! of a 32 200 B message, and the reachable case (one shot owning both an ingress and an egress
//! fragment) was 87 471 B — 2.7x a message the transport can carry at all.
//!
//! Protocol v39 moved perforations to the reliable lane, so what these tests lock is no longer a
//! ceiling to creep up on but a much stronger property: **a snapshot's size does not depend on
//! how much shooting has happened.**

use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorBreachSet, ArmorFrame, ArmorMaterial,
    ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, MAX_ARMOR_BREACHES, MAX_ARMOR_SCARS,
    MAX_BREACH_FRAGMENTS_PER_GROUP, ShellType, TRACK_HP_MAX, TankId, TeamId, VehicleKind,
};
use glam::Vec3;
use net::transport::{MAX_DATAGRAM_PAYLOAD, MAX_FRAGMENTS, fragment_message};
use net::{ProtocolMessage, ShellSnapshot, Snapshot, SnapshotDelivery, TankSnapshot, encode_frame};

/// A full 7v7.
const BATTLE_TANKS: u64 = 14;

fn lobe(index: u64, seed: u64) -> ApertureLobe {
    let entry = Vec3::new(index as f32 * 0.31 - 1.5, 0.9, 1.2);
    let normal = Vec3::new(0.0, 0.26, 0.966);
    ApertureLobe {
        entry_local: entry,
        exit_local: entry - normal * 0.1,
        entry_normal_local: normal,
        exit_normal_local: -normal,
        direction_local: -normal,
        thickness_m: 0.1,
        outer: BreachContour::new(0.05, 0.046, 0.7, 0.12),
        inner: BreachContour::new(0.07, 0.06, 0.87, 0.16),
        fracture_seed: seed,
    }
}

fn breach(group: u64, fragment: u64) -> ArmorBreach {
    let frame = match fragment % 3 {
        0 => ArmorFrame::Hull,
        1 => ArmorFrame::Turret,
        _ => ArmorFrame::Mantlet,
    };
    ArmorBreach::new(
        ArmorBreachDescriptor {
            breach_id: group,
            surface: ArmorSurfaceId::new(frame, ArmorZone::HullSide),
            frame,
            zone: ArmorZone::HullSide,
            material: ArmorMaterial::RolledSteel,
            face: BreachFace::Ingress,
            shell_type: ShellType::ArmorPiercing,
            created_tick: 4_000 + group,
            impact_angle_degrees: 22.0,
            impact_energy_kj: 1_400.0,
            projectile_diameter_m: 0.1,
            residual_penetration_mm: 55.0,
        },
        lobe(group * 4 + fragment, 0xA5C0 + group * 8 + fragment),
    )
}

fn breach_set(groups: u64, fragments_per_group: u64) -> ArmorBreachSet {
    let mut set = ArmorBreachSet::default();
    for group in 0..groups {
        for fragment in 0..fragments_per_group {
            set.add(breach(group, fragment));
        }
    }
    set
}

fn tank(id: u64, breaches: ArmorBreachSet) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(id),
        team: TeamId(1 + (id % 2) as u16),
        vehicle: VehicleKind::TigerII,
        position: [12.0 * id as f32, 1.0, 240.0],
        yaw_rad: 0.31,
        hull_pitch_rad: 0.02,
        hull_roll_rad: -0.01,
        turret_yaw_rad: 0.44,
        turret_yaw_velocity_rad_s: 0.1,
        gun_pitch_rad: 0.03,
        hit_points: 900,
        reload_remaining_s: 4.1,
        aim_dispersion_mrad: 2.6,
        module_hit_points: [200, 200, 200, 150, 225, 60],
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [TRACK_HP_MAX; 2],
        ammo_counts: [18, 7, 5],
        selected_ammo: 0,
        spotted_by_teams_mask: 0b11,
        armor_breaches: breaches,
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    }
}

/// A late-battle 7v7 delivery, dressed with `groups x fragments` perforations per tank plus the
/// world around them: the full crater ledger, shells in the air, third-party combat feedback and
/// an urban-scale cover list.
fn late_battle_delivery(groups: u64, fragments_per_group: u64) -> Vec<u8> {
    let snapshot = Snapshot {
        server_tick: 180_000,
        tanks: (0..BATTLE_TANKS)
            .map(|id| tank(id, breach_set(groups, fragments_per_group)))
            .collect(),
        shells: (0..8)
            .map(|i| ShellSnapshot {
                shell_id: game_core::ShellId(i),
                owner: TankId(i % BATTLE_TANKS),
                position: [i as f32 * 3.0, 6.0, 210.0],
                velocity_mps: [0.0, -3.0, 780.0],
                shell_type: ShellType::ArmorPiercing,
                caliber_mm: 88.0,
                drag_per_s: 0.09,
                age_seconds: 0.35,
            })
            .collect(),
        damage_events: vec![game_core::DamageEvent::default(); 6],
        shell_impacts: vec![game_core::ShellImpact::default(); 4],
        detached_turrets: vec![TankId(3), TankId(9)],
        cover_states: vec![0_u8; 160],
        craters: (0..sim::MAX_CRATERS)
            .map(|i| terrain::CraterRecord::from_world(i as f32 * 7.0, 300.0, 2.4, 0.6, 0))
            .collect(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    };
    encode_frame(&ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
        session_id: 0xD00D,
        snapshot,
        last_processed_input_seq: Some(91_234),
        local_motion: Default::default(),
    }))
    .expect("a snapshot always encodes")
}

/// The v39 property, and the reason the ceiling stopped being something to creep up on: how much
/// a battle has been shot at does not change what a snapshot costs.
#[test]
fn a_snapshots_size_does_not_depend_on_how_much_shooting_has_happened() {
    let pristine = late_battle_delivery(0, 0).len();
    let saturated =
        late_battle_delivery(MAX_ARMOR_BREACHES as u64, MAX_BREACH_FRAGMENTS_PER_GROUP as u64)
            .len();
    assert_eq!(
        pristine, saturated,
        "perforations are back on the snapshot: a pristine battle encodes to {pristine} B but a \
         saturated one to {saturated} B. They belong on the reliable lane — permanent, \
         append-only state must not be re-sent at snapshot cadence."
    );
}

/// And the absolute bound, with the headroom stated so the next person to add a field can see
/// what they are spending.
#[test]
fn a_full_7v7_snapshot_fits_one_transport_message_with_room_to_spare() {
    let frame =
        late_battle_delivery(MAX_ARMOR_BREACHES as u64, MAX_BREACH_FRAGMENTS_PER_GROUP as u64);
    let capacity = MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS;
    let fragments = frame.len().div_ceil(MAX_DATAGRAM_PAYLOAD);
    println!(
        "WIRE BUDGET: a saturated {BATTLE_TANKS}-tank snapshot is {} B of {capacity} B \
         ({fragments}/{MAX_FRAGMENTS} fragments, {} B spare, {:.0} KiB/s per client at 20 Hz)",
        frame.len(),
        capacity - frame.len(),
        frame.len() as f32 * 20.0 / 1024.0
    );
    assert!(
        fragment_message(1, &frame).is_ok(),
        "a saturated {BATTLE_TANKS}-tank snapshot is {} B ({fragments}/{MAX_FRAGMENTS} \
         fragments) but the transport holds {capacity} B; past this the host's send FAILS and \
         those crews stop receiving the world without the session ending.",
        frame.len()
    );
    // Three quarters of the message stays free. That is the standing budget: a new field is a
    // decision someone makes on purpose, not a surprise the transport reports for them.
    assert!(
        frame.len() * 4 <= capacity,
        "the snapshot now fills {} B of {capacity} B — over a quarter, which is where the old \
         growth-with-shooting problem started",
        frame.len()
    );
}

/// The transport must still REFUSE an oversized message rather than truncate it: a truncated
/// snapshot would decode as a plausible smaller world instead of an error the host can log.
#[test]
fn an_oversized_message_is_refused_not_silently_truncated() {
    let capacity = MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS;
    let oversized = vec![0_u8; capacity + 1];
    assert!(matches!(
        fragment_message(1, &oversized),
        Err(net::transport::TransportError::TooLarge { .. })
    ));
}

/// THE BATTLEFIELD'S MEMORY IS ONE BUDGET, and this holds the three shares comparable.
///
/// They were set one at a time and never put side by side. What they said together was:
///
///   * craters — 64 for the WHOLE battle, one map, fourteen tanks, seven minutes;
///   * armour scars — 24 PER TANK, so 336 across a 7v7;
///   * cover scars — 8 per object, up to roughly 800 on the urban map.
///
/// The ground, the surface a player looks at for the whole battle and the only record of where
/// the fighting actually happened, was budgeted at a twelfth of what tank paint got. That is why
/// the field never read as shelled — not because craters are expensive, but because nobody put
/// the three numbers on the same page.
///
/// The wire cost of the raised cap is already covered by
/// `a_full_7v7_snapshot_fits_one_transport_message_with_room_to_spare`, whose fixture fills the
/// crater ledger to `MAX_CRATERS`. What is NOT otherwise covered is the relationship, so it lives
/// here: the ground may not fall back to a fraction of what the vehicles standing on it get.
#[test]
fn the_ground_is_budgeted_on_a_par_with_the_vehicles_standing_on_it() {
    let armour_across_the_battle = MAX_ARMOR_SCARS * BATTLE_TANKS as usize;
    assert!(
        sim::MAX_CRATERS * 2 >= armour_across_the_battle,
        "the whole battlefield gets {} craters against {armour_across_the_battle} armour scars —          the ground is the most-looked-at surface in the game and cannot be an afterthought",
        sim::MAX_CRATERS
    );
    // And the ceiling is stated rather than discovered: the overlay files craters under a u16
    // bucket index, and a cap past that would deform the ground in the wrong place instead of
    // failing. `crater_ledger.rs` asserts it at compile time; this is the reader-facing half.
    assert!(sim::MAX_CRATERS <= u16::MAX as usize);
}
