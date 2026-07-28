//! Perforations converge over the reliable lane, not the snapshot (protocol v39).
//!
//! They used to be re-sent in full for every tank in every snapshot, which grew a battle's wire
//! cost monotonically with the shooting until the snapshot no longer fit one transport message —
//! at which point the host's send failed and that crew silently stopped receiving the world.
//!
//! What replaces it has to earn the same guarantee the snapshot gave for free: that a client
//! ends up holding exactly the set the authoritative simulation holds, including for a crew that
//! joined after the shooting started. That is what these tests are.

use game_core::{ArmorBreachSet, TankId, TankSpec, TeamId};
use glam::Vec3;
use net::CombatEvent;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fire() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

/// Run a duel until the target has taken `wanted` perforations, returning the authoritative sim
/// and every breach record it emitted along the way, in order.
fn shoot_until_breached(
    wanted: usize,
) -> (SimulationState, TankId, Vec<sim::event_stamp::ArmorBreachRecord>) {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 30.0));
    state.tank_mut(target).expect("target").yaw_rad = std::f32::consts::PI;
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.012;
    let step = FixedTimestep::from_hz(60);

    let mut stream = Vec::new();
    let mut shot = 0;
    for tick in 0..4_000 {
        // Keep the target alive so the duel can carve several holes, and slide it sideways
        // between shots so the holes land APART: hits close enough to overlap merge into one
        // aperture group by design, and a fixture that only ever merged would not exercise the
        // multi-group replication this test is about.
        if let Some(tank) = state.tank_mut(target) {
            tank.hit_points = tank.hit_points.max(500);
        }
        let commands: Vec<(TankId, TankCommand)> = if tick % 40 == 0 {
            const PITCHES: [f32; 4] = [-0.012, -0.006, -0.017, -0.009];
            if let Some(tank) = state.tank_mut(shooter) {
                tank.gun_pitch_rad = PITCHES[shot % PITCHES.len()];
            }
            shot += 1;
            vec![(shooter, fire())]
        } else {
            Vec::new()
        };
        state.apply_commands(&commands, step);
        stream.extend(state.armor_breach_events().iter().cloned());
        if state.tank(target).expect("target").armor_breaches.breaches().len() >= wanted {
            break;
        }
    }
    (state, target, stream)
}

/// The convergence claim, end to end: a client that applies the replicated stream through the
/// same `ArmorBreachSet::add` holds bit-for-bit what the server holds.
#[test]
fn replaying_the_replicated_stream_reproduces_the_authoritative_set() {
    let (state, target, stream) = shoot_until_breached(3);
    let authoritative = state.tank(target).expect("target").armor_breaches.clone();
    assert!(
        authoritative.breaches().len() >= 3,
        "the fixture duel must actually carve several perforations, got {}",
        authoritative.breaches().len()
    );

    let mut client = engine::ArmorBreachStore::default();
    for record in &stream {
        client.apply(record.tank, record.breach.clone());
    }

    assert_eq!(
        client.get(target),
        authoritative,
        "a client replaying the stream must hold the authoritative set"
    );
}

/// The snapshot must no longer be the carrier — otherwise the wire cost this change exists to
/// remove is still there, just paid twice.
#[test]
fn the_snapshot_carries_no_perforations_at_all() {
    let (state, target, stream) = shoot_until_breached(3);
    assert!(!stream.is_empty(), "the duel emitted a stream");

    let snapshot = net::Snapshot::from(&state);
    let tank = snapshot.tanks.iter().find(|tank| tank.tank_id == target).expect("target present");
    assert_eq!(
        tank.armor_breaches,
        ArmorBreachSet::default(),
        "perforations left the snapshot; the reliable lane carries them"
    );
}

/// A crew that joins after the shooting started gets the world's existing perforations as a
/// SEED, then the ongoing stream — and lands on the same set as everyone else. Without the seed
/// the additions have no baseline and the late joiner would see undamaged steel forever.
#[test]
fn a_late_joiner_seeded_with_the_current_state_converges_too() {
    let (state, target, stream) = shoot_until_breached(3);
    let authoritative = state.tank(target).expect("target").armor_breaches.clone();

    // A crew present from the first shot.
    let mut early = engine::ArmorBreachStore::default();
    for record in &stream {
        early.apply(record.tank, record.breach.clone());
    }

    // A crew that arrives now: the seed is every hull's complete set, delivered as the same kind
    // of additions the stream carries.
    let mut late = engine::ArmorBreachStore::default();
    for record in state.armor_breach_state() {
        late.apply(record.tank, record.breach);
    }

    assert_eq!(late.get(target), authoritative, "the seed alone reproduces the set");
    assert_eq!(late.get(target), early.get(target), "both crews see the same steel");
}

/// The seed is what sizes the reliable lane, so its worst case must fit the queue rather than
/// disconnect the very crew it exists to serve.
#[test]
fn the_join_seed_fits_the_reliable_lane() {
    let worst_case_records =
        14 * game_core::MAX_ARMOR_BREACHES * game_core::MAX_BREACH_FRAGMENTS_PER_GROUP;
    assert!(
        worst_case_records <= server::MAX_PENDING_COMBAT_EVENTS,
        "a full 7v7's perforation seed is {worst_case_records} events but the lane holds {}; \
         raise the capacity rather than refusing a legitimate late joiner",
        server::MAX_PENDING_COMBAT_EVENTS
    );
}

/// One perforation must fit one datagram: the lane sends the largest prefix that fits, and an
/// event too large for a datagram on its own cannot make progress at all — it fails the session.
#[test]
fn one_perforation_fits_one_datagram() {
    let (state, target, _) = shoot_until_breached(1);
    let breach = state
        .tank(target)
        .expect("target")
        .armor_breaches
        .breaches()
        .first()
        .expect("a perforation")
        .clone();
    let message = net::ProtocolMessage::CombatEventBatch {
        session_id: 1,
        events: vec![net::SequencedCombatEvent {
            delivery_seq: 0,
            event: CombatEvent::ArmorBreach(net::ArmorBreachDelta { tank: target, breach }),
        }],
    };
    let encoded = net::encode_frame(&message).expect("encodes");
    assert!(
        encoded.len() <= net::transport::MAX_DATAGRAM_PAYLOAD,
        "a single perforation encodes to {} B, above the {} B datagram the lane sends",
        encoded.len(),
        net::transport::MAX_DATAGRAM_PAYLOAD
    );
}
