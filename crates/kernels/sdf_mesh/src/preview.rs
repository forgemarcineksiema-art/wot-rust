//! A tiny renderer-free CPU rasteriser for spike comparison images (orthographic, **z-buffered**,
//! flat-shaded by vertex normal). Judges SDF/CAD/revolve meshes without wgpu. Two entry points: one
//! flat colour per mesh, or per-triangle material colours.

use glam::{Vec2, Vec3};
use vehicle_geometry::{GeometryMesh, MaterialRole};

const BG: [u8; 4] = [244, 244, 240, 255];

struct Basis {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
}

struct Tri {
    p: [Vec2; 3],
    depth: [f32; 3],
    color: [u8; 4],
}

/// Render `meshes`, each with one flat base colour, from a yaw/pitch orbit; returns PNG bytes.
pub fn render_png(
    meshes: &[(&GeometryMesh, [u8; 3])],
    yaw_deg: f32,
    pitch_deg: f32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let basis = basis(yaw_deg, pitch_deg);
    let mut tris = Vec::new();
    for (mesh, color) in meshes {
        let flat = *color;
        collect(mesh, &basis, |_| flat, &mut tris);
    }
    encode_png_rgba(w, h, &rasterize(tris, w, h))
}

/// Render `meshes` shaded by each triangle's material (armour, cast, barrel steel, track, rubber).
pub fn render_by_material(
    meshes: &[&GeometryMesh],
    yaw_deg: f32,
    pitch_deg: f32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let basis = basis(yaw_deg, pitch_deg);
    let mut tris = Vec::new();
    for mesh in meshes {
        collect(mesh, &basis, preview_material_color, &mut tris);
    }
    encode_png_rgba(w, h, &rasterize(tris, w, h))
}

/// Composite several material-shaded panels into one contact sheet (row-major grid). Each panel is
/// `(meshes, yaw_deg, pitch_deg)`. Returns PNG bytes.
pub fn render_contact_sheet(
    panels: &[(&[&GeometryMesh], f32, f32)],
    cols: u32,
    panel_w: u32,
    panel_h: u32,
) -> Vec<u8> {
    let rows = (panels.len() as u32).div_ceil(cols.max(1));
    let (cw, ch) = (cols * panel_w, rows * panel_h);
    let mut canvas = vec![0u8; (cw * ch * 4) as usize];
    for px in canvas.chunks_exact_mut(4) {
        px.copy_from_slice(&BG);
    }
    for (i, (meshes, yaw, pitch)) in panels.iter().enumerate() {
        let basis = basis(*yaw, *pitch);
        let mut tris = Vec::new();
        for mesh in *meshes {
            collect(mesh, &basis, preview_material_color, &mut tris);
        }
        let panel = rasterize(tris, panel_w, panel_h);
        let (ox, oy) = ((i as u32 % cols) * panel_w, (i as u32 / cols) * panel_h);
        for y in 0..panel_h {
            let d = (((oy + y) * cw + ox) * 4) as usize;
            let s = (y * panel_w * 4) as usize;
            let row = (panel_w * 4) as usize;
            canvas[d..d + row].copy_from_slice(&panel[s..s + row]);
        }
    }
    encode_png_rgba(cw, ch, &canvas)
}

fn collect(
    mesh: &GeometryMesh,
    basis: &Basis,
    color_of: impl Fn(MaterialRole) -> [u8; 3],
    out: &mut Vec<Tri>,
) {
    let verts = mesh.vertices();
    for idx in mesh.indices().chunks_exact(3) {
        let v = [&verts[idx[0] as usize], &verts[idx[1] as usize], &verts[idx[2] as usize]];
        let normal = (v[0].normal + v[1].normal + v[2].normal).normalize_or_zero();
        let light = (0.5 + 0.5 * normal.dot(-basis.forward).abs()).clamp(0.32, 1.0);
        let base = color_of(v[0].material);
        out.push(Tri {
            p: v.map(|vtx| Vec2::new(vtx.position.dot(basis.right), vtx.position.dot(basis.up))),
            depth: v.map(|vtx| vtx.position.dot(basis.forward)),
            color: [
                (f32::from(base[0]) * light) as u8,
                (f32::from(base[1]) * light) as u8,
                (f32::from(base[2]) * light) as u8,
                255,
            ],
        });
    }
}

