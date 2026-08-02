//! The layer folders under `crates/` mean something, and this is where they start meaning it.
//!
//! `crates/{foundation,kernels,vehicle,world,runtime,render,apps,tooling}` reads like a dependency
//! order and was never enforced as one, so the folder was a comment. Two rules give it teeth:
//! nothing depends UPWARD, and an app never depends on another app.
//!
//! Both land GREEN, carrying today's violations as a named, commented allowlist — the shape
//! `duplication.rs` already uses, and the only one that starts protecting immediately instead of
//! waiting for a cleanup PR while fresh violations keep arriving. Burn the list down; do not grow
//! it.

use std::collections::HashMap;
use std::path::Path;

use quality::workspace_root;
use quality::{CrateFacts, crate_facts};

/// Depth order of the layer folders. A crate may depend on its own layer or on anything below it,
/// never above. Same-layer is allowed on purpose: `kernels` share `vehicle_geometry`, and forbidding
/// that would forbid the tree as designed rather than as broken.
fn layer_rank(layer: &str) -> Option<usize> {
    Some(match layer {
        "foundation" => 0,
        "kernels" => 1,
        // Content built out of kernels: the fleet and the maps.
        "vehicle" | "world" => 2,
        "runtime" => 3,
        "render" => 4,
        "apps" => 5,
        // Test-only architecture gates: they read the whole tree by definition.
        "tooling" => 6,
        _ => return None,
    })
}

/// Edges that exist today and are known wrong. Each is a line in `docs/battle-first`, not a
/// shrug — and each is meant to disappear, taking its entry with it.
const UPWARD_ALLOWLIST: &[(&str, &str)] = &[
    // `scene_build` assembles renderer vertices, so it is a render-layer crate sitting in `world`.
    // The fix is a move, not a rewrite (see target-architecture.md, L4).
    ("scene_build", "renderer_api"),
];

/// App-to-app edges that exist today and are known wrong.
const APP_TO_APP_ALLOWLIST: &[(&str, &str)] = &[
    // The editor pulls the whole client — winit, wgpu, cpal, audio, server — for FOUR symbols
    // (`theme`, the `push_*` primitives, `text_width`, `hud_font_atlas`). They are a `ui_kit`
    // waiting to be extracted.
    ("editor", "client"),
    // BURNED (W4): the client→server edge died when the library half of `server` moved to
    // `runtime` as `battle_host` — the client now hosts a battle through a runtime crate, and
    // `server` is the thin binary the target architecture always said it was.
];

#[test]
fn every_crate_sits_in_a_known_layer() {
    let unknown: Vec<_> = crate_facts(&workspace_root())
        .into_iter()
        .filter(|krate| layer_rank(&krate.layer).is_none())
        .map(|krate| format!("{} in crates/{}/", krate.name, krate.layer))
        .collect();
    assert!(
        unknown.is_empty(),
        "these crates sit in a folder with no declared depth — add it to `layer_rank` and say \
         where it belongs in the order: {unknown:?}"
    );
}

#[test]
fn no_crate_depends_on_a_higher_layer() {
    let facts = crate_facts(&workspace_root());
    let layer_of: HashMap<&str, &str> =
        facts.iter().map(|krate| (krate.name.as_str(), krate.layer.as_str())).collect();

    let mut offenders = Vec::new();
    for krate in &facts {
        let Some(rank) = layer_rank(&krate.layer) else { continue };
        for dep in &krate.deps {
            let Some(dep_rank) = layer_of.get(dep.as_str()).and_then(|layer| layer_rank(layer))
            else {
                continue;
            };
            if dep_rank > rank && !UPWARD_ALLOWLIST.contains(&(krate.name.as_str(), dep.as_str())) {
                offenders.push(format!(
                    "{} (crates/{}) depends on {dep} (crates/{})",
                    krate.name,
                    krate.layer,
                    layer_of[dep.as_str()]
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a crate may depend on its own layer or below, never above:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn an_app_does_not_depend_on_another_app() {
    let facts = crate_facts(&workspace_root());
    let apps: Vec<&str> = facts
        .iter()
        .filter(|krate| krate.layer == "apps")
        .map(|krate| krate.name.as_str())
        .collect();

    let mut offenders = Vec::new();
    for krate in facts.iter().filter(|krate| krate.layer == "apps") {
        for dep in &krate.deps {
            if apps.contains(&dep.as_str())
                && !APP_TO_APP_ALLOWLIST.contains(&(krate.name.as_str(), dep.as_str()))
            {
                offenders.push(format!("{} depends on {dep}", krate.name));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "apps COMPOSE the layers below them; they do not stack on each other. Whatever is shared \
         belongs in a library both can reach:\n  {}",
        offenders.join("\n  ")
    );
}

/// An allowlist that outlives its violation stops being a record and becomes permission.
#[test]
fn the_allowlists_describe_edges_that_actually_exist() {
    let facts = crate_facts(&workspace_root());
    let has_edge = |from: &str, to: &str| {
        facts.iter().any(|krate| krate.name == from && krate.deps.iter().any(|dep| dep == to))
    };
    let stale: Vec<_> = UPWARD_ALLOWLIST
        .iter()
        .chain(APP_TO_APP_ALLOWLIST)
        .filter(|(from, to)| !has_edge(from, to))
        .collect();
    assert!(
        stale.is_empty(),
        "these allowlist entries no longer describe anything — the edge is gone, so delete the \
         entry and let the rule protect it: {stale:?}"
    );
}

const _: fn(&CrateFacts) -> &Path = |krate| krate.dir.as_path();
