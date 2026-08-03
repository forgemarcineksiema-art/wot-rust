# Multiplayer Production — the road from v43 to a Steam battle

**Status: register measured 2026-08-03; program proposed, wave order awaiting the user's
sign-off.** The release target is confirmed: multiplayer on Steam, and the release decision
itself is a QUALITY verdict. This document is the networking half of that verdict: what a
production battle between humans still lacks, with evidence, and in what order it gets built.

Context docs: `w1-networking-primer.md` (the v38 foundation), `server-first-policy.md`
(authority doctrine), `ops/dedicated-server.md` (today's runbook). v38–v43 hardened the
SESSION — envelope, ACK lanes, reliable combat channel, terminal gate, wire budget. Nothing
since has touched **discovery, identity, trust, or ops**.

## The debt register (evidence at file:line, risk-ordered)

| # | Area | Current state | Production needs | Scope |
|---|------|---------------|------------------|-------|
| 1 | **Seat hijack via spoofed hello** | any new `ClientHello` from a seated address re-keys the session and KEEPS the tank (`battle_host/src/remote.rs:57-79,154-155,177-204`); the victim's own packets are then rejected | refuse re-keying a live seated session; server nonce/cookie; identity-bound seats | S mitigation / L auth |
| 2 | **No authentication, no encryption** | a "player" is a `SocketAddr`; `session_id` is a client-chosen cleartext u64 (`net/src/session.rs:114-129`); zero crypto deps | Steam auth tickets + AEAD session (or SDR, which gives both) | L |
| 3 | **Unvalidated entries, lobby counts them, no rate limits** | every datagram from an unknown address allocates a `RemoteClient` BEFORE validation (`remote.rs:153-155`); lobby start counts `clients.len()` (`remote.rs:261,448`) — 7 junk datagrams start a battle and eat the human seats | validate-before-retain, client cap, per-address token bucket, count only seated | S–M |
| 4 | **Owner identity on third-party shells/impacts/shots** | `snapshot_filter.rs:62,76,81` clones `shells`, `shell_impacts`, `shots_fired` with `owner`/`shooter` intact — v41's `ShotFired` names every unspotted shooter on the map; behavior LOCKED by `net/tests/snapshot_filter.rs:39`; `docs/spotting-policy.md:30-31` claims the opposite of the code | opaque handles or drop the field (key presentation on `shell_id`); re-anchor the lock; fix the doc | M |
| 5 | **One battle per process** | single `Phase` enum (`remote.rs:86-89`), process exits after the battle (`server/src/main.rs:82-85`); restart-as-rotation is the documented model | battle-id routing, N battles per host, rotation without restart | M–L |
| 6 | **No discovery/join UI; silent SP fallback** | `WOT_CONNECT` env var only; connect failure silently falls back to a LOCAL BOT BATTLE (`client/src/app/mod.rs:563-601`) — on Steam, "play multiplayer" quietly becomes singleplayer | join screen with honest errors + server list/matchmaker | M (+L coordinator) |
| 7 | **No lag compensation for the shooter** | no position history anywhere; remote hulls render ~90–125 ms behind truth at 80–150 ms RTT (50 ms interp + one-way), ~1 hull width on a fast crosser; deferral documented `server-first-policy.md:71-77` | shooter turret/gun rewind with a cap + a LossyLoopback hit test at 0/80/150 ms; jitter buffer + clock sync | M |
| 8 | **Ops surface** | 125-line main, no metrics/health/drain/config/`--map` (`server/src/main.rs`) | metrics endpoint, SIGTERM drain, config file, map flag, panic artifacts | M |
| 9 | **Transport ceilings** | fixed 1150 B MTU, no PMTU; lost fragment kills a whole snapshot (~13% at 2% loss on 7 fragments); ≤8 KB/snapshot → 1.3 Mbit/s per client ceiling (`net/tests/snapshot_budget.rs:169-196`); "future ack lane" bytes unused (`transport.rs:58`); combat-lane retransmit at blind 60 Hz | PMTU or smaller payload, loss estimate on the ack lane, RTO, delta snapshots | M–L |
| 10 | **Steam seam** | only `trait Transport` is ready (`transport.rs:8,37-41`); peers are `SocketAddr` throughout (`remote.rs:102`, `session.rs:56`) | `PeerId` abstraction, SDR transport impl, lobbies/tickets/packaging | M + L |
| 11 | **Client panics on map mismatch** | `assert!` on map-content divergence (`client/src/app/session.rs:400-405`) — a hostile or stale server crashes the client | readable refusal at the door | S |
| 12 | **Garage inert online** | `VehicleSelection` ignored by the host (`remote.rs:241`); humans get roster slots, slot 0 = BENCHMARK (`setup.rs:75-89`) | seat = chosen vehicle + loadout, server-validated | M |
| 13 | **No battle clock on the wire, no reconnect** | HUD hides the timer remotely (`session.rs:155-162`); no re-dial after a drop | clock in lifecycle messages; in-client reconnect to the freed seat | S each |
| 14 | **Dead config + doc drift** | `interpolation_delay_ticks` has no consumer (`net/src/lib.rs:138`); `spotting-policy.md` contradicts the filter | wire it or delete; correct the doc | S |

## Wave plan (proposal)

- **N0 — Close the door (S, 1–2 PR).** Register rows 1(mitigation), 3, 11, 14. No new
  systems: refuse live-seat re-keys behind a server nonce, validate-before-retain + client cap
  + token bucket, count only seated clients for lobby start, replace the map-mismatch panic
  with a refusal message, delete dead config, fix the spotting-policy paragraph. Locks:
  spoofed-hello test (hijack refused), junk-datagram test (lobby does not start), mismatch
  test (client survives with a message).
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
