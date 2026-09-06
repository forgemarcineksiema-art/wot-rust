# Multiplayer Production — the road to a Steam battle

**Status: register measured 2026-08-03 at wire v43 (v48 today — the five bumps since, v44
spotting privacy through v48 roster identity, closed no register block); program proposed,
wave order awaiting the user's sign-off.** The release target is confirmed: multiplayer on Steam, and the release decision
itself is a QUALITY verdict. This document is the networking half of that verdict: what a
production battle between humans still lacks, with evidence, and in what order it gets built.

Context docs: `w1-networking-primer.md` (the v38 foundation), `server-first-policy.md`
(authority doctrine), `ops/dedicated-server.md` (today's runbook). v38–v43 hardened the
SESSION — envelope, ACK lanes, reliable combat channel, terminal gate, wire budget. Nothing
since has touched **discovery, identity, trust, or ops**.

## The debt register (evidence at file:line, risk-ordered)

| # | Area | Current state | Production needs | Scope |
|---|------|---------------|------------------|-------|
| 1 | **Seat hijack via spoofed hello** | any new `ClientHello` from a seated address re-keys the session and KEEPS the tank (`remote.rs::begin_session`); this IS the fast-reconnect feature (`remote_reconnect.rs`), so a spoof is indistinguishable from a reconnect without identity — **measured, moved to N5** | identity-bound seats (Steam auth) — no address/timing guard is possible | L auth (N5) |
| 2 | **No authentication, no encryption** | a "player" is a `SocketAddr`; `session_id` is a client-chosen cleartext u64 (`net/src/session.rs:114-129`); zero crypto deps | Steam auth tickets + AEAD session (or SDR, which gives both) | L |
| 3 | **Unvalidated entries, lobby counts them, no rate limits** | every datagram from an unknown address allocates a `RemoteClient` BEFORE validation (`remote.rs:153-155`); lobby start counts `clients.len()` (`remote.rs:261,448`) — 7 junk datagrams start a battle and eat the human seats | validate-before-retain, client cap, per-address token bucket, count only seated | S–M |
| 4 | **Owner identity on third-party shells/impacts/shots** | `snapshot_filter.rs:62,76,81` clones `shells`, `shell_impacts`, `shots_fired` with `owner`/`shooter` intact — v41's `ShotFired` names every unspotted shooter on the map; behavior LOCKED by `net/tests/snapshot_filter.rs:39`; `docs/spotting-policy.md:30-31` claims the opposite of the code | opaque handles or drop the field (key presentation on `shell_id`); re-anchor the lock; fix the doc | M |
| 5 | **One battle per process** | single `Phase` enum (`remote.rs:86-89`), process exits after the battle (`server/src/main.rs:82-85`); restart-as-rotation is the documented model | battle-id routing, N battles per host, rotation without restart | M–L |
| 6 | **No discovery/join UI; silent SP fallback** | `WOT_CONNECT` env var only; connect failure silently falls back to a LOCAL BOT BATTLE (`client/src/app/mod.rs:563-601`) — on Steam, "play multiplayer" quietly becomes singleplayer | join screen with honest errors + server list/matchmaker | M (+L coordinator) |
| 7 | **No lag compensation for the shooter** | no position history anywhere; remote hulls render ~90–125 ms behind truth at 80–150 ms RTT (50 ms interp + one-way), ~1 hull width on a fast crosser; deferral documented `server-first-policy.md:71-77` | shooter turret/gun rewind with a cap + a LossyLoopback hit test at 0/80/150 ms; jitter buffer + clock sync | M |
| 8 | **Ops surface** | 125-line main, no metrics/health/drain/config/`--map` (`server/src/main.rs`) | metrics endpoint, SIGTERM drain, config file, map flag, panic artifacts | M |
| 9 | **Transport ceilings** — since 2026-09-06 a COST condition: the owner's ~1 000 online on one cheap Hetzner box holds only with delta snapshots (`docs/game-design.md` row 24) | fixed 1150 B MTU, no PMTU; lost fragment kills a whole snapshot (~13% at 2% loss on 7 fragments); ≤8 KB/snapshot → 1.3 Mbit/s per client ceiling (`net/tests/snapshot_budget.rs:169-196`); "future ack lane" bytes unused (`transport.rs:58`); combat-lane retransmit at blind 60 Hz | PMTU or smaller payload, loss estimate on the ack lane, RTO, delta snapshots | M–L |
| 10 | **Steam seam** | only `trait Transport` is ready (`transport.rs:8,37-41`); peers are `SocketAddr` throughout (`remote.rs:102`, `session.rs:56`) | `PeerId` abstraction, SDR transport impl, lobbies/tickets/packaging | M + L |
| 11 | **Client panics on map mismatch** | `assert!` on map-content divergence (`client/src/app/session.rs:400-405`) — a hostile or stale server crashes the client | readable refusal at the door | S |
| 12 | **Garage inert online** | `VehicleSelection` ignored by the host (`remote.rs:241`); humans get roster slots, slot 0 = BENCHMARK (`setup.rs:75-89`) | seat = chosen vehicle + loadout, server-validated | M |
| 13 | **No battle clock on the wire, no reconnect** | HUD hides the timer remotely (`session.rs:155-162`); no re-dial after a drop | clock in lifecycle messages; in-client reconnect to the freed seat | S each |
| 14 | **Dead config + doc drift** | `interpolation_delay_ticks` has no consumer (`net/src/lib.rs:138`); `spotting-policy.md` contradicts the filter | wire it or delete; correct the doc | S |
| ~~15~~ | ~~**Humans on one side only**~~ — **CLOSED (2026-09-06)** by game-modes M6: a snake by hello order across both teams, "full" = both teams' seats | `random_7v7_setup_for_humans` (`battle_host/src/setup.rs`) reserves the first `humans` slots of TEAM ONE and fills team two with bots; the lobby's "full" is one team's seats (`LOBBY_FULL_PLAYERS = SEATS_PER_TEAM`, `remote.rs`) — the dedicated host hosts co-op against bots, never humans against humans | seats dealt across both teams by the queue's rules (game-modes R4), "full" = both teams' seats; the per-viewer filter needs nothing | M (game-modes M6 — the FIRST online row: the owner, 2026-09-06, humans on both teams is the mode's definition) |
| 16 | **The largest format against the transport** (found 2026-09-06, `docs/game-modes.md` finding 3) | a saturated 14-tank snapshot was 5 245 B of 32 200 B (5/28 fragments); MEASURED at thirty (M5a): 8 029 B — the line re-based to 40 fragments (46 000 B), so 17.5 %, 7 fragments, 157 KiB/s per client at 20 Hz; a lost fragment still kills its snapshot (one in seven at 2 % loss) | the owner (2026-09-06): every budget sized for 14 tanks is raised to the largest format and the payload optimised — the transport's line re-based so a 30-tank snapshot keeps the quarter's headroom, then row 9's delta snapshots / smaller payload until it rides five fragments again; both in game-modes M5, before the format meets humans (M6, M7) | M–L (game-modes M5) |

