# Geometry pipeline audit — 2026-08-08

Everything here is measured, and every number states the command that produced it. Where a
conclusion is a judgement call it says so.

The audit started from a flat question — *"some modules look primitive, and it is the geometry's
fault"* — and ended somewhere else: **the geometry stack is in better shape than the picture
suggests, and the picture is held down by settings and by coverage, not by missing capability.**

---

## 1. Where geometry really comes from

`vehicle_build::PartShape` has three arms. Real construction sites across the whole repo:

| arm | what it means | uses |
|---|---|---:|
| `Plates(ConvexSolid)` | geometry **described**, meshed at bake | 5 |
| `Cast { sdf, min, max, budget }` | geometry **described**, meshed by SDF | **0** |
| `Mesh(GeometryMesh)` | already-built triangles; `mesh()` is a `clone()` | **38** |

So 38 of 43 parts arrive already built. The routing `part.rs` describes as *"the whole point of
the hybrid"* — plate to `solid`, casting to `sdf_mesh` — routes five parts and one dead arm.

`GeneratorKind` is a **separate author-typed field**, independent of `PartShape`:
Revolve 24, Sweep 13, Solid 7, CastLoft 3, Panel 1, **Sdf 0**. Its only consumers are
`vehicle_build/src/manifest.rs` and `vehicle_forge/src/part_manifest.rs` (`kernel_name()`).

**Consequence:** the Forge part report states which kernel built each part, and that statement is
unverifiable by construction. Once `PartShape::Mesh` holds the mesh, provenance is gone. The
report can be wrong and nothing can catch it.

**Cost of the dead arm:** `PartShape::Cast` keeps `sdf` + `sdf_mesh` (1,168 LOC) as dependencies of
`vehicle_build`, compiled and linted on every build. `docs/procedural-kernel-program.md` already
predicted this — *"the next cast part either proves this row or retires it"*. It has not.

## 2. Two parallel architectures, and the good one has one user

`VehiclePart` is constructed **only** in `vehicle_build/src/t54*.rs`. The other eight vehicles
never touch the part system; they go through `vehicle_recipes` + `assemble()` with direct
`MeshBuilder` calls.

`vehicle_forge/src/production_bake.rs` says so itself:

> T-54 is the Forge benchmark and uses its hybrid description. Other vehicles intentionally stay on
> the legacy recipe path **until their own Forge migration is complete**. […] Legacy vehicles still
> flatten then globally cluster **until their own Forge migration lands**.

