# <Vehicle designation> — reference dossier

> Template (Genialna Flota, PR-05). Copy for each vehicle's PR-X.1 "Dossier and measure".
> Rule of the program: **data first, model second** — this file's anchor numbers become
> `DimensionTarget`/`RatioTarget` entries in the vehicle's reference pack *before* anyone
> edits the blueprint RON. Worked examples: `t-54.md` (richest), `is-3.md` ("Reference
> anatomy" + "Shape locks").

## Identity

Which real vehicle and which configuration, precisely: production year/batch, factory,
the specific surviving specimen if one anchors the dossier (museum, inventory number).
State what this model deliberately is NOT (nearby variants ruled out and why).

## Reference anatomy (anchor numbers)

The tape-measure truth. Every row must carry a source; "confidence" is how much the sources
agree (high = multiple independent, medium = single good source, low = derived/estimated).

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length (over tracks) | _ m | _ | _ | `DimensionKind::HullLength` |
| Width (over tracks) | _ m | _ | _ | `DimensionKind::HullWidth` |
| Height to turret roof | _ m | _ | _ | `DimensionKind::HeightToTurretRoof` |
| Overall length (gun forward) | _ m | _ | _ | `DimensionKind::OverallLengthWithGun` |
| Road wheel diameter | _ m | _ | _ | `DimensionKind::RoadWheelDiameter` |
| Turret plan (L × W) | _ × _ m | _ | _ | `RatioKind::TurretLengthToWidth` |
| Turret ring position | _ | _ | _ | `RatioKind::TurretRingPositionOnHull` |
| (vehicle-specific rows: casemate height, glacis angle, wheel count/spacing, …) | | | | benchmark cage assert |

## Form rules (what makes it *this* tank)

The five-to-ten sentences a modeller must not violate: wheel count and interleave, return
rollers or none, turret construction (cast/welded/casemate) and its signature masses,
muzzle furniture (brake/evacuator), fender/stowage character, anything the silhouette is
recognized by at 300 m.

## Shape locks

Map each form rule to the test that enforces it (benchmark cage assert, ratio target,
dimension anchor). A rule without a lock is a wish.

| Rule | Lock |
| --- | --- |
| _ | `tests/<vehicle>_benchmark.rs::<assert>` |

## Sources

Numbered list: museums (specimen + what was measured there), factory drawings, manuals,
photo galleries, model-kit cross-checks (the MiniArt pattern from the T-54). Each source
notes what it is trusted FOR — a museum specimen with post-war modifications is a shape
source, not a fittings source.

## Gameplay translation

What of the above reaches `TankSpec`/blueprint and where the model deliberately deviates
(gameplay honesty, budget, readability) — every deviation listed with its reason.

State the four fleet-slot facts explicitly — each is authored or consciously absent, never an
omission:

- `cupola_height`: authored from the records, or derived from the hitbox apex (a derived value
  is SHOOTABLE armor — say which, and why).
- `glacis_ports`: the bow's aimable weakspot patches — with the smear retired, a front without
  them presents only the mantlet and the cupola.
- `turret_loft`: without one the turret armor volume is a swept circle, not the casting.
- `<slug>.visual.ron`: which parts (if any) it authors — `None` is a decision, not an omission.

## Known deviations & follow-ups

The current model's measured misses (from the Studio report's Δ columns) and which wave
owns each fix.
