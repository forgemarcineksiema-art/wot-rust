//! Unit meshes for the belt's end wheels — the smooth front idler and the toothed rear drive
//! sprocket. Split from `running_gear_wheels` (the road wheel) to stay within the reviewability
//! budget.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::running_gear_wheels::wheel_disc_at;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// The larger end wheel (drive sprocket / idler), centred at the origin with its axle along X.
pub fn end_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    idler_unit_mesh(kin)
}

/// Smooth front idler wheel, centred at the origin with its axle along X. Built as a steel disc
/// wheel with a rubber tire rim and a proud hub — a *smooth* sibling of the road wheel, so the front
/// of the track reads as a plain wheel against the toothed drive sprocket at the rear.
pub fn idler_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    match kin.idler_face {
        game_core::IdlerFace::Smooth => smooth_idler(kin),
        game_core::IdlerFace::Openwork => openwork_idler(kin),
    }
}

fn smooth_idler(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(20);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.86, half_w, seg, MaterialRole::TrackMetal))
        .append(&tread_band(0.0, r, half_w * 0.9, seg))
        .append(&wheel_disc_at(0.0, r * 0.28, half_w * 1.12, seg, MaterialRole::TrackMetal))
        .build()
}

/// A spoked idler casting: the same read as the vehicle's openwork road wheels, at the end
/// wheel's own radius. The KV-1's idler is a webbed casting, and a smooth drum beside six spoked
/// road wheels is the kind of mismatch the close-up review exists to catch.
fn openwork_idler(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(20);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    // Recessed web the spokes read against, plus the hub and the tread band the track rides.
    let mut builder = MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.74, half_w * 0.30, seg, MaterialRole::TrackMetal))
        .append(&tread_band(0.0, r, half_w * 0.9, seg))
        .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.12, seg, MaterialRole::TrackMetal));
    // Radial arms bridging hub to rim, proud of the web and buried at both ends so no tip floats.
    let arms = kin.wheel_spokes.max(3);
    for i in 0..arms {
        let angle = (i as f32 / arms as f32) * std::f32::consts::TAU;
        builder = builder.append(&crate::running_gear_wheels::spoke_arm(
            angle,
            r * 0.18,
            r * 0.80,
            r,
            half_w * 0.62,
        ));
    }
    builder.build()
}

/// A rubber tire tread ring at `radius`, spanning `center_x ± half_width` along the axle, with
/// side lips running inward to 0.82·radius. The lips radially overlap the idler's steel disc
/// (0.86 r, on wider planes), so the band reads as a solid tire instead of opening an annular
/// see-through window between tread and disc. The centre groove gives the shoes' guide horns a
/// channel around the wrap, exactly like the road wheels' dual-tire groove.
fn tread_band(center_x: f32, radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.82, center_x - half_width),
                ProfilePoint::new(radius, center_x - half_width),
                ProfilePoint::new(radius, center_x - half_width * 0.34),
                ProfilePoint::new(radius * 0.90, center_x),
                ProfilePoint::new(radius, center_x + half_width * 0.34),
                ProfilePoint::new(radius, center_x + half_width),
                ProfilePoint::new(radius * 0.82, center_x + half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// Rear drive sprocket in the real T-54 layout: a steel drum with TWO toothed rings flanking the
/// track shoes at the drum's outer edges. The teeth pass OUTSIDE the link plates (they never
/// intersect the belt) and their count comes from the link pitch on the wrap circle, so with the
/// wrap-radius spin the teeth and the shoes they flank visibly move together — the meshing read.
pub fn sprocket_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(16);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    // The toothed rings live INSIDE the belt band: their outer face lands exactly on the
    // blueprint's `outer_x` (the documented width over tracks), and the teeth are radially
    // capped under the shoe plates so they never clip them — they show in the gaps between
    // shoes, which is where a real sprocket's teeth read anyway. (They used to flank the
    // shoes OUTBOARD, baking every vehicle 5–8 cm wider per side than its dossier.)
    let tooth_half = 0.028;
    let ring_x = (kin.band_half_width - tooth_half).max(0.02);
    let wrap_r = crate::running_gear_belt::wrap_radius(kin);
    let tooth_outer_r = (r * 1.10).min(wrap_r - 0.032);
    let pitch = (kin.belt_length() / kin.link_count().max(1) as f32).max(0.05);
    let teeth = ((std::f32::consts::TAU * wrap_r) / pitch).round().max(8.0) as usize;

    let mut builder = MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.62, half_w, seg, MaterialRole::TrackMetal))
        .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.15, seg, MaterialRole::TrackMetal));
    for side in [-1.0_f32, 1.0] {
        let center_x = side * ring_x;
        // Thin carrier ring the teeth root into.
        builder = builder.append(&wheel_disc_at(
            center_x,
            r * 0.80,
            0.022,
            seg,
            MaterialRole::TrackMetal,
        ));
        for i in 0..teeth {
            let angle = (i as f32 / teeth as f32) * std::f32::consts::TAU;
            builder = builder.append(&sprocket_tooth(
                center_x,
                angle,
                r * 0.66,
                tooth_outer_r,
                tooth_half,
            ));
        }
    }
    builder.build()
}

fn sprocket_tooth(
    center_x: f32,
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    half_width: f32,
) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let section = vec![
        radial * inner_r - tangent * 0.048,
        radial * inner_r + tangent * 0.048,
        radial * outer_r + tangent * 0.022,
        radial * outer_r - tangent * 0.022,
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::new(center_x, 0.0, 0.0),
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: half_width,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}
