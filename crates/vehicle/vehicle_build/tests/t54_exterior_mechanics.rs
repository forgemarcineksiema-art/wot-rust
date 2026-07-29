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

// ---------------------------------------------------------------------------------------------
// PR-26b: the mechanics of the things a crew touches.
// ---------------------------------------------------------------------------------------------

/// K8. Every hatch gets the hardware that makes a lid a lid: the collar it seats on, the hinge it
/// swings about, the handle a crewman pulls. Before this the whole FLEET had none — `grep hinge`
/// over the repository returned nothing, and every hatch was one `revolve::drum` puck.
#[test]
fn every_hatch_carries_a_coaming_a_hinge_and_a_handle() {
    let description = t54_description();
    for hatch in ["cupola_hatch", "driver_hatch", "loader_hatch"] {
        let lid = part_mesh(hatch).bounds().expect("lid bounds");
        let coaming = description
            .parts
            .iter()
            .find(|p| p.key.name == hatch && p.key.instance == 100)
            .unwrap_or_else(|| panic!("{hatch} has a coaming"))
            .mesh()
            .bounds()
            .expect("coaming bounds");
        assert!(
            coaming.min.y <= lid.min.y + 1.0e-3,
            "{hatch}: the collar is UNDER the cover it seats, {:.3} vs {:.3}",
            coaming.min.y,
            lid.min.y
        );
        assert!(
            coaming.max.x - coaming.min.x > lid.max.x - lid.min.x,
            "{hatch}: and reaches wider than it, so there is a step to see"
        );

        let hinge = description
            .parts
            .iter()
            .find(|p| p.key.name == hatch && p.key.instance == 101)
            .unwrap_or_else(|| panic!("{hatch} has a hinge"))
            .mesh()
            .bounds()
            .expect("hinge bounds");
        assert!(
            hinge.max.z < lid.max.z,
            "{hatch}: the hinge is BEHIND the lid — these covers open forward"
        );

        let handle = description
            .parts
            .iter()
            .find(|p| p.key.name == hatch && p.key.instance == 102)
            .unwrap_or_else(|| panic!("{hatch} has a handle"))
            .mesh()
            .bounds()
            .expect("handle bounds");
        // A handle a hand can get behind: proud of the cover's crown, or out past its rim. The
        // commander's lid gets the rim placement — it is the tallest thing on the tank and a bar
        // stacked on top of it puts the vehicle above its own collision box.
        assert!(
            handle.max.y > lid.max.y || handle.max.z > lid.max.z,
            "{hatch}: the handle must stand clear of the cover, where a hand goes"
        );
    }
}

/// K10. A DShK on a T-54 turns on the LOADER'S HATCH RING: the loader stands in his own hatch and
/// swings the gun round it. Ours stood on a pedestal beside the hatch — a gun he cannot reach.
#[test]
fn the_dshk_turns_on_the_loaders_hatch_ring() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let f = &bp.hybrid().expect("hybrid").fittings;
    let ring = part_mesh("dshk_ring").bounds().expect("ring bounds");
    let ring_center_x = (ring.min.x + ring.max.x) * 0.5;
    let ring_center_z = (ring.min.z + ring.max.z) * 0.5;
    assert!(
        (ring_center_x - f.loader_hatch_center.x).abs() < 0.02
            && (ring_center_z - f.loader_hatch_center.z).abs() < 0.02,
        "the ring is concentric with the loader's hatch, not beside it: ({ring_center_x:.3}, \
         {ring_center_z:.3}) vs ({:.3}, {:.3})",
        f.loader_hatch_center.x,
        f.loader_hatch_center.z
    );
    // And the gun rides it.
    let mount = part_mesh("dshk_mount").bounds().expect("mount bounds");
    let mount_x = (mount.min.x + mount.max.x) * 0.5;
    let mount_z = (mount.min.z + mount.max.z) * 0.5;
    let on_ring = ((mount_x - ring_center_x).powi(2) + (mount_z - ring_center_z).powi(2)).sqrt();
    assert!(
        on_ring > 0.10 && on_ring <= (ring.max.x - ring.min.x) * 0.5 + 0.02,
        "the pintle sits ON the ring, not at its centre and not off it: {on_ring:.3}"
    );
}

/// K10. A 12.7 mm gun has a 12.7 mm bore. The DShK barrel inherited `..v.gun`, which handed it the
/// D-10T's 100 mm — a bore WIDER THAN ITS OWN TUBE, so the muzzle turned itself inside out.
#[test]
fn the_dshk_has_its_own_calibre() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let tank_gun = &bp.hybrid().expect("hybrid").gun;
    let barrel = part_mesh("dshk_barrel").bounds().expect("barrel bounds");
    let radius = (barrel.max.x - barrel.min.x) * 0.5;
    assert!(
        radius < tank_gun.bore_radius,
        "the AA gun's tube is slimmer than the tank gun's BORE: {radius:.4} vs {:.4}",
        tank_gun.bore_radius
    );
}

