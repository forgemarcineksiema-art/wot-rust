use std::io;

use glam::{Mat4, Vec2, Vec3};
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, MaterialRole, RunningGearKinematics, idler_unit_mesh,
    return_roller_unit_mesh, road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh,
    swing_arm_unit_mesh, track_link_unit_mesh,
};

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
    pub(super) fn from_loaded(file: &'static str, bytes: Vec<u8>) -> Self {
        Self { file, bytes }
    }

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

/// Render one review camera at an arbitrary resolution — the Forge Studio's tile renderer.
/// The artifact path above stays on the fixed 256x160 so artifact bytes do not move.
/// Studio-only production review: the baked hull plus the same rest-pose instanced running gear
/// used by the client. Artifact bytes remain unchanged, while authors finally see the wheels,
/// suspension, sprockets, and moving track that they are actually judging.
/// `gear` is the kinematics to draw: a Studio live override passes the kinematics built from
/// the EDITED track so the tiles show the belt the author is tuning, not the embedded one.
pub(super) fn render_camera_sized_with_running_gear(
    gear: Option<&RunningGearKinematics>,
    vehicle: &BakedVehicle,
    camera: &ReviewCameraSpec,
    width: u32,
    height: u32,
) -> Vec<u8> {
    render_camera_at_with_gear(gear, vehicle, camera, width, height)
}

pub(super) fn encode_png_sized(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, io::Error> {
    encode_review_png(width, height, rgba)
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
    render_camera_at(vehicle, camera, WIDTH, HEIGHT)
}

fn render_camera_at(
    vehicle: &BakedVehicle,
    camera: &ReviewCameraSpec,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let basis = camera_basis(camera);
    let tris = projected_tris(vehicle, &basis);
    rasterize_projected(tris, camera, width, height)
}

fn render_camera_at_with_gear(
    gear: Option<&RunningGearKinematics>,
    vehicle: &BakedVehicle,
    camera: &ReviewCameraSpec,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let basis = camera_basis(camera);
    let mut tris = projected_tris(vehicle, &basis);
    if let Some(kin) = gear {
        let road_wheel = road_wheel_unit_mesh(kin);
        let idler = idler_unit_mesh(kin);
        let sprocket = sprocket_unit_mesh(kin);
        let link = track_link_unit_mesh(kin);
        let swing_arm = swing_arm_unit_mesh(kin);
        let return_roller = return_roller_unit_mesh(kin);
        for placement in running_gear_placements(kin, 0.0, 0.0) {
            let mesh = match placement.part {
                GearPart::RoadWheel => &road_wheel,
                GearPart::Idler => &idler,
                GearPart::Sprocket => &sprocket,
                GearPart::Link => &link,
                GearPart::SwingArm => &swing_arm,
                GearPart::ReturnRoller => &return_roller,
            };
            tris.extend(projected_mesh_tris(mesh, placement.transform, &basis));
        }
    }
    rasterize_projected(tris, camera, width, height)
}

fn rasterize_projected(
    mut tris: Vec<ProjectedTri>,
    camera: &ReviewCameraSpec,
    width: u32,
    height: u32,
) -> Vec<u8> {
    tris.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    let (min, max) = projected_bounds(&tris);
    let scale = image_scale(min, max, camera.distance_scale(), width, height);
    let centre = (min + max) * 0.5;
    let mut pixels = vec![0; (width * height * 4) as usize];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }

    for tri in tris {
        let screen = [
            to_screen(tri.p[0], centre, scale, width, height),
            to_screen(tri.p[1], centre, scale, width, height),
            to_screen(tri.p[2], centre, scale, width, height),
        ];
        fill_triangle(&mut pixels, width, height, screen, tri.color);
        draw_triangle_edges(&mut pixels, width, height, screen);
    }
    pixels
}

fn projected_tris(vehicle: &BakedVehicle, basis: &CameraBasis) -> Vec<ProjectedTri> {
    let mut tris = Vec::new();
    for submesh in vehicle.submeshes() {
        tris.extend(projected_mesh_tris(&submesh.mesh, Mat4::IDENTITY, basis));
    }
    tris
}

fn projected_mesh_tris(
    mesh: &GeometryMesh,
    transform: Mat4,
    basis: &CameraBasis,
) -> Vec<ProjectedTri> {
    let vertices = mesh.vertices();
    mesh.indices()
        .chunks_exact(3)
        .map(|indices| {
            let a = &vertices[indices[0] as usize];
            let b = &vertices[indices[1] as usize];
            let c = &vertices[indices[2] as usize];
            let positions = [a, b, c].map(|vertex| transform.transform_point3(vertex.position));
            let normal = [a, b, c]
                .into_iter()
                .map(|vertex| transform.transform_vector3(vertex.normal))
                .sum::<Vec3>()
                .normalize_or_zero();
            ProjectedTri {
                p: positions.map(|position| project(position, basis)),
                depth: positions.into_iter().sum::<Vec3>().dot(basis.forward) / 3.0,
                color: shaded_material(normal, a.material, basis),
            }
        })
        .collect()
}