fn preview_material_color(material: MaterialRole) -> [u8; 3] {
    match material {
        MaterialRole::RolledArmor => [108, 116, 92],
        MaterialRole::CastArmor => [120, 126, 104],
        MaterialRole::BarrelSteel => [58, 61, 60],
        MaterialRole::TrackMetal => [46, 46, 42],
        MaterialRole::Rubber => [26, 26, 28],
        MaterialRole::InteriorPrimer => [118, 133, 99],
        MaterialRole::InteriorMachinery => [46, 51, 43],
        MaterialRole::Ammunition => [110, 79, 33],
        MaterialRole::ExposedSteel => [92, 75, 57],
        MaterialRole::Canvas => [92, 92, 79],
        MaterialRole::Glass => [184, 199, 204],
    }
}

fn rasterize(tris: Vec<Tri>, w: u32, h: u32) -> Vec<u8> {
    let (min, max) = bounds(&tris);
    let scale = fit_scale(min, max, w, h);
    let centre = (min + max) * 0.5;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for px in pixels.chunks_exact_mut(4) {
        px.copy_from_slice(&BG);
    }
    let mut zbuffer = vec![f32::INFINITY; (w * h) as usize];
    for tri in &tris {
        let screen = tri.p.map(|p| viewport_point(p, centre, scale, w, h));
        fill(&mut pixels, &mut zbuffer, w, h, screen, tri.depth, tri.color);
    }
    pixels
}

fn basis(yaw_deg: f32, pitch_deg: f32) -> Basis {
    let (yaw, pitch) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
        .normalize_or_zero();
    let right = Vec3::Y.cross(forward).normalize_or_zero();
    Basis { right, up: forward.cross(right).normalize_or_zero(), forward }
}

fn bounds(tris: &[Tri]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for tri in tris {
        for p in tri.p {
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

fn fit_scale(min: Vec2, max: Vec2, w: u32, h: u32) -> f32 {
    let extent = (max - min).max(Vec2::splat(0.001));
    ((w as f32 - 24.0) / extent.x).min((h as f32 - 24.0) / extent.y)
}

fn viewport_point(p: Vec2, centre: Vec2, scale: f32, w: u32, h: u32) -> Vec2 {
    let q = (p - centre) * scale;
    Vec2::new(w as f32 * 0.5 + q.x, h as f32 * 0.5 - q.y)
}

fn fill(
    pixels: &mut [u8],
    zbuffer: &mut [f32],
    w: u32,
    h: u32,
    p: [Vec2; 3],
    depth: [f32; 3],
    color: [u8; 4],
) {
    let min = p.iter().fold(Vec2::splat(f32::MAX), |a, p| a.min(*p)).floor();
    let max = p.iter().fold(Vec2::splat(f32::MIN), |a, p| a.max(*p)).ceil();
    let area = orient2d(p[0], p[1], p[2]);
    if area.abs() < 0.001 {
        return;
    }
    for y in (min.y.max(0.0) as u32)..=(max.y.min(h as f32 - 1.0) as u32) {
        for x in (min.x.max(0.0) as u32)..=(max.x.min(w as f32 - 1.0) as u32) {
            let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let (w0, w1, w2) =
                (orient2d(p[1], p[2], q), orient2d(p[2], p[0], q), orient2d(p[0], p[1], q));
            if [w0, w1, w2].iter().all(|v| v.signum() == area.signum() || v.abs() < 0.001) {
                let z = (w0 * depth[0] + w1 * depth[1] + w2 * depth[2]) / area;
                let i = (y * w + x) as usize;
                if z < zbuffer[i] {
                    zbuffer[i] = z;
                    pixels[i * 4..i * 4 + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().and_then(|mut w| w.write_image_data(rgba)).expect("png encode");
    }
    bytes
}
