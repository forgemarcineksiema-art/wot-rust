//! Shared armament shapes: the revolved gun assembly, drum/domed cupolas, and the Soviet cast
//! dome turret.

use glam::Vec3;

use super::{SG_BARREL, SG_CAST, SG_CUPOLA, SG_MANTLET, SG_RING};
use crate::{Axis, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec};

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
    let origin = Vec3::new(0.0, plan.axis_y, 0.0);
    let mut builder = MeshBuilder::new().capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(plan.radius, plan.breech_z),
                ProfilePoint::new(plan.radius, plan.muzzle_z),
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
        builder = builder.capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(brake_radius, plan.muzzle_z - 0.42),
                    ProfilePoint::new(brake_radius, plan.muzzle_z - 0.04),
                ],
                axis: Axis::Z,
                segments: plan.segments,
                material: MaterialRole::BarrelSteel,
                smoothing: SG_BARREL,
            },
        );
    }
    builder.build()
}

/// Append a small cupola (drum or domed) onto a turret roof.
pub(crate) fn add_cupola(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    base_y: f32,
    radius: f32,
    height: f32,
    domed: bool,
) -> MeshBuilder {
    let origin = Vec3::new(x, 0.0, z);
    let profile = if domed {
        vec![
            ProfilePoint::new(radius, base_y),
            ProfilePoint::new(radius, base_y + height * 0.55),
            ProfilePoint::new(radius * 0.5, base_y + height),
        ]
    } else {
        vec![ProfilePoint::new(radius, base_y), ProfilePoint::new(radius, base_y + height)]
    };
    builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile,
            axis: Axis::Y,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CUPOLA,
        },
    )
}

/// Append a low visible collar around the turret ring so the rotating submesh reads as seated in
/// the hull deck rather than merely touching it at a mathematical plane.
pub(crate) fn add_turret_ring(
    builder: MeshBuilder,
    center_z: f32,
    base_y: f32,
    radius: f32,
    height: f32,
    segments: usize,
) -> MeshBuilder {
    builder.capped_revolve_at(
        Vec3::new(0.0, 0.0, center_z),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius, base_y - height * 0.35),
                ProfilePoint::new(radius * 1.04, base_y + height * 0.65),
            ],
            axis: Axis::Y,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_RING,
        },
    )
}

/// Append a fixed socket on the turret/casemate face behind the elevating mantlet. The moving
/// mantlet still lives in the gun submesh, but this collar keeps the joint visually anchored.
pub(crate) fn add_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
) -> MeshBuilder {
    let Some((radius, back_z, front_z)) = mantlet else {
        return builder;
    };
    let span = (front_z - back_z).max(0.12);
    let socket_back = back_z - span * 0.25;
    let socket_front = back_z + span * 0.55;
    builder.capped_revolve_at(
        Vec3::new(0.0, axis_y, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.96, socket_back),
                ProfilePoint::new(radius * 1.08, socket_front),
            ],
            axis: Axis::Z,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_MANTLET,
        },
    )
}

/// A low, rounded Soviet cast turret dome centred on the turret ring. Circular in plan; the
/// four-point profile bulges at the shoulder and tapers to the roof, reading as a cast casting
/// rather than a box.
pub(crate) fn cast_dome_turret(
    center_z: f32,
    base_radius: f32,
    roof_radius: f32,
    base_y: f32,
    roof_y: f32,
    segments: usize,
) -> MeshBuilder {
    let span = roof_y - base_y;
    MeshBuilder::new().capped_revolve_at(
        Vec3::new(0.0, 0.0, center_z),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(base_radius * 0.86, base_y),
                ProfilePoint::new(base_radius, base_y + span * 0.20),
                ProfilePoint::new(base_radius * 0.80, base_y + span * 0.55),
                ProfilePoint::new(roof_radius, roof_y),
            ],
            axis: Axis::Y,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_CAST,
        },
    )
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
