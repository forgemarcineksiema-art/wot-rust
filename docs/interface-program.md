# Interface 2.0 — A True HUD

Approved 2026-09-05. The owner's verdict on the interface, verbatim:

> „Trzeba cały interfejs, HUD przeprojektować, przebudować, stworzyć od nowa, w końcu lepszy i
> prawdziwy interfejs HUD."
>
> „Jak obecnie jest tak płasko, z jednym akcentem, kolorem — no to jest do dupy."
>
> „Przy projektowaniu tego nie ma blokad. Trzeba to zrobić porządnie, aby graczom się podobało,
> było czytelnie i intuicyjnie, a także klimatycznie."

This document has two halves. **Part I is the design** — the target look, the layout, the rules
and the locks that hold them: the bible. **Part II is the register and the campaign** — every
defect between today's screen and Part I, the wave that closes it, and the lock or the frame that
proves it closed. When the register is empty, Part I graduates to `docs/interface-policy.md` and
this file becomes history, the way `docs/art-direction-program.md` describes for the picture.

Where this document and `docs/game-design.md` disagree, the reconciliation table of that file
wins; rows 13–20 (2026-09-05) record the decisions this program was built on.

## Why this program exists

The interface had a register before it had a design. `docs/inny-poziom-program.md` carries lane
**U** (U1–U11), V3 and L1–L3: eleven repairs of a toolkit and a garage, none of which describes
what the screen should look like. On 2026-09-05 none of the eleven was closed and no PR from the
lane had landed. The lane's premise — „in parallel from day one on files no other wave touches" —
was false the week it was written: nine commits from the A, S, G and Z lanes edited
`crates/apps/client/src/hud/` between 2026-09-01 and 2026-09-05, because the HUD is where every
other wave's work becomes visible.

