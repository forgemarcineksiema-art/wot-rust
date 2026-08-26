//! Connection bring-up and liveness over the unreliable transport. Exactly two things need
//! reliability in this protocol and both live here: the HELLO exchange (resend with backoff
//! until answered) and the heartbeat (Ping cadence; silence past the timeout is a dead peer).
//! Everything else — inputs, snapshots — is redundant by design and rides fire-and-forget.
//! The machine is driven by an explicit millisecond clock, never `Instant`, so every timing
//! path is a deterministic test.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::transport::{Reassembler, Transport, fragment_message};
use crate::{NetError, ProtocolMessage, decode_frame, encode_frame};

/// First resend after this long; doubles each attempt up to the cap.
const RESEND_BASE_MS: u64 = 250;
const RESEND_CAP_MS: u64 = 2_000;
/// A handshake or a connected peer that stays silent this long is gone.
pub const TIMEOUT_MS: u64 = 10_000;
/// Idle ping cadence once connected.
const HEARTBEAT_MS: u64 = 1_000;
static SESSION_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Connecting,
    Connected,
    /// Terminal: the server never answered, refused us, or went silent.
    Failed(SessionFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFailure {
    TimedOut,
    Refused,
    /// The host closed an already-finished battle. Kept distinct from refusal so the live
    /// client never calls an orderly match shutdown a broken connection.
    BattleOver,
    /// Reliable personal combat feedback could no longer be retained for this peer.
    CombatEventOverflow,
}

/// One peer's sequenced, fragmenting endpoint: every outgoing message gets a fresh sequence and
/// splits to datagrams; incoming datagrams reassemble into decoded protocol messages.
pub struct Endpoint {
    peer: SocketAddr,
    next_seq: u16,
    reassembler: Reassembler,
}

impl Endpoint {
    pub fn new(peer: SocketAddr) -> Self {
        Self { peer, next_seq: 0, reassembler: Reassembler::default() }
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn send(
        &mut self,
        transport: &mut dyn Transport,
        message: &ProtocolMessage,
    ) -> Result<(), NetError> {
        let frame = encode_frame(message)?;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        for datagram in
            fragment_message(seq, &frame).map_err(|error| NetError::Transport(error.to_string()))?
        {
            transport
                .send(self.peer, &datagram)
                .map_err(|error| NetError::Transport(error.to_string()))?;
        }
        Ok(())
    }

    /// Feed one raw datagram (already filtered to this peer by the caller); yields a decoded
    /// message when one completes. Malformed datagrams and stale-version frames are reported,
    /// not panicked over — a public port hears garbage.
    pub fn accept(&mut self, datagram: &[u8]) -> Result<Option<ProtocolMessage>, NetError> {
        let Some(frame) = self
            .reassembler
            .accept(datagram)
            .map_err(|error| NetError::Transport(error.to_string()))?
        else {
            return Ok(None);
        };
        decode_frame(&frame).map(Some)
    }
}

/// The client half of the handshake: resend `ClientHello` with backoff until `ServerHello`
/// arrives, then keep the line warm with pings and watch for silence.
pub struct ClientSession {
    pub endpoint: Endpoint,
    session_id: u64,
    state: SessionState,
    hello: ProtocolMessage,
    next_resend_ms: u64,
    resend_interval_ms: u64,
    started_ms: u64,
    last_heard_ms: u64,
    next_ping_ms: u64,
    /// The crew's garage pick, repeated alongside every hello resend and heartbeat until
    /// the caller clears it (on being seated). Unreliable-friendly by redundancy, like the
    /// input window: the server keeps newest-wins and ignores picks once the roster spawns.
    vehicle_selection: Option<game_core::VehicleKind>,
}

impl ClientSession {
    pub fn connect(server: SocketAddr, now_ms: u64) -> Self {
        Self::connect_with_session_id(server, now_ms, generate_session_id(now_ms))
    }

    /// Deterministic constructor for tests and callers that persist their own connection
    /// incarnation. Production callers normally use [`Self::connect`].
    pub fn connect_with_session_id(server: SocketAddr, now_ms: u64, session_id: u64) -> Self {
        Self {
            endpoint: Endpoint::new(server),
            session_id,
            state: SessionState::Connecting,
            hello: ProtocolMessage::ClientHello {
                session_id,
                protocol_version: crate::PROTOCOL_VERSION,
            },
            next_resend_ms: now_ms,
            resend_interval_ms: RESEND_BASE_MS,
            started_ms: now_ms,
            last_heard_ms: now_ms,
            next_ping_ms: now_ms + HEARTBEAT_MS,
            vehicle_selection: None,
        }
    }

    /// Ask the lobby to seat this crew in `kind`. Sent (and re-sent) until [`Self::
    /// clear_vehicle_selection`]; a pick that arrives after the battle starts is a no-op on
    /// the server, so there is no race to lose — only a benchmark hull to fall back to.
    pub fn request_vehicle(&mut self, kind: game_core::VehicleKind) {
        self.vehicle_selection = Some(kind);
    }

    /// Stop repeating the pick — the caller saw `StartBattle` and the seat is settled.
    pub fn clear_vehicle_selection(&mut self) {
        self.vehicle_selection = None;
    }

    fn selection_message(&self, now_ms: u64) -> Option<ProtocolMessage> {
        self.vehicle_selection.map(|requested_vehicle| {
            ProtocolMessage::VehicleSelection(crate::ClientVehicleSelection {
                session_id: self.session_id,
                client_tick: now_ms,
                requested_vehicle,
            })
        })
    }

    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// Drive timers and the wire once per frame/tick. Decoded non-handshake messages (snapshots,
    /// pongs…) are returned for the caller to consume.
    pub fn tick(
        &mut self,
        now_ms: u64,
        transport: &mut dyn Transport,
    ) -> Result<Vec<ProtocolMessage>, NetError> {
        let mut inbox = Vec::new();
        while let Some((from, datagram)) =
            transport.recv().map_err(|error| NetError::Transport(error.to_string()))?
        {
            if from != self.endpoint.peer() {
                continue;
            }
            match self.endpoint.accept(&datagram) {
                Ok(Some(message)) => {
                    if message.session_id() != Some(self.session_id) {
                        continue;
                    }
                    self.last_heard_ms = now_ms;
                    match message {
                        ProtocolMessage::ServerHello { .. } => {
                            if self.state == SessionState::Connecting {
                                self.state = SessionState::Connected;
                            }
                            inbox.push(message);
                        }
                        ProtocolMessage::Disconnect { reason, .. } => {
                            self.state = SessionState::Failed(match reason {
                                crate::DisconnectReason::BattleOver => SessionFailure::BattleOver,
                                crate::DisconnectReason::CombatEventOverflow => {
                                    SessionFailure::CombatEventOverflow
                                }
                                crate::DisconnectReason::Quit
                                | crate::DisconnectReason::Refused => SessionFailure::Refused,
                            });
                            inbox.push(message);
                        }
                        other => inbox.push(other),
                    }
                }
                Ok(None) => {}
                // Garbage from the network is a fact of public ports, not a crash.
                Err(_) => {}
            }
        }

        match self.state {
            SessionState::Connecting => {
                if now_ms.saturating_sub(self.started_ms) >= TIMEOUT_MS {
                    self.state = SessionState::Failed(SessionFailure::TimedOut);
                } else if now_ms >= self.next_resend_ms {
                    let hello = self.hello.clone();
                    self.endpoint.send(transport, &hello)?;
                    if let Some(selection) = self.selection_message(now_ms) {
                        self.endpoint.send(transport, &selection)?;
                    }
                    self.resend_interval_ms = (self.resend_interval_ms * 2).min(RESEND_CAP_MS);
                    self.next_resend_ms = now_ms + self.resend_interval_ms;
                }
            }
            SessionState::Connected => {
                if now_ms.saturating_sub(self.last_heard_ms) >= TIMEOUT_MS {
                    self.state = SessionState::Failed(SessionFailure::TimedOut);
                } else if now_ms >= self.next_ping_ms {
                    let ping = ProtocolMessage::Ping {
                        session_id: self.session_id,
                        client_time_us: now_ms * 1_000,
                    };
                    self.endpoint.send(transport, &ping)?;
                    if let Some(selection) = self.selection_message(now_ms) {
                        self.endpoint.send(transport, &selection)?;
                    }
                    self.next_ping_ms = now_ms + HEARTBEAT_MS;
                }
            }
            SessionState::Failed(_) => {}
        }
        Ok(inbox)
    }
}

fn generate_session_id(now_ms: u64) -> u64 {
    let wall_clock = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let folded_time = wall_clock as u64 ^ (wall_clock >> 64) as u64;
    let process = u64::from(std::process::id());
    let nonce = SESSION_NONCE.fetch_add(1, Ordering::Relaxed);
    game_core::math::splitmix64(
        folded_time
            ^ now_ms.rotate_left(17)
            ^ process.rotate_left(32)
            ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::LossyLoopback;

    fn server_addr() -> SocketAddr {
        "127.0.0.1:40000".parse().expect("addr")
    }

    /// A scripted server on the same lossy wire: answers every completed ClientHello.
    fn server_answers(wire: &mut LossyLoopback, endpoint: &mut Endpoint) {
        let mut pending = Vec::new();
        while let Some((_, datagram)) = wire.recv().expect("recv") {
            if let Ok(Some(message)) = endpoint.accept(&datagram) {
                pending.push(message);
            }
        }
        for message in pending {
            if let ProtocolMessage::ClientHello { session_id, .. } = message {
                let hello = ProtocolMessage::ServerHello {
                    session_id,
                    protocol_version: crate::PROTOCOL_VERSION,
                    map_id: Default::default(),
                    weather: Default::default(),
                    map_content_hash: 0,
                };
                endpoint.send(wire, &hello).expect("send");
            }
        }
    }

    #[test]
    fn the_handshake_completes_under_heavy_loss() {
        let mut wire = LossyLoopback::new(1234);
        wire.drop_percent = 30;
        let mut client = ClientSession::connect(server_addr(), 0);
        let mut server_side = Endpoint::new(server_addr());
        let mut connected_at = None;
        for step in 0..80_u64 {
            let now = step * 125;
            client.tick(now, &mut wire).expect("tick");
            server_answers(&mut wire, &mut server_side);
            if *client.state() == SessionState::Connected {
                connected_at = Some(now);
                break;
            }
        }
        assert!(connected_at.is_some(), "30% loss must not stop the hello exchange");
    }

    #[test]
    fn a_silent_server_times_out_instead_of_hanging() {
        let mut wire = LossyLoopback::new(7);
        wire.drop_percent = 100;
        let mut client = ClientSession::connect(server_addr(), 0);
        for step in 0..200_u64 {
            client.tick(step * 100, &mut wire).expect("tick");
        }
        assert_eq!(*client.state(), SessionState::Failed(SessionFailure::TimedOut));
    }

    #[test]
    fn a_connected_line_stays_warm_and_detects_silence() {
        let mut wire = LossyLoopback::new(9);
        let mut client = ClientSession::connect(server_addr(), 0);
        let mut server_side = Endpoint::new(server_addr());
        client.tick(0, &mut wire).expect("tick");
        server_answers(&mut wire, &mut server_side);
        client.tick(10, &mut wire).expect("tick");
        assert_eq!(*client.state(), SessionState::Connected);

        // The server falls silent; pings go unanswered; the line is declared dead.
        for step in 1..=120_u64 {
            client.tick(10 + step * 100, &mut wire).expect("tick");
            // Drain the wire so nothing echoes back.
            while wire.recv().expect("recv").is_some() {}
        }
        assert_eq!(*client.state(), SessionState::Failed(SessionFailure::TimedOut));
    }

    #[test]
    fn a_server_disconnect_is_terminal_and_reported() {
        let mut wire = LossyLoopback::new(11);
        let mut client = ClientSession::connect(server_addr(), 0);
        let mut server_side = Endpoint::new(server_addr());
        client.tick(0, &mut wire).expect("tick");
        server_answers(&mut wire, &mut server_side);
        client.tick(10, &mut wire).expect("tick");

        let goodbye = ProtocolMessage::Disconnect {
            session_id: client.session_id(),
            reason: crate::DisconnectReason::BattleOver,
        };
        server_side.send(&mut wire, &goodbye).expect("send");
        let inbox = client.tick(20, &mut wire).expect("tick");
        assert!(inbox.iter().any(|m| matches!(m, ProtocolMessage::Disconnect { .. })));
        assert_eq!(*client.state(), SessionState::Failed(SessionFailure::BattleOver));
    }

    #[test]
    fn stale_session_packets_are_rejected_before_liveness_or_state_changes() {
        let mut wire = LossyLoopback::new(13);
        let session_id = 0xCAFE_BABE_1234_5678;
        let mut client = ClientSession::connect_with_session_id(server_addr(), 0, session_id);
        let mut server_side = Endpoint::new(server_addr());
        client.tick(0, &mut wire).expect("tick");
        server_answers(&mut wire, &mut server_side);
        client.tick(10, &mut wire).expect("tick");
        assert_eq!(*client.state(), SessionState::Connected);

        let stale_goodbye = ProtocolMessage::Disconnect {
            session_id: session_id.wrapping_sub(1),
            reason: crate::DisconnectReason::Refused,
        };
        server_side.send(&mut wire, &stale_goodbye).expect("send");
        let inbox = client.tick(TIMEOUT_MS - 1, &mut wire).expect("tick");
        assert!(inbox.is_empty(), "old-incarnation traffic must not escape to the app");
        assert_eq!(*client.state(), SessionState::Connected);
        while wire.recv().expect("drain outgoing heartbeat").is_some() {}

        client.tick(TIMEOUT_MS + 10, &mut wire).expect("tick");
        assert_eq!(
            *client.state(),
            SessionState::Failed(SessionFailure::TimedOut),
            "the rejected packet must not refresh last_heard"
        );
    }

    #[test]
    fn deterministic_constructor_exposes_the_selected_session_id() {
        let session_id = 0x1020_3040_5060_7080;
        let client = ClientSession::connect_with_session_id(server_addr(), 7, session_id);
        assert_eq!(client.session_id(), session_id);
        assert_eq!(client.hello.session_id(), Some(session_id));
    }
}