## Wave plan (proposal)

- **N0 — Close the door (S, 1–2 PR). PARTLY LANDED.** Register rows 3, 11, 14, plus the parts
  of 1 that do not need identity. Shipped so far: validate-before-retain via a table cap
  (`MAX_TRACKED_CLIENTS = 32`, unknown sources dropped at the cap), unestablished-source fast
  aging (2 s vs 10 s), and lobby start / seat assignment counting only crews that completed a
  hello — so a spoofed-datagram flood can neither start the battle, take a seat, nor grow the
  table without bound. Locks: `unestablished_sources_neither_start_the_battle_nor_overrun_the_table`.
  **Measured finding on row 1 (seat hijack):** it CANNOT be closed at N0. A spoofed source is
  address-identical to the real client, and the host already treats a different session_id on a
  seated address as the fast-reconnect feature (`remote_reconnect.rs`) — keeping the tank on
  purpose. A timing/address guard cannot separate a reconnect from a takeover; it only breaks
  the reconnect. **Row 1 moves to N5 (authentication):** with a real identity the seat binds to
  the player, not the socket, and the takeover has nothing to spoof. A `NOTE` in
  `begin_session` records this so no one re-adds the broken guard. Still open in N0: the
  map-mismatch panic → refusal message, and deleting the dead `interpolation_delay_ticks`.
- **N1 — Honest wire (M, 1–2 PR).** Row 4: strip `owner`/`shooter` from third-party shells,
  impacts and `ShotFired` (presentation keys on `shell_id`); per-viewer boolean instead of the
  full `spotted_by_teams_mask`. Re-anchor the locking test to the new promise.
- **N2 — A server you can rent (M, 2–3 PR).** Rows 8, 13, 5(first half): `--map`/config file,
  metrics text endpoint, SIGTERM drain, battle clock on the wire, reconnect to the freed seat,
  battle rotation in-process (single battle at a time is fine; no restart between rounds).
