# Interface 2.0 — The Policy

The interface's target look, its layout, its rules and the locks that hold them. Graduated on
2026-09-06 from Part I of `docs/interface-program.md`, when that program's register closed — the
way `docs/art-direction-policy.md` stands for the picture. The program file is history: its register
says what was wrong, which lock closed it and which PR landed it; this file says what the screen
is. A change to the interface cites a section of this policy or adds a dated decision to the table
below.

The owner's verdict of 2026-09-05, verbatim, is the mandate:

> „Trzeba cały interfejs, HUD przeprojektować, przebudować, stworzyć od nowa, w końcu lepszy i
> prawdziwy interfejs HUD."
>
> „Jak obecnie jest tak płasko, z jednym akcentem, kolorem — no to jest do dupy."
>
> „Przy projektowaniu tego nie ma blokad. Trzeba to zrobić porządnie, aby graczom się podobało,
> było czytelnie i intuicyjnie, a także klimatycznie."

Where this document and `docs/game-design.md` disagree, the reconciliation table of that file wins;
rows 13–20 (2026-09-05) record the decisions below.

---

## The decisions this policy is built on

Taken 2026-09-05 with the owner, unless dated otherwise, for the program that built this policy. Each is a commitment, not a default.

| Decision | Choice |
|---|---|
| **The look** | **Steel, enamel and instrument glass.** Panels are physical 1940–50s instrument plates with depth — bevel, inset, shadow, brushed or painted steel, worn stencil — glass over live readouts with a soft reflection band, warm tungsten lamp light on live values, and a FULL semantic palette: team self/ally/enemy, ammunition by type, module ok/damaged/destroyed, verdict pen/held/ricochet/shatter, an HP ramp, one commit red. This revokes GDD §10 „ciemny, płaski, jeden kolor akcentu, zero gradientów" and the policy's „instrument, not decoration" (reconciliation row 13). Kept from the old look: tabular digits, contrast floors, no weather on the HUD (row 9) |
| **The layout** | **World of Tanks 1:1 as the base, plus our own elements** — GDD §15.1 taken literally: minimap bottom-right, team lists at the sides, damage panel bottom-left, ammunition bottom-centre, timer and score top-centre; default keys WoT 1:1 (Shift sniper, T target, Z commands, M minimap, R/F cruise control, Space handbrake, Ctrl free cursor, left button fire). Muscle memory is a retention feature. Our own: the hit log with penetration against effective armour, the visibility budget, the sixth-sense lamp, the reload arc |
| **T is not an aim assist** | T marks the hull under the reticle as THE target (full marker, the team's „attack this" ping) and never lays the gun. „ŻADNEGO aim-assistu" (the owner, 2026-09-02) stands above GDD §15.1's „T cel" |
| **The order** | Foundation (F) → battle HUD (H) → product shell (P) → garage (G). The garage was rebuilt three times and stands; the battle is where the player lives |
| **The toolkit** | `ui_kit`, one draw call, one pipeline, no egui (Inny Poziom; `RENDER_SURFACE` keeps the empty `egui` row). Depth, material and glass come from SDF-shaped plates, one procedural material sheet and SDF glyphs, all in the same pass. `HudVertex` grows by APPENDING lanes. The HUD gets `PassId::Hud` and a measured budget: 0.5 ms (GDD §9) is the TARGET, the FLOOR is the first MX330 record |
| **The blend state stays straight alpha** (F2, 2026-09-05) | The plan had premultiplied blending land with the new vertex, for an additive lamp glow. It would have moved every look golden by rounding for a style nothing drew yet, in the PR whose whole proof is „no pixel moved". So F2 keeps `ALPHA_BLENDING`; the glass band lightens under straight alpha; a true additive lamp, if the H wave wants one, lands with its own bless and its own reason |
| **The fonts** | A pair under the SIL Open Font License: **Big Shoulders Stencil Display** (labels, weights 500 and 900) and **IBM Plex Sans Condensed** (values and text, 400/600/700). Latin Extended-A baked; an unknown glyph renders a visible tofu box and never skips; a `LICENSE.md` beside each file in the shape of `assets/flora/bark/*/LICENSE.md`; an FNV-1a hash lock on the embedded bytes. Tabular digits are a fixed digit cell in the layout engine — `ab_glyph` does no OpenType shaping, so the font cannot be trusted with it |
| **The reticle stack is untouched** | A1–A12, `crates/apps/client/src/hud/reticle*.rs`, `scope_overlay.rs`, `spot_bracket.rs`, the `reticle_strip` probe: carried verbatim through the new draw list as a legacy payload; their vertex locks are HELD by the ratchet, not burned. The one deliberate exception is H27, and it is its own PR |
| **The reticle is simple** | The owner, 2026-09-05 evening: „celownik powinien być prosty, jak najmniej rozpraszaczy i zbędnych dla gracza informacji, jakiś dodatkowych kółeczek celowników, jedynie istotne i ważne rzeczy". Every mark on the reticle answers one of four questions the player asks in the second they look at it — where will I hit, will it penetrate, may I fire, how long until I may — or it goes. No extra rings. The World of Tanks base is the LAYOUT of the screen, never a licence to add a mark because WoT has one. H27 audits today's stack against this rule |
| **Numbers are quiet** | The owner, the same evening: „liczby z dmg po trafieniu wroga — to też nie może być tak natrętnie kolorowo, przesadnie". A floating number is one muted tone per outcome family, small, brief, never animated in scale, never stacked; the world's own flash carries the event (S7). H9 lands under this rule |
| **Honesty** | The HUD never invents. An unseen enemy exists on the client only through a roster MANIFEST (vehicle, team, seat, crew kind) that withholds positions; aggregates such as the team HP pool come from the server; a kill between two unseen hulls arrives as an event without a position; the observer who spotted you is named only after the battle; the replay button is disabled and says why until the viewer exists (L3) |
| **Names** | There is no identity on the wire (the protocol carries no `String`). Every hull is „vehicle · seat": `T-54 · C`, the seat a letter in team order from the roster. When Steam identity lands, the seat column becomes the nickname column and nothing else moves |
| **The number and the word** | GDD §15.1's floating damage numbers stay, SUBORDINATE to S7: colour by OUTCOME family (pen / held / module / fire), never by shell type; a zero-damage outcome is the word (A6); the number's ink stays under twice the penetration flash |
| **Sixth sense** | Lit for the whole spotted span — the own mask is live at 20 Hz and hiding the unspot would be a lie the data does not tell; a chime once per span. V4's server-side delay is inherited when it lands, not re-modelled here |
| **Death** | The intel half of the HUD (top bar, team lists, minimap, feed) stays at 0.7 alpha with a spectate strip; the allies' panels come from the wire and never their aim; vitals, reticle and ammunition go. The destroyed hull is LOCKED in the garage until `BattleEnded` and says so |
| **Measured, not described** | Every HUD state is a byte-exact golden (`crates/apps/client/tests/goldens/hud/`, eleven states in two size classes); no battle text under 16 px at 1080p and no acted-on number under 24 px; ink over glass at 3:1 or better, measured on the golden; a zero-modal lock; the HUD pass budget; one 1080p frame signed by the owner |
| **The look pass** (2026-09-06, evening) | The frames of the finished program were reviewed and read flat: every plate one olive fill, the brushed and the painted steel one plate, no rim, the legend a dim noise, the tree's nation words sunk into the panel, the inspector's marker lost under the plate. So: the sheet's amplitudes raised (brushed lines 0.14 → 0.22, paint blotch 0.10 → 0.14), the bevel three units at 0.55, the glass band 0.26, the brushed steel cooler and the painted more olive at the same luminance (the contrast locks unmoved), the legend and the chips at 18 u in the label's ink, a nation swatch at each tree line's head, a ringed 22 cm marker. Every HUD and garage golden re-recorded once for it |
| **Ownership of files** | During the H wave `crates/apps/client/src/hud/**` belongs to this program; a lane that needs a HUD element goes through the draw list, and the ratchet refuses a new call to the old primitives outside the reticle files. Files owned by other lanes on 2026-09-05 — `app/lifecycle.rs`, `app/input*.rs`, `app/loop_step.rs`, `renderer_*` (the window lane's audit of 2026-09-05), `crates/vehicle/**` (Forge 2.0) — are consumed, not edited: F4 reads the DPI scale the window lane hands it, P7 lands after that lane's queue |

---

# Part I — The design

## 1. The look: steel, enamel and instrument glass

A tank's instruments are stamped steel plates with enamel dials under glass, lit by a small warm
lamp, stencilled by hand and worn by gloves. That is the whole reference. The interface is built
of four materials and three lights.

**Materials** — a 512 × 512 sheet of sixteen 128 px tiles, generated procedurally like the icons
(deterministic, hash-locked, no art asset and no licence question): brushed steel (anisotropic line
noise), painted steel (low-frequency blotch and scratches), enamel black, a worn-stencil mask, a
glass reflection band, a rivet, and a 256 × 256 region reserved for the baked minimap relief.

**Depth** — a plate is a rounded or chamfered box evaluated as a signed distance in the fragment
shader from the vertex's local coordinate, with a 2 u bevel lit from the top-left and a 1 u
hairline. A pressed control is the same plate inset (the bevel's sign flipped, albedo × 0.85, its
label shifted 1 u down-right). Nothing about the depth is geometry, so it scales with DPI for free.

**Glass** — a readout that changes in battle (HP, reload, timer, ammunition count) sits under a
glass tint with an additive reflection band; the band drifts 1–2 u with the camera's yaw so the
glass reads as glass and never as a gradient.

**Lamp** — one warm tungsten colour, red > green > blue, applied additively to live values and to
the focus ring. It is the only glow in the interface, and it is warmer than the text.

**Palette** — the semantic block is data, not constants, and swaps whole:

| Role | Standard | Note |
|---|---|---|
| team self / ally / enemy | lamp-white / steel blue / signal red | ally blips are round, enemy blips are diamonds — shape carries the pair too |
| ammunition AP / APCR / HEAT / HE | brass / white-brass / violet-grey / olive | indexed by `ShellType` order (append-only) |
| module ok / damaged / destroyed | green enamel / amber / red | the three states of the damage panel and the ears |
| verdict pen / held / ricochet / shatter | green / red / white / grey | pen ▲, held ▬ — shape again |
| hp ramp | green → amber → red | the same ramp on every bar |
| commit | one red | worn by BATTLE and EXIT alone (U10) |

Three colour-blind palettes (deuteranopia, protanopia, tritanopia) are the same block with other
values; the lock simulates each with the Machado 2009 matrices and refuses a palette in which any
semantic pair differs by hue alone.

**Typography** — Big Shoulders Stencil Display for labels, headers and banners (the paint through
the stencil); IBM Plex Sans Condensed for values and text (the engineer's lettering). Sizes in `u`,
where 1 u is 1 px at 1080p times the user's UI scale: caption 11, label 14, value 18, display 40.
Every digit occupies one fixed cell per face and size, so a counter never jitters. Every string
that reaches the atlas is measured, wrapped or ellipsised by the layout engine, never clipped by
accident. A glyph the atlas does not carry renders a box; it never disappears.

**Motion** — none on the reticle (reconciliation row 6: the picture under the player's hand never
moves); fades of at most 150 ms elsewhere; the hit-direction arcs and the floating numbers keep
their S5/S7 timings.

**Sound** — the existing `UiClick` (with its accent for commit), `UiReject`, and a new
`SixthSense` chime. Nothing else speaks.

## 2. The battle HUD

### 2.1 The map

The World of Tanks base, 16:9, with our own elements in their WoT-shaped places:

```
+------------------------------------------------------------------------------+
| 60 FPS · 45 ms          ally 4 |||||||||||||||||......... 6 enemy            |
| kill feed (3 rows)                 12:31   (o) sixth sense                   |
|                                 SEEN FROM 308 m · MOVING                     |
| ALLIES                                                              ENEMIES  |
|  o T-54 · A  ====                                               ====  Tiger·A|
|  o IS-3 · B  ==                     hit-direction ring          ---- unseen  |
|  x T-34 · C  dead                 ( reload arc + reticle )      x     dead   |
|                                                                              |
|                              hit log under the reticle:                      |
|                              > BR-412D  201 > 162 mm @ 38  TURRET FRONT  PEN |
|                              < Pzgr.39  194 > 231 mm @ 61  HULL SIDE   HELD |
|                                                                              |
| +- damage panel -+                                          +-- minimap ---+ |
| | silhouette     |  42 KM/H  ^^    [ FIRE ]                 | A B C D E .. | |
| | 6 modules      |                                          | 1 grid, ring | |
| | 5 crew, repair |     [1 AP 24] [2 APCR 10] [3 HE 6]        | blips, ghost | |
| | HP 1240/1800   |                                          | ping         | |
| +----------------+                                          +--------------+ |
+------------------------------------------------------------------------------+
```

| Element | Region · size class | Data today | Keys | Row |
|---|---|---|---|---|
| Top bar: timer, frag counter, team HP pool | top centre · value / display | timer present; frags by inference (wrecks always ride the snapshot); the HP pool is OWED (W-2); the denominator is the roster (W-1) | — | H1 |
| Team lists („ears") | left and right edges under the top bar · label | OWED — a roster manifest (W-1); HP for allies exact, for spotted enemies quantized, for unseen enemies none | Ctrl-click a row when dead: spectate | H2 |
| Kill feed | top-left under the FPS/RTT line · label | OWED — a public kill event (W-3) | — | H3 |
| Damage panel: silhouette, six modules, five crew, repair clocks, fires, HP | bottom-left · value | modules, tracks, crew, fires present; repair clocks OWED (W-4) | — | H4, H17 |
| Speed and cruise control | bottom-left, beside the panel · value | speed present; cruise is client-side (throttle is already an axis) | R / F | H5 |
| Ammunition: designation, penetration, damage, count, switching | bottom centre · value | present locally in `ShellSpec`, unmodelled | 1 / 2 / 3 | H6 |
| Reload arc with an honest denominator | around the reticle (unchanged geometry) | present; the denominator is wrong today | — | H7 |
| Hit log: round, pen › effective @ angle, zone, word, damage | under the reticle, right-aligned · label | present on the wire, dropped by the model; distance of taken hits OWED (W-6) | N collapses | H8 |
| Floating numbers | over the hit point | present (S7) | — | H9 |
| Markers: full for the target, dimmed for the rest | over visible hulls | present (spotting bit) | T marks | H10, H11 |
| Hit direction | ring at 0.30 (unchanged) | present (S5) | — | H12 |
| Sixth-sense lamp | under the timer · display | present and unread | — | H13 |
| Visibility budget line | under the lamp · label | computable from the roster (W-1) and the sim's own factors | — | H14 |
| Minimap: grid, circles, turret yaw, identities, ghosts, pings, three sizes | bottom-right | relief/cover/blips present; identities W-1, pings W-5 | M size, Ctrl-click ping | H15 |
| Command wheel | radial around the cursor | OWED (W-5) | Z | H16 |
| FPS · RTT · snapshot age | top-left · label | present, only logged | — | H18 |
| Spectate strip | bottom centre, dead only | present (allies ride the snapshot) | ← → | H19 |
| Outcome banner → results | centre | present | Enter | H20 |
| HUD editor, presets, palettes | pause menu | client-side | Ctrl+R reset | H21, H22 |

### 2.2 The states

The HUD golden instrument (F8) renders one frozen battlefield frame — the `prokhorovka_sniper_contact`
scene, a T-54 at 300 m, backlit — and draws the HUD over it in named states, at 960 × 540 and at a
1.5× size class, byte-exact:

`third_person_idle`, `sniper_aiming_hull`, `reloading`, `hit_taken`, `module_destroyed`,
`on_fire`, `spotted`, `kill_confirmed`, `outcome_banner`, `pause_menu`, `hud_editor_open` at the
first bless; `team_lists_mixed`, `kill_feed_and_numbers`, `command_wheel_open`,
`dead_spectating`, `preset_minimal`, `preset_full`, `palette_deuteranopia` as the H rows land.

A state is appended, never renamed; `every_hud_state_is_under_an_image_lock` refuses a state
without a golden.

### 2.3 The rules

- **Two seconds to read** (GDD §15.2) is two floors, not a sentence: no battle string under 16 px
  at 1080p (0.030 clip), no acted-on number — HP, reload, ammunition, timer, fuze — under 24 px; ink
  over its plate or glass at a luminance ratio of 3:1 or better, measured on the rendered golden.
  One 1080p frame is signed by the owner and its bless PR says so.
- **Zero modals in battle.** The modal layer may hold the escape menu and nothing else; a toast,
  a confirmation or a tip in battle fails the lock, not a review.
- **No other players' statistics in battle** (GDD §15.1). A team-list row carries the vehicle,
  the seat, the HP bar and the state, and nothing about the player. Win rates, damage and kills
  never enter the ears.
- **Three presets** — minimal (reticle stack, HP, ammunition, small minimap, lamp), standard
  (everything but the budget line and the RTT lamp), full — and an editor that moves and scales
  every element but the reticle, persisted beside the garage file.
- **The reticle stack is not movable and not restyled.** The dispersion ring, the gun marker,
  the verdict, the scope frame and the readouts are A1–A12's; this program draws around them.
- **The reticle is simple.** Four questions — where will I hit, will it penetrate, may I fire,
  how long until I may — and one mark per answer. A mark that answers none of them is a
  distraction and goes, whatever lane built it (H27). No extra rings, ever.
- **Numbers are quiet.** One muted tone per outcome family, small, brief, no scale animation,
  no stacking. The flash in the world is the event; the number is its caption (H9).

## 3. The shell

- **Results** (P1, P2): a full-screen plate over the frozen last frame — the outcome word, the
  map, the weather, the duration, the vehicle; the player's own numbers (damage, hits,
  penetrations, bounces taken, kills, spots); a TEAM tab that is the roster with alive/dead and no
  statistics; the **timeline** — one time axis with every own shot (round, target, distance,
  pen › effective @ angle, zone, word, damage), every hit taken (attacker if known, else
  „unseen"), every SPOTTED span with its observer named from `BattleEnded.spotting_log` (W-7) and
  only from there, every kill, the death, every module lost or repaired. „Przez krzak o gęstości
  0,4" appears the day V1 puts density on the event. A REPLAY button, disabled, with the reason.
- **The ledger** (P3): every record on the client is backed by an event id from the reliable
  combat lane, `shots_fired` or `BattleEnded`; nothing is synthesised.
- **Battle log and history** (P4, P5): the same timeline widget over a stored battle; one JSON per
  battle under `battles/` beside `garage.json`, on that file's pattern (version, tolerant load,
  backup, atomic write).
- **Settings** (P6): master gain, mouse sensitivity per zoom step, UI scale, HUD preset, palette,
  daylight override (migrated out of `garage.json`), Shift as hold or toggle, borderless
  fullscreen (the F11 toggle the window lane shipped in PR #710, persisted).
- **Keybinds** (P7): an `Action` table with contexts (battle, garage, global) replaces the
  hard-coded matches; a rebinding screen with conflict detection; `keybinds.json`.
- **Escape** (P8): always a way out — the battle menu grows STAY · SETTINGS · KEYBINDS · HUD
  EDITOR · EXIT TO GARAGE; a cold garage offers SETTINGS · KEYBINDS · QUIT; every screen's Escape
  closes one layer, and QUIT or the battle is at most three presses away.

## 4. The garage

Tabs across the top bar: GARAGE · TECH TREE · ARMOUR · BATTLES · REPLAYS · STATISTICS · SETTINGS.

- **Stats** (G1, G2): every row labelled, a real number with its unit, a bar against the roster's
  stock minimum and maximum with the class median ticked; three derived rows from GDD §15.4 —
  effective front at 0° through the one penetration resolver, power per tonne, dispersion on the
  move through the sim's own bloom.
- **Compare** (G3): two hulls side by side with signed deltas.
- **Nameplate** (G4): tier · class · nation and one role line per vehicle.
- **Ammunition** (G5): designation, penetration at 100 m, damage, count, and one line on the rack.
- **Controls** (G6, G7): idle / hover / pressed / disabled on everything clickable, tooltips, a
  hint strip printing the keys the binding table actually holds; commit red on BATTLE alone.
- **The hero** (G8): click a module on the tank to open its slot; drag the turret.
- **Filters** (G9): class, nation, tier chips above the carousel.
- **Armour inspector** (G11): point mode — the cursor's ray through `resolve_traced_impact` with
  the selected round, a distance slider and the hull's attitude gives NOMINAL · EFFECTIVE · ANGLE
  · ZONE · PEN/HELD; „shoot me" mode — round, distance and hull yaw colour the whole hull by the
  penetration map. The inspector equals the shell on a thousand points, or it does not ship.
- **Tech tree** (G12): horizontal, columns by tier until the class bands land (an R-lane row),
  rows by class, edges along each line, a node that says what it is and what follows it — and
  nothing it cannot know: no XP, no research state, no locks (GDD §15.6's „ile XP brakuje" waits
  for an XP that exists).

## 5. The default key map

Context: B battle, G garage, ★ global. „Today" is the hard-coded arm as of 2026-09-05.

| Action | Default | World of Tanks | Today | Context |
|---|---|---|---|---|
| Drive | W S A D, arrows | same | same | B |
| Handbrake | Space | Space | Ctrl brakes, Space fires | B |
| Fire | left button | left button | left button and Space | B |
| Sniper | Shift (toggle; hold as a setting) | Shift | Shift hold, V toggle (V stays an alias) | B |
| Free look | right button hold, Alt | right button | Alt | B |
| Zoom | wheel | wheel | wheel | B |
| Target mark + attack ping | T | T („attack target") | unbound | B |
| Command wheel | Z | Z | unbound (Z selects ammunition in the garage) | B |
| Minimap size | M | M | unbound (M cycles the map in the garage) | B |
| Cruise control forward / reverse | R / F | R / F | unbound (R repairs in the garage) | B |
| Ammunition | 1 2 3 | 1 2 3 | 1 2 3 in battle, Z X C in the garage | B, G |
| Free cursor | Ctrl hold | Ctrl | — | B |
| Hit log collapse | N | (a mod) | — | B |
| Spectate previous / next | ← → | ← → | — | B, dead |
| Garage | G | — | G | B |
| Fullscreen | F11 | — | F11 (PR #710) | ★ |
| Escape | Esc | Esc | Esc | ★ |
| Vehicle previous / next; battle | ← →; Enter | — | same | G |
| Slot focus / cycle | [ ] / Q E | — | same | G |
| Map / daylight / inspector / repair / tree | M / L / I / R / T | — | same | G |
| HUD editor reset | Ctrl+R | — | — | editor |

The garage's Z/X/C move to 1/2/3 so one hand learns one ammunition row. Cross-context reuse
(M, R, T) is allowed by the binding table's context rule; a conflict inside one context refuses
the save.

## 6. Wire changes

All append-only, `#[serde(default)]` on fields, appended variants on enums; one bump of
`PROTOCOL_VERSION` carries W-1 to W-6 in the H wave's second PR; W-7 rides the same bump if the P
wave starts before the next wire PR, else its own. Replay fixtures are re-pinned once per bump.
**Today's wire: v53** (`crates/runtime/net/src/lib.rs`; v53 is the one program's J4, the dive share of the hull pitch — no interface field). **W-1 to W-6 landed as v51 on
2026-09-05** (the H wave's second PR): `BattleRoster` rides the seat word, `Snapshot.team_hit_points`
and the sparse team-private `Snapshot.repair_clocks`, `CombatEvent::{Kill, TeamCommand}` on the
reliable lane, `ProtocolMessage::TeamCommand` with `net::TeamCommandLimiter` on the server (five
per sixty seconds), `DamageEvent.distance_m` as the shell's travelled path; the session hands the
client `BattleSessionTick::{kills, team_commands}` and `roster()`, kept in `app::battle_intel`
until H1/H2/H3/H16 draw them. Fixtures re-pinned (`*_v51.hex`). **W-7 landed as v52 on 2026-09-06**:
`BattleEnded.spotting_log` per recipient, kept by both hosts (`battle_host::SpottingLog`) from the
observer masks they already compute, closed at the end, empty until then on either host; the ledger
names the observers once ended (`an_observer_is_named_only_after_the_battle`). Fixture
`battle_ended_v52.hex`; every fixture re-pinned (the frame carries the version).

| # | Change | Wave | Why |
|---|---|---|---|
| W-1 | `ProtocolMessage::BattleRoster { session_id, entries: Vec<RosterEntry { tank_id, team, vehicle, seat, crew_kind }> }` after `StartBattle` and on join | H | the snapshot filter strips unseen enemies; a manifest names the field without locating anyone |
| W-2 | `Snapshot.team_hit_points: [u32; 2]` | H | the team HP pool; an aggregate cannot be inverted into a position |
| W-3 | `CombatEvent::Kill { victim, killer: Option<TankId>, cause, occurred_tick }` broadcast to every crew | H | a kill between two unseen hulls never reaches the feed today |
| W-4 | `TankSnapshot.module_repair_s: [f32; 6]`, `track_repair_s: [f32; 2]`, concealed for enemies | H | `CrewRepair` is server-only; the panel's repair clocks need it |
| W-5 | `ProtocolMessage::TeamCommand { … }` client → server, `TeamCommandRelay { … }` server → team; `TeamCommand::{Attack, Help, Reloading, Affirmative, Negative, BackToBase, FollowMe, Ping}`; the rate limit of five per sixty seconds enforced on the server | H | the wheel and the pings need a relay, and a limit that a client cannot mod away |
| W-6 | `DamageEvent.distance_m: f32` | H | the distance of a hit from an unseen attacker is known only to the server |
| W-7 | `BattleEnded.spotting_log: Vec<SpottingRecord { observer, distance_m, from_tick, to_tick }>` per recipient — **landed as v52 (2026-09-06)** | P | „kto cię wykrył i kiedy", delivered after the battle so a live client never holds the observer (`the_spotting_log_names_every_enemy_that_saw_the_crew_and_never_an_ally`) |

No wire is needed for: the sixth sense (the own mask), the visibility budget (the roster and the
sim's own factors), the honest reload (a `game_core` move), cruise control, the target mark, the
HUD editor, the palettes, the RTT readout, the last-known ghosts, the fire lamp, ammunition
penetration and damage, and the ledger apart from W-6 and W-7.

---
