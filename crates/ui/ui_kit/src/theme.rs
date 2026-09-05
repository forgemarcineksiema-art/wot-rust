//! The theme (interface program F4/F6): steel, enamel and instrument glass as data.
//!
//! The old theme was nineteen flat constants around one amber accent; the owner's verdict on
//! that look is in `docs/interface-program.md`. [`Theme`] is a VALUE — plates, bevels, the
//! lamp, the size classes and a swappable [`Semantic`] block — so a colour-blind palette is a
//! field swap and a restyle is an edit here, never at a call site. The old `color::*` constants
//! and `tagged` stay as aliases while a legacy builder still reads them; each migrated element
//! stops reading them, and the F5 ratchet counts what is left.
//!
//! Two rules the locks hold: every label token clears WCAG contrast on every plate it can sit
//! on, and no semantic pair — ally/enemy, penetrated/held, module ok/destroyed — is carried by
//! hue alone, in any palette, under simulated colour vision deficiency.

/// Corner cut of a full-size panel, in clip-y units (`push_panel` aspect-corrects x so the cut
/// stays 45 degrees on screen). Legacy.
pub const CHAMFER_PANEL: f32 = 0.022;
/// Corner cut of a small interactive slot/button. Legacy.
pub const CHAMFER_SLOT: f32 = 0.012;
/// Thickness of a hairline rule, clip-y units. Legacy.
pub const HAIRLINE_THICKNESS: f32 = 0.0022;

/// A token's RGB with a caller-chosen alpha. The legacy HUD test suite tags features by exact
/// vertex-color equality, so two features that share a token must still differ in bytes —
/// the alpha is that tag. Retired with the last vertex-equality test (F5's ratchet).
pub const fn tagged(base: [f32; 4], alpha: f32) -> [f32; 4] {
    [base[0], base[1], base[2], alpha]
}

/// Straight RGBA in `[0, 1]`.
pub type Rgba = [f32; 4];

/// A plate material: the sheet tile it is cut from and the colour the tile modulates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateMaterial {
    pub tile: u32,
    pub color: Rgba,
}

/// The four materials a plate can be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plates {
    pub steel_brushed: PlateMaterial,
    pub steel_painted: PlateMaterial,
    pub enamel_black: PlateMaterial,
    pub glass: PlateMaterial,
}

/// Text tokens: what ink sits on the plates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextTokens {
    pub label: Rgba,
    pub label_dim: Rgba,
    pub value: Rgba,
    pub unit: Rgba,
    pub shadow: Rgba,
}

/// The size classes, in `u` (one pixel at 1080p): every text size in the interface is one of
/// these four, so a screen is set in four sizes and not in forty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeClasses {
    pub caption: f32,
    pub label: f32,
    pub value: f32,
    pub display: f32,
}

/// The penetration verdict's colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    pub pen: Rgba,
    pub no_pen: Rgba,
    pub ricochet: Rgba,
    pub shatter: Rgba,
}

/// The block a palette swaps: every colour that MEANS something.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Semantic {
    pub team_self: Rgba,
    pub team_ally: Rgba,
    pub team_enemy: Rgba,
    /// Indexed by `game_core::ShellType` order: AP, APCR, HEAT, HE.
    pub ammo: [Rgba; 4],
    /// Module ok, damaged, destroyed.
    pub module: [Rgba; 3],
    pub verdict: Verdict,
    /// Health ramp: full, half, low.
    pub hp_ramp: [Rgba; 3],
    /// The ONE red a commit wears: BATTLE, EXIT. Nothing else.
    pub commit: Rgba,
}

/// The palettes the semantic block comes in. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Standard,
    Deuteranopia,
    Protanopia,
    Tritanopia,
}

