# Battle First — program

**The battle is the product. Nothing around it matters until it feels the way it should.**

Everything in this folder came out of one audit session on **2026-08-01** (master @ `101068a`):
four rounds of structural audit, a file-by-file inventory of the client, the T-54 stack and the
remaining crates, a depth audit of seven gameplay systems, the first look at rendered frames, and
the first battle ever actually played and measured.

## STATUS — proposed, not started

Nothing here is implemented. Every finding carries a `file:line`; every number was measured, not
estimated. Three earlier claims were withdrawn after verification and are listed as such.

## The documents

| file | what it holds |
|---|---|
| [`program.md`](program.md) | **The plan.** Waves W0–W5, ordered by what the player feels. Start here. |
| [`audit-register.md`](audit-register.md) | Every confirmed defect with `file:line`, plus the withdrawn theses. |
| [`measurements.md`](measurements.md) | All hard numbers in one place — the baseline to compare against later. |
| [`playtest-2026-08-01.md`](playtest-2026-08-01.md) | The first battle actually played: what happened, what broke. |
| [`visual-review-2026-08-01.md`](visual-review-2026-08-01.md) | What the game looks and feels like today — defects, the absence of a style, and the strategic risk. |
| [`six-lenses.md`](six-lenses.md) | The same findings read through six professional perspectives; where they converge, and the one decision where they conflict. |
| [`first-principles.md`](first-principles.md) | Questioning the premises: the terrain primitive, armour zones, what spotting physically is, whether HP belongs at all. |
| [`gate-rules.md`](gate-rules.md) | Specification of the twelve `quality` rules that make W0. |
| [`target-architecture.md`](target-architecture.md) | Seven layers, three laws, and the documentation rule. |
| [`tank-anatomy.md`](tank-anatomy.md) | The modular-tank design: what already exists, what is missing, and why ERA and crew multipliers are the wrong next step. |

## The one sentence

This codebase makes excellent local decisions and has no mechanism to make them global — so the
same good idea appears once and is missed five times next door. The cure is not a better diagram;
it is a ratchet. **A fix without a rule is unfinished.**

## The one surprise

The three most serious findings did not come from reading code. They came from running the game
once: **the armour never ricocheted a single shell in 53 hits**, a 7v7 does not resolve inside its
own timer, and thirteen bots killed nobody in a minute. None of that is visible in the source.

## Related, outside this folder

- `docs/architecture.md`, `docs/engineering-rules.md` — the current (partly stale) statements this
  program proposes to correct.
- `docs/vehicle-forge-policy.md` — contains the project's own best statement of this program's
  thesis: *"A contract nobody runs on the real thing is a document, not a gate."*
- `crates/apps/client/examples/play_session.rs` — **temporary instrument**, the harness that
  produced `playtest-2026-08-01.md`. Delete it or promote it into the planned `probe` binary; do
  not leave it growing the client's example count.