/// The camera frame, in the world's own convention (+X is the vehicle's RIGHT side — see
/// `ArmorZone::RightTrack`, the left-fender exhaust at x −1.34 and the port-side cupola).
///
/// The viewer stands on the `+forward` side and looks back along `-forward`: the painter's
/// order draws larger `p·forward` last, so the surfaces facing `+forward` are the ones seen.
/// For THAT viewer screen-right is `forward × up`, not `up × forward`. The old order mirrored
/// every tile: a T-54 photographed head-on shows its port-side cupola on the viewer's RIGHT,
/// while the review tiles put it on the left — silently flipping every asymmetry judgement
/// (fender stowage, exhaust, cupola, DShK) a reviewer made from these images.
fn camera_basis(camera: &ReviewCameraSpec) -> CameraBasis {
    let yaw = camera.yaw_deg().to_radians();
    let pitch = camera.pitch_deg().to_radians();
    let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
        .normalize_or_zero();
    // A camera looking straight down (or up) has no yaw-defined right in the Y reference;
    // fall back to the hull's forward axis so a top view keeps a stable, non-degenerate frame.
    let reference_up = if forward.y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
    let right = forward.cross(reference_up).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
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

fn image_scale(min: Vec2, max: Vec2, distance_scale: f32, width: u32, height: u32) -> f32 {
    // The margins scale with the tile so a studio tile keeps the artifact framing proportions.
    let extent = (max - min).max(Vec2::splat(0.001));
    let scale_x = (width as f32 - 28.0 * width as f32 / WIDTH as f32) / extent.x;
    let scale_y = (height as f32 - 22.0 * height as f32 / HEIGHT as f32) / extent.y;
    scale_x.min(scale_y) / distance_scale.max(0.1)
}

fn to_screen(point: Vec2, centre: Vec2, scale: f32, width: u32, height: u32) -> Vec2 {
    let p = (point - centre) * scale;
    Vec2::new(width as f32 * 0.5 + p.x, height as f32 * 0.5 - p.y)
}

fn shaded_material(normal: Vec3, material: MaterialRole, basis: &CameraBasis) -> [u8; 4] {
    let light = (0.55 + 0.45 * normal.dot(-basis.forward).abs()).clamp(0.35, 1.0);
    let base = review_material_color(material);
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
        MaterialRole::InteriorPrimer => [118, 133, 99, 255],
        MaterialRole::InteriorMachinery => [46, 51, 43, 255],
        MaterialRole::Ammunition => [110, 79, 33, 255],
        MaterialRole::ExposedSteel => [92, 75, 57, 255],
        MaterialRole::Canvas => [92, 92, 79, 255],
        MaterialRole::Glass => [184, 199, 204, 255],
        MaterialRole::Timber => [89, 69, 48, 255],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ReviewCamera, ReviewCameraSet};

    fn spec(kind: ReviewCamera) -> ReviewCameraSpec {
        ReviewCameraSet::standard_vehicle_review()
            .cameras()
            .iter()
            .find(|camera| camera.kind() == kind)
            .expect("standard camera")
            .clone()
    }

    /// Screen-space x of a world point through a camera. Positive = right half of the tile.
    fn screen_x(kind: ReviewCamera, point: Vec3) -> f32 {
        project(point, &camera_basis(&spec(kind))).x
    }

    /// The chirality lock. World convention: +X is the vehicle's RIGHT side, +Z its bow.
    /// A tile must read like a photograph taken from that camera's side:
    /// - head-on, the tank faces you, so its right side is on YOUR left;
    /// - from behind, its right side is on your right;
    /// - from the port side, the bow runs to the left of frame (and vice versa).
    ///
    /// This is the invariant that was inverted: the old basis used `up × forward` for
    /// screen-right, mirroring every tile and silently reversing every asymmetry judgement
    /// (cupola side, exhaust side, fender stowage) an author made from a Studio image.
    #[test]
    fn tiles_read_like_photographs_not_mirrors() {
        let starboard = Vec3::new(1.0, 1.0, 0.0);
        let bow = Vec3::new(0.0, 1.0, 1.0);

        assert!(
            screen_x(ReviewCamera::Front, starboard) < 0.0,
            "head-on: the vehicle's right side belongs on the viewer's LEFT"
        );
        assert!(
            screen_x(ReviewCamera::Rear, starboard) > 0.0,
            "from behind: the vehicle's right side belongs on the viewer's RIGHT"
        );
        assert!(
            screen_x(ReviewCamera::LeftProfile, bow) < 0.0,
            "port-side view: the bow runs LEFT of frame"
        );
        assert!(
            screen_x(ReviewCamera::RightProfile, bow) > 0.0,
            "starboard-side view: the bow runs RIGHT of frame"
        );
    }

    /// The frame must stay right-handed on screen: up is up, and the profile views are mirrors
    /// of each other rather than the same picture twice.
    #[test]
    fn the_camera_frame_keeps_up_up_and_the_two_profiles_opposed() {
        for kind in [
            ReviewCamera::Front,
            ReviewCamera::Rear,
            ReviewCamera::LeftProfile,
            ReviewCamera::RightProfile,
            ReviewCamera::Top,
        ] {
            let basis = camera_basis(&spec(kind));
            assert!(
                basis.up.y > 0.0,
                "{kind:?}: the camera's up must point up, got {:?}",
                basis.up
            );
            assert!(
                basis.right.dot(basis.up).abs() < 1.0e-5
                    && basis.right.dot(basis.forward).abs() < 1.0e-5,
                "{kind:?}: the basis must stay orthogonal"
            );
        }
        let roof = Vec3::new(0.0, 2.0, 0.0);
        assert!(
            project(roof, &camera_basis(&spec(ReviewCamera::Front))).y > 0.0,
            "a point above the hull must project above the tile centre"
        );
    }
}
