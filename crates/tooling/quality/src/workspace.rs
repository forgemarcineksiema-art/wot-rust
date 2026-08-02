//! Layout-agnostic workspace discovery for the architecture gates.
//!
//! Crates are grouped into layer folders under `crates/` (e.g. `crates/kernels/sdf`), so gate tests
//! can no longer assume `crates/<name>`. These helpers walk to any depth: a directory that directly
//! contains a `Cargo.toml` is treated as a crate; otherwise it is a grouping folder and we recurse.

use std::fs;
use std::path::{Path, PathBuf};

/// Every crate manifest under `<root>/crates`, at any nesting depth.
pub fn crate_manifests(root: &Path) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    collect_crate_manifests(&root.join("crates"), &mut manifests);
    manifests
}

/// Every crate `src` directory under `<root>/crates`.
pub fn crate_src_dirs(root: &Path) -> Vec<PathBuf> {
    crate_manifests(root)
        .iter()
        .filter_map(|manifest| {
            let src = manifest.parent()?.join("src");
            src.is_dir().then_some(src)
        })
        .collect()
}

/// The `src` directory of the crate named `name`, wherever its layer folder sits.
pub fn crate_src_dir(root: &Path, name: &str) -> PathBuf {
    crate_manifests(root)
        .into_iter()
        .filter_map(|manifest| manifest.parent().map(Path::to_path_buf))
        .find(|dir| dir.file_name().is_some_and(|crate_dir| crate_dir == name))
        .map(|dir| dir.join("src"))
        .unwrap_or_else(|| panic!("crate `{name}` should exist somewhere under crates/"))
}

fn collect_crate_manifests(dir: &Path, manifests: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("crate entry should be readable").path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        } else {
            collect_crate_manifests(&path, manifests);
        }
    }
}

/// One crate, parsed once, for every gate that reasons about workspace shape.
///
/// Each rule used to re-read and re-parse the manifests it cared about. Three rules doing that
/// three ways is how two of them end up disagreeing about what a dependency is — the same
/// duplication `crate::duplication` exists to police, committed inside the gate crate itself.
#[derive(Debug, Clone)]
pub struct CrateFacts {
    pub name: String,
    /// The grouping folder directly under `crates/` — the layer this crate declares itself in.
    pub layer: String,
    pub dir: PathBuf,
    /// Workspace crates in `[dependencies]`. Dev-dependencies are deliberately excluded: a test
    /// reaching sideways for a fixture is not the shipped shape of the tree.
    pub deps: Vec<String>,
    /// A binary, example or bench target — the things that justify a crate nothing depends on.
    pub has_executable_target: bool,
}

/// Parse every workspace crate once.
pub fn crate_facts(root: &Path) -> Vec<CrateFacts> {
    let manifests = crate_manifests(root);
    let names: Vec<String> = manifests.iter().filter_map(|manifest| crate_name(manifest)).collect();
    manifests
        .iter()
        .filter_map(|manifest| {
            let dir = manifest.parent()?.to_path_buf();
            let text = fs::read_to_string(manifest).ok()?;
            let layer = dir
                .parent()
                .and_then(Path::file_name)
                .map(|layer| layer.to_string_lossy().into_owned())
                .unwrap_or_default();
            Some(CrateFacts {
                name: crate_name(manifest)?,
                layer,
                deps: declared_dependencies(&text, &names),
                has_executable_target: dir.join("src/main.rs").is_file()
                    || text.contains("[[bin]]")
                    || dir.join("examples").is_dir()
                    || dir.join("benches").is_dir(),
                dir,
            })
        })
        .collect()
}

fn crate_name(manifest: &Path) -> Option<String> {
    manifest.parent()?.file_name().map(|name| name.to_string_lossy().into_owned())
}

/// Workspace crates named in `[dependencies]`, stopping at the next section so dev- and
/// build-dependencies are not counted as the shipped shape.
fn declared_dependencies(manifest: &str, workspace_crates: &[String]) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some(name) = trimmed.split(['.', ' ', '=']).next() else {
            continue;
        };
        if workspace_crates.iter().any(|candidate| candidate == name) {
            deps.push(name.to_string());
        }
    }
    deps
}

/// The workspace root: the nearest ancestor whose `Cargo.toml` declares `[workspace]`.
///
/// Layout-agnostic on purpose — moving a crate between layer directories must not touch this.
///
/// Fourteen files in this crate had a byte-identical copy of these four lines, including three
/// added the same afternoon the duplication gate was extended to reach them. That is not a story
/// about carelessness; it is what a helper too small to bother sharing does in a codebase with no
/// rule against it.
pub fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
    {
        assert!(dir.pop(), "a Cargo.toml with [workspace] should exist in an ancestor");
    }
    dir
}

/// Every `.rs` file under `root`, recursively. Empty when `root` does not exist.
pub fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths);
    paths
}

fn collect_rust_files(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

/// Every integration-test file in the workspace: `crates/<layer>/<crate>/tests/**.rs`.
///
/// Unit tests inside `src` are deliberately out of scope for the rules that use this — an env read
/// or a skipping loop there is production code, which is a different argument from a test.
pub fn integration_test_files(root: &Path) -> Vec<PathBuf> {
    crate_manifests(root)
        .iter()
        .filter_map(|manifest| manifest.parent().map(|dir| dir.join("tests")))
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| rust_files(&dir))
        .collect()
}

/// A path as the repo writes it: relative to the workspace root, forward slashes on every OS.
///
/// Allowlists in the gate name files as strings, and a `\` from Windows would silently match
/// nothing — an allowlist that matches nothing forgives nothing and hides its next offender.
pub fn repo_relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string().replace('\\', "/")
}

/// `#[cfg(test)] mod tests` lives in files named `tests.rs` / `*_tests.rs`.
pub fn is_test_module_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}
