//! Architecture gate: every UI string constant is listed in its module's `ALL`, so the
//! coverage test that walks `ALL` walks every string the client can print.
//!
//! The old test hand-listed 26 of the file's 28 constants (interface program F3 found the eight
//! arc-limit and hit-outcome strings uncovered). A list a human maintains beside the thing it
//! lists drifts; this gate counts both sides.

use std::fs;

use quality::workspace::workspace_root;

const UI_STRINGS: &str = "crates/apps/client/src/ui_strings.rs";

#[test]
fn every_ui_string_constant_is_listed_in_all() {
    let root = workspace_root();
    let source = fs::read_to_string(root.join(UI_STRINGS))
        .unwrap_or_else(|_| panic!("{UI_STRINGS} should be readable"));

    let mut offenders = Vec::new();
    let mut modules = 0usize;
    for block in source.split("pub(crate) mod ").skip(1) {
        let name: String =
            block.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let body = &block[..block.rfind('}').unwrap_or(block.len())];
        let constants: Vec<&str> = body
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("pub const ")?;
                let ident: &str = rest.split(':').next()?.trim();
                (ident != "ALL").then_some(ident)
            })
            .collect();
        let Some(all_start) = body.find("pub const ALL: &[&str] = &[") else {
            offenders.push(format!("mod {name}: no `ALL` list"));
            continue;
        };
        let all_body = &body[all_start..];
        let all_end = all_body.find("];").unwrap_or(all_body.len());
        let listed = &all_body[..all_end];
        modules += 1;
        for ident in &constants {
            if !listed.contains(ident) {
                offenders.push(format!("mod {name}: `{ident}` is not in `ALL`"));
            }
        }
    }

    assert!(modules >= 2, "{UI_STRINGS}: expected the garage and battle modules, found {modules}");
    assert!(
        offenders.is_empty(),
        "{UI_STRINGS}: a string the coverage test cannot see is a string that can render as tofu:\n  {}",
        offenders.join("\n  "),
    );
}
