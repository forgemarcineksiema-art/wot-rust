// The gate holds rules about CODE. The old `core_architecture_docs_exist` here — a hard list of
// 23 `docs/` paths — and the phrase-grep tests in the sibling files are gone by decision
// (battle-first, 2026-08-01): a policy document earns its keep by being cited from the tests
// that enforce it, not by a test asserting the file exists or contains a sentence.
use quality::{rust_files, workspace_root};
use std::fs;

/// Every piece of the render surface, and the exhaustive list of crates allowed to name it.
///
/// Default-deny: a crate absent from a row may not declare that dependency in its manifest nor
/// name it in its source. That is the difference between this table and the five hand-written
/// tests it replaces — those protected `world_forge`, `map_forge`, `vehicle_forge` and `server`
/// by name, and a sixth crate that should have been renderer-free was protected by nobody.
///
/// The rows are not one policy but four, and writing them out makes that visible:
///   - `wgpu` is confined to a single crate, so a backend swap has one blast radius.
///   - `winit` reaches only the two crates that own a window.
///   - `renderer_wgpu` is the backend; only the apps that present may bind it.
///   - `renderer_api` is the boundary type layer, so it reaches further ON PURPOSE — including
///     down into `scene_build`, which builds vertex buffers in the layout the API declares.
const RENDER_SURFACE: &[(&str, &[&str])] = &[
    ("wgpu", &["renderer_wgpu"]),
    ("winit", &["client", "editor"]),
    // Declared by nobody today. The empty row is the statement: a UI toolkit re-entering this
    // workspace names its crates here first.
    ("egui", &[]),
    ("renderer_wgpu", &["client", "editor"]),
    // `ui_kit` (W4): the shared HUD kit emits `HudVertex` triangles — it owns the boundary
    // TYPES without touching a device or a window, which is exactly why the editor can draw
    // through it instead of pulling the whole client.
    ("renderer_api", &["client", "editor", "renderer_wgpu", "scene_build", "ui_kit"]),
];