/// K10, without the recorded compromise. The first ring mount laid the gun nearly flat so the
/// vehicle stayed inside its collision box; a DShK is fired by a loader STANDING in his hatch,
/// and the mount is a pintle, a cradle on a trunnion pin, an elevation arc, spade grips and a
/// sight. The protrusion above the hitbox apex is catalogued and enforced in `hitbox_fit`.
#[test]
fn the_dshk_stands_at_its_fighting_height_with_its_controls() {
    let description = t54_description();
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let roof = bp.turret.roof_y;

    for piece in [
        "dshk_mount",
        "dshk_cradle",
        "dshk_trunnion_pin",
        "dshk_elevation_arc",
        "dshk_receiver",
        "dshk_ammo_box",
        "dshk_grip",
        "dshk_sight",
        "dshk_barrel",
    ] {
        assert!(
            description.parts.iter().any(|p| p.key.name == piece),
            "the mount carries its {piece}"
        );
    }

    let receiver = part_mesh("dshk_receiver").bounds().expect("receiver");
    let axis_y = (receiver.min.y + receiver.max.y) * 0.5;
    assert!(
        axis_y > roof + 0.20,
        "the receiver rides a real pintle, not the roof plate: axis {axis_y:.3} vs roof {roof:.2}"
    );

    let barrel = part_mesh("dshk_barrel").bounds().expect("barrel");
    let barrel_len = barrel.max.z - barrel.min.z;
    assert!(
        (barrel_len - 1.07).abs() <= 0.03,
        "the DShK barrel is a documented 1070 mm, got {barrel_len:.3}"
    );
    let grips = part_mesh("dshk_grip").bounds().expect("grips");
    let overall = barrel.max.z - grips.min.z;
    assert!(
        (overall - 1.625).abs() <= 0.08,
        "grips to muzzle is the documented 1625 mm envelope, got {overall:.3}"
    );
    assert!(
        grips.min.z < receiver.min.z,
        "the spade grips hang behind the receiver, where the gunner's hands go"
    );

    let sight = part_mesh("dshk_sight").bounds().expect("sight");
    assert!(sight.max.y > receiver.max.y, "the AA sight stands above the receiver");

    let arc = part_mesh("dshk_elevation_arc").bounds().expect("arc");
    assert!(
        arc.max.y - arc.min.y > 0.12,
        "the elevation arc sweeps a real quadrant, got {:.3}",
        arc.max.y - arc.min.y
    );
}

/// K9. A tow hook is a hook: a throat a shackle drops into and a catch across its mouth. These
/// were 240 x 220 x 200 bricks — a boss with nowhere for a shackle to go.
#[test]
fn the_tow_hooks_have_a_throat_and_a_catch() {
    let description = t54_description();
    for index in 0..2u16 {
        let has = |name: &str| {
            description.parts.iter().any(|p| p.key.name == name && p.key.instance == index)
        };
        assert!(has("tow_hook"), "hook {index} has its bracket");
        assert!(has("tow_hook_throat"), "hook {index} has a throat to take a shackle");
        assert!(has("tow_hook_catch"), "hook {index} has a catch across the mouth");
    }
    // The throat opens FORWARD, so the catch stands ahead of the bracket it closes.
    let bracket = part_mesh("tow_hook").bounds().expect("bracket");
    let catch = part_mesh("tow_hook_catch").bounds().expect("catch");
    assert!(catch.max.z > bracket.max.z, "the catch closes the mouth, which faces the bow");
}

/// K9. A stowed cable is spliced round a thimble at each end and clamped to the plate along its
/// run. Ours were bare tubes floating on a standoff.
#[test]
fn the_tow_cables_are_thimbled_and_clamped() {
    let description = t54_description();
    for index in 0..2u16 {
        let hardware = description
            .parts
            .iter()
            .find(|p| p.key.name == "tow_cable_hardware" && p.key.instance == index)
            .unwrap_or_else(|| panic!("cable {index} carries its hardware"))
            .mesh();
        assert!(hardware.triangle_count() > 100, "thimbles and clamps are real geometry");
    }
}

/// K9. The log is strapped to the tank by something.
#[test]
fn the_unditching_beam_is_banded_to_its_brackets() {
    let bands = part_mesh("unditching_beam_bands").bounds().expect("band bounds");
    let beam = part_mesh("unditching_beam").bounds().expect("beam bounds");
    assert!(
        bands.min.x > beam.min.x && bands.max.x < beam.max.x,
        "the bands sit along the log, inboard of its ends"
    );
    assert!(
        bands.max.y > beam.max.y - 0.02,
        "and wrap it rather than lying under it: {:.3} vs {:.3}",
        bands.max.y,
        beam.max.y
    );
}
