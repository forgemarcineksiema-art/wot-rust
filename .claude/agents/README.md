# The Avengers — project agent roster

Custom Claude Code subagents for the "honest tank" project. Each carries this repo's
hard-earned lessons in its system prompt, so delegated work starts with the house rules
instead of rediscovering them. Invoke via the Agent tool (auto-delegation matches on the
`description` fields) or ask for one by name.

| Callsign | Agent | Mission | When assembled |
| --- | --- | --- | --- |
| 🏹 Hawkeye | `hawkeye-dossier` | Reference research that never misses — graded sources, variant-contamination watch, provenance-ready dossier tables | New vehicle dossiers, ReferenceSpec anchors, conflicting numbers |
| 🤖 Stark | `stark-audit` | Construction audits: reads the geometry math, calls a brick a brick, hunts buried tris and lying tests | K-register sweeps, part PR reviews, model-logic bar |
| 🔮 Strange | `strange-cascade` | Sees every timeline of a change: locks, hand-synced constants, goldens, replays, protocol — silent drift made loud BEFORE the edit | Any blueprint/dimension/enum change plan |
| 🛡️ Cap | `cap-goldens` | The shield: golden and fixture blessing with baseline-first discipline and reviewable diffs — never a rubber stamp | GOLDEN_BAKE_HASHES, studio tiles, replay re-pins |
| 💪 Hulk | `hulk-perf` | Smashes frame drops with measurements: perf_capture, budgets, ungoverned-cost hunts; one-look MX330 @ 60 | Perf before/after, densifying PRs, budget raises |
| 🎩 JARVIS | `jarvis-verify` | Runs the estate's machinery: staged verify, pitfall-proof (exit codes, worktree fmt, incremental corruption), inherited-red detection | Merge gate, pre-PR checks, "is master green?" |
| ⚡ Thor | `thor-blender` | Wields the hammer: Blender MCP loop, calibrated overlays, section-diffs vs masters. Iron law: numbers come back, meshes never do | S-sessions, shape-PR visual verification |

## House doctrine every agent inherits

- Honesty first: what blocks the shell blocks the eye; documented numbers beat convenient ones;
  a gate that cannot fail is not a gate.
- Every promise lives in a locking test; every re-bless names its cause and shows its diff.
- Evidence means `file:line`; a claim without one is a hypothesis and must say so.
- Program of record: `docs/model-idealny-t54.md` (M/K registers), dossiers in `docs/vehicles/`.
