# Vehicle Movement Policy

Vehicle movement is a server authoritative, fixed tick system. The renderer and
client camera may interpolate snapshots, but they do not decide position, speed,
turn rate, traction, or collision outcomes.

## Model

The hull is a planar (2.5D) rigid body: it carries a world-frame velocity *vector* and a yaw rate
with rotational inertia, not a single scalar forward speed. Because the velocity is a vector that
only rotates with the hull through lateral friction, the hull keeps its momentum through a turn and
can break grip and slide (drift) when a turn at speed — or low-traction ground — exceeds the
lateral grip cap. The height axis follows drivable terrain kinematically, but a drop steeper than
the tracks can follow (a crest, a cliff) goes genuinely ballistic: the hull flies, ignores drive
input while airborne, and absorbs a landing impact that can damage the suspension.

The custom tank controller derives its settings from `TankSpec`. The power-to-weight ratio sets the
acceleration, the spec speed caps set forward and reverse targets, and the spec turn rate is the
*steady-state* yaw rate the angular ramp converges to (heavier hulls spool up slower). Steering is
two-track style and decoupled from the throttle, so a hull can pivot in place under neutral steer.
Braking is explicit through the input command `brake`; it is not hidden inside render delta time or
camera input.

This is intentionally a controlled tank-battle movement model, not a full track simulation (no
per-track terramechanics and no sprung suspension in the authoritative state). The goal is weighty,
repeatable handling that stays deterministic so it can survive networking, replays, and regression
tests.

## Hull Attitude and the Support Envelope

The hull's ground contact is its running gear, not a point: terrain is sampled at the road-wheel
stations of the vehicle's `ContactFootprint` (from the blueprint `TrackShape` — the same stations
the rendered wheels are placed by). The hull rests as a rigid beam on the highest supported
stations, which is what makes tank-shaped behavior emerge: trenches narrower than the wheel pitch
are bridged instead of swallowed, and a nose pushed past a crest hangs level until the centre of
mass passes the last support, then rotates down onto the far slope.

Hull pitch and roll are **authoritative**: computed kinematically from the support plane,
rate-limited (no springs, no oscillation state in the sim) and frozen while airborne, so they stay
deterministic and replay-stable. They feed gameplay — gun elevation limits are hull-relative (hull
down over a crest genuinely adds depression), armor impact angles include the hull's tilt, and the
hitbox tilts with the hull. Weight-transfer theatrics (brake dive, acceleration squat, turn lean,
heave) remain a client-side presentation spring layered ON TOP of the authoritative attitude and
never feed back into it.

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

## Contact Carries Momentum

A tick runs in the rigid-body order, and the order is the point:

1. every living hull decides a velocity, and **nobody moves** — commanded or not, because a tank
   nobody gave an order to is a tank sitting still, not a tank exempt from physics;
2. hull-to-hull contacts are solved against how far apart the hulls actually are;
3. the surviving velocities are spent and resolved against the world.

`physics::advance_tank_on_world` and `settle_tank_on_world` are steps 1 and 3, and the client
predictor runs the same three in the same order — its neighbours enter the contact solve as
immovable bodies, because the client is not authoritative over them.

There is no fourth step. A positional pass used to end the tick by pushing anything still
overlapping apart, and it was the last place a hull could arrive somewhere it never travelled to:
measured, it cleared a three-metre spawn overlap in ONE TICK by moving a hull 1.49 m — eighty-nine
metres per second on a vehicle whose top speed is fourteen. Separation is a velocity now, all of
it, capped at a metre per second plus another for every metre a hull is buried.

Contact is **speculative**: the separating-axis test reports how far apart two hulls are, signed,
and the constraint allows exactly the closing that shuts that gap this tick and no more. Hulls
therefore come to rest TOUCHING rather than a detection margin short of each other — the margin
that used to decide where they parked held two T-54s 0.12 m apart at the box and 0.40 m apart at
the metal. The solver also remembers: each contact is filed under the pair's identity and the
touching feature, and starts the next tick holding the impulse it ended this one with, which is
what keeps a long queue from sinking into itself.

Hull-to-hull is NOT a veto anywhere any more. `resolve_tank_collision` — a hard "hold the previous
position" stop with an interpenetration escape hatch beside it — lost its last caller when the
predictor moved onto the solver, and left the workspace the way rapier did.

Contact is an **impulse**, not a veto. Touching hulls exchange normal momentum, an off-centre
contact exchanges angular momentum too (a t-bone slews its victim), and Coulomb friction bounded
by the normal impulse resists sliding across the contact. Restitution is zero: armour plate does
not bounce, it shoves. The solve is Jacobi — gathered against one shared state and applied
together — so the outcome cannot depend on where a tank sits in the roster.

Ram damage reads the **resolved impulse**, not a closing speed of its own. Two answers to "did
these tanks collide" is one answer too many.

**The invariant this bought, stated so it is not lost again: nothing may DELETE momentum; it may
only resist it.** Three places broke that rule, and all three were invisible for as long as a
hull's own drive was the only thing that could put velocity into a hull:

- the static track-lock zeroed any undriven hull under its grab threshold, whatever momentum was
  there. It now removes only what `mu_s * g * traction * cos(theta) * dt` could actually arrest;
- a thrown track forced the velocity to zero every tick. It now removes the **drive** — no thrust,
  no steering, the shed belts dragging through the brake channel — and leaves the momentum. (A ram
  throws the victim's track on the first hit, so the old rule made a shoved hull unpushable the
  instant it was hit, and froze a hull thrown in mid-air.)
