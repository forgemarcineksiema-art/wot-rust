//! Rows and columns (interface program F4): the two ways elements line up inside a plate. No
//! retained tree — a builder hands out child rectangles in order, with padding and a gap, and
//! the list is rebuilt every frame like everything else in the HUD.

use crate::rect::Rect;

/// Lays children left to right inside a frame.
#[derive(Debug, Clone)]
pub struct Row {
    inner: Rect,
    gap: f32,
    cursor: f32,
}

impl Row {
    /// A row inside `frame`, padded by `pad_px` on every side, with `gap_px` between children.
    pub fn new(frame: Rect, pad_px: f32, gap_px: f32) -> Self {
        let inner = frame.inset(pad_px);
        Self { inner, gap: gap_px, cursor: inner.x }
    }

    /// The next child, `width_px` wide and the row's full inner height.
    pub fn next(&mut self, width_px: f32) -> Rect {
        let rect = Rect::new(self.cursor, self.inner.y, width_px.max(0.0), self.inner.h);
        self.cursor = rect.right() + self.gap;
        rect
    }

    /// Whatever width is left, as one child.
    pub fn fill(&mut self) -> Rect {
        let width = (self.inner.right() - self.cursor).max(0.0);
        self.next(width)
    }

    /// Width still unclaimed.
    pub fn remaining(&self) -> f32 {
        (self.inner.right() - self.cursor).max(0.0)
    }

    pub fn inner(&self) -> Rect {
        self.inner
    }
}

/// Lays children top to bottom inside a frame.
#[derive(Debug, Clone)]
pub struct Column {
    inner: Rect,
    gap: f32,
    cursor: f32,
}

impl Column {
    pub fn new(frame: Rect, pad_px: f32, gap_px: f32) -> Self {
        let inner = frame.inset(pad_px);
        Self { inner, gap: gap_px, cursor: inner.y }
    }

    /// The next child, `height_px` tall and the column's full inner width.
    pub fn next(&mut self, height_px: f32) -> Rect {
        let rect = Rect::new(self.inner.x, self.cursor, self.inner.w, height_px.max(0.0));
        self.cursor = rect.bottom() + self.gap;
        rect
    }

    pub fn fill(&mut self) -> Rect {
        let height = (self.inner.bottom() - self.cursor).max(0.0);
        self.next(height)
    }

    pub fn remaining(&self) -> f32 {
        (self.inner.bottom() - self.cursor).max(0.0)
    }

    pub fn inner(&self) -> Rect {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_lays_children_left_to_right_with_its_gap() {
        let mut row = Row::new(Rect::new(0.0, 0.0, 100.0, 20.0), 4.0, 6.0);
        let a = row.next(20.0);
        let b = row.next(20.0);
        assert_eq!(a, Rect::new(4.0, 4.0, 20.0, 12.0));
        assert_eq!(b.x, a.right() + 6.0);
        let rest = row.fill();
        assert_eq!(rest.right(), 96.0);
        assert_eq!(row.remaining(), 0.0);
    }

    #[test]
    fn a_column_with_padding_never_overlaps_its_children() {
        let frame = Rect::new(10.0, 10.0, 50.0, 100.0);
        let mut column = Column::new(frame, 5.0, 3.0);
        let rects: Vec<Rect> = (0..4).map(|_| column.next(15.0)).collect();
        for pair in rects.windows(2) {
            assert!(pair[0].intersect(&pair[1]).is_none(), "children overlap: {pair:?}");
            assert!(pair[1].y >= pair[0].bottom() + 3.0);
        }
        for r in &rects {
            assert!(frame.inset(5.0).encloses(r), "{r:?} escaped the padded frame");
        }
    }
}
