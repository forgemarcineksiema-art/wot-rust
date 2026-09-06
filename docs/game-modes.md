# Game modes — two modes, two formats, and the queue that fills them

**Status: the owner's decision of 2026-09-06, recorded; the queue DESIGNED and, the same
evening, DECIDED — the owner: "Akceptuję twoje propozycje (R1–R10)"; nothing built, and nothing
is to be built until the owner's word ("jeszcze pracy z kodem nie rozpoczynaj … jedynie praca
nad dokumentami").** This document is a chapter of the game's design in the sense of
`docs/game-design.md`: it adds rows 22 and 23 to that file's reconciliation table (the text of §2
and §3.4 says "7v7" and "7 minut"; the table wins) and it is where the modes, the formats and the
matchmaking are decided. The rules R1–R10 were written as proposals on 2026-09-06 and accepted the
same evening; the first VALUES in them (the fill deadlines, the clocks) are a playtest's to move,
by a dated row. The three findings of Part II each have a decision in Part IV-b. The rows at the
end are the work, in order, each with the test that locks it.

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
| Humans | on BOTH teams, as many as the queue found | the player alone, on team one |
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

- **seats per team today: 7** — `battle_host::SEATS_PER_TEAM`, named by this document's PR
  (it was the literal `7` in three places of `setup.rs` and the lobby's threshold). Humans sit on
  **team one only** (`random_7v7_setup_for_humans` reserves the first `humans` team-one slots;
  team two is all bots): the dedicated host of today hosts co-op against bots, never humans
  against humans. That is the largest single gap between "multiplayer" as the code has it and
  PvP as Part I means it (row M6).
- **The lobby** (`crates/runtime/battle_host/src/remote.rs`, `crates/apps/server/src/main.rs`):
  one process, one battle; starts when seven established crews are seated or the deadline passes
  (`--lobby-wait-s`, default 30 s), every empty seat a bot; an empty lobby never starts; the next
  lobby opens when the battle ends. **lobby table cap today: 32** tracked addresses (the flood cap
  of N0) — sized "well above the seven seats plus reconnect churn"; thirty seated crews plus their
  reconnects press against it.
- **battle time limit today: 600** s, one value for the random battle
  (`RANDOM_BATTLE_TIME_LIMIT_S`, "ten minutes fits the 7v7 scale"). The design document's 7 minutes
  (§3.4, Załącznik A) is the DESIGN; the code has the safety net. Per-format clocks are M2's data.
- **matchmaking spread today: ±1** tier (`VehicleKind::MATCHMAKING_SPREAD`,
  `matchmaking_pool`): the bots deploy from the human's tier ±1 (a bracket too thin to field two
  designs falls back to the whole park). There is no rating, no identity (a player is a
  `SocketAddr`; `docs/multiplayer-production-program.md` rows 1, 2, 6), no coordinator, no queue
  screen; a client joins a host by the `WOT_CONNECT` variable and a failed connection is a loud
  refusal (netcode block 2).
- **The roster is honest about bots already**: protocol v51's `RosterEntry.crew_kind` is
  `Bot | Human` (`crates/runtime/net/src/roster.rs`), so the HUD's lists, the kill feed and the
  results screen can mark a bot without a new wire field.
- **The wire** (measured 2026-09-06 by
  `a_full_7v7_snapshot_fits_one_transport_message_with_room_to_spare`, `crates/runtime/net/tests/snapshot_budget.rs`):
  a saturated 14-tank snapshot is 5 245 B of the transport's 32 200 B (5 of 28 fragments,
  102 KiB/s per client at 20 Hz). The standing rule is a QUARTER of the transport (8 050 B) so a
  new field is a decision, not a surprise.
- **The renderer**: `worst_case_7v7_battle_fits_the_vehicle_instance_budget` and its aperture
  twin (`crates/apps/client/src/vehicle/render_frame.rs`) lock 14 hulls of the instance-heaviest
  vehicle into the 1 MiB instance buffer. The frame: 7v7 on the MX330 measured 59 FPS p50 in
  2026-08 (the 4× MSAA instrument; the game ships 1×).
