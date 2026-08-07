//! Aggregating a rotation of timed frames into numbers somebody can act on.
//!
//! The hard part is not the arithmetic. It is refusing to print.
//!
//! This project has twice been told something false by a frame capture, and once by the capture's
//! own method: sequential probe runs measure the laptop's thermal ramp rather than the scene, and
//! a baseline walked 19.2 -> 23.8 ms across four runs with nothing changed. The defence is to
//! rotate configurations INSIDE one process in short blocks, so every config visits every thermal
//! state. But a rotation only defends anything if it actually completed — a run cut short, a
//! config that errored out of a block, a cycle count changed mid-edit, and the aggregate silently
//! becomes "config A when the box was cold against config B when it was hot", which is precisely
//! the reading the rotation exists to prevent, now wearing the rotation's authority.
//!
//! So [`RotationStats::imbalance`] is checked before anything is formatted, and an unbalanced
//! rotation reports WHAT IS MISSING instead of numbers. Numbers that are wrong are worse than no
//! numbers, because they get quoted.

use std::fmt::Write as _;

/// A sorted series read at the points a frame budget is argued at.
///
/// p50 for the typical frame, p95 and p99 for the ones the one-look policy calls bugs, and max
/// because a single 40 ms frame is a visible hitch no percentile will show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentiles {
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
    pub max: f32,
    pub samples: usize,
}

/// The percentile of an ALREADY SORTED series, by nearest rank.
///
/// One implementation, deliberately: the HUD's own p95 readout and this share a definition of
/// what "p95" means, and a second convention would make two honest numbers disagree.
fn percentile(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
    sorted[index]
}

fn percentiles_of(samples: &[f32]) -> Percentiles {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f32::total_cmp);
    Percentiles {
        p50: percentile(&sorted, 0.50),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
        max: sorted.last().copied().unwrap_or(0.0),
        samples: sorted.len(),
    }
}

#[derive(Debug, Clone)]
struct ConfigSamples {
    /// Bitmask of the rotation cycles this config was sampled in.
    cycles_seen: u64,
    frame_ms: Vec<f32>,
    unattributed_ms: Vec<f32>,
    /// `[pass][sample]`. A pass the config never encodes stays empty and reports nothing, which
    /// is different from reporting zero.
    pass_ms: Vec<Vec<f32>>,
}

/// Timed frames from one interleaved rotation, kept per config and per pass.
#[derive(Debug, Clone)]
pub struct RotationStats {
    cycles: usize,
    per_config: Vec<ConfigSamples>,
}

impl RotationStats {
    /// # Panics
    /// If `cycles` exceeds 64 — the visit ledger is a bitmask, and a rotation that deep would
    /// silently stop being checked.
    pub fn new(configs: usize, cycles: usize, passes: usize) -> Self {
        assert!(cycles <= 64, "the rotation ledger tracks at most 64 cycles, not {cycles}");
        Self {
            cycles,
            per_config: (0..configs)
                .map(|_| ConfigSamples {
                    cycles_seen: 0,
                    frame_ms: Vec::new(),
                    unattributed_ms: Vec::new(),
                    pass_ms: vec![Vec::new(); passes],
                })
                .collect(),
        }
    }

    /// Record one timed frame. `pass_ms[i]` is `None` for a pass this frame did not encode.
    ///
    /// The residual — frame minus the sum of its passes — is computed here rather than by the
    /// caller, so it cannot be quietly left out of a report.
    pub fn record(&mut self, config: usize, cycle: usize, frame_ms: f32, pass_ms: &[Option<f32>]) {
        let entry = &mut self.per_config[config];
        entry.cycles_seen |= 1 << cycle;
        entry.frame_ms.push(frame_ms);
        let mut attributed = 0.0;
        for (pass, sample) in pass_ms.iter().enumerate() {
            if let Some(ms) = sample {
                entry.pass_ms[pass].push(*ms);
                attributed += *ms;
            }
        }
        entry.unattributed_ms.push((frame_ms - attributed).max(0.0));
    }

