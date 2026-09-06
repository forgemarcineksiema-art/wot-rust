# Vehicle presentation bible

**What this is.** The one place that says what every tank LOOKS like — its nation's paint, its
scheme, its markings, its wear — so the garage, the battle and the studio show the same vehicle
and no material or texture work starts from a guess. The owner's directive (2026-09-05): „ustalić
wygląd i kolor, prezentację każdego czołgu, każdej nacji i żeby w całej grze wyglądały tak samo".
The design document's row 21 sets the rule this file lives by: **the team colour is a separate
read (markers, team list, minimap), never a repaint of the tank.**

**The law.** One material law across garage, battle and studio: the vehicle shader's material
table (`renderer_wgpu/src/shaders/vehicle.wgsl`) is the only place a role's albedo and roughness
live; the nation's paint (`game_core::Nation::paint`) is the only tint the armour takes; the
running gear, the barrel, rubber, canvas, glass and timber keep their absolute materials. A
vehicle looks the same in the hangar and on the field because both paths read the same two
things.

## 1. Nation paint (landed 2026-09-07, K24-1)

The base coat, in the shader's tint space (it multiplies the armour's ~0.44 grey albedo, so a
tint of 0.40 is ~0.18 on the plate). Two floors on every value, locked by
`nation_paints_are_distinct_and_never_read_as_burnt`: luma ≥ 0.36 (the shader reads a tint under
~0.30 luma as a burnt wreck, and full dirt drags a coat 60 % toward mud at luma 0.29), and every
pair of coats at least 0.10 apart — „czołg na 400 m musi być do zobaczenia".

| Nation | Coat | Period | Tint (linear) | Vehicles |
| --- | --- | --- | --- | --- |
| USSR | 4BO protective green — olive-khaki | 1938 → | (0.33, 0.41, 0.24) | T-34-85, T-54 obr. 1951, IS-3 |
| Germany | Dunkelgelb RAL 7028 — the factory base | Feb 1943 – 1945 | (0.62, 0.55, 0.34) | Tiger I (late E), Tiger II, Jagdtiger, Panther II (1945 paper vehicle) |
| Britain | Deep bronze green — the post-war base | 1945 → | (0.25, 0.42, 0.36) | Centurion Mk 3 |

What changed on screen: until K24 every hull carried the TEAM tint — green for the player's
side, red-brown for the other — and every tank on the field was one of two colours and nobody's.
Now a Tiger is sand, a T-54 olive, a Centurion bronze, on both teams; the enemy is the marker over
its hull (interface program H10), the team list and the minimap's diamond blip.

The garage hero used to wear a pale showroom tint (0.72, 0.76, 0.62) nothing wore in battle; it
wears its nation's coat now, in the same light.

## 2. Schemes (owed)

Per vehicle: the period scheme over the base coat. The design document asks for camouflage from
noise per nation; the client already carries a `CamoPattern` overlay (summer / winter / desert
multipliers on the base coat) with no per-nation authoring. Owed: the German 1944 three-tone
(dunkelgelb with olivgrün RAL 6003 and rotbraun RAL 8017 patches, ambush dots on late Tigers),
the Soviet plain 4BO with winter whitewash, the British plain bronze green. Each scheme is a
per-nation noise recipe with a name, not a texture.

## 3. Markings (owed)

„Oznaczenia taktyczne, numery, godła jednostek proceduralnie — czołg jest czyjś." Per nation:
Soviet three-digit turret numbers in white and slogans; German three-digit turret numbers
(black with white outline, or red) and the Balkenkreuz on the hull sides; British WD numbers,
the arm-of-service square and the formation sign on the fenders. Procedural stencils on the
turret sides and hull, numbers from the tank's roster slot so two T-54s in one battle never wear
the same number.

## 4. Wear (owed)

Clean-build intent stays (no rust, no battle damage on a fresh vehicle): service wear only —
dust on the running gear (the shader's dry-earth band), worn paint on the edges the crew climbs
over, exhaust soot behind the stacks. Battle marks are the damage system's, never painted on.

## 5. Contact sheet

`cargo run -p client --example probe -- vehicle_lineup_views` renders the roster side by side in
one light; the look goldens (`client/tests/goldens/look`, `WOT_UPDATE_GOLDENS=all`) hold the
garage hero and the battlefield frames. Every change to this file re-records both and shows the
sheet.
