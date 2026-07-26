# Garage — Design Decisions

This records the *choose-on-purpose* forks taken from `garage-beta-reference.md`.
The beta reference is historical research, not a spec; every deviation from it
is documented here with a reason and a status.

## Decisions

### Carousel as the primary vehicle selector (not the vertical tech tree)

- **Status:** implemented (`crates/apps/client/src/app/garage/panels/carousel.rs`).
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

### Carousel scrolls a fixed window (no pagination)

- **Status:** implemented (`layout.rs::carousel_window` / `clamp_carousel_scroll`).
- **Beta reference:** "endless" tank storage (`garage-beta-reference.md` line 31).
- **Decision:** the carousel is a single centered row of at most `CAR_VISIBLE`
  cells; a larger roster slides a window through it with arrows at either end.
  The tech tree view stays the answer to *browsing* a large roster.
- **Watch this:** `CAR_VISIBLE` is **8** and `VehicleKind::PLAYABLE` is **8**.
  The roster is exactly at capacity, so the scrolling path — window, clamp and
  both arrows — has never been seen in a review render. The ninth vehicle is the
  first one to exercise it; look at the carousel when it lands.

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

### Per-vehicle persistent loadouts (behaviour change)

- **Status:** implemented (`garage/persistence.rs`, `garage/mod.rs`,
  `draft.rs::{to_saved, from_saved}`).
- **What changed:** the garage used to hold a *single* draft and reset it to
  stock on every vehicle switch (locked by
  `selecting_a_new_vehicle_resets_the_draft_to_its_stock_loadout`). Now each
  vehicle keeps its own edited draft: switching away stashes the outgoing draft
  and switching back restores it. The reset-on-switch test is replaced by
  `switching_vehicles_restores_each_vehicles_saved_loadout`.
- **Persistence:** the selected vehicle and every edited draft are written as
  versioned JSON to `%APPDATA%/wot-prototype/garage.json` (XDG/`HOME/.config`
  elsewhere; `garage.json` in the working dir as a last resort). Saved on every
  mutating action and on Battle. Load is best-effort: a missing, corrupt, or
  wrong-`version` file degrades to the stock garage, never a panic. Restore
  re-applies option indices through the compatibility-checked install path, so
  a stale/removed option silently falls back to stock.
- **Purity:** `GarageState::default()` stays filesystem-free (tests, offscreen
  renders); the real client opts in via `ClientApp::enable_garage_persistence()`
  at startup.

### The VEHICLE column carries the aiming promise

- **Status:** implemented (`panels/stats.rs::rows`).
- **What changed:** the column printed six numbers — HP, kW, km/h, °/s, mm of
  penetration, seconds of reload — and stopped. It is now nine, grouped as *what
  keeps the hull alive* (HP, hull/turret front armour), *what moves it* (power,
  speed, traverse) and *what it kills with* (penetration, dispersion, aim time,
  reload).
- **Why:** the pitch of this game is a gun with no ±25% roll that groups at
  0.1–0.3 mrad. `dispersion_mrad` and `aim_time_seconds` are that promise written
  as numbers, and neither of them was on the screen where the player picks a gun
  and presses Battle. Armour was the same omission on the receiving end.
- **The plate is derived, not measured against the rows.** `layout::stat_panel()`
  computes the panel from `STAT_ROWS`, because the six-row plate was hand-sized
  beside its rows and the seventh would have printed onto the hangar floor.

### Both screen tabs are tabs

- **Status:** implemented (`panels/topbar.rs`, `layout::GARAGE_TAB_*`).
- **What changed:** the top-bar plate stopped at y = 0.86 while the tab row
  hit-tested 0.785–0.845, so `GARAGE` and `TECH TREE` rendered as dim grey text
  on the hangar wall with no plate behind them. The bar now reaches down to 0.78
  and closes on a hairline, and `GARAGE` — previously drawn as a tab that
  answered to no click at all — returns to the hangar from the tech tree.
- **Lock:** `the_top_bar_plate_carries_every_control_that_sits_on_it` asserts
  every top-bar rect is inside the plate, so a control cannot hang off it again.

## Known limitations (deferred deliberately)

- **Orbit camera `MIN_PITCH = -0.05`.** The inspection camera cannot look up
  at the tank from below; it is clamped just below horizon. Acceptable for
  turntable inspection; revisit if under-hull viewing becomes a goal.
- **Per-frame overlay allocations.** `overlay::build` re-allocates the HUD
  vertex buffer and calls `format!` per slot per frame. Fine at garage scale;
  not worth caching until profiling shows it.
- **The crew column is five words and a slider.** Flattening crew to one scalar
  is the decision above, but the panel still spends the whole left third of the
  screen restating five role names that never change. Worth revisiting when crew
  gains anything per-role to say.