- **The spawn grid**: seven offsets in a three-column grid behind each zone's centre
  (`random_battle_spawn_position`: one at −8 m, three at −22 m, three at −40 m, ±1.5 m jitter),
  `slot % 7` past that — an eighth seat would land on the first.
- **The bots**: the route brain, the cover scoring, the five-term target choice, the unstuck arc
  (`battle_host/src/bots.rs`, `bot_combat.rs`); `cargo bench -p server --bench battle_tick`
  reads ~52–70 µs a tick at 14 tanks. Target choice is per pair: 30 tanks is 4.8× the pairs.
- **The maps**: five shipped, every one with a bot-battle soak at 7v7
  (`crates/runtime/battle_host/tests/*_battle.rs`); no seeded rotation (`docs/ROADMAP.md`,
  "PARTIAL").

## Part III — the queue (the matchmaker), designed

### Decided

- **D1 (the owner, 2026-09-06)** — two modes as Part I; PvP in two formats by the player's
  choice; bots fill.
- **D2 (`docs/game-design.md` §2, §16)** — the band is tier ±1 ("Klasa 1–4 w paśmie, spread ±1";
  "Matchmaking: prosty, ±1"); the class bands replace tiers when the R lane lands them.
- **D3 (§17)** — bots in PvP are marked, always.
- **D4 (`docs/ROADMAP.md`)** — skill matchmaking from day one, OpenSkill; identity is Steam
  (netcode block 1).

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
| **R8** Per-format clocks, first values | 7v7: 7 min = 420 s (§3.4); 15v15: 10 min = 600 s; the format's clock IS the battle's time limit — today's one `RANDOM_BATTLE_TIME_LIMIT_S` becomes the format's row (finding 2) | the design document's 7 min was written for 7v7; twice the hulls on the same 1000 m takes longer to resolve; the playtest is the arbiter, not this table |
| **R9** 15v15 ships per map, behind two gates | a map offers 15v15 only when its spawn zones seat fifteen (M4) and its worst view at 30 hulls meets the MX330 budget (M5); a map that fails ships 7v7-only, honestly listed | the one-look policy: a frame drop is a bug, and thirty hulls in view is the frame's worst case by construction |
| **R10** The coordinator is one small service | a queue process beside the hosts (the hosts register, the coordinator hands each crew a host address, a seat token and the format; one host process per battle as today); the matchmaker itself is a **pure, deterministic function** `tickets × now → battles` in its own crate, tested without a socket | the netcode program already owes discovery (row 6) and identity (N5); the pure core is what a lock can hold, and it is the same code the AI battle uses to deal its 29 bots |

### The format table (R1, R3, R8 — the two rows `BattleFormat` carries)

| Format | Seats per team | Clock | Fill deadline | Maps |
|---|---|---|---|---|
| 7v7 | 7 | 420 s | 30 s | every shipped map |
| 15v15 | 15 | 600 s | 60 s | the maps that pass M4 (fifteen seats) and M5 (the MX330 at thirty hulls, the wire) |

### The population arithmetic, honestly

§2 sized 7v7 for 200 online in EU prime time: 15–30 battles at once, a queue under a minute.
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
| The format through the host | a literal, named (`SEATS_PER_TEAM`) | R1 — M2 |
| The AI battle | the local 7v7 with bots on both sides | the seat count — M2, the garage entry — M3 |
| Humans on both sides of a dedicated host | team one only | M6 |
| The queue screen | none (`WOT_CONNECT`, a loud refusal) | the interface program's P lane (M7 names the row) |
| The matchmaker (pure) | the lobby's "first seven, then bots" | the crate — M7 |
| The coordinator (service) | none | M7, with the netcode program's N4 (discovery, `PeerId`) |
| Identity and rating | none; a player is an address | N5 (Steam), then M8 |
| Bot substitution | none; a lost crew's hull stands still | M9 |

## Part IV — what 15v15 costs (the constraints, with today's numbers)

A format is not a number in a table until each of these has a measurement at 30 tanks.

