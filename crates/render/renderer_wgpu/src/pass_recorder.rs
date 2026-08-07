//! The one place a render pass is opened.
//!
//! Every pass in a frame goes through [`PassRecorder::begin`], which takes a [`PassId`] and
//! derives the debug label and the timestamp writes from it. A call site cannot name its own
//! label, cannot forget the timestamps, and cannot open a pass the profiler has never heard of —
//! not by discipline, but because the descriptor is not its to fill in.
//!
//! That is the whole reason this type exists. An instrument wired into ten call sites decays: the
//! eleventh pass is added by someone who has no reason to know the profiler exists, and from then
//! on the frame's timings quietly sum to less than the frame. Routing every pass through one
//! constructor makes the eleventh pass instrumented by construction, and
//! `begin_render_pass_is_only_called_from_the_recorder` keeps it that way.
//!
//! **Slots are allocated in encoding order, not per `PassId`.** SSAO is skipped at zero strength,
//! bloom at zero mips, and the scene takes either one pass or two — so a fixed slot per pass would
//! leave holes, and resolving a query nobody wrote is a validation error on some backends and
//! garbage on the rest. Allocating on first use keeps the written queries a contiguous prefix,
//! which one `resolve_query_set` covers exactly.

use crate::frame_graph::PassId;
use crate::frame_profiler::FrameProfiler;

/// Which end(s) of a pass this call writes a timestamp at.
///
/// Only the bloom ladder needs anything but [`TimestampSpan::Whole`]: it is 2N passes under one
/// identity, so the first opens the span, the last closes it, and the rungs between are timed as
/// part of it rather than separately. A per-rung budget is not a number anyone could act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimestampSpan {
    /// A pass that begins and ends its own measurement — almost all of them.
    Whole,
    /// The first pass of a multi-pass span: writes the start, leaves the end to the last.
    Start,
    /// The last pass of a multi-pass span: writes the end into the slot `Start` opened.
    End,
    /// A pass inside a span: measured by the span, writes nothing of its own.
    Inside,
}

/// Opens every render pass of a frame, and knows which ones actually ran.
pub(crate) struct PassRecorder<'p> {
    profiler: &'p FrameProfiler,
    /// Passes in the order their slots were allocated; `index * 2` is the begin slot. Read
    /// back by the results pass that lands with the readback, which is what maps a slot to a pass.
    order: Vec<PassId>,
}

impl<'p> PassRecorder<'p> {
    pub(crate) fn new(profiler: &'p FrameProfiler) -> Self {
        Self { profiler, order: Vec::with_capacity(PassId::COUNT) }
    }

