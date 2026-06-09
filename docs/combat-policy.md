# Combat Policy

Combat is server authoritative. `TankCommand.fire` is input intent only; the
server simulation decides reload, projectile spawning, hit detection,
penetration, hull/module damage, and the resulting `Snapshot` data.

## Fire And Reload

Each tank owns a reload timer derived from its gun spec. A fire command creates a
projectile only when reload is ready, the tank is alive, the gun works, and the
ammo rack is not destroyed. Firing starts reload immediately. Client frame time,
renderer state, and camera mode do not affect reload.

## Shell Types

`ShellSpec` carries a deterministic `ShellType`:

- AP is the baseline kinetic round with normalization, overmatch, and moderate
  distance falloff.
- APCR has higher velocity/close-range penetration, weaker normalization, lower
  module damage, and harsher distance falloff.
- HEAT keeps penetration over distance and uses a high ricochet threshold, but
  does not overmatch.
- HE never ricochets. If it fails to penetrate, it still applies small external
  hull damage and can critically damage exposed running gear.

There is no random damage roll in the first playable slice. Damage, module
damage, ricochet, overmatch, and range falloff are replay-stable functions of the
shell, armor facet, impact angle, and travelled distance.

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
fast shells do not skip targets or pass through ridges and cover. The nearest of
the terrain and cover hits absorbs the shell; a nearer tank hit resolves as
damage. Friendly and dead targets are ignored.

## Shared Shell Trace

Shell flight has exactly one implementation, `sim::shell_trace`, used by every path
that needs to know where a shell goes: the authoritative server step
(`step_shells`), the client's reticle impact and penetration preview, and the
client's straight aim-ray sweep. The per-segment collision (`segment_impact`:
analytic ray versus hull-local AABB for tanks, a one metre stepped sweep for
terrain, and slab tests for cover) and the ballistic integration (`trace_shell`:
semi-implicit gravity-then-move at the simulation tick `dt`) are the same code for
all of them.

This is a hard rule, not a convenience. If the preview used a different timestep,
integration order, or intersection test, the reticle would predict an impact the
server resolves elsewhere, and under input latency the player would see hits the
server never confirms. The reticle matches the authority; it is not allowed to be
independently "more precise". Both sides build neutral `TraceTank` targets (the
server from `TankState`, the client from `TankSnapshot`) and pre-filter valid
targets before tracing. The server additionally drops teammates, which the client
cannot — a snapshot carries no team — so the preview may still mark a friendly. That
is a data-availability limit, not a trajectory mismatch.

## Armor And Damage

Combat uses per-vehicle oriented hitboxes with armor facing resolution for hull
front/side/rear and turret or casemate front/side/rear. `ArmorProfile` now keeps
the old six readable thickness values plus `ArmorFacet` data: nominal thickness,
visual/mechanical slope, and a weakspot multiplier. The impact normal produces
an impact angle, then `game_core::resolve_penetration_at_distance()` applies
shell normalization, ricochet, AP/APCR overmatch, shell-specific distance
falloff, and effective armor thickness.

Penetrating hits subtract hull hit points, then resolve module damage from the
armor zone and the hull-local hit point. The module map is deterministic and
volume-biased: exposed track zones damage suspension, mantlet hits damage the
gun, rear hull and rear-side engine-bay hits damage the engine, turret side/rear
hits damage the ammo rack, and frontal armor hits damage the gun or turret
assembly depending on the plate. Enclosed module volumes use replay-stable hit
chances derived from the local hit point, so a penetrating shell may pass through
without damaging a module; exposed running gear and mantlet hits remain direct.
Non-penetrating AP/APCR or HEAT hits emit a bounce `DamageEvent` with zero damage
and no module. HE surface hits emit non-penetrating damage and can throw tracks.

## Ramming

Tank-to-tank collision is not only positional. The simulation captures tank
velocities at the start of a fixed tick, resolves movement and hull collision,
then applies ramming damage if the final hulls are in contact and the closing
speed is above the ramming threshold. Ramming damages both vehicles, always uses
`DamageCause::Ram`, and applies suspension module damage so a high-speed ram can
immobilize the target or the attacker.

## Networking And Regression

Authoritative snapshots carry tanks, projectiles, current aim dispersion, live
module hit points, destroyed module masks, and damage events. Damage events carry
cause, armor zone, hit position, and module data so the client can distinguish
shell penetration, HE track crits, bounces, ramming, and exact impact feedback.
Protocol snapshot tests cover command wire format; protocol v11 includes module
HP and turret yaw velocity payloads. The server buffers damage events until the
next emitted snapshot so cadence does not drop hit feedback. Combat tests and
replay regression fixtures cover fire, projectile travel, penetration, ricochet,
overmatch, HE surface damage, module damage, ramming, and damage as the system
grows. A parity test locks that the shared shell trace resolves the same tank
impact as the authoritative step, so the client preview and the server cannot
drift apart.

The replay regression rule is mandatory for later combat expansion.
