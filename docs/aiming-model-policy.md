# Aiming Model Policy

The aiming model follows a WoT-style core with War Thunder-style player
responsibility. The client gives the player a responsive desired sight ray, but
combat truth remains server authoritative.

## Reticle Layers

The persistent HUD reticle has three neutral layers:

- central reticle: the desired sight and camera target under the screen center,
- gun marker: where the current barrel direction sits at the desired target
  range, so the player can see turret and elevation catch-up,
- aiming circle: the server-replicated current dispersion radius after aim time,
  movement bloom, shot bloom, and gun damage.

The central marker is intentionally smaller and less opaque than the gun marker.
In third-person it should communicate sight intent without covering enemy
silhouettes or competing with the actual barrel marker.

The HUD must not show penetration colors, weak-spot hints, guaranteed hit
colors, or a permanent magic impact marker. The player learns armor layouts,
shell ballistics, and lead by reading the vehicle, range, and shell behavior.

## Gun Aim

Mouse input updates `DesiredAim` yaw and pitch as sight intent. The gun does not
blindly copy desired pitch. The client resolves the desired sight point, then
commands turret yaw and gun pitch toward the ballistic solution for the current
vehicle gun and shell velocity.

Turret traverse and gun elevation speed are vehicle properties today. Crew and
module modifiers may layer on top later, but they must feed the same command
path rather than bypassing it.

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

## Sweep And Authority

Client reticle prediction uses the same muzzle origin and obstacle segment
sweep shape as the local shell preview path: terrain, static cover, and vehicle
hitboxes are all considered. The result may drive internal status, debugging,
and training tools, but the normal HUD does not render the actual impact point
as a player aid.

The server remains authoritative for firing, projectile travel, penetration,
damage, module state, and snapshots. The client reticle is predictive only; if a
debug server-reticle overlay is added later, it must be clearly optional.

## Sniper Camera

Sniper camera eye placement is anchored to the current hull/turret sight mount
so it does not slide sideways before the turret catches up. The view direction
still tracks `DesiredAim` immediately for responsive aiming. Mouse wheel in
sniper mode changes sniper FOV/zoom, not third-person boom distance.