impl Palette {
    pub const ALL: [Palette; 4] =
        [Palette::Standard, Palette::Deuteranopia, Palette::Protanopia, Palette::Tritanopia];
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub plates: Plates,
    /// The lit rim of a plate, in `u`.
    pub bevel_u: f32,
    /// The inset of a pressed plate, in `u`.
    pub inset_u: f32,
    /// A hairline rule, in `u` (the emitter floors it at one pixel).
    pub hairline_u: f32,
    /// How much a pane of glass tints what it covers.
    pub glass_alpha: f32,
    /// Warm tungsten: the one glow, on live values and the focus ring.
    pub lamp: Rgba,
    pub text: TextTokens,
    pub sizes: SizeClasses,
    /// The keyboard focus ring, in `u`.
    pub focus_ring_u: f32,
    /// A hovered control's wash: mixed into its colour by this much.
    pub hover_wash: f32,
    /// A disabled control's alpha multiplier.
    pub disabled_alpha: f32,
    pub semantic: Semantic,
}

impl Theme {
    /// The standard look.
    pub fn standard() -> Self {
        use crate::sheet::SheetTile;
        Self {
            palette: Palette::Standard,
            plates: Plates {
                steel_brushed: PlateMaterial {
                    tile: SheetTile::BrushedSteel.index(),
                    color: [0.30, 0.31, 0.33, 0.94],
                },
                steel_painted: PlateMaterial {
                    tile: SheetTile::PaintedSteel.index(),
                    color: [0.24, 0.26, 0.22, 0.95],
                },
                enamel_black: PlateMaterial {
                    tile: SheetTile::EnamelBlack.index(),
                    color: [0.07, 0.07, 0.08, 0.96],
                },
                glass: PlateMaterial {
                    tile: SheetTile::GlassBand.index(),
                    color: [0.10, 0.12, 0.14, 0.35],
                },
            },
            bevel_u: 2.0,
            inset_u: 1.5,
            hairline_u: 1.0,
            glass_alpha: 0.12,
            lamp: [1.0, 0.86, 0.62, 1.0],
            text: TextTokens {
                label: [0.93, 0.91, 0.84, 0.97],
                label_dim: [0.66, 0.67, 0.62, 0.85],
                value: [0.98, 0.95, 0.86, 0.98],
                unit: [0.70, 0.71, 0.65, 0.70],
                shadow: [0.03, 0.04, 0.03, 0.60],
            },
            sizes: SizeClasses { caption: 11.0, label: 14.0, value: 18.0, display: 40.0 },
            focus_ring_u: 2.0,
            hover_wash: 0.10,
            disabled_alpha: 0.45,
            semantic: Semantic::standard(),
        }
    }

    /// The same theme with its semantic block in `palette`.
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self.semantic = Semantic::for_palette(palette);
        self
    }

    /// A colour washed toward the lamp by the hover amount: what a hovered control wears.
    pub fn washed(&self, color: Rgba) -> Rgba {
        let w = self.hover_wash;
        [
            color[0] + (self.lamp[0] - color[0]) * w,
            color[1] + (self.lamp[1] - color[1]) * w,
            color[2] + (self.lamp[2] - color[2]) * w,
            color[3],
        ]
    }
}

impl Semantic {
    /// The standard semantic block: blue allies, red enemies; brass for AP, white-brass for
    /// APCR, violet-grey for HEAT, olive for HE; green/amber/red modules; a verdict where
    /// penetration is bright and held is dark, so the pair survives every kind of colour vision.
    pub fn standard() -> Self {
        Self {
            team_self: [1.0, 0.94, 0.80, 1.0],
            team_ally: [0.40, 0.62, 0.95, 1.0],
            team_enemy: [0.86, 0.22, 0.18, 1.0],
            ammo: [
                [0.84, 0.66, 0.30, 1.0],
                [0.92, 0.88, 0.72, 1.0],
                [0.62, 0.56, 0.74, 1.0],
                [0.58, 0.64, 0.36, 1.0],
            ],
            module: [[0.45, 0.80, 0.42, 1.0], [0.95, 0.70, 0.20, 1.0], [0.62, 0.14, 0.12, 1.0]],
            verdict: Verdict {
                pen: [0.55, 0.92, 0.45, 1.0],
                no_pen: [0.55, 0.12, 0.10, 1.0],
                ricochet: [0.94, 0.94, 0.90, 1.0],
                shatter: [0.62, 0.62, 0.60, 1.0],
            },
            hp_ramp: [[0.45, 0.80, 0.42, 1.0], [0.95, 0.70, 0.20, 1.0], [0.85, 0.20, 0.16, 1.0]],
            commit: [0.72, 0.16, 0.12, 1.0],
        }
    }

