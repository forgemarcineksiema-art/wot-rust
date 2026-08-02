use std::cell::Cell;
use std::collections::HashSet;

use renderer_api::{PipelineKey, RenderError};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineWarmupStats {
    pub created: usize,
    pub reused: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineHotReloadStats {
    pub replaced: usize,
    pub created: usize,
}

#[derive(Debug, Default)]
pub struct PipelineRegistry {
    cached: HashSet<PipelineKey>,
    /// How many times a draw call has asked for a pipeline that was not prewarmed.
    ///
    /// This counted nothing until 2026-08-02: it was never written, so
    /// `compilation_requests_during_draw` could only ever return 0, and the four tests asserting
    /// that were asserting the initialiser. A stall in the first frame of a battle is exactly the
    /// class of fault the one-look policy calls a game bug, and the instrument meant to catch it
    /// was a constant.
    ///
    /// A `Cell` because the draw path holds `&self` — counting a miss must not force a caller to
    /// hold the registry mutably in the middle of a frame.
    draw_compilation_requests: Cell<usize>,
}

impl PipelineRegistry {
    pub fn prewarm(&mut self, keys: impl IntoIterator<Item = PipelineKey>) -> PipelineWarmupStats {
        let mut stats = PipelineWarmupStats::default();
        for key in keys {
            if self.cached.insert(key) {
                stats.created += 1;
            } else {
                stats.reused += 1;
            }
        }
        stats
    }

    pub fn hot_reload(
        &mut self,
        keys: impl IntoIterator<Item = PipelineKey>,
    ) -> PipelineHotReloadStats {
        let mut stats = PipelineHotReloadStats::default();
        for key in keys {
            if self.cached.replace(key).is_some() {
                stats.replaced += 1;
            } else {
                stats.created += 1;
            }
        }
        stats
    }

    /// A draw call asking for a pipeline. A hit is free; a MISS is recorded, because a miss is the
    /// moment the frame would have had to compile a shader to finish drawing.
    pub fn require_for_draw(&self, key: &PipelineKey) -> Result<(), RenderError> {
        if self.cached.contains(key) {
            Ok(())
        } else {
            self.draw_compilation_requests.set(self.draw_compilation_requests.get() + 1);
            Err(RenderError::new(format!("missing prewarm for pipeline key: {key:?}")))
        }
    }

    pub fn cached_pipeline_count(&self) -> usize {
        self.cached.len()
    }

    pub fn compilation_requests_during_draw(&self) -> usize {
        self.draw_compilation_requests.get()
    }
}