    /// What is missing, if anything. `None` means every config visited every cycle and the
    /// aggregate compares like with like.
    pub fn imbalance(&self) -> Option<String> {
        let complete = if self.cycles >= 64 { u64::MAX } else { (1u64 << self.cycles) - 1 };
        let mut report = String::new();
        for (config, entry) in self.per_config.iter().enumerate() {
            let missing: Vec<usize> =
                (0..self.cycles).filter(|cycle| entry.cycles_seen & (1 << cycle) == 0).collect();
            if !missing.is_empty() {
                let _ = write!(report, "\n  config[{config}] never sampled cycle(s) {missing:?}");
            }
        }
        if self.per_config.iter().all(|entry| entry.cycles_seen == complete) && report.is_empty() {
            return None;
        }
        Some(format!(
            "rotation INVALID — sequential thermal states cannot be compared as if they were one \
             run:{report}"
        ))
    }

    /// The frame totals for one config.
    pub fn frame(&self, config: usize) -> Percentiles {
        percentiles_of(&self.per_config[config].frame_ms)
    }

    /// What this config spent outside any pass.
    pub fn unattributed(&self, config: usize) -> Percentiles {
        percentiles_of(&self.per_config[config].unattributed_ms)
    }

    /// One pass of one config, or `None` if that config never encoded it.
    pub fn pass(&self, config: usize, pass: usize) -> Option<Percentiles> {
        let samples = &self.per_config[config].pass_ms[pass];
        (!samples.is_empty()).then(|| percentiles_of(samples))
    }
}

#[cfg(test)]
mod tests {
    use super::{Percentiles, RotationStats};

    /// Nearest rank on a hand-written series, so the convention is pinned rather than inferred
    /// from whatever the first caller happened to want.
    #[test]
    fn percentiles_match_a_hand_computed_series() {
        let mut stats = RotationStats::new(1, 1, 1);
        // 1..=100 ms, recorded out of order to prove the aggregate sorts.
        for (index, ms) in (1..=100).rev().enumerate() {
            stats.record(0, 0, ms as f32, &[Some(ms as f32)]);
            assert_eq!(stats.frame(0).samples, index + 1);
        }
        let frame = stats.frame(0);
        assert_eq!(
            frame,
            Percentiles { p50: 51.0, p95: 96.0, p99: 100.0, max: 100.0, samples: 100 }
        );
        assert_eq!(stats.pass(0, 0).expect("the pass ran"), frame);
    }

    /// The whole point of the type. A rotation that did not complete describes two thermal states
    /// pretending to be one run, and this is the case where numbers are worse than silence.
    #[test]
    fn an_unbalanced_rotation_reports_invalid_instead_of_numbers() {
        let mut stats = RotationStats::new(2, 3, 1);
        for cycle in 0..3 {
            stats.record(0, cycle, 10.0, &[Some(4.0)]);
        }
        stats.record(1, 0, 12.0, &[Some(5.0)]);
        stats.record(1, 1, 12.0, &[Some(5.0)]);

        let complaint = stats.imbalance().expect("config 1 never reached cycle 2");
        assert!(complaint.contains("INVALID"), "{complaint}");
        assert!(complaint.contains("config[1]"), "{complaint}");
        assert!(complaint.contains('2'), "the missing cycle has to be named: {complaint}");
        assert!(!complaint.contains("config[0]"), "a complete config is not a complaint");

        stats.record(1, 2, 12.0, &[Some(5.0)]);
        assert!(stats.imbalance().is_none(), "a completed rotation has nothing to complain about");
    }

    /// The residual is recorded on every frame, including the frames where it is zero. A report
    /// that only mentions the gap when it is large is a report that teaches the reader the passes
    /// are the whole frame.
    #[test]
    fn the_report_always_carries_the_unattributed_residual() {
        let mut stats = RotationStats::new(1, 1, 2);
        stats.record(0, 0, 10.0, &[Some(4.0), Some(3.0)]);
        stats.record(0, 0, 10.0, &[Some(5.0), Some(5.0)]);
        let residual = stats.unattributed(0);
        assert_eq!(residual.samples, 2, "every frame contributes a residual, even a zero one");
        assert_eq!(residual.p50, 3.0);
        assert_eq!(residual.max, 3.0);

        // A pass the config never encodes reports nothing rather than zero: "did not run" and
        // "ran for free" are different facts and only one of them is a budget.
        let mut skipped = RotationStats::new(1, 1, 2);
        skipped.record(0, 0, 8.0, &[Some(4.0), None]);
        assert!(skipped.pass(0, 0).is_some());
        assert!(skipped.pass(0, 1).is_none());
        assert_eq!(skipped.unattributed(0).p50, 4.0);
    }
}
