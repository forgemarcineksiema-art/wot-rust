# Product — the road from a good battle to a product people buy

**Status: approved by the user 2026-08-04.** Four decisions are binding and everything below
obeys them: a **micro budget** (~30–60 EUR/month — the Steam app fee, a VPS, a domain; no
commissioned art), **going public fast** (the name now, a Steam page within two months),
**buy-to-play at 20–25 EUR** (no free-to-play), and the release ladder **demo → Early Access
→ 1.0**.

This document exists because the last plan of this kind did not. The master plan "Do Wydania"
lived in a session memory file whose detail was later overwritten; its progress log stops at
PR #184 while master is past #461, and its meta/release waves — the ones that turn a battle
into a product — were never started. **Plans live in `docs/`, next to the code they govern.**

Context: `ROADMAP.md` (the whole picture and the honest gap list), `multiplayer-production-program.md`
(the networking half of the release verdict), `hala-2-program.md`, `art-direction-program.md`
and `world-scale-program.md` (the picture), `battle-first/program.md` (the battle itself).

Naming: four documents already use "W0–W5" for four different wave schemes. This program uses
**P** (market), **E** (engineering to the named builds) and **R** (retention) instead, and the
builds have names rather than numbers.

## 1. The verdict on "AAA"

AAA is a class of budget, not a class of quality: 100–500 people, 50–300 M USD, 3–6 years. A
solo author with AI agents will not produce that, and aiming at it would spread the work so
thin that the one thing this project already has — depth — would be the first casualty.

What the 2023–2026 market did show is the mechanism by which a tiny team beats a giant:
**two or three pillars executed better than the giant executes them, and everything else
deliberately narrow, without embarrassment.** Battlebit Remastered (three people) against
Battlefield, Valheim (five), Manor Lords (one).

So the target is **premium AA in an empty slot: honest tank PvP** — a game Wargaming cannot
copy without dismantling the economy that pays for it, because the ±25 % roll and the premium
shell are not bugs in their design, they are the product being sold.

The project's unfair advantages, both already real: an AI-native pace (460+ merged PRs since
May 2026) and a repo where **every promise is test-locked rather than marketed**. The second
one is also the marketing.

## 2. Market evidence (read 2026-08-04)

| Fact | Number | Why it matters here |
|---|---|---|
| World of Tanks, Steam | ~46 k concurrent (peak 240 k in Sept 2025, trough 43.8 k July 2026); EU/NA declining, Asia growing | The audience exists, is unhappy, and is reachable |
| The decade-old complaint | ±25 % RNG "punishes skill"; premium vehicles read as pay-to-win | Our two headline features answer the two headline complaints |
| **World of Tanks: HEAT** (Wargaming, new in-house engine, F2P, May 2026) | Mixed 51 % positive; peak 5.2 k concurrent, −82 % since; −54 % month over month | A new engine did not win those players back — **the problem is economic, not technical**, which is exactly the gap a small honest game can enter |
| Gunner, HEAT, PC! | ~6.5 M USD gross, 91 % positive — and it is **PvE only** | The "honest tank" niche pays. Nobody occupies the PvP half of it |
| Armored Warfare | PvP effectively dead, PvE-only population | No live competitor in the slot |

The wedge is current, not historical: HEAT proved that thousands of veterans will try a new
tank game in a single day. Wargaming could not keep them. Honesty is how we keep them.

## 3. What the product lacks (verified at master d733d8f, 2026-08-04)

The battle is not the problem. 3D armor plates, ricochet, module and track damage, rack
cook-off the crew can fight, spotting through real cover, destruction, procedural audio and
competent bots all exist and are test-locked. What is missing is everything *around* it.

