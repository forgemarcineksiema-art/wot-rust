//! The one place a render pass is opened, and the only thing that can draw into one.
//!
//! Every pass goes through [`PassRecorder::begin`], which takes a [`PassId`] and derives the debug
//! label and the timestamp writes from it. A call site cannot name its own label, cannot forget
//! the timestamps, and cannot open a pass the profiler has never heard of — the descriptor is not
//! its to fill in.
//!
//! That is the whole reason this type exists. An instrument wired into ten call sites decays: the
//! eleventh pass is added by someone who has no reason to know the profiler exists, and from then
//! on the frame's timings quietly sum to less than the frame. Routing every pass through one
//! constructor makes the eleventh pass instrumented by construction.
//!
//! The same argument applies to the draws inside a pass, and there it is settled by something
//! stronger than a rule: [`CountedPass`] owns the `wgpu::RenderPass` **privately** and exposes
//! only counting draws. There is no way to reach past it, so there is no uncounted draw to write.
//! What the gate then has to protect is narrower and easier to state — no module but this one may
//! so much as name `wgpu::RenderPass`.
//!
//! **Slots are allocated in encoding order, not per `PassId`.** SSAO is skipped at zero strength,
//! bloom at zero mips, and the scene takes either one pass or two — so a fixed slot per pass would
//! leave holes, and resolving a query nobody wrote is a validation error on some backends and
//! garbage on the rest. Allocating on first use keeps the written queries a contiguous prefix,
//! which one `resolve_query_set` covers exactly.
//!
//! Counting needs no GPU feature and no adapter support, so it happens on every frame the renderer
//! draws, timed or not. It is the half of the instrument that is available everywhere — including
//! on a machine where `TIMESTAMP_QUERY` is missing, and inside an ordinary `cargo test`.

use crate::frame_graph::PassId;
use crate::frame_profiler::FrameProfiler;

/// What one pass submitted. Draws are the submission cost, triangles the geometry cost — and
/// instances are here because this project has already measured a case where the bottleneck was
/// the NUMBER OF INSTANCES rather than the triangles in them (the grass programme). A counter that
/// could not express that lesson would have to be widened the first time it mattered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassCounts {
    pub draws: u32,
    pub triangles: u64,
    pub instances: u64,
}

impl PassCounts {
    fn add(&mut self, triangles: u64, instances: u64) {
        self.draws += 1;
        self.triangles += triangles;
        self.instances += instances;
    }

    fn merge(&mut self, other: &Self) {
        self.draws += other.draws;
        self.triangles += other.triangles;
        self.instances += other.instances;
    }
}

/// What a whole frame submitted, per pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounts {
    per_pass: [PassCounts; PassId::COUNT],
}

impl FrameCounts {
    /// What one pass submitted this frame. Zero for a pass the frame skipped — which is a fact
    /// worth reading rather than an absence: a scene pass at zero draws is a black frame.
    pub fn pass(&self, id: PassId) -> PassCounts {
        self.per_pass[id.index()]
    }

    /// The frame's totals, which is what a draw-call budget is stated against.
    pub fn total(&self) -> PassCounts {
        let mut total = PassCounts::default();
        for counts in &self.per_pass {
            total.merge(counts);
        }
        total
    }
}

/// Which pass owns which timestamp slot, for the frame that just ran.
///
/// Slots are handed out in encoding order, so slot `2i` is the start of `slots[i]` and `2i + 1`
/// its end. Reading the query set back without this is reading numbers with no names on them.
#[derive(Debug, Clone, Copy, Default)]
pub struct PassOrder {
    slots: [Option<PassId>; PassId::COUNT],
    len: usize,
}

impl PassOrder {
    /// How many passes this frame encoded — and so how many timestamp PAIRS it wrote.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Each encoded pass with its slot index, in the order the frame encoded them.
    pub fn iter(&self) -> impl Iterator<Item = (usize, PassId)> + '_ {
        self.slots[..self.len].iter().enumerate().filter_map(|(i, id)| id.map(|id| (i, id)))
    }
}

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

/// A render pass that counts what is drawn into it.
///
/// It owns the `wgpu::RenderPass` privately and forwards the four state setters unchanged. The two
/// draw calls are the point: they are the only draws reachable from outside this module, so a
/// frame's draw and triangle totals cannot silently miss one.
pub(crate) struct CountedPass<'e, 'c> {
    pass: wgpu::RenderPass<'e>,
    counts: &'c mut PassCounts,
}

