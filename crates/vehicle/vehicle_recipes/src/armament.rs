//! Shared armament shapes: the revolved gun assembly and its moving fittings.

use glam::Vec3;

use super::{SG_BARREL, SG_MANTLET};
use vehicle_geometry::{
    Axis, GeometryMesh, GeometryVertex, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
};

/// Main-gun plan: a revolved barrel plus optional cast mantlet, bore evacuator, and muzzle brake.
/// All parts share the trunnion's `axis_y` so the whole assembly elevates cleanly as one submesh.
pub(crate) struct GunPlan {
    /// Y of the barrel centreline (matches the trunnion mount Y).
    pub axis_y: f32,
    /// Z where the barrel begins (just inside the mantlet).
    pub breech_z: f32,
    /// Z of the muzzle tip.
    pub muzzle_z: f32,
    pub radius: f32,
    pub segments: usize,
    /// Optional cast mantlet `(radius, back_z, front_z)`.
    pub mantlet: Option<(f32, f32, f32)>,
    /// Optional bore-evacuator bulge `(fraction_along_barrel, radius)`. The fraction places the
    /// bulge centre between the breech (`0.0`) and the muzzle tip (`1.0`), so the fitting rides
    /// the barrel and can never drift off it however the mount frames move.
    pub evacuator: Option<(f32, f32)>,
    /// Optional muzzle-brake radius.
    pub muzzle_brake: Option<f32>,
}

/// Build the gun submesh from a [`GunPlan`].
pub(crate) fn build_gun(plan: &GunPlan) -> GeometryMesh {
    build_gun_with_mantlet_scale(plan, 1.0, 1.0)
}

/// Build the gun with an optionally flattened moving mantlet. T-54 uses this to model the low,
/// broad oval mantlet mask seated in the turret cheeks instead of a round revolved collar.
pub(crate) fn build_gun_with_mantlet_scale(
    plan: &GunPlan,
    mantlet_x_scale: f32,
    mantlet_y_scale: f32,
) -> GeometryMesh {
    let origin = Vec3::new(0.0, plan.axis_y, 0.0);
    let mut builder = MeshBuilder::new().capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(plan.radius, plan.breech_z),
                // 2 mm short of the muzzle: the bore insert forms the true tip (below), so the
                // faces never fight and the derived muzzle mount stays EXACTLY at muzzle_z.
                ProfilePoint::new(plan.radius, plan.muzzle_z - 0.002),
            ],
            axis: Axis::Z,
            segments: plan.segments,
            material: MaterialRole::BarrelSteel,
            smoothing: SG_BARREL,
        },
    );
    if let Some((radius, back_z, front_z)) = plan.mantlet {
        // A gently bulged cast mantlet (Saukopf-style) reads rounder than a plain collar.
        let mid = (back_z + front_z) * 0.5;
        if (mantlet_x_scale - 1.0).abs() < f32::EPSILON
            && (mantlet_y_scale - 1.0).abs() < f32::EPSILON
        {
            builder = builder.capped_revolve_at(
                origin,
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(radius * 0.72, back_z),
                        ProfilePoint::new(radius, mid),
                        ProfilePoint::new(radius * 0.82, front_z),
                    ],
                    axis: Axis::Z,
                    segments: plan.segments,
                    material: MaterialRole::CastArmor,
                    smoothing: SG_MANTLET,
                },
            );
        } else {
            builder = builder.append(&oval_mantlet_mesh(
                origin,
                radius,
                back_z,
                mid,
                front_z,
                mantlet_x_scale,
                mantlet_y_scale,
                plan.segments,
            ));
        }
    }
    if let Some((fraction, radius)) = plan.evacuator {
        assert!(
            (0.05..=0.95).contains(&fraction),
            "bore-evacuator fraction {fraction} must sit on the barrel, away from both ends"
        );
        let center_z = plan.breech_z + (plan.muzzle_z - plan.breech_z) * fraction;
        builder = builder.capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, center_z - 0.20),
                    ProfilePoint::new(radius, center_z + 0.20),
                ],
                axis: Axis::Z,
                segments: plan.segments,
                material: MaterialRole::BarrelSteel,
                smoothing: SG_BARREL,
            },
        );
    }
    if let Some(brake_radius) = plan.muzzle_brake {
        // A real double-baffle brake, not a closed sausage (model-logic audit #4/#6, shapes
        // from the reference photos: KwK 36/43, KwK 42, D-25T all carry two baffle rings with
        // OPEN chambers between them). An inner blast tube spans the brake; two outer rings
        // sit on it with visible gaps — from the side the chambers read as real openings.
        let tube_r = (plan.radius * 1.08).min(brake_radius * 0.62);
        let back = plan.muzzle_z - 0.46;
        builder = builder.capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(tube_r, back),
                    ProfilePoint::new(tube_r, plan.muzzle_z - 0.002),
                ],
                axis: Axis::Z,
                segments: plan.segments,
                material: MaterialRole::BarrelSteel,
                smoothing: SG_BARREL,
            },
        );
        for (ring_back, ring_front) in
            [(back + 0.02, back + 0.15), (plan.muzzle_z - 0.15, plan.muzzle_z - 0.02)]
        {
            builder = builder.capped_revolve_at(
                origin,
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(brake_radius * 0.86, ring_back),
                        ProfilePoint::new(brake_radius, (ring_back + ring_front) * 0.5),
                        ProfilePoint::new(brake_radius * 0.86, ring_front),
                    ],
                    axis: Axis::Z,
                    segments: plan.segments,
                    material: MaterialRole::BarrelSteel,
                    smoothing: SG_BARREL,
                },
            );
        }
    }
    // The bore: every gun ends in a HOLE, not a plug (audit #4 — "którędy pocisk wyleci?").
    // v2 (user report: the flush dark disc still read as a painted circle): the muzzle face
    // is an ANNULUS at exactly muzzle_z (the derived muzzle mount stays honest) and the bore
    // is a dark FUNNEL receding 11 cm into the tube — the taper self-shades, so the opening
    // reads as depth from any angle instead of a flat cap.
    let face_r = plan.muzzle_brake.map_or(plan.radius, |b| (plan.radius * 1.08).min(b * 0.62));
    let bore_r = plan.radius * 0.62;
    builder = builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(face_r, plan.muzzle_z - 0.001),
                ProfilePoint::new(face_r, plan.muzzle_z),
                ProfilePoint::new(bore_r, plan.muzzle_z),
            ],
            axis: Axis::Z,
            segments: plan.segments,
            material: MaterialRole::BarrelSteel,
            smoothing: SG_BARREL,
        },
    );
    builder = builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(bore_r, plan.muzzle_z - 0.004),
                ProfilePoint::new(bore_r * 0.45, plan.muzzle_z - 0.11),
            ],
            axis: Axis::Z,
            segments: plan.segments,
            material: MaterialRole::TrackMetal,
            smoothing: SG_BARREL,
        },
    );
    builder.build()
}