| # | Gap | Evidence | Track |
|---|---|---|---|
| 1 | **The game has no name** — and the placeholder carries someone else's trademark | `ui_strings.rs:70` `WINDOW_TITLE = "WOT Rust Prototype"`; `README.md:1` | P |
| 2 | **Two humans have never played it** | `ROADMAP.md:54` "the maps have never met a second human" | E, P |
| 3 | **Multiplayer has no door** — entry is an env var, and a failed connect silently becomes a bot battle | `client/src/app/mod.rs:577,602,608` | E |
| 4 | **The garage is inert online** — the wire carries `ClientVehicleSelection` (`net/src/lib.rs:162,440`) and **no consumer exists in `battle_host`** | grep: only net's own tests read it | E |
| 5 | **Nothing records that a battle happened** — one banner line, no scoreboard, no stats, no identity | `hud/outcome.rs`; only local `garage.json` persists | R |
| 6 | **No settings of any kind** — sensitivity is a source constant | `client/src/app/input.rs:8-9` | E |
| 7 | **Polish cannot be rendered** — the glyph atlas bakes ASCII only | `ui_kit/src/font/bake.rs:42` `(0x20u8..=0x7E)` | E |
| 8 | **No packaging, no crash reporting, no Steam** | version 0.1.0 workspace-wide, zero git tags, no panic hook, no steamworks dependency | E |
| 9 | **The min-spec claim has no current evidence** | last MX330 numbers 2026-08-03: 96.7 % of the 16.667 ms frame **with no vehicles, no FX and no HUD**; five renderer features have shipped since without a re-measurement | E |
| 10 | **No go-to-market artifact of any kind** | no store copy, capsule plan, devlog script, press kit or playtest plan anywhere in the repo | P |

Networking's own register (identity, trust, lag compensation, coordinator) is not repeated
here — it lives in `multiplayer-production-program.md` and this program consumes it.

## 4. Track P — market

**Name (this week).** Criteria in priority order: carries the honest-steel identity; works in
English and Polish; searchable on Steam; clean against Wargaming and Gaijin marks (note
*Caliber* is a Wargaming title, and nothing public may keep "WoT"); a free .com or .gg.
Shortlist recommended: **Hull Down** (the position skill earns; no Steam collision found),
**Sabot** (five letters, pure physics), **Steel Verdict** (a verdict is a result, not a roll).
Also on the list: Honest Steel, Overmatch, Mantlet, Stalwart, Glacis, Steel Will, Bystra,
Zero Roll. Whatever wins, the tagline carries the identity: **"Armor doesn't lie."**
Process: user picks three → screening (EUIPO/TMview classes 9 and 41, USPTO, Steam and
SteamDB search, domains) → decision → domain (~15 EUR). An EUIPO registration (~850 EUR) is
deliberately deferred to first revenue; screening plus dated public first use covers Early
Access.

**Steam presence (page live ~mid-September).** Buy the app fee on day one: it is 100 USD,
recoupable at 1 000 USD of adjusted gross, and it unlocks depots, builds and — decisively for
a solo developer — **Steam Playtest, which is free and turns testers into an auto-updating
audience** instead of a mailing list for hand-built zips. Capsules come out of the existing
`--example probe` renders: one recognizable tank silhouette, three-quarter from a low camera,
title in two words of a heavy OFL grotesque, **no battle scenes** (they are mud at 231×87).
Store copy leads with the wedge — "Tank PvP with no dice. You hit because you aimed." — and
every bullet under it is a promise with a test behind it. **No competitor is ever named on
the store page or in a devlog; the comparison is the audience's to make.**

**Devlogs (monthly, from the page going live).** Five to eight minutes, English with Polish
subtitles, **the author's own voice** — a synthetic narrator is free ammunition for the
"AI slop" accusation, and an accented human is an asset. The first eight episodes are already
sitting in the repo as finished systems: the manifesto (100 shots, one group), the vehicle
forge (RON to tank), the ammo rack that cannot lie, one look on a 300-EUR laptop, the map
forge, bots that flank, sound with no sound files, and why 7v7 and eras. Distribution: YouTube
→ a Steam event → Discord → `r/rust` and `r/rust_gamedev` (a real Rust game is front-page
material there), Polish outlets (arhn.eu), and the English tank channels that cover indie
armor (ConeOfArc first).