impl CountedPass<'_, '_> {
    pub(crate) fn set_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        self.pass.set_pipeline(pipeline);
    }

    pub(crate) fn set_bind_group<'a, BG>(
        &mut self,
        index: u32,
        bind_group: BG,
        offsets: &[wgpu::DynamicOffset],
    ) where
        Option<&'a wgpu::BindGroup>: From<BG>,
    {
        self.pass.set_bind_group(index, bind_group, offsets);
    }

    pub(crate) fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: wgpu::BufferSlice<'_>) {
        self.pass.set_vertex_buffer(slot, buffer_slice);
    }

    pub(crate) fn set_index_buffer(
        &mut self,
        buffer_slice: wgpu::BufferSlice<'_>,
        index_format: wgpu::IndexFormat,
    ) {
        self.pass.set_index_buffer(buffer_slice, index_format);
    }

    /// Every pipeline in this renderer is a `TriangleList`, which is what lets vertices divide by
    /// three with nothing left to interpret. A strip or a fan would make this arithmetic quietly
    /// wrong, so `every_pipeline_is_a_triangle_list` holds that premise still.
    pub(crate) fn draw(
        &mut self,
        vertices: core::ops::Range<u32>,
        instances: core::ops::Range<u32>,
    ) {
        let instance_count = u64::from(instances.end.saturating_sub(instances.start));
        let triangles = u64::from(vertices.end.saturating_sub(vertices.start)) / 3;
        self.counts.add(triangles * instance_count, instance_count);
        self.pass.draw(vertices, instances);
    }

    pub(crate) fn draw_indexed(
        &mut self,
        indices: core::ops::Range<u32>,
        base_vertex: i32,
        instances: core::ops::Range<u32>,
    ) {
        let instance_count = u64::from(instances.end.saturating_sub(instances.start));
        let triangles = u64::from(indices.end.saturating_sub(indices.start)) / 3;
        self.counts.add(triangles * instance_count, instance_count);
        self.pass.draw_indexed(indices, base_vertex, instances);
    }
}

/// Opens every render pass of a frame, and knows what each one submitted.
pub(crate) struct PassRecorder<'p> {
    profiler: &'p FrameProfiler,
    /// Passes in the order their slots were allocated; `index * 2` is the begin slot. Read back
    /// by the results pass that lands with the readback, which is what maps a slot to a pass.
    order: Vec<PassId>,
    counts: FrameCounts,
}

impl<'p> PassRecorder<'p> {
    pub(crate) fn new(profiler: &'p FrameProfiler) -> Self {
        Self { profiler, order: Vec::with_capacity(PassId::COUNT), counts: FrameCounts::default() }
    }

    /// Open a pass that measures itself end to end.
    pub(crate) fn begin<'e, 'r>(
        &'r mut self,
        encoder: &'e mut wgpu::CommandEncoder,
        pass: PassId,
        color_attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>],
        depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
    ) -> CountedPass<'e, 'r> {
        self.begin_span(
            encoder,
            pass,
            TimestampSpan::Whole,
            color_attachments,
            depth_stencil_attachment,
        )
    }

    /// Open a pass that is one part of a multi-pass span (the bloom ladder).
    pub(crate) fn begin_span<'e, 'r>(
        &'r mut self,
        encoder: &'e mut wgpu::CommandEncoder,
        pass: PassId,
        span: TimestampSpan,
        color_attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>],
        depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
    ) -> CountedPass<'e, 'r> {
        let timestamp_writes = self.timestamp_writes(pass, span);
        let counts = &mut self.counts.per_pass[pass.index()];
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(pass.label()),
            color_attachments,
            depth_stencil_attachment,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        CountedPass { pass: render_pass, counts }
    }

    /// What this frame submitted, per pass. Collected on every frame — counting needs no GPU
    /// feature, so this half of the instrument works on adapters that cannot time a thing.
    pub(crate) fn counts(&self) -> FrameCounts {
        self.counts
    }

    /// Which pass owns which timestamp slot this frame — the names for the numbers the query set
    /// gives back.
    pub(crate) fn order(&self) -> PassOrder {
        let mut slots = [None; PassId::COUNT];
        for (slot, pass) in self.order.iter().enumerate() {
            slots[slot] = Some(*pass);
        }
        PassOrder { slots, len: self.order.len() }
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
    use super::{PassCounts, PassRecorder, TimestampSpan};
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

    /// The arithmetic a draw-call budget is stated in, checked against numbers worked by hand.
    /// Instanced draws MULTIPLY: four triangles at five instances is twenty, not four — and that
    /// is the whole difference between a fleet costing 66 draws and costing 2302.
    #[test]
    fn a_draw_counts_its_triangles_times_its_instances() {
        let mut counts = PassCounts::default();
        counts.add(4 * 5, 5);
        counts.add(1, 1);
        assert_eq!(counts, PassCounts { draws: 2, triangles: 21, instances: 6 });

        let mut total = PassCounts::default();
        total.merge(&counts);
        total.merge(&counts);
        assert_eq!(total, PassCounts { draws: 4, triangles: 42, instances: 12 });
    }
}