1. **The wire.** Linear in hulls: a saturated 30-tank snapshot ≈ 11 240 B — 35 % of the
   transport, 10 of 28 fragments, ~219 KiB/s per client at 20 Hz. It FITS, and it breaks the
   quarter rule (8 050 B). A lost fragment still kills the whole snapshot: at 2 % loss, one
   snapshot in ten at five fragments, one in five at ten. **Decided (2026-09-06, finding 3):**
   the budget is per format — 7v7 keeps the quarter rule; the largest format's lock is HALF the
   transport at no more than ten fragments, as the interim ceiling; and 15v15 ONLINE waits for
   netcode row 9 (delta snapshots or a smaller per-tank payload, with a loss estimate on the ack
   lane), which moves from N5 to before M6 for that format — one snapshot in five lost at 2 %
   loss is a stutter on every remote hull, and no budget line fixes that. The AI battle is
   offline and does not wait. A full-human 15v15 host uploads ~6.4 MiB/s; a 7v7, ~1.4.
2. **The frame.** Thirty hulls in view is the worst case the one-look policy has never met; the
   vehicle instance and aperture locks (`render_frame.rs`) are re-derived at the LARGEST format,
   and `perf_capture` gains a "15v15 worst view" row measured cold on the MX330 (the A→B→A
   sandwich, `docs/engineering-rules.md`). The verdict is recorded like the garage's 4× MSAA
   NO-GO of 2026-08-15: a number and a date, per map (R9). The lever if it fails is the vehicle
   LOD ladder and instancing (lane K), never a quality option.
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
   header ("up to seven human crews") means seven co-op seats. **Decision:** no human-against-human
   test and no queue work (M7) before M6; M6 deals seats across both teams by R4 and makes "full"
   both teams' seats; the per-viewer filter needs nothing. Recorded as
   `docs/multiplayer-production-program.md` register row 15 and in `docs/ROADMAP.md`'s gap list.
   Lock: M6's two-client test.
2. **The design's 7-minute clock was never implemented**; the code has one 600 s limit for the
   7v7. **Decision:** the clock is the format's row (R8: 420 s for 7v7, 600 s for 15v15), the
   battle's time limit as DATA, the HUD's clock (on the wire since v45) counting the format's
   value; today's 600 s on the 7v7 is debt, closed by M2 together with the format. Recorded as
   `docs/game-design.md` reconciliation row 23. Lock:
   `the_7v7_clock_is_seven_minutes_and_the_15v15_clock_ten` (M2).
3. **Thirty tanks break the wire's quarter rule.** **Decision:** Part IV item 1 — the budget per
   format, half the transport at ten fragments as the largest format's ceiling, and delta
   snapshots (netcode row 9) BEFORE 15v15 goes online with humans; the AI battle does not wait.
   Recorded as `docs/multiplayer-production-program.md` register row 16. Lock: M5's
   `a_full_snapshot_of_the_largest_format_fits_its_budget`.

## Part V — the rows (lane M), in order

