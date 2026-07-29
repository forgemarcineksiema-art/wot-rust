//! Fitting mechanics: the small pieces of hardware that make a lid a lid.
//!
//! A hatch is not a disc. It is an opening with a raised collar round it, a pressed cover that
//! sits ON that collar, a hinge the cover swings about and a handle a crewman pulls. Before this
//! module the fleet had none of those: `grep hinge` over the whole repository returned nothing,
//! and every hatch, cupola and lamp on every vehicle was one `revolve::drum` puck.
//!
//! These are generic. A hinge is a hinge on a T-54's loader hatch and on a Tiger's engine deck.

use glam::Vec3;
use revolve::{merge, revolve, translate};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// A hinge: `knuckles` barrels threaded on one pin, with a leaf plate behind them.
///
/// `center` is the middle of the pin, `axis` the pin's direction, and `reach` how far the leaf
/// extends from the pin (toward the lid it carries). The leaf is what welds to the cover, the
/// barrels are what the pin runs through, and the alternation between them is the only reason a
/// hinge reads as a hinge and not as a bar.
pub fn hinge(
    center: Vec3,
    axis: Vec3,
    span: f32,
    barrel_radius: f32,
    reach: f32,
    knuckles: usize,
) -> GeometryMesh {
    let axis = axis.normalize_or_zero();
    let axis = if axis == Vec3::ZERO { Vec3::X } else { axis };
    let knuckles = knuckles.max(2);
    let pitch = span / knuckles as f32;
    // The leaf lies FLAT on the plate it welds to: perpendicular to the pin and perpendicular to
    // up, which is the direction a hinge leaf runs on a roof or a deck.
    let leaf_dir = if axis.cross(Vec3::Y).length() > 1.0e-3 {
        axis.cross(Vec3::Y).normalize()
    } else {
        Vec3::Z
    };

    let mut pieces = Vec::with_capacity(knuckles + 1);
    for k in 0..knuckles {
        let t = (k as f32 + 0.5) / knuckles as f32 - 0.5;
        let half = pitch * 0.38;
        let profile =
            [(-half, 0.0_f32), (-half, barrel_radius), (half, barrel_radius), (half, 0.0)];
        pieces.push(translate(
            &revolve(axis, &profile, 8, MaterialRole::BarrelSteel, SmoothingGroup(3)),
            center + axis * (t * span),
        ));
    }
    // The leaf: a thin plate from the pin out to where it welds.
    pieces.push(leaf(
        center + leaf_dir * (reach * 0.5),
        axis * (span * 0.42),
        leaf_dir * (reach * 0.5),
        barrel_radius * 0.45,
    ));
    merge(&pieces).weld_and_smooth()
}

/// A grab handle: a bar standing off a surface on two welded feet.
///
/// The feet are the point. A bar floating on a standoff is a bar floating; the feet are what say
/// it is attached, and they are what a viewer's eye reads at any distance where the bar itself is
/// two pixels wide.
pub fn grab_handle(a: Vec3, b: Vec3, normal: Vec3, standoff: f32) -> GeometryMesh {
    let n = normal.normalize_or_zero();
    let n = if n == Vec3::ZERO { Vec3::Y } else { n };
    let (top_a, top_b) = (a + n * standoff, b + n * standoff);
    let bar = crate::weld::handle_rail(&[top_a, top_b], standoff * 0.30);
    let foot_radius = standoff * 0.34;
    let feet: Vec<GeometryMesh> = [(a, top_a), (b, top_b)]
        .into_iter()
        .map(|(base, top)| crate::weld::handle_rail(&[base - n * 0.004, top], foot_radius))
        .collect();
    merge(&[vec![bar], feet].concat()).weld_and_smooth()
}

/// A hatch coaming: the raised ring collar welded round an opening, which the cover seats on.
///
/// Distinct from the cover for a reason — on a real vehicle they are two pieces of steel, and the
/// step between them is what makes a closed hatch read as closed rather than as a disc painted on
/// the roof.
/// `segments` too: a hatch collar a player leans over wants 20; a cable thimble or a canister
/// strap reads round at 12 and nobody ever sees it closer than a metre. The triangle budget is a
/// gameplay promise (one look, MX330 at 60), so resolution is spent where the eye goes.
///
/// `material` is the CALLER's, not this function's. A collar round a hatch is rolled armour; a
/// thimble spliced into a rope and a band strapping a log are not — and a generator that hard-codes
/// one material hands every caller the same steel. That mattered: with `RolledArmor` baked in, the
/// rope thimbles and the log bands counted as hull PLATES, and the instrument that measures hull
/// length measures plates. It reported the T-54 at 6.414 m against a Locked 6.235.
pub fn coaming(
    center: Vec3,
    axis: Vec3,
    radius: f32,
    height: f32,
    wall: f32,
    material: MaterialRole,
    segments: usize,
) -> GeometryMesh {
    let axis = axis.normalize_or_zero();
    let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
    let inner = (radius - wall).max(radius * 0.4);
    // Closed at both ends: an annulus swept up, then back down its inside face.
    let profile =
        [(0.0_f32, inner), (height, inner), (height, radius), (0.0, radius), (0.0, inner)];
    translate(&revolve(axis, &profile, segments.max(8), material, SmoothingGroup(3)), center)
}

/// A thin rectangular plate: centre plus three half-axes. Used for hinge leaves and clamp straps.
fn leaf(centre: Vec3, du: Vec3, dv: Vec3, thickness: f32) -> GeometryMesh {
    let dw = du.cross(dv).normalize_or_zero() * thickness;
    crate::scatter::plate_box(centre, du, dv, dw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hinge_is_barrels_on_a_pin_with_a_leaf_behind_them() {
        let mesh = hinge(Vec3::ZERO, Vec3::X, 0.30, 0.014, 0.09, 3);
        let b = mesh.bounds().expect("hinge bounds");
        assert!((b.max.x - b.min.x) > 0.20, "the knuckles spread along the pin");
        assert!(b.max.z - b.min.z > 0.05, "and the leaf reaches out from it");
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean hinge");
    }

    #[test]
    fn a_grab_handle_stands_on_feet_that_reach_its_surface() {
        let mesh = grab_handle(Vec3::new(-0.1, 0.0, 0.0), Vec3::new(0.1, 0.0, 0.0), Vec3::Y, 0.05);
        let b = mesh.bounds().expect("handle bounds");
        assert!(b.min.y <= 0.0, "the feet reach the plate they are welded to: {:.4}", b.min.y);
        assert!(b.max.y >= 0.045, "and the bar stands off it");
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean handle");
    }

    #[test]
    fn a_coaming_is_a_ring_not_a_disc() {
        let mesh = coaming(Vec3::ZERO, Vec3::Y, 0.20, 0.04, 0.03, MaterialRole::RolledArmor, 20);
        let b = mesh.bounds().expect("coaming bounds");
        assert!((b.max.y - b.min.y - 0.04).abs() < 1.0e-3, "it stands its own height");
        // Nothing on the axis: a collar has a hole in it, which is what an opening is.
        let on_axis =
            mesh.vertices().iter().filter(|v| v.position.x.hypot(v.position.z) < 0.15).count();
        assert_eq!(on_axis, 0, "the collar is open in the middle");
    }
}
