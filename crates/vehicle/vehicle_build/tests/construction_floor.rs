//! THE CONSTRUCTION FLOOR: a part must be built like the thing it is named after.
//!
//! Every other budget in this workspace is a ceiling derived from what a thing already costs, so
//! it can stop a regression and can never ask for more. This asks. It is the third floor after
//! the material law and `material_floor.rs`, and the one the 2026-08-08 geometry audit
//! (`docs/geometry-pipeline-audit-2026-08-08.md`) named as carrying the most weight.
//!
//! **The verdict is per SEMANTICS, never by threshold — and getting that wrong first is how this
//! file learned it.** `upper_hull` presents six planes and that is CORRECT: a rolled hull *is* six
//! plates carrying their armour angles, and the `solid` kernel building it exactly is the whole
//! point of the kernel-selection rule. The same six planes on an engine grille is a different
//! verdict entirely. So there is no blanket number here; there is a classification, and every
//! exterior part that presents a box must appear in one of the two lists below with its reason.
//!
//! The debt list is a countdown, in the pattern `crate_hygiene`'s orphan allowlist established:
//! entries may be REMOVED as the parts get built, never added to get green. Two tests keep it
//! honest — one fails if a part is unclassified, the other fails if a debt entry no longer fails,
//! so the list can neither rot nor outlive its cause.

use std::collections::BTreeMap;

use glam::Vec3;

/// A box presents six planes; a chamfered box presents about fourteen. Under ten planes a part is
/// a box, at most with a corner knocked off it.
const BOX_PLANES: usize = 10;

mod common;
/// Roles that are only ever seen through a breach. The floor is about what the vehicle presents
/// to the world, so these are out of scope. Shared with `t54_turret_containment`, which judges
/// exactly the parts this test excuses — one predicate, so a role cannot fall between them.
use common::is_interior;

/// Exterior parts for which a box is the RIGHT answer, and why. Rolled armour really is flat
/// plate; bar stock really is a prism. Building either as anything else would be the mistake.
const PLATES_AND_BARS: &[(&str, &str)] = &[
    ("upper_hull", "rolled plate assembly — six planes carrying the blueprint's armour angles"),
    ("lower_tub", "the narrow lower hull, same: exact plate, exact slope, sharp seam"),
    ("engine_deck_panel", "a hinged deck plate is a plate"),
    ("transmission_cover", "a flat bolted access cover"),
    ("deck_weld_bead", "a weld bead is a raised bar, not a body"),
    ("hull_plate_seam", "the same, along the plate joint"),
    ("fender_bracket", "an L-bracket is bar stock cut and bent"),
    (
        "fender_flap_rib",
        "a pressed stiffening bead on the tail flap is a raised bar riding the slope face — the \
         press leaves a straight-sided ridge, and three of them are the reference rear view's \
         read of the flap",
    ),
    (
        "turret_aerial",
        "a whip aerial IS a tapered rod — 28 mm at the ferrule down to 8 mm at the tip, which is \
         one to two pixels at any range this vehicle is looked at. Eight sides is already more \
         than the roundness law asks of that radius (five); rounder would be triangles spent \
         below the pixel.",
    ),
];

/// Exterior parts that present a box and SHOULD NOT — the audit's measured findings, executable.
///
/// Each entry says what the part has to become. Remove an entry when the part is built; never add
/// one to make this test pass.
const DEBT: &[(&str, &str)] = &[
    // EMPTY, and it got here honestly rather than by never being filled.
    //
    // It opened with three entries. Two were real and are burned: `driver_periscope` and
    // `turret_periscope` are now a housing, a GLASS prism and two armoured cheeks instead of a box
    // with one corner cut.
    //
    // The third, `deck_grille`, was WITHDRAWN — my error, and worth recording because of how it
    // was made. I read it off an island inventory (eleven islands, 12 triangles and 6 planes each)
    // and called it eleven flat boxes without reading what the islands were. `solid::t54_deck_grille`
    // builds a 0.12 m shadowed well, four frame rails and louvres raked 0.62 rad — and carries a
    // comment recording that axis-aligned slats were the OLD defect, already fixed. A rail is a box
    // and that is correct; the assembly is a grille. Measuring a proxy instead of reading the
    // construction is the same mistake three times in one audit.
];
struct PartFacts {
    planes: Vec<Vec3>,
    interior: bool,
}

