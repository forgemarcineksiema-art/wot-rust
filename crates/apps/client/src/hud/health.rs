pub(crate) fn health_color(frac: f32) -> [f32; 4] {
    if frac > 0.5 {
        [0.30, 0.82, 0.34, 0.92]
    } else if frac > 0.25 {
        [0.92, 0.78, 0.20, 0.92]
    } else {
        [0.90, 0.26, 0.22, 0.92]
    }
}
