//! The editor overlay, in the client's "military instrument" art direction (one toolkit,
//! one look): a status header, the contract report with the jump-to-problem cursor, a
//! document overview keyed to the 12 `TerrainMapLayer`s with REAL counts (never dead
//! stubs), and a help/readout footer. Pure view code over [`OverlayModel`] — testable
//! without a window.

use client::theme::{self, color};
use client::{push_hairline, push_panel, push_text, push_text_right};
use map_forge::Severity;
use renderer_api::HudVertex;

const TEXT_H: f32 = 0.038;
const LINE: f32 = 0.052;

/// Everything the overlay draws, gathered by the app once per frame.
pub struct OverlayModel {
    /// The armed brush readout for the footer (empty when navigating).
    pub brush_line: String,
    /// The tool inspector (stamp knobs, a selected object): a title plus `(line, active)`
    /// rows; empty rows hide the panel.
    pub inspector_title: String,
    pub stamp_lines: Vec<(String, bool)>,
    pub document_label: String,
    pub dirty: bool,
    pub compile_ms: f32,
    /// Report rows for display: identical messages collapse into one row with a count;
    /// `positions` holds each collapsed entry's index in the jump cycle
    /// (`markers::problem_positions` order — empty for entries with no world position).
    pub problems: Vec<ProblemRow>,
    /// The jump-to-problem cursor (indexes [`crate::markers::problem_positions`] order).
    pub selected_problem: Option<usize>,
    pub map_size_m: f32,
    pub layer_lines: Vec<(String, String)>,
    pub show_overview: bool,
    pub camera_line: String,
    pub probe_line: String,
    pub status: String,
}

/// One displayed report row (possibly a collapsed group of identical messages).
pub struct ProblemRow {
    pub severity: Severity,
    pub check: &'static str,
    pub message: String,
    pub positions: Vec<usize>,
}

/// Collapse a report into display rows: identical `(severity, check, message)` entries
/// group into one row; each positioned entry keeps its index in the jump cycle
/// ([`crate::markers::problem_positions`] order).
pub fn problem_rows(report: &map_forge::MapReport) -> Vec<ProblemRow> {
    let mut rows: Vec<ProblemRow> = Vec::new();
    let mut position_index = 0_usize;
    for entry in &report.entries {
        let position = entry.at.is_some().then(|| {
            let index = position_index;
            position_index += 1;
            index
        });
        if let Some(row) = rows.iter_mut().find(|row| {
            row.severity == entry.severity
                && row.check == entry.check
                && row.message == entry.message
        }) {
            row.positions.extend(position);
        } else {
            rows.push(ProblemRow {
                severity: entry.severity,
                check: entry.check,
                message: entry.message.clone(),
                positions: position.into_iter().collect(),
            });
        }
    }
    rows
}