    /// Open a pass that measures itself end to end.
    pub(crate) fn begin<'e>(
        &mut self,
        encoder: &'e mut wgpu::CommandEncoder,
        pass: PassId,
        color_attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>],
        depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
    ) -> wgpu::RenderPass<'e> {
        self.begin_span(
            encoder,
            pass,
            TimestampSpan::Whole,
            color_attachments,
            depth_stencil_attachment,
        )
    }

    /// Open a pass that is one part of a multi-pass span (the bloom ladder).
    pub(crate) fn begin_span<'e>(
        &mut self,
        encoder: &'e mut wgpu::CommandEncoder,
        pass: PassId,
        span: TimestampSpan,
        color_attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>],
        depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
    ) -> wgpu::RenderPass<'e> {
        let timestamp_writes = self.timestamp_writes(pass, span);
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(pass.label()),
            color_attachments,
            depth_stencil_attachment,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// Copy this frame's timestamps out of the query set, ready to be read back.
    ///
    /// Resolves exactly the prefix that was written, which is the point of allocating on first
    /// use. Called once, on the same encoder, before submit — a resolve on a later encoder would
    /// read a query set the GPU may not have finished writing.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(active) = self.profiler.active() else { return };
        let written = self.order.len() as u32 * 2;
        if written == 0 {
            return;
        }
        let bytes = u64::from(written) * std::mem::size_of::<u64>() as u64;
        encoder.resolve_query_set(active.query_set(), 0..written, active.resolve_buffer(), 0);
        encoder.copy_buffer_to_buffer(
            active.resolve_buffer(),
            0,
            active.readback_buffer(),
            0,
            bytes,
        );
    }

    fn timestamp_writes(
        &mut self,
        pass: PassId,
        span: TimestampSpan,
    ) -> Option<wgpu::RenderPassTimestampWrites<'p>> {
        let active = self.profiler.active()?;
        if span == TimestampSpan::Inside {
            return None;
        }
        let base = self.slot_for(pass);
        let (beginning_of_pass_write_index, end_of_pass_write_index) = match span {
            TimestampSpan::Whole => (Some(base), Some(base + 1)),
            TimestampSpan::Start => (Some(base), None),
            TimestampSpan::End => (None, Some(base + 1)),
            TimestampSpan::Inside => unreachable!("returned above"),
        };
        Some(wgpu::RenderPassTimestampWrites {
            query_set: active.query_set(),
            beginning_of_pass_write_index,
            end_of_pass_write_index,
        })
    }

    /// The begin slot for this pass, allocating one the first time the pass is seen.
    fn slot_for(&mut self, pass: PassId) -> u32 {
        if let Some(index) = self.order.iter().position(|seen| *seen == pass) {
            return index as u32 * 2;
        }
        self.order.push(pass);
        (self.order.len() - 1) as u32 * 2
    }
}

#[cfg(test)]
mod tests {
    use super::{PassRecorder, TimestampSpan};
    use crate::frame_graph::PassId;
    use crate::frame_profiler::FrameProfiler;

    /// The guarantee the shipped game rests on: with no profiler, no pass writes a timestamp and
    /// no slot is allocated, so the command stream is the one this renderer emitted before the
    /// instrument existed. Asserted at the only place that could write one, and with no GPU —
    /// a promise in a comment would not survive the eleventh pass.
    #[test]
    fn a_renderer_without_the_profiler_writes_no_timestamps() {
        let disabled = FrameProfiler::Disabled;
        let mut recorder = PassRecorder::new(&disabled);

        for pass in PassId::ALL {
            for span in [
                TimestampSpan::Whole,
                TimestampSpan::Start,
                TimestampSpan::End,
                TimestampSpan::Inside,
            ] {
                assert!(
                    recorder.timestamp_writes(*pass, span).is_none(),
                    "{pass:?} would write a timestamp with the profiler disabled"
                );
            }
        }
        assert!(recorder.order.is_empty(), "a disabled frame allocated a timestamp slot");
    }

    /// Slots are handed out in ENCODING order, not per pass, so the written queries stay a
    /// contiguous prefix even though SSAO, bloom and the refraction split are all conditional.
    /// Resolving a query nobody wrote is a validation error on some backends and garbage on the
    /// rest, which is the failure this allocation order exists to make impossible.
    #[test]
    fn slots_are_allocated_in_encoding_order_and_a_span_reuses_its_own() {
        let disabled = FrameProfiler::Disabled;
        let mut recorder = PassRecorder::new(&disabled);

        // A frame that skips SSAO entirely still numbers its passes 0, 1, 2... with no holes.
        assert_eq!(recorder.slot_for(PassId::ShadowNear), 0);
        assert_eq!(recorder.slot_for(PassId::Scene), 2);
        assert_eq!(recorder.slot_for(PassId::Post), 4);
        // The bloom ladder opens and closes ONE span: asking again returns the same slot.
        assert_eq!(recorder.slot_for(PassId::Bloom), 6);
        assert_eq!(recorder.slot_for(PassId::Bloom), 6, "a span must not allocate twice");
        assert_eq!(
            recorder.order,
            [PassId::ShadowNear, PassId::Scene, PassId::Post, PassId::Bloom]
        );
    }
}
