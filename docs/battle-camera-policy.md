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

The current desktop binding uses `1` for Third-person and `2` for Sniper. These
keys change only the local camera mode.

Free look (`Alt`) orbits only the camera and never moves the aim: on release
the camera returns to the sight lane (yaw and pitch), instead of the turret
swinging to wherever the player glanced.

## Terrain And Collision

The camera samples terrain before rendering so the eye does not clip below the
heightmap. Third-person boom collision checks static cover collision through
camera obstacles. Runtime battlefield maps feed their `static_cover` bounds into
the same obstacle list used by tests.

Gun pitch is authoritative simulation/server state and is carried in tank
snapshots. The client keeps a separate desired aim yaw/pitch for local camera
feel and reticle feedback, then commands turret and gun toward the ballistic
solution for the desired sight point. The actual shell path remains driven by
the predicted/authoritative turret yaw and gun pitch.
