//! Per-pass GPU timing: the capability negotiation and the three states it can land in.
//!
//! **This module does not time anything yet.** It establishes what timing would cost and whether
//! this machine can do it at all; the `timestamp_writes` that make a frame report itself land in
//! the next change, behind a recorder that is the only place allowed to open a pass.
//!
//! Why it needs its own negotiation instead of just asking: `GpuContext` requests
//! `Features::empty()`, deliberately. Asking for an optional feature the adapter lacks is a device
//! creation ERROR, not a downgrade — so a renderer that asked for timestamps unconditionally would
//! simply fail to start on the machines that need profiling least. The negotiation is a pure
//! function over (what we want, what the adapter has), which makes the interesting half testable
//! with no GPU at all.
//!
//! Why the shipped path must stay `Disabled` rather than `cfg`-gated: `cfg(debug_assertions)`
//! would make the instrument unavailable in the only build that matters — a release build on the
//! min spec. So it is a runtime state, defaulting off, and the guarantee that shipping costs
//! nothing is a test rather than a promise.

use crate::frame_graph::PassId;
use crate::pass_recorder::PassOrder;

/// Two timestamps per pass: one written at the start of the pass, one at the end.
const SLOTS_PER_PASS: u32 = 2;

/// The GPU features a device must be created with, given what the caller wants and what the
/// adapter can actually do.
///
/// Only `TIMESTAMP_QUERY` is ever requested. The `*_INSIDE_PASSES` / `*_INSIDE_ENCODERS`
/// variants are native-only and thinly supported; writing on pass BOUNDARIES needs neither, and
/// boundaries are the granularity a per-pass budget is stated in anyway.
pub(crate) fn required_features(want_timing: bool, available: wgpu::Features) -> wgpu::Features {
    if want_timing && available.contains(wgpu::Features::TIMESTAMP_QUERY) {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    }
}

/// What each pass of one frame cost on the GPU, in milliseconds.
///
/// `frame_ms` is the span from the first pass's start to the last pass's end, so it is NOT the sum
/// of the parts: whatever the GPU spends between passes — resolves, layout transitions, idle
/// waiting on a previous submit — lands in the gap. That gap is reported rather than hidden,
/// because a per-pass table that quietly sums to less than the frame invites the reader to
/// believe the passes are the whole story.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTimings {
    per_pass_ms: [f32; PassId::COUNT],
    ran: [bool; PassId::COUNT],
    frame_ms: f32,
}

impl FrameTimings {
    /// What this pass cost, or `None` if the frame did not encode it.
    pub fn pass_ms(&self, id: PassId) -> Option<f32> {
        self.ran[id.index()].then(|| self.per_pass_ms[id.index()])
    }

    /// The passes added up.
    pub fn sum_ms(&self) -> f32 {
        self.per_pass_ms.iter().sum()
    }

    /// First start to last end.
    pub fn frame_ms(&self) -> f32 {
        self.frame_ms
    }

    /// What the frame spent outside any pass. Never negative in practice; clamped so a
    /// nanosecond of measurement noise cannot print as a negative residual.
    pub fn unattributed_ms(&self) -> f32 {
        (self.frame_ms - self.sum_ms()).max(0.0)
    }
}

/// The per-pass timing instrument, in the only three states it can be in.
///
/// `Unavailable` carries its reason because "no numbers" and "no numbers BECAUSE this adapter
/// cannot" are different facts, and a probe that prints the second one saves the next person the
/// afternoon this one cost.
#[derive(Default)]
pub enum FrameProfiler {
    /// Nobody asked for timing. The shipped path, and the state in which the encoder emits a
    /// byte-for-byte unchanged command stream — and the default, so forgetting to decide ships
    /// the cheap thing.
    #[default]
    Disabled,
    /// Timing was asked for and cannot be given here.
    Unavailable { reason: String },
    /// Timing is armed: the query set and its buffers exist and the tick period is known.
    Active(ActiveProfiler),
}

/// The GPU-side resources a timed frame needs. Allocated once, reused every frame.
pub struct ActiveProfiler {
    query_set: wgpu::QuerySet,
    /// `resolve_query_set` writes u64 ticks here; it is not CPU-visible.
    resolve: wgpu::Buffer,
    /// The CPU-visible copy the results are read back through.
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick, from the queue. Timestamps are meaningless without it.
    period_ns: f32,
}

impl ActiveProfiler {
    pub fn query_set(&self) -> &wgpu::QuerySet {
        &self.query_set
    }

    pub fn resolve_buffer(&self) -> &wgpu::Buffer {
        &self.resolve
    }

    pub fn readback_buffer(&self) -> &wgpu::Buffer {
        &self.readback
    }

