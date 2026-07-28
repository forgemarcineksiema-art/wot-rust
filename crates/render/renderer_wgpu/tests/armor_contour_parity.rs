//! The CPU/GPU contract for a perforation's outline.
//!
//! `game_core::BreachContour` says of itself: "never presentation-only: collision samples this
//! same contour". Nothing enforced that across the Rust/WGSL seam, and the two sides drifted:
//! the shader phased its irregularity waves off the angle taken BEFORE the contour's own
//! rotation, the CPU off the angle taken AFTER it. Since `sim::breach_space` gives every breach
//! a uniformly random `rotation`, the drawn hole and the collision hole were different shapes on
//! every perforation in the game — you could see through steel a shell would not pass, and vice
//! versa.
//!
//! These tests lock the seam from both ends: the SHAPE (a boundary-radius sweep against the
//! authoritative CPU predicate) and the STRUCTURE (one contour evaluation in the shader library,
//! never a fourth copy-paste).

use std::f32::consts::TAU;

use game_core::math::hash_unit;
use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorFrame, ArmorMaterial, ArmorSurfaceId,
    ArmorZone, BreachContour, BreachFace, ShellType,
};
use glam::Vec3;
use renderer_wgpu::vehicle_shader_source;

/// A faithful Rust transcription of `aperture_contour_metric` in `shaders/camera_common.wgsl`.
/// Inside the contour exactly when the returned value is <= 1.0. If the shader changes, this
/// must change with it — and the parity sweep below is what notices when only one of them did.
fn shader_contour_metric_sq(
    contour: BreachContour,
    phase_a: f32,
    phase_b: f32,
    local_x: f32,
    local_y: f32,
) -> f32 {
    let (sin_r, cos_r) = contour.rotation_rad.sin_cos();
    let rotated_x = local_x * cos_r + local_y * sin_r;
    let rotated_y = -local_x * sin_r + local_y * cos_r;
    let angle = rotated_y.atan2(rotated_x);
    let rough = 1.0
        + contour.irregularity
            * ((angle * 3.0 + phase_a).sin() * 0.62 + (angle * 5.0 + phase_b).sin() * 0.38);
    let mx = rotated_x / (contour.major_radius_m * rough).max(0.005);
    let my = rotated_y / (contour.minor_radius_m * rough).max(0.005);
    mx * mx + my * my
}

/// The two deterministic wave phases the renderer uploads in `shape.yz`
/// (`client::vehicle::render_frame::hash_phase`), which is what `BreachContour` derives
/// internally from the same seed.
fn phases(seed: u64) -> (f32, f32) {
    (hash_unit(seed) * TAU, hash_unit(seed.rotate_left(29)) * TAU)
}

/// A breach whose plate-plane tangent basis is exactly (+X, +Y), so a plane offset `(a, b)` is
/// the local point `entry + (a, b, 0)`. `armor_surface_basis(+Z, d)` projects `d` into the plate:
/// a direction with a +X component gives `u = +X`, hence `v = n x u = +Y`.
fn breach_with(contour: BreachContour, seed: u64) -> ArmorBreach {
    let lobe = ApertureLobe {
        entry_local: Vec3::ZERO,
        exit_local: Vec3::new(0.0, 0.0, -0.1),
        entry_normal_local: Vec3::Z,
        exit_normal_local: Vec3::NEG_Z,
        direction_local: Vec3::new(0.3, 0.0, -1.0).normalize(),
        thickness_m: 0.1,
        outer: contour,
        inner: contour,
        fracture_seed: seed,
    };
    ArmorBreach::new(
        ArmorBreachDescriptor {
            breach_id: seed,
            surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::HullSide),
            frame: ArmorFrame::Hull,
            zone: ArmorZone::HullSide,
            material: ArmorMaterial::RolledSteel,
            face: BreachFace::Ingress,
            shell_type: ShellType::ArmorPiercing,
            created_tick: 0,
            impact_angle_degrees: 0.0,
            impact_energy_kj: 0.0,
            projectile_diameter_m: 0.1,
            residual_penetration_mm: 0.0,
        },
        lobe,
    )
}

