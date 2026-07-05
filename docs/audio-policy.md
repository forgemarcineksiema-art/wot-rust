# Audio policy (procedural v1)

Every sound in the game is synthesized at runtime from DSP first principles.
There are no recorded assets, no sample banks, no audio files on disk — the
same rule the visual side lives by (procedural geometry, no imported models)
applied to the ear. A gun's report is derived from its caliber, an engine's
tone from its RPM and load, a hit's ring from what the shell did to the plate.

## The boundary

- **`crates/runtime/audio`** is the whole audible world and is renderer- and
  device-free: no `cpal`, no `wgpu`, no window. `AudioEngine` is a pure
  function from (events, control state) to interleaved stereo `f32` samples.
  Its noise sources run on the same splitmix64 family as the simulation's
  dispersion, so **the same event sequence renders bit-identically** — every
  design claim (caliber scaling, penetration tails, RPM fundamentals, stereo
  decorrelation) is locked by unit tests that analyse rendered waveforms
  (peak / RMS / zero-crossing / low-frequency share probes in
  `audio::voice`).
- **`client/src/audio_out.rs`** is the only device-touching audio code in the
  workspace: it opens the default cpal output and calls `AudioEngine::render`
  from the platform callback. **No output device means a silent game, never a
  crash** — headless runs, CI, and exotic sample formats all degrade to
  silence.

## Event flow (the FX-queue twin)

Gameplay code never talks to the device. It queues `audio::AudioEvent`s on
`ClientApp::pending_audio` exactly where the matching visual cue spawns
(snapshot ingest for shots/impacts, the reload crossing for the breech clack,
garage actions for UI clicks), and `flush_audio` drains the queue to the
engine **once per presented frame** together with the listener pose (battle
camera eye/facing), the player powerplant state (RPM / load / ground speed)
and the scene wind level (open field vs sheltered hangar). Headless, the
queue drains to nowhere; gameplay behaves identically with and without ears.

## Spatialization is honest physics

- Loudness falls off as `1/r` against an 18 m reference distance.
- Air absorption low-passes with distance (~20 kHz at the muzzle, ~2 kHz at
  half a kilometre) — far battles rumble, near ones crack.
- Bearing pans constant-power; screen-right is `(-forward.z, 0, forward.x)`,
  matching the renderer's right-handed, Y-up look-at.
- Sound **travels at 343 m/s**: a shot on the far flank flashes first and
  bangs more than a second later. This is a deliberate feel differentiator
  and an honest range cue. The one exception is the player's own gun, which
  answers the trigger instantly — the TPP camera sits ~15 m behind the
  muzzle and a 45 ms lag there reads as input latency, not distance.
- One-shot voices are budgeted (40); an over-budget barrage drops its
  quietest voice, never grows, and the master bus soft-clips.

## Remote powerplants

Every replicated tank hums: the `remote` module keeps one persistent engine
bed per vehicle in earshot (nearest 4 within 260 m), spatialized like a
one-shot but living across frames. A tank that dies or drops out of the
report spools its bed down instead of cutting. Ground speed is estimated
client-side from the snapshot pair (the wire replicates no velocity).
**Audio leaks nothing the renderer doesn't**: the snapshot is already
spotting-filtered per viewer, so an unspotted enemy has no engine sound
either — the audible world and the visible world are the same world.

## Terrain occlusion

The straight line ear -> source is scored against the heightmap once per
event (the audio crate itself knows no terrain — the client passes a 0..1
judgment): the deepest ridge intrusion over the sight line, with a meter of
grace, saturating at ~6 m. A masked source loses most of its level and its
highs close far harder than open-air absorption — only the low thud
diffracts over the hill. The player's own gun is never occluded.

## What stays out of v1 (known, deliberate)

- Surface-dependent track noise, HE-specific impact voicing.
- A volume/mixer options surface; master gain is a constant today.