- contact had no tangential friction at all, so hulls pressed together slid along each other for
  free and a queue squirted sideways.

## Terrain Contact

Terrain contact comes from heightmap sampling. The controller samples ahead, behind, and to the
sides of the hull to estimate slope, side slope, roughness, height, and traction. Slope behaviour
is one source of truth: gravity is projected onto the terrain plane, so the same term resists
uphill motion, accelerates downhill, and pulls the hull sideways on a side slope. Roughness reduces
traction and turn grip. The tank is grounded to the sampled terrain height after each fixed tick.

Track grip is finite. Longitudinally the tracks can deliver at most `mu * g * traction * cos(theta)`
of thrust, so steady climbing stalls at the hull's gradeability (its longitudinal grip coefficient,
~60% / ~31° by default). Steeper faces are handled in a **momentum-climb band** up to a ceiling
(~0.68 grade / ~34°): the grip *slips* (falls off as `(gradeability/grade)^2`) instead of vanishing,
so a committed run-up scrabbles the hull a bounded, energy-limited way up a hump before it bleeds off
and stalls — no free crest, but no invisible wall either. Above the ceiling a face is a hard barrier
(the tracks find no drive and the nose digs in), so a cliff or the railway embankment cannot be
driven straight over and must be crossed at prepared gaps. Because the relevant grade is the
component *along the heading* (`forward_slope`), hitting a steep face at an angle lowers it below the
ceiling — a diagonal run-up can scale terrain a head-on charge cannot. That angle-of-attack climb is
emergent, and is the intended "clever, fair" steep climb: skill and commitment, not a bump-over.

Static (parked) hold: a stopped, undriven hull **locks its tracks** and holds any slope up to
`static_grip_mu * traction` (~0.9 / ~42°) — the demand `g*grade*inv` is met by static friction
`mu_s*g*traction*inv`, the `cos(theta)` cancels, so the hold is simply `grade <= mu_s * traction`.
Within it the hull neither creeps downhill nor side-slides while you line up a shot; steeper (or too
slick) it never grabs and the kinetic model below lets it slide. Static grip is higher than the
kinetic grips (`mu_s > mu_k`), so the hull "sticks" then breaks loose. A neutral-steer pivot still
turns a parked hull in place — only the linear drift is locked.

Changing direction is not free. Track brakes hold a *starting* hull — creep against the commanded
direction, the gravity rollback the hold above is for — but established momentum is a different
thing: a hull already rolling bleeds its speed through the force model before it can reverse.
Tapping S at 8 m/s does not erase 8 m/s in one tick; it commits the crew to a deceleration. The
consequence reaches past feel: anything that plans around stopping must read a braking DISTANCE
rather than assume an instant reversal. The bots' deep-water escape is the worked example — see
`server/src/bot_routes.rs` and the `server/tests/bot_water.rs` soak, which is the test that
catches a drive-model change from the gameplay side.

Laterally (while moving), friction saturates at `mu * g * traction`: below it the hull tracks its
nose, above it (a hard turn at speed, or a steep low-traction face) it slides. The lateral friction
impulse only ever cancels sideways velocity, never reverses it, so the step stays stable at 60 Hz.

Static cover (buildings, treelines, wrecks) is a hard obstacle as well: the shared drive step
keeps the hull out of cover footprints, sliding along a face rather than sticking, so the
predicted hull stops exactly where the server stops it (see Shared Drive Step).

Collapsed buildings are part of that ground. A `CoverPhase::Rubble` object leaves the movement
collision entirely and enters the support envelope as `terrain::RubbleMound` — a truncated pyramid
with flanks at the angle of repose of broken masonry. Both the resting line and the drive's slope
probe read `max(terrain, debris)`, so the hull rides the pile AND pays it: the crossing tilts the
hull and bleeds speed through the same force model every slope uses. Intact cover is unchanged —
it blocks in plan at any height, so nothing ends up on a roof. See `docs/honest-steel-policy.md`,
"Rubble is terrain".

A hull that stops being a tank does not stop being an object. The drive step is skipped for dead
hulls — a wreck neither drives nor steers nor slides — but its VERTICAL is still resolved, every
tick, commanded or not (`sim::wreck::settle_wrecks`), against the same support envelope a live
hull reads. Without it a hull killed in mid-flight hung at the altitude it died at for the rest of
the battle, and a wreck standing where a later crater opened floated over the hole. A wreck already
resting on its support is a bit-identical no-op, so replays are unaffected.

No physics engine sits under any of this. rapier3d left the workspace 2026-08-02; `parry3d`
survives only as the narrow footprint-intersection query
(`physics::parry_query::tank_footprints_intersect_query`,
`crates/runtime/physics/src/parry_query.rs:7`), which today has no production caller — the
live tank-vs-tank test is the hand-rolled SAT in `crates/runtime/physics/src/collision.rs`.
`crates/tooling/quality/tests/parry_feature_rules.rs` pins parry's features
(`default-features = false`, `dim3`/`f32`, no SIMD, no parallel) and asserts the manifest
contains no "rapier": re-adding an engine is a design decision with its own tests, not a
dependency drive-by. Tank movement truth stays in the custom controller so replays and
server simulation remain stable.

## Boundaries

`physics` owns the custom movement math and terrain contact helpers. `sim` owns
when fixed ticks are applied and copies the resulting authoritative state into
tank snapshots. `game_core` owns vehicle parameters. `net` carries input commands
and snapshots. No movement rule depends on `wgpu`, renderer frame time, or window
events.
