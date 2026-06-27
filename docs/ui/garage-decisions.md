# Garage — Design Decisions

This records the *choose-on-purpose* forks taken from `garage-beta-reference.md`.
The beta reference is historical research, not a spec; every deviation from it
is documented here with a reason and a status.

## Decisions

### Carousel as the primary vehicle selector (not the vertical tech tree)

- **Status:** implemented (`crates/client/src/app/garage/panels/carousel.rs`).
- **Beta reference:** beta-WoT had no carousel — vehicle choice ran through the
  vertical tech tree / a list (see `garage-beta-reference.md` lines 25–26).
- **Decision:** adopt the post-release horizontal carousel as the main selection
  path because it is ergonomically superior for a small owned roster. The
  vertical tech tree survives as a **browse-only** second screen
  (`Hangar` ↔ `TechTree` view toggle), not as a selection path.
- **Why not 1:1 beta:** the beta tree was also the research/unlock UI; we have
  no economy yet, so a tree as the *primary* selector would be pure friction.

### Crew model: one shared `proficiency` scalar (no per-role skills)

- **Status:** implemented (`game_core::Crew`, `garage/draft.rs::adjust_proficiency`).
- **Beta reference:** crew with leveling per role (commander, gunner, loader,
  driver, radiotelegrafista) — see `garage-beta-reference.md` line 31.
- **Decision:** flatten crew to a single `proficiency: f32` (0.0–1.0+) that
  applies uniformly. The crew panel (`panels/crew.rs`) still draws all five
  roles for visual identity, but the slider edits one scalar.
- **Why not 1:1 beta:** per-role skills are a major system (skill tree,
  brothers-in-arms, retraining, casualties). It is out of scope for the
  prototype. The flat scalar is a deliberate simplification, not a regression.

### No tiers and no matchmaking

- **Status:** out of scope (prototype).
- **Beta reference:** matchmaking spread of roughly ±4 tiers
  (`garage-beta-reference.md` line 33).
- **Decision:** the prototype has no tier system and no matchmaking. The tech
  tree view groups vehicles by **nation** (USSR / Germany), not by tier.

### Carousel does not scale (fixed row, no pagination)

- **Status:** known limitation.
- **Beta reference:** "endless" tank storage (`garage-beta-reference.md` line 31).
- **Decision:** the carousel is a single centered row sized for the current
  5-vehicle roster. Above ~7–8 vehicles the geometry would overflow; scroll /
  pagination is deferred. The tech tree view is the answer to browsing a larger
  roster without carousel geometry breaking.

### Backward module cycle (Shift+click, right-click, `Q`)

- **Status:** implemented (G1).
- **Beta reference:** not present — beta only cycled forward through options.
- **Decision:** add an explicit backward cycle. This is a *deliberate
  improvement* over the beta reference, not a deviation. Routed through
  `GarageHit::ModuleCycle(slot, dir)` with `dir = -1` from Shift+click,
  right-click, and the `Q` key (when a slot is focused).

### Keyboard loadout editing (focus + cycle + ammo + crew)

- **Status:** implemented (G1).
- **Beta reference:** not specified — beta-era input was mouse-driven.
- **Decision:** full keyboard path for accessibility and speed:
  - `[` / `]` — move focus between module slots.
  - `Q` / `E` — cycle the focused slot's option backward / forward.
  - `Z` / `X` / `C` — select ammo slot 0 / 1 / 2.
  - `-` / `=` — adjust crew proficiency down / up.
  - `T` — toggle between `Hangar` and `TechTree` views.
  - `ArrowLeft` / `ArrowRight` — cycle vehicles (unchanged).
  - `Digit1`–`Digit5` — select vehicle by carousel index (unchanged).
  - `Enter` — commit to battle (unchanged).
  - `Escape` — close the garage (unchanged).

### Tech tree is browse-only (no economy, no research)

- **Status:** implemented (G3).
- **Beta reference:** the vertical tree was the research + unlock + selection UI.
- **Decision:** the tech tree view shows vehicles grouped by nation in
  vertical columns (the signature beta flourish). Clicking a node selects that
  vehicle and returns to the hangar. There is **no research state, no
  unlock gating, no XP, no credits** — every `VehicleKind::PLAYABLE` vehicle
  is selectable. The tree is an organisational view, not a progression system.

## Known limitations (deferred deliberately)

- **No cross-app persistence.** The loadout draft is in-memory only; restarting
  the client loses module swaps. A save/load layer is out of scope for the
  prototype.
- **Orbit camera `MIN_PITCH = -0.05`.** The inspection camera cannot look up
  at the tank from below; it is clamped just below horizon. Acceptable for
  turntable inspection; revisit if under-hull viewing becomes a goal.
- **Per-frame overlay allocations.** `overlay::build` re-allocates the HUD
  vertex buffer and calls `format!` per slot per frame. Fine at garage scale;
  not worth caching until profiling shows it.
- **Cosmetic tabs.** `DEPOT`, `STORE`, `BARRACKS` tabs in the top bar are
  decorative; only `GARAGE` and `TECH TREE` are hit-testable.
