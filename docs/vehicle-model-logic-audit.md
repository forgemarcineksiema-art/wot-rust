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
| 1 | ~~Identical chamfered "klocki" on every vehicle~~ **FIXED (deck part)**: `blueprint_deck_details` DELETED; `recipes/deck_details.rs` shares only CONSTRUCTION (hinged rect/round hatch, guarded headlight drum, tow hook), every vehicle owns its LAYOUT from the photo comparison — Tiger I/II/Panther II two bow round hatches + one central Bosch light, Jagdtiger bow pair ahead of the casemate, T-34 driver hatch IN THE GLACIS + twin periscope hoods + MG ball, T-54 driver hatch left + guarded light right, Centurion hatch right + NO lights (they belong on the future fender boxes), IS-3 pike-aware rear-ring hatch (its guarded light waits for the pike-plane standoff in W2). Remaining for dossier PRs: per-vehicle light clusters on fender boxes/glacis per F4. | fleet | `deck_details.rs` | clone |
| 2 | ~~Identical raised engine-deck panels on every rear deck~~ **FIXED (family level)**: three honest family decks — German central grille frame + TWO round cooling-fan armours over dark intake mesh (Tiger/Panther signature), Soviet transverse louvre strips (T-54/T-34/IS), British flat panel + dark mesh grille (Centurion). Engine bay starts BEHIND the turret seat (`engine_bay_front`) — the old frame ran under the turret skirt. Per-VEHICLE deck deltas (louvre counts, hatch splits) are dossier work. | fleet | `deck_details.rs` engine_deck_* | clone |
| 3 | Cupola = plain drum, far too small for a human to pass (⌀~0.5 m, no lid, no vision blocks) | fleet (dome vehicles) | `add_cupola` + blueprint `cupola_radius` | clone + logic |
| 4 | ~~Muzzle is a CLOSED cap; brake is a closed sausage~~ **FIXED** (fleet): every gun ends in a recessed dark bore; every braked gun (KwK 36/43, KwK 42, D-25T, PaK 44) wears a real double-baffle — inner blast tube + two rings with OPEN chambers (shapes from the photo comparison). T-54/T-34 stay honestly clean. Deferred to dossier PRs: Centurion 20pdr counterweight swelling, Jagdtiger cast collar. | fleet | `armament.rs` | logic |
| 5 | No engine ventilation anywhere: no intake louvres, no radiator grilles; exhaust exists only on Tiger I | fleet | recipes (missing feature) | logic |
| 6 | Mantlet collar identical across guns (one bulge profile, scaled) — **muzzle half solved with #4**; per-vehicle mantlet MASSES (Walzenblende, Turmblende, Saukopf collar, G-blende) remain W1 dossier work | fleet | `build_gun` mantlet profile | clone |
| 7 | ~~Tracks are taut straight bands that do NOT touch the road wheels~~ **FIXED** (fleet): top run drapes onto its carriers (rests on wheels / hangs between rollers / lifted by proud wheels), v27 tension read preserved (driven lifts off, slack settles); static band follows the SAME path as the links, true wrap positions, redundant wrap drums removed | fleet | `running_gear_belt` support-polyline + drape; band loft in `chassis_blueprint` | logic |
| 8 | ~~IS-3 standing on black pallets~~ **FIXED by #228** (root cause was the old static band's bottom box hanging below the small wheels, not the mudguards); verified by `is3_studio` render | IS-3 | old `blueprint_running_gear` | floater |
| 9 | ~~T-54: interior ammunition visible OUTSIDE the hull~~ **FIXED**: the 20-round main rack topped out 9 cm PROUD of the 1.58 m foredeck (fits_within checks the hitbox box, not the deck plane); centre lowered 12 cm — rack now stops at 1.50, museum rounds with it | T-54 | `damage_layout/t54.rs` id 5 | floater |
| 10 | T-54 floaters, three sub-findings | T-54 | — | floater |
| 10a | ~~Periscope chimneys~~ **FIXED**: 24 cm slabs shrunk to Mk.4-scale heads (~6 cm proud, rooted in the casting; cage bound updated from the chimney value) | T-54 | `t54_hybrid.rs` periscope_center/half | floater |
| 10b | ~~DShK ammo can floating 7 mm off the receiver~~ **FIXED**: hangs on the receiver wall | T-54 | `t54_dshk.rs` | floater |
| 10c | ~~Open turret-ring annulus~~ **FIXED**: ring collar drum (r=ring+0.13, Turret group) seals the deck aperture under the dome skirt | T-54 | `t54.rs` turret_ring_collar | floater |
| 13 | ~~"Plate with three holes"~~ **FIXED** — and the seam diagnosis was WRONG: the loft shell is watertight; a full hit-walk on the window ray showed a FRONT-facing InteriorPrimer face AT z 1.095 IN FRONT of the casting (z 1.067). The "plate" was the **10-RT radio panel and its three dials poking through the turret front** (`radio_control_face` z 1.095, `radio_control_dial` ×3 z 1.114; the gameplay Radio OBB reached z 1.18 vs casting ~1.05 — `fits_within` is hitbox-blind, same class as the ammo rack). Radio pulled 18 cm rearward; verified gone in `t54_bow_probe`. | T-54 | `damage_layout/t54.rs` id 7 | floater |
| 11 | Turrets sit in/on visually wrong seats (IS-3 dome rises from a rectangular cutout) | IS-3 (at least) | recipe hull top vs dome interface | logic |
| 12 | Hatches exist as flat plates only — **HULL hatches FIXED**: every deck/glacis hatch now carries a hinge bar/lug and a grab handle (reads as a door, not a sticker). TURRET hatches (cupola lids, loader hatches) remain with #3. | fleet | deck details ✓ / turret fittings open | logic |