/// One rule where there were five, applying to all 32 crates instead of four.
///
/// `server` is the reason this is default-deny rather than a layer rule: `crates/apps` sits ABOVE
/// `crates/render`, so the dependency direction is legal and only policy keeps the server
/// headless. Policy that lives in one hand-written test is policy that protects one crate.
#[test]
fn the_render_surface_stays_confined_to_the_crates_that_own_it() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for krate in quality::crate_facts(&root) {
        let manifest = fs::read_to_string(krate.dir.join("Cargo.toml"))
            .expect("every crate manifest is readable");
        let sources: Vec<String> = rust_files(&krate.dir.join("src"))
            .into_iter()
            .map(|path| {
                let text = fs::read_to_string(&path).expect("Rust source is readable");
                format!("{}\u{0}{text}", path.display())
            })
            .collect();

        for (surface, permitted) in RENDER_SURFACE {
            if krate.name == *surface || permitted.contains(&krate.name.as_str()) {
                continue;
            }
            if manifest_has_dependency(&manifest, surface) {
                offenders.push(format!("{} manifest depends on {surface}", krate.name));
            }
            for source in &sources {
                let (path, text) = source.split_once('\u{0}').expect("path and text are joined");
                if source_uses_crate(text, surface) {
                    offenders.push(format!("{path} references {surface}"));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the render surface reached a crate that does not own it — either the dependency is wrong, \
         or RENDER_SURFACE should say so:\n{}",
        offenders.join("\n")
    );
}

/// Permission granted and never used is permission nobody re-examined. A name in the table that no
/// longer depends on its surface has to leave, so the table keeps describing the real graph.
#[test]
fn the_render_surface_table_grants_nothing_unused() {
    let root = workspace_root();
    let facts = quality::crate_facts(&root);
    let mut stale = Vec::new();

    for (surface, permitted) in RENDER_SURFACE {
        for name in *permitted {
            let Some(krate) = facts.iter().find(|krate| krate.name == *name) else {
                stale.push(format!("{surface}: `{name}` is not a crate in this workspace"));
                continue;
            };
            let manifest = fs::read_to_string(krate.dir.join("Cargo.toml"))
                .expect("every crate manifest is readable");
            if !manifest_has_dependency(&manifest, surface) {
                stale.push(format!("{surface}: `{name}` no longer depends on it"));
            }
        }
    }

    assert!(
        stale.is_empty(),
        "RENDER_SURFACE grants permission nobody uses:\n{}",
        stale.join("\n")
    );
}

#[test]
fn workspace_has_protocol_snapshots_replays_and_benchmarks() {
    let root = workspace_root();
    for required_path in [
        // A wire fixture a test actually READS. The old entry named `input_command_v1.hex`,
        // which no test had opened for many protocol versions — the gate was guarding a file
        // whose deletion nothing else would have noticed.
        "crates/runtime/net/tests/snapshots/input_command_v33.hex",
        "crates/runtime/sim/tests/replays/drive_forward_v1.json",
        "crates/runtime/sim/benches/fixed_tick.rs",
        "crates/runtime/sim/benches/combat_hot_path.rs",
        "crates/runtime/net/benches/protocol_codec.rs",
        "scripts/verify.ps1",
        // Dormant while CI billing is blocked, and kept for exactly that reason: restoring CI
        // should be one commit, not an archaeology exercise.
        ".github/workflows/ci.yml",
    ] {
        assert!(
            root.join(required_path).is_file(),
            "missing required quality gate artifact: {required_path}"
        );
    }
}

/// Every blueprint-born vehicle ships a baked asset, counted rather than named.
///
/// The list this replaced was hand-typed and had already drifted: it named seven of the eight
/// vehicles, and the missing one (Centurion) was therefore protected by nobody — the exact
/// failure this file's `RENDER_SURFACE` rule was made default-deny to avoid. Counting the two
/// directories against each other means the ninth vehicle is covered on the day it is authored.
#[test]
fn every_blueprint_vehicle_has_a_baked_asset() {
    let root = workspace_root();
    let count = |dir: &str, suffix: &str| -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            panic!("{dir} must exist — it is where this gate looks");
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(suffix))
            .map(|n| n.trim_end_matches(suffix).to_string())
            .collect();
        names.sort();
        names
    };

    let blueprints = count("crates/foundation/game_core/blueprints", ".blueprint.ron");
    let assets = count("assets/vehicles", ".vehicle.json");

    assert!(!blueprints.is_empty(), "the blueprint fleet cannot be empty");
    assert_eq!(
        blueprints, assets,
        "every blueprint vehicle ships a baked asset and vice versa;\n  blueprints: {blueprints:?}\n  assets:     {assets:?}"
    );
}

fn manifest_has_dependency(manifest: &str, dependency: &str) -> bool {
    manifest.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with('#')
            && (trimmed.starts_with(&format!("{dependency} ="))
                || trimmed.starts_with(&format!("{dependency}.workspace"))
                || trimmed.starts_with(&format!("{dependency}.path")))
    })
}

fn source_uses_crate(source: &str, crate_name: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with("//")
            && (names_path_root(trimmed, &format!("{crate_name}::"))
                || trimmed.starts_with(&format!("use {crate_name}"))
                || trimmed.starts_with(&format!("extern crate {crate_name}")))
    })
}

/// `renderer_wgpu::WindowRenderer` is not a use of `wgpu`, and a plain substring search cannot tell
/// the difference. The name has to START a path segment to count.
///
/// This mattered the moment the render-surface rule stopped scanning a hand-picked list of crates:
/// every crate that legitimately binds `renderer_wgpu` read as a `wgpu` violation.
fn names_path_root(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(at, _)| {
        at == 0
            || !matches!(line.as_bytes()[at - 1], b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
    })
}
