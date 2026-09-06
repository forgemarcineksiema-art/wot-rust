//! The welded-in casemate as library parts (Forge 2.0 K3, the Jagdtiger, 2026-09-06): the fixed
//! fighting compartment lofted from the blueprint's plan — a rectangular prism whose four walls
//! lean their own slopes (`TurretShape`: front 15°, sides 25° ON the hull's side planes, rear 5°)
//! — the commander's low periscope housing where a turret would carry a cupola, the ventilator
//! dome, the two flush crew hatches on the rear roof, the massive cast collar of the gun seated
//! ring by ring on the leaning face, and the spare-shoe racks hung on the side walls. The
//! construction is the Jagdtiger recipe's, lifted below the seam; the proportions come from the
//! visual file (`CasemateVisual`), the armour numbers from the blueprint.

use game_core::{CasemateVisual, ShoeRackVisual, TurretShape, VehicleBlueprint};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudeSpec, GeometryMesh, GeometryVertex, LoftSection, LoftSpec, MaterialRole,
    MeshBuilder, SmoothingGroup, SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::{SG_CUPOLA, SG_HARD, SG_MANTLET};
use crate::turret_fittings::add_flush_ring_hatch;

/// The casemate for `bp`, or `None` when its visual file authors no casemate.
pub fn casemate_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?.casemate?;
    Some(casemate_parts(bp, &visual))
}

/// Shell, periscope housing, ventilator, the two flush hatches, the cast collar, the shoe racks.
pub fn casemate_parts(bp: &VehicleBlueprint, v: &CasemateVisual) -> Vec<VehiclePart> {
    let t = &bp.turret;
    let part = |key: PartKey,
                material: MaterialRole,
                smoothing: SmoothingGroup,
                lod: PartLod,
                mesh: GeometryMesh| VehiclePart {
        key,
        submesh: SubmeshKind::Turret,
        material,
        smoothing,
        shape: PartShape::Mesh(mesh),
        lod,
        generator: GeneratorKind::Sweep,
    };
    let mut parts = vec![part(
        PartKey::new("turret_shell"),
        MaterialRole::RolledArmor,
        SG_HARD,
        PartLod::Silhouette,
        casemate_shell(t).build(),
    )];
    // The commander's periscope housing: a low box, not a Tiger cupola — the blueprint's cupola
    // station and radius, the visual file's height.
    parts.push(part(
        PartKey::new("periscope_housing"),
        MaterialRole::RolledArmor,
        SG_HARD,
        PartLod::Detail,
        MeshBuilder::new()
            .plate_box(
                Vec3::new(t.cupola_x, t.roof_y + v.periscope_housing_half_height, t.cupola_z),
                Vec3::new(t.cupola_radius, v.periscope_housing_half_height, t.cupola_radius),
                0.02,
                MaterialRole::RolledArmor,
                SG_HARD,
            )
            .build(),
    ));
    if let Some((x, dz, half)) = v.ventilator {
        parts.push(part(
            PartKey::new("turret_ventilator"),
            MaterialRole::CastArmor,
            SG_CUPOLA,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    Vec3::new(x, t.roof_y + half * 0.26, t.ring_z + dz),
                    Vec3::new(half, half * 0.26, half),
                    half * 0.16,
                    MaterialRole::CastArmor,
                    SG_CUPOLA,
                )
                .build(),
        ));
    }
    if let Some((x, dz_behind, radius)) = v.hatches {
        for (i, sign) in [1.0_f32, -1.0].into_iter().enumerate() {
            parts.push(part(
                PartKey::indexed("casemate_hatch", i as u16),
                MaterialRole::CastArmor,
                SG_CUPOLA,
                PartLod::Detail,
                add_flush_ring_hatch(
                    MeshBuilder::new(),
                    sign * x,
                    t.ring_z - dz_behind,
                    t.roof_y,
                    radius,
                    sign,
                )
                .build(),
            ));
        }
    }
    parts.push(part(
        PartKey::new("mantlet_socket"),
        MaterialRole::CastArmor,
        SG_MANTLET,
        PartLod::Silhouette,
        gun_collar(t, bp.gun.trunnion_y, v),
    ));
    if let Some(rack) = v.racks {
        parts.extend(shoe_racks(t, &rack));
    }
    parts
}

/// Z of the casemate's face at height `y`: the face leans back with height at
/// `front_slope_deg`, so anything that must sit ON it is placed against this function.
fn face_plane_z(t: &TurretShape, y: f32) -> f32 {
    let lean = t.front_slope_deg.to_radians().tan();
    t.ring_z + t.plan_half_length - lean * (y - t.ring_y)
}

