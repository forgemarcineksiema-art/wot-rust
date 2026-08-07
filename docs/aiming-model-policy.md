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
  movement bloom, shot bloom, and gun damage. It is the FULL envelope, not a
  percentile: the shell is never outside it. The server squares its shot draw,
  so half the shots land inside the inner quarter of the drawn radius — the gun
  feels more accurate than the circle promises, and that is the deliberate
  trade for a promise that never breaks (register G11).

The gun's own state is a COLOUR, on one line just outside the aiming circle:
the reload arc drains RED, and closes into one full GREEN circle when the
breech is shut. Green holds a beat and dissolves; a loaded gun then draws
nothing. The circle brightens separately when the dispersion has settled onto
THIS gun's minimum — damage included, since a wounded gun recovers toward a
wider floor and reaching it is still "the aim has been taken".

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
| Block range (refusals only) | yes | yes |
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

The trace flies the FIRING SOLUTION, not the live barrel: the ballistic arc
toward the sight point, folded through the hull pose and clamped to the gun's
own limits (`crate::aim::firing_solution`, shared with the gun commands that
chase it). So the status, the impact marker and the penetration verdict all
follow the crosshair instead of flickering through whatever the barrel sweeps
across mid-traverse.

BLOCKED is ONE question — does that shot arrive where the player is pointing? —
answered by that one trace: the gun cannot reach the elevation (judged in the
HULL frame, so a nose-down hull gets the depression it really has), or the shell
dies on terrain, cover, an ally or a wreck along the way. An open-sky shot that
simply expires is not blocked; it has no target, which is different. The
arrival window follows the ARRIVAL ANGLE rather than the range: a plunging shot
is judged in centimetres, a grazing one touches shallow ground metres early
through the same few centimetres of vertical slack.

There is no second geometry — no straight-line chord — because a ballistic arc
rides above its own chord and a crest the chord grazes is not a blocked shot
(register G1–G3).

And a refusal must NAME ITS CAUSE. "Blocked" on its own leaves the only number
on screen — the range — answering a question the player cannot act on, because
while blocked that range belongs to something the gun cannot reach. So a shot
stopped SHORT of the crosshair prints the metres to whatever eats it, on the row
under the range, in the broken marker's grey, and a distant impact X is led back
to the crosshair by a hairline. A shot that sails PAST the sight point prints
nothing: it was obstructed by nothing, and its range is a fact about the shell's
lifetime, not about the battlefield.

The server remains authoritative for firing, projectile travel, penetration,
damage, module state, and snapshots. The client reticle is predictive only; if a
debug server-reticle overlay is added later, it must be clearly optional.

## The Sight's Promise

**If the sight shows you a whole tank, either you can hit it, or the sight tells
you in metres where the shot dies.**

The reticle can always be right and still be useless, and that is exactly what
happened (`docs/sight-honesty-program.md`): a refusal is correct, honest, and
unactionable when nothing in the picture or the readouts says what refused it.
The promise is therefore about the SEAM, not the verdict — between what the eye
reaches and what the gun can reach there must be no silent gap.

Two rules carry it:

- **The eye is the gun's telescope, not a vantage point.** The sniper eye stands
  in the band a real gunner's optic occupies over the bore (0.08–0.16 m; the
  T-54's TSh-2-22, the T-34-85's TSh-16 and the Panther's TZF-12a all live
  there). Height at the origin is not a free comfort: a 320 m shot leaves the
  muzzle about 2 mrad above the line to its target, so **every centimetre of eye
  above the bore buys about five metres of ground the eye clears and the shell
  does not**. The number is a reference measurement, guarded by
  `camera/sniper.rs::the_sniper_eye_sits_where_a_real_gunners_telescope_does`.
- **The seam is measured, not argued.** `hud/reticle/seam_tests.rs` sweeps 30 000
  hull placements per shipped map through the live path and ratchets the share of
  sight-reachable hulls the gun cannot reach. The ceilings there are measured
  numbers; raising one is a decision for the program doc, never a way to get a
  run green.

## Sniper Camera

Sniper camera eye placement is anchored to the current hull/turret sight mount
so it does not slide sideways before the turret catches up. The view direction
still tracks `DesiredAim` immediately for responsive aiming. Mouse wheel in
sniper mode steps a discrete magnification ladder, not third-person boom
distance; mouse sensitivity scales with the FOV ratio so maximum zoom stays
controllable. The aiming circle is the angular dispersion projected through the
actual view FOV, so it magnifies together with the world under sniper zoom.
