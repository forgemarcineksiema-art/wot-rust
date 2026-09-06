# Game modes — two modes, two formats, and the queue that fills them

**Status: the owner's decision of 2026-09-06, recorded; the queue's rules R1–R10 accepted the
same evening; M2 (the format as data) LANDED — begun by GPT-6 Astra in the `wot-codex` worktree
(`codex/battle-formats`, never gated), finished, gated and merged by Claude after the owner's
"kontynuuj tą pracę" (2026-09-06, which lifted the documents-only pause for lane M); M3–M9
open.** This document is a chapter of the game's design in the sense of
`docs/game-design.md`: it adds rows 22 and 23 to that file's reconciliation table (the text of §2
and §3.4 says "7v7" and "7 minut"; the table wins) and it is where the modes, the formats and the
matchmaking are decided. The rules R1–R10 were written as proposals on 2026-09-06 and accepted the
same evening; the owner's second message of that evening fixed three things — humans sit on BOTH
teams in multiplayer and bots only fill the empty seats (the mode's definition), the clocks are
7 and 15 minutes, and every budget that was sized for 14 tanks is raised to the largest format
with the optimisation that makes it fit. The fill deadlines are the one first value left for a
playtest to move, by a dated row. The three findings of Part II each have a decision in
Part IV-b. The rows at the end are the work, in order, each with the test that locks it.

## The owner's words (2026-09-06, verbatim)

> Gra będzie miała dwa tryby gry.
>
> 1. 15 vs 15, multiplayer, jak nie ma graczy to dołączają boty. A w sumie to multiplayer możemy
>    podzielić na wybór, albo 7 vs 7 lub 15 vs 15. Tu już chyba trzeba będzie zaprojektować
>    matchmaking?
> 2. Tryb z samymi botami AI, 15 vs 15.

Yes — the choice of two formats is what makes a matchmaker necessary: one lobby that starts when
seven crews arrive (today) cannot serve two queues, two sides of humans and a bot fill that has to
mirror the band. Part III designs it.

## Vocabulary

Three words that the design document uses loosely and this one keeps apart:

| Word | Means | Values |
|---|---|---|
| **Mode** | who is on the field and where the authority runs | **PvP** (humans online, bots in every empty seat) · **AI battle** (the player and bots, offline) |
| **Format** | seats per team | **7v7** · **15v15** |
| **Objective** | what ends the battle before the clock | standard (two bases) and the moving objective — `docs/game-design.md` §3.4; today: last team standing, then the clock |

The objective is an R-lane product row of `docs/inny-poziom-program.md` and this document does
not touch it: both modes and both formats play whatever objective the map carries. The
`PracticeDuel` (one enemy, no clock — `crates/runtime/battle_host/src/setup.rs`) is a training
scenario of §17 ("samouczek, tryb treningowy"), not a third mode on the button.

## Part I — the two modes (decided)

| | PvP | AI battle |
|---|---|---|
| Format | 7v7 or 15v15, **the player's choice** | 15v15 |
| Humans | on BOTH teams, as many as the queue found (the owner, 2026-09-06: "w multiplayer ludzie muszą być w jednej jak i drugiej drużynie, boty mają zapełniać puste miejsca") | the player alone, on team one |
| Bots | every seat the queue did not fill, **marked** (§17: "jawnie oznaczone") | 29, marked the same way |
| Authority | the dedicated host (`battle_host::remote`), one battle per process | the in-process host (`battle_host::local`), no network, no account |
| Needs | the coordinator (Part III), identity (netcode block 1 / N5) for rating | nothing that does not exist today except the format |
| Counts | for rating (Part III, R8) and the battle history (P5) | for the battle history only |

The AI battle is not a demo of PvP; it is the population insurance §19 names ("problem pustego
serwera rozwiązany przed premierą: PvE-first, boty") made into a mode a player picks on purpose,
and it is the first of the two to be playable, because it is today's local battle with a
different seat count.

## Part II — what the repository has today, measured (2026-09-06)

The numbers a reader would quote back are pinned to their source by the `quality` gate
(`crates/tooling/quality/tests/roadmap_claims.rs`); a moved number fails the gate, not the reader.

- **seats per team today: 7** in the default format; **15** in `FifteenVsFifteen`.
  `game_core::BattleFormat` owns the count, clock and spawn formation; `RandomBattleConfig`
  carries it through setup and the lobby. `battle_host::SEATS_PER_TEAM` is a compatibility
  alias for the seven-seat default. Since M6 the crews are dealt across **both teams**
  (`random_battle_setup_for_humans`, a snake by hello order — `human_team` — with at most one
  more crew on a side; humans take a team's first seats, bots the rest); before it the host
  reserved team one for the crews and hosted co-op against bots, never humans against humans.
- **The lobby** (`crates/runtime/battle_host/src/remote.rs`, `crates/apps/server/src/main.rs`):
  one process, one battle; starts when the format's one-team capacity is seated or the deadline passes
  (`--lobby-wait-s`, default 30 s), every empty seat a bot; an empty lobby never starts; the next
  lobby opens when the battle ends. **lobby table cap today: 64** tracked addresses (the flood cap
  of N0) — re-based by M5 from 32 ("well above the seven seats plus reconnect churn") to seat the
  largest format's thirty crews plus their reconnects; the flood-cap lock reads the constant.
- **battle time limit today: 420** s for 7v7, **900** s for 15v15, owned by
  `BattleFormat::time_limit_s()`. `RANDOM_BATTLE_TIME_LIMIT_S` is the compatibility alias
  for 7v7; the live host reads the selected format. The existing `StartBattle` word carries
  its authoritative deadline to each remote crew without a protocol change.
- **matchmaking spread today: ±1** tier (`VehicleKind::MATCHMAKING_SPREAD`,
  `matchmaking_pool`): the bots deploy from the human's tier ±1 (a bracket too thin to field two
  designs falls back to the whole park). There is no rating, no identity (a player is a
  `SocketAddr`; `docs/multiplayer-production-program.md` rows 1, 2, 6), no coordinator, no queue
  screen; a client joins a host by the `WOT_CONNECT` variable and a failed connection is a loud
  refusal (netcode block 2).
- **The roster is honest about bots already**: protocol v51's `RosterEntry.crew_kind` is
  `Bot | Human` (`crates/runtime/net/src/roster.rs`), so the HUD's lists, the kill feed and the
  results screen can mark a bot without a new wire field.
- **The wire** (measured 2026-09-06 by `a_full_snapshot_of_the_largest_format_fits_its_budget`,
  `crates/runtime/net/tests/snapshot_budget.rs`, since M5 built at the LARGEST format): a
  saturated 30-tank snapshot is 8 029 B of the transport's 46 000 B (7 of 40 fragments,
  157 KiB/s per client at 20 Hz); the 14-tank one was 5 245 B of 32 200 B (5 of 28) — the
  fixture's world (craters, cover states, shells) is most of the bytes, so thirty tanks cost
  1.5× fourteen, not 2.1×. The standing rule is a QUARTER of the transport (11 500 B) so a new
  field is a decision, not a surprise.
- **The renderer**: `worst_case_battle_of_the_largest_format_fits_the_vehicle_instance_budget`,
  its aperture twin and `every_damaged_frame_of_the_largest_format_has_a_header`
  (`crates/apps/client/src/vehicle/render_frame.rs`) lock thirty hulls of the instance-heaviest
  vehicle into the 1 MiB instance buffer, 6 144 apertures and 128 damage headers (M5; they were
  14 hulls, 3 072 and 64); the crater ledger holds 384 (was 256) so the ground keeps half the
  armour scars of thirty tanks. The frame: 7v7 on the MX330 measured 59 FPS p50 in 2026-08 (the 4×
  MSAA instrument; the game ships 1×); the "full + 15v15" row of `perf_capture` exists since M5
  and has no cold MX330 number yet (M5b).
- **The spawn grid**: seven offsets in a three-column grid behind each zone's centre
  (`random_battle_spawn_position`: one at −8 m, three at −22 m, three at −40 m, ±1.5 m jitter),
  `slot % 7` past that — an eighth seat would land on the first.
- **The bots**: the route brain, the cover scoring, the five-term target choice, the unstuck arc
  (`battle_host/src/bots.rs`, `bot_combat.rs`); `cargo bench -p server --bench battle_tick`
  reads ~52–70 µs a tick at 14 tanks. Target choice is per pair: 30 tanks is 4.8× the pairs.
- **The maps**: five shipped, every one with a bot-battle soak at 7v7
  (`crates/runtime/battle_host/tests/*_battle.rs`); no seeded rotation (`docs/ROADMAP.md`,
  "PARTIAL"). Since M4 every blueprint says which formats it offers and the report certifies
  every seat of each (`map_forge::formats`); all five offer both.

## Part III — the queue (the matchmaker), designed

### Decided

- **D1 (the owner, 2026-09-06)** — two modes as Part I; PvP in two formats by the player's
  choice; bots fill.
- **D2 (`docs/game-design.md` §2, §16)** — the band is tier ±1 ("Klasa 1–4 w paśmie, spread ±1";
  "Matchmaking: prosty, ±1"); the class bands replace tiers when the R lane lands them.
- **D3 (§17)** — bots in PvP are marked, always.
- **D4 (`docs/ROADMAP.md`)** — skill matchmaking from day one, OpenSkill; identity was to be
  Steam (netcode block 1) — **the owner, 2026-09-06 evening: "Steama na razie jeszcze nie"**, so
  the identity behind a rating is an open decision (a coordinator-issued or local account key
  would do for closed playtests) and M8 waits on it; M7's coordinator does not need Steam.
- **D5 (the owner, 2026-09-06 evening)** — the population target is **~1 000 online**, hosting
  must be **cheap** (one Hetzner dedicated, a fixed ~40–50 €/month), and 15v15 must be
  **"mega płynne"**: no lag, no stutter, no FPS drops. `docs/game-design.md` row 24 does the
  arithmetic: the sim fits with room to spare, the wire fits only with delta snapshots — a cost
  condition, at the head of the netcode queue.

### The rules R1–R10 (proposed 2026-09-06, accepted by the owner the same evening)

| # | Rule | Why this and not the other thing |
|---|---|---|
| **R1** The format is data | `BattleFormat { seats_per_team, time_limit_s, fill_deadline_s }` is a table with two rows, an append-only identity enum on the wire (`SevenVsSeven`, `FifteenVsFifteen`) and never a literal; `SEATS_PER_TEAM` is its first row | four files carried the `7`; a third format (a 3v3 for a tutorial, a 10v10 for a playtest) is a row, not a grep |
| **R2** One queue per (format, band) | the ticket is (identity, vehicle, format, rating, entered-at); the player picks the format in the garage beside BATTLE (remembered in `settings.json`; 7v7 default) | the owner's choice makes the split explicit; a single queue that picks the format for the player would take the choice away |
| **R3** The band never widens; the bots fill | a ticket waits at most the format's fill deadline (**7v7: 30 s** — today's lobby; **15v15: 60 s**); at the deadline the battle starts and every empty seat is a bot from the same band | widening to ±2 is a WoT habit the creed forbids (no surprise that outranks the picture); the deadline is the promise §2 makes ("kolejka poniżej minuty") — a wait is never longer, only botter |
| **R4** Humans split evenly | at most one more human on one side; seats dealt by rating in a snake (1-2-2-1); tickets that arrived together (a platoon, later) stay together | humans against humans is the mode; a 6-against-1 fill would make PvP into co-op with a hostage |
| **R5** Bots mirror the band | each bot's tier is dealt so that the two teams' tier histograms match (the fill mirrors the humans' median first, then the ends); postures as today (every third bot overwatch) | a team of Tigers against a team of Panzer IVs is the ±25 % RNG by another door |
| **R6** A battle with humans on one side only counts nothing | rating moves only when both teams seated at least one human; a bot is a fixed-rating filler at the band's mean; the update's weight is the human share of the roster | a solo player at 03:00 must never farm or lose rating against bots; the AI battle exists for that hour |
| **R7** A crew that leaves for good becomes a bot | the seat stays the crew's through the reconnect budget (netcode block 4), then the hull is driven by the bot brain; the roster flips `crew_kind`, the list says so | the primer's §8 ("podmiana gracza na bota po rozłączeniu"); a dead hull for the rest of the battle is a 14-against-15 the other crews did not sign for |
| **R8** Per-format clocks | 7v7: 7 min = 420 s (§3.4); 15v15: **15 min = 900 s** (the owner, 2026-09-06: "w 15 vs 15 bitwa ma trwać 15 minut, w 7 vs 7 — 7 minut"); the format's clock IS the battle's time limit — today's one `RANDOM_BATTLE_TIME_LIMIT_S` becomes the format's row (finding 2) | the design document's 7 min was written for 7v7; twice the hulls on the same 1000 m takes longer to resolve; the values are the owner's, a playtest may move them by a dated row |
| **R9** 15v15 ships per map, behind two gates | a map offers 15v15 when its spawn zones seat fifteen (M4) and its worst view at 30 hulls meets the MX330 budget (M5); a map that fails is OPTIMISED until it passes (the owner: "zadbać o optymalizacje") and says "7v7 only" meanwhile — a state on the way, never a shipping category | the one-look policy: a frame drop is a bug, and thirty hulls in view is the frame's worst case by construction |
| **R10** The coordinator is one small service | a queue process beside the hosts (the hosts register, the coordinator hands each crew a host address, a seat token and the format; one host process per battle as today); the matchmaker itself is a **pure, deterministic function** `tickets × now → battles` in its own crate, tested without a socket | the netcode program already owes discovery (row 6) and identity (N5); the pure core is what a lock can hold, and it is the same code the AI battle uses to deal its 29 bots |

### The format table (R1, R3, R8 — the two rows `BattleFormat` carries)

| Format | Seats per team | Clock | Fill deadline | Maps |
|---|---|---|---|---|
| 7v7 | 7 | 420 s | 30 s | every shipped map |
| 15v15 | 15 | 900 s | 60 s | every shipped map, once M4 (fifteen seats) and M5 (the budgets at thirty, the MX330) pass on it |

### The population arithmetic, honestly

The owner's target since 2026-09-06 is ~1 000 online (D5): ~33 battles of 15v15 or ~70 of 7v7 at
once — more than enough for both queues to fill with crews in a play window, and the reason the
wire must go on a diet (row 24). §2 had sized 7v7 for 200 online in EU prime time: 15–30 battles
at once, a queue under a minute; that arithmetic still governs the quiet hours.
Two formats split that population: the 15v15 queue at the same 200 fills at most six all-human
battles, and every one below that threshold is a battle with bots in it. R3 turns the split from
a wait into a fill — the promise stays "under a minute", the price is the bot share, and the
queue screen prints it (humans found / seats, the bots that will fill, the countdown, CANCEL and
the other format one click away) so the player can change formats before the deadline, not after.
The announced play windows of §19 are where 15v15 fills with humans; outside them it is the AI
battle with a few strangers in it, and it says so.

### Where it runs (the pieces, and which exist)

| Piece | Today | Owed |
|---|---|---|
| The format through the host | `BattleFormat` through setup, local authority and lobby (M2) | thirty-hull release measurements — M5 |
| The AI battle | `new_ai_battle`: one human, 29 bots, 900 s; no socket (M2) | the garage entry — M3 |
| Humans on both sides of a dedicated host | dealt in a snake by hello order, "full" = both teams' seats (M6) | by rating — M8 |
| The queue screen | none (`WOT_CONNECT`, a loud refusal) | the interface program's P lane (M7 names the row) |
| The matchmaker (pure) | `crates/runtime/matchmaker` — tickets × now → battle plans (M7a) | the coordinator calls it — M7b |
| The coordinator (service) | none | M7b, with the netcode program's N4 (discovery, `PeerId` — no Steam needed) |
| Identity and rating | none; a player is an address | N5 (Steam), then M8 |
| Bot substitution | past a minute of silence (the client's re-dial budget) the bot brain drives the hull and the roster says so; a returning crew takes it back (M9) | — |

## Part IV — what 15v15 costs (the constraints, with today's numbers)

A format is not a number in a table until each of these has a measurement at 30 tanks.

1. **The wire.** MEASURED at thirty (M5a, 2026-09-06), not extrapolated: a saturated 30-tank
   snapshot is 8 029 B — the world around the tanks is most of the bytes, so thirty cost 1.4×
   fourteen (5 245 B), not the 2.1× first estimated here. **Decided (the owner, 2026-09-06,
   finding 3): the budget was sized for 14 tanks, so it is RAISED to the largest format and the
   payload is OPTIMISED** — the transport's line re-based (28 → 40 fragments, 46 000 B) so the
   largest format's snapshot keeps the quarter's headroom (it sits at 17.5 %, 7 fragments), then
   delta snapshots or a smaller per-tank payload with a loss estimate on the ack lane (netcode
   row 9) as the optimisation, because a raised line changes what FITS and not the loss
   arithmetic: a lost fragment still kills its snapshot — at 2 % loss one snapshot in ten at five
   fragments, one in seven at seven. Both are M5; the line landed (M5a), the diet is owed (M5b)
   before the format meets its first humans (M6, M7). The AI battle is offline. A full-human
   15v15 host uploads ~4.6 MiB/s at 157 KiB/s per client; a 7v7, ~1.4.
2. **The frame.** Thirty hulls in view is the worst case the one-look policy has never met; the
   vehicle instance and aperture locks (`render_frame.rs`) are re-derived at the LARGEST format,
   and `perf_capture` gains a "15v15 worst view" row measured cold on the MX330 (the A→B→A
   sandwich, `docs/engineering-rules.md`). The verdict is recorded like the garage's 4× MSAA
   NO-GO of 2026-08-15: a number and a date, per map (R9). The lever if it fails is the vehicle
   LOD ladder and instancing (lane K), never a quality option — the owner's rule: the budget rises
   to thirty and the optimisation makes it fit; the MX330 stays the floor (`docs/game-design.md`
   row 2).
3. **The sim and the bots.** The tick is cheap (~70 µs at 14); the pair-wise target choice and
   the per-viewer snapshot filter both grow ~4.8×, still far under the 16.7 ms tick — measure
   with `battle_tick` at 30 and lock the ceiling. §16's "one core, a dozen 7v7" becomes fewer
   15v15 per core; the hosting cost stays negligible at ~20 € a copy.
4. **The spawn zones.** Fifteen seats need a wider grid (five columns: 1 + 5 + 5 + 4 over four
   rows, ~90 × 60 m) and a map gate that every seat lands inside the zone's radius, on ground a
   tank can leave (`map_forge` report, next to the playability BFS) — per map, so a map may say
   "7v7 only" (R9).
5. **The interface.** Team lists of fifteen (seat letters A–O), thirty marks on the minimap, a
   kill feed that scrolls faster; the roster manifest carries thirty entries. The lists were built
   for seven — a size question for the interface program, not a new element.
6. **Replays and goldens.** The 7v7 fixtures stay byte-identical (the format's first row IS
   today's battle); 15v15 gets its own soak per map alongside the 7v7 one.

## Part IV-b — the three findings, and what is done about them (decided 2026-09-06)

The owner, the same evening: "z tymi trzema rzeczami też trzeba będzie coś zrobić". Each gets a
decision here, a register row where a register exists, and a lock in the row that closes it.

1. **Humans sit on team one only.** The dedicated host of today is co-op against bots; its
   header ("up to seven human crews") means seven co-op seats. **The owner (2026-09-06): "w
   multiplayer ludzie muszą być w jednej jak i drugiej drużynie, boty mają zapełniać puste
   miejsca" — that is the DEFINITION of the mode, so the team-one-only host is a defect, not a
   choice. Decision:** M6 is the first online row — before M7 and before any online test; it
   deals seats across both teams by R4 and makes "full" both teams' seats; the per-viewer filter
   needs nothing. Recorded as `docs/multiplayer-production-program.md` register row 15 and in
   `docs/ROADMAP.md`'s gap list. Lock: M6's two-client test —
   `two_crews_on_opposite_teams_see_each_other_as_enemies_and_the_filter_hides_what_it_hid`,
   landed 2026-09-06.
2. **The design's 7-minute clock was never implemented**; the code has one 600 s limit for the
   7v7. **The owner (2026-09-06): "w 15 vs 15 bitwa ma trwać 15 minut, w 7 vs 7 — 7 minut".
   Decision:** the clock is the format's row (R8: 420 s for 7v7, 900 s for 15v15), the battle's
   time limit as DATA, the HUD's clock (on the wire since v45) counting the format's value;
   today's 600 s on the 7v7 is debt, closed by M2 together with the format. Recorded as
   `docs/game-design.md` reconciliation row 23. Lock:
   `the_7v7_clock_is_seven_minutes_and_the_15v15_clock_fifteen` (M2).
3. **Thirty tanks break the wire's quarter rule** — as they must: that rule, the vehicle
   instance buffer, the aperture budget, the lobby's table cap and the `14` in every worst-case
   lock were sized for 14 tanks. **The owner (2026-09-06): "podnieść wszystkie budżety i zadbać o
   optymalizacje". Decision:** every budget whose basis was 14 is re-based to the largest format
   (M5 opens with the table of them), each with its measurement at thirty; and the optimisation
   that makes thirty fit the wire and the MX330 — delta snapshots or a smaller per-tank payload
   (netcode row 9), the vehicle LOD ladder (lane K) — is lane M's work in M5, never a gate that
   keeps the format offline and never a quality option. Recorded as
   `docs/multiplayer-production-program.md` register row 16. Lock:
   `a_full_snapshot_of_the_largest_format_fits_its_budget` and the instance, aperture and
   header locks at thirty — landed as M5a (2026-09-06).

## Part V — the rows (lane M), in order

| ID | Row | Evidence | Closes when |
|---|---|---|---|
| ~~M1~~ | ~~The seat count is a literal in four places~~ — **CLOSED (2026-09-06)** with this document: `SEATS_PER_TEAM` named and exported, `setup.rs` and the lobby threshold count from it | `crates/runtime/battle_host/src/battle.rs` | the four claims of `roadmap_claims.rs` on this document; every existing 7v7 lock unchanged |
| ~~M2~~ | ~~**The format is data**~~ — **CLOSED (2026-09-06)**, begun by GPT-6 Astra, finished by Claude (R1, R8, finding 2): `game_core::BattleFormat` (seats, clock, spawn formation; append-only) through `RandomBattleConfig.format`, the setup, the local host (`new_random`, `new_ai_battle`, `new_random_for_humans`), the lobby's threshold and `StartBattle`'s clock from the format; the clocks 420 s / 900 s; the five-column grid for fifteen; `BattleMode` grows `Random15v15` and `AiBattle`; the 7v7 deployment byte-identical on all five maps | `crates/foundation/game_core/src/battle_format.rs`, `crates/runtime/battle_host/src/battle.rs`, `setup.rs`, `local.rs`, `remote.rs` | `a_15v15_setup_spawns_thirty_tanks_with_the_player_on_team_one`; `the_7v7_format_is_todays_battle_byte_for_byte` (the replay fixtures and the five map soaks untouched); `every_seat_of_every_format_lands_inside_its_zone`; `the_7v7_clock_is_seven_minutes_and_the_15v15_clock_fifteen` |
| M3 | **The AI battle on the button**: the garage's BATTLE entry becomes BATTLE (online, the format switch 7v7 · 15v15, disabled with the reason until M7) and AI BATTLE (offline 15v15, today's local path); the results screen says which mode it was | `crates/apps/client/src/app/garage/actions.rs`, `crates/apps/client/src/app/mod.rs` | `the_ai_battle_is_fifteen_against_fifteen_and_needs_no_socket`; the garage golden (the G lane draws it) |
| ~~M4~~ | ~~**The 15-seat spawn gate per map**~~ — **CLOSED (2026-09-06)**: `GameplaySpec.formats` in every shipped blueprint (`formats: [SevenVsSeven, FifteenVsFifteen]`; empty means every format), the report's `formats` check judges every seat of every offered format WHERE THE HOST DEPLOYS IT — `BattleFormat::seat_position` is the one arithmetic both use, at the jitter's four corners: inside the zone's radius, on the map, dry, clear of cover by the widest hull — and `map_forge::formats(map)` is what the host and the queue read; all five maps offer both. The compiled map is unchanged, so no golden moved | `crates/foundation/game_core/src/battle_format.rs`, `crates/world/map_forge/src/report.rs`, `crates/world/map_forge/src/catalog.rs`, `crates/world/map_forge/blueprints/*.map.ron` | `a_map_offers_only_the_formats_its_zones_seat` (a 50 m zone certifies 7v7 and refuses 15v15, naming the seat), `every_shipped_map_offers_both_formats_and_seats_them`; the host's `every_seat_of_every_format_lands_inside_its_zone` and `the_7v7_format_is_todays_battle_byte_for_byte` unchanged |
| ~~M5a~~ | ~~**The budgets re-based**~~ — **CLOSED (2026-09-06)** (Part IV 1–3, finding 3; the owner: every budget sized for 14 tanks is raised to the largest format): `MAX_FRAGMENTS` 28 → 40 (46 000 B; the wire format unchanged), `MAX_DAMAGE_APERTURES` 3 072 → 6 144, `MAX_DAMAGE_HEADERS` 64 → 128, `MAX_TRACKED_CLIENTS` 32 → 64, `sim::MAX_CRATERS` 256 → 384 (the ground stays at half the armour scars of thirty tanks), the instance buffer locked at thirty; every lock counts from `BattleFormat::LARGEST`; the saturated 30-tank snapshot measured at 8 029 B (17.5 %, 7 fragments); `battle_tick` gains `random_15v15_tick`, `perf_capture` builds its lineup per format and gains "full + 15v15" | `crates/runtime/net/src/transport.rs`, `crates/runtime/net/tests/snapshot_budget.rs`, `crates/render/renderer_wgpu/src/scene_renderer/armor_damage.rs`, `crates/apps/client/src/vehicle/render_frame.rs`, `crates/runtime/battle_host/src/remote.rs`, `crates/runtime/battle_host/benches/battle_tick.rs`, `crates/apps/client/examples/probe/perf_capture.rs` | `a_full_snapshot_of_the_largest_format_fits_its_budget`, `worst_case_battle_of_the_largest_format_fits_the_vehicle_instance_budget`, `..._fits_the_grouped_aperture_budget`, `every_damaged_frame_of_the_largest_format_has_a_header`, the flood-cap lock against the constant |
| M5b | **The optimisation and the measurements**: the cold MX330 number of "full + 15v15" per shipped map (`WOT_MAP=<slug> cargo run --release -p client --example probe -- perf_capture` — the probe reads the map the way the game does since 2026-09-06; its cycles interleave the rows, so one cold run is the sandwich) with the verdict recorded, per map — owed until the machine is idle and cool (two sessions and a gate keep the MX330 warm all day); `random_15v15_tick` recorded against the 7v7 rows; the payload diet — designed 2026-09-06 as the netcode program's "Delta snapshots" section (four PRs: the world's ledgers to the reliable lane, the pose quantised, the slow state delta against a per-client baseline acked on the input batch, the one-datagram lock) until a 30-tank snapshot rides ONE datagram; the vehicle LOD ladder where a map fails the frame (lane K) — a map that fails is optimised until it passes, never a quality option | `crates/apps/client/examples/probe/perf_capture.rs`, `crates/runtime/battle_host/benches/battle_tick.rs`, `crates/runtime/net/src/` | the recorded measurement per map, with the date; the snapshot at five fragments |
| ~~M6~~ | ~~**Humans on both sides**~~ — **CLOSED (2026-09-06)** (R4, finding 1 — the mode's definition, the first online row): `human_team` deals the crews in a snake by hello order (1-2-2-1; by rating when M8 lands), at most one more on a side, humans in a team's first seats and bots after; the lobby's "full" = both teams' seats; the anti-wallhack filter unchanged (it is per viewer already); one crew is still the desktop battle bit for bit | `crates/runtime/battle_host/src/remote.rs`, `setup.rs` | `two_crews_on_opposite_teams_see_each_other_as_enemies_and_the_filter_hides_what_it_hid` (a two-client `MemoryHub` lock, armed the way netcode block 3's was) |
| ~~M7a~~ | ~~**The matchmaker's pure core**~~ — **CLOSED (2026-09-06)** (R2–R5, R10): `crates/runtime/matchmaker` — `deal(tickets, now) → Deal { battles, waiting }`; one queue per (format, band): a battle anchors on its oldest ticket and admits only crews within ±1 tier of it, starts when both sides are full or when the anchor waited the format's fill deadline (`BattleFormat::fill_deadline_s`, 30 s / 60 s), crews dealt in a snake by rating or arrival (`BattleFormat::snake_side` — the host's `human_team` deals through the same rule), bots mirror the crews' tier histogram, the rest take the anchor's tier; no socket, no clock, no randomness; and the host seats a plan as dealt (`LocalAuthoritativeServer::new_from_plan` — the coordinator's constructor) | `crates/runtime/matchmaker/src/lib.rs`, `crates/foundation/game_core/src/battle_format.rs` | `the_band_never_widens_and_the_deadline_always_starts`, `humans_split_evenly_and_bots_mirror_the_tier_histogram`, `the_same_tickets_deal_the_same_battle`, `a_full_queue_starts_at_once_and_a_crew_is_dealt_once`, `with_ratings_the_snake_deals_by_rating`; battle_host: `a_plan_dealt_by_the_matchmaker_is_seated_as_dealt` |
| M7b | **The coordinator and the queue screen** (R10): the coordinator process (holds the tickets, calls `matchmaker::deal`, registers hosts, hands each crew a host address, a seat token and the format — plain UDP, no Steam), the seat token in the hello (a wire bump, additive), the queue screen — a P-lane row this document owes the interface program: format, humans found / seats, bots that will fill, countdown, CANCEL, the other format | `crates/apps/server/src/main.rs`, the netcode program's N4 (`PeerId`, the join screen) | a two-crew battle dealt by the coordinator over `MemoryHub`; the queue screen's golden |
| M8 | **Identity and rating** (D4, R6): after N5 — OpenSkill over identity-bound tickets, the store, the weight by human share | the netcode program's N5 | `a_battle_with_humans_on_one_side_moves_no_rating`; `a_bot_is_a_fixed_rating_filler` |
| ~~M9~~ | ~~**Bot substitution**~~ — **CLOSED (2026-09-06)** (R7): a seat freed by silence, a goodbye or an overflow remembers when; past `BOT_TAKEOVER_MS` (60 s — the client's thirty re-dials two seconds apart) the bot brain adopts the hull with the roster's next seeded route and posture, the roster flips to `Bot` and is repeated to every seated crew for twenty ticks; a crew that claims the seat later takes the hull back and the roster names a crew again | `crates/runtime/battle_host/src/remote.rs`, `bots.rs`, `local.rs` | `a_crew_that_never_returns_becomes_a_bot_and_the_roster_says_so` (two crews, one silent: Human inside the budget, Bot past it — on the host and on A's wire — and Human again when a fresh session claims the hull) |

**Order.** M2 → M3 first (the AI battle at 15v15 is playable with no network and answers the
frame question early), M4 and M5 in the same week (M5a's budgets landed; M5b's measurements decide
whether 15v15 ships on every map or on some — and the optimisation until every map does), M6 first among the online
rows (before M7 and before any online test), M7 with N4, M8 after N5, M9 when the reconnect
budget has met a real drop. The netcode program's waves interleave: nothing in lane M waits for
N3 (lag compensation), M7 cannot land before N4's discovery, and row 9's delta snapshots are
M5's optimisation, done before the format meets its first humans (finding 3). The documents-only
pause of 2026-09-06 was lifted the same day, when the owner handed Astra's unfinished M2 to Claude
("kontynuuj tą pracę"); M3 onward starts on the owner's word as before.

**Locks that already exist and change meaning.** Every `7v7` in a test name is a format's name,
not a fact about the game; when M2 lands, the format is the fixture's parameter and the names stay
(the 7v7 fixtures ARE the first format). New rows lock the largest format, never both by hand.
