//! The Panther II: blueprint-born, the last German off the legacy path. The wedge is the
//! character — the fleet's steepest German glacis (55°) and 29° leaned sides lofted on the
//! armor volume planes, the deliberately NARROW Schmalturm with its cone Saukopf mantlet and
//! hard-converging cheeks, and seven overlapped steel wheels of the Tiger II school.

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

pub(crate) fn panther_ii(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::PantherII).expect("Panther II has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .append(&panther_hull_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                schmalturm(t),
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
            0.12,
            14,
        ),
        bp.gun.trunnion_y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.28,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::PantherII, hull, turret, gun, *mounts)
}

/// The Schmalturm: a deliberately narrow prism — a small leaned front plate barely wider than
/// the Saukopf, cheeks opening to the modest beam and converging hard (25°) to a slim roof,
/// the walls standing ON the armor prism planes at the ring seat.
fn schmalturm(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let height = t.roof_y - t.ring_y;
    let (front_in, side_in, rear_in) = (
        height * t.front_slope_deg.to_radians().tan(),
        height * t.side_slope_deg.to_radians().tan(),
        height * t.rear_slope_deg.to_radians().tan(),
    );
    let ring_plan = vec![
        Vec2::new(0.46, front_z),
        Vec2::new(t.plan_half_width, front_z - 0.85),
        Vec2::new(t.plan_half_width, t.ring_z - 0.55),
        Vec2::new(0.62, rear_z),
        Vec2::new(-0.62, rear_z),
        Vec2::new(-t.plan_half_width, t.ring_z - 0.55),
        Vec2::new(-t.plan_half_width, front_z - 0.85),
        Vec2::new(-0.46, front_z),
    ];
    let side_roof = t.plan_half_width - side_in;
    let roof_plan = vec![
        Vec2::new(0.36, front_z - front_in),
        Vec2::new(side_roof, front_z - 0.85),
        Vec2::new(side_roof, t.ring_z - 0.55),
        Vec2::new(side_roof - 0.03, rear_z + rear_in),
        Vec2::new(-(side_roof - 0.03), rear_z + rear_in),
        Vec2::new(-side_roof, t.ring_z - 0.55),
        Vec2::new(-side_roof, front_z - 0.85),
        Vec2::new(-0.36, front_z - front_in),
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

/// Visual-only hull character inside the hitbox: the Panther family's twin exhaust pipes on
/// the rear plate and the driver's periscope housing riding the long ramp.
fn panther_hull_details(hull: &HullShape) -> GeometryMesh {
    let mut builder = MeshBuilder::new();
    for x in [-0.45_f32, 0.45] {
        builder = builder.capped_revolve_at(
            Vec3::new(x, 0.0, -hull.half_len + 0.30),
            RevolveSpec {
                profile: vec![ProfilePoint::new(0.08, 0.95), ProfilePoint::new(0.08, 1.90)],
                axis: Axis::Y,
                segments: 10,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        );
    }
    let run = (hull.deck_y - hull.sponson_y) * hull.glacis_slope_deg.to_radians().tan();
    builder = builder.plate_box(
        Vec3::new(-0.55, hull.deck_y + 0.03, hull.half_len - run - 0.28),
        Vec3::new(0.15, 0.05, 0.11),
        0.03,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    builder.build()
}