**Community and playtests.** A small bilingual Discord from day one. Recruit 20–50 ex-WoT
veterans where they actually are (r/WorldofTanks, WoTLabs, Polish clan Discords), run a fixed
weekly evening slot, and measure four things: week-over-week return (**>50 %**), median
session (**>45 min**), "I hit because I aimed" agreement (**>80 %**), and pay intent at 20 EUR
(**>40 %**). Consent and retention are stated in the signup form; telemetry is
pseudonymized and server-side.

**Demo and Next Fest.** The demo is **offline against bots** — zero server risk, and the bots
are good enough to be a feature rather than an apology. Target the **February 2027 Next Fest**
(22 Feb – 1 Mar); October 2026 leaves no runway for wishlists and a title gets exactly one
festival, ever. A **go/no-go gate on 30 November 2026** decides it on playtest metrics, while
moving is still free.

## 5. Track E — engineering to three named builds

The spine: **buy the Steam app on day zero, but run the first playtest over plain UDP to a
5-EUR VPS**, because `ops/dedicated-server.md` already works. Steam then arrives in three
roles, in this order: distribution and updates (Playtest, at M-PT), **identity** (auth tickets
— a SteamID64 seat closes the hijack row that cannot be closed any other way, and unlocks
persistence), and only before Early Access **transport** (SDR gives encryption, NAT traversal
and DDoS shielding for free). The demo is offline, so SDR is never on the critical path.

**Gate zero, before anything is tagged.** A `full_battle_probe` — fourteen tanks fighting, FX
and HUD on, p50/p95/p99 — and a ritual: **every named build is measured on the MX330 before it
is tagged**, with the number written into `battle-first/measurements.md`. The one-look policy
is a promise the project currently cannot evidence; this makes it evidence again. If the
number is over budget, the build gets cheaper — never a quality option.

- **M-PT "First Contact"** (~19–24 PRs): the connect flow becomes a state machine with honest
  errors (the silent singleplayer fallback dies), one PLAY ONLINE button instead of an env
  var, the garage choice honored online, client reconnect, a **results screen** (the slot
  where Track R later renders), volume and sensitivity settings, a panic hook that writes a
  crash file, Steam identity, depots and Playtest, server-side playtest telemetry, map
  rotation. Deliberately waiting: lag compensation, PMTU, crypto, rebinding, music, Polish.
- **The look wave** (while the playtest runs): the dark plane on clear and overcast — eleven
  of sixteen `LOOK DEBT` lines — then trees at scale, the camera FOV and Prokhorovka's absent
  horizon. This is now marketing critical path: the store page is made of these frames, and
  Hala 2.0's garage is the hero shot.
- **M-DEMO** (~14–18 PRs): a demo content boundary behind a build flag, the **Poligon**
  (`PracticeDuel` exists in `battle_host` and is unreachable from the client — it becomes the
  training range), first-battle hints, **Polish localization** (the glyph atlas keyed by
  `char` instead of `u8` — the refactor that also makes German and Russian cheap), release
  profile and versioning discipline.
- **M-EA** (~32–42 PRs): SDR behind `trait Transport`, a **coordinator** (its own binary,
  SQLite, ticket validation, an in-repo Weng-Lin/OpenSkill rating, queue and assignment,
  bots filling at the deadline),
  N systemd processes rather than multi-battle, persistence, and **lag compensation, which is
  decided here and the recommendation is to build it**: a public population brings 80–150 ms,
  the store page promises what-you-see-is-what-you-shoot, and the deferral's own numbers
  (~90–125 ms of visual lead) break that promise in public. Player-facing replay is cut from
  Early Access; `FrameRecorder` stays a support tool.

## 6. Track R — honest retention

