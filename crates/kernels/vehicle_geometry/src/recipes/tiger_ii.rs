//! The Tiger II Ausf. B: blueprint-born. The slope is the character — the fleet's longest
//! glacis and 25° leaned upper sides lofted directly on the armor volume planes
//! ([`super::blueprint_prism_hull`]), a long faceted Henschel turret whose plates carry their
//! real 10°/21°/20° and whose bustle closes the rear armor plane, the low left cupola, and
//! the five-metre KwK 43 L/71 dominating the silhouette over nine overlapped road wheels.

use game_core::{HitboxProfile, HullShape, MountFrames, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_HARD, add_cupola, add_mantlet_socket, add_turret_ring, assemble,
    blueprint_deck_details, blueprint_prism_hull, blueprint_running_gear, build_gun, shade_hull,
};
use crate::{
    Axis, BakedVehicle, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder,
    ProfilePoint, RevolveSpec,
};

pub(crate) fn tiger_ii(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::TigerII)
        .expect("Tiger II has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .append(&tiger_ii_hull_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                henschel_turret(t),
                t.cupola_x,
                t.cupola_z,
                t.roof_y,
                t.cupola_radius,
                bp.hull.hitbox_center_y + bp.hull.hitbox_half_height - t.roof_y,
                true,
            ),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            0.13,
            14,
        ),
        bp.gun.trunnion_y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.35,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::TigerII, hull, turret, gun, *mounts)
}

/// The Henschel: a long faceted prism lofted from the turret plan. The mid side walls stand ON
/// the armor side planes (leaning their 21° between ring and roof), the front plate leans its
/// 10° from the armor front plane, and the bustle's rear wall closes the rear armor plane at
/// `ring_z - plan_half_length`, leaning its 20°. The plan tapers toward the narrow front plate.
fn henschel_turret(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let height = t.roof_y - t.ring_y;
    let (front_in, side_in, rear_in) = (
        height * t.front_slope_deg.to_radians().tan(),
        height * t.side_slope_deg.to_radians().tan(),
        height * t.rear_slope_deg.to_radians().tan(),
    );
    // Ring plan (x, z), swept front → +x → rear → -x like `cast_ring`: a narrow front plate,
    // cheeks opening to the full beam, long converging walls, and the bustle's rear wall.
    let ring_plan = vec![
        Vec2::new(0.80, front_z),
        Vec2::new(t.plan_half_width, front_z - 1.00),
        Vec2::new(t.plan_half_width, t.ring_z - 0.65),
        Vec2::new(0.85, rear_z),
        Vec2::new(-0.85, rear_z),
        Vec2::new(-t.plan_half_width, t.ring_z - 0.65),
        Vec2::new(-t.plan_half_width, front_z - 1.00),
        Vec2::new(-0.80, front_z),
    ];
    // Every wall retreats by ITS plate slope at the roof; the corner points stay a hair inside
    // the converged side line so the octagon remains convex for the loft kernel.
    let side_roof = t.plan_half_width - side_in;
    let roof_plan = vec![
        Vec2::new(side_roof - 0.04, front_z - front_in),
        Vec2::new(side_roof, front_z - 1.00),
        Vec2::new(side_roof, t.ring_z - 0.65),
        Vec2::new(side_roof - 0.06, rear_z + rear_in),
        Vec2::new(-(side_roof - 0.06), rear_z + rear_in),
        Vec2::new(-side_roof, t.ring_z - 0.65),
        Vec2::new(-side_roof, front_z - 1.00),
        Vec2::new(-(side_roof - 0.04), front_z - front_in),
    ];
    MeshBuilder::new().loft(
        Vec3::ZERO,
        LoftSpec {
            sections: vec![
                LoftSection::new(t.ring_y, ring_plan),
                LoftSection::new(t.roof_y, roof_plan),
            ],
            axis: Axis::Y,
            material: MaterialRole::RolledArmor,
            smoothing: SG_HARD,
            cap_ends: true,
        },
    )
}

/// Visual-only hull character inside the hitbox: twin exhausts hugging the 30° rear plate and
/// the driver's periscope housing on the long glacis.
fn tiger_ii_hull_details(hull: &HullShape) -> GeometryMesh {
    let mut builder = MeshBuilder::new();
    for x in [-0.55_f32, 0.55] {
        builder = builder.capped_revolve_at(
            Vec3::new(x, 0.0, -hull.half_len + 0.34),
            RevolveSpec {
                profile: vec![ProfilePoint::new(0.09, 0.95), ProfilePoint::new(0.09, 1.95)],
                axis: Axis::Y,
                segments: 10,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        );
    }
    // The driver's periscope box riding the glacis line, just left of the centre.
    let run = (hull.deck_y - hull.sponson_y) * hull.glacis_slope_deg.to_radians().tan();
    builder = builder.plate_box(
        Vec3::new(-0.60, hull.deck_y + 0.03, hull.half_len - run - 0.30),
        Vec3::new(0.16, 0.06, 0.12),
        0.03,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    builder.build()
}
