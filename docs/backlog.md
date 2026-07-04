# Engineering Backlog

From the 2026-06-03 deep review. The first fix pass is **done and gated green**
(`./scripts/verify.ps1`: fmt + clippy -D warnings + test + check + bench --no-run).
Every fix landed with a test that locks it.

Legend: `[x]` done · `[ ]` open · `[?]` needs your decision.

---

## Done — contact-honesty fix pass (2026-06-10)

From the 2026-06-10 deep review (empirically verified on the live sim). Every fix landed
with a locking test; a new engineering rule requires a negative test for every
contact-shape approximation.

- [x] **Phantom ramming** (`sim/ramming.rs`) — contact was a center-distance circle of summed
  half-lengths, so a clean 4.5 m side pass dealt 360+720 HP and threw both tracks, and a t-bone
  fired 1.6 m early. Now the same XZ SAT movement collides (`physics::tank_footprints_touch`)
  with one tick of closing distance as dynamic slop. Locked by `sim/tests/ramming_contact.rs`.
- [x] **Cover interpenetration** (`physics::cover`) — driving blocked a 1.6 m point radius, so a
  T-55A buried its nose 1.6 m (Jagdtiger 2.5 m) into buildings, putting the muzzle inside the
  wall. Cover now blocks the real oriented footprint (cover boxes are yaw-0 obstacles in the
  shared SAT). Locked by `physics/tests/cover_footprint.rs` + `sim/tests/cover_combat.rs`.
- [x] **Invisible cover** — static cover blocked movement/shells/camera but was never rendered;
  `client::battlefield_scene_mesh` now draws the exact sim boxes (per-kind colors). Locked by a
  mesh test; windowed client and both offscreen examples use it.
- [x] **Unbounded turret yaw** — `step_aiming` now wraps into (-PI, PI]; ten-minute traverse test.
- [x] **Cleanups** — dead `ModuleSlot::struck_by` removed, `digit_count` deduped into
  `hud_number`, reticle arc check reads `sim::MIN/MAX_GUN_PITCH_RAD`.
- [x] **Repo has history** — baseline commit + one commit per fix (was: zero commits).

## Done — combat honesty / protocol v12 (2026-06-11)

- [x] **Wrecks and allies block shells** — the shared trace splits live enemies (targets) from
  teammates + wrecks (absorbing blockers); both server and reticle preview use the same split
  (`TankSnapshot.team`, protocol v12). Locked by `sim/tests/shell_blockers.rs` and the
  reticle-sweep tests, including the negative lock (clean enemy hit emits no impact event).
- [x] **Shots no longer die silently** — every absorbed shell (terrain/cover/hull) emits a
  replicated `ShellImpact`; the server buffers them to the next snapshot like damage events,
  and the client draws short-lived impact puffs at the death point.
- [x] **Enemy health bars** now appear only over *live enemies* (team-filtered; no permanent
  "0" over wrecks, no bars over teammates).

## Open — from the 2026-06-10 deep review

- [ ] **Vehicle JSON assets are stale (all six)** — they predate facets/shell_type and nothing
  loads them; add a regenerate-and-compare gate in `quality` or drop the files.
- [x] **HUD vs aiming policy** — resolved by the hybrid honesty matrix in
  `docs/aiming-model-policy.md`: third person fully neutral (gun marker restored, no pen
  colors/mm/impact X), sniper mode speaks penetration.
- [ ] **Renderer contract layer is parallel fiction** — `WgpuRenderer`/`PipelineRegistry`/upload
  queues are used only by their own tests while `SceneRenderer` bypasses them; wire or demote.
  Same for stale debug-tools promises (no uncaptured-error handler on the live device) and dead
  `RenderSettings::vsync`/`limit_profile`; MSAA 4x failure should fall back to 1x, not abort.
- [ ] **Enemy health bars render through terrain** (no occlusion check) and show exact HP at any
  range — revisit together with spotting.
