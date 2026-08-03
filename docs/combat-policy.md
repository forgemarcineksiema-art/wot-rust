# Combat Policy

Combat is server authoritative. `TankCommand.fire` is input intent only; the
server simulation decides reload, projectile spawning, hit detection,
penetration, hull/module damage, and the resulting `Snapshot` data.

## Fire And Reload

Each tank owns a reload timer derived from its gun spec. A fire command creates a
projectile only when reload is ready, the tank is alive, the gun works, and the
ammo rack is not destroyed. Firing starts reload immediately. Client frame time,
renderer state, and camera mode do not affect reload.

Two player-visible rules ride on that timer. A fire click landing inside the last
`FIRE_BUFFER_S = 0.3` s of the reload is HELD and released the tick the breech closes
(`crates/runtime/sim/src/combat.rs:40` — sized to human anticipation timing, not to hide
the reload); earlier clicks are genuine misfires. And switching ammunition restarts the
FULL reload — the loader swaps the round out of the breech — and cancels any held click
(`crates/runtime/sim/src/state.rs:427-435`).

## Shell Types

`ShellSpec` carries a deterministic `ShellType`:

- AP is the baseline kinetic round with normalization, overmatch, and moderate
  distance falloff (it holds its speed).
- APCR has higher velocity/close-range penetration, weaker normalization, lower
  module damage, and harsher distance falloff (its light core bleeds speed
  fastest).
- HEAT keeps penetration over distance and uses a high ricochet threshold, but
  does not overmatch and pays a multiple against spaced screens.
- HE never ricochets and fuzes on the first surface it touches. If it fails to
  penetrate, it still applies small external hull damage, can critically damage
  exposed running gear, and its `explosive_radius_m` throws splash damage
  (`DamageCause::Splash`) at every vehicle inside the radius — attenuated
  linearly by distance to the hull and soaked by the external plate facing the
  burst (roof from above, glacis from ahead, side from the flank). Allies are
  protected exactly like direct fire; the owner's own HE can
  hurt the owner.

Kinetic penetration is a function of TRAVELLED DISTANCE, converted to speed in
closed form — nothing reads the projectile's live velocity vector at impact.
`resolve_impact_penetration` (`crates/runtime/sim/src/combat.rs:527-533`) passes
`distance_m` only, and `ShellSpec::penetration_mm_at_distance`
(`crates/foundation/game_core/src/weapon.rs:130-142`) recomputes the impact
speed as `muzzle_velocity − drag · distance` and takes a De Marre-style power
of the speed ratio. The approximation is honest because it is CONSISTENT: the
server resolution, the reticle's penetration preview, and the HUD readout all
use the same formula, so the player is never promised a number the server will
not deliver — but it ignores the speed gravity adds back on a plunging arc (the
flown arc itself uses the shared `game_core::math::integrate_shell_step`, with
real drag and gravity). Chemical rounds (HEAT/HE) ignore impact speed.

There is no random damage roll — ever; this is a standing design law, not a
slice limitation. Damage, module damage, ricochet, overmatch, and range falloff
are replay-stable functions of the shell, armor facet, impact angle, and
travelled distance.

## Aim Time And Dispersion

Each live tank owns current aim dispersion in milliradians. The gun's base
dispersion is the settled minimum; aim time recovers bloom toward that minimum.
Movement, steering, turret traverse, gun elevation, and firing increase bloom.
Partial gun damage raises the minimum dispersion, while a destroyed gun prevents
firing entirely. When the server spawns a shell it applies deterministic,
center-biased dispersion to the current barrel direction, so replay and network
outcomes stay reproducible.

## Projectiles And Hits

