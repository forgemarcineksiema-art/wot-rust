//! Architecture gate: a public getter must read a field that something actually writes.
//!
//! The shape this catches is quiet and self-confirming. A struct carries a private field, a
//! `pub fn` returns it, and tests assert on the getter — but nothing in the crate ever assigns
//! the field, so the getter returns `Default::default()` forever and every assertion on it is an
//! assertion about the initialiser. The tests pass. They would pass with the surrounding logic
//! deleted.
//!
//! `PipelineRegistry::compilation_requests_during_draw` was exactly this: a counter of mid-frame
//! shader compiles that was never incremented, read by four tests that each asserted it was zero.
//! Four green tests, proving nothing about the thing they were named after.

use std::collections::BTreeSet;
use std::fs;

use quality::workspace::{crate_src_dirs, rust_files, workspace_root};

/// Fields allowed to be written by nobody, with the reason.
///
/// Empty on landing, and it should stay hard to add to: the honest fixes are to write the field
/// or to delete it along with its getter. An entry here is a claim that a value which never
/// changes is still worth publishing.
const DECORATIVE_FIELD_ALLOWLIST: &[(&str, &str)] = &[];

#[test]
fn no_public_getter_reads_a_field_nothing_writes() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        let files = rust_files(&src_dir);
        // A field can be written from anywhere in its own crate, so the read/write census has to
        // cover the whole crate rather than the file the struct is declared in.
        let crate_text = files
            .iter()
            .map(|path| fs::read_to_string(path).expect("Rust source should be readable"))
            .collect::<Vec<_>>()
            .join("\n");

        for path in &files {
            let source = fs::read_to_string(path).expect("Rust source should be readable");
            for field in private_fields(&source) {
                if occurrences(&crate_text, &field) != 2 {
                    continue;
                }
                let Some(getter) = getter_returning(&source, &field) else { continue };
                let key = format!("{}.{field}", struct_of(&source, &field).unwrap_or_default());
                if DECORATIVE_FIELD_ALLOWLIST.iter().any(|(name, _)| *name == key) {
                    continue;
                }
                offenders.push(format!(
                    "{}: `{key}` is never written, yet `{getter}` publishes it — every assertion \
                     on that getter is an assertion about the field's initialiser",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a getter over a field nothing writes is a promise with no keeper:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_allowlist_names_only_fields_that_still_exist() {
    let root = workspace_root();
    let declared: BTreeSet<String> = crate_src_dirs(&root)
        .iter()
        .flat_map(|dir| rust_files(dir))
        .flat_map(|path| {
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            private_fields(&source)
                .into_iter()
                .filter_map(|field| {
                    struct_of(&source, &field).map(|owner| format!("{owner}.{field}"))
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let stale: Vec<&str> = DECORATIVE_FIELD_ALLOWLIST
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !declared.contains(*name))
        .collect();

    assert!(
        stale.is_empty(),
        "the allowlist forgives fields that no longer exist — delete these entries: {stale:?}"
    );
}

/// Names of private (non-`pub`) struct fields declared at one level of indentation.
///
/// Four spaces and a `name: type` is a struct field in this tree; anything deeper is inside a
/// function body or a nested item, where a `name: value` is a binding or a struct literal.
fn private_fields(source: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in source.lines() {
        if !line.starts_with(char::is_whitespace) {
            in_struct = line.contains("struct ") && line.trim_end().ends_with('{');
            continue;
        }
        if !in_struct {
            continue;
        }
        let Some(rest) = line.strip_prefix("    ") else { continue };
        if rest.starts_with(char::is_whitespace) || rest.starts_with("pub") || rest.starts_with('/')
        {
            continue;
        }
        let Some((name, _)) = rest.split_once(':') else { continue };
        let name = name.trim();
        if !name.is_empty()
            && name.chars().all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            fields.push(name.to_string());
        }
    }
    fields
}

/// The struct a field was declared in, for a readable offender key.
fn struct_of(source: &str, field: &str) -> Option<String> {
    let mut current = None;
    for line in source.lines() {
        if !line.starts_with(char::is_whitespace) {
            current = line.split_once("struct ").filter(|_| line.trim_end().ends_with('{')).map(
                |(_, rest)| {
                    rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or_default()
                        .to_string()
                },
            );
            continue;
        }
        if let Some(rest) = line.strip_prefix("    ")
            && rest.split_once(':').map(|(name, _)| name.trim()) == Some(field)
        {
            return current;
        }
    }
    None
}

/// The name of a `pub fn` in this file whose body is nothing but `self.<field>`.
///
/// Deliberately narrow. A getter that computes something with the field is not this defect — the
/// pattern being caught is a value published verbatim that nothing ever sets.
fn getter_returning(source: &str, field: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("pub fn ") {
            continue;
        }
        let body = lines.get(index + 1)?.trim();
        let reads = body == format!("self.{field}")
            || body == format!("self.{field}.get()")
            || body == format!("self.{field}.clone()");
        if reads && lines.get(index + 2).map(|next| next.trim()) == Some("}") {
            let name = trimmed
                .trim_start_matches("pub fn ")
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()?;
            return Some(name.to_string());
        }
    }
    None
}

/// How many times an identifier appears as a whole word.
///
/// Two is the signature of a decorative field: the declaration, and the getter that reads it. A
/// field that is assigned anywhere — `self.f = x`, `f: value` in a literal, `&mut self.f` — has a
/// third occurrence, which is why this counts rather than trying to parse assignment.
fn occurrences(text: &str, needle: &str) -> usize {
    let bytes = text.as_bytes();
    text.match_indices(needle)
        .filter(|(at, _)| {
            let before = *at == 0 || !is_word_byte(bytes[at - 1]);
            let after_index = at + needle.len();
            let after = after_index >= bytes.len() || !is_word_byte(bytes[after_index]);
            before && after
        })
        .count()
}

fn is_word_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}
