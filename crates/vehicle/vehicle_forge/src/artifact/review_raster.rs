use glam::Vec2;

pub fn fill_triangle(pixels: &mut [u8], width: u32, height: u32, p: [Vec2; 3], color: [u8; 4]) {
    let min = p.iter().fold(Vec2::splat(f32::MAX), |acc, p| acc.min(*p)).floor();
    let max = p.iter().fold(Vec2::splat(f32::MIN), |acc, p| acc.max(*p)).ceil();
    let min_x = min.x.max(0.0) as u32;
    let min_y = min.y.max(0.0) as u32;
    let max_x = max.x.min(width as f32 - 1.0) as u32;
    let max_y = max.y.min(height as f32 - 1.0) as u32;
    let area = edge(p[0], p[1], p[2]);
    if area.abs() < 0.001 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(p[1], p[2], q);
            let w1 = edge(p[2], p[0], q);
            let w2 = edge(p[0], p[1], q);
            if [w0, w1, w2].iter().all(|w| w.signum() == area.signum() || w.abs() < 0.001) {
                set_pixel(pixels, width, x, y, color);
            }
        }
    }
}

pub fn draw_triangle_edges(pixels: &mut [u8], width: u32, height: u32, p: [Vec2; 3]) {
    draw_line(pixels, width, height, p[0], p[1]);
    draw_line(pixels, width, height, p[1], p[2]);
    draw_line(pixels, width, height, p[2], p[0]);
}

pub fn set_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

fn draw_line(pixels: &mut [u8], width: u32, height: u32, a: Vec2, b: Vec2) {
    let steps = ((b - a).length().ceil() as u32).max(1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = a.lerp(b, t);
        if p.x >= 0.0 && p.y >= 0.0 && p.x < width as f32 && p.y < height as f32 {
            set_pixel(pixels, width, p.x as u32, p.y as u32, [28, 30, 28, 255]);
        }
    }
}

fn edge(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}
