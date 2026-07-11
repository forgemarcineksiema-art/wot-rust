//! The Jagdtiger: blueprint-born, the first CASEMATE on the blueprint path. The fighting
//! compartment is welded into the hull — its 25° side walls are lofted on the SAME armor
//! planes as the hull's leaned upper sides, one unbroken slope from sponson to roof — with
//! the 250 mm face leaning only 15°, the commander's periscope housing instead of a cupola,
//! and the 12.8 cm PaK 44's near-three-metre overhang past the bow. The sim clamps a
//! casemate's yaw at zero; the armor volume is a fixed prism (`TurretForm::Casemate`).

use game_core::{HitboxProfile, MountFrames, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_HARD, add_mantlet_socket, assemble, blueprint_deck_details, blueprint_prism_hull,
    blueprint_running_gear, build_gun, shade_hull,
};
use crate::{Axis, BakedVehicle, LoftSection, LoftSpec, MaterialRole, MeshBuilder};

pub(crate) fn jagdtiger(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::Jagdtiger).expect("Jagdtiger has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let casemate =
        add_mantlet_socket(casemate_superstructure(t), bp.gun.trunnion_y, mantlet, 14).build();

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
