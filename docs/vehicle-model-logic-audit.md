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
| 7 | ~~Tracks are taut straight bands that do NOT touch the road wheels~~ **FIXED** (fleet): top run drapes onto its carriers (rests on wheels / hangs between rollers / lifted by proud wheels), v27 tension read preserved (driven lifts off, slack settles); static band follows the SAME path as the links, true wrap positions, redundant wrap drums removed | fleet | `running_gear_belt` support-polyline + drape; band loft in `chassis_blueprint` | logic |
| 8 | ~~IS-3 standing on black pallets~~ **FIXED by #228** (root cause was the old static band's bottom box hanging below the small wheels, not the mudguards); verified by `is3_studio` render | IS-3 | old `blueprint_running_gear` | floater |
| 9 | ~~T-54: interior ammunition visible OUTSIDE the hull~~ **FIXED**: the 20-round main rack topped out 9 cm PROUD of the 1.58 m foredeck (fits_within checks the hitbox box, not the deck plane); centre lowered 12 cm — rack now stops at 1.50, museum rounds with it | T-54 | `damage_layout/t54.rs` id 5 | floater |
| 10 | T-54 floaters, three sub-findings | T-54 | — | floater |
| 10a | ~~Periscope chimneys~~ **FIXED**: 24 cm slabs shrunk to Mk.4-scale heads (~6 cm proud, rooted in the casting; cage bound updated from the chimney value) | T-54 | `t54_hybrid.rs` periscope_center/half | floater |
| 10b | ~~DShK ammo can floating 7 mm off the receiver~~ **FIXED**: hangs on the receiver wall | T-54 | `t54_dshk.rs` | floater |
| 10c | ~~Open turret-ring annulus~~ **FIXED**: ring collar drum (r=ring+0.13, Turret group) seals the deck aperture under the dome skirt | T-54 | `t54.rs` turret_ring_collar | floater |
| 13 | **OPEN — the "plate with three holes"**: the turret loft's LEFT-CHEEK seam is an open boundary (turret shell has ~700 boundary edges); through it the camera sees the LIT inner primer skin with three dark D-10 round noses. Diagnosis pinned by pixel-exact raycast (tank-local frame after the example's FRAC_PI_2 hull yaw): window ≈ screen (858..926, 298..352) in `t54_bow_probe`, first FRONT hit = Turret/InteriorPrimer@y≈1.73, no exterior surface on the ray. Fix belongs in `t54_turret_loft` (close the cheek seams / make the shell watertight around the modulations) — NOT a patch plate (tried, does not cover the window). Secondary: inner skin faces OUTWARD there (it should be inward-only). | T-54 | `t54_turret_loft.rs` + `cast_loft` seam closure | logic |
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
