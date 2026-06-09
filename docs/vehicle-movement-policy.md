# Vehicle Movement Policy

Vehicle movement is a server authoritative, fixed tick system. The renderer and
client camera may interpolate snapshots, but they do not decide position, speed,
turn rate, traction, or collision outcomes.

## Model

The custom tank controller derives movement settings from `TankSpec`.
The power-to-weight ratio controls acceleration, the spec speed caps control forward and
reverse targets, and the spec turn rate remains the baseline yaw rate. Braking is
explicit through the input command `brake`; it is not hidden inside render delta
time or camera input.

This is intentionally a controlled tank-battle movement model, not a realistic
track simulation. The goal is repeatable handling that can survive networking,
replays, and regression tests.

## Shared Drive Step

One fixed-tick function, `sim::step_tank_drive`, advances a hull: movement (terrain, cover, and
tank collision), turret and gun aiming, and aim-dispersion bloom — each gated by module health.
The authoritative server and the client predictor call it with the same command and `dt`, so the
local tank is simulated by exactly the same code as the authority, not a parallel reimplementation
that drifts. The server projects its `TankState` into the step's neutral state and writes the
result back; the predictor stores that neutral state directly.

Aim dispersion is part of this step, not a separate concern. It recovers toward the gun's settled
minimum over aim time, while hull motion and turret traverse bloom it outward; the predictor
evolves it at 60 Hz seeded from the last snapshot, so the reticle's aim circle tracks the server
between 20 Hz snapshots instead of holding a stale value. Firing stays server-only — shells are
authoritative.

Snapshots carry live module hit points in stable slot order, so the client predictor can apply
partial gun damage to aim dispersion between snapshots instead of approximating damaged-but-alive
modules from a destroyed bit. A parity test locks that the predictor matches the server's pose and
dispersion tick-for-tick.

## Terrain Contact

Terrain contact comes from heightmap sampling. The controller samples ahead,
behind, and to the sides of the hull to estimate slope, side slope, roughness,
height, and traction. Uphill slope reduces acceleration and target speed; roughness
reduces traction and turn grip. The tank is grounded to the sampled terrain height
after each fixed tick.

Each tank also has a maximum climbable grade (its gradeability, ~60% by default). A slope
steeper than that limit collapses climb speed to zero, so steep terrain such as the railway
embankment acts as a real barrier that must be crossed at prepared gaps, not driven over
anywhere. Gentle slopes still only slow the tank.

Static cover (buildings, treelines, wrecks) is a hard obstacle as well: the shared drive step
keeps the hull out of cover footprints, sliding along a face rather than sticking, so the
predicted hull stops exactly where the server stops it (see Shared Drive Step).

Rapier remains useful for broadphase, world collision, raycasts, and simple
bodies. Tank movement truth stays in the custom controller so replays and server
simulation remain stable.

## Boundaries

`physics` owns the custom movement math and terrain contact helpers. `sim` owns
when fixed ticks are applied and copies the resulting authoritative state into
tank snapshots. `game_core` owns vehicle parameters. `net` carries input commands
and snapshots. No movement rule depends on `wgpu`, renderer frame time, or window
events.
