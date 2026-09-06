//! The welded box turret as library parts (Forge 2.0 K3, step 4b): the shell lofted from the
//! blueprint's plan (a flat front plate leaned by `front_slope_deg` — the armour's own plane —
//! side walls at `plan_half_width`, a faceted bent rear or a flat rear plate), the stowage bin
//! whose back face closes the rear armour plane when the turret carries one, the cast drum
//! cupola, the ring collar and the mantlet socket — up to five parts with their own keys, so the
//! inventory sees a shell, a bin, a cupola, a ring and a socket. The construction is the Tiger
//! recipe's, lifted below the seam; the plan's proportions come from the visual file
//! (`WeldedTurretVisual`), the armour numbers from the blueprint. The Henschel turret (the
//! Tiger II, 2026-09-06) is the same loft with `leaned_walls` — every wall retreats at the roof
//! by ITS plate slope, the armour's leaned planes — a `flat_rear_half_width` plate on the rear
//! armour plane instead of the bustle facets, no bin, and the Turmblende's oval socket.

use game_core::{TurretShape, VehicleBlueprint, WeldedTurretVisual};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, LoftSection, LoftSpec, MaterialRole, MeshBuilder, SmoothingGroup, SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;
use crate::turret_fittings::{
    add_german_cast_cupola, add_mantlet_socket, add_oval_mantlet_socket, add_turret_ring,
};

/// The welded turret for `bp`, or `None` when its visual file authors no welded turret.
pub fn welded_turret_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?.welded_turret?;
    Some(welded_turret_parts(bp, &visual))
}

/// Shell, bin (when `bin_depth` claims any of the prism), cupola, ring collar and mantlet socket.
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
    let mut parts =
        vec![part("turret_shell", MaterialRole::RolledArmor, SG_HARD, plate_shell(t, v).build())];
    if v.bin_depth > 0.0 {
        parts.push(part(
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
        ));
    }
    parts.push(part(
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
    ));
    parts.push(part(
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
    ));
    let segments = usize::from(v.socket_segments);
    let socket = match v.socket_scale {
        Some((x_scale, y_scale)) => add_oval_mantlet_socket(
            MeshBuilder::new(),
            bp.gun.trunnion_y,
            mantlet,
            x_scale,
            y_scale,
            segments,
        ),
        None => add_mantlet_socket(MeshBuilder::new(), bp.gun.trunnion_y, mantlet, segments),
    };
    parts.push(part("mantlet_socket", MaterialRole::CastArmor, SmoothingGroup(6), socket.build()));
    parts
}

/// The shell: a prism lofted from the turret's plan between the ring seat and the roof. The
/// plan is authored front-first and swept through +x to the rear. The front plate always leans
/// its `front_slope_deg`; with `leaned_walls` the side and rear walls lean theirs too (the
/// corner points stay a hair inside the converged side line so the plan remains convex for the
/// loft kernel), otherwise they stand vertical on the plan.
fn plate_shell(t: &TurretShape, v: &WeldedTurretVisual) -> MeshBuilder {
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length + v.bin_depth;
    let height = t.roof_y - t.ring_y;
    let front_in = height * t.front_slope_deg.to_radians().tan();
    let (side_in, rear_in) = if v.leaned_walls {
        (height * t.side_slope_deg.to_radians().tan(), height * t.rear_slope_deg.to_radians().tan())
    } else {
        (0.0, 0.0)
    };
    // The starboard half of the plan at a station: `side_x` is where the side wall stands,
    // `front_dz` / `rear_dz` how far the front and rear plates have retreated, and the hairs
    // keep the leaned corners inside the converged side line.
    let half = |side_x: f32, front_dz: f32, rear_dz: f32, hairs: (f32, f32)| -> Vec<Vec2> {
        let mut half = vec![
            Vec2::new(v.cheek_x.min(side_x - hairs.0), front_z - front_dz),
            Vec2::new(side_x, front_z - v.cheek_setback),
            Vec2::new(side_x, t.ring_z - v.wall_end_behind_ring),
        ];
        match v.flat_rear_half_width {
            Some(width) => half.push(Vec2::new(width.min(side_x - hairs.1), rear_z + rear_dz)),
            None => {
                half.extend(v.bustle.iter().map(|(x, dz)| Vec2::new(*x, rear_z + dz + rear_dz)))
            }
        }
        half
    };
    // The whole plan: the starboard half, the bustle's centre point (a flat rear needs none),
    // then the port half mirrored, rear to front.
    let ring = |half: Vec<Vec2>, rear_dz: f32| -> Vec<Vec2> {
        let mut plan = half.clone();
        if v.flat_rear_half_width.is_none() {
            plan.push(Vec2::new(0.0, rear_z + rear_dz));
        }
        plan.extend(half.iter().rev().map(|p| Vec2::new(-p.x, p.y)));
        plan
    };
    let hairs = if v.leaned_walls { (0.04, 0.06) } else { (0.0, 0.0) };
    let ring_plan = ring(half(t.plan_half_width, 0.0, 0.0, (0.0, 0.0)), 0.0);
    let roof_plan = ring(half(t.plan_half_width - side_in, front_in, rear_in, hairs), rear_in);
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
