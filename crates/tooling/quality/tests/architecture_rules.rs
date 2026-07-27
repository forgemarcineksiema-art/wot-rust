use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn core_architecture_docs_exist() {
    let root = workspace_root();
    for required_doc in [
        "docs/architecture.md",
        "docs/aiming-model-policy.md",
        "docs/armored-battle-domain.md",
        "docs/battle-camera-policy.md",
        "docs/combat-policy.md",
        "docs/debug-tools-policy.md",
        "docs/destruction-program.md",
        "docs/engineering-rules.md",
        "docs/gpu-upload-policy.md",
        "docs/pipeline-policy.md",
        "docs/physics-policy.md",
        "docs/render-boundary.md",
        "docs/server-first-policy.md",
        "docs/simulation-render-separation.md",
        "docs/testing-and-regression.md",
        "docs/map-forge-policy.md",
        "docs/terrain-large-world-policy.md",
        "docs/vehicle-forge-policy.md",
        "docs/vehicle-geometry-policy.md",
        "docs/vehicle-movement-policy.md",
        "docs/wgpu-capability-model.md",
        "docs/winit-event-loop-policy.md",
        "docs/wgsl-layout-policy.md",
    ] {
        assert!(
            root.join(required_doc).is_file(),
            "missing required architecture doc: {required_doc}"
        );
    }
}

#[test]
fn world_forge_stays_renderer_free() {
    let root = workspace_root();
    let manifest = fs::read_to_string(root.join("crates/world/world_forge/Cargo.toml"))
        .expect("world_forge manifest exists");
    let mut offenders = Vec::new();
    for dependency in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
        if manifest_has_dependency(&manifest, dependency) {
            offenders.push(format!("world_forge manifest depends on {dependency}"));
        }
    }
    for source in rust_files(&root.join("crates/world/world_forge/src")) {
        let source_text = fs::read_to_string(&source).expect("world_forge source is readable");
        for crate_name in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
            if source_uses_crate(&source_text, crate_name) {
                offenders.push(format!("{} references {crate_name}", source.display()));
            }
        }
    }
    assert!(offenders.is_empty(), "world_forge must stay renderer-free: {offenders:?}");
}

#[test]
fn map_forge_stays_renderer_free() {
    let root = workspace_root();
    let manifest = fs::read_to_string(root.join("crates/world/map_forge/Cargo.toml"))
        .expect("map_forge manifest exists");
    let mut offenders = Vec::new();
    for dependency in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
        if manifest_has_dependency(&manifest, dependency) {
            offenders.push(format!("map_forge manifest depends on {dependency}"));
        }
    }
    for source in rust_files(&root.join("crates/world/map_forge/src")) {
        let source_text = fs::read_to_string(&source).expect("map_forge source is readable");
        for crate_name in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
            if source_uses_crate(&source_text, crate_name) {
                offenders.push(format!("{} references {crate_name}", source.display()));
            }
        }
    }
    assert!(offenders.is_empty(), "map_forge must stay renderer-free: {offenders:?}");
}

