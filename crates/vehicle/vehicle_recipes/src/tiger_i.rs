//! The Tiger I Ausf. E: blueprint-born. The slab is the character — a nearly vertical hull of
//! flat plates on the SAME planes the armor volumes shoot against, a broad horseshoe turret of
//! genuinely vertical walls (flat front plate carrying the boxy mantlet, faceted bent rear),
//! the drum cupola on the left, the Rommelkiste stowage bin closing the turret's rear armor
//! plane, twin exhaust stacks on the rear plate, and the interleaved eight-wheel running gear.

use game_core::roundness::round_segments;
use game_core::{HitboxProfile, HullShape, MountFrames, TurretShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, SG_HARD, add_german_cast_cupola, add_mantlet_socket, add_turret_ring, assemble,
    gun_group, shade_hull,
};
use vehicle_geometry::{
    Axis, BakedVehicle, ExtrudeSpec, GeometryMesh, LoftSection, LoftSpec, MaterialRole,
    MeshBuilder, ProfilePoint, RevolveSpec,
};

pub(crate) fn tiger_i(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let pieces = tiger_i_pieces(hitbox, mounts, super::deck_details::DeckOmit::default());
    let concat = |pieces: Vec<(&'static str, GeometryMesh)>| {
        revolve::merge(&pieces.into_iter().map(|(_, mesh)| mesh).collect::<Vec<_>>())
    };
    assemble(
        VehicleKind::TigerI,
        concat(pieces.hull),
        concat(pieces.turret),
        concat(pieces.gun),
        pieces.mounts,
    )
}

