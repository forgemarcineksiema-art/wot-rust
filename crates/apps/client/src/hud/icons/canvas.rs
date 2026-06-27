/// A tiny coverage canvas with normalized-coordinate primitives ([0,1] maps across `n` px).
pub(super) struct Canvas {
    pub(super) px: Vec<u8>,
    n: i32,
}

impl Canvas {
    pub(super) fn new(n: i32) -> Self {
        Self { n, px: vec![0u8; (n * n) as usize] }
    }

    fn plot(&mut self, x: i32, y: i32, value: u8) {
        if x >= 0 && y >= 0 && x < self.n && y < self.n {
            let idx = (y * self.n + x) as usize;
            self.px[idx] = self.px[idx].max(value);
        }
    }

    fn for_bbox(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        mut f: impl FnMut(&mut Self, f32, f32, i32, i32),
    ) {
        let lo_x = ((x0 * self.n as f32).floor() as i32 - 1).max(0);
        let hi_x = ((x1 * self.n as f32).ceil() as i32 + 1).min(self.n - 1);
        let lo_y = ((y0 * self.n as f32).floor() as i32 - 1).max(0);
        let hi_y = ((y1 * self.n as f32).ceil() as i32 + 1).min(self.n - 1);
        for py in lo_y..=hi_y {
            for px in lo_x..=hi_x {
                let u = (px as f32 + 0.5) / self.n as f32;
                let v = (py as f32 + 0.5) / self.n as f32;
                f(self, u, v, px, py);
            }
        }
    }

    pub(super) fn disc(&mut self, cx: f32, cy: f32, r: f32) {
        self.for_bbox(cx - r, cy - r, cx + r, cy + r, |s, u, v, px, py| {
            if (u - cx).hypot(v - cy) <= r {
                s.plot(px, py, 255);
            }
        });
    }

    pub(super) fn punch_disc(&mut self, cx: f32, cy: f32, r: f32) {
        self.for_bbox(cx - r, cy - r, cx + r, cy + r, |s, u, v, px, py| {
            if (u - cx).hypot(v - cy) <= r {
                let idx = (py * s.n + px) as usize;
                s.px[idx] = 0;
            }
        });
    }

    pub(super) fn ring(&mut self, cx: f32, cy: f32, r_outer: f32, r_inner: f32) {
        self.for_bbox(cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer, |s, u, v, px, py| {
            let d = (u - cx).hypot(v - cy);
            if d <= r_outer && d >= r_inner {
                s.plot(px, py, 255);
            }
        });
    }

    pub(super) fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.for_bbox(x0, y0, x1, y1, |s, u, v, px, py| {
            if u >= x0 && u <= x1 && v >= y0 && v <= y1 {
                s.plot(px, py, 255);
            }
        });
    }

    pub(super) fn tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        let min_x = a[0].min(b[0]).min(c[0]);
        let max_x = a[0].max(b[0]).max(c[0]);
        let min_y = a[1].min(b[1]).min(c[1]);
        let max_y = a[1].max(b[1]).max(c[1]);
        self.for_bbox(min_x, min_y, max_x, max_y, |s, u, v, px, py| {
            if point_in_tri([u, v], a, b, c) {
                s.plot(px, py, 255);
            }
        });
    }
}

fn point_in_tri(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let edge = |p: [f32; 2], q: [f32; 2], r: [f32; 2]| {
        (r[0] - p[0]) * (q[1] - p[1]) - (r[1] - p[1]) * (q[0] - p[0])
    };
    let d1 = edge(p, a, b);
    let d2 = edge(p, b, c);
    let d3 = edge(p, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}
