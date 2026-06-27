//! Reviewability budget for Rust sources, charged to *production* code.
//!
//! The line limit exists to keep modules reviewable, but charging test lines against it fights
//! the testing policy: every fix lands with a locking test, and the most discoverable home for a
//! unit test is right next to the code it locks. Counting those lines pushed tests into sibling
//! files as pure ceremony. So the budget skips test code — inline `#[cfg(test)] mod` spans and
//! wholly-test files — while a generous total backstop still caps any single file, so test code
//! cannot grow unbounded either.

use std::path::Path;

/// Maximum production lines per Rust source file.
pub const MAX_RUST_FILE_LINES: usize = 220;

/// Backstop for any single file including its test code.
pub const MAX_RUST_FILE_TOTAL_LINES: usize = 440;

/// True for paths that are wholly test code and get only the total backstop: integration `tests/`
/// trees and unit-test sibling modules (`tests.rs` / `*_tests.rs`).
pub fn is_test_code_path(path: &Path) -> bool {
    let in_tests_dir = path.components().any(|component| component.as_os_str() == "tests");
    in_tests_dir
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}

/// Lines of production code in a Rust source: total lines minus inline `#[cfg(test)] mod` spans.
///
/// Matches rustfmt output structurally, like the other column-0 heuristics in this crate: the
/// marker is a column-0 `#[cfg(test)]` directly above a column-0 `mod` line. A declaration
/// (`mod foo;`) spans two lines; an inline block (`mod foo {`) runs to the next column-0 `}`.
/// `#[cfg(test)]` on anything other than a `mod` (say a test-only helper fn) still counts as
/// production — conservative, and rare enough not to matter.
pub fn production_line_count(source: &str) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    let mut count = 0;
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim_end() == "#[cfg(test)]"
            && lines.get(index + 1).is_some_and(|next| next.starts_with("mod "))
        {
            let declaration = lines[index + 1].trim_end().ends_with(';');
            index += 2;
            if !declaration {
                while index < lines.len() && lines[index] != "}" {
                    index += 1;
                }
                index += 1;
            }
            continue;
        }
        count += 1;
        index += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn inline_test_module_block_does_not_count_as_production() {
        let source = "fn real() {}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn check() {}\n}\n";
        assert_eq!(production_line_count(source), 2);
    }

    #[test]
    fn test_module_declarations_do_not_hide_later_production_code() {
        // The `app/mod.rs` pattern: cfg(test) mod declarations at the top, real code below.
        let source = "#[cfg(test)]\nmod camera_tests;\nmod camera;\n\nfn real() {}\n";
        assert_eq!(production_line_count(source), 3);
    }

    #[test]
    fn cfg_test_on_a_helper_fn_still_counts_as_production() {
        // The `aim.rs` pattern: a test-only free helper mid-file, production code after it.
        let source = "#[cfg(test)]\nfn helper() {}\n\nfn real() {}\n";
        assert_eq!(production_line_count(source), 4);
    }

    #[test]
    fn source_without_tests_counts_every_line() {
        let source = "fn a() {}\nfn b() {}\n";
        assert_eq!(production_line_count(source), 2);
    }

    #[test]
    fn test_code_paths_cover_integration_trees_and_sibling_test_modules() {
        assert!(is_test_code_path(Path::new("crates/runtime/sim/tests/turret_volume.rs")));
        assert!(is_test_code_path(Path::new("crates/apps/client/src/app/camera_tests.rs")));
        assert!(is_test_code_path(Path::new("crates/apps/client/src/predict/tests.rs")));
        assert!(!is_test_code_path(Path::new("crates/apps/client/src/app/reticle.rs")));
        assert!(!is_test_code_path(Path::new("crates/foundation/game_core/src/math.rs")));
    }
}