/// The Tiger I as the pieces its recipe is made of (Forge 2.0 K3): the slab hull, the deck
/// with its guards and flaps, the hull details (stacks, spare links, visor, bow ball), the
/// horseshoe turret with cupola, ring and mantlet socket, and the gun group. `tiger_i` is
/// these concatenated in this order and welded — `shade_hull` is a per-vertex height shade,
/// so shading the pieces or their concatenation is the same bake.
pub(crate) fn tiger_i_pieces(
    _hitbox: &HitboxProfile,
    mounts: &MountFrames,
    omit: super::deck_details::DeckOmit,
) -> super::RecipePieces {
    let bp = super::active_blueprint(VehicleKind::TigerI).expect("Tiger I has a blueprint");
    let hull = vec![
        (
            "recipe_hull_slab",
            shade_hull(tiger_slab_hull(&bp.hull, bp.armor.hull_bow_shelf).build()),
        ),
        ("recipe_hull_deck", shade_hull(super::deck_details::tiger_i_deck(&bp, omit))),
        ("recipe_hull_details", shade_hull(tiger_hull_details(&bp.hull, bp.armor.hull_bow_shelf))),
    ];

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_german_cast_cupola(
                horseshoe_turret(t),
                t.cupola_x,
                t.cupola_z,
                t.roof_y,
                t.cupola_radius,
                t.cupola_proud_m(&bp.hull),
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

    let gun = gun_group(
        VehicleKind::TigerI,
        &GunPlan {
            axis_y: bp.gun.trunnion_y,
            breech_z: bp.gun.trunnion_z - 0.30,
            muzzle_z: bp.gun.muzzle_z,
            radius: bp.gun.barrel_radius,
            segments: bp.gun.segments,
            mantlet,
            evacuator: bp.gun.evacuator,
            muzzle_brake: bp.gun.muzzle_brake,
        },
    );

    super::RecipePieces {
        hull,
        turret: vec![("recipe_turret", turret)],
        gun: vec![("recipe_gun", gun)],
        mounts: *mounts,
    }
}

/// The horseshoe: a vertical prism lofted from the turret's plan — a flat front plate (leaned
/// back by `front_slope_deg`, the SAME plane the armor prism bakes), straight vertical side
/// walls out at `plan_half_width` (the armor side planes), and a faceted bent rear wall — plus
/// the Rommelkiste stowage bin whose back face closes the rear armor plane at
/// `ring_z - plan_half_length`.
fn horseshoe_turret(t: &TurretShape) -> MeshBuilder {
    // The bin claims the last 0.21 m of the armor prism; the bent wall ends where it begins.
    let bin_depth = 0.21;
    let front_z = t.ring_z + t.plan_half_length;
    let rear_z = t.ring_z - t.plan_half_length + bin_depth;
    // Plan ring at the ring seat (x, z), authored front-first and swept through +x to the rear,
    // the same winding `cast_ring` uses. Front plate ~1.56 m wide, then the cheeks open to the
    // full beam, run straight, and bend around the bustle in facets.
    let ring_plan = vec![
        Vec2::new(0.78, front_z),
        Vec2::new(t.plan_half_width, front_z - 0.60),
        Vec2::new(t.plan_half_width, t.ring_z - 0.30),
        Vec2::new(0.86, rear_z + 0.32),
        Vec2::new(0.48, rear_z + 0.08),
        Vec2::new(0.0, rear_z),
        Vec2::new(-0.48, rear_z + 0.08),
        Vec2::new(-0.86, rear_z + 0.32),
        Vec2::new(-t.plan_half_width, t.ring_z - 0.30),
        Vec2::new(-t.plan_half_width, front_z - 0.60),
        Vec2::new(-0.78, front_z),
    ];
    // Only the front plate leans: at the roof its z retreats by the plate's slope run; the
    // vertical walls keep their plan.
    let lean = (t.roof_y - t.ring_y) * t.front_slope_deg.to_radians().tan();
    let roof_plan: Vec<Vec2> = ring_plan
        .iter()
        .map(|p| if p.y > front_z - 0.01 { Vec2::new(p.x, p.y - lean) } else { *p })
        .collect();
    let mut builder = MeshBuilder::new().loft(
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
    );
    // The Rommelkiste: its back face IS the rear armor plane — the plate a shell into the
    // bustle actually meets.
    builder = builder.chamfered_prism(
        Vec3::new(0.0, t.ring_y + 0.45, rear_z - bin_depth * 0.5),
        Vec3::new(0.80, 0.25, bin_depth * 0.5),
        0.04,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    builder
}

/// The slab hull, face-honest: unlike the generic `blueprint_hull` (which pivots the glacis at
/// the belly), every front and rear plate here is authored ON the plane equation the armor
/// volumes bake — the fold ridge at the sponson step, the driver's plate leaning its 9° above
/// it, the derived lower nose raking back below it, and the 8° rear pair. What you see leaning
/// is what the penetration model resolves. An authored bow shelf (`hull_bow_shelf`) sets the
/// driver's plate back from the nose line behind the near-horizontal glacis: the box behind
/// the plate and the shelf's wedge ahead of it are two convex extrusions on the volumes'
/// planes (the bow is concave at the shelf's top edge, and `extrude` sweeps convex sections).
fn tiger_slab_hull(hull: &HullShape, shelf: Option<(f32, f32)>) -> MeshBuilder {
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    // The lower-plate slope derives from the glacis exactly like the armor model's zone table.
    let lower = (hull.glacis_slope_deg * 0.45).to_radians().tan();
    let rear = hull.rear_slope_deg.to_radians().tan();
    let step = hull.sponson_y;
    let nose_y = hull.belly_y + hull.nose_rise;
    let lower_section = vec![
        Vec2::new(-hull.half_len + (step - hull.belly_y) * rear, hull.belly_y),
        Vec2::new(hull.half_len - (step - nose_y) * lower, nose_y),
        Vec2::new(hull.half_len, step),
        Vec2::new(-hull.half_len, step),
    ];
    // Where the driver's plate stands at height `y`: leaning back from its fold — the nose
    // line, or the shelf's top edge.
    let plate_z = |y: f32| match shelf {
        Some((top, setback)) => hull.half_len - setback - (y - top) * glacis,
        None => hull.half_len - (y - step) * glacis,
    };
    let upper_section = vec![
        Vec2::new(-hull.half_len, step),
        Vec2::new(plate_z(step), step),
        Vec2::new(plate_z(hull.deck_y), hull.deck_y),
        Vec2::new(-hull.half_len + (hull.deck_y - step) * rear, hull.deck_y),
    ];
    let upper = |section: Vec<Vec2>| ExtrudeSpec {
        section,
        axis: Axis::X,
        half_depth: hull.half_width,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
    };
    let mut builder = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: lower_section,
                axis: Axis::X,
                half_depth: hull.lower_half_width,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        )
        .extrude(Vec3::ZERO, upper(upper_section));
    if let Some((top, setback)) = shelf {
        // The shelf's wedge: the nose line, the plate's foot at the sponson, the shelf's top.
        let wedge = vec![
            Vec2::new(hull.half_len, step),
            Vec2::new(plate_z(step), step),
            Vec2::new(hull.half_len - setback, top),
        ];
        builder = builder.extrude(Vec3::ZERO, upper(wedge));
    }
    builder
}

/// Visual-only hull character inside the hitbox: the twin exhaust stacks standing proud on the
/// rear plate (the Tiger's tail-end signature) and the driver's visor + bow MG ball on the
/// near-vertical driver's plate.
fn plate_z_for(hull: &HullShape, shelf: Option<(f32, f32)>, y: f32) -> f32 {
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    match shelf {
        // On the driver's plate above the shelf's top edge: leaning back from the set-back fold.
        Some((top, setback)) if y >= top => hull.half_len - setback - (y - top) * glacis,
        _ => hull.half_len - (y - hull.belly_y - hull.nose_rise).max(0.0) * glacis,
    }
}

fn tiger_hull_details(hull: &HullShape, shelf: Option<(f32, f32)>) -> GeometryMesh {
    let mut builder = MeshBuilder::new();
    // Exhaust stacks: vertical cylinders flanking the engine plate.
    for x in [-0.62_f32, 0.62] {
        builder = builder.capped_revolve_at(
            Vec3::new(x, 0.0, -hull.half_len + 0.12),
            RevolveSpec {
                profile: vec![ProfilePoint::new(0.10, 1.00), ProfilePoint::new(0.10, 2.02)],
                axis: Axis::Y,
                // Roundness law: segments from the radius, not a hand-typed 10 that facets a
                // free-standing 0.10 m stack at 4.9 mm (well over the 2.8 mm tolerance).
                segments: round_segments(0.10),
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        );
    }
    // Late-E exhaust shields: three-sided sheet shrouds around each stack (the Feifel air
    // cleaners are deliberately ABSENT — the modelled variant is a late Ausf. E, which
    // dropped them; dossier identity in docs/vehicles/panzerkampfwagen-vi-tiger.md).
    for x in [-0.62_f32, 0.62] {
        let z = -hull.half_len + 0.12;
        // Back plate (behind the stack) and two side cheeks, open toward the engine deck.
        builder = builder.plate_box(
            Vec3::new(x, 1.55, z - 0.16),
            Vec3::new(0.17, 0.55, 0.012),
            0.012,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
        for side in [-1.0_f32, 1.0] {
            builder = builder.plate_box(
                Vec3::new(x + side * 0.17, 1.55, z - 0.02),
                Vec3::new(0.012, 0.55, 0.15),
                0.012,
                MaterialRole::RolledArmor,
                SG_HARD,
            );
        }
    }
    // Spare track links racked on the lower bow plate — the classic field-standard row.
    // Fittings rule: their outermost face lies ON the driver's-plate armor plane, never
    // proud of it, so nothing visible is un-hittable air.
    for i in 0..4 {
        let x = -0.72 + i as f32 * 0.48;
        builder = builder.plate_box(
            Vec3::new(x, 0.88, plate_z_for(hull, shelf, 0.88) - 0.065),
            Vec3::new(0.20, 0.115, 0.045),
            0.04,
            MaterialRole::TrackMetal,
            SG_HARD,
        );
    }
    // Driver's visor (left) and bow MG ball (right) riding the driver's plate. The plate stands
    // at glacis_slope_deg from vertical, so their seats sit just ahead of it — and where the
    // plate is set back behind the bow shelf, they go back with it.
    let plate_z = |y: f32| plate_z_for(hull, shelf, y);
    builder = builder.plate_box(
        Vec3::new(0.55, 1.62, plate_z(1.62) + 0.02),
        Vec3::new(0.20, 0.07, 0.05),
        0.03,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    builder = builder.capped_revolve_at(
        Vec3::new(-0.62, 1.58, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.13, plate_z(1.58) - 0.04),
                ProfilePoint::new(0.09, plate_z(1.58) + 0.09),
            ],
            axis: Axis::Z,
            // Roundness law over its widest radius: a hand-typed 10 facets this bow ball at
            // 6.4 mm — the worst single facet in the fleet, on a prominent feature.
            segments: round_segments(0.13),
            material: MaterialRole::CastArmor,
            smoothing: SG_HARD,
        },
    );
    builder.build()
}
