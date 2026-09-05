//! Architecture gate (Forge 2.0 K2): a kernel knows planes, lofts and sweeps — never a vehicle.
//!
//! `crates/kernels/solid/src/` carried `t54.rs`, `t54_fittings.rs` and `t54_plates.rs`: a
//! quarter of the kernel was one tank's hull, deck grille and periscopes, reading the blueprint's
//! `HullVisual` from `game_core`. That is how a "geometry kernel" becomes T-54-shaped and every
//! later vehicle either reuses the T-54's parts under the T-54's name or grows its own copy.
//! The parts moved to the fleet part library in `vehicle_build` (the content layer, where a
//! vehicle may be named) and this gate keeps the kernels clean: no file under `crates/kernels/`
//! — source, tests, examples or benches — may be named after a vehicle.
//!
//! The token list is checked against the roster: every playable slug must start with one of
//! them, so a new vehicle cannot slip in unnamed.

use quality::workspace::{repo_relative, rust_files, workspace_root};

/// The fleet's family tokens as they appear in file names. A new vehicle adds its family here
/// — `every_roster_slug_starts_with_a_listed_family` says so.
const VEHICLE_TOKENS: &[&str] =
    &["t54", "t34", "is3", "tiger", "panther", "jagdtiger", "centurion"];

#[test]
fn no_file_under_kernels_is_named_after_a_vehicle() {
    let root = workspace_root();
    let kernels = root.join("crates").join("kernels");
    let offenders: Vec<String> = rust_files(&kernels)
        .into_iter()
        .filter(|path| {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            VEHICLE_TOKENS.iter().any(|token| stem.split('_').any(|part| part == *token))
        })
        .map(|path| repo_relative(&path, &root))
        .collect();
    assert!(
        offenders.is_empty(),
        "a kernel file is named after a vehicle — vehicle parts belong in the part library \
         (`crates/vehicle/vehicle_build`), the kernel keeps the operator:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_roster_slug_starts_with_a_listed_family() {
    let root = workspace_root();
    let source =
        std::fs::read_to_string(root.join("crates/foundation/game_core/src/vehicle_kind.rs"))
            .expect("vehicle_kind.rs");
    // `VehicleKind::X => "slug",` — the arms of the first match after `fn slug` (line-based:
    // the working copy may carry CRLF).
    let slugs: Vec<&str> = source
        .lines()
        .skip_while(|line| !line.contains("pub fn slug("))
        .skip(1)
        .take_while(|line| line.trim() != "}")
        .filter_map(|line| line.split('"').nth(1))
        .collect();
    assert!(slugs.len() >= 8, "the roster's slugs were not read: {slugs:?}");
    let unlisted: Vec<&str> = slugs
        .iter()
        .copied()
        .filter(|slug| !VEHICLE_TOKENS.iter().any(|token| slug.starts_with(token)))
        .collect();
    assert!(
        unlisted.is_empty(),
        "a roster slug has no family token in `VEHICLE_TOKENS`, so the kernel gate cannot see \
         files named after it: {unlisted:?}"
    );
}
