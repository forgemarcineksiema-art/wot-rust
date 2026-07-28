//! The IS-3 pike hull, face by face: the bow plates are authored on the EXACT plane equations
//! the armor volumes bake (same fold ridge, same slopes, same ±sweep), so the visible bow and
//! the penetration model are one geometry. Split from `is3.rs` for the file budget.

use game_core::HullShape;
use glam::{Vec2, Vec3};

use super::SG_HARD;
use crate::{Axis, ExtrudeSpec, MaterialRole, MeshBuilder};

/// All the pike's anchor points, derived from the SAME plane equations the armor volumes bake:
/// the fold ridge at the sponson step, the upper faces (glacis slope + plan sweep), and the
/// lower faces (the derived lower-plate slope + the same sweep).
struct PikeFrame {
    /// Fold ridge tip: the hull's forwardmost point, on the centerline at the sponson step.
    fold: Vec3,
    /// Upper ridge apex, where the two upper faces meet the deck.
    apex: Vec3,
    /// Upper face corner at the deck and the full hull width (+x side).
    deck_corner: Vec3,
    /// Upper face corner at the sponson step and the full hull width (+x side).
    step_corner: Vec3,
    /// Lower ridge foot, where the two lower faces meet the belly on the centerline.
    foot: Vec3,
    /// Lower face corner at the sponson step and the tub width (+x side).
    tub_step_corner: Vec3,
    /// Lower face corner at the belly and the tub width (+x side).
    belly_corner: Vec3,
}

fn pike_frame(hull: &HullShape) -> PikeFrame {
    let sweep = hull.pike_sweep_deg.to_radians();
    let glacis = hull.glacis_slope_deg.to_radians();
    // The lower-plate slope derives from the glacis exactly like the armor model's zone table.
    let lower = (hull.glacis_slope_deg * 0.45).to_radians();
    let (s, d, b) = (hull.sponson_y, hull.deck_y, hull.belly_y);
    let length = hull.half_len;

    // Upper face plane normal (right face): Ry(sweep) * (0, sin g, cos g).
    let upper = Vec3::new(sweep.sin() * glacis.cos(), glacis.sin(), sweep.cos() * glacis.cos());
    // Lower face plane normal (right face): Ry(sweep) * (0, -sin l, cos l).
    let lower_n = Vec3::new(sweep.sin() * lower.cos(), -lower.sin(), sweep.cos() * lower.cos());

    // z on a plane through the fold, at a given (x, y): n·p = n·fold.
    let fold = Vec3::new(0.0, s, length);
    let z_on = |n: Vec3, x: f32, y: f32| (n.dot(fold) - n.x * x - n.y * y) / n.z;

    PikeFrame {
        fold,
        apex: Vec3::new(0.0, d, z_on(upper, 0.0, d)),
        deck_corner: Vec3::new(hull.half_width, d, z_on(upper, hull.half_width, d)),
        step_corner: Vec3::new(hull.half_width, s, z_on(upper, hull.half_width, s)),
        foot: Vec3::new(0.0, b, z_on(lower_n, 0.0, b)),
        tub_step_corner: Vec3::new(
            hull.lower_half_width,
            s,
            z_on(lower_n, hull.lower_half_width, s),
        ),
        belly_corner: Vec3::new(hull.lower_half_width, b, z_on(lower_n, hull.lower_half_width, b)),
    }
}