    /// The block for `palette`. The colour-blind palettes keep every pair apart by LUMINANCE
    /// and by the blue axis, which survives the red-green deficiencies; tritanopia keeps the
    /// red axis instead.
    pub fn for_palette(palette: Palette) -> Self {
        let mut s = Self::standard();
        match palette {
            Palette::Standard => {}
            Palette::Deuteranopia | Palette::Protanopia => {
                s.team_ally = [0.30, 0.55, 1.0, 1.0];
                s.team_enemy = [0.95, 0.55, 0.10, 1.0];
                s.module =
                    [[0.30, 0.60, 1.0, 1.0], [0.95, 0.80, 0.30, 1.0], [0.40, 0.12, 0.08, 1.0]];
                s.verdict.pen = [0.55, 0.75, 1.0, 1.0];
                s.verdict.no_pen = [0.45, 0.14, 0.06, 1.0];
                s.hp_ramp =
                    [[0.30, 0.60, 1.0, 1.0], [0.95, 0.80, 0.30, 1.0], [0.55, 0.16, 0.08, 1.0]];
                s.ammo = [
                    [0.90, 0.70, 0.25, 1.0],
                    [0.96, 0.94, 0.86, 1.0],
                    [0.45, 0.50, 0.90, 1.0],
                    [0.30, 0.32, 0.30, 1.0],
                ];
            }
            Palette::Tritanopia => {
                // Blue and yellow fold together for a tritanope: the HEAT round goes dark
                // instead of violet, so the ammunition reads by luminance.
                s.ammo = [
                    [0.90, 0.68, 0.28, 1.0],
                    [0.96, 0.94, 0.86, 1.0],
                    [0.34, 0.28, 0.44, 1.0],
                    [0.55, 0.62, 0.34, 1.0],
                ];
                s.team_ally = [0.20, 0.72, 0.62, 1.0];
                s.team_enemy = [0.92, 0.18, 0.30, 1.0];
                s.module =
                    [[0.30, 0.78, 0.55, 1.0], [0.95, 0.60, 0.40, 1.0], [0.55, 0.10, 0.18, 1.0]];
                s.verdict.pen = [0.60, 0.95, 0.70, 1.0];
                s.verdict.no_pen = [0.50, 0.08, 0.14, 1.0];
                s.hp_ramp =
                    [[0.30, 0.78, 0.55, 1.0], [0.95, 0.60, 0.40, 1.0], [0.80, 0.16, 0.24, 1.0]];
            }
        }
        s
    }
}

