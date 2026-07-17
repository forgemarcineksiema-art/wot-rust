//! The Jagdtiger: blueprint-born, the first CASEMATE on the blueprint path. The fighting
//! compartment is welded into the hull — its 25° side walls are lofted on the SAME armor
//! planes as the hull's leaned upper sides, one unbroken slope from sponson to roof — with
//! the 250 mm face leaning only 15°, the commander's periscope housing instead of a cupola,
//! and the 12.8 cm PaK 44's near-three-metre overhang past the bow. The sim clamps a
//! casemate's yaw at zero; the armor volume is a fixed prism (`TurretForm::Casemate`).

use game_core::{HitboxProfile, MountFrames, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_HARD, add_oval_mantlet_socket, assemble, blueprint_prism_hull,
    blueprint_running_gear, build_gun, shade_hull,
};
use crate::{Axis, BakedVehicle, LoftSection, LoftSpec, MaterialRole, MeshBuilder};

pub(crate) fn jagdtiger(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::Jagdtiger).expect("Jagdtiger has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&super::deck_details::jagdtiger_deck(&bp))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    // The MASSIVE cast collar of the 12.8 cm at the casemate face (dossier JT.2): the photo
    // shows a near-round casting over a metre across, not the slight ring the shared socket
    // drew — same oval construction as the T-54 mask and the Tiger II Turmblende, its own
    // scales. Height clamped so the casting stays ON the face: the gun axis sits 0.39 m over
    // the ring plane, and a lip spilling below it would inflate the casemate-height ratio.
    let casemate = add_oval_mantlet_socket(
        casemate_side_racks(casemate_superstructure(t), t),
        bp.gun.trunnion_y,
        mantlet,
        1.50,
        1.02,
        14,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.20,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::Jagdtiger, hull, casemate, gun, *mounts)
}

/// The fixed fighting compartment: a prism lofted from the casemate plan whose long side walls
/// stand ON the armor side planes (the hull's own 25° flank continued), the 250 mm face leaning
/// its 15°, and the rear wall at its 5°. No cupola — the commander gets a low periscope
/// housing on the right roof.
fn casemate_superstructure(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let height = t.roof_y - t.ring_y;
    let (front_in, side_in, rear_in) = (
        height * t.front_slope_deg.to_radians().tan(),
        height * t.side_slope_deg.to_radians().tan(),
        height * t.rear_slope_deg.to_radians().tan(),
    );
    let ring_plan = vec![
        Vec2::new(0.90, front_z),
        Vec2::new(t.plan_half_width, front_z - 0.85),
        Vec2::new(t.plan_half_width, rear_z + 0.55),
        Vec2::new(0.95, rear_z),
        Vec2::new(-0.95, rear_z),
        Vec2::new(-t.plan_half_width, rear_z + 0.55),
        Vec2::new(-t.plan_half_width, front_z - 0.85),
        Vec2::new(-0.90, front_z),
    ];
    let side_roof = t.plan_half_width - side_in;
    let roof_plan = vec![
        Vec2::new(side_roof - 0.05, front_z - front_in),
        Vec2::new(side_roof, front_z - 0.85),
        Vec2::new(side_roof, rear_z + 0.55),
        Vec2::new(side_roof - 0.03, rear_z + rear_in),
        Vec2::new(-(side_roof - 0.03), rear_z + rear_in),
        Vec2::new(-side_roof, rear_z + 0.55),
        Vec2::new(-side_roof, front_z - 0.85),
        Vec2::new(-(side_roof - 0.05), front_z - front_in),
    ];
    MeshBuilder::new()
        .loft(
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
        // The commander's periscope housing: a low box, not a Tiger cupola.
        .plate_box(
            Vec3::new(t.cupola_x, t.roof_y + 0.035, t.cupola_z),
            Vec3::new(t.cupola_radius, 0.035, t.cupola_radius),
            0.02,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
}

/// Spare-shoe rows racked ON the casemate side walls (dossier JT.2, photo: rows of brackets
/// with track shoes on the flank). Fittings rule: the wall leans 25° inward with height, so
/// each shoe's outer face lies ON the armor plane at its TOP edge — everything at or under
/// the plane, nothing floating in un-hittable air.
fn casemate_side_racks(mut builder: MeshBuilder, t: &TurretShape) -> MeshBuilder {
    let lean = t.side_slope_deg.to_radians().tan();
    let row_y = t.ring_y + 0.42;
    let half_y = 0.115;
    let thickness = 0.075;
    // A shoe is a PARALLELOGRAM prism leaning WITH the wall (an axis-aligned box either sinks
    // into the plate or floats off it on a 25-degree lean): its outer face follows
    // wall_x(y) = plan_half_width - (y - ring_y) * tan(lean), proud by `thickness`.
    let wall_x = |y: f32| t.plan_half_width - (y - t.ring_y) * lean;
    let (y0, y1) = (row_y - half_y, row_y + half_y);
    for sign in [-1.0_f32, 1.0] {
        let (xo0, xo1) = (wall_x(y0) + thickness, wall_x(y1) + thickness);
        let section = if sign > 0.0 {
            vec![
                Vec2::new(xo0 - thickness, y0),
                Vec2::new(xo0, y0),
                Vec2::new(xo1, y1),
                Vec2::new(xo1 - thickness, y1),
            ]
        } else {
            vec![
                Vec2::new(-(xo0 - thickness), y0),
                Vec2::new(-(xo1 - thickness), y1),
                Vec2::new(-xo1, y1),
                Vec2::new(-xo0, y0),
            ]
        };
        for i in 0..4 {
            let z = t.ring_z - 1.05 + i as f32 * 0.50;
            builder = builder.extrude(
                Vec3::new(0.0, 0.0, z),
                crate::ExtrudeSpec {
                    section: section.clone(),
                    axis: Axis::Z,
                    half_depth: 0.20,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
        }
    }
    builder
}
