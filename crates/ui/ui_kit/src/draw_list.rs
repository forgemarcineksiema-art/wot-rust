//! The semantic draw list (interface program F4/F5): what the screen IS, before it is triangles.
//!
//! A screen is a list of elements, each keyed by a name the tests and the hit test can ask for,
//! each with a rectangle in pixels, a depth, a clip, an interaction state and a payload — a
//! plate, a run of text, an icon, a bar, a pane of glass, or a legacy batch of vertices carried
//! verbatim (the reticle stack rides that way for as long as the program says). ONE emitter
//! turns the list into `HudVertex` triangles; ONE hit test reads the same rectangles the
//! emitter drew from, so what is lit under the cursor is exactly what a click activates.
//!
//! The list is rebuilt every frame, like the HUD always was. There is no retained tree.

use std::hash::Hash;

use renderer_api::{HudVertex, hud_style};

use crate::font::{self, Style};
use crate::icons::HudIcon;
use crate::rect::Rect;
use crate::theme::Theme;
use crate::ui::Ui;

/// Anything that can name an element: an app's own `enum`, `Copy` and comparable.
pub trait ElementKey: Copy + Eq + Hash + std::fmt::Debug {}
impl<T: Copy + Eq + Hash + std::fmt::Debug> ElementKey for T {}

/// How an interactive element is drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetState {
    #[default]
    Idle,
    Hover,
    Pressed,
    Focused,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Whether digits share one fixed cell so a counter never jitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DigitMode {
    #[default]
    Proportional,
    Tabular,
}

