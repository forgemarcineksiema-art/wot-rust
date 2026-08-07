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
