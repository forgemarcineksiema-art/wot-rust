//! The snapshot's WIRE budget.
//!
//! `transport` caps one protocol message at `MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS` bytes and
//! refuses anything larger — it never truncates. That refusal reaches the host as a `send`
//! error, and a host that drops it leaves a crew with no world state at all while its input
//! ACKs and reliable combat events keep arriving, so the battle looks frozen rather than
//! disconnected. Nothing measured how close a real battle sits to that ceiling.
//!
//! The dominant term is per-tank `ArmorBreachSet`: persistent perforations are re-sent WHOLE in
//! every snapshot, so the payload grows monotonically with the shooting. These tests put the
//! ceiling in code as a ratchet — raising it is progress, lowering it is a regression that costs
//! a player their world.

use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorBreachSet, ArmorFrame, ArmorMaterial,
    ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, MAX_APERTURE_LOBES, MAX_ARMOR_BREACHES,
    MAX_BREACH_FRAGMENTS_PER_GROUP, ShellType, TRACK_HP_MAX, TankId, TeamId, VehicleKind,
};
use glam::Vec3;
use net::transport::{MAX_DATAGRAM_PAYLOAD, MAX_FRAGMENTS, fragment_message};
use net::{ProtocolMessage, ShellSnapshot, Snapshot, SnapshotDelivery, TankSnapshot, encode_frame};

/// A full 7v7.
const BATTLE_TANKS: u64 = 14;

/// How many single-fragment breach groups per tank a full 7v7 snapshot must still carry inside
/// one transport message, with the rest of a late-battle world (crater ledger, shells in flight,
/// third-party events, an urban cover list) riding along.
///
/// FLOOR, measured: 12 — which is exactly `MAX_ARMOR_BREACHES`, so the wire holds the sim's
/// group cap only while every group stays a SINGLE fragment with a SINGLE lobe. See the debt
/// recorded by `the_wire_cannot_carry_the_simulations_true_worst_case` below.
const MIN_BREACH_GROUPS_ON_THE_WIRE: u64 = 12;

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
        // Groups are spread far apart so none of them merges into a neighbour.
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

/// A late-battle 7v7 delivery: every tank carries `groups x fragments x lobes` perforations, and
/// the world around them carries the full crater ledger, shells in the air, third-party combat
/// feedback and an urban-scale cover list.
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
    };
    encode_frame(&ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
        session_id: 0xD00D,
        snapshot,
        last_processed_input_seq: Some(91_234),
        local_motion: Default::default(),
    }))
    .expect("a snapshot always encodes")
}

fn fits_one_message(frame: &[u8]) -> bool {
    fragment_message(1, frame).is_ok()
}

/// The ratchet. Lowering the number of perforations a battle can replicate is not a cosmetic
/// regression: past the ceiling the host's snapshot send FAILS and that crew stops receiving
/// the world while everything else about its session keeps working.
#[test]
fn a_full_7v7_snapshot_carries_the_simulations_breach_group_cap() {
    let frame = late_battle_delivery(MIN_BREACH_GROUPS_ON_THE_WIRE, 1);
    let fragments = frame.len().div_ceil(MAX_DATAGRAM_PAYLOAD);
    let capacity = MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS;
    assert!(
        fits_one_message(&frame),
        "a {BATTLE_TANKS}-tank battle with {MIN_BREACH_GROUPS_ON_THE_WIRE} perforations per tank \
         encodes to {} B ({fragments}/{MAX_FRAGMENTS} fragments) but the transport holds only \
         {capacity} B. The host's `SnapshotDelivery` send now FAILS for every crew: the battle \
         freezes without disconnecting. Shrink the snapshot (the per-tank `ArmorBreachSet` is \
         the dominant term and is re-sent whole every tick) rather than raising MAX_FRAGMENTS — \
         at 20 Hz this payload is already {:.0} KiB/s per client.",
        frame.len(),
        frame.len() as f32 * 20.0 / 1024.0
    );
}

/// Where the ceiling actually is, and how little room is left under it. Sweeping rather than
/// asserting one size keeps the number honest and prints it for the next person to change the
/// protocol.
#[test]
fn the_snapshot_ceiling_is_measured_not_assumed() {
    let mut ceiling = 0;
    for groups in 0..=MAX_ARMOR_BREACHES as u64 {
        if fits_one_message(&late_battle_delivery(groups, 1)) {
            ceiling = groups;
        } else {
            break;
        }
    }
    let at_ceiling = late_battle_delivery(ceiling, 1);
    let capacity = MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS;
    println!(
        "WIRE BUDGET: {ceiling} single-fragment perforations per tank fit a {BATTLE_TANKS}-tank \
         snapshot ({} B of {capacity} B, {} B spare, {:.0} KiB/s per client at 20 Hz)",
        at_ceiling.len(),
        capacity - at_ceiling.len(),
        at_ceiling.len() as f32 * 20.0 / 1024.0
    );
    assert!(
        ceiling >= MIN_BREACH_GROUPS_ON_THE_WIRE,
        "the wire now carries only {ceiling} perforations per tank, below the locked floor of \
         {MIN_BREACH_GROUPS_ON_THE_WIRE}; something added to the snapshot without paying for it"
    );
}

/// The recorded DEBT, in the shape the art-direction program uses: what today achieves, against
/// what the simulation can actually produce. `sim::breach_space` creates ingress AND egress
/// fragments for one shot, and merges nearby hits into extra lobes, so the true worst case is
/// `MAX_ARMOR_BREACHES x MAX_BREACH_FRAGMENTS_PER_GROUP x MAX_APERTURE_LOBES` per tank — far
/// above what one message holds.
///
/// This test does NOT assert the worst case fits (it does not). It asserts the failure is LOUD:
/// the transport must refuse an oversized message rather than truncate it, because a truncated
/// snapshot would decode as a plausible smaller world instead of an error the host can log.
#[test]
fn the_wire_cannot_carry_the_simulations_true_worst_case() {
    let worst = late_battle_delivery(MAX_ARMOR_BREACHES as u64, 3);
    let capacity = MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS;
    let fragments = worst.len().div_ceil(MAX_DATAGRAM_PAYLOAD);
    println!(
        "WIRE DEBT: the sim's reachable worst case ({MAX_ARMOR_BREACHES} groups x 3 fragments) \
         encodes to {} B = {fragments}/{MAX_FRAGMENTS} fragments, {:.1}x the {capacity} B \
         transport message. Full cap is {MAX_ARMOR_BREACHES}x{MAX_BREACH_FRAGMENTS_PER_GROUP}x\
         {MAX_APERTURE_LOBES} lobes per tank. Closing it means taking persistent breaches off \
         the newest-wins snapshot (they are append-only per tank — the v38 reliable lane already \
         carries exactly that shape of state).",
        worst.len(),
        worst.len() as f32 / capacity as f32
    );
    match fragment_message(1, &worst) {
        Ok(datagrams) => {
            // If a future change makes it fit, that is progress — but it must really fit.
            let rebuilt: usize = datagrams.iter().map(|d| d.len() - 8).sum();
            assert_eq!(rebuilt, worst.len(), "fragmentation must never lose bytes");
        }
        Err(error) => assert!(
            matches!(error, net::transport::TransportError::TooLarge { .. }),
            "an oversized snapshot must be REFUSED, never silently truncated: {error}"
        ),
    }
}
