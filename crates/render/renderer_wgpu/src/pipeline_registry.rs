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
    draw_compilation_requests: usize,
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

    pub fn require_for_draw(&self, key: &PipelineKey) -> Result<(), RenderError> {
        if self.cached.contains(key) {
            Ok(())
        } else {
            Err(RenderError::new(format!("missing prewarm for pipeline key: {key:?}")))
        }
    }

    pub fn cached_pipeline_count(&self) -> usize {
        self.cached.len()
    }

    pub fn compilation_requests_during_draw(&self) -> usize {
        self.draw_compilation_requests
    }
}
