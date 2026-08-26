//! The enums that ARE the wire and the assets, and the constant that has to keep listing all of
//! them.
//!
//! CLAUDE.md calls these append-only: the variant order is a stored identity, so `MapId`,
//! `SceneryKind`, `VehicleKind` and their siblings can gain a variant but never lose or reorder
//! one. Every coverage test in this repo — armour zones, shader material ids, map goldens —
//! iterates an `ALL` constant to ask "is each one handled?". That question is only as honest as
//! `ALL` is complete, and nothing was checking that.
//!
//! `VehicleKind::ALL` was locked by `assert_eq!(VehicleKind::ALL.len(), 8)`. Add a ninth variant
//! and forget the constant, and the length is still 9 and the assertion still passes: the lock
//! held the number, not the roster. This file reads both the enum body and the constant and
//! compares them name by name.

use quality::workspace_root;
use std::path::{Path, PathBuf};

/// Enums whose variant set is an identity — stored in blueprints, on the wire, or in a baked
/// asset — and which therefore must carry a complete `ALL`.
///
/// This list is checked against the sources: an entry naming an enum that no longer exists fails,
/// so the register cannot rot into a list of good intentions.
const IDENTITY_ENUMS: &[&str] = &[
    "ArmorZone",
    "DamageCause",
    "MapId",
    "MaterialRole",
    "ModuleSlot",
    "Nation",
    "Penetrator",
    "RoadSurface",
    "RoundId",
    "SceneryKind",
    "ShellType",
    "StaticCoverKind",
    "VehicleKind",
];

#[test]
fn every_identity_enum_carries_an_all_constant() {
    let sources = rust_sources(&workspace_root());
    let mut missing = Vec::new();

    for name in IDENTITY_ENUMS {
        let Some((path, text)) = sources.iter().find(|(_, text)| declares_enum(text, name)) else {
            missing.push(format!("`{name}` is in IDENTITY_ENUMS but declared nowhere"));
            continue;
        };
        if all_constant(text, name).is_none() {
            missing.push(format!("{}: `{name}` has no `pub const ALL`", path.display()));
        }
    }

    assert!(
        missing.is_empty(),
        "an append-only enum with no ALL is an enum no coverage test can walk:\n  {}",
        missing.join("\n  ")
    );
}