#[allow(clippy::too_many_arguments)]
fn oval_mantlet_mesh(
    origin: Vec3,
    radius: f32,
    back_z: f32,
    mid_z: f32,
    front_z: f32,
    x_scale: f32,
    y_scale: f32,
    segments: usize,
) -> GeometryMesh {
    assert!(segments >= 3);
    let rings = [(radius * 0.72, back_z), (radius, mid_z), (radius * 0.82, front_z)];
    let mut vertices = Vec::with_capacity(segments * rings.len() + 2);
    for segment in 0..segments {
        let angle = (segment as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let normal =
            Vec3::new(cos / x_scale.max(0.001), sin / y_scale.max(0.001), 0.0).normalize_or_zero();
        for (ring_radius, z) in rings {
            vertices.push(GeometryVertex::new(
                origin + Vec3::new(cos * ring_radius * x_scale, sin * ring_radius * y_scale, z),
                normal,
                MaterialRole::CastArmor,
                SG_MANTLET,
            ));
        }
    }

    let mut indices = Vec::with_capacity(segments * 18);
    let ring_count = rings.len() as u32;
    for segment in 0..segments as u32 {
        let next = (segment + 1) % segments as u32;
        for row in 0..ring_count - 1 {
            let a = segment * ring_count + row;
            let b = next * ring_count + row;
            let c = b + 1;
            let d = a + 1;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    let back_center = vertices.len() as u32;
    vertices.push(GeometryVertex::new(
        origin + Vec3::new(0.0, 0.0, back_z),
        -Vec3::Z,
        MaterialRole::CastArmor,
        SG_MANTLET,
    ));
    let front_center = vertices.len() as u32;
    vertices.push(GeometryVertex::new(
        origin + Vec3::new(0.0, 0.0, front_z),
        Vec3::Z,
        MaterialRole::CastArmor,
        SG_MANTLET,
    ));
    for segment in 0..segments as u32 {
        let next = (segment + 1) % segments as u32;
        indices.extend_from_slice(&[back_center, next * ring_count, segment * ring_count]);
        indices.extend_from_slice(&[
            front_center,
            segment * ring_count + ring_count - 1,
            next * ring_count + ring_count - 1,
        ]);
    }

    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_with_evacuator(fraction: f32) -> GunPlan {
        GunPlan {
            axis_y: 1.8,
            breech_z: 1.0,
            muzzle_z: 5.0,
            radius: 0.10,
            segments: 8,
            mantlet: None,
            evacuator: Some((fraction, 0.15)),
            muzzle_brake: None,
        }
    }

    /// The evacuator is authored as a fraction of the exposed barrel, so wherever the mount
    /// frames put the muzzle, the bulge must land strictly between the breech and the muzzle tip.
    #[test]
    fn evacuator_fraction_keeps_the_bulge_on_the_barrel() {
        let plan = plan_with_evacuator(0.5);
        let gun = build_gun(&plan);
        let bulge: Vec<f32> = gun
            .vertices()
            .iter()
            .filter(|vertex| {
                let dx = vertex.position.x;
                let dy = vertex.position.y - plan.axis_y;
                (dx * dx + dy * dy).sqrt() > plan.radius + 0.01
            })
            .map(|vertex| vertex.position.z)
            .collect();

        assert!(!bulge.is_empty(), "evacuator bulge should be wider than the barrel");
        let center = (plan.breech_z + plan.muzzle_z) * 0.5;
        for z in bulge {
            assert!(z > plan.breech_z && z < plan.muzzle_z, "bulge at z {z} left the barrel");
            assert!((z - center).abs() <= 0.21, "bulge at z {z} not centred on the fraction");
        }
    }

    #[test]
    #[should_panic(expected = "must sit on the barrel")]
    fn evacuator_fraction_off_the_barrel_is_rejected() {
        build_gun(&plan_with_evacuator(1.10));
    }
}
