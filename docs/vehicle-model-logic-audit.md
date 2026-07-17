# Model Logic Audit — 2026-07-17 (user review, garage distance)

The user reviewed the fleet at GARAGE distance and rejected the quality bar. Every finding
below was reproduced in-repo (`garage_hangar_review` example) and traced to code. This file
is the working ledger for the fix program; a row leaves only when the fix is verified at
garage distance, not when a gate goes green.

**The process failure this ledger exists to fix:** models were reviewed at lineup distance
and accepted on numbers (dimensions, ratios, budgets). None of those gates ask: can a human
enter this hatch? does the barrel have a bore? does the smoke have anywhere to leave? is
anything floating? does the track actually carry the wheels? From now on the seal gate IS
the close-up functional review; numbers are necessary, never sufficient.

## The two systemic causes

1. **The clone factory.** `recipes/chassis_blueprint.rs::blueprint_deck_details` stamps an
   identical driver hatch, two identical headlight pods, four identical tow hooks, one
   identical antenna pot and the same two engine-deck panels onto EVERY blueprint vehicle;
   `add_cupola` stamps the same undersized drum; `build_gun` gives every gun the same
   capped-sausage muzzle brake and the same bulged mantlet collar. One shape, whole fleet —
   this is why "klocki są praktycznie we wszystkich pojazdach i tak samo".
2. **No functional-logic pass.** Nothing — human or automated — ever asked the model to
   make mechanical sense up close.

## Defect ledger

| # | Defect | Where seen | Code root | Class |
|---|---|---|---|---|
| 1 | Identical chamfered "klocki" (headlight pods, antenna pots, tow hooks, driver hatch) on every vehicle | fleet | `blueprint_deck_details` | clone |
| 2 | Identical raised engine-deck panels on every rear deck | fleet | `blueprint_deck_details` (panels loop) | clone |
| 3 | Cupola = plain drum, far too small for a human to pass (⌀~0.5 m, no lid, no vision blocks) | fleet (dome vehicles) | `add_cupola` + blueprint `cupola_radius` | clone + logic |
| 4 | Muzzle is a CLOSED cap — no bore; "muzzle brake" is a fatter closed sausage, no baffles/ports | fleet | `armament.rs::build_gun` (capped revolve; brake = capped cylinder) | logic |
| 5 | No engine ventilation anywhere: no intake louvres, no radiator grilles; exhaust exists only on Tiger I | fleet | recipes (missing feature) | logic |
| 6 | Mantlet collar identical across guns (one bulge profile, scaled) | fleet | `build_gun` mantlet profile | clone |
| 7 | Tracks are taut straight bands that do NOT touch the road wheels — wheels hang in air above the bottom run; no sag, no wrap contact | fleet | `running_gear_belt` (stadium path) + belt band y-placement | logic |
| 8 | IS-3: dark mudguard flaps reach BELOW the belt — the tank appears to stand on black pallets | IS-3 | `recipes/is3.rs` fender/mudguard extents | floater |
| 9 | T-54: interior ammunition (gold rounds) visible OUTSIDE the hull, lying on the glacis | T-54 | interior parts vs hull occlusion (t54_interior placement/frame) | floater |
| 10 | T-54: block floating in the air near the bow; 3-hole block on the glacis shoulder oddly attached | T-54 | `t54_kit.rs` fender boxes (hardcoded z vs fender span) / details | floater |
| 11 | Turrets sit in/on visually wrong seats (IS-3 dome rises from a rectangular cutout) | IS-3 (at least) | recipe hull top vs dome interface | logic |
| 12 | Hatches exist as flat plates only; no hinges, no handles, no way to read them as openable | fleet | deck details / turret fittings | logic |

## The new gate (applies to every vehicle PR from now on)

Before any "sealed" claim:
1. **Garage-distance renders** (front, rear ¾ low, flank close, turret close, gun close) —
   reviewed for: floaters, interpenetrations, closed openings, scale absurdities.
2. **Functional checklist**: every hatch passes a 0.55 m torso; the barrel has a visible
   bore; engine deck has intake AND exhaust paths; tracks carry the wheels (contact, sag);
   every attached thing has a bracket/support; nothing shares its exact shape with another
   vehicle unless the real vehicles shared it.
3. Numbers gates (dimensions/ratios/budgets) stay — as the floor, not the bar.

## Fix order (root cause first)

1. **Track physics read** (defect 7): wheels ride IN the belt — bottom run carries the
   wheels with sag between stations, top run rests on/sags over wheels (T-54 already has
   top-run logic; generalize contact honestly). Fleet-wide, one kernel change.
2. **Gun logic** (4, 6): bore the muzzle, per-gun muzzle furniture (real brake shapes:
   single/double-baffle where documented), per-vehicle mantlet from its dossier.
3. **Floaters & interpenetrations** (8, 9, 10, 11): eliminate one by one at garage distance.
4. **De-clone program** (1, 2, 3, 12): retire `blueprint_deck_details` in favour of
   per-vehicle deck sets sourced from each dossier (hatch sizes and positions, lights,
   ventilation layout per vehicle); cupolas become per-vehicle parts at human scale.
5. **Ventilation** (5): per-family intake/exhaust from photos (dossier item).

## Status

- Tiger I "SEALED" claim (PR #226): **REOPENED** — sealed on numbers, fails the new gate
  (closed muzzle, cloned deck fittings, taut tracks).
