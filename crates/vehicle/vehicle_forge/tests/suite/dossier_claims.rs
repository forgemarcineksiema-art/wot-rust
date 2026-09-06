//! The dossier quotes the bake it describes.
//!
//! `docs/vehicles/t-54.md` said "capped at 22,000 triangles" for a month while `vehicle_build`
//! held the cap at 29,000 and the bake measured 27,565 (Inny Poziom K7). The two BUDGET numbers
//! are pinned by `quality`'s `roadmap_claims`; the MEASURED count cannot be pinned there, because
//! `quality` may not build a vehicle. This crate can — so the dossier's measurement is read here,
//! against the same LOD0 bake `shipped_cost` prints.

use std::fs;
use std::path::PathBuf;

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;

const DOSSIER: &str = "docs/vehicles/t-54.md";
const ANCHOR: &str = "**LOD0 measured: ";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The claim is written bold — `**LOD0 measured: 27 565**` — so the number is whatever digits
/// sit between the anchor and the closing `**`, thousands separators ignored.
fn bold_number_after(doc: &str, anchor: &str) -> Option<usize> {
    let tail = doc.split_once(anchor)?.1;
    let claim = &tail[..tail.find("**")?];
    claim.chars().filter(char::is_ascii_digit).collect::<String>().parse().ok()
}

#[test]
fn the_dossier_quotes_the_triangle_count_the_bake_measures() {
    let path = workspace_root().join(DOSSIER);
    let doc = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{DOSSIER} readable: {e}"));
    assert_eq!(
        doc.matches(ANCHOR).count(),
        1,
        "the measured claim needs exactly one home in {DOSSIER}, or the copies disagree"
    );
    let claimed = bold_number_after(&doc, ANCHOR).expect("a bold number follows the anchor");

    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("the T-54 bakes");
    let measured: usize = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();

    assert_eq!(
        claimed, measured,
        "{DOSSIER} says `{ANCHOR}{claimed}` but the LOD0 bake measures {measured} — write the \
         number the code owns (`cargo test -p vehicle_forge --test shipped_cost -- --nocapture` \
         prints it); the dossier quoted a dead cap for a month once (K7)"
    );
}

#[test]
fn the_bold_claim_is_read_with_its_separators_and_nothing_else() {
    assert_eq!(
        bold_number_after("x **LOD0 measured: 27 565** tris", "**LOD0 measured: "),
        Some(27_565)
    );
    assert_eq!(bold_number_after("x **n: 1,234** y", "**n: "), Some(1_234));
    assert_eq!(bold_number_after("x **n: 12 y", "**n: "), None, "an unclosed claim is no claim");
    assert_eq!(bold_number_after("nothing here", "**n: "), None);
}
