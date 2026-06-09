# Armored Battle Domain

This project is not a general-purpose engine. It is a domain engine for armored
vehicle battles on large terrain maps.

## Product Shape

The default game shape is narrow on purpose:

- camera modes are mainly third-person and sniper views,
- world content is terrain, static battlefield objects, vehicles, and effects,
- maps are outdoor battlefields, not dense indoor AAA spaces,
- destruction is selective gameplay state, not full-world destruction,
- skeletal animation is not a foundational renderer problem,
- terrain, LOD, shadows, spotting, shell physics, and networking are core.

## Consequences

Prefer direct systems for tank battles over generic engine abstractions. A
feature belongs in this foundation when it supports large-map armored combat,
authoritative multiplayer simulation, battlefield visibility, projectile
physics, terrain traversal, or vehicle presentation.

Defer or reject features whose primary purpose is a different genre:

- general indoor streaming,
- character-animation-first gameplay,
- fully destructible interior spaces,
- generic scene graph tools for every possible game type,
- renderer features that do not help terrain, vehicles, shadows, effects, or UI.

The `engine` crate stays a thin ECS/world utility layer. It must not become a
general-purpose product or the place where domain rules hide.