WoT's retention engine is manufactured scarcity of power. Ours cannot be a gentler treadmill,
because the player we are building for already quit one. The replacement is **accumulated
proof**: nothing ever makes the tank stronger, and everything certifies that the player got
better. Tagline: *your account does not grow stronger, it grows truer.*

This is affordable because the simulation already emits an authoritative forensic stream —
`DamageEvent` carries impact angle, effective versus nominal armor, penetration, zone, facing,
ricochet and cause, and v38 delivers personal truth exactly once. **The whole meta is a fold
over that stream plus a seven-table SQLite. No new combat code.**

- **Debrief (EA)** — the verdict with its reasoning ("your team was the 46 % underdog"; a
  matchmaker that can show its math is itself the anti-WoT statement), a team table carrying
  only facts the sim can testify to, and **per-shot forensics**: *penetration — IS-3 upper
  front, 110 mm at 61°, 228 mm effective against 244 mm of penetration*. A kill-cam that is a
  spreadsheet, and the spreadsheet is true. No other game in the genre can print this
  honestly, because every number in it went through a roll.
- **Service record (EA local, 1.0 synced)** — append-only receipts; aggregates are always
  recounted from them, and every statistic shows its formula.
- **Mastery** — per-vehicle qualifications against absolute, published thresholds (works at
  twenty players, never decays), then percentile rings whose formula and current thresholds
  are published, computed over a median of thirty battles so no login pressure exists.
- **Rating and seasons** — the rating is visible with its uncertainty and **the matchmaker
  queues on the number it shows**; quarterly seasons are content beats with a σ-inflation soft
  reset. Never a battle pass, an XP weekend, or timed power.
- **Collection without grind (1.0)** — kill rings on the barrel, mastery emblems, camouflage
  earned by map mastery, an etched service plate in the hangar. Two rules printed in the UI:
  **paint is paint — 0 % to anything**, and what is earned is never sold.
- **Poligon (EA, and in the demo)** — five sixty-second exercises that teach a WoT veteran
  what to *unlearn*; every step skippable, because a twelve-year veteran must never be
  marched through a tutorial.
- **Bots, labeled (EA)** — callsigns carry `[SI]`, the pre-battle screen says how many seats
  the AI holds **before** the player commits, and a battle is rated only with three or more
  humans a side. Hiding bots buys a fuller-looking lobby once and costs the entire honest
  position the first time someone notices. **The label is the marketing.**

One contradiction this program resolves: `ROADMAP.md` listed "module unlocks without
pay-to-win" as meta work. Modules carry real stat deltas, so gating them behind time *is*
power behind time. There are no unlocks, ever — the line is corrected in this PR.

## 7. Calendar

| When | Engineering (E/R) | Product (P) |
|---|---|---|
| Aug, week 1 | Hala 2.0 T1b/c · **gate zero: `full_battle_probe` + an MX330 measurement** | **name decided** · Steamworks account and tax interview · domain · Discord |
| Aug, weeks 2–4 | the M-PT wave · a friends-and-family smoke on the VPS | app fee → appID · rename of public artifacts · capsules v1 · tester recruiting |
| Sept | **M-PT tagged; the first battle between two humans** · the look wave | store copy · **page submitted, then live with devlog #1** · testers move to Steam Playtest |
| Oct–Nov | the M-DEMO wave · weekly playtests feeding fixes | devlogs #2–3 · outreach · **demo scope frozen 15 Nov** · **Next Fest go/no-go 30 Nov** |
| Dec–Jan | M-EA: coordinator, matchmaking, persistence, lag compensation; Track R's debrief and service record | devlogs #4–5 · festival registration · demo page assets |
| Feb 2027 | Early Access candidate | demo page live · **Next Fest 22 Feb – 1 Mar** |
| Mar–May 2027 | hardening from festival data; week-one hotfix readiness | **Early Access gate and price** · press and curator keys · **launch** |

Roughly 100–120 PRs to Early Access across E, R and the remaining Hala 2.0, planned at 12–15
PRs a week against an observed ~35 — the difference is the room ops, playtests and marketing
will actually take.

