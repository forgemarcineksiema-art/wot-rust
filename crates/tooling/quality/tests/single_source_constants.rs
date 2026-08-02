//! Architecture gate: a physical fact has one authority, and every other crate reads it.
//!
//! This is the repo's most expensive habit, made into a rule instead of a fix. The climb grade —
//! how steep a face a tank drives — existed as THREE hand-written numbers: 0.6 in the physics
//! controller, 0.55 in the map contract's drive graph, 0.5 in the road check. Each was reasonable
//! alone. Together they meant the map gate refused ground the game let a hull climb, and nothing
//! anywhere put the two constants on the same page.
//!
//! They agree now because they are one constant. The rule is what stops a fourth copy: outside its
//! owning crate, a constant naming one of these facts must be ASSIGNED from the owner, never
//! written as a literal.

use std::fs;

use quality::workspace::{crate_src_dirs, repo_relative, rust_files, workspace_root};

/// Facts with a single authority: the fragment a constant's name carries, and the crate that owns
/// the number. Add a row when a fix like the climb grade's is made, so it stays made.
const SINGLE_SOURCE: &[(&str, &str, &str)] = &[(
    "CLIMB_GRADE",
    "game_core",
    "What ground a tank drives. The map contract's drive graph and the physics controller must \
     refuse exactly the same slopes, or a map walls off ground the hull takes.",
)];

#[test]
fn a_shared_fact_is_written_down_once() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        let owner = src_dir
            .parent()
            .and_then(|dir| dir.file_name()?.to_str())
            .unwrap_or_default()
            .to_string();
        for path in rust_files(&src_dir) {
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            for (line_number, line) in source.lines().enumerate() {
                let Some((name, value)) = const_declaration(line) else { continue };
                for (fragment, authority, why) in SINGLE_SOURCE {
                    if !name.contains(fragment) || owner == *authority {
                        continue;
                    }
                    if value.contains(&format!("{authority}::")) {
                        continue;
                    }
                    offenders.push(format!(
                        "{}:{}: `{name}` writes its own value for a fact `{authority}` owns — \
                         assign it from `{authority}::` instead. {why}",
                        repo_relative(&path, &root),
                        line_number + 1,
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a second copy of a shared number is a disagreement waiting for the conditions that \
         reveal it:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_named_authority_is_a_crate_that_exists() {
    let root = workspace_root();
    let crates: Vec<String> = crate_src_dirs(&root)
        .iter()
        .filter_map(|dir| Some(dir.parent()?.file_name()?.to_str()?.to_string()))
        .collect();
    let missing: Vec<&str> = SINGLE_SOURCE
        .iter()
        .map(|(_, authority, _)| *authority)
        .filter(|authority| !crates.iter().any(|name| name == authority))
        .collect();

    assert!(missing.is_empty(), "these authorities are not crates: {missing:?}");
}

/// `const NAME: T = value;` — the name and the value text, for a column-0 or indented declaration.
fn const_declaration(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("pub const ")
        .or_else(|| trimmed.strip_prefix("pub(crate) const "))
        .or_else(|| trimmed.strip_prefix("const "))?;
    let (name, value) = rest.split_once('=')?;
    let name = name.split(':').next()?.trim();
    Some((name.to_string(), value.to_string()))
}
