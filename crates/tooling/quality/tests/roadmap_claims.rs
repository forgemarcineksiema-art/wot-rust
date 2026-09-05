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
    Claim {
        // The dossier quoted "capped at 22,000" for a month against 29,000 in `t54.rs` (K7).
        doc: "docs/vehicles/t-54.md",
        anchor: "**LOD0 triangle budget: ",
        source: "crates/vehicle/vehicle_build/src/t54.rs",
        owner: Owner::NumberAfter("pub const MEDIUM_LOD0_TRI_BUDGET: usize = "),
        why: "the benchmark's budget is the number every later vehicle is measured against, and \
              the dossier is where a reader looks it up",
    },
    Claim {
        doc: "docs/vehicles/t-54.md",
        anchor: "**cast-turret budget: ",
        source: "crates/foundation/game_core/src/vehicle_blueprint/t54_hybrid.rs",
        owner: Owner::NumberAfter("budget: "),
        why: "the casting's share of the budget is a construction decision the dossier explains; \
              a stale share misleads the next turret",
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

/// The digits immediately following `prefix`, if any. A space, `_` or `,` BETWEEN two digits is
/// a thousands separator (`29 000` in a doc, `29_000` in Rust), so a number may be written the way
/// its home writes it; the first non-digit that is not such a separator ends the number.
fn number_after(haystack: &str, prefix: &str) -> Option<usize> {
    let start = haystack.find(prefix)? + prefix.len();
    let rest = &haystack.as_bytes()[start..];
    let mut digits = String::new();
    let mut i = 0;
    while i < rest.len() {
        let c = rest[i];
        if c.is_ascii_digit() {
            digits.push(c as char);
        } else if matches!(c, b' ' | b'_' | b',')
            && !digits.is_empty()
            && rest.get(i + 1).is_some_and(u8::is_ascii_digit)
        {
            // a separator inside the number
        } else {
            break;
        }
        i += 1;
    }
    digits.parse().ok()
}

#[test]
fn a_number_is_read_the_way_its_home_writes_it() {
    assert_eq!(number_after("budget: 29 000 (`x`)", "budget: "), Some(29_000));
    assert_eq!(number_after("= 29_000;", "= "), Some(29_000));
    assert_eq!(number_after("v27,565 tris", "v"), Some(27_565));
    assert_eq!(number_after("**Fleet**: 8 blueprint-born", "**Fleet**: "), Some(8));
    assert_eq!(number_after("count: none", "count: "), None);
}

/// How many times `entry` occurs between `open` and the `];` that closes it.
fn count_entries(haystack: &str, open: &str, entry: &str) -> Option<usize> {
    let start = haystack.find(open)? + open.len();
    let body = &haystack[start..];
    let end = body.find("];")?;
    Some(body[..end].matches(entry).count())
}
