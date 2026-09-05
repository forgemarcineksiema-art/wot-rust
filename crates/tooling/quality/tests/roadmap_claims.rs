//! Architecture gate: the numbers the roadmap quotes are the numbers the code owns.
//!
//! `docs/ROADMAP.md` said the wire was v43 while `net` shipped v45. Two protocol bumps had
//! landed without the one document a reader starts from noticing, and nothing broke — which is
//! the whole problem. A document that describes DATA rots silently, and the next reader is the
//! one who writes a store page, a devlog or a plan on top of a number that stopped being true.
//!
//! The fix is not to correct the number; it is to make the correction permanent. Each countable
//! claim carries a short anchor, and this gate reads both sides of it. Prose stays prose — only
//! the numbers a stranger would quote back at us are pinned.

use std::fs;

use quality::workspace::workspace_root;

/// Where the authoritative number lives inside its owning file.
enum Owner {
    /// The digits that follow a literal, e.g. `pub const PROTOCOL_VERSION: u16 = `.
    NumberAfter(&'static str),
    /// The entries of a slice literal: count `entry` between `open` and the closing `];`.
    EntriesIn { open: &'static str, entry: &'static str },
}

struct Claim {
    /// The document making the claim, repo-relative.
    doc: &'static str,
    /// The literal in the doc immediately before the number it claims.
    anchor: &'static str,
    /// The file that owns the fact, repo-relative.
    source: &'static str,
    owner: Owner,
    /// What a reader loses when the two disagree.
    why: &'static str,
}

const CLAIMS: &[Claim] = &[
    Claim {
        doc: "docs/ROADMAP.md",
        anchor: "**wire v",
        source: "crates/runtime/net/src/lib.rs",
        owner: Owner::NumberAfter("pub const PROTOCOL_VERSION: u16 = "),
        why: "the protocol version is the first thing a networking reader trusts, and a stale \
              one sends them looking for a wire that no longer exists",
    },
    Claim {
        // The same fact claimed in a second doc rotted five versions deep (43 against 48)
        // before anyone noticed — the exact failure the header describes, in the one doc
        // that TEACHES the protocol-bump procedure.
        doc: "docs/testing-and-regression.md",
        anchor: "`PROTOCOL_VERSION = ",
        source: "crates/runtime/net/src/lib.rs",
        owner: Owner::NumberAfter("pub const PROTOCOL_VERSION: u16 = "),
        why: "the doc that walks a reader through a protocol bump must quote the wire that \
              exists, or the walkthrough calibrates them against a ghost",
    },
    Claim {
        doc: "docs/ROADMAP.md",
        anchor: "**Fleet**: ",
        source: "crates/foundation/game_core/src/vehicle_kind.rs",
        owner: Owner::NumberAfter("pub const PLAYABLE: [VehicleKind; "),
        why: "the roster count is a store-page number; it must come from the roster, not from \
              memory of when it was last counted",
    },
    Claim {
        doc: "docs/ROADMAP.md",
        anchor: "**Maps**: ",
        source: "crates/foundation/terrain/src/map_id.rs",
        owner: Owner::EntriesIn {
            open: "pub const SHIPPED: &'static [MapId] = &[",
            entry: "MapId::",
        },
        why: "the shipped-map count is the other store-page number, and `SHIPPED` is already \
              the catalog rotation, goldens and baked assets follow",
    },
    Claim {
        // "Two of six species are placed" outlived two retirements and route 2 by three
        // days (2026-09-05): the species roster is the one flora number a reader repeats.
        doc: "docs/ROADMAP.md",
        anchor: "**Living species: ",
        source: "crates/world/scene_build/src/tree_lod.rs",
        owner: Owner::NumberAfter("pub const LADDER_SPECIES: [TreeSpecies; "),
        why: "the living-species count follows the owner's retirements (willow, pine) and \
              the ladder is the only place that knows which species are still planted",
    },
];

#[test]
fn the_docs_quote_the_numbers_the_code_owns() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for claim in CLAIMS {
        let doc = fs::read_to_string(root.join(claim.doc))
            .unwrap_or_else(|_| panic!("{} should be readable", claim.doc));
        let source = fs::read_to_string(root.join(claim.source))
            .unwrap_or_else(|_| panic!("{} should be readable", claim.source));

        let Some(claimed) = number_after(&doc, claim.anchor) else {
            offenders.push(format!(
                "{}: no number follows `{}` — the anchor this gate reads is gone, so the \
                 claim it protected is unchecked again",
                claim.doc, claim.anchor,
            ));
            continue;
        };

        let owned = match &claim.owner {
            Owner::NumberAfter(prefix) => number_after(&source, prefix),
            Owner::EntriesIn { open, entry } => count_entries(&source, open, entry),
        };
        let Some(owned) = owned else {
            offenders.push(format!(
                "{}: the number this gate reads has moved — update the owner rule for `{}`",
                claim.source, claim.anchor,
            ));
            continue;
        };

        if claimed != owned {
            offenders.push(format!(
                "{} claims `{}{claimed}` but {} owns {owned} — {}",
                claim.doc, claim.anchor, claim.source, claim.why,
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "a gated doc is quoting numbers the code has moved past:\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn every_anchor_appears_once_so_no_second_copy_can_drift() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for claim in CLAIMS {
        let doc = fs::read_to_string(root.join(claim.doc))
            .unwrap_or_else(|_| panic!("{} should be readable", claim.doc));
        let hits = doc.matches(claim.anchor).count();
        if hits != 1 {
            offenders.push(format!(
                "`{}` appears {hits} times in {} — a checked claim needs exactly one home, \
                 or the copies disagree and the gate still passes",
                claim.anchor, claim.doc,
            ));
        }
    }

    assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
}

/// The digits immediately following `prefix`, if any.
fn number_after(haystack: &str, prefix: &str) -> Option<usize> {
    let start = haystack.find(prefix)? + prefix.len();
    let digits: String = haystack[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// How many times `entry` occurs between `open` and the `];` that closes it.
fn count_entries(haystack: &str, open: &str, entry: &str) -> Option<usize> {
    let start = haystack.find(open)? + open.len();
    let body = &haystack[start..];
    let end = body.find("];")?;
    Some(body[..end].matches(entry).count())
}