- [ ] **Combat hot path has no benchmark** (`step_shells`/SAT/ramming at 30 tanks, 100 shells).
- [ ] **From the systems audit (2026-06-10):** turret occupies the hitbox's full width (second
  turret OBB per vehicle is the fix), the Roof zone uses a horizontal normal (flat shots get a
  21.6 mm strip), the LowerPlate band covers ~40% of frontal height, Tiger II's pennable turret
  inverts its hull-down incentive, and `AIM_MAX_RANGE_M` (600 m) is short for a 1000 m map.

## Done — camera feel fix pass (2026-06-12)

From the 2026-06-12 camera audit. Every fix landed with a locking test.

- [x] **Sniper vertical axis was inverted vs third-person** (mouse back aimed *up*; a test even
  locked the wrong sign). Sniper now matches TPP sense; desired pitch no longer drifts in TPP
  and entering sniper aligns the view to the actual gun pitch. `client/app/input.rs` tests.
- [x] **Mouse sensitivity now scales with sniper FOV** (`look_sensitivity_scale`) — max zoom was
  ~20x too fast to aim with.
- [x] **Own vehicle hidden in sniper view** — the eye sits at turret-roof height inside the
  player mesh (`visible_render_tanks`).
- [x] **Discrete magnification ladder + scroll-through modes** (`camera/zoom.rs`): FOV steps
  18/12/8/5/3°, wheel sweeps one zoom axis boom↔sniper, WoT-style.
- [x] **Aim circle projected through the real FOV** — was `mrad * 18.0` clip constant (≈10x too
  big in TPP, ~2x too small at max zoom, never reacted to zoom).
- [x] **Free look release returns the camera to the aim** (yaw + pitch) instead of swinging the
  turret to wherever the player glanced; releasing Alt in sniper no longer teleports the aim.
- [x] **Wheel zoom ignored while the garage is open / battle not started.**

## Done — camera feel fix pass 2 (2026-06-13)

From the player-reported scroll wobble + barrel slide. Every fix landed with a locking test.

- [x] **Zoom rotated the whole scene** — the over-shoulder offset sat on the eye only, so the
  eye→target direction swung with the boom length and dragged the aim point sideways on every
  scroll click. The lateral offset now shifts the whole sight lane (target + eye), so the view
  direction is invariant under zoom. `third_person_view_direction_is_invariant_under_zoom`.
