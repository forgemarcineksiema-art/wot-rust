//! Visual tokens of the "military instrument" art direction: the UI reads like 1950s military
//! equipment — flat graphite panels with chamfered corners (an armor-plate cut), hairline rules,
//! warm off-white stencil markings, one amber signal accent, and deliberately no gradients, glow,
//! or gold. This module is the color analogue of `ui_strings`: every panel/readout tint flows
//! through these tokens so the look is retuned here, never at call sites. Semantic combat colors
//! (penetration green / no-pen red / health ramp) stay with their features — they carry meaning,
//! not style.

/// Corner cut of a full-size panel, in clip-y units (`push_panel` aspect-corrects x so the cut
/// stays 45 degrees on screen).
pub const CHAMFER_PANEL: f32 = 0.022;
/// Corner cut of a small interactive slot/button.
pub const CHAMFER_SLOT: f32 = 0.012;
/// Thickness of a hairline rule, clip-y units.
pub const HAIRLINE_THICKNESS: f32 = 0.0022;

/// A token's RGB with a caller-chosen alpha. The HUD test suite tags features by exact
/// vertex-color equality, so two features that share a token must still differ in bytes —
/// the alpha is that tag.
pub const fn tagged(base: [f32; 4], alpha: f32) -> [f32; 4] {
    [base[0], base[1], base[2], alpha]
}

pub mod color {
    /// Flat graphite panel fill.
    pub const PANEL: [f32; 4] = [0.085, 0.090, 0.095, 0.86];
    /// Hairline rule separating panel zones (under headers, between groups).
    pub const HAIRLINE: [f32; 4] = [0.78, 0.80, 0.75, 0.22];
    /// Primary markings: warm off-white, like stenciled paint on steel.
    pub const TEXT: [f32; 4] = [0.92, 0.92, 0.87, 0.96];
    /// Secondary markings and labels.
    pub const TEXT_DIM: [f32; 4] = [0.64, 0.66, 0.61, 0.80];
    /// Bright numeric readouts (the "needle" values on an instrument).
    pub const VALUE: [f32; 4] = [0.95, 0.93, 0.85, 0.95];
    /// The one signal accent — selection, focus, attention. Amber, used sparingly.
    pub const ACCENT: [f32; 4] = [0.95, 0.65, 0.15, 0.95];
    /// Dimmed accent for selected-but-not-hot surfaces.
    pub const ACCENT_DIM: [f32; 4] = [0.55, 0.40, 0.14, 0.93];
    /// Commit/danger red (battle button, rejected fit).
    pub const SIGNAL: [f32; 4] = [0.70, 0.19, 0.14, 0.96];
    /// Interactive slot fill at rest.
    pub const SLOT: [f32; 4] = [0.140, 0.145, 0.150, 0.92];
    /// Slot holding the current selection.
    pub const SLOT_SELECTED: [f32; 4] = ACCENT_DIM;
    /// Slot holding keyboard focus.
    pub const SLOT_FOCUSED: [f32; 4] = [0.30, 0.25, 0.13, 0.93];
    /// Slot flashing a rejected action.
    pub const REJECTED: [f32; 4] = [0.45, 0.13, 0.10, 0.92];
    /// Translucent wash over whatever clickable element the cursor is on.
    pub const HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.08];
    /// Icon tints.
    pub const ICON: [f32; 4] = [0.86, 0.87, 0.82, 0.95];
    pub const ICON_DIM: [f32; 4] = [0.60, 0.62, 0.58, 0.80];

    /// Drop shadow under every glyph: what keeps stencil markings legible over a sunlit
    /// field. Alpha here is the base; it scales with the text's own alpha.
    pub const TEXT_SHADOW: [f32; 4] = [0.03, 0.04, 0.03, 0.60];

    // Battle readouts (referenced by `hud/number.rs`).
    /// Primary battle values: speed, HP, target distance.
    pub const READOUT: [f32; 4] = [0.93, 0.92, 0.86, 0.94];
    /// Ambient diagnostics that should not compete for attention (FPS, zoom).
    pub const READOUT_SOFT: [f32; 4] = [0.74, 0.76, 0.70, 0.75];
    /// Unit tags next to values (KM/H, M).
    pub const UNIT: [f32; 4] = [0.68, 0.70, 0.64, 0.60];
}

#[cfg(test)]
mod tests {
    use super::color;

    #[test]
    fn tokens_are_valid_premultipliable_rgba() {
        let all = [
            color::PANEL,
            color::HAIRLINE,
            color::TEXT,
            color::TEXT_DIM,
            color::VALUE,
            color::ACCENT,
            color::ACCENT_DIM,
            color::SIGNAL,
            color::SLOT,
            color::SLOT_SELECTED,
            color::SLOT_FOCUSED,
            color::REJECTED,
            color::HOVER,
            color::ICON,
            color::ICON_DIM,
            color::READOUT,
            color::READOUT_SOFT,
            color::UNIT,
            color::TEXT_SHADOW,
        ];
        for c in all {
            assert!(c.iter().all(|ch| (0.0..=1.0).contains(ch)), "channel out of range: {c:?}");
        }
    }

    /// The instrument look depends on contrast discipline: panels stay dark so markings read,
    /// and the accent is warmer than it is bright (amber, not white-hot). Locking constants is
    /// the point here, so the constant-assertion lint is deliberately silenced.
    #[test]
    #[expect(clippy::assertions_on_constants)]
    fn panels_stay_dark_and_markings_stay_bright() {
        let luma = |c: [f32; 4]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        assert!(luma(color::PANEL) < 0.15, "panel must read as dark graphite");
        assert!(luma(color::SLOT) < 0.20, "slots stay near-panel dark");
        assert!(luma(color::TEXT) > 0.80, "primary markings must be bright off-white");
        assert!(
            color::ACCENT[0] > color::ACCENT[1] && color::ACCENT[1] > color::ACCENT[2],
            "the accent is amber: red > green > blue"
        );
    }
}