Projectiles are fixed tick simulation objects. They store owner, shell spec,
position, velocity, age, and travelled distance. Hit detection sweeps the
projectile segment for the tick against terrain, static cover objects
(axis-aligned boxes such as buildings and treelines), and vehicle hitboxes so
fast shells do not skip targets or pass through ridges and cover. The nearest
impact wins. Only live enemies resolve as damage; live teammates and every
wreck are *blockers* — they absorb the shell without taking damage, so a dead
hull is hard cover and a friendly hull cannot be shot through. Every absorbed
shell (terrain, cover, or a blocking hull) emits a replicated `ShellImpact`
with its surface and position, so the firing client always learns where a shot
died instead of the shell silently vanishing between snapshots. Shells that
expire into open sky emit nothing.

## Shared Shell Trace

Shell flight has exactly one implementation, `sim::shell_trace`, used by every path
that needs to know where a shell goes: the authoritative server step
(`step_shells`), the client's reticle impact and penetration preview, and the
client's straight aim-ray sweep. The per-segment collision (`segment_impact`:
analytic ray versus two hull-local boxes per tank, a one metre stepped sweep for
terrain, and slab tests for cover) and the ballistic integration (`trace_shell`:
semi-implicit gravity-then-move at the simulation tick `dt`) are the same code for
all of them.

Blueprint vehicles resolve against BAKED CONVEX ARMOR VOLUMES
(`game_core::vehicle_armor_volumes`, built from the same `VehicleBlueprint`
numbers the visible plates are generated from — what you see is literally what
you shoot). A vehicle is a set of zone-tagged half-space volumes: the upper
hull (glacis/deck/sides/rear above the sponson fold), the lower tub (nose
plate/belly/tub sides), each track band as its real belt box from
`TrackShape`, and the cast turret as a ring of sloped sector planes whose
normal sweeps around the casting. The ENTERING plane of the nearest volume is
the struck plate: its true normal, its zone, and any weakspot patch riding on
it — the mantlet is a real circle centered on the gun line, not a width band.
Consequences are physical: the narrow T-54 hull leaves honest air between the
hull wall and the hitbox width (shots threading over the tracks fly on), the
deck is the real 1.58/1.30 m hull roof, and dome cheeks auto-glance where the
casting curves away. The hitbox stays as the broad phase only.

Legacy (non-blueprint) vehicles keep the two-box band model: below the armor
split (`turret_min_y`) the full-plan hull slab applies; above it only the
per-vehicle *turret box* (`HitboxProfile::with_turret_plan`) connects, and it
traverses with the turret about the ring axis (`TraceTank::for_kind` sources
the pivot from `MountFrames` so it cannot desync). The turret plan is sized to
the visual turret submesh and `vehicle_geometry`'s turret fit/fill test keeps
the numbers honest. Casemates hold traverse at zero, so their box stays fixed.
This path (`shell_trace/legacy_boxes.rs`) shrinks one vehicle at a time as the
fleet migrates onto blueprints.

Shells also *spawn* at the visible muzzle: `game_core::math::muzzle_world_position`
pivots the muzzle mount about the trunnion (pitch), the turret ring (traverse),
and the hull origin (yaw) — the same chain the renderer applies to the gun
submesh — and the client reticle uses the identical function, locked by
`muzzle_position_matches_server_shell_origin`. Dispersion perturbs only the
velocity direction, never the spawn point.

This is a hard rule, not a convenience. If the preview used a different timestep,
integration order, or intersection test, the reticle would predict an impact the
server resolves elsewhere, and under input latency the player would see hits the
server never confirms. The reticle matches the authority; it is not allowed to be
independently "more precise". Both sides split the battle into neutral `TraceTank`
sets with the same rule — live enemies are damageable targets, live teammates and
wrecks are absorbing blockers (the server from `TankState`, the client from
`TankSnapshot`, which carries the team since protocol v12) — so the preview and
the authority agree on friendly and wreck blocks, not just on trajectories.

Shell collision is a swept BODY, not an infinitely thin ray. Its radius is half the authored
caliber (clamped only against malformed or modded extremes). Convex armor volumes are expanded by
that radius plane-by-plane, which is the exact sphere-vs-convex Minkowski sweep; the reported hit
point is then projected back onto the original armor plane. Terrain, water, cover, legacy hitboxes,
the authoritative step, and the reticle preview use the same radius. Camera and selection rays pass
zero radius and remain true sight lines.

