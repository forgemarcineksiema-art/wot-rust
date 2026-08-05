# Aiming Model Policy

The aiming model follows a WoT-style core with War Thunder-style player
responsibility. The client gives the player a responsive desired sight ray, but
combat truth remains server authoritative.

## Reticle Layers

The persistent HUD reticle has three layers in every mode:

- central reticle: the desired sight and camera target under the screen center,
- gun marker: where the current barrel direction sits at the desired target
  range, so the player can see turret and elevation catch-up; it fades out as
  the barrel converges on the sight instead of stacking a second glyph there.
  It is a diamond, because every circle at this sight means the dispersion of
  this gun. Its fade band is ANGULAR — 0.75..1.6 of the live aiming circle, not
  a fixed screen distance — so the same barrel error reads the same way at any
  zoom, and a barrel already inside its own dispersion cone says nothing,
- aiming circle: the server-replicated current dispersion radius after aim time,
  movement bloom, shot bloom, and gun damage.

## The Hybrid Honesty Matrix

How much the reticle says about armor depends on the camera mode. Third person
is situational awareness; sniper mode is deliberate aimed fire where the
millimeter duel is the skill loop.

| Element | Third person | Sniper |
| --- | --- | --- |
| Central marker | neutral, always | pen verdict by color (green/red) |
| Gun marker | yes (fades when merged) | yes (usually merged) |
| Aiming circle | yes | yes (FOV-projected, magnifies) |
| Penetration color | never | yes |
| Pen/armor mm readout | never | yes |
| Real-impact marker (amber X) | never | yes, fades in as it separates |
| BLOCKED broken form | yes | yes |
| Target distance | yes | yes |
| Reload arc / hit confirm | yes | yes |

Third person must not show penetration colors, weak-spot hints, guaranteed hit
colors, or an impact marker: at that altitude the player reads the battlefield,
not an armor oracle. Sniper mode may speak: the player has committed to a
deliberate shot, and the verdict plus the pen-vs-armor millimeters teach the
armor model instead of replacing it. The BLOCKED form and the gun marker report
the player's own gun and draw in both modes — they leak nothing about the
target. The penetration hint keeps being computed in third person so a mode
switch answers instantly; only its rendering is gated.

## Gun Aim

Mouse input updates `DesiredAim` yaw and pitch as sight intent. The gun does not
blindly copy desired pitch. The client resolves the desired sight point, then
commands turret yaw and gun pitch toward the ballistic solution for the current
vehicle gun and shell velocity.

Turret traverse speed is a vehicle property — it comes from the installed
turret module (`crates/foundation/game_core/src/modules/loadout.rs:87`,
`turret.traverse.rate_rad_s()` assembled into `TankSpec::turret_rotation_rad_s`).
Gun elevation speed is NOT: the whole fleet shares
`GUN_ELEVATION_RATE_RAD_S = 0.5` (`crates/runtime/sim/src/aiming.rs:6,43-45`).
Crew and module modifiers may layer on top later, but they must feed the same
command path rather than bypassing it.

## Aim Time And Dispersion

`GunSpec::dispersion_mrad` is the settled minimum dispersion. `aim_time_seconds`
controls deterministic recovery toward that minimum. Hull movement, hull
steering, turret traverse, gun elevation, and firing add bloom up to the gun's
maximum dispersion. Partial gun damage raises the minimum dispersion; a
destroyed gun still cannot fire.

The server simulation owns the live dispersion value and applies a deterministic
center-biased shot offset when spawning a shell. Snapshots replicate the current
dispersion so the client can draw the aiming circle without inventing combat
truth locally.

The offset has a floor: the radial draw is clamped to at least 0.15 before it
is squared (`crates/runtime/sim/src/aim_dispersion.rs:92`, applied at `:65`),
so no shot ever lands dead centre — the minimum offset is 2.25% of the current
dispersion radius. The same squaring is what produces the center bias.

## Sweep And Authority

Client reticle prediction uses the same muzzle origin and obstacle segment
sweep shape as the local shell preview path: terrain, static cover, and vehicle
hitboxes are all considered. The result drives the reticle status in both modes
and the sniper-mode impact marker; third person does not render the actual
impact point as a player aid.

The server remains authoritative for firing, projectile travel, penetration,
damage, module state, and snapshots. The client reticle is predictive only; if a
debug server-reticle overlay is added later, it must be clearly optional.

## Sniper Camera

Sniper camera eye placement is anchored to the current hull/turret sight mount
so it does not slide sideways before the turret catches up. The view direction
still tracks `DesiredAim` immediately for responsive aiming. Mouse wheel in
sniper mode steps a discrete magnification ladder, not third-person boom
distance; mouse sensitivity scales with the FOV ratio so maximum zoom stays
controllable. The aiming circle is the angular dispersion projected through the
actual view FOV, so it magnifies together with the world under sniper zoom.
