# Gate rules — specification for Wave 0

Twelve rules in `crates/tooling/quality`. Each is a tree scan + allowlist + teaching message, in the
form the repo already uses in `duplication.rs` — whose allowlist is burned to zero.

**Every rule lands GREEN**, carrying an explicit, commented allowlist of today's violations. New
violations are blocked from day one; the list is burned down in later PRs.

## Why an allowlist and not "fix first"

A rule that waits for a big cleanup PR does not exist until that PR lands, and in the meantime fresh
violations keep arriving. A rule that ships with a visible allowlist starts protecting immediately
and makes the debt countable.

---

| # | rule | what it forbids | allowlist at landing |
|---|---|---|---|
| 1 | `layer_rules` | a crate depending on its own layer or above (`crates/<layer>/<crate>`); the DAG in `target-architecture.md` | `editor→client`, `client→server`, `scene_build→renderer_api` |
| 2 | `exhaustive_dispatch` | `_ =>` or `..` in a match on an identity enum (`VehicleKind`, `MapId`, `ArmorZone`, `ShellType`, `ModuleSlot`, …) | `modules/catalog.rs:88`, `:103`, `armor/vehicle_volumes.rs:48` |
| 3 | `coverage_asserts` | a `for … in Enum::ALL/PLAYABLE` loop that can `continue` past missing data without a coverage floor. Model: `spaced_armor.rs:53` (`assert!(skirted >= 2, …)`) | today's 18 sites |
| 4 | `shader_binding` | an enum crossing into WGSL whose variant count does not equal the shader's branch count. Concretely: `MaterialRole::COUNT` vs the branches of `material_params()` | empty — fix on landing |
| 5 | `no_silent_caps` | `.take(N)`, `min(_, N)`, `[..N]` on a domain collection without a named constant and a test. `.take(16)` becomes `.take(MAX_OBSERVERS)` where `MAX_OBSERVERS = u16::BITS as usize`, so the tie to the mask lives in the code | `spotting.rs:270`, `vehicle.wgsl:249` |
| 6 | `data_contracts` | a contract report that iterates what exists without asserting what must exist (spawn zone per team, blueprint per playable kind, …) | empty — `report.rs` fixed on landing |
| 7 | `no_orphan_crates` | a workspace crate with no dependents and no binary target | empty after deleting `panel`, `shell`, `experimental_geometry` |
| 8 | `manifest_hygiene` | a crates.io dependency declared outside the workspace table; a raw `path =` dep; a manifest not inheriting `version`/`rust-version` | empty — `png` ×8, `ab_glyph`, `sim` path dep, 2 manifests fixed on landing |
| 9 | `gate_completeness` | a test gated behind `env::var` that is not on an explicit, justified list | `look_goldens` (to be taken off the opt-in) |
| 10 | `duplication` **extended to `tests/`** | the existing free-function duplicate scan, which today skips `tests/` — and therefore misses `workspace_root` copied into 10 of `quality`'s own 11 test files | today's `quality` duplicates |
| 11 | `naming_rules` | mixed module conventions (`mod.rs` XOR sibling file) · mixed test conventions · `foo/foo_bar.rs` stuttering · N files sharing a prefix instead of a directory · a `#[cfg(test)]` file without a marker in its name · a directory grouping on more than one axis | today's — this is the largest list |
| 12 | `honest_getters` | a field never written plus a public getter reading it | `pipeline_registry.rs:20,63` |

---

## Notes on the hard ones

**Rule 4** needs no macro or code generation. Have the enum carry `COUNT`, have the test parse the
shader's branch chain, compare the two numbers. The boundary stops being two independent constants
and becomes one constant plus one test. The same shape works for every future enum reaching the GPU.

**Rule 5** does not forbid caps — it forbids **anonymous** caps. A cap with a name and a test is a
design decision; a bare literal tied to an implicit type invariant is a trap that waits for a mode
change.

**Rule 11** is where most of today's violations live and where the burn-down is slowest. Land it
with a generous allowlist and shrink it opportunistically; the value is in blocking new sprawl, not
in a big-bang rename.

## What is deliberately NOT a rule

Assertions that a document exists or contains a phrase. Wave 5 removes the 29 hard-required `docs/`
paths and the five phrase checks — roughly 44% of today's gate — because they ratchet **against**
cleaning up stale documentation. Their replacement is editorial, not executable: **a policy cites
its enforcing test; a rule with no test is a note, not a policy.**