pub mod color {
    //! Legacy tokens of the flat look, kept as aliases while the legacy builders read them.
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
    /// The old signal accent — selection, focus, attention. Amber.
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

/// Relative luminance of a straight sRGB colour, the WCAG way.
pub fn relative_luminance(c: Rgba) -> f32 {
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio between two colours, ignoring alpha.
pub fn contrast_ratio(a: Rgba, b: Rgba) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The colour a viewer with the given deficiency sees, by the Machado, Oliveira and Fernandes
/// (2009) matrices at full severity, applied in linear light.
pub fn simulate_deficiency(c: Rgba, palette: Palette) -> Rgba {
    let m: [[f32; 3]; 3] = match palette {
        Palette::Standard => return c,
        Palette::Protanopia => [
            [0.152_286, 1.052_583, -0.204_868],
            [0.114_503, 0.786_281, 0.099_216],
            [-0.003_882, -0.048_116, 1.051_998],
        ],
        Palette::Deuteranopia => [
            [0.367_322, 0.860_646, -0.227_968],
            [0.280_085, 0.672_501, 0.047_413],
            [-0.011_820, 0.042_940, 0.968_881],
        ],
        Palette::Tritanopia => [
            [1.255_528, -0.076_749, -0.178_779],
            [-0.078_411, 0.930_809, 0.147_602],
            [0.004_733, 0.691_367, 0.303_900],
        ],
    };
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let enc = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    let l = [lin(c[0]), lin(c[1]), lin(c[2])];
    let r = [
        m[0][0] * l[0] + m[0][1] * l[1] + m[0][2] * l[2],
        m[1][0] * l[0] + m[1][1] * l[1] + m[1][2] * l[2],
        m[2][0] * l[0] + m[2][1] * l[1] + m[2][2] * l[2],
    ];
    [enc(r[0]), enc(r[1]), enc(r[2]), c[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Every label token clears WCAG AA over every plate it can sit on (4.5:1 for text,
    /// 3:1 for the unit tags that sit beside a value), in every palette — the plates do not
    /// change with the palette, so this holds once.
    #[test]
    fn every_label_token_clears_contrast_on_every_plate() {
        let theme = Theme::standard();
        let plates =
            [theme.plates.steel_brushed, theme.plates.steel_painted, theme.plates.enamel_black];
        for plate in plates {
            for (name, token, floor) in [
                ("label", theme.text.label, 4.5),
                ("value", theme.text.value, 4.5),
                ("label_dim", theme.text.label_dim, 3.0),
                ("unit", theme.text.unit, 3.0),
            ] {
                let ratio = contrast_ratio(token, plate.color);
                assert!(
                    ratio >= floor,
                    "{name} over tile {} reads {ratio:.2}:1, floor {floor}",
                    plate.tile
                );
            }
        }
    }

    /// No semantic pair differs by hue alone: under every palette, simulated for the
    /// deficiency the palette is for, each pair keeps a luminance distance or a blue-axis
    /// distance a viewer can see.
    #[test]
    fn every_palette_keeps_the_semantic_pairs_apart() {
        for palette in Palette::ALL {
            let s = Semantic::for_palette(palette);
            let pairs = [
                ("ally/enemy", s.team_ally, s.team_enemy),
                ("pen/no_pen", s.verdict.pen, s.verdict.no_pen),
                ("module ok/destroyed", s.module[0], s.module[2]),
                ("hp full/low", s.hp_ramp[0], s.hp_ramp[2]),
                ("ammo AP/HEAT", s.ammo[0], s.ammo[2]),
            ];
            for (name, a, b) in pairs {
                let (sa, sb) = (simulate_deficiency(a, palette), simulate_deficiency(b, palette));
                let luminance = (relative_luminance(sa) - relative_luminance(sb)).abs();
                let chroma =
                    ((sa[0] - sb[0]).powi(2) + (sa[1] - sb[1]).powi(2) + (sa[2] - sb[2]).powi(2))
                        .sqrt();
                assert!(
                    luminance >= 0.20 || chroma >= 0.35,
                    "{palette:?}: {name} collapse under simulation — luminance gap {luminance:.2}, chroma {chroma:.2}"
                );
            }
        }
    }

    #[test]
    fn the_lamp_is_warmer_than_the_text() {
        let t = Theme::standard();
        assert!(t.lamp[0] > t.lamp[1] && t.lamp[1] > t.lamp[2], "tungsten: red > green > blue");
        assert!(t.lamp[0] / t.lamp[2] > t.text.value[0] / t.text.value[2], "warmer than the ink");
    }

    #[test]
    fn size_classes_are_ordered() {
        let s = Theme::standard().sizes;
        assert!(s.caption < s.label && s.label < s.value && s.value < s.display);
        assert!(s.caption >= 11.0, "no caption under 11 u: the two-second rule's floor");
    }

    #[test]
    fn a_palette_swap_changes_only_the_semantic_block() {
        let a = Theme::standard();
        let b = Theme::standard().with_palette(Palette::Deuteranopia);
        assert_eq!(a.plates, b.plates);
        assert_eq!(a.text, b.text);
        assert_ne!(a.semantic.team_enemy, b.semantic.team_enemy);
        assert_eq!(b.palette, Palette::Deuteranopia);
        assert_eq!(a.washed([0.0, 0.0, 0.0, 1.0])[0], a.lamp[0] * a.hover_wash);
    }
}
