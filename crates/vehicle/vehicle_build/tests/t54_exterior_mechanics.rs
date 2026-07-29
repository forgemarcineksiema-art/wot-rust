//! K8 and K11: the exterior fittings that had a shape but not a mechanism.
//!
//! Three generators in the `detail` kernel — `louvre_slats`, `bolt_head`, `casting_seam` — existed
//! since that kernel was written and had never been called once. What stood in for them was a row
//! of flat boards over a hole, plates with nothing holding them down, and a cast turret with no
//! mould line. And the headlight, the one part on this vehicle whose job is to point somewhere,
//! was built by a helper that revolves about Y — so it lay on its side, shining at the sky.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::{MaterialRole, SubmeshKind};

fn part_mesh(name: &str) -> vehicle_geometry::GeometryMesh {
    t54_description()
        .parts
        .iter()
        .find(|part| part.key.name == name)
        .unwrap_or_else(|| panic!("the vehicle carries a {name}"))
        .mesh()
}

/// K8. A headlight points FORWARD. Its lamp body must be deeper across the beam axis than it is
/// tall, and its lens must be the frontmost thing on it — which is what "the lens faces the road"
/// means when you write it down as geometry.
#[test]
fn the_headlight_faces_forward_and_shows_glass() {
    let body = part_mesh("headlight").bounds().expect("headlight bounds");
    let lens = part_mesh("headlight_lens").bounds().expect("lens bounds");

    assert!(
        lens.min.z >= body.max.z - 0.02,
        "the lens sits in the FACE of the lamp, not inside it: lens {:.3} vs body front {:.3}",
        lens.min.z,
        body.max.z
    );
    // A lamp lying on its side is as wide as it is deep and its flat faces point up. This one is
    // a drum on the Z axis: its round section is in XY and its depth is along Z.
    let width = body.max.x - body.min.x;
    let height = body.max.y - body.min.y;
    assert!(
        (width - height).abs() < 0.02,
        "the drum's round section stands in the XY plane: {width:.3} x {height:.3}"
    );

    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let glass = hull.vertices().iter().filter(|v| v.material == MaterialRole::Glass).count();
    assert!(glass > 0, "the lens is glass, not the steel drum behind it");
}

/// K8. A lamp bolted to nothing floats, and a lens with nothing over it does not survive a wood.
#[test]
fn the_headlight_is_bracketed_and_guarded() {
    let body = part_mesh("headlight").bounds().expect("headlight bounds");
    let bracket = part_mesh("headlight_bracket").bounds().expect("bracket bounds");
    assert!(
        bracket.min.y < body.min.y,
        "the stalk reaches DOWN to the fender it stands on: {:.3} vs {:.3}",
        bracket.min.y,
        body.min.y
    );
    let guard = part_mesh("headlight_guard").bounds().expect("guard bounds");
    assert!(
        guard.max.z >= body.max.z - 0.01,
        "the hoop stands at the glass, where the branches are"
    );
}

/// K11. A louvre that does not lean is a plank. The deck slats were axis-aligned boxes, so the
/// "louvered" grille was a row of boards and the shadow band under them did all the work.
#[test]
fn the_deck_grille_carries_raked_louvres() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let v = bp.hybrid().expect("hybrid");
    let deck_top = v.deck.center.y + v.deck.half.y;
    let solids = solid::t54_deck_grille(&v.detail, deck_top);

    // Every slat plate must present a face whose normal has BOTH a vertical and a fore-aft
    // component — that is what a rake is, and an axis-aligned box has neither pair.
    let raked = solids
        .into_iter()
        .filter(|s| {
            s.clone()
                .into_planes()
                .iter()
                .any(|p| p.normal.y.abs() > 0.15 && p.normal.z.abs() > 0.15)
        })
        .count();
    assert!(
        raked >= v.detail.grille_slats,
        "every louvre leans: {raked} raked plates for {} slats",
        v.detail.grille_slats
    );
}

/// K11. Bolted panels need bolts, and a cast turret has the line where its mould parted.
#[test]
fn the_deck_is_bolted_and_the_turret_carries_its_mould_line() {
    let bolts = part_mesh("engine_deck_bolts").bounds().expect("bolt bounds");
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let v = bp.hybrid().expect("hybrid");
    let deck_top = v.deck.center.y + v.deck.half.y;
    assert!(
        bolts.min.y >= deck_top - 0.005 && bolts.max.y <= deck_top + 0.03,
        "the bolt heads sit ON the deck: {:.3}..{:.3} against a deck at {deck_top:.3}",
        bolts.min.y,
        bolts.max.y
    );
    assert!(bolts.max.x - bolts.min.x > 0.8, "in two rows, one down each seam");

    let seam = part_mesh("turret_casting_seam").bounds().expect("seam bounds");
    assert!(
        seam.max.x - seam.min.x > 1.5 && seam.max.z - seam.min.z > 1.5,
        "the mould line runs right round the casting: {:.2} x {:.2}",
        seam.max.x - seam.min.x,
        seam.max.z - seam.min.z
    );
    assert!(seam.max.y - seam.min.y < 0.30, "at one height, as a parting line is");
}
