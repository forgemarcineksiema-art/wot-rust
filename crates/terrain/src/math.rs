pub fn gaussian2(x: f32, z: f32, cx: f32, cz: f32, sx: f32, sz: f32, amp: f32) -> f32 {
    let dx = (x - cx) / sx;
    let dz = (z - cz) / sz;
    amp * (-0.5 * (dx * dx + dz * dz)).exp()
}

pub fn gaussian1(value: f32, center: f32, sigma: f32, amp: f32) -> f32 {
    let d = (value - center) / sigma;
    amp * (-0.5 * d * d).exp()
}