The specification of the target interface exists, but not as a design document: it is chapter 15
of `docs/game-design.md` — a HUD laid out like World of Tanks, team lists, a hit log with
penetration against effective armour, a minimap with view-range circles and pings, a post-battle
timeline, a garage with one button, an armour inspector that answers „how thick, here" — and most
of it is not built. There is no other written home for the interface in the tree: `docs/ui/*` and
the hala-4 register were retired in August; the art-direction policy spent one sentence on the UI
(„instrument, not decoration") and the owner has revoked it.

What the repository ships, measured on 2026-09-05:

- `crates/ui/ui_kit/src/` is 1 097 lines of `push_x(&mut Vec<HudVertex>, …)` in clip space with a
  hand-threaded aspect ratio. There is no rectangle type, no anchor, no padding, no clipping, no
  measured or wrapped text, no notion of DPI (`scale_factor` is read nowhere in `crates/`), no
  hover/press/focus state beyond one white 8 % wash.
- The battle HUD is 7 386 lines in 27 modules; 22 elements sit on hard-coded clip-space floats
  (`crates/apps/client/src/hud.rs:175-264`). Absent entirely: team lists, a kill feed, a top bar
  with the score and the team HP, a command wheel, pings, a HUD editor, a ping readout.
- One font (`assets/fonts/BarlowCondensed-Medium.ttf`) at one weight, baked for ASCII 0x20–0x7E
  (`crates/ui/ui_kit/src/font/bake.rs:42`); an unknown glyph is skipped in silence
  (`crates/ui/ui_kit/src/font/layout.rs:66-68`); `crates/apps/client/src/ui_strings.rs` forbids a
  Polish letter.
- The battle HUD has no picture lock of any kind: `crates/apps/client/tests/goldens/look/` holds
  eight garage frames and one sniper frame rendered WITHOUT a HUD. The HUD's locks are 105 unit
  tests over vertex colours, and the garage's 149 more — the real cost of any redesign (U2).
- The HUD's cost has never been measured: it is one draw call inside the FXAA pass
  (`crates/render/renderer_wgpu/src/scene_renderer/draw.rs:345-350`), there is no `PassId::Hud`,
  and `crates/apps/client/examples/probe/perf_capture.rs` never uploads a HUD. The design document
  gives it 0.5 ms; nothing checks.
- The default keys the design document names — T, Z, M, R, F — are unbound; the key map is a
  hard-coded `match` (`crates/apps/client/src/app/input.rs:92-131`) with no table, no rebinding
  and no persistence.

Three dishonesties found on the way in, which the register carries as rows because the honesty
doctrine ranks them above the look:

- The reload arc divides by the stock reload (`crates/apps/client/src/app/prediction.rs:98-101`)
  while the server reloads through the wounded breech and the loader
  (`crates/runtime/sim/src/tank_state.rs:151-159`). With a damaged gun the arc lies (H7).
- The hit log throws away the penetration, the effective armour, the angle, the zone and the
  outcome word that `game_core::DamageEvent` already carries on the wire (H8).
- The player's own `spotted_by_teams_mask` reaches the client unconcealed and nothing reads it:
  the sixth sense is a HUD element away (H13).

## The decisions this program is built on

Taken 2026-09-05 with the owner, unless dated otherwise. Each is a commitment, not a default.

| Decision | Choice |
|---|---|
| **The look** | **Steel, enamel and instrument glass.** Panels are physical 1940–50s instrument plates with depth — bevel, inset, shadow, brushed or painted steel, worn stencil — glass over live readouts with a soft reflection band, warm tungsten lamp light on live values, and a FULL semantic palette: team self/ally/enemy, ammunition by type, module ok/damaged/destroyed, verdict pen/held/ricochet/shatter, an HP ramp, one commit red. This revokes GDD §10 „ciemny, płaski, jeden kolor akcentu, zero gradientów" and the policy's „instrument, not decoration" (reconciliation row 13). Kept from the old look: tabular digits, contrast floors, no weather on the HUD (row 9) |
| **The layout** | **World of Tanks 1:1 as the base, plus our own elements** — GDD §15.1 taken literally: minimap bottom-right, team lists at the sides, damage panel bottom-left, ammunition bottom-centre, timer and score top-centre; default keys WoT 1:1 (Shift sniper, T target, Z commands, M minimap, R/F cruise control, Space handbrake, Ctrl free cursor, left button fire). Muscle memory is a retention feature. Our own: the hit log with penetration against effective armour, the visibility budget, the sixth-sense lamp, the reload arc |
| **T is not an aim assist** | T marks the hull under the reticle as THE target (full marker, the team's „attack this" ping) and never lays the gun. „ŻADNEGO aim-assistu" (the owner, 2026-09-02) stands above GDD §15.1's „T cel" |
| **The order** | Foundation (F) → battle HUD (H) → product shell (P) → garage (G). The garage was rebuilt three times and stands; the battle is where the player lives |
| **The toolkit** | `ui_kit`, one draw call, one pipeline, no egui (Inny Poziom; `RENDER_SURFACE` keeps the empty `egui` row). Depth, material and glass come from SDF-shaped plates, one procedural material sheet and SDF glyphs, all in the same pass. `HudVertex` grows by APPENDING lanes. The HUD gets `PassId::Hud` and a measured budget: 0.5 ms (GDD §9) is the TARGET, the FLOOR is the first MX330 record |
| **The fonts** | A pair under the SIL Open Font License: **Big Shoulders Stencil Display** (labels, weights 500 and 900) and **IBM Plex Sans Condensed** (values and text, 400/600/700). Latin Extended-A baked; an unknown glyph renders a visible tofu box and never skips; a `LICENSE.md` beside each file in the shape of `assets/flora/bark/*/LICENSE.md`; an FNV-1a hash lock on the embedded bytes. Tabular digits are a fixed digit cell in the layout engine — `ab_glyph` does no OpenType shaping, so the font cannot be trusted with it |
| **The reticle stack is untouched** | A1–A12, `crates/apps/client/src/hud/reticle*.rs`, `scope_overlay.rs`, `spot_bracket.rs`, the `reticle_strip` probe: carried verbatim through the new draw list as a legacy payload; their vertex locks are HELD by the ratchet, not burned |
| **Honesty** | The HUD never invents. An unseen enemy exists on the client only through a roster MANIFEST (vehicle, team, seat, crew kind) that withholds positions; aggregates such as the team HP pool come from the server; a kill between two unseen hulls arrives as an event without a position; the observer who spotted you is named only after the battle; the replay button is disabled and says why until the viewer exists (L3) |
| **Names** | There is no identity on the wire (the protocol carries no `String`). Every hull is „vehicle · seat": `T-54 · C`, the seat a letter in team order from the roster. When Steam identity lands, the seat column becomes the nickname column and nothing else moves |
| **The number and the word** | GDD §15.1's floating damage numbers stay, SUBORDINATE to S7: colour by OUTCOME family (pen / held / module / fire), never by shell type; a zero-damage outcome is the word (A6); the number's ink stays under twice the penetration flash |
| **Sixth sense** | Lit for the whole spotted span — the own mask is live at 20 Hz and hiding the unspot would be a lie the data does not tell; a chime once per span. V4's server-side delay is inherited when it lands, not re-modelled here |
| **Death** | The intel half of the HUD (top bar, team lists, minimap, feed) stays at 0.7 alpha with a spectate strip; the allies' panels come from the wire and never their aim; vitals, reticle and ammunition go. The destroyed hull is LOCKED in the garage until `BattleEnded` and says so |
| **Measured, not described** | Every HUD state is a byte-exact golden (`crates/apps/client/tests/goldens/hud/`, eleven states in two size classes); no battle text under 16 px at 1080p and no acted-on number under 24 px; ink over glass at 3:1 or better, measured on the golden; a zero-modal lock; the HUD pass budget; one 1080p frame signed by the owner |
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
**Today's wire: v50** (`crates/runtime/net/src/lib.rs`).

| # | Change | Wave | Why |
|---|---|---|---|
| W-1 | `ProtocolMessage::BattleRoster { session_id, entries: Vec<RosterEntry { tank_id, team, vehicle, seat, crew_kind }> }` after `StartBattle` and on join | H | the snapshot filter strips unseen enemies; a manifest names the field without locating anyone |
| W-2 | `Snapshot.team_hit_points: [u32; 2]` | H | the team HP pool; an aggregate cannot be inverted into a position |
| W-3 | `CombatEvent::Kill { victim, killer: Option<TankId>, cause, occurred_tick }` broadcast to every crew | H | a kill between two unseen hulls never reaches the feed today |
| W-4 | `TankSnapshot.module_repair_s: [f32; 6]`, `track_repair_s: [f32; 2]`, concealed for enemies | H | `CrewRepair` is server-only; the panel's repair clocks need it |
| W-5 | `ProtocolMessage::TeamCommand { … }` client → server, `TeamCommandRelay { … }` server → team; `TeamCommand::{Attack, Help, Reloading, Affirmative, Negative, BackToBase, FollowMe, Ping}`; the rate limit of five per sixty seconds enforced on the server | H | the wheel and the pings need a relay, and a limit that a client cannot mod away |
| W-6 | `DamageEvent.distance_m: f32` | H | the distance of a hit from an unseen attacker is known only to the server |
| W-7 | `BattleEnded.spotting_log: Vec<SpottingRecord { observer, distance_m, from_tick, to_tick }>` per recipient | P | „kto cię wykrył i kiedy", delivered after the battle so a live client never holds the observer |

No wire is needed for: the sixth sense (the own mask), the visibility budget (the roster and the
sim's own factors), the honest reload (a `game_core` move), cruise control, the target mark, the
HUD editor, the palettes, the RTT readout, the last-known ghosts, the fire lamp, ammunition
penetration and damage, and the ledger apart from W-6 and W-7.

---

# Part II — The register

Columns: the defect, the evidence, the wave, and what closes it. IDs by wave: **F** foundation,
**H** battle HUD, **P** product shell, **G** garage. A row closes with a number or a frame, never
with a sentence; a closed row is struck through and annotated, never deleted; IDs are never
reused. Rows that absorb a row of `docs/inny-poziom-program.md` say so, and that row points here.

Numbers under lock in this document: **stat rows today: 9**
(`crates/apps/client/src/app/garage/layout.rs`), **icons baked today: 21**
(`crates/ui/ui_kit/src/icons.rs`); the wire version above.

## F — foundation

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| F1 | **The HUD's cost has never been measured.** One draw call inside the FXAA pass; no `PassId`; the frame instrument never uploads a HUD | `crates/render/renderer_wgpu/src/scene_renderer/draw.rs:345-350`, `crates/render/renderer_wgpu/src/frame_graph.rs:20-45`, `crates/apps/client/examples/probe/perf_capture.rs` | F | `PassId::Hud` appended (label `hud_pass`, a `LdrAntialiased` resource between FXAA and the HUD, condition Always so the pass list stays static); `perf_capture` gains the row „full + 7v7 + HUD"; lock `the_full_hud_pass_stays_under_its_floor_on_the_min_spec` — FLOOR = the first MX330 record × 1.15, TARGET 0.5 ms printed as HUD DEBT; asserts on the MX330, prints elsewhere |
| F2 | **The HUD vertex can only say „solid or coverage".** No local coordinate, no material, no glass; straight-alpha blending cannot glow | `crates/render/renderer_api/src/scene.rs:277-300`, `crates/render/renderer_wgpu/src/shaders/hud.wgsl`, `crates/render/renderer_wgpu/src/scene_pipeline.rs:55-98` | F | `HudVertex` grows by appending `local`, `extent`, `params` and `style` (64 bytes; the old constructors zero the new lanes); `hud_style::{SOLID, GLYPH, PLATE, SHEET, GLASS}` pinned at both ends of the shader boundary; premultiplied blending; the material sheet bound beside the atlas; the vertex buffer's byte capacity doubled so the count stays 16 384 and `hud_budget.rs` holds; locks `the_hud_vertex_grew_by_appending`, `hud_style_values_are_bound_at_both_ends`, `the_legacy_constructors_leave_the_new_lanes_at_zero`, `the_material_sheet_is_deterministic`; the garage goldens re-blessed with the rounding drift stated |
| F3 | **One font, one weight, ASCII only, unknown glyphs skipped, no licence or hash lock on the file** | `crates/ui/ui_kit/src/font/bake.rs:13-22`, `crates/ui/ui_kit/src/font/layout.rs:66-68`, `crates/apps/client/src/ui_strings.rs:78-116` (26 of 28 constants listed) | F | the pair embedded from `assets/fonts/<family>/` with a `LICENSE.md` each, Barlow removed with its last reader; a font manifest with FNV-1a hashes; an SDF atlas covering ASCII, Latin-1 letters and Latin Extended-A, plus a tofu glyph; `ui_strings.rs` lists every constant in an `ALL` per module; locks `every_embedded_font_matches_its_manifest_hash`, `every_font_has_its_licence_beside_it`, `the_pair_covers_latin_extended_a`, `an_unknown_glyph_renders_tofu_never_skips`, `every_ui_string_is_covered_by_both_faces`, `the_atlas_is_a_signed_distance_field` (absorbs U3) |
| F4 | **`ui_kit` has no rectangle, anchor, padding, clipping, DPI, measured text or digit cell**; the window's `ScaleFactorChanged` is unhandled | `crates/ui/ui_kit/src/primitives.rs`, `crates/ui/ui_kit/src/font/layout.rs`, `crates/apps/client/src/app/lifecycle.rs` | F | a `Ui` context (viewport in px, DPI, user scale, a clip stack), `Rect` in px, the unit `u`, nine anchors, row and column builders, `measure_text` with wrapping and ellipsis, a tabular digit cell, CPU-side quad clipping so the draw stays one call; the scale comes from the window lane, `1.0` until it hands one over; locks `an_anchor_at_every_corner_stays_inside_the_viewport`, `a_row_lays_children_left_to_right_with_its_gap`, `a_column_with_padding_never_overlaps_its_children`, `a_clipped_glyph_keeps_its_texel_density`, `wrapped_text_never_exceeds_its_width`, `ellipsis_replaces_the_tail_not_the_head`, `one_u_is_one_px_at_1080p_and_scales_with_the_user`, `tabular_digits_share_one_cell` (absorbs the layout half of U1) |
| F5 | **No semantic draw list.** Tests assert vertex colours; the garage hit test re-reads the constants the panels draw from | `crates/apps/client/src/hud.rs:175-264`, `crates/apps/client/src/app/garage/overlay.rs:128-210`, `crates/apps/client/src/app/garage/layout.rs`, `crates/apps/client/src/hud/reticle_overlay_tests.rs` | F | `DrawList<K>` of elements (id, rect, z, clip, state, payload) with one emitter and one `hit()`; `HudElement` and `GarageElement` enums; the reticle stack carried as a legacy payload verbatim; the ratchet `vertex_colour_equality_assertions_only_go_down` seeded per file at F5 (reticle files HELD, every other file burns to zero as its element migrates) and refusing new calls to the old primitives outside the reticle files; locks `the_draw_list_emits_the_legacy_hud_byte_for_byte`, `the_reticle_stack_is_emitted_verbatim` (absorbs U2) |
| F6 | **The theme is nineteen flat constants around one amber accent**: no plate materials, no bevel, no glass, no semantic palette, no size classes, no colour-blind variant (GDD §15.2) | `crates/ui/ui_kit/src/theme.rs` | F | a `Theme` value: plates, bevel, inset, hairline, glass alpha, lamp, text tokens, size classes, the `Semantic` block and `Palette::{Standard, Deuteranopia, Protanopia, Tritanopia}`; the old colour constants kept as aliases while a legacy builder lives; locks `every_label_token_clears_contrast_on_every_plate` (4.5:1 text, 3:1 icons), `every_palette_keeps_the_semantic_pairs_apart`, `the_lamp_is_warmer_than_the_text`, `size_classes_are_ordered` (absorbs the theme half of U1) |
| F7 | **One 8 % hover wash is the whole interaction model**; no pressed, focused, disabled or tooltip state; the cursor is ignored in battle | `crates/apps/client/src/app/garage/overlay.rs:57-59`, `crates/apps/client/src/app/input.rs`, `crates/apps/client/src/hud/pause_menu.rs` | F | an `Interaction<K>` machine (hover, press, release, keyboard focus, tooltip delay) rendered by the emitter — inset PRESSED, a lamp focus ring, disabled at 45 %; the cursor tracked in px in every mode; locks `every_clickable_has_three_states`, `a_click_is_a_release_on_the_element_that_was_pressed`, `tab_walks_focus_in_layout_order_and_skips_disabled`, `a_tooltip_waits_its_delay_and_dies_with_the_hover`, `the_cursor_is_tracked_in_battle` (absorbs the interaction half of U1) |
| F8 | **No battle-HUD golden of any kind.** The only scoped frame is a scene through a lens without a HUD | `crates/apps/client/src/look_harness.rs`, `crates/apps/client/tests/goldens/look/prokhorovka_sniper_contact.png` | F | `HudState::ALL` × two size classes rendered over one frozen frame, byte-exact under `crates/apps/client/tests/goldens/hud/`, `WOT_UPDATE_GOLDENS=hud` as its own scope; a footprint floor and a near-white ceiling; the probe `hud_states` writing 1080p frames and a per-element vertex census; lock `every_hud_state_is_under_an_image_lock` |
| F9 | **The 16 384-vertex cap is a guess against an un-itemised HUD.** The minimap relief alone is 7 776 vertices | `crates/render/renderer_wgpu/src/scene_renderer.rs:38-42`, `crates/apps/client/src/hud/minimap.rs` | F | the census of F8; lock `the_full_hud_state_fits_the_buffer_with_headroom` (at most 14 000 on the busiest state); the relief baked into the sheet as one quad is the H wave's first row (H0) |
| F10 | **The written record still says egui and glyphon (§15.8), „dark, flat, one accent" (§10) and „instrument, not decoration"** — all overruled on 2026-09-05, none dated in a table | `docs/game-design.md`, `docs/art-direction-policy.md` | F0 | reconciliation rows 13–20 dated; the policy sentence replaced; `docs/inny-poziom-program.md` points its U, V3, L1 and L2 rows here; this document under the `quality` locks `every_evidence_path_in_the_interface_program_exists` and `every_register_id_in_the_interface_program_is_unique` and the number claims of `roadmap_claims.rs` |

## H — battle HUD

Every row lands with its `hud_states` golden. Evidence is the client as of 2026-09-05.

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| H0 | The minimap relief is 1 296 quads a frame | `crates/apps/client/src/hud/minimap.rs` | H (first) | the relief baked once per battlefield into the sheet's reserved region and drawn as one quad; the census drops by ~7 700 |
| H1 | **No top bar**: no frag counter, no team HP pool; the timer floats alone | `crates/apps/client/src/hud/readouts.rs`, `crates/apps/client/src/hud.rs` | H | W-1, W-2; `the_top_bar_counts_wrecks_it_can_see_and_sums_hp_it_was_told`; `team_lists_mixed` |
| H2 | **No team lists**, and unseen enemies cannot be represented — the filter strips them | `crates/runtime/net/src/snapshot_filter.rs:130-146` | H | W-1; three class icons appended to `HudIcon`; `a_team_list_row_carries_vehicle_seat_hp_and_state_and_nothing_about_the_player`; `an_unseen_enemy_row_has_no_bar_and_no_position` (absorbs the team-list half of U4) |
| H3 | **No kill feed**; a kill between two unseen hulls never arrives | `crates/runtime/net/src/snapshot_filter.rs:78-95`, `crates/apps/client/src/hud/kill_marker.rs` | H | W-3; `a_kill_between_two_unseen_hulls_still_reaches_the_feed_without_a_position` |
| H4 | **The damage panel shows four of six modules**, no silhouette, no repair progress; the rack, track and crew callouts are four separate instruments | `crates/apps/client/src/hud/module_panel.rs`, `crates/runtime/sim/src/repair.rs:44-52`, `crates/apps/client/src/hud/track_callout.rs`, `crates/apps/client/src/hud/rack_callout.rs`, `crates/apps/client/src/hud/crew_panel.rs` | H | W-4; the `HitboxProfile` silhouette with six modules, five crew, tracks, fires and repair clocks; a dead radio prints RADIO OUT; the four callout files retired; `the_damage_panel_shows_all_six_modules_and_their_repair_clocks`, `a_dead_radio_says_so`; `spotted_and_burning` |
| H5 | **No cruise control**; R and F unbound | `crates/apps/client/src/app/input.rs:92-131`, `crates/apps/client/src/app/input_state.rs` | H | a cruise level on the input state driving the throttle axis; `cruise_control_latches_the_throttle_until_the_brake` (lands through the window lane's binding work, see P7) |
| H6 | **Ammunition is an icon and a count**; designation, penetration and damage sit unused in `ShellSpec`; the switching penalty is invisible | `crates/apps/client/src/hud/ammo_panel.rs:22-33`, `crates/foundation/game_core/src/weapon.rs`, `crates/runtime/sim/src/state.rs:536-543` | H | `every_ammo_slot_prints_designation_penetration_and_damage`; `a_switch_shows_its_cost_before_the_snapshot_confirms_it` |
| H7 | **The reload arc lies**: the client divides by the stock reload, the server reloads through the wounded breech and the loader | `crates/apps/client/src/app/prediction.rs:98-101`, `crates/runtime/sim/src/tank_state.rs:151-159` | H (first PR) | `full_reload_seconds` moves into `game_core` and both sides call it; `the_reload_arc_denominator_is_the_servers_reload_not_the_stock_one`; the `reticle_strip` frame byte-identical |
| H8 | **The hit log drops penetration, effective armour, angle, zone and the outcome word** that the wire carries; bounces are excluded | `crates/apps/client/src/hud/damage_log.rs:34-49`, `crates/foundation/game_core/src/damage.rs:133-217` | H | W-6; every field printed, bounces join; N collapses; `a_hit_log_row_prints_pen_effective_angle_zone_and_the_outcome_word`, `a_bounce_earns_a_row`; `sniper_aiming_hull` |
| H9 | Floating numbers carry no semantic colour and owe their restyle under S7 | `crates/apps/client/src/hit_indicator.rs` | H | colour by outcome family; the S7 and A6 locks green after the restyle; `kill_feed_and_numbers` |
| H10 | **Every visible hull wears the same marker**; no target distinction, no distance | `crates/apps/client/src/hud/health_bar.rs`, `crates/apps/client/src/hud/spot_bracket.rs` | H | `only_the_target_wears_a_full_marker` |
| H11 | **No target mark** (T) | `crates/apps/client/src/app/input.rs:92-131` | H | the mark follows the hull's snapshot and dies with its visibility; the gun never moves by it; `a_target_mark_follows_the_hull_and_dies_with_its_visibility_and_never_moves_the_gun` |
| H12 | The hit-direction arcs and verdict owe their restyle | `crates/apps/client/src/hud/hit_direction.rs` | H | the S5 locks stay; `hit_taken` |
| H13 | **The sixth sense is unread**: the own mask survives the filter and nothing lights | `crates/runtime/net/src/snapshot_filter.rs`, `crates/apps/client/src/hud.rs:53-98` | H | a lamp and a `SixthSense` audio event; `the_lamp_is_lit_exactly_while_the_own_mask_says_spotted`, `the_chime_plays_once_per_span` (absorbs the HUD half of V4) |
| H14 | **The visibility budget is invisible** | `crates/runtime/sim/src/spotting.rs:47-79` | H | „SEEN FROM … · MOVING / FIRED" from the roster's longest view range and the sim's own factor; `the_budget_line_is_the_enemys_longest_view_range_times_the_sims_own_factor` (absorbs V3; bush and camouflage eaters when V1 and V2 land) |
| H15 | **The minimap has bare blips**: no identity, no turret yaw, no circles, no grid, no ghosts, no pings, one size | `crates/apps/client/src/hud/minimap.rs:45-63`, `crates/apps/client/src/app/minimap_build.rs` | H | W-1, W-5; a 10 × 10 lettered grid, the view-range and seen-from circles (no draw circle — there is no such mechanic), turret yaw, identities, ghosts fading over ten seconds, three sizes on M, Ctrl-click ping; `the_minimap_forgets_a_ghost_in_ten_seconds_and_never_invents_one`, `every_blip_carries_its_class_and_seat` |
| H16 | **No commands, no pings** (Z) | `crates/runtime/net/src/lib.rs` | H | W-5; `a_sixth_command_in_a_minute_is_refused_by_the_server_and_knocked_on_the_client`, `a_ping_lands_on_the_minimap_and_in_the_world_for_the_team_only`; `command_wheel_open` |
| H17 | Engine and fuel fire are on the wire and nowhere on the HUD | `crates/runtime/net/src/lib.rs:197-287` | H | `a_burning_hull_wears_the_lamp_and_a_cooking_rack_counts_down`; `on_fire` |
| H18 | **RTT is only logged** | `crates/apps/client/src/app/session.rs` | H | `the_rtt_readout_prints_the_sessions_number_and_local_says_local` |
| H19 | **Death is a blank screen orbiting the wreck**; no spectate, no lock in the garage | `crates/apps/client/src/app/render.rs:652-659`, `crates/apps/client/src/app/camera_link.rs:89-96` | H | `a_dead_player_keeps_the_intel_hud_and_never_a_reticle`, `spectating_an_ally_shows_their_panel_from_the_wire_and_nothing_about_their_aim`; `dead_spectating` |
| H20 | The outcome banner ends in a hint; there is nothing after it | `crates/apps/client/src/hud/outcome.rs` | H | the hand-off to P1 on Enter or after three seconds; `outcome_banner` |
| H21 | **22 elements on hard-coded floats; no editor, presets or persistence** | `crates/apps/client/src/hud.rs:175-264` | H | a `HudLayout` of placements per element, three presets, `hud_layout.json` on the persistence pattern, the reticle not movable; `every_hud_element_but_the_reticle_is_movable_and_its_placement_survives_a_restart`; `preset_minimal`, `preset_full`, `hud_editor_open` (absorbs the layout half of U4) |
| H22 | **No colour-blind palette** | `crates/ui/ui_kit/src/theme.rs` | H | `no_semantic_pair_differs_by_hue_alone_in_any_palette`; `palette_deuteranopia` |
| H23 | **„Two seconds to read" is a sentence**; strings at 9–12 px ship | `crates/apps/client/src/app/garage/panels/loadout.rs:54-55`, `crates/apps/client/src/app/garage/panels/inspector_legend.rs:22-24` | H | `no_battle_string_renders_below_the_size_floor`, `every_battle_readout_sits_on_glass_at_three_to_one`; the owner-signed 1080p golden |
| H24 | Nothing forbids a popup in battle | `crates/apps/client/src/hud.rs` | H | `nothing_modal_appears_in_battle_but_the_escape_menu` over every state |
| H25 | The reticle stack must survive the wave untouched | `crates/apps/client/src/hud/reticle_overlay_tests.rs` | H | `reticle_strip` byte-identical through every H PR; the reticle files' ratchet ceilings HELD |
| H26 | The HUD's cost has a target and no number | `crates/render/renderer_wgpu/tests/hud_budget.rs` | H (last) | the F1 lock's FLOOR at or under the 0.5 ms TARGET, or the debt recorded per element with its measurement |

## P — product shell

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| P1 | **No results screen** — a banner and a hint | `crates/apps/client/src/hud/outcome.rs`, `crates/apps/client/src/ui_strings.rs` | P | `the_results_screen_prints_only_what_the_ledger_holds`; `shell_results_summary` |
| P2 | **No battle timeline**; observers are never named | `crates/runtime/net/src/lib.rs` | P | W-7; `every_timeline_row_is_backed_by_a_wire_event_id`, `an_observer_is_named_only_after_the_battle`; `shell_results_timeline` |
| P3 | **Nothing accumulates a battle**: the damage log is a six-entry, eight-second pulse | `crates/apps/client/src/hud/damage_log.rs:19-20`, `crates/runtime/net/src/lib.rs:478-492` | P (first) | a `BattleLedger` fed only from the reliable lane, the own `shots_fired`, the own mask transitions and `BattleEnded`; `the_ledger_holds_one_record_per_event_id_and_never_a_synthesised_one`; a ledger rebuilt from a recorded session equals the live one |
| P4 | **No battle log screen** | `docs/inny-poziom-program.md` | P | the timeline widget over a stored battle under a BATTLES tab; `shell_battle_log` (absorbs L2) |
| P5 | **No battle history on disk**; only the garage persists | `crates/apps/client/src/app/garage/persistence.rs` | P | one file per battle plus an index on that file's pattern; `a_battle_is_written_once_at_its_end_and_a_corrupt_file_degrades_to_an_empty_history` |
| P6 | **No settings**: sensitivity is two constants, gain is pinned, UI scale does not exist, the daylight override lives in the garage file | `crates/apps/client/src/app/input.rs`, `crates/apps/client/src/app/garage/persistence.rs`, `crates/apps/client/src/camera/zoom.rs` | P | `settings.json`; `every_setting_round_trips_through_the_file_and_a_missing_file_is_the_defaults`, `sensitivity_is_read_per_zoom_step`; `shell_settings` |
| P7 | **Keys are hard-coded**; no rebinding, no conflict detection, no persistence | `crates/apps/client/src/app/input.rs:92-131`, `crates/apps/client/src/app/garage/actions.rs:175-250` | P (after the window lane's input queue) | an `Action` table with contexts replacing both matches; `keybinds.json`; `every_action_has_a_default_key_and_no_two_share_one_in_a_context`, `the_hard_coded_key_match_is_gone`; `shell_keybinds_conflict` |
| P8 | **Escape offers no exit from a cold garage**; the pause menu has two entries | `crates/apps/client/src/app/garage/mod.rs:255-259`, `crates/apps/client/src/hud/pause_menu.rs:33-38` | P | `escape_always_offers_a_way_out` — from every screen QUIT or the battle in at most three presses (absorbs U5 with P1, P6 and P7) |
| P9 | Borderless fullscreen exists on F11 (PR #710) and is not remembered | `crates/apps/client/src/app/input.rs:41-48` | P | the setting persisted and applied at start; `borderless_is_the_only_fullscreen_and_it_is_a_setting` |
| P10 | A replay button would lie until L3 | `docs/inny-poziom-program.md` | P | disabled, with the reason and the `WOT_RECORD` path when a recording exists; `the_replay_button_is_disabled_and_says_why_until_a_viewer_exists` |

## G — garage

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| G1 | **The stat column is nine anonymous numbers** | `crates/apps/client/src/app/garage/panels/stats.rs:136-146`, `crates/apps/client/src/app/garage/layout.rs:83` | G | `every_stat_row_prints_its_own_label_a_number_and_a_bar_against_the_roster` (absorbs U6) |
| G2 | No derived values (GDD §15.4) | `crates/apps/client/src/app/garage/panels/stats.rs` | G | `the_derived_rows_agree_with_the_resolver_and_the_sim_to_1e_3` |
| G3 | No compare; the good/bad delta colours are orphaned | `crates/apps/client/src/app/garage/layout.rs:138-139` | G | `a_compare_column_shows_both_numbers_and_the_signed_delta`; `garage_compare` |
| G4 | **„Which tank am I in and why" is unanswerable**: the nameplate is tier and nation | `crates/apps/client/src/app/garage/panels/nameplate.rs` | G | `the_nameplate_names_the_class_and_the_role` (absorbs U7) |
| G5 | **Ammunition is illegible and unexplained**: designations at 0.016 under the screen's own 0.022 floor | `crates/apps/client/src/app/garage/panels/loadout.rs:54-55`, `crates/apps/client/src/app/garage/panels/inspector_legend.rs:22-24` | G | `no_garage_string_renders_below_the_legibility_floor` (absorbs U8) |
| G6 | **No press state, no tooltips, no key legend**: fourteen keys bound and one printed | `crates/apps/client/src/app/garage/overlay.rs:57-59`, `crates/apps/client/src/app/garage/actions.rs:175-250` | G | `every_clickable_has_three_states`, `the_hint_strip_prints_the_bound_keys_not_literals` (absorbs U9) |
| G7 | **BACK wears the commit red** | `crates/apps/client/src/app/garage/panels/techtree.rs:101` | G | `signal_red_is_only_worn_by_commit` (absorbs U10) |
| G8 | **Clicking a module on the 3D tank does nothing**; the turret cannot be turned | `crates/apps/client/src/app/garage/overlay.rs:157-210` | G | `clicking_a_module_on_the_hero_opens_its_slot`, `dragging_the_turret_turns_it_and_nothing_else` (absorbs U11) |
| G9 | No carousel filters | `crates/apps/client/src/app/garage/panels/carousel.rs` | G | `a_filtered_carousel_cycles_only_what_passes_the_chips` |
| G10 | Two tabs of seven | `crates/apps/client/src/app/garage/panels/topbar.rs`, `crates/world/scene_build/src/review_views.rs:120-148` | G | `GarageScreen` appended with the new screens, one golden each under `every_garage_screen_is_under_an_image_lock` |
| G11 | **The inspector answers nothing at a point**; no „shoot me" mode | `crates/apps/client/src/vehicle/armor_overlay.rs`, `crates/foundation/game_core/src/armor/impact.rs:65` | G | `the_inspector_equals_the_shell_on_a_thousand_points` (absorbs L1); `garage_inspector_point`, `garage_inspector_shoot_me` |
| G12 | The tree has no edges and no „what follows"; columns are nation-and-class pairs | `crates/apps/client/src/app/garage/panels/techtree.rs`, `crates/apps/client/src/app/garage/layout.rs:252-265` | G | `a_tree_node_says_what_it_is_and_what_follows_it_and_nothing_it_cannot_know` |
| G13 | Eight goldens lock the old look | `crates/apps/client/tests/goldens/look/garage_screen.png` | G (last) | one bless PR with the before/after numbers; the garage bounds of `look_goldens.rs` re-measured and moved only with the number in the message |
| G14 | No locked-vehicle state after a death | `crates/apps/client/src/app/garage/mod.rs` | G | `a_destroyed_vehicle_is_locked_until_its_battle_ends_and_says_so` |

---

# Part III — Waves and PRs

About 38 PRs. Estimates assume continuous sessions and the gate on one laptop.

**F — foundation (7 PRs, 3–4 days).** F0 this document → F1 `PassId::Hud` (the look goldens
green without a re-record: an empty pass changes no pixel) → F2 the vertex, the shader, the sheet,
premultiplied blending (garage goldens re-blessed) → F3 the fonts, the SDF atlas, Latin
Extended-A (re-blessed again — one attributable PR each beats one unattributable bless) → F4 the
layout layer, the theme, the interaction machine (no consumer yet, no pixel moves) → F5 the client
builds draw lists, the ratchet is seeded, the cursor is tracked (all 254 existing tests green, no
pixel moves) → F6 the HUD golden instrument and the budget lock (first bless of the HUD goldens;
the MX330 number recorded). F4 can run beside F1–F3.

**H — battle HUD (15 PRs, 5–7 days).** H7 the honest reload first; then the wire bump (W-1 to
W-6, the filter's conceal rules, the relay and its rate limit, fixtures re-pinned — a full gate);
then H0 + H1 + H2; H4 + H17; H5 + H6; H8 + H9; H10 + H11; H13 + H14; H15; H16; H3 + H18;
H19 + H20; H21; H22; H23 + H24 + H26. Each PR re-blesses only its own `hud_*` goldens and burns
its ratchet entries.

**P — product shell (7 PRs, 3 days).** P3 the ledger → P7 keybinds → P6 + P9 settings → P8
escape → W-7 if it did not ride the H bump → P1 + P2 + P10 results → P4 + P5 history and log.

**G — garage (9 PRs, 3–4 days).** G1 + G2 → G4 + G5 → G6 + G7 → G8 → G3 + G9 → G11 → G12 →
G10 + G14 → G13 the bless.

Inherited later, never blocking: V1 and V2 (the budget line's bush and camouflage eaters), V4
(the sixth sense's delay), L3 (the replay button), the R-lane class bands (the tree's column key),
Steam identity (the seat column becomes names).

# Verification

- The gates are `CLAUDE.md`'s: `scripts/preflight.ps1` first, `scripts/verify-pr.ps1 -Crates
  <touched>` per PR, the full `scripts/verify.ps1` for every wire PR and every golden-bless PR.
- Every renderer row (F1, F2, F3, H0) lands with a cold MX330 A→B→A measurement; the HUD pass
  number is read from `perf_capture`'s new row, never from a single warm run.
- Goldens are blessed in the PR that moves them, with the before/after in the message; a bless
  PR never carries a design change; the reticle frame `reticle_strip` is byte-identical through
  every PR of this program.
- Every wire change is append-only and bumps `PROTOCOL_VERSION` once per batch; replay fixtures
  are re-pinned in the same PR.
- During the H wave `crates/apps/client/src/hud/**` belongs to this program; other lanes reach a
  HUD element through the draw list, and the ratchet refuses a new call to the old primitives.
- Human review: `cargo run -p client --example probe -- hud_states` (every state at 1080p),
  `-- reticle_strip` (must not change), `-- garage_hangar_review`, and
  `WOT_LOOK_GOLDENS=1 cargo test -p client --test look_goldens` before any PR that touches a
  plate, a font or a palette.
- This document is under lock: every path in an Evidence cell exists in the tree, every register
  ID is unique, and the numbers it pins are read from the code by `roadmap_claims.rs`.
