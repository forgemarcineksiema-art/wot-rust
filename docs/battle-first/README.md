# Battle First — program

**The battle is the product. Nothing around it matters until it feels the way it should.**

Everything in this folder came out of one audit session on **2026-08-01** (master @ `101068a`):
four rounds of structural audit, a file-by-file inventory of the client, the T-54 stack and the
remaining crates, a depth audit of seven gameplay systems, the first look at rendered frames, and
the first battle ever actually played and measured.

## STATUS — executed

The proposal became the program: waves W0–W4 landed across PRs **#363–#428** (closed 2026-08-02/03)
— the gate rules, the shot-feel wave, the ammunition pass, the fleet-technology restructure, and
the combat-system ranking through the weakspot-patch retirement. Wave-by-wave status lives in the
STATUS block of [`program.md`](program.md). Every finding here carries a `file:line`; every number
was measured, not estimated; withdrawn claims are kept on the page as withdrawn.

## The documents — what is living

| file | what it holds |
|---|---|
| [`program.md`](program.md) | **The plan and its STATUS.** Waves W0–W5, ordered by what the player feels. Start here. |
| [`audit-register.md`](audit-register.md) | Every confirmed defect with `file:line`; struck rows carry the PR that closed them. |
| [`measurements.md`](measurements.md) | All hard numbers in one place — the baseline to compare against later. |
| [`combat-system.md`](combat-system.md) | The shot chain end to end, and the forward ranking with its 2026-08-03 score. |
| [`fleet-numbers-audit.md`](fleet-numbers-audit.md) | Every number a shot resolves against — armour, penetration, damage, the debt registers. |
| [`target-architecture.md`](target-architecture.md) | Seven layers, three laws, and the documentation rule. |

Six closed artifacts were deleted on 2026-08-03, their surviving content salvaged first: the
playtest and visual-review one-shots (corrected numbers live in `measurements.md` and
`audit-register.md` §A; the visual defects live in `docs/art-direction-program.md` with
`file:line` causes), the six-lenses reading (its one surviving line moved to `program.md`), the
first-principles pass (its spotting question was answered differently and shipped as #399; its
terrain question was answered by the densification withdrawal in `measurements.md`), the W0
gate-rules spec (the rules ARE the 18 test files in `crates/tooling/quality/tests/`; the
allowlist rationale moved to `target-architecture.md`), and the tank-anatomy review (its
crew-design constraints moved to `program.md`).

## The one sentence

This codebase makes excellent local decisions and has no mechanism to make them global — so the
same good idea appears once and is missed five times next door. The cure is not a better diagram;
it is a ratchet. **A fix without a rule is unfinished.**

## The one surprise

Running the game produced three dramatic findings — and **all three were my own instrument error**
(the harness filtered out every zero-damage event, which is exactly what a ricochet is). Corrected:
the armour bounces ~10 % of impacts, the battle resolves on its 600 s timer, and 9 of 14 tanks die
in a real fight. See [`audit-register.md`](audit-register.md) §A.

The lesson is worth more than the findings would have been, and it is the same one this program
argues everywhere else: **an instrument nobody checked is not evidence.**

## Related, outside this folder

- `docs/architecture.md`, `docs/engineering-rules.md` — the current (partly stale) statements this
  program proposes to correct; `engineering-rules.md` vs `verify.ps1` is still unreconciled
  (register E1).
- `docs/vehicle-forge-policy.md` — contains the project's own best statement of this program's
  thesis: *"A contract nobody runs on the real thing is a document, not a gate."*