| ID | Row | Evidence | Closes when |
|---|---|---|---|
| ~~M1~~ | ~~The seat count is a literal in four places~~ — **CLOSED (2026-09-06)** with this document: `SEATS_PER_TEAM` named and exported, `setup.rs` and the lobby threshold count from it | `crates/runtime/battle_host/src/battle.rs` | the four claims of `roadmap_claims.rs` on this document; every existing 7v7 lock unchanged |
| M2 | **The format is data** (R1, R8, finding 2): `BattleFormat` through `RandomBattleConfig`, the setup, the local host, the lobby (`LOBBY_FULL_PLAYERS` from the format), the clock per format (420 s / 600 s); the five-column spawn grid; `BattleMode` grows `AiBattle` | `crates/runtime/battle_host/src/battle.rs`, `setup.rs`, `local.rs`, `remote.rs` | `a_15v15_setup_spawns_thirty_tanks_with_the_player_on_team_one`; `the_7v7_format_is_todays_battle_byte_for_byte` (the replay fixtures and the five map soaks untouched); `every_seat_of_every_format_lands_inside_its_zone`; `the_7v7_clock_is_seven_minutes_and_the_15v15_clock_ten` |
| M3 | **The AI battle on the button**: the garage's BATTLE entry becomes BATTLE (online, the format switch 7v7 · 15v15, disabled with the reason until M7) and AI BATTLE (offline 15v15, today's local path); the results screen says which mode it was | `crates/apps/client/src/app/garage/actions.rs`, `crates/apps/client/src/app/mod.rs` | `the_ai_battle_is_fifteen_against_fifteen_and_needs_no_socket`; the garage golden (the G lane draws it) |
| M4 | **The 15-seat spawn gate per map**: the `map_forge` report row, the blueprint's format list (`formats: [SevenVsSeven, FifteenVsFifteen]`), the five maps re-reported | `crates/world/map_forge/src/report.rs`, `crates/world/map_forge/blueprints/*.map.ron` | `a_map_offers_only_the_formats_its_zones_seat`; the goldens re-blessed deliberately |
| M5 | **The measurements** (Part IV 1–3, finding 3): the wire lock per format (7v7 a quarter, 15v15 half at ten fragments), the instance and aperture locks at the largest format, the `perf_capture` 15v15 row and the `battle_tick` row at 30 — the GO/NO-GO per map on the MX330 | `crates/runtime/net/tests/snapshot_budget.rs`, `crates/apps/client/src/vehicle/render_frame.rs`, `crates/apps/client/examples/probe/perf_capture.rs` | `a_full_snapshot_of_the_largest_format_fits_its_budget`; the recorded verdict, per map, with the date |
| M6 | **Humans on both sides** of the dedicated host (R4, finding 1): seats dealt across the two teams, the lobby's "full" = both teams' seats, the anti-wallhack filter unchanged (it is per viewer already) | `crates/runtime/battle_host/src/remote.rs`, `setup.rs` | `two_crews_on_opposite_teams_see_each_other_as_enemies_and_the_filter_hides_what_it_hid` (a two-client `MemoryHub` lock, armed the way netcode block 3's was) |
| M7 | **The matchmaker and the coordinator** (R2, R3, R5, R10): the pure crate (`tickets × now → battles`), the coordinator process, the host registration, the seat token in the hello (a wire bump, additive), the queue screen — a P-lane row this document owes the interface program: format, humans found / seats, bots that will fill, countdown, CANCEL, the other format | a new runtime crate, `crates/apps/server/src/main.rs`, the netcode program's N4 | `the_band_never_widens_and_the_deadline_always_starts`, `humans_split_evenly_and_bots_mirror_the_tier_histogram`, `the_same_tickets_deal_the_same_battle` (determinism); the queue screen's golden |
| M8 | **Identity and rating** (D4, R6): after N5 — OpenSkill over identity-bound tickets, the store, the weight by human share | the netcode program's N5 | `a_battle_with_humans_on_one_side_moves_no_rating`; `a_bot_is_a_fixed_rating_filler` |
| M9 | **Bot substitution** (R7): a crew past its reconnect budget hands the hull to the bot brain; the roster flips | `crates/runtime/battle_host/src/remote.rs`, `bots.rs` | `a_crew_that_never_returns_becomes_a_bot_and_the_roster_says_so` |

**Order.** M2 → M3 first (the AI battle at 15v15 is playable with no network and answers the
frame question early), M4 and M5 in the same week (the measurements decide whether 15v15 ships on
every map or on some), M6 before any human-vs-human test and before M7, M7 with N4, M8 after N5,
M9 when the reconnect budget has met a real drop. The netcode program's waves interleave: nothing
in lane M waits for N3 (lag compensation), M7 cannot land before N4's discovery, and 15v15 ONLINE
waits for netcode row 9 (delta snapshots), which moves ahead of M6 for that format (finding 3).
None of it starts before the owner's word: documents only (2026-09-06).

**Locks that already exist and change meaning.** Every `7v7` in a test name is a format's name,
not a fact about the game; when M2 lands, the format is the fixture's parameter and the names stay
(the 7v7 fixtures ARE the first format). New rows lock the largest format, never both by hand.
