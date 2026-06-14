use std::io;

use glam::{Vec2, Vec3};
use vehicle_geometry::{BakedVehicle, GeometryVertex, MaterialRole};

use super::review_raster::{draw_triangle_edges, fill_triangle};
use super::{ReviewCameraSet, ReviewCameraSpec};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 160;
const BACKGROUND: [u8; 4] = [244, 244, 240, 255];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedReviewImage {
    file: &'static str,
    bytes: Vec<u8>,
}

impl BakedReviewImage {
    pub fn file(&self) -> &'static str {
        self.file
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn bake_review_images(
    vehicle: &BakedVehicle,
    cameras: &ReviewCameraSet,
) -> Result<Vec<BakedReviewImage>, io::Error> {
    cameras
        .cameras()
        .iter()
        .map(|camera| {
            let pixels = render_camera(vehicle, camera);
            Ok(BakedReviewImage {
                file: camera.file_name(),
                bytes: encode_review_png(WIDTH, HEIGHT, &pixels)?,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
struct ProjectedTri {
    p: [Vec2; 3],
    depth: f32,
    color: [u8; 4],
}

struct CameraBasis {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
}

fn render_camera(vehicle: &BakedVehicle, camera: &ReviewCameraSpec) -> Vec<u8> {
    let basis = camera_basis(camera);
    let mut tris = projected_tris(vehicle, &basis);
    tris.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    let (min, max) = projected_bounds(&tris);
    let scale = image_scale(min, max, camera.distance_scale());
    let centre = (min + max) * 0.5;
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }

    for tri in tris {
        let screen = [
            to_screen(tri.p[0], centre, scale),
            to_screen(tri.p[1], centre, scale),
            to_screen(tri.p[2], centre, scale),
        ];
        fill_triangle(&mut pixels, WIDTH, HEIGHT, screen, tri.color);
        draw_triangle_edges(&mut pixels, WIDTH, HEIGHT, screen);
    }
    pixels
}

fn projected_tris(vehicle: &BakedVehicle, basis: &CameraBasis) -> Vec<ProjectedTri> {
    let mut tris = Vec::new();
    for submesh in vehicle.submeshes() {
        let vertices = submesh.mesh.vertices();
        for indices in submesh.mesh.indices().chunks_exact(3) {
            let a = &vertices[indices[0] as usize];
            let b = &vertices[indices[1] as usize];
            let c = &vertices[indices[2] as usize];
            tris.push(ProjectedTri {
                p: [
                    project(a.position, basis),
                    project(b.position, basis),
                    project(c.position, basis),
                ],
                depth: (a.position + b.position + c.position).dot(basis.forward) / 3.0,
                color: shaded_material(a, b, c, basis),
            });
        }
    }
    tris
}

fn camera_basis(camera: &ReviewCameraSpec) -> CameraBasis {
    let yaw = camera.yaw_deg().to_radians();
    let pitch = camera.pitch_deg().to_radians();
    let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
        .normalize_or_zero();
    let right = Vec3::Y.cross(forward).normalize_or_zero();
    let up = forward.cross(right).normalize_or_zero();
    CameraBasis { right, up, forward }
}

fn project(position: Vec3, basis: &CameraBasis) -> Vec2 {
    Vec2::new(position.dot(basis.right), position.dot(basis.up))
}

fn projected_bounds(tris: &[ProjectedTri]) -> (Vec2, Vec2) {
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

fn image_scale(min: Vec2, max: Vec2, distance_scale: f32) -> f32 {
    let extent = (max - min).max(Vec2::splat(0.001));
    let scale_x = (WIDTH as f32 - 28.0) / extent.x;
    let scale_y = (HEIGHT as f32 - 22.0) / extent.y;
    scale_x.min(scale_y) / distance_scale.max(0.1)
}

fn to_screen(point: Vec2, centre: Vec2, scale: f32) -> Vec2 {
    let p = (point - centre) * scale;
    Vec2::new(WIDTH as f32 * 0.5 + p.x, HEIGHT as f32 * 0.5 - p.y)
}

fn shaded_material(
    a: &GeometryVertex,
    b: &GeometryVertex,
    c: &GeometryVertex,
    basis: &CameraBasis,
) -> [u8; 4] {
    let normal = (a.normal + b.normal + c.normal).normalize_or_zero();
    let light = (0.55 + 0.45 * normal.dot(-basis.forward).abs()).clamp(0.35, 1.0);
    let base = review_material_color(a.material);
    [
        (f32::from(base[0]) * light) as u8,
        (f32::from(base[1]) * light) as u8,
        (f32::from(base[2]) * light) as u8,
        255,
    ]
}

fn review_material_color(material: MaterialRole) -> [u8; 4] {
    match material {
        MaterialRole::RolledArmor => [108, 116, 92, 255],
        MaterialRole::CastArmor => [118, 124, 102, 255],
        MaterialRole::BarrelSteel => [58, 61, 60, 255],
        MaterialRole::TrackMetal => [42, 42, 39, 255],
        MaterialRole::Rubber => [20, 20, 22, 255],
    }
}

fn encode_review_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(rgba)?;
    }
    Ok(bytes)
}