    pub fn period_ns(&self) -> f32 {
        self.period_ns
    }

    /// The pair of query indices this pass writes: (start of pass, end of pass).
    pub fn slots(&self, pass: PassId) -> (u32, u32) {
        let base = pass.index() as u32 * SLOTS_PER_PASS;
        (base, base + 1)
    }

    /// Read the last resolved frame's timestamps back and turn ticks into milliseconds.
    ///
    /// Blocks on the device: the caller is a probe that has already fenced the frame, and a
    /// non-blocking read would hand back the frame before last with nothing saying so. Returns
    /// `None` when the frame encoded no passes or the mapping failed — never a zeroed table,
    /// which would read as "every pass was free".
    pub fn read(&self, device: &wgpu::Device, order: PassOrder) -> Option<FrameTimings> {
        if order.is_empty() {
            return None;
        }
        let written = order.len() * SLOTS_PER_PASS as usize;
        let bytes = (written * std::mem::size_of::<u64>()) as u64;
        let slice = self.readback.slice(..bytes);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
            return None;
        }
        let ticks: Vec<u64> = slice
            .get_mapped_range()
            .chunks_exact(std::mem::size_of::<u64>())
            .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
            .collect();
        self.readback.unmap();
        if ticks.len() < written {
            return None;
        }

        let to_ms = |ticks: u64| ticks as f64 * f64::from(self.period_ns) / 1.0e6;
        let mut timings = FrameTimings::default();
        for (slot, pass) in order.iter() {
            let begin = ticks[slot * 2];
            let end = ticks[slot * 2 + 1];
            // A pass whose end precedes its start is a driver reporting nonsense, not a negative
            // duration; drop it rather than let it eat the residual.
            if end < begin {
                continue;
            }
            timings.per_pass_ms[pass.index()] = to_ms(end - begin) as f32;
            timings.ran[pass.index()] = true;
        }
        let first = ticks[0];
        let last = ticks[written - 1];
        timings.frame_ms = if last >= first { to_ms(last - first) as f32 } else { 0.0 };
        Some(timings)
    }
}

impl FrameProfiler {
    /// Arm the profiler if the caller wants timing AND the device was created with the feature.
    ///
    /// Note the device, not the adapter: asking an adapter that supports timestamps is not enough
    /// if the device was built without the feature, and that mismatch is exactly the kind of
    /// silent half-state this project keeps finding. Reading it back off the device makes the
    /// question unambiguous.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, want_timing: bool) -> Self {
        if !want_timing {
            return Self::Disabled;
        }
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Self::Unavailable {
                reason: "the device was created without TIMESTAMP_QUERY — build the GpuContext \
                         with GpuContextOptions { pass_timing: true }"
                    .to_string(),
            };
        }
        let slots = PassId::COUNT as u32 * SLOTS_PER_PASS;
        let bytes = u64::from(slots) * std::mem::size_of::<u64>() as u64;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame_pass_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: slots,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_pass_timestamps_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_pass_timestamps_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self::Active(ActiveProfiler {
            query_set,
            resolve,
            readback,
            period_ns: queue.get_timestamp_period(),
        })
    }

    /// The armed instrument, if there is one.
    pub fn active(&self) -> Option<&ActiveProfiler> {
        match self {
            Self::Active(active) => Some(active),
            _ => None,
        }
    }

    /// Why this frame carries no timings, for a probe to print. `None` when it does carry them.
    pub fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable { reason } => Some(reason),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::required_features;

    /// Asking for a feature the adapter lacks is a device-creation ERROR in wgpu, not a silent
    /// downgrade — so the negotiation has to refuse on our side. A renderer that got this wrong
    /// would fail to start on exactly the weak machines this project profiles for.
    #[test]
    fn required_features_never_asks_for_what_the_adapter_lacks() {
        let none = wgpu::Features::empty();
        let has = wgpu::Features::TIMESTAMP_QUERY;

        assert_eq!(required_features(true, has), has, "wanted and available: ask");
        assert_eq!(required_features(true, none), none, "wanted, not available: do not ask");
        assert_eq!(required_features(false, has), none, "not wanted: do not ask");
        assert_eq!(required_features(false, none), none, "neither: do not ask");
    }

    /// The shipped game asks for nothing optional, on any adapter. This is the guarantee that
    /// the instrument costs the player nothing — as a test, because as a comment it would rot the
    /// first time somebody needed one more feature "just for a moment".
    #[test]
    fn the_shipped_context_requests_no_optional_features() {
        assert_eq!(
            required_features(false, wgpu::Features::all()),
            wgpu::Features::empty(),
            "an untimed context must request nothing, even where everything is available"
        );
    }
}
