use quality::duplication::duplicated_free_functions;

/// Complements the per-file line limit: that rule keeps modules small, this one keeps the same
/// helper from being pasted into several crates to stay under it. New cross-`src` duplicates must
/// move into a shared module (e.g. `game_core::math`) or earn a justified allowlist entry.
#[test]
fn free_functions_are_not_duplicated_across_src_modules() {
    let offenders = duplicated_free_functions();

    assert!(
        offenders.is_empty(),
        "hoist these duplicated free functions into a shared module (e.g. game_core::math) \
         instead of copy-pasting them per crate (or add a justified allowlist entry):\n{}",
        offenders.join("\n")
    );
}
