//! The `Ui` context (interface program F4): the viewport in physical pixels, the unit every
//! size is written in, the clip stack, and the one conversion to clip space.
//!
//! **The unit `u`** is one pixel at 1080p times the user's UI scale, so that
//! `px = u * viewport_h / 1080 * user_scale`. Every theme size is in `u`: a caption that is 11 px
//! on a 1080p monitor is 22 px on a 4K one and 16.5 px when the user asks for 1.5×. The window
//! lane hands the DPI in through the viewport's physical size; nothing here reads a scale factor
//! of its own.

use crate::rect::Rect;

/// The height, in pixels, at which one `u` is one pixel.
pub const REFERENCE_HEIGHT_PX: f32 = 1080.0;

/// Where an element hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];

    /// The anchor's point on a unit square: `(0, 0)` top-left, `(1, 1)` bottom-right.
    fn fraction(self) -> [f32; 2] {
        match self {
            Anchor::TopLeft => [0.0, 0.0],
            Anchor::Top => [0.5, 0.0],
            Anchor::TopRight => [1.0, 0.0],
            Anchor::Left => [0.0, 0.5],
            Anchor::Center => [0.5, 0.5],
            Anchor::Right => [1.0, 0.5],
            Anchor::BottomLeft => [0.0, 1.0],
            Anchor::Bottom => [0.5, 1.0],
            Anchor::BottomRight => [1.0, 1.0],
        }
    }
}

/// What a frame's layout runs against.
#[derive(Debug, Clone)]
pub struct Ui {
    viewport_px: [f32; 2],
    user_scale: f32,
    clip: Vec<Rect>,
}

impl Ui {
    /// A context for a `width` x `height` physical-pixel viewport at the user's UI scale.
    pub fn new(width_px: u32, height_px: u32, user_scale: f32) -> Self {
        Self {
            viewport_px: [width_px.max(1) as f32, height_px.max(1) as f32],
            user_scale: user_scale.clamp(0.5, 3.0),
            clip: Vec::new(),
        }
    }

    /// The 1080p context at scale one: what the goldens and the unit tests lay out in.
    pub fn reference() -> Self {
        Self::new(1920, 1080, 1.0)
    }

    /// A 1080-tall context of the given aspect at scale one: what a legacy builder that only
    /// knows an aspect ratio lays out in (its payloads are clip space and ignore the context).
    pub fn for_aspect(aspect: f32) -> Self {
        Self::new((REFERENCE_HEIGHT_PX * aspect.max(0.1)).round() as u32, 1080, 1.0)
    }

    pub fn viewport(&self) -> Rect {
        Rect::new(0.0, 0.0, self.viewport_px[0], self.viewport_px[1])
    }

    pub fn aspect(&self) -> f32 {
        self.viewport_px[0] / self.viewport_px[1]
    }

    pub fn user_scale(&self) -> f32 {
        self.user_scale
    }

    /// Pixels per `u`.
    pub fn scale(&self) -> f32 {
        self.viewport_px[1] / REFERENCE_HEIGHT_PX * self.user_scale
    }

    /// `u` to physical pixels.
    pub fn px(&self, u: f32) -> f32 {
        u * self.scale()
    }

    /// Physical pixels to `u`.
    pub fn u(&self, px: f32) -> f32 {
        px / self.scale()
    }

    /// A `size_u` rectangle hung from `anchor` of the viewport, moved `offset_u` inward from
    /// the anchored edges (an offset moves a top-right anchor left and down).
    pub fn anchor(&self, anchor: Anchor, size_u: [f32; 2], offset_u: [f32; 2]) -> Rect {
        self.anchor_in(&self.viewport(), anchor, size_u, offset_u)
    }

    /// The same, hung from `anchor` of an arbitrary `frame`.
    pub fn anchor_in(
        &self,
        frame: &Rect,
        anchor: Anchor,
        size_u: [f32; 2],
        offset_u: [f32; 2],
    ) -> Rect {
        let w = self.px(size_u[0]);
        let h = self.px(size_u[1]);
        let f = anchor.fraction();
        let ox = self.px(offset_u[0]) * (1.0 - 2.0 * f[0]);
        let oy = self.px(offset_u[1]) * (1.0 - 2.0 * f[1]);
        let x = frame.x + (frame.w - w) * f[0] + ox;
        let y = frame.y + (frame.h - h) * f[1] + oy;
        Rect::new(x, y, w, h)
    }

    /// Restrict everything pushed until the matching `pop_clip` to `rect` (intersected with
    /// the clip already in force).
    pub fn push_clip(&mut self, rect: Rect) {
        let effective = match self.clip.last() {
            Some(outer) => outer.intersect(&rect).unwrap_or(Rect::new(rect.x, rect.y, 0.0, 0.0)),
            None => rect,
        };
        self.clip.push(effective);
    }

    pub fn pop_clip(&mut self) {
        self.clip.pop();
    }

    /// The clip in force, if any.
    pub fn clip(&self) -> Option<Rect> {
        self.clip.last().copied()
    }

    /// A pixel position to clip space (`[-1, 1]`, y up).
    pub fn to_clip(&self, px: [f32; 2]) -> [f32; 2] {
        [px[0] / self.viewport_px[0] * 2.0 - 1.0, 1.0 - px[1] / self.viewport_px[1] * 2.0]
    }

    /// A pixel length along y to clip units (the HUD's height unit).
    pub fn clip_per_px(&self) -> f32 {
        2.0 / self.viewport_px[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_u_is_one_px_at_1080p_and_scales_with_the_user() {
        assert_eq!(Ui::new(1920, 1080, 1.0).px(14.0), 14.0);
        assert_eq!(Ui::new(3840, 2160, 1.0).px(14.0), 28.0);
        assert_eq!(Ui::new(1920, 1080, 1.5).px(10.0), 15.0);
        assert_eq!(Ui::new(1280, 720, 1.0).u(10.0), 15.0);
    }

    #[test]
    fn an_anchor_at_every_corner_stays_inside_the_viewport() {
        let ui = Ui::reference();
        for anchor in Anchor::ALL {
            let rect = ui.anchor(anchor, [200.0, 100.0], [16.0, 16.0]);
            assert!(ui.viewport().encloses(&rect), "{anchor:?} left the viewport: {rect:?}");
            assert_eq!(rect.size(), [200.0, 100.0]);
        }
        let br = ui.anchor(Anchor::BottomRight, [200.0, 100.0], [16.0, 16.0]);
        assert_eq!([br.right(), br.bottom()], [1920.0 - 16.0, 1080.0 - 16.0]);
        let c = ui.anchor(Anchor::Center, [200.0, 100.0], [0.0, 0.0]);
        assert_eq!(c.center(), [960.0, 540.0]);
    }

    #[test]
    fn clips_nest_by_intersection() {
        let mut ui = Ui::reference();
        ui.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
        ui.push_clip(Rect::new(50.0, 50.0, 100.0, 100.0));
        assert_eq!(ui.clip(), Some(Rect::new(50.0, 50.0, 50.0, 50.0)));
        ui.pop_clip();
        assert_eq!(ui.clip(), Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
        ui.pop_clip();
        assert_eq!(ui.clip(), None);
    }

    #[test]
    fn clip_space_is_y_up_and_centred() {
        let ui = Ui::reference();
        assert_eq!(ui.to_clip([0.0, 0.0]), [-1.0, 1.0]);
        assert_eq!(ui.to_clip([960.0, 540.0]), [0.0, 0.0]);
        assert_eq!(ui.to_clip([1920.0, 1080.0]), [1.0, -1.0]);
    }
}
