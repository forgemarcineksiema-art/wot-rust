# Battle Camera Policy

The game camera is a vehicle-combat system, not a generic scene camera. It reads
server snapshots and produces renderer cameras; it must not own authoritative
simulation.

## Modes

Third-person is the default driving and coarse-aiming camera. It follows the
hull, uses an orbit boom, keeps terrain clearance, and shortens the boom against
static cover collision. Its target point is ahead of the vehicle and its eye is
slightly over the shoulder, so large hulls and turrets do not sit directly over
the desired sight while the player is aiming without sniper view. It is tuned
for reading terrain, cover, vehicle silhouette, enemy silhouette, and shell
tracers.

Sniper is the precision aiming camera. Its eye is anchored near the current
hull/turret gun sight so it does not slide sideways before the turret catches
up. The view direction follows the mouse-driven desired aim immediately, with
the same vertical sense as third-person (mouse back looks down). Entering
sniper aligns the view to where the gun actually points, never to stale desired
pitch. The player's own vehicle is hidden in sniper view because the eye sits
inside the turret mesh.

Zoom is one continuous wheel axis: scrolling in shortens the third-person boom,
hands over to sniper at the shortest boom, then steps a discrete magnification
ladder (FOV steps, roughly geometric so each click is a similar relative
magnification change); scrolling back out steps the ladder and exits to the
shortest boom. Mouse-look sensitivity scales with the sniper FOV ratio so
on-screen cursor speed stays roughly constant across magnifications.

## Inputs And Truth

The client camera consumes server snapshots for tank position, hull yaw, and
turret yaw. Local camera input may orbit, zoom, or switch modes, but the camera
does not move the tank and does not own authoritative simulation. Gameplay truth
continues to live in `server` and `sim`.

The desktop camera bindings change only the local camera mode. `V` toggles
Third-person and Sniper; the mouse wheel is the primary path, zooming the
third-person boom in, handing over to Sniper at the shortest boom, then stepping
the magnification ladder. `Shift` is a hold: pressing it opens the scope on the
current crosshair point, releasing it returns to whatever mode was active before
the hold (third person, or sniper if it was already open via `V`). Driving brake
lives on `Ctrl`.

Key entry (`V`, `Shift`) always opens at the default sniper magnification, never the
last wheel step: a reflex peek must never snap open at maximum zoom. The wheel dials
deeper from there. Only the wheel's own third-person-to-sniper handover keeps its
widest-step entry, since it is one continuous sweep in from the boom.

Free look (`Alt`) orbits only the camera and never moves the aim: on release
the camera returns to the sight lane (yaw and pitch), instead of the turret
swinging to wherever the player glanced.

## Terrain And Collision

The camera samples terrain before rendering so the eye does not clip below the
heightmap. Third-person boom collision checks static cover collision through
camera obstacles. Runtime battlefield maps feed their `static_cover` bounds into
the same obstacle list used by tests.

Continuity discipline (the anti-jump rules, locked in `camera_feel.rs`):

- **The boom's terrain cut is continuous.** The 32-step march refines its
  clear→blocked crossing by bisection, so the cut slides instead of stepping in
  boom/32 = 0.375 m quanta (`the_boom_cut_slides_continuously_past_a_ridge`).
  Engage/release remain honest discontinuous events; only the slide is smooth.
- **The clearance lift releases at a bounded rate.** The LOGICAL camera keeps
  the hard eye-over-terrain clamp. The PRESENTED camera owns the lift as state:
  it rises instantly (the eye never renders from under the ground) and lets go
  at 3 m/s once the ground falls away — an instant release moved only the eye,
  which ROTATES the whole view in one frame
  (`a_passed_ridge_releases_the_clearance_lift_smoothly`). It is applied LAST,
  after the boom smoothing and the feel shifts, so no later stage can push the
  eye back under the ground it just cleared.

Gun pitch is authoritative simulation/server state and is carried in tank
snapshots. The client keeps a separate desired aim yaw/pitch for local camera
feel and reticle feedback, then commands turret and gun toward the ballistic
solution for the desired sight point. The actual shell path remains driven by
the predicted/authoritative turret yaw and gun pitch.

## The Feel Layer (Immersja B1-B3)

