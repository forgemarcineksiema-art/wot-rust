# Real-photo comparison — 2026-07-17 (user-requested)

Method: one canonical reference photo per vehicle (Wikipedia lead image; Bundesarchiv /
museum specimens), viewed AS IMAGES and compared against same-class game renders
(`*_probe` / `*_studio` examples at garage distance). Per the model-logic bar: findings are
mechanical/recognition features, not decoration wishes. Reference URLs at the bottom;
photos deliberately not committed (license/attribution simpler as links).

## Cross-fleet findings (highest value first)

| # | Finding | Applies to |
| --- | --- | --- |
| F1 | **Drive sprocket is at the WRONG END for the German line.** Tiger I/II, Jagdtiger, Panther II all drive from the FRONT (transmission forward — teeth clearly visible on the front end wheel in every photo); our shared placement puts the toothed sprocket at the REAR for the whole fleet (correct only for the Soviet family, T-34 and Centurion). Needs a blueprint field (`drive_front`) consumed by placements + static band + benchmark cages. | Tiger I, Tiger II, Jagdtiger, Panther II |
| F2 | **Muzzle furniture is real geometry on almost every gun and we have closed sausages.** Tiger I/II: large double-baffle brakes with OPEN chambers; Panther II specimen: ball-shaped double-baffle; IS-3 D-25T: big two-chamber brake with visible openings; Centurion 20pdr (Type A): cylindrical counterweight swelling; T-34-85 ZiS-S-53: honestly plain (our clean barrel is CORRECT there). Ties to ledger #4/#6. | fleet |
| F3 | **Bow mudguards/fenders are a fleet-wide missing signature.** T-55 family: big curved sweeps over the idlers; T-34: curved guards; Tiger II / Panther II: hinged flaps; Jagdtiger: large flat guards; Centurion: angular box guards CARRYING the headlights and mesh baskets. Only IS-3 has them today. | fleet minus IS-3 |
| F4 | **Headlights are per-vehicle, not the cloned pods.** Late Tiger I: ONE central Bosch light; T-54/55: guarded cluster on the glacis; IS-3: one guarded light; Centurion: lights ON the front fender boxes. Replaces part of clone defect #1. | fleet |

## Per-vehicle deltas (photo → game)

**Tiger I** (Bundesarchiv front-¾): spare-link row should span the FULL lower bow
(continuous ~10 shoes; ours is 4 sparse blocks); wide flat Walzenblende mantlet with
binocular sight apertures (ours: generic bulged collar); TWO forward roof hatches
(driver + radio op — ours has the cloned single hatch plate); front-drive (F1);
one central headlight (F4); front track tops visibly OPEN beside the hull between
mudguards (our slab covers the full width).

**Tiger II** (Bundesarchiv front-¾): Henschel turret is LONG with curved Turmblende
front — ours is a faceted wedge (known W1 item, confirmed); big open double-baffle brake
(F2); side skirts along the upper run (real, commonly fitted — we have none); hinged
front fender flaps (F3); front-drive (F1); steel-rim wheels have bolted dish faces —
ours read as generic rubber-band wheels.

**Jagdtiger** (Aberdeen): massive CAST COLLAR around the 12.8 cm at the casemate face —
ours has a bare barrel through a flat plate; spare-track hangers ON the casemate side
(rows of brackets); large flat bow guards (F3); front-drive (F1); this specimen has no
brake — pak 44 with brake is also documented, dossier decides (PR-JT.1).

**Panther II** (Fort Benning — OUR DECIDED configuration): wears the **Panther G turret**
with the curved cast mantlet band — our current narrow Schmalturm-ish wedge is the WRONG
configuration (already decided in the masterplan; this photo is the shape source);
ball double-baffle brake (F2); classic long-glacis bow with big curved fender sweeps and
ONE glacis headlight (F3/F4); front-drive (F1).

**IS-3** (museum): spare links BOLTED on the pike cheeks; turret handrails all around the dome —
**absent entirely, not faint**: there is no handrail geometry anywhere and no rail variant in
`ForgePartKind`, so the earlier note claiming "the part table has rails" was wrong; guarded
headlight (F4, deliberately deferred — it needs a standoff off the pike plane). D-25T brake
chambers ✓ already a real open-chambered double-baffle with a receding bore funnel (F2 satisfied
for this gun). Fenders/mudguards ✓; drums ✓; flush roof hatches + TPK periscope ✓ (the IS-3
correctly carries NO raised cupola).

**T-54 family** (T-55 museum shot as family silhouette): big curved BOW MUDGUARDS over
the idlers are the missing bow signature (F3) — everything else on the bow (splash board,
cables, fender line) already reads well; headlight cluster placement should be verified
against the T-54-3 dossier (right-side cluster on the family).

**T-34-85** (running specimen): driver's HATCH IS IN THE GLACIS FACE (big rectangular
plate with twin periscopes — ours has none: the cloned deck hatch sits on the roof
instead); hull MG ball right of centre; curved bow guards (F3); turret grab rails; rear
sponson fuel drums (already planned W2); cast-seam character on the turret.

**Centurion Mk 3** (Borden): front fender BOXES carry the headlights and a mesh stowage
basket (F3/F4); turret smoke-discharger banks flank the front; side + rear turret stowage
bins (bustle bin ✓ exists — side bin missing); 20pdr Type A muzzle counterweight swelling
(F2); rear drive ✓ correct as-is.

## Encoding plan

- F1 → new `TrackShape::drive_front` + placement/band/cage updates (one PR, fleet).
- F2 → ledger #4/#6 implementation PR (bore + per-gun furniture from these photos).
- F3/F4 + per-vehicle rows → their W1/W2 dossier-and-shape PRs; each row above becomes a
  dossier anchor or form rule before geometry moves.

## References

- Tiger I: commons `Bundesarchiv Bild 101I-299-1805-16` (CC-BY-SA 3.0 de)
- Tiger II: commons `Bundesarchiv Bild 101I-721-0398-21A` (CC-BY-SA 3.0 de)
- Jagdtiger: commons `Jagdtiger at Aberdeen proving grounds 2008.jpg`
- Panther II: commons `Panther II US Army Armor & Cavalry Collection.jpg`
- IS-3: commons `IS3.jpg`; T-54/55: commons `T-55 4.jpg`
- Centurion: commons `Centurion cfb borden 1.JPG`; T-34-85: commons `Tank T-34.JPG`
