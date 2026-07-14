# Dedicated server — deploy & playtest (N6)

The first-playtest runbook: one VPS with a public IP, up to seven human crews, bots in every
empty seat. No NAT punching, no coordinator — the lobby address is handed out by hand. The
coordinator/matchmaking arrives in wave W3; Steam Datagram Relay (public NAT-free play) in W5.

## What travels

- **One UDP port** (default `40000`), datagrams ≤ ~1.2 KB. No TCP, no HTTP, nothing else.
- The map never crosses the wire — both sides regenerate it from `map_id` in `ServerHello`.
- Every snapshot leaves the host already filtered per viewer (anti-wallhack is server-side).

## Server (Linux VPS)

```sh
# Build on the box (or cross-compile and copy the binary):
cargo build --release -p server

# Smoke it by hand first:
./target/release/server --bind 0.0.0.0:40000 --lobby-wait-s 30 --seed 7
```

- `--lobby-wait-s` — how long the lobby waits before starting with bots in the empty seats.
- `--seed` — deterministic battle seed; `0` derives one from the clock.
- Firewall: allow **UDP 40000 in** (e.g. `ufw allow 40000/udp`). Nothing outbound is needed.
- Watch the log: `tick over budget` warnings mean the box is too small or a battle bug —
  either way, report it with the tick numbers.

### systemd unit (`/etc/systemd/system/wot-server.service`)

```ini
[Unit]
Description=WOT dedicated battle server
After=network.target

[Service]
ExecStart=/opt/wot/server --bind 0.0.0.0:40000 --lobby-wait-s 30
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info
User=wot
WorkingDirectory=/opt/wot

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload && sudo systemctl enable --now wot-server
journalctl -u wot-server -f     # the server's story, live
```

## Clients

```sh
WOT_CONNECT=<VPS_IP>:40000 cargo run --release -p client
```

- The client waits for the lobby to seat it; any failure falls back to a LOCAL battle — if you
  ended up fighting bots on Bystra without a lobby wait, the connect failed (check the log for
  `remote connect failed` / `the lobby never seated us`).
- `WOT_RECORD=battle.rec` additionally records every accepted frame; attach the file to any bug
  report — it replays through the same ingest the live game uses.
- The `net` log line (every 2 s) is the console net-HUD: `rtt_ms`, `snapshot_age_ms`,
  `server_tick`. RTT under ~60 ms on regional hosting is the design point.

## Smoke checklist (2–4 players, ~15 minutes)

1. **Join**: all clients seat within the lobby wait; each is assigned a DIFFERENT tank
   (team one, slots in join order); bots fill the rest and the battle starts for everyone at
   once.
2. **Honesty**: before contact, nobody sees red silhouettes or minimap marks beyond own-team
   vision (snapshots carry ~7 of 14 tanks — verify with `WOT_RECORD` if in doubt).
3. **Combat**: trade shots between two humans — hits, penetrations, breach glow and module
   damage must match what the shooter and the target each see from their side.
4. **Disconnect**: kill one client mid-battle. The battle must keep running; the abandoned tank
   idles after ~half a second of held commands. Restart that client with the same
   `WOT_CONNECT` — it must inherit the freed seat mid-battle and converge instantly.
5. **Finish**: play to elimination or the 10-minute clock; every client gets the same outcome
   screen (`BattleEnded` repeats; a lost datagram may delay it a snapshot or two).
6. **Collect**: `journalctl -u wot-server`, every client's `net` lines and any `WOT_RECORD`
   files. Network bugs become `LossyLoopback` fixtures — that is the pipeline.

## Known limits of this slice (tracked, deliberate)

- Battle clock is not replicated — remote HUD hides the timer (v30 candidate).
- One battle per process; restart the unit between rounds (`Restart=on-failure` also re-arms
  the lobby after the process exits on its own).
- Host:port is typed by hand (garage UI field is a tracked follow-up; the coordinator replaces
  both in W3).
