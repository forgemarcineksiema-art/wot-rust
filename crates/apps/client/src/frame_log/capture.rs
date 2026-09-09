//! Export only at shutdown: no filesystem writes on the frame path.
use std::fmt::Write as _;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::{FrameLog, Phase, TickPart};

impl FrameLog {
    pub fn write_capture(&self, path: &Path) -> std::io::Result<()> {
        let mut frames = BufWriter::new(std::fs::File::create(path.with_extension("frames.csv"))?);
        self.write_frames(&mut frames)?;
        frames.flush()?;
        let mut gpu = BufWriter::new(std::fs::File::create(path.with_extension("gpu.csv"))?);
        writeln!(gpu, "observed_at_s,pass,ms")?;
        for sample in &self.gpu {
            writeln!(gpu, "{:.6},gpu_frame,{:.6}", sample.at_s, sample.frame_ms)?;
            for (pass, ms) in &sample.passes {
                // CSV quote escaping also keeps diagnostic names round-trippable.
                writeln!(gpu, "{:.6},\"{}\",{:.6}", sample.at_s, pass.replace('"', "\"\""), ms)?;
            }
        }
        gpu.flush()?;
        std::fs::write(path, self.report())
    }

    fn write_frames(&self, out: &mut impl Write) -> std::io::Result<()> {
        write!(out, "frame,at_s,total_ms")?;
        for phase in Phase::ALL {
            write!(out, ",{}_ms", phase.name())?;
        }
        for part in TickPart::ALL {
            write!(out, ",tick_{}_ms", part.name())?;
        }
        writeln!(
            out,
            ",fixed_ticks,unattributed_ms,craters,particles,vehicle_instances,scenery_instances,fx_vertices"
        )?;
        for (index, frame) in self.frames.iter().enumerate() {
            write!(out, "{},{:.6},{:.6}", index + 1, frame.at_s, frame.total_ms)?;
            for ms in frame.phases_ms {
                write!(out, ",{ms:.6}")?;
            }
            for ms in frame.tick_parts_ms {
                write!(out, ",{ms:.6}")?;
            }
            let w = frame.workload;
            writeln!(
                out,
                ",{},{:.6},{},{},{},{},{}",
                frame.fixed_ticks,
                frame.unattributed_ms(),
                w.craters,
                w.particles,
                w.vehicle_instances,
                w.scenery_instances,
                w.fx_vertices
            )?;
        }
        Ok(())
    }

    pub(super) fn append_windows(&self, out: &mut String) {
        let mut windows = std::collections::BTreeMap::<u32, Vec<f32>>::new();
        for frame in &self.frames {
            windows.entry((frame.at_s / 60.0) as u32).or_default().push(frame.total_ms);
        }
        let _ = writeln!(out, "60-second windows by completion time (first/last may be partial):");
        for (minute, mut values) in windows {
            values.sort_by(f32::total_cmp);
            let n = values.len();
            let percentile = |p: f32| values[((n - 1) as f32 * p).round() as usize];
            let _ = writeln!(
                out,
                "  [{},{}): {n} frames, p50 {:.2}, p95 {:.2}, p99 {:.2}, max {:.2} ms; over25 {}, over50 {}, over100 {}",
                minute * 60,
                (minute + 1) * 60,
                percentile(0.5),
                percentile(0.95),
                percentile(0.99),
                percentile(1.0),
                values.iter().filter(|v| **v > 25.0).count(),
                values.iter().filter(|v| **v > 50.0).count(),
                values.iter().filter(|v| **v > 100.0).count()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_readback_is_opt_out_and_unavailable_samples_are_not_zero_measurements() {
        let mut log = FrameLog::new();
        for _ in 0..59 {
            log.end_frame(16.0);
        }
        assert!(log.gpu_sample_due());
        log.set_gpu_enabled(false);
        assert!(!log.gpu_sample_due());
        log.set_gpu_enabled(true);
        log.record_gpu_attempt(None);
        log.record_gpu_attempt(Some((20.0, vec![("scene_pass".into(), 14.0)])));
        assert_eq!(log.gpu_attempts, 2);
        assert_eq!(log.gpu.len(), 1);
        assert!(log.report().contains("gpu frame           20.000"));
    }

    #[test]
    fn capture_keeps_early_and_late_hitches_beyond_old_ring_and_hitch_limits() {
        let mut log = FrameLog::new();
        for index in 0..5000 {
            log.add_ms(Phase::Render, if index == 4999 { 900.0 } else { 30.0 });
            log.end_frame(if index == 4999 { 1200.0 } else { 31.0 });
            log.frames.back_mut().unwrap().at_s = index as f32 * 0.031;
        }
        assert_eq!(log.hitches().len(), 5000);
        let mut bytes = Vec::new();
        log.write_frames(&mut bytes).unwrap();
        let csv = String::from_utf8(bytes).unwrap();
        assert_eq!(csv.lines().count(), 5001);
        let columns = csv.lines().next().unwrap().split(',').count();
        assert!(csv.lines().all(|line| line.split(',').count() == columns));
        assert!(csv.lines().nth(1).unwrap().starts_with("1,"));
        assert!(csv.lines().last().unwrap().starts_with("5000,"));
        let report = log.report();
        assert!(report.contains("[0,60)") && report.contains("[120,180)"));
        assert!(report.contains("max 1200.00 ms"));
    }

    #[test]
    fn completion_clock_attributes_a_long_render_to_its_own_interval() {
        let mut log = FrameLog::new();
        let start = log.completed;
        log.add_ms(Phase::Render, 8.0);
        let first = log.complete_frame_at(start + std::time::Duration::from_millis(10));
        log.add_ms(Phase::Render, 100.0);
        let second = log.complete_frame_at(start + std::time::Duration::from_millis(120));
        assert!((first.total_ms - 10.0).abs() < 0.001);
        assert!((second.total_ms - 110.0).abs() < 0.001);
        assert_eq!(second.phase_ms(Phase::Render), 100.0);
        assert!((second.unattributed_ms() - 10.0).abs() < 0.001);
    }
}