## 8. Decision gates

Everything else in this program is executable without asking. These are not.

| # | Decision | Due |
|---|---|---|
| 1 | **The name** — three candidates chosen, then screened | this week |
| 2 | The `steamworks` dependency and the 100-USD app fee (the autonomy boundary N4/N5 names) | day zero |
| 3 | Accepting the residual risk of a closed playtest on cleartext UDP until SDR lands | at M-PT |
| 4 | Devlog narration: the author's voice (recommended) or an alternative | before 1 Sept |
| 5 | Crew proficiency in rated battles (recommended: fixed at 1.0, the slider survives on the Poligon) and whether barrel kill rings are visible to enemies | with the debrief design |
| 6 | The demo's content boundary — era, vehicles, maps | 15 Nov |
| 7 | Next Fest February: go or slip | 30 Nov |
| 8 | The Early Access date and price (19.99 / 22.99 / 24.99 EUR), read off pay-intent data | Mar 2027 |
| 9 | **Business form and the legal name** — the copyright holder a `LICENSE` file and a store page must both carry, the tax form, and an accountant who knows Steam revenue. `Cargo.toml` says `license = "Proprietary"` and no `LICENSE` file exists; that stays deliberate until a name exists, rather than being guessed here | before the first payout, and before the store page |

## 9. Release criteria — the numbers nothing in this repo had declared

The user's own release rule stands: the decision is a **quality verdict on hard evidence**,
and readiness is never announced without it. These are the thresholds that verdict reads.

| Criterion | Minimum | Comfortable |
|---|---|---|
| Wishlists on the Early Access date | **7 000** | 10–15 000 |
| Playtester week-over-week return | >50 % | >60 % |
| "I hit because I aimed" | >80 % agree | >90 % |
| Pay intent at 20 EUR | >40 % | >55 % |
| First 50 reviews | **≥90 % positive** | — |
| Frame time, MX330, full battle | ≤16.667 ms p95 | — |
| Infrastructure | <40 EUR/month | — |

Funnel arithmetic, stated honestly: week-one sales run 12–22 % of wishlists, and about 12 EUR
of a 22-EUR sale survives VAT, Valve's share and refunds. 7 000 wishlists is therefore roughly
840–1 540 copies and **10–19 k EUR net in week one** — validation and a year of runway at
micro costs, not a salary. Below 4 000 wishlists four weeks out, the date moves; the date is
ours, the festival is not.

Success at AA scale is not WoT's concurrency. It is **5–15 thousand players who bought once,
stayed, and told someone** — the goal recorded when this game's market position was first
written down.

## 10. Risks

1. **The frame budget is already spent on min spec** (96.7 % with nothing in the scene) — gate
   zero, a measurement before every tag, and a slimming wave pre-planned.
2. **Multiplayer cold start** — labeled bots as a first-class feature, an offline demo, one EU
   region, fixed peak hours; and the first two-human battle happens in September, not at launch.
3. **Trademark exposure** — no competitor's marks anywhere, generic comparisons only, "WOT"
   removed from public artifacts, screening before the name is committed.
4. **The "AI slop" accusation** — radical disclosure defuses it: procedural authored code is
   not a generative asset pipeline, the simulation's depth is the counter-evidence, and the
   Steam disclosure is answered more fully than required (tooling assistance is exempt; we
   state it anyway).
5. **One festival, one chance** — the 30 November gate, decided on measurements.
6. **Bus factor of one** — every operation is a runbook in `docs/`, one devlog always in
   reserve, and this program lives in the repository rather than in a chat.

## 11. What this program does not cover

The battle's own remaining work (`battle-first/program.md`), the picture
(`art-direction-program.md`, `world-scale-program.md`, `hala-2-program.md`) and the networking
register (`multiplayer-production-program.md`) keep their own documents. This program
sequences them against dates and names the evidence that ends each one.