/// The IS-3 hull: two body prisms (upper hull to the pike's deck corner, lower tub to the pike's
/// belly corner) plus the explicit pike bow — four armor faces, the deck triangle behind the
/// apex, side-wall fill triangles, the sponson underside, and the belly tip.
pub(super) fn is3_pike_hull(hull: &HullShape) -> MeshBuilder {
    let pike = pike_frame(hull);
    let rear_run = (hull.deck_y - hull.belly_y).max(0.1) * hull.rear_slope_deg.to_radians().tan();
    let material = MaterialRole::RolledArmor;

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
    let mut builder = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: lower_section,
                axis: Axis::X,
                half_depth: hull.lower_half_width,
                material,
                smoothing: SG_HARD,
            },
        )
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: upper_section,
                axis: Axis::X,
                half_depth: hull.half_width,
                material,
                smoothing: SG_HARD,
            },
        );

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

        // The upper pike face: apex -> fold -> step corner -> deck corner, all on the armor
        // plane (this winding puts the normal outward: up-forward-outboard).
        quad(&mut builder, [pike.apex, pike.fold, pike.step_corner, pike.deck_corner]);
        // The lower pike face: fold -> foot -> belly corner -> tub step corner (outward:
        // down-forward-outboard).
        quad(&mut builder, [pike.fold, pike.foot, pike.belly_corner, pike.tub_step_corner]);
        // Upper side-wall fill: the hull side continues under the face's slanted top edge.
        tri(
            &mut builder,
            [
                Vec3::new(hull.half_width, hull.deck_y, pike.deck_corner.z),
                pike.step_corner,
                Vec3::new(hull.half_width, hull.sponson_y, pike.deck_corner.z),
            ],
        );
        // Lower tub-wall fill, same idea at the tub width.
        tri(
            &mut builder,
            [
                Vec3::new(hull.lower_half_width, hull.sponson_y, pike.belly_corner.z),
                pike.tub_step_corner,
                Vec3::new(hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            ],
        );
        // Sponson underside over the pike zone: a flat fan at the step height, facing down.
        // The boundary is WALKED, then fanned from `under_anchor`, so the region is tiled once
        // and only once. The step edge runs fold -> tub step -> step corner because those three
        // are exactly collinear: both pike planes pass through the fold and carry the same plan
        // sweep, so at the fold's own height their traces are the SAME line (see the test below).
        // Fanning straight from the fold to the step corner instead swallows the tub corner's
        // window whole — that overlap was this hull's recorded winding debt.
        // (Each window is pushed as [boundary, anchor, next], which faces the plane down.)
        let under_anchor = Vec3::new(hull.lower_half_width, hull.sponson_y, pike.belly_corner.z);
        let under_inner = Vec3::new(hull.lower_half_width, hull.sponson_y, pike.deck_corner.z);
        let under_outer = Vec3::new(hull.half_width, hull.sponson_y, pike.deck_corner.z);
        let boundary =
            [pike.fold, pike.tub_step_corner, pike.step_corner, under_outer, under_inner];
        for pair in boundary.windows(2) {
            tri(&mut builder, [pair[0], under_anchor, pair[1]]);
        }
    }

    // Deck triangle behind the apex (full width, facing up).
    builder.push_tri(
        [
            Vec3::new(hull.half_width, hull.deck_y, pike.deck_corner.z),
            Vec3::new(-hull.half_width, hull.deck_y, pike.deck_corner.z),
            pike.apex,
        ],
        material,
        SG_HARD,
    );
    // Belly tip under the pike (full width, facing down).
    builder.push_tri(
        [
            Vec3::new(-hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            Vec3::new(hull.lower_half_width, hull.belly_y, pike.belly_corner.z),
            pike.foot,
        ],
        material,
        SG_HARD,
    );

    builder
}

#[cfg(test)]
mod tests {
    use game_core::{VehicleBlueprint, VehicleKind};

    use super::*;

    /// The fact the sponson-underside fan is built on, and the reason its boundary order is
    /// FORCED rather than chosen: at the fold's own height, both pike planes trace the same line.
    ///
    /// Each plane passes through the fold and is swept by the same `pike_sweep_deg` about the
    /// vertical, so each trace at `y = sponson_y` has slope `dz/dx = -tan(sweep)` — the glacis
    /// and lower slopes cancel out entirely. So the fold, the tub step corner and the step corner
    /// are collinear for ANY combination of slopes, sweep and widths. Fan across that line and
    /// the middle point's window is swallowed by its neighbour; walk through it and the region
    /// tiles exactly once.
    #[test]
    fn the_pike_step_line_runs_straight_through_the_tub_corner() {
        let hull = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("blueprint").hull;
        let pike = pike_frame(&hull);

        for point in [pike.fold, pike.tub_step_corner, pike.step_corner] {
            assert!(
                (point.y - hull.sponson_y).abs() < 1.0e-6,
                "the step line lives at the sponson step: {point:?}"
            );
        }

        // Cross product of the two in-plan spans: zero area means one line.
        let a = pike.tub_step_corner - pike.fold;
        let b = pike.step_corner - pike.fold;
        let area2 = (a.x * b.z - a.z * b.x).abs();
        assert!(
            area2 < 1.0e-5,
            "fold {:?}, tub step {:?} and step corner {:?} must be collinear (2*area {area2}) — \
             if a shape change ever breaks this, the underside fan must be re-walked, not just \
             re-wound",
            pike.fold,
            pike.tub_step_corner,
            pike.step_corner
        );

        // And the walk is monotone outboard, so fanning it cannot double back on itself.
        assert!(
            pike.fold.x < pike.tub_step_corner.x && pike.tub_step_corner.x < pike.step_corner.x,
            "the boundary walk must step outboard through the tub corner"
        );
    }
}
