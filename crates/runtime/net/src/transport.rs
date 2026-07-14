//! Datagram transport under the protocol frames: plain UDP plus the little reliability the game
//! actually needs. Snapshots are full state every 50 ms and inputs are sent redundantly, so a
//! lost datagram is answered by the next one — no retransmission, no streams, no async runtime.
//! What this layer adds is (1) fragmentation, because a v27 snapshot with live breaches can
//! exceed one MTU, and (2) sequencing metadata so a receiver can drop stale fragments. The
//! [`Transport`] trait keeps the wire swappable: real sockets in the game, a deterministic
//! [`LossyLoopback`] in tests (drop/duplicate/reorder with a seed, no flaky network), and a
//! Steam Datagram Relay implementation later without touching callers.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};

/// Practical safe payload per datagram: conservative Ethernet MTU minus IP/UDP/our header.
pub const MAX_DATAGRAM_PAYLOAD: usize = 1_150;
/// One message may span at most this many fragments (a hair under 32 KiB of frame bytes).
pub const MAX_FRAGMENTS: usize = 28;
/// Reassembly keeps at most this many in-flight messages per peer; older sequences are dropped.
const REASSEMBLY_WINDOW: usize = 8;

pub const DATAGRAM_MAGIC: [u8; 2] = *b"WD";
pub const DATAGRAM_HEADER_LEN: usize = 8;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum TransportError {
    #[error("message of {len} bytes exceeds {max} fragments")]
    TooLarge { len: usize, max: usize },
    #[error("datagram too short: {len}")]
    DatagramTooShort { len: usize },
    #[error("bad datagram magic")]
    BadMagic,
    #[error("socket error: {0}")]
    Socket(String),
}

/// Where datagrams travel. `send`/`recv` move whole datagrams (header + payload) — the
/// fragmentation above them never sees the medium.
pub trait Transport {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) -> Result<(), TransportError>;
    /// Non-blocking: `None` when nothing is waiting.
    fn recv(&mut self) -> Result<Option<(SocketAddr, Vec<u8>)>, TransportError>;
}

/// Split one encoded protocol frame into sequenced datagrams.
pub fn fragment_message(seq: u16, frame: &[u8]) -> Result<Vec<Vec<u8>>, TransportError> {
    let count = frame.len().div_ceil(MAX_DATAGRAM_PAYLOAD).max(1);
    if count > MAX_FRAGMENTS {
        return Err(TransportError::TooLarge { len: frame.len(), max: MAX_FRAGMENTS });
    }
    Ok((0..count)
        .map(|index| {
            let chunk = &frame[index * MAX_DATAGRAM_PAYLOAD
                ..(((index + 1) * MAX_DATAGRAM_PAYLOAD).min(frame.len()))];
            let mut datagram = Vec::with_capacity(DATAGRAM_HEADER_LEN + chunk.len());
            datagram.extend_from_slice(&DATAGRAM_MAGIC);
            datagram.extend_from_slice(&seq.to_le_bytes());
            datagram.push(index as u8);
            datagram.push(count as u8);
            datagram.extend_from_slice(&[0, 0]); // reserved (future ack lane)
            datagram.extend_from_slice(chunk);
            datagram
        })
        .collect())
}

struct Pending {
    parts: Vec<Option<Vec<u8>>>,
    received: usize,
}

/// Per-peer reassembler. An incomplete message is simply abandoned when it falls out of the
/// window — the next snapshot supersedes it, which is the whole design.
#[derive(Default)]
pub struct Reassembler {
    pending: HashMap<u16, Pending>,
    /// Highest sequence seen, for staleness ordering (wrapping compare).
    latest_seq: Option<u16>,
}

fn seq_newer(a: u16, b: u16) -> bool {
    a.wrapping_sub(b) < u16::MAX / 2 && a != b
}

impl Reassembler {
    /// Feed one datagram; returns the full frame bytes when a message completes.
    pub fn accept(&mut self, datagram: &[u8]) -> Result<Option<Vec<u8>>, TransportError> {
        if datagram.len() < DATAGRAM_HEADER_LEN {
            return Err(TransportError::DatagramTooShort { len: datagram.len() });
        }
        if datagram[..2] != DATAGRAM_MAGIC {
            return Err(TransportError::BadMagic);
        }
        let seq = u16::from_le_bytes([datagram[2], datagram[3]]);
        let index = datagram[4] as usize;
        let count = (datagram[5] as usize).clamp(1, MAX_FRAGMENTS);
        if index >= count {
            return Ok(None);
        }
        if self.latest_seq.is_none_or(|latest| seq_newer(seq, latest)) {
            self.latest_seq = Some(seq);
        }
        let payload = datagram[DATAGRAM_HEADER_LEN..].to_vec();
        let pending = self
            .pending
            .entry(seq)
            .or_insert_with(|| Pending { parts: vec![None; count], received: 0 });
        if pending.parts.len() != count || pending.parts[index].is_some() {
            return Ok(None); // duplicate or inconsistent — the next message supersedes.
        }
        pending.parts[index] = Some(payload);
        pending.received += 1;
        if pending.received == count {
            let pending = self.pending.remove(&seq).expect("just present");
            self.evict_stale();
            let mut frame = Vec::new();
            for part in pending.parts {
                frame.extend_from_slice(&part.expect("all received"));
            }
            return Ok(Some(frame));
        }
        self.evict_stale();
        Ok(None)
    }

