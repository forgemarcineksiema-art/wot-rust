//! The welded box turret as library parts (Forge 2.0 K3, step 4b): the horseshoe shell lofted
//! from the blueprint's plan (a flat front plate leaned by `front_slope_deg` — the armour's own
//! plane — vertical side walls at `plan_half_width`, a faceted bent rear), the stowage bin whose
//! back face closes the rear armour plane, the cast drum cupola, the ring collar and the mantlet
//! socket — five parts with their own keys, so the inventory sees a shell, a bin, a cupola, a
//! ring and a socket. The construction is the Tiger recipe's, lifted below the seam; the plan's
//! proportions come from the visual file (`WeldedTurretVisual`), the armour numbers from the
//! blueprint.

use game_core::{TurretShape, VehicleBlueprint, WeldedTurretVisual};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, LoftSection, LoftSpec, MaterialRole, MeshBuilder, SmoothingGroup, SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;
use crate::turret_fittings::{add_german_cast_cupola, add_mantlet_socket, add_turret_ring};

/// The welded turret for `bp`, or `None` when its visual file authors no welded turret.
pub fn welded_turret_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?.welded_turret?;
    Some(welded_turret_parts(bp, &visual))
}

/// Shell, bin, cupola, ring collar and mantlet socket.
pub fn welded_turret_parts(bp: &VehicleBlueprint, v: &WeldedTurretVisual) -> Vec<VehiclePart> {
    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let rear_z = t.ring_z - t.plan_half_length + v.bin_depth;
    let part =
        |key: &'static str, material: MaterialRole, smoothing: SmoothingGroup, mesh| VehiclePart {
            key: PartKey::new(key),
            submesh: SubmeshKind::Turret,
            material,
            smoothing,
            shape: PartShape::Mesh(mesh),
            lod: PartLod::Silhouette,
            generator: GeneratorKind::Sweep,
        };
    vec![
        part("turret_shell", MaterialRole::RolledArmor, SG_HARD, horseshoe_shell(t, v).build()),
        part(
            "turret_bin",
            MaterialRole::RolledArmor,
            SG_HARD,
            MeshBuilder::new()
                .chamfered_prism(
                    Vec3::new(0.0, t.ring_y + v.bin_rise, rear_z - v.bin_depth * 0.5),
                    Vec3::new(v.bin_half_width, v.bin_half_height, v.bin_depth * 0.5),
                    0.04,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .build(),
        ),
        part(
            "cupola_drum",
            MaterialRole::CastArmor,
            SmoothingGroup(3),
            add_german_cast_cupola(
                MeshBuilder::new(),
                t.cupola_x,
                t.cupola_z,
                t.roof_y,
                t.cupola_radius,
                t.cupola_proud_m(&bp.hull),
            )
            .build(),
        ),
        part(
            "turret_ring_collar",
            MaterialRole::RolledArmor,
            SmoothingGroup(7),
            add_turret_ring(
                MeshBuilder::new(),
                t.ring_z,
                t.ring_y,
                t.ring_radius,
                v.ring_height,
                usize::from(v.ring_segments),
            )
            .build(),
        ),
        part(
            "mantlet_socket",
            MaterialRole::CastArmor,
            SmoothingGroup(6),
            add_mantlet_socket(
                MeshBuilder::new(),
                bp.gun.trunnion_y,
                mantlet,
                usize::from(v.socket_segments),
            )
            .build(),
        ),
    ]
}

/// The horseshoe: a vertical prism lofted from the turret's plan. Plan ring at the ring seat,
/// authored front-first and swept through +x to the rear; only the front plate leans.
fn horseshoe_shell(t: &TurretShape, v: &WeldedTurretVisual) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length + v.bin_depth;
    let mut half: Vec<Vec2> = vec![
        Vec2::new(v.cheek_x, front_z),
        Vec2::new(t.plan_half_width, front_z - v.cheek_setback),
        Vec2::new(t.plan_half_width, t.ring_z - v.wall_end_behind_ring),
    ];
    half.extend(v.bustle.iter().map(|(x, dz)| Vec2::new(*x, rear_z + dz)));
    let mut ring_plan = half.clone();
    ring_plan.push(Vec2::new(0.0, rear_z));
    ring_plan.extend(half.iter().rev().map(|p| Vec2::new(-p.x, p.y)));
    let lean = (t.roof_y - t.ring_y) * t.front_slope_deg.to_radians().tan();
    let roof_plan: Vec<Vec2> = ring_plan
        .iter()
        .map(|p| if p.y > front_z - 0.01 { Vec2::new(p.x, p.y - lean) } else { *p })
        .collect();
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