#[test]
fn vehicle_forge_stays_renderer_free() {
    let root = workspace_root();
    let manifest = fs::read_to_string(root.join("crates/vehicle/vehicle_forge/Cargo.toml"))
        .expect("vehicle_forge manifest exists");
    let mut offenders = Vec::new();

    for dependency in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
        if manifest_has_dependency(&manifest, dependency) {
            offenders.push(format!("vehicle_forge manifest depends on {dependency}"));
        }
    }

    for source in rust_files(&root.join("crates/vehicle/vehicle_forge/src")) {
        let source_text = fs::read_to_string(&source).expect("vehicle_forge source is readable");
        for crate_name in ["renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
            if source_uses_crate(&source_text, crate_name) {
                offenders.push(format!("{} references {crate_name}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "vehicle_forge must stay an authoring/bake layer, not a renderer layer:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn wgpu_stays_confined_to_renderer_backend() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for manifest in quality::crate_manifests(&root) {
        let crate_name = crate_name_from_manifest(&manifest);
        let manifest_text =
            fs::read_to_string(&manifest).expect("crate manifest should be readable");

        if crate_name != "renderer_wgpu" && manifest_has_dependency(&manifest_text, "wgpu") {
            offenders.push(format!("{crate_name} manifest depends on wgpu"));
        }

        if ["game_core", "sim", "net", "physics"].contains(&crate_name.as_str()) {
            for dependency in ["renderer", "renderer_api", "renderer_wgpu"] {
                if manifest_has_dependency(&manifest_text, dependency) {
                    offenders.push(format!("{crate_name} manifest depends on {dependency}"));
                }
            }
        }
    }

    for crate_name in ["game_core", "sim", "net", "physics", "renderer_api", "server"] {
        for source in rust_files(&quality::crate_src_dir(&root, crate_name)) {
            let source_text = fs::read_to_string(&source).expect("Rust source should be readable");
            if source_uses_crate(&source_text, "wgpu") {
                offenders.push(format!("{} references wgpu", source.display()));
            }
        }
    }

    assert!(offenders.is_empty(), "wgpu must stay behind renderer_wgpu:\n{}", offenders.join("\n"));
}

#[test]
fn server_stays_headless_and_renderer_free() {
    let root = workspace_root();
    let manifest = fs::read_to_string(root.join("crates/apps/server/Cargo.toml"))
        .expect("server manifest exists");
    let mut offenders = Vec::new();

    for dependency in ["renderer", "renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
        if manifest_has_dependency(&manifest, dependency) {
            offenders.push(format!("server manifest depends on {dependency}"));
        }
    }

    for source in rust_files(&root.join("crates/apps/server/src")) {
        let source_text = fs::read_to_string(&source).expect("server source should be readable");
        for crate_name in ["renderer", "renderer_api", "renderer_wgpu", "wgpu", "winit", "egui"] {
            if source_uses_crate(&source_text, crate_name) {
                offenders.push(format!("{} references {crate_name}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "server must stay headless and renderer-free:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn workspace_has_protocol_snapshots_replays_and_benchmarks() {
    let root = workspace_root();
    for required_path in [
        "crates/runtime/net/tests/snapshots/input_command_v1.hex",
        "crates/runtime/sim/tests/replays/drive_forward_v1.json",
        "crates/runtime/sim/benches/fixed_tick.rs",
        "crates/runtime/sim/benches/combat_hot_path.rs",
        "crates/runtime/net/benches/protocol_codec.rs",
        "assets/maps/prokhorovka_hill_252_2.terrain.json",
        "assets/vehicles/t54_1951.vehicle.json",
        "assets/vehicles/tiger_i_ausf_e.vehicle.json",
        "assets/vehicles/tiger_ii_ausf_b.vehicle.json",
        "assets/vehicles/jagdtiger.vehicle.json",
        "assets/vehicles/panther_ii.vehicle.json",
        "assets/vehicles/is3.vehicle.json",
        "assets/vehicles/t34_85.vehicle.json",
        "docs/maps/prokhorovka-hill-252-2.md",
        "docs/vehicles/panzerkampfwagen-vi-tiger.md",
        "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
        "docs/vehicles/jagdtiger.md",
        "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
        "docs/vehicles/is-3.md",
        "docs/vehicles/kv-1.md",
        "scripts/verify.ps1",
        ".github/workflows/ci.yml",
    ] {
        assert!(
            root.join(required_path).is_file(),
            "missing required quality gate artifact: {required_path}"
        );
    }
}

fn workspace_root() -> PathBuf {
    // Layout-agnostic: the nearest ancestor whose Cargo.toml declares [workspace].
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
    {
        assert!(dir.pop(), "a Cargo.toml with [workspace] should exist in an ancestor");
    }
    dir
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths);
    paths
}

fn crate_name_from_manifest(manifest: &Path) -> String {
    manifest
        .parent()
        .and_then(Path::file_name)
        .expect("crate manifest should have crate directory")
        .to_string_lossy()
        .into_owned()
}

fn collect_rust_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("workspace crates directory should be readable") {
        let entry = entry.expect("workspace entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
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
            && (trimmed.contains(&format!("{crate_name}::"))
                || trimmed.starts_with(&format!("use {crate_name}"))
                || trimmed.starts_with(&format!("extern crate {crate_name}")))
    })
}
