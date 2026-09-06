//! K14: THE FENDER STOWAGE IS HELD DOWN. Every fuel tank and stowage bin on the T-54's fenders is
//! strapped over the top from shelf to shelf and bracketed to the shelf under its outboard edge —
//! parts under the box's own key, so the inventory reads them as the box's.

use vehicle_build::t54_description;
use vehicle_geometry::MeshBounds;

fn bounds(part: &vehicle_build::VehiclePart) -> MeshBounds {
    part.mesh().bounds().expect("a part has bounds")
}

#[test]
fn every_fender_box_is_strapped_and_bracketed_to_the_shelf() {
    let description = t54_description();
    let boxes: Vec<_> = description
        .parts
        .iter()
        .filter(|p| {
            (p.key.name == "fuel_tank" || p.key.name == "stowage_bin") && p.key.instance < 8
        })
        .collect();
    assert_eq!(boxes.len(), 8, "two tanks and six bins on the shelves");
    for bx in &boxes {
        let b = bounds(bx);
        let side = b.min.x.signum();
        let hardware: Vec<_> = description
            .parts
            .iter()
            .filter(|p| {
                p.key.name == bx.key.name
                    && p.key.instance >= 128
                    && (p.key.instance - bx.key.instance) % 8 == 0
            })
            .collect();
        let straps: Vec<_> = hardware.iter().filter(|p| p.key.instance < 192).collect();
        let brackets: Vec<_> = hardware.iter().filter(|p| p.key.instance >= 192).collect();
        // The flat fuel tanks are strapped; the bins are bolted through their flanges.
        let expected_straps = if bx.key.name == "fuel_tank" { 8 } else { 0 };
        assert_eq!(
            straps.len(),
            expected_straps,
            "{:?}: two straps on a tank, none on a bin",
            bx.key
        );
        assert_eq!(brackets.len(), 4, "{:?}: two angle brackets of two legs", bx.key);
        if expected_straps > 0 {
            // The straps reach over the top and down both faces to the shelf.
            let top_run = straps.iter().map(|p| bounds(p).max.y).fold(f32::MIN, f32::max);
            let low = straps.iter().map(|p| bounds(p).min.y).fold(f32::MAX, f32::min);
            assert!(top_run > b.max.y, "{:?}: a strap runs over the lid", bx.key);
            assert!(low < b.min.y + 0.01, "{:?}: the strap legs reach the shelf", bx.key);
            let hugs_inboard = straps.iter().any(|p| bounds(p).min.x < b.min.x - 0.001);
            let hugs_outboard = straps.iter().any(|p| bounds(p).max.x > b.max.x + 0.001);
            assert!(
                hugs_inboard && hugs_outboard,
                "{:?}: the straps hug both faces of the box",
                bx.key
            );
        }
        // The brackets sit on the shelf under the outboard edge.
        for br in &brackets {
            let bb = bounds(br);
            assert!(bb.min.y < b.min.y + 0.005, "{:?}: a bracket foot on the shelf", bx.key);
            let outboard_face = if side > 0.0 { b.max.x } else { b.min.x };
            let bracket_x = if side > 0.0 { bb.min.x } else { bb.max.x };
            assert!(
                (bracket_x - outboard_face) * side > -0.002,
                "{:?}: a bracket outboard of the box's face",
                bx.key
            );
        }
    }
}