The camera is a body, not a tripod — but only the PRESENTED camera. Two cameras
exist by law (`quality/tests/camera_rules.rs`): the LOGICAL camera
(`render_camera`) that aiming and sight-solving read, and the PRESENTED camera
(`present()`) that reaches the renderer. Every feel channel lives strictly in
the presented layer; the logical camera's output is bit-identical with every
feel input loaded, and the sniper never moves for any of them — aiming
tolerates no theatrics.

The channel inventory, with its caps:

- **Follow spring** (`smoothing.rs`): critically damped anchor, per axis group —
  omega 16 horizontally (~0.13 s lag; losing the hull sideways is losing the
  game) and omega 9 vertically with a SHORT 0.05 m leash. The vertical channel
  obeys the hill law (player verdict 2026-08-22): **a hill is signal the player
  steers by, not noise for the suspension to eat.** The spring filters only the
  cm-scale velocity kinks of the 5 m heightfield snap (~34% of a 2 Hz small
  train passes, vs 62% at the old isotropic omega 16); anything larger than the
  leash is terrain and is tracked — the eye may never separate from the hull
  vertically by more than 0.05 m (~0.5% of the frame height at the default
  boom), and half a second after a slope flattens the ride is over. The first
  tune (omega 7, 0.35 m leash) floated the frame through 0.7 m at every crest
  and was retired as a spring ride; the player then cut the leash to 0.10 and
  again to 0.05 the same day. Locked by
  `a_hill_ride_never_floats_the_frame_beyond_the_short_leash` and
  `a_bump_train_reaches_the_presented_eye_attenuated`; horizontal leash stays
  0.6 m; a pinned leash self-limits the spring's velocity through its damping
  (omega * leash / 2), so releases settle instead of kicking
  (`a_leash_ride_settles_without_a_windup_kick`).
- **Speed FOV**: +2.5 deg at 14 m/s, driven by tick-domain rigid-body speed,
  never by presented-position differences.
- **Own-shot kick**: anchor velocity impulse, 0.9 m/s back + 0.5 m/s down.
- **Damage shudder / landing slam**: event-driven anchor impulses; a landing
  may inject at most 1.5 m/s (3.0 stacked a second dip onto every crest).
- **Sprung-hull ride** (B1, renegotiated twice): the presented TPP takes 35 %
  of the hull spring's dynamic dive residual (spring minus authoritative
  attitude — a steady slope contributes nothing); hard cap 0.008 rad. The cap
  is a gameplay bound, not just a runaway guard: the residual spikes exactly on
  hill entry/exit, and the old 0.02 rad cap tilted the screen-centre crosshair
  1.15 deg off the logical aim on every slope transition. The shot's rock arrives
  through this same chain: the hull's fire impulse rocks the spring, the
  residual rides into the rig. The HEAVE half is retired — the anchor's soft
  vertical spring carries the ride, and stacking a second, differently phased
  vertical filter on top re-added the bounce it removes. The residual is read
  the same frame the presentation spring steps (the presentation world syncs
  BEFORE the camera presents): a one-frame-stale residual is a derivative of
  hull pitch, spiking exactly on the bump it was meant to soften.
- **Ride tremor** (B2): terrain roughness with the slope component removed,
  times speed, as two inharmonic vertical beats; hard cap 0.05 m. A standing
  tank never trembles. Both beats sit well under the 60 FPS display's 30 Hz
  Nyquist rate (11 + 8.3 Hz) — the second beat shipped at 29.7 Hz and rendered
  as a per-frame up/down strobe rather than a shiver, locked out by
  `the_ride_tremor_shivers_instead_of_strobing_frame_to_frame`.

Four laws bind every present and future channel:

0. **Immersion is never designed against gameplay.** Every feel channel carries
   a hard cap sized so the channel can corroborate the tank's motion but can
   never move the frame or the sight in a way the player must fight (player
   verdict 2026-08-22, the hill ride). A channel that needs a big cap to be
   felt does not ship.
1. **The sniper is bit-rigid.** Any channel that moves the sniper eye is priced
   by the reticle-seam ratchet (30k placements per map) before it ships.
2. **No RNG in presentation.** Every channel is a deterministic function of its
   input sequence — phase accumulators and springs, never dice.
3. **Feel dies with the freeze.** Every channel's inputs must retire through
   `predict.rs::freeze_motion()` (zeroed speed, accel, impulses) so a terminal
   world never carries a living camera. The lock
   `every_feel_channel_retires_when_motion_freezes` excites every channel at
   once and asserts the presented camera settles to bit-stability — a future
   channel with unretired state fails it by construction.
