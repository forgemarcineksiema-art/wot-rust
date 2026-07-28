//! The Panther II: blueprint-born, the last German off the legacy path. The wedge is the
//! character — the fleet's steepest German glacis (55°) and 29° leaned sides lofted on the
//! armor volume planes, the deliberately NARROW Schmalturm with its cone Saukopf mantlet and
//! hard-converging cheeks, and seven overlapped steel wheels of the Tiger II school.

use game_core::{HitboxProfile, HullShape, MountFrames, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_HARD, add_german_cast_cupola, add_oval_mantlet_socket, add_turret_ring, assemble,
    blueprint_prism_hull, build_gun_with_mantlet_scale, shade_hull,
};
use crate::{
    Axis, BakedVehicle, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder,
    ProfilePoint, RevolveSpec,
};

pub(crate) fn panther_ii(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::PantherII).expect("Panther II has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&super::deck_details::panther_ii_deck(&bp))
            .append(&panther_hull_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    // The G-blende: the curved cast mantlet band spanning the narrow front plate — the same
    // oval construction as the T-54 mask / Tiger II Turmblende, Panther G's own scales.
    let turret = add_oval_mantlet_socket(
        add_turret_ring(
            add_german_cast_cupola(
                panther_g_turret(t),
                t.cupola_x,
                t.cupola_z,
                t.roof_y,
                t.cupola_radius,
                t.cupola_proud_m(&bp.hull),
            ),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            0.12,
            14,
        ),
        bp.gun.trunnion_y,
        mantlet,
        2.00,
        1.10,
        12,
    )
    .build();

    let gun = build_gun_with_mantlet_scale(
        &GunPlan {
            axis_y: bp.gun.trunnion_y,
            breech_z: bp.gun.trunnion_z - 0.28,
            muzzle_z: bp.gun.muzzle_z,
            radius: bp.gun.barrel_radius,
            segments: bp.gun.segments,
            mantlet,
            evacuator: bp.gun.evacuator,
            muzzle_brake: bp.gun.muzzle_brake,
        },
        2.00,
        1.10,
    );

    assemble(VehicleKind::PantherII, hull, turret, gun, *mounts)
}

/// The Panther Ausf. G turret (dossier PII.1, Fort Benning specimen): a NARROW leaned front
/// plate whose cheeks SPLAY outward in plan all the way to the full beam at the rear third,
/// closed by a wide flat rear wall with the bustle — the opposite read of the old Schmalturm
/// wedge, whose cheeks converged rearward. Walls stand ON the armor prism planes at the seat.
fn panther_g_turret(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let height = t.roof_y - t.ring_y;
    let (front_in, side_in, rear_in) = (
        height * t.front_slope_deg.to_radians().tan(),
        height * t.side_slope_deg.to_radians().tan(),
        height * t.rear_slope_deg.to_radians().tan(),
    );
    // Front plate half-width ~0.60 (the G plate behind the Blende), full beam from the rear
    // third back, slight corner chamfers at the rear wall.
    let ring_plan = vec![
        Vec2::new(0.60, front_z),
        Vec2::new(t.plan_half_width, t.ring_z - 0.35),
        Vec2::new(t.plan_half_width, rear_z + 0.22),
        Vec2::new(t.plan_half_width - 0.12, rear_z),
        Vec2::new(-(t.plan_half_width - 0.12), rear_z),
        Vec2::new(-t.plan_half_width, rear_z + 0.22),
        Vec2::new(-t.plan_half_width, t.ring_z - 0.35),
        Vec2::new(-0.60, front_z),
    ];
    let side_roof = t.plan_half_width - side_in;
    let roof_rear = rear_z + rear_in;
    let roof_plan = vec![
        Vec2::new(0.50, front_z - front_in),
        Vec2::new(side_roof, t.ring_z - 0.35),
        Vec2::new(side_roof, roof_rear + 0.22),
        Vec2::new(side_roof - 0.10, roof_rear),
        Vec2::new(-(side_roof - 0.10), roof_rear),
        Vec2::new(-side_roof, roof_rear + 0.22),
        Vec2::new(-side_roof, t.ring_z - 0.35),
        Vec2::new(-0.50, front_z - front_in),
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
