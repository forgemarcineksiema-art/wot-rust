//! The pike-nosed welded hull as library parts (Forge 2.0 K3, the IS-3, 2026-09-06): the tub
//! and the upper box as prisms ending where the pike takes over, and the pike bow itself —
//! four armour faces meeting at the fold ridge, the deck triangle behind the apex, the
//! side-wall fills, the sponson underside and the belly tip — every face authored on the EXACT
//! plane equations the armour volumes bake (same fold ridge, same slopes, same ±sweep), so the
//! visible bow and the penetration model are one geometry. The construction is the IS-3
//! recipe's (`is3_hull.rs`), lifted below the seam.

use game_core::{HullShape, VehicleBlueprint};
use glam::{Vec2, Vec3};
use vehicle_geometry::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;

/// The pike hull's plates: the tub, the upper box and the pike bow.
pub fn pike_hull_parts(bp: &VehicleBlueprint) -> Vec<VehiclePart> {
    let hull = &bp.hull;
    let pike = PikeFrame::of(hull);
    let rear_run = (hull.deck_y - hull.belly_y).max(0.1) * hull.rear_slope_deg.to_radians().tan();
    let part = |key: &'static str, mesh: GeometryMesh| VehiclePart {
        key: PartKey::new(key),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
        shape: PartShape::Mesh(mesh),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Solid,
    };
    // Body prisms end at vertical cuts where the pike takes over at full width.
    let lower_section = vec![
        Vec2::new(-hull.half_len, hull.belly_y),
        Vec2::new(pike.belly_corner.z, hull.belly_y),
        Vec2::new(pike.belly_corner.z, hull.sponson_y),
        Vec2::new(-hull.half_len + rear_run * 0.5, hull.sponson_y),
    ];
    let upper_section = vec![
        Vec2::new(-hull.half_len + rear_run * 0.5, hull.sponson_y),
        Vec2::new(pike.deck_corner.z, hull.sponson_y),
        Vec2::new(pike.deck_corner.z, hull.deck_y),
        Vec2::new(-hull.half_len + rear_run, hull.deck_y),
    ];
    let prism = |section: Vec<Vec2>, half_depth: f32| {
        MeshBuilder::new()
            .extrude(
                Vec3::ZERO,
                ExtrudeSpec {
                    section,
                    axis: Axis::X,
                    half_depth,
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            .build()
    };
    vec![
        part("slab_tub", prism(lower_section, hull.lower_half_width)),
        part("slab_upper_box", prism(upper_section, hull.half_width)),
        part("slab_bow_pike", pike_bow(hull, &pike)),
    ]
}

/// All the pike's anchor points, derived from the SAME plane equations the armour volumes
/// bake: the fold ridge at the sponson step, the upper faces (glacis slope + plan sweep), and
/// the lower faces (the derived lower-plate slope + the same sweep).
struct PikeFrame {
    fold: Vec3,
    apex: Vec3,
    deck_corner: Vec3,
    step_corner: Vec3,
    foot: Vec3,
    tub_step_corner: Vec3,
    belly_corner: Vec3,
}

impl PikeFrame {
    fn of(hull: &HullShape) -> Self {
        let sweep = hull.pike_sweep_deg.to_radians();
        let glacis = hull.glacis_slope_deg.to_radians();
        // The lower-plate slope derives from the glacis exactly like the armour model's zone
        // table.
        let lower = (hull.glacis_slope_deg * 0.45).to_radians();
        let (step, deck, belly) = (hull.sponson_y, hull.deck_y, hull.belly_y);
        let upper = Vec3::new(sweep.sin() * glacis.cos(), glacis.sin(), sweep.cos() * glacis.cos());
        let lower_n = Vec3::new(sweep.sin() * lower.cos(), -lower.sin(), sweep.cos() * lower.cos());
        let fold = Vec3::new(0.0, step, hull.half_len);
        let z_on = |n: Vec3, x: f32, y: f32| (n.dot(fold) - n.x * x - n.y * y) / n.z;
        Self {
            fold,
            apex: Vec3::new(0.0, deck, z_on(upper, 0.0, deck)),
            deck_corner: Vec3::new(hull.half_width, deck, z_on(upper, hull.half_width, deck)),
            step_corner: Vec3::new(hull.half_width, step, z_on(upper, hull.half_width, step)),
            foot: Vec3::new(0.0, belly, z_on(lower_n, 0.0, belly)),
            tub_step_corner: Vec3::new(
                hull.lower_half_width,
                step,
                z_on(lower_n, hull.lower_half_width, step),
            ),
            belly_corner: Vec3::new(
                hull.lower_half_width,
                belly,
                z_on(lower_n, hull.lower_half_width, belly),
            ),
        }
    }
}

/// The pike bow: four armour faces, the deck triangle behind the apex, the side-wall fills, the
/// sponson underside and the belly tip.
fn pike_bow(hull: &HullShape, pike: &PikeFrame) -> GeometryMesh {
    let material = MaterialRole::RolledArmor;
    let mut builder = MeshBuilder::new();
    for side in [1.0_f32, -1.0] {
        let at = |p: Vec3| Vec3::new(p.x * side, p.y, p.z);
        // Faces are authored CCW for the +x side; the mirror flips winding, so swap two points.
        let quad = |b: &mut MeshBuilder, points: [Vec3; 4]| {
            if side > 0.0 {
                b.push_quad(points, material, SG_HARD);
            } else {
                b.push_quad(
                    [at(points[3]), at(points[2]), at(points[1]), at(points[0])],
                    material,
                    SG_HARD,
                );
            }
        };
        let tri = |b: &mut MeshBuilder, points: [Vec3; 3]| {
            if side > 0.0 {
                b.push_tri(points, material, SG_HARD);
            } else {
                b.push_tri([at(points[2]), at(points[1]), at(points[0])], material, SG_HARD);
            }
        };
        // The upper pike face (outward: up-forward-outboard) and the lower one (down-forward-
        // outboard), both on their armour planes.
        quad(&mut builder, [pike.apex, pike.fold, pike.step_corner, pike.deck_corner]);
        quad(&mut builder, [pike.fold, pike.foot, pike.belly_corner, pike.tub_step_corner]);
        // The side-wall fills under the faces' slanted top edges, at the hull and tub widths.
        tri(
            &mut builder,
            [
                Vec3::new(hull.half_width, hull.deck_y, pike.deck_corner.z),
                pike.step_corner,
                Vec3::new(hull.half_width, hull.sponson_y, pike.deck_corner.z),
            ],
        );
        tri(
            &mut builder,
            [
                Vec3::new(hull.lower_half_width, hull.sponson_y, pike.belly_corner.z),
                pike.tub_step_corner,
                Vec3::new(hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            ],
        );
        // The sponson underside over the pike zone: the boundary walked, then fanned from one
        // anchor, so the region is tiled once — the fold, the tub step corner and the step
        // corner are collinear (both pike planes pass through the fold with one sweep).
        let under_anchor = Vec3::new(hull.lower_half_width, hull.sponson_y, pike.belly_corner.z);
        let under_inner = Vec3::new(hull.lower_half_width, hull.sponson_y, pike.deck_corner.z);
        let under_outer = Vec3::new(hull.half_width, hull.sponson_y, pike.deck_corner.z);
        let boundary =
            [pike.fold, pike.tub_step_corner, pike.step_corner, under_outer, under_inner];
        for pair in boundary.windows(2) {
            tri(&mut builder, [pair[0], under_anchor, pair[1]]);
        }
    }
    // The deck triangle behind the apex (facing up) and the belly tip under the pike (down).
    builder.push_tri(
        [
            Vec3::new(hull.half_width, hull.deck_y, pike.deck_corner.z),
            Vec3::new(-hull.half_width, hull.deck_y, pike.deck_corner.z),
            pike.apex,
        ],
        material,
        SG_HARD,
    );
    builder.push_tri(
        [
            Vec3::new(-hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            Vec3::new(hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            pike.foot,
        ],
        material,
        SG_HARD,
    );
    builder.build()
}