- [x] **Sniper barrel/world slid sideways on every mouse move** — the eye sat 1.35 m ahead of
  the turret and rode a lateral arc as the turret caught up. The eye now sits on the per-vehicle
  turret-ring axis (a point on the traverse axis doesn't translate when the turret slews), height
  from the vehicle trunnion — also retiring the global `sniper_eye_height_m`/`forward_offset_m`.
  `sniper_eye_does_not_translate_while_the_turret_traverses`.
- [x] **High-res/touchpad wheel stepped the ladder repeatedly** — fractional scroll events now
  accumulate to whole notches (`wheel_pending_lines`).
- [x] **Boom collision is an exact ray-AABB slab + terrain occlusion** — `CameraObstacle::
  segment_entry` replaces 32-point sampling (thin cover no longer tunnels), and the boom shortens
  in front of a terrain ridge instead of looking through it. Split into `camera/collision.rs`.
- [x] **Reticle traces deduplicated** — one sight sweep per fixed-tick batch feeds both turret
  and elevation (`SightSolution`), and one ballistic trace per frame feeds both reticle status
  and the pen hint (`reticle_report`). `AIM_MAX_RANGE_M` 600→1200 for the 1000 m map.
- [x] **Projection has one source of truth** — `CameraProjectionPolicy::webgpu_default()` is now
  `0.5/2000` and the client render path + both offscreen examples read it instead of hardcoding.

## Open — from the 2026-06-12 camera audit

- [ ] **No hull pitch/roll in `TankSnapshot`** — vehicles stay level on slopes, so camera, sniper
  sight and meshes ignore terrain tilt; needs sim+net+render work (biggest remaining camera-feel
  gap on a hilly 1000 m map).
- [ ] **No boom-length smoothing** — the slab/terrain cut is exact but instantaneous, so the eye
  still pops when an obstacle enters/leaves the line; add a critically-damped boom-length spring.
- [ ] **TPP max boom 18 m** — likely too short for the 1000 m Prokhorovka; retune with the map.

---

## Done — playable render slice (2026-06-03)

A real wgpu renderer now draws the battle; verified headlessly with offscreen PNG
screenshots (`cargo run -p client --example screenshot -- out.png`).

- [x] **Real GPU device/queue** (`renderer_wgpu/gpu_context.rs`) with software-adapter fallback.
- [x] **Offscreen render + readback** (`offscreen.rs`) — render to texture, copy back, save PNG.
- [x] **Camera view-projection** (`renderer_api/scene.rs::view_projection_matrix`) — real
  perspective with WebGPU `[0,1]` depth (`perspective_rh`), unit-tested (near→0, far→1).
- [x] **Lit scene** (`scene.wgsl` + `scene_pipeline.rs` + `scene_renderer.rs`): depth-tested,
  backface-culled terrain (height/slope colored) + tanks (hull/turret/gun) + shell tracers.
- [x] **Windowed path** (`window_renderer.rs`) — surface config, present, resize; client passes
  `Arc<Window>` without naming `wgpu`.
- [x] **Interactive client** (`client/src/app/{mod,input,render}.rs`): WASD drive, mouse aims the
  turret, Space fires, wheel zooms, 1/2 switch third-person/sniper.
- [x] **Terrain-aware authoritative server** (`server/local.rs`) — drives on the heightmap
  (`apply_commands_on_terrain`) and spawns on the map interior; combat test still passes.
- [x] **HUD** (`client/hud.rs` + `hud.wgsl`) — center crosshair + health bar overlay.

Smoothness + HUD (2026-06-04):
- [x] **Reload indicator** — `reload_remaining_s` added to `TankSnapshot`; HUD bottom-center
  reload bar (orange while reloading → cyan when ready).
- [x] **Snapshot interpolation** (`render_state.rs`) — render-time alpha lerps tanks (shortest
  angle) and extrapolates shells by velocity between 20 Hz snapshots; unit-tested.
- [x] **Client prediction + reconciliation** (`client/predict.rs`) — local hull predicted each
  frame via the physics controller, gently reconciled to each authoritative snapshot.
- [x] **Combat review fix pass** — knocked-out tanks stop accepting commands, custom loadouts
  preserve module health, module damage feeds client prediction, combat uses per-vehicle
  hitboxes, terrain sweep, turret side/rear armor, range falloff, and a golden non-empty combat
  snapshot fixture.
- [x] **Armor/shell/module deepening** — armor facets now carry slope and weakspot data, shells
  have AP/APCR/HEAT/HE behavior, `DamageEvent` carries cause/module data over protocol v8, ammo
  rack is a replicated module slot, HE can throw tracks, and high-speed ramming damages both tanks.

Renderer follow-ups (deferred):
- [ ] Predict/interpolate the turret yaw too (currently hull-only prediction + interpolated turret).
- [x] Mesh handle registry + per-object model matrix for baked vehicle submeshes.
- [ ] Real network transport (server still in-process); shadows; LOD; spotting/visibility.

## Done (2026-06-03 fix pass)

### Correctness bugs
- [x] **Heightfield collider scale & origin** (`physics/lib.rs`) — now uses `extent_m()`
  as Rapier scale + half-extent translation; test asserts ~750 m AABB aligned to origin.
- [x] **Client spiral-of-death** (`client/loop_policy.rs`) — `MAX_CATCHUP_TICKS = 8`, drops
  backlog after a long stall; test feeds a 5 s stall and asserts the tick count is bounded.
- [x] **Heightmap exact far-edge off-by-one + NaN guard** (`terrain/heightmap.rs`) — closed
  interval `[0, extent]` is sampleable; test samples exactly at `extent_m()`.
- [x] **Braking overrides throttle** (`physics/movement.rs`) — brake drives target speed to 0;
  test: throttle+brake decelerates.
- [x] **net: bincode trailing-bytes** (`net/lib.rs`) — shared `wire_codec()` (fixint, byte-stable
  vs the v2 hex fixture) that **rejects trailing bytes**; negative tests added (trailing /
  truncated / garbage discriminant / empty).
- [x] **economy arithmetic** (`game_core/economy.rs`) — uniform `saturating_*`; saturation test.
- [x] **server CLI** (`server/main.rs`) — `validate_args` → clean `Err`/exit 1 (was panic/101);
  `--max-ticks N` now runs exactly N (was off-by-one). Verified at runtime.

### Robustness / API / hygiene
- [x] **`RenderError: Display + std::error::Error`** (`renderer_api/lib.rs`) — `?`/`.context()` works.
- [x] **`FixedTimestep` stores integer Hz** as source of truth (`sim/timestep.rs`); dt derived on demand.
- [x] **`orbit_yaw_rad` wraps** into (−π, π] (`client/camera/controller.rs`) + getter + test.
- [x] **`HeightMap` fields encapsulated** (private + `width()/height()/cell_size_m()` getters).
- [x] **Removed dead client deps** (`egui`, `engine`, `physics` were unused in `client`).
- [x] **`engine::Time/Transform`** use `#[derive(Default)]`.
- [x] **Deleted stray `crates/net/tests/zz_temp_audit.rs`**.

### Tests / docs
- [x] **Replay regression tightened** — exact `x/y/yaw == 0` invariants + tight `z`/turret golden +
  double-run determinism check (was loose `>=` floors).
- [x] **`fixed_tick` test actually tests determinism** (double-run `assert_eq`).
- [x] **`snapshot_combat`** is now a real encode/decode wire roundtrip.
- [x] **Armor plate-selection test** (each `ArmorFacing` → its own plate).
- [x] **README** — fixed `convert-gltf` placeholder path; added `t54-1951` command.
- [x] **`debug-tools-policy.md`** — added "contract before the renderer exists" caveat.
- [x] **Note**: `#[serde(default)]` on `TankCommand::brake` is KEPT (it lets JSON replay fixtures
  omit `brake`); the review's "misleading" flag was about *bincode* wire-compat only — now
  documented in code. It is load-bearing for the replay fixtures (a test caught this).

---

## Open — deferred deliberately (mostly waiting on the renderer/transport milestones)

- [ ] **`PROTOCOL_VERSION` on the wire** — the local binary fixtures are at v8, but real transport
  still needs a version field in the frame header / handshake.
- [ ] **`latest_snapshot()` borrowing accessor** — low value until there's a render loop calling it.
- [ ] **`ServerTickConfig` redundant `server_tick_hz`** — cosmetic dedup.
- [ ] **glTF `convert` loads no geometry** — rename to "manifest summary" or load buffers.
- [ ] **`pipeline_registry` draw-compilation counter is dead** — wire it or drop the `== 0` asserts
  (revisit when the real renderer is built).
- [ ] **`tools` integration tests** — needs extracting `tools` logic into a lib crate first.
- [ ] **`map_plan` / large-world rebase are inert** — wire or clearly mark as roadmap stubs.

## Decisions

Resolved 2026-06-03:
- [x] **Toolchain** → pinned `nightly-2026-02-12` (`rust-toolchain.toml` + CI + README).
- [x] **`engine` crate** → keep it and wire into the render-side ECS later; the dead
  `client → engine` dependency edge was removed in the meantime.
- [x] **Penetration model** → AP/APCR/HEAT/HE semantics now cover normalization, ricochet,
  caliber overmatch, HE surface damage, range falloff, armor facets, and deterministic module hits.

Still open (your call when relevant):
- [?] **Neutral-steer**: should a stopped tank pivot in place? (`movement.rs` floor 0.18)
- [?] **Casemate vs turret**: add `ArmorFacing::CasemateFront` / vehicle-class tag (Jagdtiger).
- [?] **Projection**: build the real `[0,1]` perspective matrix now, or with the render step?