It never landed. "Fleet forge migration" (#119–#125) made the fleet **blueprint-born**, not
**part-born**.

What the other eight lose: per-part LOD policy (`Silhouette` / `MountCritical` / `Detail`), stable
part identity across LOD, the part manifest, the part graph, attachment anchors. Their LOD is
flatten-then-cluster.

Measured shipped static bakes (`cargo test -p vehicle_forge --test fleet_draw_cost -- --nocapture`):

| vehicle | static tris | gear near | draws |
|---|---:|---:|---:|
| **T-54** | **21 508** | 39 632 | 204 |
| Tiger I | 3 296 | 38 736 | 228 |
| Panther II | 2 616 | 24 120 | 112 |
| Tiger II | 2 532 | 28 712 | 132 |
| Jagdtiger | 2 460 | 28 936 | 134 |
| IS-3 | 1 950 | 39 632 | 206 |
| T-34-85 | 1 722 | 23 088 | 110 |
| Centurion | 1 708 | 24 816 | 118 |

A 6.5x–12.6x spread. **This is architectural, not effort:** the T-54 is not better built, it is on
a different pipeline. (Caveat: ~52% of the T-54's bake is interior geometry — `InteriorMachinery`
24%, `Ammunition` 19%, `InteriorPrimer` 9% — so its *exterior* is ~10.3k. The gap is 3x–6x on
exteriors, still architectural.)

## 3. The data/code seam in the blueprint

`t54_1951.blueprint.ron` carries `kind, hull, track, turret, gun, armor`. The nine-block visual
layer (`CompleteVisual`: hull, hull_plates, turret, turret_loft, gun, deck, fender, fittings,
detail) lives in **Rust** — `t54_hybrid.rs`, `t54_hybrid_turret.rs`. The T-54's hatch positions and
radii are hardcoded at `t54_hybrid.rs:223`.

The data mechanism exists: `tiger_i_ausf_e.visual.ron`. One vehicle uses it, for one part.

## 4. The material system was wired end to end and turned down to nothing

This is the finding that reframes the rest.

**The pipeline is complete and alive.** `vehicle_forge/src/artifact/` synthesizes four maps per
material family — albedo, **normal**, ao/roughness/metalness, cavity. `vehicle.wgsl` has both a
tangent-space normal-map branch (`mapping_mode == 0`) and a **triplanar** branch
(`TRIPLANAR_SCALE = 0.5`), plus wear grain, running-gear dust, wetness and wreck charring.
`cargo run -p tools -- forge-lineup --out target/forge` bakes artifacts; the client loads them
(measured: 8/8 load).

**The artifact is a cache, not a missing feature.** `artifact/default_materials.rs` states that a
clean checkout without `target/forge` must render the same five roles. Measured: rendering with
artifacts vs without is **0 pixels different**. That is correct behaviour.

**The defect was in the amplitudes.** `material_synthesis.rs::profile()`, in u8/255:

| role | normal_jitter | undulation | fine_grain | cavity_amp |
|---|---:|---:|---:|---:|
| RolledArmor | **3** | 2 | 3 | 6 |
| CastArmor | **5** | 7 | 2 | 3 |
| BarrelSteel | 2 | 1 | 2 | 3 |

A jitter of 3/255 is a **1.2% normal perturbation**; cast and rolled armour were separated by
2/255. The comment above the table claimed these values "still distinguish cast armour, plate,
painted barrel steel, tracks and rubber". They did not. **This — not any missing capability — is
why the fleet read as one flat plastic tone.**

Calibrated off a rendered ladder on `t54_studio` (x1 / x3 / x6 / x10, deltas against x1):

| level | jitter | changed px | max delta | read |
|---|---:|---:|---:|---|
| x1 (shipping) | 3/255 | — | — | smooth plastic; casting indistinguishable from plate |
| x3 | 9/255 | 8.6% | 26 | restrained; the casting starts to live |
| x6 | 21/255 | 15.6% | 49 | casting reads; turret edging toward sandy |
| x10 | 30/255 | 19.8% | 78 | too far — pumice, visible noise on the flank |

Shipped at **x4**, locked by `vehicle_forge/tests/material_floor.rs`.

**Still open, found while measuring:** the roughness channel has span 0 across all five families.
Every role answers the sun with one uniform gloss.

**Also open:** `vehicle.wgsl` clamps the texture layer with `min(material_id, 4u)`, so roles 5–11
(including `Glass`) sample the Rubber layer.

## 5. Negative results — things that do NOT move the picture

Recorded so they are not retried.

**Vertex-baked ambient occlusion on world geometry.** A full contact-occlusion pass was built
(disc-to-point form factor, spatial grid, deterministic, 5 tests green) and **reverted unmerged**.
On the tenement probe it changed 1.2% of pixels at a max delta of 15/255 — invisible at 4x zoom.
Raising the radius to 2.5 m, raising strength to 0.85, and adding a ground apron occluder each
moved it by nothing. Cost was 1.29 ms per tenement (~60 ms on an urban map load).

Three structural reasons: a flat facade is physically unoccluded (correct); the reveals that do
darken are 9 cm deep (0.20 m leaf, pane recessed 0.09) and about three pixels at 30 m; and
decisively, **vertex-baked shading has nowhere to live on walls whose vertices are metres apart**.
The world already has SSAO, enabled by default.

The same pass would work on **vehicles** — 21,508 triangles on a 6 m hull puts vertices
centimetres apart — where it could replace the four hand-authored `cavity.rs` bands.

**Material-role coverage alone.** PR #514 (the material law) is correct and worth keeping, but a
7 cm lens on an 8.5 cm lamp does not read at studio or gameplay resolution. It removes a class of
wrongness; it does not move the picture.

## 6. The pattern, fifth instance

The register above already names this. This audit adds four rows:

| lesson | applied | skipped |
|---|---|---|
| author the documented track shoe count | T-54, Tiger I, IS-3 | Tiger II, Jagdtiger, Panther II, Centurion, T-34-85 — shoes 1.65x–2.05x too long |
| lock the visible mesh inside its armour volume | T-54 turret, IS-3 pike | 7 of 9 vehicles |
| build parts through `VehiclePart` | T-54 | 8 of 9 vehicles |
| author visual data as `.visual.ron` | Tiger I, one part | everything else, in Rust |

## 7. How to evolve the core

The core does not need new capability. Everything reached for in this audit already existed:
normal mapping, per-role materials, a part system, per-part LOD, data-authored visuals, a golden
gate. **What is missing is universality, and a gate that can ask for it.**

`docs/vehicle-geometry-policy.md#the-material-law` states the mechanism: every budget in this
workspace is a **ceiling** derived from what a thing already costs, which can stop a regression but
can never ask for more quality. `scene_build/src/foliage.rs` names the failure — *"a ratchet
wearing a budget's name"*.

Three moves, in this order:

1. **Finish the migrations that were declared temporary.** The eight non-T-54 vehicles onto
   `VehiclePart`; the T-54's visual blocks into `.visual.ron`. Neither needs invention; both are
   documented as pending in the code that skips them.
2. **Make `GeneratorKind` derivable rather than typed**, or delete it. A label that cannot be
   checked makes the part report a claim, not a record. Retire `PartShape::Cast` with it, and the
   1,168 LOC it holds, unless a cast part proves the row.
3. **Add floors where a ceiling is doing a floor's job.** `material_floor.rs` and the material law
   are the first two. The construction floor is next, and it is the one this audit says matters
   most: shallow window reveals, stowage bins with no lids, periscopes as L-blocks, a Centurion
   hull with no fittings, and a T-54 that carries no external fuel tanks.

The ordering lesson from this session, stated plainly because it cost four wrong guesses:
**check what is already built and switched off before building anything new.**
