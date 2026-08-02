//! The Jagdtiger: blueprint-born, the first CASEMATE on the blueprint path. The fighting
//! compartment is welded into the hull — its 25° side walls are lofted on the SAME armor
//! planes as the hull's leaned upper sides, one unbroken slope from sponson to roof — with
//! the 250 mm face leaning only 15°, the commander's periscope housing instead of a cupola,
//! and the 12.8 cm PaK 44's near-three-metre overhang past the bow. The sim clamps a
//! casemate's yaw at zero; the armor volume is a fixed prism (`TurretForm::Casemate`).

use game_core::{HitboxProfile, MountFrames, TurretShape, VehicleBlueprint, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_CUPOLA, SG_HARD, SG_MANTLET, add_flush_ring_hatch, assemble, blueprint_prism_hull,
    build_gun, shade_hull,
};
use vehicle_geometry::{
    Axis, BakedVehicle, ExtrudeSpec, GeometryMesh, GeometryVertex, LoftSection, LoftSpec,
    MaterialRole, MeshBuilder,
};

pub(crate) fn jagdtiger(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::Jagdtiger).expect("Jagdtiger has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&super::deck_details::jagdtiger_deck(&bp))
            .append(&hull_flank_stowage(&bp))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let casemate = casemate_roof_furniture(casemate_side_racks(casemate_superstructure(t), t), t)
        .append(&casemate_gun_collar(t, bp.gun.trunnion_y))
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

/// Z of the casemate's 250 mm face at height `y`. The face leans back with height at
/// `front_slope_deg`, so anything that must sit ON it has to be placed against this function
/// rather than against one nominal Z.
fn face_z_at(t: &TurretShape, y: f32) -> f32 {
    t.ring_z + t.plan_half_length - (y - t.ring_y) * t.front_slope_deg.to_radians().tan()
}

/// The fixed fighting compartment: a RECTANGULAR prism in plan (dossier JT.3). The welded
/// original's superstructure is a box — its 250 mm face spans the FULL deck width and meets each
/// side wall at a sharp vertical corner. All four walls lean (front 15°, sides 25°, rear 5°), and
/// the side walls stand ON the hull's own armor planes, so the slope runs unbroken from sponson
/// to roof.
fn casemate_superstructure(t: &TurretShape) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length;
    let height = t.roof_y - t.ring_y;
    let (front_in, side_in, rear_in) = (
        height * t.front_slope_deg.to_radians().tan(),
        height * t.side_slope_deg.to_radians().tan(),
        height * t.rear_slope_deg.to_radians().tan(),
    );
    let w = t.plan_half_width;
    let ring_plan = vec![
        Vec2::new(w, front_z),
        Vec2::new(w, rear_z),
        Vec2::new(-w, rear_z),
        Vec2::new(-w, front_z),
    ];
    let side_roof = w - side_in;
    let roof_plan = vec![
        Vec2::new(side_roof, front_z - front_in),
        Vec2::new(side_roof, rear_z + rear_in),
        Vec2::new(-side_roof, rear_z + rear_in),
        Vec2::new(-side_roof, front_z - front_in),
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

/// The casemate roof (dossier JT.3): the commander's low periscope housing, the ventilator dome,
/// and the two flush crew hatches the crew actually got in through. Still NO cupola — every
/// fitting stays low enough that the silhouette tops out at the flat roof.
fn casemate_roof_furniture(builder: MeshBuilder, t: &TurretShape) -> MeshBuilder {
    let roof = t.roof_y;
    let builder = builder
        // The commander's periscope housing: a low box, not a Tiger cupola.
        .plate_box(
            Vec3::new(t.cupola_x, roof + 0.035, t.cupola_z),
            Vec3::new(t.cupola_radius, 0.035, t.cupola_radius),
            0.02,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
        // Ventilator dome on the forward roof, left of the periscope.
        .plate_box(
            Vec3::new(-0.42, roof + 0.030, t.ring_z + 0.62),
            Vec3::new(0.115, 0.030, 0.115),
            0.018,
            MaterialRole::CastArmor,
            SG_CUPOLA,
        );
    // Two flush crew hatches on the rear roof, hinges facing outboard.
    let hatch_z = t.ring_z - 0.95;
    add_flush_ring_hatch(
        add_flush_ring_hatch(builder, 0.52, hatch_z, roof, 0.27, 1.0),
        -0.52,
        hatch_z,
        roof,
        0.27,
        -1.0,
    )
}

/// The massive cast collar of the 12.8 cm at the casemate face (dossier JT.2/JT.3). The photo
/// shows a near-round casting over a metre across standing PROUD of the plate, the barrel
/// emerging from its throat and the moving ball working inside it — exactly as on the original.
///
/// It cannot be a plain revolve: the face leans 15°, so a Z-aligned cylinder buries its lower
/// half in the plate while its upper half floats free — which is how the collar came to read as
/// a cave rather than a casting. Every ring is therefore seated against [`face_z_at`] at its OWN
/// height, so the root lies on the armor plane the whole way round and each ring ahead of it
/// stands off that plane by the same amount.
fn casemate_gun_collar(t: &TurretShape, axis_y: f32) -> GeometryMesh {
    // (radius, stand-off ahead of the face plane). The root ring sits exactly ON the plate; the
    // throat stays wider than the barrel plus its elevation sweep so the gun never clips it.
    const PROFILE: [(f32, f32); 4] = [(0.60, 0.00), (0.57, 0.07), (0.44, 0.15), (0.26, 0.22)];
    const SEGMENTS: usize = 16;
    // The casting is an OVAL, wider than it is tall: a circle 1.20 m across would hang below the
    // ring plane and spill onto the forward deck, which is what the height clamp on the old
    // socket was for. Squashing it in Y keeps the whole collar on the face.
    const Y_SCALE: f32 = 0.617;

    let mut vertices = Vec::with_capacity(SEGMENTS * PROFILE.len());
    let normal_y = Vec3::new(0.0, 1.0 / Y_SCALE, 0.0);
    for segment in 0..SEGMENTS {
        let angle = (segment as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let normal = (Vec3::X * cos + normal_y * sin).normalize_or_zero();
        for (radius, stand_off) in PROFILE {
            let y = axis_y + sin * radius * Y_SCALE;
            vertices.push(GeometryVertex::new(
                Vec3::new(cos * radius, y, face_z_at(t, y) + stand_off),
                normal,
                MaterialRole::CastArmor,
                SG_MANTLET,
            ));
        }
    }

    let rows = PROFILE.len() as u32;
    let mut indices = Vec::with_capacity(SEGMENTS * (PROFILE.len() - 1) * 6);
    for segment in 0..SEGMENTS as u32 {
        let next = (segment + 1) % SEGMENTS as u32;
        for row in 0..rows - 1 {
            let (a, b) = (segment * rows + row, next * rows + row);
            indices.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

/// A prism section lying ON a leaning armor plane: the outer face follows the plane at both its
/// bottom and top edge, standing `proud` off it, so nothing sinks into the plate or floats in
/// un-hittable air whatever the lean.
fn leaning_pad(wall_x: impl Fn(f32) -> f32, sign: f32, y0: f32, y1: f32, proud: f32) -> Vec<Vec2> {
    let (xo0, xo1) = (wall_x(y0) + proud, wall_x(y1) + proud);
    if sign > 0.0 {
        vec![
            Vec2::new(wall_x(y0), y0),
            Vec2::new(xo0, y0),
            Vec2::new(xo1, y1),
            Vec2::new(wall_x(y1), y1),
        ]
    } else {
        vec![
            Vec2::new(-wall_x(y0), y0),
            Vec2::new(-wall_x(y1), y1),
            Vec2::new(-xo1, y1),
            Vec2::new(-xo0, y0),
        ]
    }
}

/// Spare-shoe rows racked ON the casemate side walls (dossier JT.2, photo: rows of brackets with
/// track shoes on the flank). Fittings rule: the wall leans 25° inward with height, so each
/// shoe's outer face lies ON the armor plane, everything at or under it.
///
/// The row is SIX narrow shoes on a continuous carrier rail (dossier JT.3), not four broad
/// plates: at plate width the shoes read as windows cut into the wall rather than as stowage.
fn casemate_side_racks(mut builder: MeshBuilder, t: &TurretShape) -> MeshBuilder {
    let lean = t.side_slope_deg.to_radians().tan();
    let row_y = t.ring_y + 0.42;
    let half_y = 0.115;
    let thickness = 0.075;
    let wall_x = |y: f32| t.plan_half_width - (y - t.ring_y) * lean;
    let (y0, y1) = (row_y - half_y, row_y + half_y);
    for sign in [-1.0_f32, 1.0] {
        // The carrier rail the shoes hang on, running the whole row.
        builder = builder.extrude(
            Vec3::new(0.0, 0.0, t.ring_z - 0.20),
            ExtrudeSpec {
                section: leaning_pad(wall_x, sign, y0 - 0.055, y0 - 0.020, 0.030),
                axis: Axis::Z,
                half_depth: 1.02,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        );
        for i in 0..6 {
            let z = t.ring_z - 1.05 + i as f32 * 0.34;
            builder = builder.extrude(
                Vec3::new(0.0, 0.0, z),
                ExtrudeSpec {
                    section: leaning_pad(wall_x, sign, y0, y1, thickness),
                    axis: Axis::Z,
                    half_depth: 0.125,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
        }
    }
    builder
}

/// Hull-flank stowage (dossier JT.3): tow cable, jack and its timber block, and the tool box —
/// the clutter that turns the Jagdtiger's enormous bare sponson slab into a used vehicle. Same
/// fittings rule as the casemate racks: the sponson's upper side leans 25° with height, so every
/// box lies ON that plane rather than floating off it.
fn hull_flank_stowage(bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let lean = bp.armor.hull_side.0.to_radians().tan();
    let wall_x = |y: f32| hull.half_width - (y - hull.sponson_y) * lean;
    // (centre Z, half length along Z, centre Y, half height, proud of the plate)
    let items = [
        (2.05_f32, 0.42_f32, 1.62_f32, 0.085_f32, 0.070_f32),
        (1.05, 0.30, 1.60, 0.100, 0.085),
        (0.20, 0.50, 1.63, 0.060, 0.050),
        (-1.55, 0.46, 1.61, 0.090, 0.075),
    ];
    let mut builder = MeshBuilder::new();
    for sign in [-1.0_f32, 1.0] {
        for (cz, half_z, cy, half_y, proud) in items {
            builder = builder.extrude(
                Vec3::new(0.0, 0.0, cz),
                ExtrudeSpec {
                    section: leaning_pad(wall_x, sign, cy - half_y, cy + half_y, proud),
                    axis: Axis::Z,
                    half_depth: half_z,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
        }
    }
    builder.build()
}