| 14 | **User observation (confirmed): one track-shoe design for the whole fleet.** `track_link_unit_mesh` is a single generator (plate + centre guide horns + pins) parameterized only by dimensions — the Germans, Centurion and T-34-85 read as wearing the SAME track; the T-54 and IS-3 read "dedicated" only because their wheels (starfish / twelve-rib) and belt paths differ. Real vehicles: OMSh small-pitch (T-54/IS-3 family), Kgs 63/725 double-pin (Tiger), waffle plate (T-34), Centurion's own cast shoe. Per-vehicle shoe patterns belong to the de-clone program, sourced from each dossier. Same applies to the shared road-wheel generator for the non-Soviet fleet. | fleet | `running_gear_geom` (shoe), `running_gear_wheels` (wheel) | clone |
| 15 | ~~Jagged "drunken" top run on short-span fleets~~ **FIXED** (fleet): a sinus dip per ~0.5 m span aliased against the ~0.5 m link pitch into a zigzag (Germans, Centurion; the T-54's ~0.92 m spans masked it). Spans under 0.85 m now run dead straight — on the real vehicles tension flattens them; sag lives in the long gaps (wrap→first wheel, roller bays). Verified: Tiger/T-34-85/Centurion profile renders. | fleet | `running_gear_belt` span sag threshold | logic |

| 16 | ~~German line drives from the WRONG END~~ **FIXED**: `TrackShape::drive_front` (German four = true), sprocket/idler swap in placements, thrown-track remnant drapes over whichever end drives, Tiger cage asserts bow sprocket; verified in `tiger_i_studio` render (teeth at the bow). Original finding: Tiger I/II, Jagdtiger, Panther II all have FRONT drive sprockets (photo-confirmed on all four); our shared placement puts the toothed wheel at the rear fleet-wide (correct only for Soviets, T-34, Centurion). Needs `TrackShape::drive_front` + placements/band/cages. Full photo comparison: docs/vehicle-photo-comparison-2026-07.md (also enriches #1 headlights, #3 hatches, #4/#6 muzzle furniture shapes, and adds fleet-wide bow mudguards). | German line | `running_gear_place` end-wheel assignment + blueprint | logic |

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
