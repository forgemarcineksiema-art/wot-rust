//! Provenance gate: an anchor that cites an in-repo dossier must cite a document that actually
//! CONTAINS the number. The T-54 pack once cited `docs/vehicles/t-54.md` for five dimensions
//! while that file carried none of them — a citation that verifies nothing is how fit-to-model
//! numbers survive. This test makes every `docs/` citation executable.

use std::path::PathBuf;

use game_core::VehicleKind;
use vehicle_forge::ReferencePack;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

/// Human-formatted spellings of a target value that count as "the document carries the
/// number": three/two/one/zero decimals, trailing zeros trimmed. "6.235" matches "6.235",
/// "0.81" matches inside "0.810", "90" matches a count.
fn number_candidates(value: f32) -> Vec<String> {
    let mut out = Vec::new();
    for formatted in
        [format!("{value:.3}"), format!("{value:.2}"), format!("{value:.1}"), format!("{value:.0}")]
    {
        let trimmed = if formatted.contains('.') {
            formatted.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            formatted
        };
        if !out.contains(&trimmed) {
            out.push(trimmed);
        }
    }
    out
}

#[test]
fn every_docs_cited_anchor_number_exists_in_the_cited_document() {
    let root = repo_root();
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(pack) = ReferencePack::for_vehicle(kind) else { continue };
        for target in pack.dimensions() {
            let url = target.source().url();
            if !url.starts_with("docs/") {
                continue;
            }
            let path = root.join(url);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("{kind:?}: cited dossier {url} is unreadable: {error}")
            });
            let candidates = number_candidates(target.target_m());
            assert!(
                candidates.iter().any(|candidate| text.contains(candidate.as_str())),
                "{kind:?}: {} cites {url} for {:.3}, but none of {candidates:?} appears in that \
                 document — the citation verifies nothing",
                target.kind().label(),
                target.target_m(),
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 10,
        "the provenance gate must actually check the fleet's dossier citations (checked {checked})"
    );
}