/// The fixed fighting compartment: a RECTANGULAR prism in plan whose face spans the FULL deck
/// width and meets each side wall at a sharp vertical corner; all four walls lean, the side
/// walls ON the hull's own armour planes so the slope runs unbroken from sponson to roof.
fn casemate_shell(t: &TurretShape) -> MeshBuilder {
    let (front_z, rear_z) = (t.ring_z + t.plan_half_length, t.ring_z - t.plan_half_length);
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

/// The massive cast collar of the gun at the casemate face: a near-round casting standing PROUD
/// of the plate, the barrel emerging from its throat. It cannot be a plain revolve — the face
/// leans, so a Z-aligned cylinder buries its lower half in the plate while its upper half
/// floats — so every ring is seated against [`face_plane_z`] at its OWN height: the root lies on
/// the armour plane the whole way round and each ring ahead of it stands off by its authored
/// stand-off. Squashed in Y by `collar_y_scale` so the whole collar stays on the face.
fn gun_collar(t: &TurretShape, axis_y: f32, v: &CasemateVisual) -> GeometryMesh {
    let segments = usize::from(v.collar_segments).max(8);
    let profile = &v.collar;
    let mut vertices = Vec::with_capacity(segments * profile.len());
    let normal_y = Vec3::new(0.0, 1.0 / v.collar_y_scale, 0.0);
    for segment in 0..segments {
        let angle = (segment as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let normal = (Vec3::X * cos + normal_y * sin).normalize_or_zero();
        for &(radius, stand_off) in profile {
            let y = axis_y + sin * radius * v.collar_y_scale;
            vertices.push(GeometryVertex::new(
                Vec3::new(cos * radius, y, face_plane_z(t, y) + stand_off),
                normal,
                MaterialRole::CastArmor,
                SG_MANTLET,
            ));
        }
    }
    let rows = profile.len() as u32;
    let mut indices = Vec::with_capacity(segments * (profile.len() - 1) * 6);
    for segment in 0..segments as u32 {
        let next = (segment + 1) % segments as u32;
        for row in 0..rows - 1 {
            let (a, b) = (segment * rows + row, next * rows + row);
            indices.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

/// A prism section lying ON a leaning armour plane: the outer face follows the plane at both
/// its bottom and top edge, standing `proud` off it, so nothing sinks into the plate or floats
/// in un-hittable air whatever the lean.
pub(crate) fn plate_pad(
    wall_x: impl Fn(f32) -> f32,
    sign: f32,
    y0: f32,
    y1: f32,
    proud: f32,
) -> Vec<Vec2> {
    let inner = [Vec2::new(wall_x(y0), y0), Vec2::new(wall_x(y1), y1)];
    let outer = [Vec2::new(wall_x(y0) + proud, y0), Vec2::new(wall_x(y1) + proud, y1)];
    let mut section = vec![inner[0], outer[0], outer[1], inner[1]];
    if sign < 0.0 {
        for point in &mut section {
            point.x = -point.x;
        }
        section.reverse();
    }
    section
}

/// Spare-shoe rows racked ON the casemate side walls: the wall leans inward with height, so each
/// shoe's outer face lies ON the armour plane; a continuous carrier rail under the row, and the
/// authored count of narrow shoes on it (broad plates read as windows cut into the wall).
fn shoe_racks(t: &TurretShape, rack: &ShoeRackVisual) -> Vec<VehiclePart> {
    let lean = t.side_slope_deg.to_radians().tan();
    let wall_x = |y: f32| t.plan_half_width - (y - t.ring_y) * lean;
    let row_y = t.ring_y + rack.row_y_above_ring;
    let (y0, y1) = (row_y - rack.half_y, row_y + rack.half_y);
    let shoes = usize::from(rack.shoes);
    let run = (shoes.saturating_sub(1)) as f32 * rack.pitch;
    let mut parts = Vec::with_capacity(2 + shoes * 2);
    let rack_part = |key: PartKey, mesh: GeometryMesh| VehiclePart {
        key,
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SG_HARD,
        shape: PartShape::Mesh(mesh),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    };
    for (side, sign) in [1.0_f32, -1.0].into_iter().enumerate() {
        parts.push(rack_part(
            PartKey::indexed("spare_track_rail", side as u16),
            MeshBuilder::new()
                .extrude(
                    Vec3::new(0.0, 0.0, t.ring_z + rack.first_z + run * 0.5),
                    ExtrudeSpec {
                        section: plate_pad(wall_x, sign, y0 - 0.055, y0 - 0.020, 0.030),
                        axis: Axis::Z,
                        half_depth: run * 0.5 + rack.pitch * 0.5,
                        material: MaterialRole::TrackMetal,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        ));
        for i in 0..shoes {
            let z = t.ring_z + rack.first_z + i as f32 * rack.pitch;
            parts.push(rack_part(
                PartKey::indexed("spare_track", (side * shoes + i) as u16),
                MeshBuilder::new()
                    .extrude(
                        Vec3::new(0.0, 0.0, z),
                        ExtrudeSpec {
                            section: plate_pad(wall_x, sign, y0, y1, rack.thickness),
                            axis: Axis::Z,
                            half_depth: rack.pitch * 0.37,
                            material: MaterialRole::TrackMetal,
                            smoothing: SG_HARD,
                        },
                    )
                    .build(),
            ));
        }
    }
    parts
}