## Armor And Damage

Combat uses per-vehicle oriented hitboxes with armor facing resolution for hull
front/side/rear and turret or casemate front/side/rear. `ArmorProfile` keeps
the six readable thickness values plus `ArmorFacet` data: nominal thickness,
plate slope, and a weakspot multiplier.

Plate slope lives in GEOMETRY, never in an angle sum. Each zone's outward
normal (`game_core::math::plate_normal`) folds together the facet slope, the
hull's live pitch/roll attitude, and the turret traverse; the impact angle is
the true 3D angle of incidence between the shell path and that normal. The
consequences are physical and intended: a flat shot meets a 60° glacis at 60°,
plunging fire meets the reclined plate far squarer (plunging DEFEATS sloped
armor), a nose-up hull-down pose steepens every frontal plate, and a shell
dropping on the deck resolves against the roof measured against UP.
`game_core::resolve_penetration_at_distance()` then applies shell
normalization, ricochet, AP/APCR overmatch (a caliber over three times the
plate neither ricochets nor enjoys unbounded line-of-sight gain), velocity-based
penetration falloff, and effective armor thickness.

Track and skirt zones are SPACED ARMOR, not a hull plate. Production resolves
them through `game_core::resolve_penetration_through_screens`
(`crates/runtime/sim/src/combat.rs:578`): the spaced stack is assembled
OUTERMOST FIRST — a skirt hit still has the belt behind it — and each screen
strips its own line-of-sight steel off the shell (double for HEAT — the screen
kills the jet's standoff); only the remainder challenges the hull side plate
behind it at that plate's own true angle. The honest half of the rule: a BROKEN
belt is not there any more (the thrown track lies on the ground beside the
hull), so it drops out of the screen stack — `combat.rs:574` includes the belt
only while its track pool has hit points. Hull hit points fall only if every
layer falls; the running gear takes its module damage either way, and HE bursts
on the outermost surface without ever reaching the plate. (The older
single-screen helper `resolve_penetration_through_track` survives with
test-only callers; it is not the production path.)

A front's weakness is GEOMETRY, not a multiplier. `ArmorZone::Cupola`
(`crates/foundation/game_core/src/armor/zone.rs:43`, appended wire-safe, #426)
models the commander's drum as its own convex volume standing proud of the
roof: a shell that used to graze the roof plane and auto-bounce now catches the
drum wall near-flat, and the drum resolves as side-grade casting standing
unsloped (`zone.rs:116`). `ArmorZone::GlacisPort` (`zone.rs:44-48`, #427) does
the same for the bow — the driver's visor and hull MG ball ride the glacis as
aimable patches presenting the bow plate's own steel FLAT (`zone.rs:119`),
authored only where the feature is visible. With those patches real, #428
pinned every `hull_front`/`turret_front` `weakspot_multiplier` at exactly 1.0
and the guard test holds it there against the smear creeping back
(`crates/foundation/game_core/tests/tiger_i.rs:4-26` asserts an authored,
sloped facet whose multiplier IS 1.0 — and the duel it buys:
`tiger_kwk36_beats_the_t54_port_but_not_its_honest_glacis`). The multiplier
stays in the schema for the zones that legitimately derive with it; a front's
weakness is now a patch you aim at, not a facet-wide discount.

A ricochet is a deflection, not a despawn: the shell mirrors about the struck
plate's normal, keeps flying slower and blunted
(`RICOCHET_SPEED_RETENTION`/`RICOCHET_PENETRATION_RETENTION` in
`sim::shell_continuation`, `crates/runtime/sim/src/shell_continuation.rs:8-9`),
and the next surface it finds resolves it for good — the
classic turret-roof skip into an engine deck is a real outcome. One skip per
shell, ever. The reticle preview intentionally stops at the first impact; the
skip is a server-side continuation.

Penetrating hits subtract hull hit points and then resolve internal damage. Blueprint vehicles
with an authored `DamageLayout` intersect the complete through-flight with physical module volumes;
legacy vehicles retain the deterministic zone/local-hit fallback until separately migrated.
Exposed running gear and non-penetrating HE remain direct suspension damage.

EVERY penetrating hit that enters through a plate — AP, APCR, HEAT, and HE alike, on every
blueprint vehicle — also creates a permanent `ArmorBreach` in the struck `Hull`, `Turret`, or
gun-pitched `Mantlet` frame: `crates/runtime/sim/src/combat.rs:192-217` cuts the breach for
every penetrating `ArmorEntry::Plate`, and the fleet scope is locked by the test named for it
(`reusable_aperture_physics_is_not_gated_to_the_t54_asset`, `combat.rs:905`). Later
projectiles pass through that opening only when their complete swept-body radius clears its
rim. The internal shell segment intersects the authored module volumes in distance order and
can damage more than one module before its residual penetration is exhausted; no
frontal-zone shortcut substitutes for that geometry.
Non-penetrating AP/APCR or HEAT hits emit a bounce `DamageEvent` with zero damage
and no module. HE surface hits emit non-penetrating damage and can throw tracks.

A kinetic AP/APCR perforation no longer destroys the projectile automatically. The struck LOS
steel is removed from its penetration budget, velocity falls with the square root of the remaining
energy fraction, and the shell exits forward while the just-perforated hull is ignored for the exit
step. It can then hit another aligned vehicle with the residual penetration. HE detonates on its
first surface and HEAT resolves its shaped-charge jet there; neither continues as a second flying
projectile. Blueprint vehicles additionally resolve their authored internal modules and permanent
plate channels; crew rays, fluids and free-flying spall fragments remain future layers.

## Ramming

Tank-to-tank collision is not only positional. The simulation captures tank
velocities at the start of a fixed tick, resolves movement and hull collision,
then applies ramming damage if the final hulls are in contact and the closing
speed is above the ramming threshold. Ramming damages both vehicles, always uses
`DamageCause::Ram`, and applies suspension module damage so a high-speed ram can
immobilize the target or the attacker.

## Networking And Regression

Authoritative snapshots carry tanks (with team identity), projectiles, current
aim dispersion, live module hit points, destroyed module masks, and best-effort
third-party combat events. Every emitted damage/impact event has one monotonic
`BattleEventId`, its authoritative tick, and the responsible `ShellId` where one
exists. `DamageEvent::target_destroyed` is stamped at resolution time for shell,
splash, ramming, landing, and drowning damage, so kill confirmation never guesses
from adjacent snapshots or credits a prior wound. Damage events also carry cause,
armor zone, hit position, and module data so the client can distinguish shell
penetration, HE track crits, bounces, ramming, and exact impact feedback. Protocol snapshot tests cover
command wire format; protocol v24 gives each projectile a stable `ShellId` and
replicates its type, caliber, drag, and age beside position and velocity. The
client extrapolates tracers with the shared ballistic integrator and gives AP,
APCR, HEAT, and HE distinct caliber-scaled silhouettes.

Protocol v38 gives each remote player a reliable personal combat-event tail.
Damage for which that player is source or target, plus the terminal impact of
that player's absorbed shell, repeats in sequence until acknowledged and is
deduplicated before any presentation side effect. Those personal events are
removed from that player's snapshots to prevent double playback; visible
third-party events remain best-effort snapshot feedback. Queue exhaustion or a
sequence gap is terminal, never an invisible loss. Combat tests and
replay regression fixtures cover fire, projectile travel, penetration, ricochet,
overmatch, HE surface damage, module damage, ramming, and damage as the system
grows. A parity test locks that the shared shell trace resolves the same tank
impact as the authoritative step, so the client preview and the server cannot
drift apart.

The replay regression rule is mandatory for later combat expansion.