/// The lock the length assertions could not provide.
#[test]
fn every_all_constant_names_every_variant() {
    let sources = rust_sources(&workspace_root());
    let mut offenders = Vec::new();

    for (path, text) in &sources {
        for name in enums_declared_in(text) {
            let Some(listed) = all_constant(text, &name) else { continue };
            let declared = variants_of(text, &name);
            let forgotten: Vec<_> =
                declared.iter().filter(|variant| !listed.contains(*variant)).collect();
            let invented: Vec<_> =
                listed.iter().filter(|variant| !declared.contains(*variant)).collect();

            if !forgotten.is_empty() {
                offenders.push(format!("{}: {name}::ALL is missing {forgotten:?}", path.display()));
            }
            if !invented.is_empty() {
                offenders.push(format!(
                    "{}: {name}::ALL names {invented:?}, which are not variants of it",
                    path.display()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "an ALL that is not all makes every test that walks it quietly partial:\n  {}",
        offenders.join("\n  ")
    );
}

/// `ALL` has to mean the same thing everywhere or the rule above means nothing. `MapId::ALL` used
/// to hold only the SHIPPED maps — `ALL.contains(&Scratch)` was false — while `VehicleKind` next
/// door used `ALL` for the complete set and `PLAYABLE` for the subset. A subset gets a name that
/// says which subset.
#[test]
fn no_all_constant_is_secretly_a_subset() {
    // Enforced by `every_all_constant_names_every_variant`; this test states the naming rule for a
    // reader and fails loudly if the sibling constant that made room for it disappears.
    let map_id =
        std::fs::read_to_string(workspace_root().join("crates/foundation/terrain/src/map_id.rs"))
            .expect("map_id.rs is readable");

    assert!(
        map_id.contains("pub const SHIPPED"),
        "MapId's shipped-map subset must keep a name that says it is a subset"
    );
}

/// The declaration order of every identity enum, PINNED. `every_all_constant_names_every_variant`
/// compares the enum body and its `ALL` as SETS, so it is blind to a reorder: swapping two variants
/// keeps the same names but silently changes every wire/asset discriminant from the swap point on,
/// misreading old blueprints, replays and packets. This golden holds the ORDER. Appending a variant
/// extends the matching list at its END — a deliberate, reviewed edit, blessed here. Reordering,
/// inserting, or deleting one changes an existing position and fails, naming the enum.
///
/// Seeded from the sources by the (removed) `dump_identity_enum_order` bootstrap; keep in sync only
/// by appending.
const IDENTITY_ENUM_ORDER: &[(&str, &[&str])] = &[
    (
        "ArmorZone",
        &[
            "UpperGlacis",
            "LowerPlate",
            "HullSide",
            "HullRear",
            "TurretFront",
            "Mantlet",
            "TurretSide",
            "TurretRear",
            "Roof",
            "LeftTrack",
            "RightTrack",
            "Skirt",
            "HullDeck",
            "Cupola",
            "GlacisPort",
        ],
    ),
    ("DamageCause", &["Shell", "Ram", "Impact", "Splash", "Drowning", "Fire", "AmmoRack"]),
    ("MapId", &["ProkhorovkaHill252_2", "BystraValley", "Scratch", "OrlinyPereval", "Ostrogorsk"]),
    (
        "MaterialRole",
        &[
            "RolledArmor",
            "CastArmor",
            "BarrelSteel",
            "TrackMetal",
            "Rubber",
            "InteriorPrimer",
            "InteriorMachinery",
            "Ammunition",
            "ExposedSteel",
            "Canvas",
            "Glass",
            "Timber",
        ],
    ),
    ("ModuleSlot", &["Engine", "Suspension", "Turret", "Gun", "AmmoRack", "Radio"]),
    ("Nation", &["Ussr", "Germany", "Britain"]),
    (
        "Penetrator",
        &["FullBoreSharp", "FullBoreBlunt", "TungstenCore", "ShapedCharge", "BlastCase"],
    ),
    ("RoadSurface", &["Dirt", "Ballast", "Cobble"]),
    (
        "RoundId",
        &[
            "Br412",
            "Br412D",
            "Bk5",
            "Of412",
            "Br365K",
            "Br365P",
            "O365K",
            "Br471B",
            "Of471",
            "Pzgr39",
            "Pzgr40",
            "SprgrL45",
            "Pzgr39_43",
            "Pzgr40_43",
            "Pzgr43",
            "SprgrPak80",
            "Pzgr39_42",
            "Pzgr40_42",
            "Sprgr42",
            "TwentyPdrApcbc",
            "TwentyPdrApds",
            "TwentyPdrHe",
            "Prototype120Ap",
            "Prototype120Apcr",
            "Prototype120He",
        ],
    ),
    (
        "SceneryKind",
        &[
            "Oak",
            "Poplar",
            "Willow",
            "FruitTree",
            "Rock",
            "Bush",
            "Pine",
            "Lamppost",
            "DebrisHeap",
            "FloraTree",
            "FloraPine",
            "FloraBush",
        ],
    ),
    ("ShellType", &["ArmorPiercing", "Apcr", "Heat", "HighExplosive"]),
    (
        "StaticCoverKind",
        &[
            "FarmBuilding",
            "RailCover",
            "TreeLine",
            "Wreck",
            "WoodenFence",
            "CityBuilding",
            "StoneWall",
            "TreeTrunk",
            // teren W3b (2026-08-26): the honest BIG rock - solid scenery stays under the
            // belly line, so a crag that reads as cover IS cover.
            "Crag",
            // teren W3b (2026-08-26): the Orliny col landmark - a Svan-style watchtower
            // whose felled stump lands in the hull-down band by its rubble fraction.
            "StoneTower",
        ],
    ),
    (
        "VehicleKind",
        &["T54_1951", "TigerI", "TigerII", "Jagdtiger", "PantherII", "IS3", "Centurion", "T34_85"],
    ),
];

#[test]
fn identity_enum_declaration_order_is_append_only() {
    let sources = rust_sources(&workspace_root());

    // Every identity enum must be pinned here, and every pin must still name a real enum — so the
    // golden cannot rot into a partial list while an unpinned identity enum reorders freely.
    let pinned: std::collections::BTreeSet<&str> =
        IDENTITY_ENUM_ORDER.iter().map(|(name, _)| *name).collect();
    for name in IDENTITY_ENUMS {
        assert!(pinned.contains(name), "identity enum `{name}` has no pinned declaration order");
    }

    let mut drift = Vec::new();
    for &(name, expected) in IDENTITY_ENUM_ORDER {
        let Some((path, text)) = sources.iter().find(|(_, text)| declares_enum(text, name)) else {
            drift.push(format!("`{name}` is pinned here but declared nowhere"));
            continue;
        };
        let actual = variants_of(text, name);
        if actual != expected {
            drift.push(format!(
                "{}: {name} order drifted\n    expected: {expected:?}\n    found:    {actual:?}\n    \
                 (append-only: a new variant goes at the END and extends the golden; a reorder, \
                 insert or delete is forbidden)",
                path.display()
            ));
        }
    }

    assert!(
        drift.is_empty(),
        "an identity enum's variant ORDER is its wire/asset discriminant — it is append-only:\n  {}",
        drift.join("\n  ")
    );
}

/// Every `pub enum Name {` in a file.
fn enums_declared_in(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("pub enum ") {
        let after = &rest[at + "pub enum ".len()..];
        let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && after[name.len()..].trim_start().starts_with('{') {
            names.push(name);
        }
        rest = after;
    }
    names
}

fn declares_enum(text: &str, name: &str) -> bool {
    enums_declared_in(text).iter().any(|declared| declared == name)
}

/// Variant names from the enum body: the identifiers at one level of indentation inside it.
fn variants_of(text: &str, name: &str) -> Vec<String> {
    let Some(at) = text
        .find(&format!("pub enum {name} "))
        .or_else(|| text.find(&format!("pub enum {name}\n")))
    else {
        return Vec::new();
    };
    let Some(open) = text[at..].find('{').map(|offset| at + offset) else { return Vec::new() };
    let body = braced(&text[open..]);

    let mut variants = Vec::new();
    let mut depth = 0usize;
    for line in body.lines() {
        let trimmed = line.trim();
        if depth == 0 && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
            let ident: String =
                trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let tail = trimmed[ident.len()..].trim_start();
            let starts_variant = ident.starts_with(|c: char| c.is_ascii_uppercase())
                && (tail.starts_with(',')
                    || tail.starts_with('{')
                    || tail.starts_with('(')
                    || tail.starts_with('=')
                    || tail.is_empty());
            if starts_variant {
                variants.push(ident);
            }
        }
        // Struct and tuple variants nest; a variant only counts at the body's own level.
        let opened = trimmed.matches(['{', '(']).count();
        let closed = trimmed.matches(['}', ')']).count();
        depth = (depth + opened).saturating_sub(closed);
    }
    variants
}

/// Variant names listed in `pub const ALL` for this enum, if it has one.
fn all_constant(text: &str, name: &str) -> Option<Vec<String>> {
    let marker = text.match_indices("pub const ALL:").find(|(at, _)| {
        let head: String = text[*at..].chars().take(160).collect();
        head.contains(&format!("[{name};")) || head.contains(&format!("[{name}]"))
    })?;
    let open = text[marker.0..].find('[').map(|offset| marker.0 + offset)?;
    let body = braced(&text[text[open..].find('=').map(|offset| open + offset)?..]);
    Some(
        body.split(&format!("{name}::"))
            .skip(1)
            .map(|piece| piece.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
            .collect(),
    )
}

/// The text between the first bracket of `text` and its match, exclusive.
fn braced(text: &str) -> &str {
    let bytes = text.as_bytes();
    let Some(open) = bytes.iter().position(|byte| matches!(byte, b'{' | b'[')) else { return "" };
    let mut depth = 0usize;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        match byte {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    ""
}

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    for dir in quality::crate_src_dirs(root) {
        collect(&dir, &mut sources);
    }
    sources
}

fn collect(dir: &Path, sources: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            sources.push((path, text));
        }
    }
}
