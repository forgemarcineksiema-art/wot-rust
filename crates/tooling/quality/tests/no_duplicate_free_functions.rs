use quality::duplication::duplicated_free_functions;

/// Keeps the same helper from being pasted into several crates. New cross-`src` duplicates must
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

/// The same rule one level sharper: not a shared name, the same code.
#[test]
fn no_function_body_is_pasted_into_a_second_file() {
    let offenders = quality::duplication::identical_function_bodies();
    assert!(
        offenders.is_empty(),
        "identical function bodies in more than one file — one edit will only reach one of them; \
         hoist into a shared module (a crate lib, or `tests/common/mod.rs`):\n  {}",
        offenders.join("\n  ")
    );
}

/// An allowlist that outlives its duplication stops being a record and becomes permission —
/// the same staleness rule the layer and naming gates already apply to their own lists.
/// (Found live: `t54_object_count` sat here after its two copies had drifted apart.)
#[test]
fn the_identical_body_allowlist_describes_duplicates_that_still_exist() {
    let live = quality::duplication::names_with_identical_bodies();
    let stale: Vec<&&str> = quality::duplication::IDENTICAL_BODY_ALLOWLIST
        .iter()
        .filter(|name| !live.contains(**name))
        .collect();
    assert!(
        stale.is_empty(),
        "these entries no longer describe any identical pair — the duplication is gone, so \
         delete the entry and let the rule protect the name again: {stale:?}"
    );
}
