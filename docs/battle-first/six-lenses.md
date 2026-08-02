# Six lenses on the same project

The audit findings, read through six distinct professional perspectives. Each was asked the same
question — *what is actually wrong here?* — and each answers differently. Where they converge is
signal; where they conflict is a decision that has to be made by a human.

## 🎬 Game Director

One-person production, ~150k LOC, no CI, a €20–25 price. The question is not what to add — it is
**what slice can be shown in sixty seconds**.

The pitch is "honest tank, no RNG, real armour", and in a measured battle **53 hits produced 53
penetrations**. A promise that does not work cannot be demonstrated; a viewer sees shooting at boxes.

Take one map, one era, four vehicles, and drive the seven-minute battle to the point where it
**resolves before the clock** and where angling matters. Everything else waits. And there is no
telemetry and no crash reporting — shipping like that is shipping blind.

## 🗺️ World architect

**The map has no places.** No "that ridge", no "the church", nothing a player names in their head
and returns to. A 5 m cell cannot give birth to a place, and nobody authored any by hand.

This is not a technical problem. **Constraint creates meaning** — a corridor is more interesting than
a field because it forces a decision. 7v7 across a kilometre of open grass is not a battlefield, it
is a shooting gallery: not for want of triangles, but for want of a reason to turn.

Ostrogorsk is the right instinct. The question to ask of every map: **what does the player learn by
crossing it?** Today: nothing.

## ⚙️ CTO / engine

Measure before speaking. `perf_capture` **measures bakes, not frames** — there is not one frame-time
figure anywhere in the project, against a policy that says MX330 at 60 FPS and frame drops are bugs.
That is a wish, not a policy.

The simulation costs **35 µs per tick — 0.21 % of budget**. The architecture is fine; the
optimisation went to the wrong place.

Cut today: the dead swappable-backend layer (~400 lines of speculative abstraction, one
implementation, zero callers) and `rapier3d` + `parry3d`, compiled daily for nothing.

Use immediately: **a deterministic simulation with replay fixtures, currently used only by tests.**
That is the largest unexploited asset in the repository.

## ⚡ First principles

Question the premises. **Why is the terrain a heightmap at all?** If the gameplay lives in
micro-relief, a heightmap at any resolution is the wrong primitive — you pay across the whole map to
get a handful of places that matter.

**Why does armour carry a zone table when real convex volumes are already baked?** `ArmorZone` is a
legacy layer between two representations of the same tank.

And the strongest one: **there is 400× headroom in the simulation.** That is not "we are fast", it is
"we are not using the budget". See [`first-principles.md`](first-principles.md) for the full pass.

## 🔧 Optimisation engineer

Scene bake 233 ms. Ground maps 108 ms. Statics rebuild 22–28 ms — **longer than a frame**. Grass
costs 352 µs per frame on the CPU just to populate.

362–438k static vertices. On an MX330 (~1.4 TFLOPS) with shadow cascades, SSAO, HDR and bloom at
1080p, that is tight.

**This is a bake problem, not a simulation problem** — and the frame has never been profiled.
Everything said about performance is opinion until a frame flamegraph exists.

Determinism on one thread at 35 µs is a good decision — **do not touch it**. Threads belong in the
bakes and the renderer, never in the tick, or the replays and the client/server parity go with them.

## 🎨 Art director

**The barrel is too thin.** A proportion error — the most basic failure in vehicle art — visible in
every frame of every vehicle. A thin barrel ruins an otherwise good turret; fix proportions before
adding any detail.

**The T-54's turret front reads as a mushroom.** The side is good, but the money angle is the front,
and it does not say "casting".

And the systemic point behind every one of those: **this is not a style, it is the absence of one.**
Nothing has a material — tank, ground, trees and buildings all read as the same matte plastic, while
a full PBR pipeline sits built and unused (twelve material roles, three of which never reach the
shader). The lighting is flat: cascades, SSAO, HDR and bloom all present, and the sun still does not
model form. The palette takes no position. And **nothing is dirty** — a tank that crossed five hundred
metres of field should be carrying that field.

The UI is not an exception to this. Its **information architecture is genuinely good** — the layouts,
the reticle's content, the module row are all right. Its **production value is a mockup**: flat grey
rectangles, one type weight, line-glyph icons, a flat-lit tank on a flat disc, nothing with depth or
response. The UI and the world are at the same level of finish; the UI simply has better-considered
content.

---

## Where they converge — this is signal

All six stop in the same place: **measure the frame and fix the battle before adding anything.** The
director because there is nothing to show, the world architect because there are no places, the CTO
because there is no number, the engineer because there is no profile, the artist because the
artifacts are visible, first principles because nobody knows how much headroom is left.

That is not a compromise. It is unanimity.

## Where they conflict — this is the decision

**The director says CUT**: one map, one era, four vehicles, a battle that works.
**First principles says ADD**: there is 400× of headroom, build a simulation deep enough that
honesty becomes provable.

Both are internally consistent and they cannot both be followed.

**Between them sits the world architect with the sentence that probably settles it:** the project
needs neither cutting nor adding, it needs **one place a player will remember**. One map with a real
ridge, a real ravine and a real town would do more than a fourth era and more than a hundredfold
deeper simulation.

Because today the tank is better than the ground it stands on.