/// Bisect the boundary radius of a predicate along one polar direction.
fn boundary_radius(direction: (f32, f32), inside: impl Fn(f32, f32) -> bool) -> f32 {
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        if inside(direction.0 * mid, direction.1 * mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The whole contract in one number: along every direction out of the hole's centre, the radius
/// at which the SHADER stops drawing steel and the radius at which COLLISION stops reporting
/// steel must be the same radius. The bug this locks out moved them apart by centimetres on a
/// centimetre-scale hole.
#[test]
fn the_shader_contour_is_the_same_shape_as_the_collision_contour() {
    // Real breach shapes from `sim::breach_space::make_breach`: calibre-class radii, the 1.5:1
    // obliquity cap on the major axis, every authored irregularity, and rotations spread around
    // the circle (the field is a uniformly random angle per shot, so no rotation is special).
    let cases = [
        (0.050_f32, 0.050_f32, 0.0_f32, 0.12_f32, 0x1234_u64),
        (0.050, 0.050, 1.1, 0.12, 0xBEEF),
        (0.075, 0.050, 2.7, 0.12, 0xC0FFEE),
        (0.075, 0.050, 5.9, 0.18, 0xFACE),
        (0.028, 0.022, 0.7, 0.08, 0x5EED),
        (0.190, 0.140, 4.2, 0.05, 0xD00D),
        (0.060, 0.055, std::f32::consts::PI, 0.22, 0xA11CE),
    ];

    let mut worst_gap_m = 0.0_f32;
    for (major, minor, rotation, irregularity, seed) in cases {
        let contour = BreachContour::new(major, minor, rotation, irregularity);
        let (phase_a, phase_b) = phases(seed);
        let breach = breach_with(contour, seed);

        for step in 0..128 {
            let angle = step as f32 / 128.0 * TAU;
            let direction = (angle.cos(), angle.sin());

            let shader = boundary_radius(direction, |x, y| {
                shader_contour_metric_sq(contour, phase_a, phase_b, x, y) <= 1.0
            });
            // The authoritative side: the same predicate a shell's clearance check runs
            // (`ArmorBreach::admits` at zero projectile radius is `BreachContour::contains`).
            let collision =
                boundary_radius(direction, |x, y| breach.admits(Vec3::new(x, y, 0.0), 0.0));

            let gap = (shader - collision).abs();
            worst_gap_m = worst_gap_m.max(gap);
            assert!(
                gap < 1.0e-4,
                "contour drifted at {:.3} rad on ({major}, {minor}, rot {rotation}, irr \
                 {irregularity}): shader edge {shader:.5} m vs collision edge {collision:.5} m \
                 ({gap:.5} m apart). The shader and `BreachContour::contains` must describe ONE \
                 hole — check the rotate-then-take-the-angle order in `aperture_contour_metric`.",
                angle
            );
        }
    }
    assert!(worst_gap_m < 1.0e-4, "worst contour gap {worst_gap_m} m");
}

/// The divergence survived because the same thirty lines were pasted into three shader
/// functions. One evaluation, or the next edit fixes two of three again.
#[test]
fn the_shader_library_evaluates_the_contour_in_exactly_one_place() {
    let source = vehicle_shader_source();
    assert_eq!(
        source.matches("fn aperture_contour_metric(").count(),
        1,
        "exactly one contour evaluation may exist in the composed vehicle shader"
    );
    // The buggy form, verbatim: the angle taken from the pre-rotation offset. Its absence is
    // the regression guard — the correct form reads `atan2(rotated.y, rotated.x)`.
    assert!(
        !source.contains("atan2(local.y, local.x)"),
        "the contour angle must be taken AFTER un-rotating into the contour frame; \
         `atan2(local.y, local.x)` phase-shifts the irregularity waves by the breach rotation"
    );
    assert!(
        source.contains("atan2(rotated.y, rotated.x)"),
        "the one contour evaluation must phase its waves off the un-rotated angle"
    );
}