/// Distinct face planes a part presents, and whether it is interior.
///
/// Parts sharing a key are ONE THING and are counted as their union, not as their weakest piece.
/// A periscope is a housing, a prism and two guards; judging it by the prism — a flat pane, which
/// really is six planes and correctly so — would call a finished device debt. Identical repeated
/// boxes are unaffected, because eleven axis-aligned boxes still present the same six normals
/// between them, which is exactly why the grille still fails.
fn part_facts() -> BTreeMap<String, PartFacts> {
    let description = vehicle_build::t54_description();
    let mut facts: BTreeMap<String, PartFacts> = BTreeMap::new();
    for part in &description.parts {
        let mesh = part.mesh();
        let mut distinct: Vec<Vec3> = Vec::new();
        for triangle in mesh.indices().chunks_exact(3) {
            let vertices = mesh.vertices();
            let (a, b, c) = (
                vertices[triangle[0] as usize].position,
                vertices[triangle[1] as usize].position,
                vertices[triangle[2] as usize].position,
            );
            let normal = (b - a).cross(c - a).normalize_or_zero();
            if normal.length_squared() > 0.5
                && !distinct.iter().any(|seen| seen.dot(normal) > 0.995)
            {
                distinct.push(normal);
            }
        }
        let entry = facts
            .entry(part.key.name.to_string())
            .or_insert(PartFacts { planes: Vec::new(), interior: is_interior(part.material) });
        for normal in distinct {
            if !entry.planes.iter().any(|seen: &Vec3| seen.dot(normal) > 0.995) {
                entry.planes.push(normal);
            }
        }
        entry.interior &= is_interior(part.material);
    }
    facts
}

fn listed_in(list: &[(&str, &str)], name: &str) -> bool {
    list.iter().any(|(entry, _)| *entry == name)
}

/// The floor itself: an exterior part that presents a box is either declared to BE a plate or a
/// bar, or it is debt. There is no third state, and no part may sit outside both lists.
#[test]
fn every_exterior_part_that_presents_a_box_is_classified() {
    let mut unclassified: Vec<String> = Vec::new();
    let mut checked = 0;
    for (name, facts) in part_facts() {
        if facts.interior {
            continue;
        }
        checked += 1;
        if facts.planes.len() >= BOX_PLANES {
            continue;
        }
        if !listed_in(PLATES_AND_BARS, &name) && !listed_in(DEBT, &name) {
            unclassified.push(format!("`{name}` presents {} planes", facts.planes.len()));
        }
    }
    // The floor the ratchet asks for: a walk that classified nothing would otherwise pass.
    assert!(checked > 20, "only {checked} exterior parts walked — the description is not loading");
    assert!(
        unclassified.is_empty(),
        "these exterior parts are built as boxes and say nothing about why:\n  {}\n\nEither add \
         them to PLATES_AND_BARS with the reason a box is correct, or to DEBT with what they must \
         become. A part that is a box by accident is the thing this file exists to catch.",
        unclassified.join("\n  ")
    );
}

/// The debt list may only shrink. An entry that no longer fails is a part that got built, and
/// leaving it listed would quietly lower the bar for the next one.
#[test]
fn no_debt_entry_outlives_the_defect_it_names() {
    let facts = part_facts();
    let mut stale: Vec<&str> = Vec::new();
    for (name, _) in DEBT {
        let Some(part) = facts.get(*name) else {
            stale.push(name);
            continue;
        };
        if part.planes.len() >= BOX_PLANES {
            stale.push(name);
        }
    }
    assert!(stale.is_empty(), "these parts are no longer boxes — take them out of DEBT: {stale:?}");
}

/// A part cannot be declared BOTH a legitimate plate and outstanding debt.
#[test]
fn the_two_lists_do_not_overlap() {
    for (name, _) in PLATES_AND_BARS {
        assert!(
            !listed_in(DEBT, name),
            "`{name}` is claimed as a correct plate and as debt at the same time"
        );
    }
    assert!(!DEBT.is_empty() || !PLATES_AND_BARS.is_empty(), "the floor classifies nothing");
}
