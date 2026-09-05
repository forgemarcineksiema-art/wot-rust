//! A rectangle in physical pixels, y down (interface program F4). The one shape every element,
//! clip region, anchor and hit test speaks; the emitter converts it to clip space exactly once.

/// Axis-aligned, in physical pixels, origin top-left, y growing downward.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// From a top-left corner and a bottom-right corner.
    pub fn from_min_max(min: [f32; 2], max: [f32; 2]) -> Self {
        Self { x: min[0], y: min[1], w: (max[0] - min[0]).max(0.0), h: (max[1] - min[1]).max(0.0) }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w * 0.5, self.y + self.h * 0.5]
    }

    pub fn size(&self) -> [f32; 2] {
        [self.w, self.h]
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Whether `point` (px) lies inside, edges inclusive on the near side.
    pub fn contains(&self, point: [f32; 2]) -> bool {
        point[0] >= self.x
            && point[0] < self.right()
            && point[1] >= self.y
            && point[1] < self.bottom()
    }

    /// The overlap of two rectangles, or `None` when they do not touch.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (x1 > x0 && y1 > y0).then(|| Rect::from_min_max([x0, y0], [x1, y1]))
    }

    /// Shrunk by `px` on every side (grown when negative); never below zero size.
    pub fn inset(&self, px: f32) -> Rect {
        Rect {
            x: self.x + px,
            y: self.y + px,
            w: (self.w - 2.0 * px).max(0.0),
            h: (self.h - 2.0 * px).max(0.0),
        }
    }

    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, ..*self }
    }

    /// Whether `other` lies entirely inside this rectangle.
    pub fn encloses(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right() + 1.0e-3
            && other.bottom() <= self.bottom() + 1.0e-3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rect_knows_its_edges_and_its_centre() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.right(), 40.0);
        assert_eq!(r.bottom(), 60.0);
        assert_eq!(r.center(), [25.0, 40.0]);
        assert!(r.contains([10.0, 20.0]) && !r.contains([40.0, 60.0]));
    }

    #[test]
    fn intersection_is_the_overlap_and_nothing_when_apart() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(&b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.intersect(&Rect::new(20.0, 20.0, 1.0, 1.0)), None);
    }

    #[test]
    fn inset_never_goes_negative() {
        let r = Rect::new(0.0, 0.0, 4.0, 4.0).inset(3.0);
        assert_eq!(r.w, 0.0);
        assert!(r.is_empty());
        assert!(Rect::new(0.0, 0.0, 10.0, 10.0).encloses(&Rect::new(1.0, 1.0, 8.0, 8.0)));
    }
}