    fn evict_stale(&mut self) {
        if self.pending.len() <= REASSEMBLY_WINDOW {
            return;
        }
        let Some(latest) = self.latest_seq else { return };
        let mut seqs: Vec<u16> = self.pending.keys().copied().collect();
        seqs.sort_by_key(|seq| latest.wrapping_sub(*seq));
        for seq in seqs.into_iter().skip(REASSEMBLY_WINDOW) {
            self.pending.remove(&seq);
        }
    }
}

/// The real wire: one non-blocking UDP socket, polled once per tick/frame.
pub struct UdpTransport {
    socket: UdpSocket,
    buffer: Vec<u8>,
}

impl UdpTransport {
    pub fn bind(addr: SocketAddr) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(addr).map_err(|e| TransportError::Socket(e.to_string()))?;
        socket.set_nonblocking(true).map_err(|e| TransportError::Socket(e.to_string()))?;
        Ok(Self { socket, buffer: vec![0; DATAGRAM_HEADER_LEN + MAX_DATAGRAM_PAYLOAD] })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.socket.local_addr().map_err(|e| TransportError::Socket(e.to_string()))
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) -> Result<(), TransportError> {
        self.socket.send_to(datagram, to).map_err(|e| TransportError::Socket(e.to_string()))?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<(SocketAddr, Vec<u8>)>, TransportError> {
        match self.socket.recv_from(&mut self.buffer) {
            Ok((len, from)) => Ok(Some((from, self.buffer[..len].to_vec()))),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(TransportError::Socket(error.to_string())),
        }
    }
}

/// In-memory multi-peer network for integration tests: one shared hub, one port per address,
/// datagrams routed by destination. Deterministic (FIFO per port, optional seeded loss), so a
/// "two clients join one server" test runs with zero sockets and zero flakes.
#[derive(Default)]
pub struct MemoryHub {
    inner: std::rc::Rc<std::cell::RefCell<HubInner>>,
}

#[derive(Default)]
struct HubInner {
    queues: HashMap<SocketAddr, std::collections::VecDeque<(SocketAddr, Vec<u8>)>>,
    seed: u64,
    counter: u64,
    drop_percent: u64,
}

impl MemoryHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_loss(seed: u64, drop_percent: u64) -> Self {
        let hub = Self::default();
        {
            let mut inner = hub.inner.borrow_mut();
            inner.seed = seed;
            inner.drop_percent = drop_percent;
        }
        hub
    }

    pub fn port(&self, addr: SocketAddr) -> HubPort {
        self.inner.borrow_mut().queues.entry(addr).or_default();
        HubPort { inner: std::rc::Rc::clone(&self.inner), addr }
    }
}

pub struct HubPort {
    inner: std::rc::Rc<std::cell::RefCell<HubInner>>,
    addr: SocketAddr,
}

impl Transport for HubPort {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) -> Result<(), TransportError> {
        let mut inner = self.inner.borrow_mut();
        if inner.drop_percent > 0 {
            inner.counter = inner.counter.wrapping_add(1);
            let roll = game_core::math::splitmix64(
                inner.seed ^ inner.counter.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            ) % 100;
            if roll < inner.drop_percent {
                return Ok(());
            }
        }
        let from = self.addr;
        inner.queues.entry(to).or_default().push_back((from, datagram.to_vec()));
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<(SocketAddr, Vec<u8>)>, TransportError> {
        Ok(self.inner.borrow_mut().queues.get_mut(&self.addr).and_then(|q| q.pop_front()))
    }
}

/// Deterministic hostile network for tests: drops, duplicates and reorders by a seeded hash —
/// the same seed always misbehaves identically, so a chaos test is a regression test.
pub struct LossyLoopback {
    queue: std::collections::VecDeque<(SocketAddr, Vec<u8>)>,
    seed: u64,
    counter: u64,
    /// Percent 0..100.
    pub drop_percent: u64,
    pub duplicate_percent: u64,
    pub reorder_percent: u64,
}

impl LossyLoopback {
    pub fn new(seed: u64) -> Self {
        Self {
            queue: Default::default(),
            seed,
            counter: 0,
            drop_percent: 0,
            duplicate_percent: 0,
            reorder_percent: 0,
        }
    }