/// What an element draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    /// A plate cut from the sheet tile, rounded by `radius_u`, lit on a `bevel_u` rim.
    Plate { tile: u32, radius_u: f32, bevel_u: f32, color: [f32; 4] },
    /// A run of text set in `style` at `size_u`, aligned inside the element's rectangle and
    /// centred on its height.
    Text {
        text: String,
        style: Style,
        size_u: f32,
        align: Align,
        color: [f32; 4],
        digits: DigitMode,
    },
    /// An icon filling the element's height.
    Icon { icon: HudIcon, color: [f32; 4] },
    /// A bar: `back` under the whole rectangle, `fill` over its left `frac`.
    Bar { frac: f32, fill: [f32; 4], back: [f32; 4] },
    /// A pane of glass over the rectangle: a tint with a reflection band at `phase`.
    Glass { radius_u: f32, phase: f32, color: [f32; 4] },
    /// Clip-space vertices appended VERBATIM, never clipped, never restyled: the reticle stack
    /// and every legacy builder during the migration.
    Legacy(Vec<HudVertex>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element<K> {
    pub id: K,
    /// Where it is, in physical pixels — the rectangle the hit test reads.
    pub rect: Rect,
    /// Painter's order: higher draws later. Ties keep insertion order.
    pub z: i16,
    /// Everything outside this rectangle is cut away (legacy payloads excepted).
    pub clip: Option<Rect>,
    pub state: WidgetState,
    /// Whether the hit test may answer with this element.
    pub interactive: bool,
    pub payload: Payload,
}

impl<K: ElementKey> Element<K> {
    pub fn new(id: K, rect: Rect, payload: Payload) -> Self {
        Self { id, rect, z: 0, clip: None, state: WidgetState::Idle, interactive: false, payload }
    }

    pub fn z(mut self, z: i16) -> Self {
        self.z = z;
        self
    }

    pub fn interactive(mut self, state: WidgetState) -> Self {
        self.interactive = true;
        self.state = state;
        self
    }

    pub fn clipped(mut self, clip: Option<Rect>) -> Self {
        self.clip = clip;
        self
    }

    /// The rectangle the hit test and the emitter use: the element's own, cut by its clip.
    pub fn visible_rect(&self) -> Option<Rect> {
        match self.clip {
            Some(clip) => self.rect.intersect(&clip),
            None => (!self.rect.is_empty()).then_some(self.rect),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawList<K> {
    elements: Vec<Element<K>>,
}

impl<K: ElementKey> Default for DrawList<K> {
    fn default() -> Self {
        Self { elements: Vec::new() }
    }
}

impl<K: ElementKey> DrawList<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, element: Element<K>) -> &mut Element<K> {
        self.elements.push(element);
        self.elements.last_mut().expect("just pushed")
    }

    pub fn find(&self, id: K) -> Option<&Element<K>> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Element<K>> {
        self.elements.iter()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// The interactive element under `point_px`: the highest z wins, the later-pushed on a tie
    /// — the same order the emitter paints, so the one the eye sees is the one that answers.
    pub fn hit(&self, point_px: [f32; 2]) -> Option<K> {
        self.elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.interactive)
            .filter(|(_, e)| e.visible_rect().is_some_and(|r| r.contains(point_px)))
            .max_by_key(|(index, e)| (e.z, *index))
            .map(|(_, e)| e.id)
    }

    /// The one emitter: every element to triangles, in painter's order, clipped.
    pub fn emit(&self, ui: &Ui, theme: &Theme) -> Vec<HudVertex> {
        let mut order: Vec<usize> = (0..self.elements.len()).collect();
        order.sort_by_key(|&i| self.elements[i].z);
        let mut vertices = Vec::new();
        for index in order {
            let element = &self.elements[index];
            let start = vertices.len();
            emit_element(element, ui, theme, &mut vertices);
            if let (Some(clip), false) =
                (element.clip, matches!(element.payload, Payload::Legacy(_)))
            {
                let clip_rect =
                    [ui.to_clip([clip.x, clip.bottom()]), ui.to_clip([clip.right(), clip.y])];
                clip_quads(&mut vertices, start, clip_rect);
            }
        }
        vertices
    }
}

fn emit_element<K: ElementKey>(
    element: &Element<K>,
    ui: &Ui,
    theme: &Theme,
    out: &mut Vec<HudVertex>,
) {
    let rect = element.rect;
    if rect.is_empty() && !matches!(element.payload, Payload::Legacy(_)) {
        return;
    }
    let state = element.state;
    let dim = |c: [f32; 4]| match state {
        WidgetState::Disabled => [c[0], c[1], c[2], c[3] * theme.disabled_alpha],
        WidgetState::Hover => theme.washed(c),
        _ => c,
    };
    match &element.payload {
        Payload::Plate { tile, radius_u, bevel_u, color } => {
            if state == WidgetState::Focused {
                let ring = rect.inset(-ui.px(theme.focus_ring_u));
                push_plate(
                    out,
                    ui,
                    ring,
                    ui.px(*radius_u + theme.focus_ring_u),
                    0.0,
                    *tile,
                    theme.lamp,
                );
            }
            let bevel = if state == WidgetState::Pressed {
                -ui.px(*bevel_u).abs()
            } else {
                ui.px(*bevel_u)
            };
            let color = if state == WidgetState::Pressed {
                [color[0] * 0.85, color[1] * 0.85, color[2] * 0.85, color[3]]
            } else {
                dim(*color)
            };
            push_plate(out, ui, rect, ui.px(*radius_u), bevel, *tile, color);
        }
        Payload::Text { text, style, size_u, align, color, digits } => {
            let size_px = ui.px(*size_u);
            let height = size_px * ui.clip_per_px();
            let aspect = ui.aspect();
            let width = match digits {
                DigitMode::Tabular => font::text_width_tabular(*style, text, height, aspect),
                DigitMode::Proportional => font::text_width_styled(*style, text, height, aspect),
            };
            let left_px = match align {
                Align::Left => rect.x,
                Align::Center => rect.x + (rect.w - width / ui.clip_per_px() * aspect) * 0.5,
                Align::Right => rect.right() - width / ui.clip_per_px() * aspect,
            };
            let top_px = rect.y + (rect.h - size_px) * 0.5;
            let [left, top] = ui.to_clip([left_px, top_px]);
            let color = dim(*color);
            match digits {
                DigitMode::Tabular => {
                    font::push_text_tabular(out, *style, text, left, top, height, aspect, color)
                }
                DigitMode::Proportional => {
                    font::push_text_styled(out, *style, text, left, top, height, aspect, color)
                }
            }
        }
        Payload::Icon { icon, color } => {
            let size = rect.h * ui.clip_per_px();
            let [left, top] = ui.to_clip([rect.x, rect.y]);
            font::push_icon(out, *icon, left, top, size, ui.aspect(), dim(*color));
        }
        Payload::Bar { frac, fill, back } => {
            push_solid_rect(out, ui, rect, dim(*back));
            let filled = Rect::new(rect.x, rect.y, rect.w * frac.clamp(0.0, 1.0), rect.h);
            if !filled.is_empty() {
                push_solid_rect(out, ui, filled, dim(*fill));
            }
        }
        Payload::Glass { radius_u, phase, color } => {
            let extent = [rect.w * 0.5, rect.h * 0.5];
            let corners = quad_corners(rect);
            let radius = ui.px(*radius_u);
            let color = dim(*color);
            for (px, local) in corners {
                out.push(HudVertex::glass(ui.to_clip(px), local, extent, radius, *phase, color));
            }
        }
        Payload::Legacy(vertices) => out.extend_from_slice(vertices),
    }
}

/// The six corners of a rectangle as (pixel position, local offset) in the emitter's quad
/// order: top-left, bottom-left, bottom-right, top-left, bottom-right, top-right.
fn quad_corners(rect: Rect) -> [([f32; 2], [f32; 2]); 6] {
    let tl = ([rect.x, rect.y], [0.0, 0.0]);
    let bl = ([rect.x, rect.bottom()], [0.0, rect.h]);
    let br = ([rect.right(), rect.bottom()], [rect.w, rect.h]);
    let tr = ([rect.right(), rect.y], [rect.w, 0.0]);
    [tl, bl, br, tl, br, tr]
}

fn push_plate(
    out: &mut Vec<HudVertex>,
    ui: &Ui,
    rect: Rect,
    radius_px: f32,
    bevel_px: f32,
    tile: u32,
    color: [f32; 4],
) {
    let extent = [rect.w * 0.5, rect.h * 0.5];
    for (px, local) in quad_corners(rect) {
        out.push(HudVertex::plate(ui.to_clip(px), local, extent, radius_px, bevel_px, tile, color));
    }
}

fn push_solid_rect(out: &mut Vec<HudVertex>, ui: &Ui, rect: Rect, color: [f32; 4]) {
    for (px, _) in quad_corners(rect) {
        out.push(HudVertex::new(ui.to_clip(px), color));
    }
}

/// Cut every axis-aligned quad emitted since `start` to `clip` (clip space, `[min, max]`),
/// re-mapping the vertex attributes that vary across the quad — the atlas uv and the plate's
/// local coordinate — so a clipped glyph keeps its texel density and a clipped plate its
/// corners. A quad entirely outside is dropped. The draw stays one call: this is the CPU's
/// scissor.
fn clip_quads(vertices: &mut Vec<HudVertex>, start: usize, clip: [[f32; 2]; 2]) {
    let mut kept: Vec<HudVertex> = Vec::with_capacity(vertices.len() - start);
    let quads: Vec<[HudVertex; 6]> =
        vertices[start..].chunks_exact(6).map(|c| [c[0], c[1], c[2], c[3], c[4], c[5]]).collect();
    for quad in quads {
        let min = [
            quad.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min),
            quad.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min),
        ];
        let max = [
            quad.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max),
            quad.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max),
        ];
        let cmin = [min[0].max(clip[0][0]), min[1].max(clip[0][1])];
        let cmax = [max[0].min(clip[1][0]), max[1].min(clip[1][1])];
        if cmin[0] >= cmax[0] || cmin[1] >= cmax[1] {
            continue;
        }
        if cmin == min && cmax == max {
            kept.extend_from_slice(&quad);
            continue;
        }
        // The attributes at the quad's two extreme corners give the linear map across it.
        let at_min = quad.iter().find(|v| v.position[0] == min[0] && v.position[1] == min[1]);
        let at_max = quad.iter().find(|v| v.position[0] == max[0] && v.position[1] == max[1]);
        let (Some(a), Some(b)) = (at_min, at_max) else {
            kept.extend_from_slice(&quad);
            continue;
        };
        let span = [(max[0] - min[0]).max(1e-6), (max[1] - min[1]).max(1e-6)];
        for v in quad {
            let mut w = v;
            w.position =
                [v.position[0].clamp(cmin[0], cmax[0]), v.position[1].clamp(cmin[1], cmax[1])];
            let t = [(w.position[0] - min[0]) / span[0], (w.position[1] - min[1]) / span[1]];
            if v.uv[0] >= 0.0 {
                w.uv = [a.uv[0] + (b.uv[0] - a.uv[0]) * t[0], a.uv[1] + (b.uv[1] - a.uv[1]) * t[1]];
            }
            w.local = [
                a.local[0] + (b.local[0] - a.local[0]) * t[0],
                a.local[1] + (b.local[1] - a.local[1]) * t[1],
            ];
            kept.push(w);
        }
    }
    vertices.truncate(start);
    vertices.extend(kept);
}

