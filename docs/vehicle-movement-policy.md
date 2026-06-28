# Vehicle Movement Policy

Vehicle movement is a server authoritative, fixed tick system. The renderer and
client camera may interpolate snapshots, but they do not decide position, speed,
turn rate, traction, or collision outcomes.

## Model

The hull is a planar (2.5D) rigid body: it carries a world-frame velocity *vector* and a yaw rate
with rotational inertia, not a single scalar forward speed. Because the velocity is a vector that
only rotates with the hull through lateral friction, the hull keeps its momentum through a turn and
can break grip and slide (drift) when a turn at speed — or low-traction ground — exceeds the
lateral grip cap. The height axis stays kinematic (the hull follows the terrain; there is no
airtime).

The custom tank controller derives its settings from `TankSpec`. The power-to-weight ratio sets the
acceleration, the spec speed caps set forward and reverse targets, and the spec turn rate is the
*steady-state* yaw rate the angular ramp converges to (heavier hulls spool up slower). Steering is
two-track style and decoupled from the throttle, so a hull can pivot in place under neutral steer.
Braking is explicit through the input command `brake`; it is not hidden inside render delta time or
camera input.

This is intentionally a controlled tank-battle movement model, not a full track simulation
(no per-track terramechanics, suspension, or hull roll). The goal is weighty, repeatable handling
that stays deterministic so it can survive networking, replays, and regression tests.

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

Terrain contact comes from heightmap sampling. The controller samples ahead, behind, and to the
sides of the hull to estimate slope, side slope, roughness, height, and traction. Slope behaviour
is one source of truth: gravity is projected onto the terrain plane, so the same term resists
uphill motion, accelerates downhill, and pulls the hull sideways on a side slope. Roughness reduces
traction and turn grip. The tank is grounded to the sampled terrain height after each fixed tick.

Track grip is finite. Longitudinally the tracks can deliver at most `mu * g * traction * cos(theta)`
of thrust, so a face steeper than the hull's gradeability (its longitudinal grip coefficient, ~60%
by default) cannot be out-pulled and the climb stalls on its own. Such an unclimbable face is also
treated as a hard barrier — momentum cannot carry a fast hull over it — so steep terrain like the
railway embankment must be crossed at prepared gaps, not driven over anywhere; gentle slopes only
slow the tank. Laterally, friction saturates at `mu * g * traction`: below it the hull tracks its
nose, above it (a hard turn at speed, or a steep low-traction face) it slides. The lateral friction
impulse only ever cancels sideways velocity, never reverses it, so the step stays stable at 60 Hz.

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
