//! K13: THE FASTENER RULE, fleet-wide. A bolt, nut, rivet or stud fastens two things and shows a
//! face outside both: its bounds are not contained in any other part's bounds. The rule was
//! written for the sprocket's forty ring bolts, which sat entirely inside the ring's metal (K9,
//! now proud by construction and locked in the kernel); this is the same sentence for every
//! fastener part the library builds, on every vehicle.

use game_core::VehicleKind;
use vehicle_forge::authoritative_description;
use vehicle_geometry::MeshBounds;

fn is_fastener(key: &str) -> bool {
    ["bolt", "nut", "rivet", "stud"].iter().any(|word| key.contains(word))
}

fn contained(inner: &MeshBounds, outer: &MeshBounds, tolerance: f32) -> bool {
    (0..3).all(|axis| {
        inner.min[axis] >= outer.min[axis] - tolerance
            && inner.max[axis] <= outer.max[axis] + tolerance
    })
}

#[test]
fn every_fastener_shows_a_face_outside_what_it_fastens() {
    let mut fasteners = 0;
    let mut faults = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let description = authoritative_description(kind).expect("describes");
        let parts: Vec<(&str, MeshBounds)> = description
            .parts
            .iter()
            .filter_map(|part| part.mesh().bounds().map(|b| (part.key.name, b)))
            .collect();
        for (key, bounds) in parts.iter().filter(|(key, _)| is_fastener(key)) {
            fasteners += 1;
            for (other, outer) in parts.iter().filter(|(other, _)| other != key) {
                if contained(bounds, outer, 1.0e-3) {
                    faults.push(format!(
                        "{kind:?}: `{key}` lies entirely inside `{other}` — a fastener sunk into \
                         what it fastens is a texture nobody drew"
                    ));
                }
            }
            println!("FASTENER {kind:?} {key}: {:?}", bounds);
        }
    }
    assert!(fasteners >= 2, "the library builds fastener parts to walk: {fasteners}");
    assert!(faults.is_empty(), "fastener faults:\n{}", faults.join("\n"));
}