pub fn overlay(model: &OverlayModel, aspect: f32) -> Vec<HudVertex> {
    let mut vertices = Vec::new();

    // Header: the document nameplate. The dirty star is the one amber accent up here.
    push_panel(
        &mut vertices,
        [0.0, 0.955],
        [1.0, 0.045],
        theme::CHAMFER_PANEL,
        aspect,
        color::PANEL,
    );
    push_hairline(&mut vertices, -1.0, 1.0, 0.908, color::HAIRLINE);
    let dirty = if model.dirty { " *" } else { "" };
    push_text(&mut vertices, &model.document_label, -0.985, 0.985, TEXT_H, aspect, color::TEXT);
    if model.dirty {
        let width = client::text_width(&model.document_label, TEXT_H, aspect);
        push_text(
            &mut vertices,
            dirty.trim(),
            -0.985 + width + 0.008,
            0.985,
            TEXT_H,
            aspect,
            color::ACCENT,
        );
    }
    let (errors, warnings) = problem_counts(model);
    let error_color = if errors > 0 { color::SIGNAL } else { color::TEXT_DIM };
    push_text_right(
        &mut vertices,
        &format!("{:.0} m   compile {:.1} ms", model.map_size_m, model.compile_ms),
        0.72,
        0.985,
        TEXT_H,
        aspect,
        color::VALUE,
    );
    push_text_right(
        &mut vertices,
        &format!("E {errors}"),
        0.86,
        0.985,
        TEXT_H,
        aspect,
        error_color,
    );
    push_text_right(
        &mut vertices,
        &format!("W {warnings}"),
        0.985,
        0.985,
        TEXT_H,
        aspect,
        if warnings > 0 { color::ACCENT_DIM } else { color::TEXT_DIM },
    );

    // The report: worst first, the selected problem carries the amber cursor (N cycles it,
    // the camera flies there).
    if !model.problems.is_empty() {
        let shown = model.problems.len().min(9);
        let rows = shown + usize::from(model.problems.len() > shown);
        // The panel hugs its text exactly: rows step by LINE, glyphs are TEXT_H tall.
        let content_h = (rows - 1) as f32 * LINE + TEXT_H;
        let top = 0.885;
        let first_text_top = top - 0.013;
        let panel_bottom = first_text_top - content_h - 0.013;
        push_panel(
            &mut vertices,
            [-0.63, (top + panel_bottom) * 0.5],
            [0.36, (top - panel_bottom) * 0.5],
            theme::CHAMFER_SLOT,
            aspect,
            color::PANEL,
        );
        let mut y = first_text_top;
        for row in model.problems.iter().take(shown) {
            let selected =
                model.selected_problem.is_some_and(|selected| row.positions.contains(&selected));
            let ink = match row.severity {
                Severity::Error => color::SIGNAL,
                Severity::Warning => color::ACCENT_DIM,
            };
            let count = if row.positions.len() > 1 {
                format!(" x{}", row.positions.len())
            } else {
                String::new()
            };
            let cursor = if selected { "> " } else { "  " };
            push_text(
                &mut vertices,
                &format!("{cursor}{}: {}{count}", row.check, row.message),
                -0.985,
                y,
                TEXT_H,
                aspect,
                if selected { color::ACCENT } else { ink },
            );
            y -= LINE;
        }
        if model.problems.len() > shown {
            push_text(
                &mut vertices,
                &format!("  ... {} more", model.problems.len() - shown),
                -0.985,
                y,
                TEXT_H,
                aspect,
                color::TEXT_DIM,
            );
        }
    }

    // The document overview (F1): the 12 terrain-map layers with live counts.
    if model.show_overview {
        // Header (title + hairline) then the lines; the panel hugs that content exactly.
        let content_h = 0.065 + (model.layer_lines.len().max(1) - 1) as f32 * LINE + TEXT_H;
        let top = 0.91;
        let bottom = top - content_h - 0.028;
        push_panel(
            &mut vertices,
            [0.735, (top + bottom) * 0.5],
            [0.26, (top - bottom) * 0.5],
            theme::CHAMFER_PANEL,
            aspect,
            color::PANEL,
        );
        push_text(&mut vertices, "DOCUMENT", 0.49, 0.895, TEXT_H, aspect, color::TEXT);
        push_hairline(&mut vertices, 0.49, 0.985, 0.845, color::HAIRLINE);
        let mut y = 0.83;
        for (label, value) in &model.layer_lines {
            push_text(&mut vertices, label, 0.49, y, TEXT_H, aspect, color::TEXT_DIM);
            push_text_right(&mut vertices, value, 0.985, y, TEXT_H, aspect, color::VALUE);
            y -= LINE;
        }
    }

    // The stamp inspector (bottom right, above the footer): the armed tool's knobs, the
    // amber row being the one Tab points at.
    if !model.stamp_lines.is_empty() {
        let rows = model.stamp_lines.len();
        let content_h = (rows - 1) as f32 * LINE + TEXT_H;
        // Anchored by the BOTTOM just above the footer; tall inspectors grow upward.
        let bottom = -0.895;
        let top = bottom + content_h + 0.026 + 0.05;
        push_panel(
            &mut vertices,
            [0.735, (top + bottom) * 0.5],
            [0.26, (top - bottom) * 0.5],
            theme::CHAMFER_PANEL,
            aspect,
            color::PANEL,
        );
        push_text(
            &mut vertices,
            &model.inspector_title,
            0.49,
            top - 0.005,
            TEXT_H,
            aspect,
            color::TEXT,
        );
        push_hairline(&mut vertices, 0.49, 0.985, top - 0.052, color::HAIRLINE);
        let mut y = top - 0.068;
        for (line, active) in &model.stamp_lines {
            let ink = if *active { color::ACCENT } else { color::TEXT_DIM };
            let prefix = if *active { "> " } else { "  " };
            push_text(&mut vertices, &format!("{prefix}{line}"), 0.49, y, TEXT_H, aspect, ink);
            y -= LINE;
        }
    }

    // Footer: help left, camera + probe readouts right — the instrument's needle values.
    push_panel(
        &mut vertices,
        [0.0, -0.958],
        [1.0, 0.042],
        theme::CHAMFER_PANEL,
        aspect,
        color::PANEL,
    );
    push_hairline(&mut vertices, -1.0, 1.0, -0.914, color::HAIRLINE);
    push_text(&mut vertices, &model.status, -0.985, -0.925, TEXT_H, aspect, color::TEXT_DIM);
    if !model.brush_line.is_empty() {
        push_text(&mut vertices, &model.brush_line, 0.30, -0.925, TEXT_H, aspect, color::ACCENT);
    }
    push_text_right(&mut vertices, &model.probe_line, 0.985, -0.925, TEXT_H, aspect, color::ACCENT);
    push_text_right(&mut vertices, &model.camera_line, 0.70, -0.925, TEXT_H, aspect, color::VALUE);

    vertices
}

fn problem_counts(model: &OverlayModel) -> (usize, usize) {
    let count = |wanted: Severity| {
        model
            .problems
            .iter()
            .filter(|row| row.severity == wanted)
            .map(|row| row.positions.len().max(1))
            .sum()
    };
    (count(Severity::Error), count(Severity::Warning))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_builds_and_scales_with_its_content() {
        let mut model = OverlayModel {
            brush_line: "brush raise  r 12 m".into(),
            inspector_title: "STAMP".into(),
            stamp_lines: vec![
                ("hill - click 1: centre".into(), false),
                ("height: 6.0 m".into(), true),
            ],
            document_label: "bystra-valley.map.ron".into(),
            dirty: true,
            compile_ms: 12.5,
            problems: vec![
                ProblemRow {
                    severity: Severity::Error,
                    check: "water_contract",
                    message: "mid-channel too shallow".into(),
                    positions: vec![0, 1],
                },
                ProblemRow {
                    severity: Severity::Warning,
                    check: "spawns",
                    message: "approach is steep".into(),
                    positions: Vec::new(),
                },
            ],
            selected_problem: Some(0),
            map_size_m: 1000.0,
            layer_lines: vec![("roads".into(), "4".into()), ("cover".into(), "132".into())],
            show_overview: true,
            camera_line: "512 431  h 61".into(),
            probe_line: "cursor 508, 430  h 8.2".into(),
            status: "F1 overview  N problem  Ctrl+P playtest".into(),
        };
        let with_everything = overlay(&model, 16.0 / 9.0).len();
        model.problems.clear();
        model.show_overview = false;
        let bare = overlay(&model, 16.0 / 9.0).len();
        assert!(with_everything > bare, "panels must appear with content");
        assert!(bare > 0, "the header and footer are always on");
    }
}
