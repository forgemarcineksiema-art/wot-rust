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