- **N3 — Hit what you saw (M, 1–2 PR).** Row 7: shooter turret/gun-angle rewind with a
  ~200 ms cap (the primer's design), `LossyLoopback` hit-registration lock at 0/80/150 ms,
  RTT instrumentation surfaced in metrics.
- **N4 — Steam skeleton (M–L, 2–3 PR).** Row 10 first half + 6 + 12: `PeerId` replaces
  `SocketAddr` in host/session; join screen in the client (honest errors, cancel, no silent
  SP fallback); `VehicleSelection` honored server-side. SDR/auth tickets land here once the
  steamworks dependency is chosen.
- **N5 — Identity and scale (L).** Rows 2, 5(second half), 9: full auth handshake, AEAD or
  SDR, N battles per process, delta snapshots. Sized when N0–N4 have burned down.

Every wave lands with locking tests on the new promises and leaves `verify.ps1` green; wire
changes bump the protocol version additively (the v24/v38 discipline).

## Delta snapshots — the design (2026-09-06; register row 9, a COST condition since game-design row 24)

**Why now.** The owner's rulings of 2026-09-06: ~1 000 online, hosting cheap, 15v15 "mega
płynne". The wire today is a FULL snapshot at 20 Hz: a saturated 30-tank snapshot measures
8 029 B (`snapshot_budget.rs`, M5a) — 157 KiB/s per client, ~1.3 Gbit/s at 1 000 clients, past
one port and any fair-use line; and it rides 7 fragments, so one lost datagram in seven kills a
whole snapshot at 2 % loss. The sim fits one cheap box with room to spare (~2 % of a core per
15v15 battle); the wire is the only thing standing between the target and a fixed ~45 €/month.

**What is in the 8 029 B** (the saturated fixture, by weight): thirty `TankSnapshot` — pose
(position, yaw, pitch, roll, turret yaw and its velocity, gun pitch, the sprung hull's
velocities) beside SLOW state (hit points, modules, ammo counts, tracks, masks, reload,
dispersion, fires, crew) — about 4.5 KB; the crater ledger re-sent whole (384 × 5 B ≈ 1.9 KB)
and the cover phases (160 B) and scars, all of them PERMANENT and append-only; shells, events,
impacts, shots (small, transient).

**The design, four PRs, each a wire bump and a full gate:**

1. **The world's ledgers leave the snapshot** (the v39 pattern that already moved perforations):
   craters, cover phases and cover scars become reliable-lane events with a baseline on join,
   exactly as `ArmorBreachDelta` does today. The snapshot stops carrying anything that grows
   with the battle — its size becomes a function of the roster alone. Lock: the saturated
   snapshot's size does not depend on how many craters exist (the twin of the v39 lock).
2. **The pose is quantised**: angles as `i16` fixed point (seven fields: 14 B instead of 28),
   the hull's velocities likewise; position stays `f32` (the map is 1 000 m and the predictor's
   reconciliation snaps on centimetres). Lock: the round-trip error under the reticle's own
   resolution at 400 m; the replay fixtures regenerated once, deliberately.
3. **The slow state goes delta against a per-client baseline.** The client acks the newest
   snapshot it applied by piggybacking `acked_snapshot_seq` on the `InputBatch` it already sends
   sixty times a second (no new datagram; the transport's two reserved header bytes stay
   reserved). The host keeps the last sixteen snapshots it sent each client; a snapshot names
   its baseline seq and carries, per tank, a changed-field mask and only the changed slow
   fields; a client whose baseline fell out of the window gets a full snapshot on the next
   tick (self-healing; newest-wins stays: a delta whose baseline the client does not hold is
   dropped and the next full one lands within a tick). Lock: after a lost baseline the client
   converges within one snapshot; a two-client `LossyLoopback` battle at 2 % / 10 % loss never
   shows a stale slow field for longer than one snapshot interval.
4. **The measurement and the locks re-based**: the saturated 15v15 snapshot fits ONE datagram
   (≤ 1 150 B) — `a_full_snapshot_of_the_largest_format_fits_one_datagram` replaces the
   quarter rule as the largest format's line — and the per-client rate is recorded (target
   ≤ 25 KiB/s at 20 Hz: 1 000 clients ≈ 200 Mbit/s at the peak, inside one Hetzner line).
   `MAX_FRAGMENTS` stays at 40 for the join baseline and the battle-over word; the battle's
   steady state never fragments.

**What stays out of scope here**: the jitter buffer and clock sync (the interpolation alpha
freezes under jitter today) and the shooter's rewind (N3) — the "mega płynne" half of the
owner's ruling that is about TIME, not bytes; they follow this program in the netcode queue,
ahead of N4.

**Order in the register**: this program moves ahead of N3 and N4 — it is the cost condition
of the one-server hosting model the game is priced on.