/// The kinds a plate can be cut from, by sheet tile index, for readers of the list.
pub fn plate_tile(style: u32) -> u32 {
    hud_style::tile(style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::SheetTile;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Id {
        Panel,
        Label,
        Button,
        Overlay,
    }

    fn plate(color: [f32; 4]) -> Payload {
        Payload::Plate { tile: SheetTile::BrushedSteel.index(), radius_u: 6.0, bevel_u: 2.0, color }
    }

    #[test]
    fn the_hit_test_reads_the_rects_the_emitter_draws_from() {
        let mut list = DrawList::new();
        list.push(
            Element::new(Id::Panel, Rect::new(0.0, 0.0, 400.0, 300.0), plate([0.2; 4]))
                .interactive(WidgetState::Idle),
        );
        list.push(
            Element::new(Id::Button, Rect::new(100.0, 100.0, 80.0, 40.0), plate([0.3; 4]))
                .z(1)
                .interactive(WidgetState::Idle),
        );
        list.push(
            Element::new(
                Id::Label,
                Rect::new(100.0, 100.0, 80.0, 40.0),
                Payload::Text {
                    text: "OK".into(),
                    style: Style::LABEL,
                    size_u: 14.0,
                    align: Align::Center,
                    color: [1.0; 4],
                    digits: DigitMode::Proportional,
                },
            )
            .z(2),
        );
        assert_eq!(
            list.hit([120.0, 120.0]),
            Some(Id::Button),
            "the higher z wins; the label is not interactive"
        );
        assert_eq!(list.hit([10.0, 10.0]), Some(Id::Panel));
        assert_eq!(list.hit([900.0, 900.0]), None);
        assert_eq!(list.find(Id::Button).map(|e| e.rect.w), Some(80.0));
    }

    #[test]
    fn a_clipped_element_answers_only_inside_its_clip() {
        let mut list = DrawList::new();
        let clip = Some(Rect::new(0.0, 0.0, 50.0, 50.0));
        list.push(
            Element::new(Id::Button, Rect::new(0.0, 0.0, 200.0, 200.0), plate([0.3; 4]))
                .interactive(WidgetState::Idle)
                .clipped(clip),
        );
        assert_eq!(list.hit([25.0, 25.0]), Some(Id::Button));
        assert_eq!(list.hit([100.0, 100.0]), None, "outside the clip the element is not there");
    }

    #[test]
    fn the_emitter_paints_in_z_order_and_a_legacy_batch_rides_verbatim() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let legacy = vec![HudVertex::new([0.1, 0.1], [1.0, 0.0, 0.0, 1.0]); 3];
        let mut list = DrawList::new();
        list.push(Element::new(Id::Overlay, Rect::default(), Payload::Legacy(legacy.clone())).z(5));
        list.push(
            Element::new(Id::Panel, Rect::new(0.0, 0.0, 100.0, 100.0), plate([0.2, 0.2, 0.2, 1.0]))
                .z(0),
        );
        let vertices = list.emit(&ui, &theme);
        assert_eq!(vertices.len(), 6 + 3);
        assert_eq!(
            hud_style::kind(vertices[0].style),
            hud_style::PLATE,
            "the plate paints first (z 0)"
        );
        assert_eq!(
            &vertices[6..],
            &legacy[..],
            "the legacy batch is appended untouched, last (z 5)"
        );
        assert_eq!(vertices[0].extent, [50.0, 50.0]);
        assert_eq!(hud_style::tile(vertices[0].style), SheetTile::BrushedSteel.index());
    }

    #[test]
    fn a_pressed_plate_is_inset_and_a_focused_one_wears_the_lamp_ring() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let rect = Rect::new(0.0, 0.0, 100.0, 40.0);
        let mut pressed = DrawList::new();
        pressed.push(
            Element::new(Id::Button, rect, plate([0.5, 0.5, 0.5, 1.0]))
                .interactive(WidgetState::Pressed),
        );
        let v = pressed.emit(&ui, &theme);
        assert!(v[0].params[1] < 0.0, "a pressed plate's bevel is negative: an inset");
        assert!(v[0].color[0] < 0.5, "a pressed plate darkens");

        let mut focused = DrawList::new();
        focused.push(
            Element::new(Id::Button, rect, plate([0.5, 0.5, 0.5, 1.0]))
                .interactive(WidgetState::Focused),
        );
        let v = focused.emit(&ui, &theme);
        assert_eq!(v.len(), 12, "the ring is a second plate under the first");
        assert_eq!(v[0].color, theme.lamp, "the ring wears the lamp");
        assert!(v[0].extent[0] > 50.0, "the ring is larger than the plate");

        let mut disabled = DrawList::new();
        disabled.push(
            Element::new(Id::Button, rect, plate([0.5, 0.5, 0.5, 1.0]))
                .interactive(WidgetState::Disabled),
        );
        let v = disabled.emit(&ui, &theme);
        assert!(v[0].color[3] < 0.5, "a disabled plate fades");
    }

    #[test]
    fn a_clipped_glyph_keeps_its_texel_density() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let rect = Rect::new(100.0, 100.0, 400.0, 40.0);
        let full = {
            let mut list = DrawList::new();
            list.push(Element::new(
                Id::Label,
                rect,
                Payload::Text {
                    text: "8".into(),
                    style: Style::VALUE,
                    size_u: 40.0,
                    align: Align::Left,
                    color: [1.0; 4],
                    digits: DigitMode::Proportional,
                },
            ));
            list.emit(&ui, &theme)
        };
        let glyph = &full[6..12]; // the shadow quad comes first, then the glyph
        let x_min = glyph.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        let x_max = glyph.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        let px_min = (x_min + 1.0) * 0.5 * 1920.0;
        let px_max = (x_max + 1.0) * 0.5 * 1920.0;
        let cut = Rect::new(0.0, 0.0, (px_min + px_max) * 0.5, 1080.0);
        let clipped = {
            let mut list = DrawList::new();
            list.push(
                Element::new(
                    Id::Label,
                    rect,
                    Payload::Text {
                        text: "8".into(),
                        style: Style::VALUE,
                        size_u: 40.0,
                        align: Align::Left,
                        color: [1.0; 4],
                        digits: DigitMode::Proportional,
                    },
                )
                .clipped(Some(cut)),
            );
            list.emit(&ui, &theme)
        };
        let clipped_glyph = &clipped[6..12];
        let cx_max = clipped_glyph.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        assert!(cx_max < x_max && cx_max > x_min, "the glyph was cut at the clip's edge");
        // Texel density: uv per clip unit is the same before and after the cut.
        let density = |q: &[HudVertex]| {
            let u_min = q.iter().map(|v| v.uv[0]).fold(f32::MAX, f32::min);
            let u_max = q.iter().map(|v| v.uv[0]).fold(f32::MIN, f32::max);
            let p_min = q.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
            let p_max = q.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
            (u_max - u_min) / (p_max - p_min)
        };
        assert!((density(glyph) - density(clipped_glyph)).abs() < 1e-3);
    }

    #[test]
    fn a_quad_entirely_outside_the_clip_is_dropped() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let mut list = DrawList::new();
        list.push(
            Element::new(Id::Panel, Rect::new(500.0, 500.0, 100.0, 100.0), plate([0.2; 4]))
                .clipped(Some(Rect::new(0.0, 0.0, 50.0, 50.0))),
        );
        assert!(list.emit(&ui, &theme).is_empty());
    }

    #[test]
    fn a_bar_fills_its_fraction_from_the_left() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let mut list = DrawList::new();
        list.push(Element::new(
            Id::Panel,
            Rect::new(0.0, 0.0, 200.0, 10.0),
            Payload::Bar { frac: 0.25, fill: [1.0; 4], back: [0.0, 0.0, 0.0, 1.0] },
        ));
        let v = list.emit(&ui, &theme);
        assert_eq!(v.len(), 12);
        let fill_right = v[6..].iter().map(|x| x.position[0]).fold(f32::MIN, f32::max);
        let back_right = v[..6].iter().map(|x| x.position[0]).fold(f32::MIN, f32::max);
        assert!((fill_right - (-1.0 + 50.0 / 960.0)).abs() < 1e-5);
        assert!((back_right - (-1.0 + 200.0 / 960.0)).abs() < 1e-5);
    }
}