    fn roll(&mut self) -> u64 {
        self.counter = self.counter.wrapping_add(1);
        game_core::math::splitmix64(self.seed ^ self.counter.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            % 100
    }
}

impl Transport for LossyLoopback {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) -> Result<(), TransportError> {
        if self.roll() < self.drop_percent {
            return Ok(());
        }
        self.queue.push_back((to, datagram.to_vec()));
        if self.roll() < self.duplicate_percent {
            self.queue.push_back((to, datagram.to_vec()));
        }
        if self.roll() < self.reorder_percent && self.queue.len() >= 2 {
            let last = self.queue.pop_back().expect("len checked");
            self.queue.insert(self.queue.len().saturating_sub(1), last);
        }
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<(SocketAddr, Vec<u8>)>, TransportError> {
        Ok(self.queue.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr() -> SocketAddr {
        "127.0.0.1:9".parse().expect("addr")
    }

    #[test]
    fn fragmentation_roundtrips_any_size_in_order() {
        let mut reassembler = Reassembler::default();
        for (seq, len) in [(1_u16, 1_usize), (2, MAX_DATAGRAM_PAYLOAD), (3, 5_000), (4, 20_000)] {
            let frame: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let datagrams = fragment_message(seq, &frame).expect("fits");
            let mut out = None;
            for datagram in datagrams {
                if let Some(done) = reassembler.accept(&datagram).expect("valid") {
                    out = Some(done);
                }
            }
            assert_eq!(out.expect("completed"), frame, "len {len}");
        }
    }

    #[test]
    fn an_oversized_message_is_refused_not_truncated() {
        let frame = vec![0_u8; MAX_DATAGRAM_PAYLOAD * MAX_FRAGMENTS + 1];
        assert!(matches!(fragment_message(9, &frame), Err(TransportError::TooLarge { .. })));
    }

    #[test]
    fn duplicates_reorder_and_partial_loss_still_deliver_the_next_message() {
        let mut reassembler = Reassembler::default();
        let frame: Vec<u8> = (0..4_000).map(|i| (i % 256) as u8).collect();
        let lost = fragment_message(7, &frame).expect("fits");
        // Lose the middle fragment of message 7 entirely.
        for datagram in lost.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, d)| d) {
            assert!(reassembler.accept(datagram).expect("valid").is_none());
        }
        // Message 8 arrives duplicated and reversed — it must still complete.
        let next = fragment_message(8, &frame).expect("fits");
        let mut done = None;
        for datagram in next.iter().rev().chain(next.iter()) {
            if let Some(out) = reassembler.accept(datagram).expect("valid") {
                done = Some(out);
            }
        }
        assert_eq!(done.expect("message 8 completes"), frame);
    }

    #[test]
    fn the_reassembly_window_abandons_stale_messages_instead_of_growing() {
        let mut reassembler = Reassembler::default();
        let frame = vec![9_u8; 3_000];
        for seq in 0..200_u16 {
            // First fragment only — never completes.
            let datagram = &fragment_message(seq, &frame).expect("fits")[0];
            reassembler.accept(datagram).expect("valid");
        }
        assert!(reassembler.pending.len() <= REASSEMBLY_WINDOW + 1);
    }

    #[test]
    fn the_lossy_loopback_is_deterministic_and_hostile() {
        let run = |seed| {
            let mut wire = LossyLoopback::new(seed);
            wire.drop_percent = 30;
            wire.duplicate_percent = 10;
            wire.reorder_percent = 20;
            let mut delivered = Vec::new();
            for i in 0..100_u8 {
                wire.send(addr(), &[i]).expect("send");
            }
            while let Some((_, datagram)) = wire.recv().expect("recv") {
                delivered.push(datagram[0]);
            }
            delivered
        };
        let a = run(42);
        assert_eq!(a, run(42), "same seed, same misbehavior");
        assert_ne!(a.len(), 100, "the wire really drops");
        assert_ne!(a, run(43), "different seed, different weather");
    }

    #[test]
    fn udp_transport_delivers_between_two_real_sockets() {
        let Ok(mut a) = UdpTransport::bind("127.0.0.1:0".parse().expect("addr")) else {
            return; // no loopback in this sandbox — the loopback tests carry the logic
        };
        let Ok(mut b) = UdpTransport::bind("127.0.0.1:0".parse().expect("addr")) else {
            return;
        };
        let to = b.local_addr().expect("addr");
        a.send(to, b"WD-hello").expect("send");
        // Non-blocking: poll briefly.
        for _ in 0..50 {
            if let Some((_, datagram)) = b.recv().expect("recv") {
                assert_eq!(datagram, b"WD-hello");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("datagram never arrived on loopback");
    }
}
