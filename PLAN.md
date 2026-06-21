# Procedural Geometry Kernel and Forge Program — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a coherent, renderer-neutral family of procedural geometry kernels for armored vehicles; use the T‑54‑3 obr. 1951 as the quality benchmark; progressively connect the result to Forge artifacts, LOD, and the PBR-lite renderer.

**Architecture:** Keep the hybrid principle: each physical class of tank part uses the representation best suited to it, while every kernel emits a common audited mesh contract. `vehicle_build` remains the executable assembly layer, `vehicle_forge` owns semantic review/baking, and rendering consumes baked artifacts without leaking GPU concepts into simulation or geometry crates.

**Tech stack:** Rust 2024, `glam`, existing `vehicle_geometry` mesh types, `sdf`/Surface Nets, `solid`, `revolve`, `cast_loft`, `vehicle_build`, `vehicle_forge`, `renderer_api`, `renderer_wgpu`, WGSL, deterministic CPU tests and headless screenshots.

---

## 1. Locked direction and non-negotiable boundaries

### 1.1 Kernel taxonomy

| Part nature | Production kernel | Why |
|---|---|---|
| Rolled/welded armour, glacis, casemate plates | `solid` + future `panel` | Exact plane normals, sharp seams, exact armour angle, low triangle cost |
| Designed cast shells: turrets, masks, rounded housings | renamed `cast_loft` | Direct silhouette control at every station; avoids “metaball lump” failure mode |
| Fluid unions, sockets, organic local transitions | `sdf` + `sdf_mesh` | CSG and smooth blending where a controlled station model would be awkward |
| Barrels, wheels, rollers, drums, sprockets | `revolve` | Exact axial symmetry and efficient radial resolution |
| Closed paths: tracks; later hoses, rails, welded bead paths | new `sweep` | A cross-section follows a stable path frame |
| Thin fabricated parts | new `panel` and `shell` | Thickness, hems, flanges, bends and hard seams without abusing cuboids |
| Bolts, weld seams, handles, casting marks | new `detail` and `scatter` | Semantic, deterministic decoration with LOD policy |
| Local visual asymmetry and wear | new bake-only `deform` | Controlled visual change that never changes collision or armour truth |
| Mesh boolean / subdivision | bake-only experimental CAD lane | Useful later, but never a mandatory runtime or mainstream vehicle dependency |

### 1.2 Explicit exclusions for normal production work

- No universal “one kernel for every tank part.”
- No runtime rebuild of complete vehicle geometry.
- No general-purpose scene graph, skeletal system, DCC dependency, or GPU dependency in geometry/Forge crates.
- No SDF meshing of armour plates merely to obtain one unified representation.
- No mesh boolean or subdivision dependency for T‑54, early vehicle migrations, collision, hit detection, or the main Forge path.
- No visual deformation that changes hitboxes, armour facets, module locations, mount frames, or authoritative replay state.

### 1.3 Naming cleanup

The committed `loft` crate overlaps conceptually with the older generic `vehicle_geometry::LoftSpec`.

- Rename the workspace package/crate `loft` to `cast_loft`.
- Rename public types to make the distinction visible:
  - `CrossSection` → `CastSection`
  - `AzimuthBump` → `CastBump`
  - `Caps` → `CastCaps`
  - `LoftSpec` → `CastLoftSpec`
  - `loft(...)` → `build_cast_loft(...)`
- Preserve `vehicle_geometry::LoftSpec` as the generic convex-section loft for fabrication and hull-like solids.
- Document the distinction:
  - generic loft = arbitrary convex 2D sections along a cardinal axis;
  - cast loft = superelliptic horizontal stations plus localized cast shaping.

This prevents future authors from putting a cast turret in a generic convex-hull loft or using a cast-specific superellipse API for armour plates.

---

## 2. Release sequence

The program ships as independently useful releases. No phase starts by deleting the previous path; each phase lands with tests, a documentation update, an atomic commit, and the canonical verification gate.

| Release | Result |
|---|---|
| R0 | Baseline and terminology fixed |
| R1 | Shared mesh-quality contract and robust cast-loft API |
| R2 | T‑54 cast-turret production benchmark through the new contract |
| R3 | Harden existing `solid`, `revolve`, `sdf`, and SDF meshing |
| R4 | General `sweep`, `panel`, and `shell` fabrication kernels |
| R5 | Semantic details, deterministic scatter, and bake-only deformation |
| R6 | Part-aware LOD and Forge source-of-truth convergence |
| R7 | Hybrid UV/triplanar surface mapping and material-family rendering |
| R8 | Per-vehicle Forge material baking, review assets, and runtime variation |
| R9 | Migrate the remaining production line |
| R10 | Isolated CAD research: mesh booleans and subdivision |

---

# R0 — Baseline, source ownership, and visual reference lock

### Task 1: Record the post-loft baseline

**Files:**

- Modify: `docs/hybrid-geometry-spike.md`
- Modify: `docs/vehicles/t-54.md`
- Create: `docs/procedural-kernel-program.md`
- Test: existing T‑54, Forge, client, and renderer test suites

- [ ] Record commit `2efa773` as the starting point: T‑54 now uses a cast loft for the turret shell, while SDF remains available for other cast work.
- [ ] Update the T‑54 document so it no longer states that the production turret shell is `sdf_mesh`.
- [ ] Add a concise “kernel selection matrix” to the new program document. It must include the table in section 1.1 and state that a part’s physical construction, not convenience, selects its generator.
- [ ] Add a “do not regress” reference list for the T‑54:
  - low, broad, front-heavy cast turret;
  - narrow turret ring and visible low overhang;
  - separate cupola;
  - mask covering the gun embrasure;
  - five large wheels per side;
  - continuous track belt and distinct end hardware;
  - sharp glacis and truthful armour normals.
- [ ] Run:

```powershell
cargo test -j 1 -p cast_loft -p vehicle_build --tests
cargo test -j 1 -p vehicle_forge
cargo test -j 1 -p client --test vehicle_asset_catalog
```

- [ ] Commit:

```powershell
git add docs/hybrid-geometry-spike.md docs/vehicles/t-54.md docs/procedural-kernel-program.md
git commit -m "docs(vehicle): define procedural kernel program"
```

### Acceptance

- T‑54’s current loft is explicitly the production benchmark, not a temporary side experiment.
- The source-of-truth rule is clear: shape generators are interchangeable behind a common mesh contract; gameplay remains independent of rendering technology.

---

# R1 — Shared mesh-quality contract and robust cast loft

## 3. Common quality contract

### Task 2: Add mesh audit types in `vehicle_geometry`

**Files:**

- Create: `crates/vehicle_geometry/src/quality.rs`
- Modify: `crates/vehicle_geometry/src/lib.rs`
- Modify: `crates/vehicle_geometry/src/mesh.rs`
- Create: `crates/vehicle_geometry/tests/mesh_quality.rs`

- [ ] Define a renderer-neutral audit API:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyExpectation {
    Any,
    Open,
    ClosedManifold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshQualitySpec {
    pub topology: TopologyExpectation,
    pub min_triangle_area: f32,
    pub normal_tolerance: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualityReport {
    pub vertices: usize,
    pub triangles: usize,
    pub bounds: Option<MeshBounds>,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
    pub degenerate_triangles: usize,
    pub invalid_indices: usize,
    pub non_finite_vertices: usize,
    pub non_unit_normals: usize,
    pub inconsistent_winding_edges: usize,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum MeshQualityError {
    #[error("mesh has {0} invalid indices")]
    InvalidIndices(usize),
    #[error("mesh has {0} non-finite vertex attributes")]
    NonFiniteVertices(usize),
    #[error("mesh has {0} degenerate triangles")]
    DegenerateTriangles(usize),
    #[error("mesh has {0} non-manifold edges")]
    NonManifoldEdges(usize),
    #[error("mesh is expected to be closed but has {0} boundary edges")]
    UnexpectedBoundaryEdges(usize),
    #[error("mesh has {0} non-unit normals")]
    NonUnitNormals(usize),
    #[error("mesh has {0} inconsistent winding edges")]
    InconsistentWinding(usize),
}
```

- [ ] Implement:

```rust
impl GeometryMesh {
    pub fn quality_report(&self, spec: MeshQualitySpec) -> MeshQualityReport;
    pub fn validate_quality(
        &self,
        spec: MeshQualitySpec,
    ) -> Result<MeshQualityReport, MeshQualityError>;
}
```

- [ ] Define edge accounting by undirected index pair:
  - one use = boundary edge;
  - exactly two oppositely directed uses = manifold and consistently wound;
  - more than two uses = non-manifold;
  - two uses in the same direction = inconsistent winding.
- [ ] Treat non-index-multiple-of-three as invalid primitive data; count its remainder as invalid indices rather than silently ignoring it.
- [ ] Treat a triangle as degenerate if it repeats an index or has squared doubled-area below `min_triangle_area`.
- [ ] Check position, normal, UV after R7, and `surface_shade` for finite values. The first version checks the currently stored attributes.
- [ ] Do not try to infer “outward” globally from an arbitrary mesh origin. The audit guarantees orientational consistency; shape-specific tests still prove semantic outwardness where that concept is meaningful.
- [ ] Write failing tests first for:
  - valid closed tetrahedron;
  - valid open quad;
  - invalid index;
  - repeated-index triangle;
  - zero-area three-point triangle;
  - one open edge in a supposedly closed mesh;
  - a non-manifold edge used by three triangles;
  - same-direction shared edge;
  - NaN position, normal, and shade.
- [ ] Run the new test until the failure is caused by missing audit behavior, then implement the smallest passing version.

### Task 3: Apply the quality contract to the existing kernel test suite

**Files:**

- Modify: `crates/vehicle_geometry/tests/kernel.rs`
- Modify: `crates/solid/src/convex.rs`
- Modify: `crates/revolve/src/revolve.rs`
- Modify: `crates/revolve/src/track.rs`
- Modify: `crates/sdf_mesh/src/surface_nets.rs`
- Modify: `crates/cast_loft/src/lib.rs`

- [ ] Replace duplicated assertions for finite vertices, valid indices, degenerate triangles, and closed-manifold checks with `validate_quality`.
- [ ] Keep generator-specific tests:
  - armour face normal matches the blueprint slope;
  - revolution caps face outward;
  - T‑54 cast shell is wider low than at the roof;
  - SDF box corners remain intentionally rounded;
  - tracks remain closed and non-self-degenerate.
- [ ] Define common specs:

```rust
pub const CLOSED_SMOOTH_MESH: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::ClosedManifold,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};

pub const OPEN_OR_CLOSED_MESH: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::Any,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};
```

- [ ] Commit:

```powershell
git add crates/vehicle_geometry crates/solid crates/revolve crates/sdf_mesh crates/cast_loft
git commit -m "feat(geometry): add common mesh quality contract"
```

## 4. Cast-loft correctness and expressive caps

### Task 4: Rename and separate the cast-loft crate

**Files:**

- Move: `crates/loft/` → `crates/cast_loft/`
- Modify: workspace `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/vehicle_build/Cargo.toml`
- Modify: all Rust imports and module documentation referring to the old crate

- [ ] Rename the package and Rust crate to `cast_loft`.
- [ ] Rename all public cast-loft types as specified in section 1.3.
- [ ] Verify there is no use of the ambiguous bare `LoftSpec` outside `vehicle_geometry` after the rename.
- [ ] Preserve the behavior of the current committed T‑54 mesh before functional cap changes.

### Task 5: Replace nullable apex caps with explicit cap policy

**Files:**

- Modify: `crates/cast_loft/src/lib.rs`
- Modify: `crates/vehicle_build/src/t54_turret_loft.rs`
- Modify: `crates/game_core/src/vehicle_blueprint/hybrid.rs`
- Modify: `crates/game_core/src/vehicle_blueprint/t54_hybrid.rs`
- Test: `crates/cast_loft/src/lib.rs`
- Test: `crates/vehicle_build/tests/t54_hybrid.rs`

- [ ] Replace `Option<Vec3>` caps with:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastCap {
    Open,
    Planar,
    Apex(Vec3),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastCaps {
    pub bottom: CastCap,
    pub top: CastCap,
}
```

- [ ] `Planar` must fan from the centroid of its terminal ring in the terminal station plane.
- [ ] `Apex` must retain current fan behavior, but validate that the apex is finite and not coincident with a ring edge.
- [ ] `Open` must emit no end faces and therefore require `TopologyExpectation::Any` or `Open` in tests.
- [ ] Convert the T‑54 shell to `Planar` bottom and top caps:
  - remove `roof_apex` and `floor_apex` from `TurretLoftVisual`;
  - preserve roof height through the final station;
  - preserve a watertight casting beneath its separate cupola;
  - avoid an artificial roof spike that can read as a pinched casting under grazing light.
- [ ] Add failing tests before implementation:
  - each cap mode has the expected boundary edge count;
  - planar caps lie on the terminal station plane;
  - capped cast loft validates as a closed manifold;
  - open cast loft validates as open but otherwise clean;
  - apex cap has no zero-area triangles.
- [ ] Keep the T‑54 silhouette and gameplay-plan tests passing after the cap migration.

### Task 6: Make cast-loft input failures explicit

**Files:**

- Modify: `crates/cast_loft/src/lib.rs`
- Modify: `crates/vehicle_build/src/t54_turret_loft.rs`
- Test: `crates/cast_loft/src/lib.rs`

- [ ] Add:

```rust
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CastLoftError {
    #[error("cast loft needs at least two stations")]
    TooFewStations,
    #[error("cast loft segment count must be at least 3")]
    TooFewSegments,
    #[error("station {index} is not finite")]
    NonFiniteStation { index: usize },
    #[error("station heights must strictly increase")]
    NonMonotonicStations,
    #[error("station {index} has non-positive extent")]
    NonPositiveExtent { index: usize },
    #[error("superellipse exponent must be finite and at least 2")]
    InvalidExponent,
    #[error("bump {index} has invalid width or position")]
    InvalidBump { index: usize },
    #[error("cap is invalid")]
    InvalidCap,
}
```

- [ ] Change production construction to:

```rust
pub fn try_build_cast_loft(
    spec: &CastLoftSpec<'_>,
) -> Result<GeometryMesh, CastLoftError>;
```

- [ ] Keep a temporary `build_cast_loft` convenience wrapper only within crate-private test/example code. Production vehicle builders must propagate or map the error to `BakeError`.
- [ ] Validate:
  - strictly increasing station height;
  - finite values;
  - positive half-width and front/rear lengths;
  - exponent `>= 2`;
  - bump widths `> 0`;
  - finite cap apexes.
- [ ] Add regression tests for every error variant.
- [ ] Commit:

```powershell
git add Cargo.toml Cargo.lock crates/cast_loft crates/vehicle_build crates/game_core
git commit -m "feat(cast-loft): add validated cap-aware cast shells"
```

### R1 acceptance

- Every generator can be tested against one common mesh-quality report.
- The T‑54 turret is a closed, validated planar-capped cast loft with no non-manifold edges, degenerate triangles, non-finite data, or invalid normals.
- `solid`, `revolve`, SDF mesh, track and cast-loft tests all use the same quality vocabulary.
- No geometry code has a renderer dependency.

---

# R2 — T‑54 as the full reference implementation

### Task 7: Turn the T‑54 into a kernel acceptance fixture

**Files:**

- Create: `crates/vehicle_build/tests/t54_kernel_contract.rs`
- Modify: `crates/vehicle_build/tests/t54_reference_quality.rs`
- Modify: `crates/vehicle_forge/tests/artifact.rs`
- Modify: `crates/client/examples/t54_loft_spike.rs`

- [ ] Test the T‑54 at `Lod0`, `Lod1`, and `Lod2`:
  - every submesh validates through `MeshQualitySpec`;
  - hull and turret remain inside their respective gameplay volumes;
  - gun can extend outside by design;
  - turret, trunnion and muzzle mount frames remain exactly unchanged;
  - LOD triangle counts fall monotonically;
  - LOD1 and LOD2 preserve all mount-critical forms;
  - LODs remain deterministic.
- [ ] Add shell-specific tests:
  - shell is lower and wider than tall;
  - low station is wider than roof station;
  - front reach exceeds rear reach at the matching height;
  - left/right cheeks are symmetric around the longitudinal plane;
  - front embrasure recess does not escape the turret plan;
  - cupola is separate from the shell and sits above the roof;
  - mantlet overlaps the recess lip but does not leave a visible front gap at neutral elevation.
- [ ] Extend the Forge artifact test to assert the artifact source hash changes if a cast-loft station, bump, cap mode, or material mapping changes.
- [ ] Preserve the multi-view headless render example as a human-review tool. It must emit front, top, three-quarter, rear-three-quarter and side views rather than only flattering angles.
- [ ] Add a deterministic CPU-raster silhouette test for the turret from front, side and top:
  - check non-empty alpha coverage;
  - lock broad proportions using coverage bounds;
  - do not commit large rendered PNG baselines as the primary regression oracle.
- [ ] Commit:

```powershell
git add crates/vehicle_build crates/vehicle_forge crates/client
git commit -m "test(t54): lock cast loft reference quality"
```

### R2 acceptance

The T‑54 is no longer only “a model that happens to render.” It is the executable reference for every later kernel: topology, silhouette, hitbox honesty, mounts, LOD and artifact determinism.

---

# R3 — Harden existing production kernels

## 5. `solid`: exact fabricated armour

### Task 8: Add fallible convex-solid construction

**Files:**

- Modify: `crates/solid/src/convex.rs`
- Modify: `crates/solid/src/lib.rs`
- Create: `crates/solid/tests/validation.rs`

- [ ] Add `ConvexSolidError` for:
  - fewer than four usable planes;
  - non-finite plane;
  - near-zero plane normal;
  - empty intersection;
  - no bounded corners;
  - face with fewer than three vertices after clipping.
- [ ] Add `Plane::try_new` and `ConvexSolid::try_new`.
- [ ] Make `to_mesh` return `Result<GeometryMesh, ConvexSolidError>` in production paths.
- [ ] Keep exact triple-plane intersections and face normals; do not replace the B-rep approach with sampling.
- [ ] Test:
  - valid box;
  - exact glacis slope;
  - duplicate plane;
  - zero normal;
  - contradictory half-spaces;
  - unbounded three-plane wedge;
  - clipped box remains a valid closed manifold.
- [ ] Keep a `box_at` convenience constructor that internally creates valid planes and cannot fail for finite positive extents.

## 6. `revolve`: explicit profile contract

### Task 9: Validate profile-driven revolutions

**Files:**

- Modify: `crates/revolve/src/revolve.rs`
- Create: `crates/revolve/src/profile.rs`
- Modify: `crates/revolve/src/lib.rs`
- Create: `crates/revolve/tests/profile_contract.rs`

- [ ] Introduce:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RevolveProfile {
    pub points: Vec<ProfilePoint>,
    pub caps: RevolveCaps,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum RevolveError {
    #[error("axis must be finite and non-zero")]
    InvalidAxis,
    #[error("profile needs at least two points")]
    TooFewPoints,
    #[error("profile point {index} is invalid")]
    InvalidPoint { index: usize },
    #[error("profile radius must be non-negative")]
    NegativeRadius,
    #[error("adjacent profile points are coincident")]
    CoincidentPoints,
    #[error("segments must be at least 3")]
    TooFewSegments,
}
```

- [ ] Preserve radius-zero cap points, but only when they form a non-degenerate adjacent transition.
- [ ] Parameterize the profile with cumulative arc length for later UV generation.
- [ ] Test axes X/Y/Z, cap winding, zero radius, negative radius, zero axis, duplicated points, unit normals and closed topology.
- [ ] Keep `merge` deliberately non-welding; add `merge_and_weld` only if a caller explicitly requests topology fusion.

## 7. `sdf` and `sdf_mesh`: robust local organic geometry

### Task 10: Validate SDF construction and add analytic normal support

**Files:**

- Modify: `crates/sdf/src/node.rs`
- Modify: `crates/sdf/src/shape.rs`
- Modify: `crates/sdf/src/lib.rs`
- Create: `crates/sdf/tests/validation.rs`
- Modify: `crates/sdf_mesh/src/surface_nets.rs`

- [ ] Add fallible constructors for sphere, cuboid, cylinder, half-space, transform, smooth operations.
- [ ] Reject:
  - negative radii or half-extents;
  - zero half-space normals;
  - non-finite rigid transforms;
  - non-unit or non-normalizable quaternions;
  - non-finite smooth radii.
- [ ] Add:

```rust
pub trait SignedDistance {
    fn eval(&self, point: Vec3) -> f32;
    fn gradient(&self, point: Vec3) -> Vec3;
}
```

- [ ] Implement exact gradients for sphere, cuboid outside regions, capped cylinder where unambiguous, half-space and rigid transforms.
- [ ] For boolean and smooth nodes, choose the branch gradient or blended gradient deterministically; use finite differences only at discontinuities or where analytic selection is undefined.
- [ ] Update Surface Nets to prefer `gradient` and retain a finite-difference fallback.
- [ ] Preserve the documented rule: SDF remains for cast transitions and sockets, not exact armour plates.

### Task 11: Improve SDF meshing without prematurely committing to an octree

**Files:**

- Modify: `crates/sdf_mesh/src/surface_nets.rs`
- Create: `crates/sdf_mesh/src/quality.rs`
- Modify: `crates/sdf_mesh/src/lib.rs`
- Extend: `crates/sdf_mesh/benches/meshing.rs`

- [ ] Add `SdfMeshingSpec`:

```rust
pub struct SdfMeshingSpec {
    pub bounds: MeshBounds,
    pub triangle_budget: usize,
    pub min_cells_per_axis: usize,
    pub max_cells_per_axis: usize,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}
```

- [ ] Return an `SdfMeshingResult` with mesh, selected grid, triangle count, budget utilization and maximum sampled residual.
- [ ] Replace hard-coded initial resolution `48` with a deterministic estimate constrained by the spec.
- [ ] Add a residual check: sampled generated vertices must stay within an explicit distance tolerance from the SDF surface.
- [ ] Add budget tests for sphere, blended turret fragment and deliberately empty field.
- [ ] Keep uniform Surface Nets as the standard production mesher for this release.
- [ ] Record octree/QEF dual contouring as an R10 prototype only; do not make it part of normal vehicle baking yet.
- [ ] Commit:

```powershell
git add crates/solid crates/revolve crates/sdf crates/sdf_mesh
git commit -m "feat(kernels): validate solid revolve and sdf generation"
```

### R3 acceptance

- No primitive generator silently creates undefined geometry from invalid input.
- SDF gradients are stable enough for cast-surface normals.
- Exact plate geometry remains exact and independent of SDF resolution.
- SDF budget results carry measurable quality data instead of only a triangle cap.

---

# R4 — New fabrication kernels

## 8. General path sweep

### Task 12: Extract the track-only path logic into `sweep`

**Files:**

- Create: `crates/sweep/Cargo.toml`
- Create: `crates/sweep/src/lib.rs`
- Create: `crates/sweep/src/frame.rs`
- Create: `crates/sweep/src/closed_path.rs`
- Create: `crates/sweep/tests/sweep_contract.rs`
- Modify: workspace `Cargo.toml`
- Modify: `crates/revolve/src/track.rs`
- Modify: `crates/revolve/src/lib.rs`

- [ ] Define a transport-frame sweep API:

```rust
pub struct SweepPath {
    pub points: Vec<Vec3>,
    pub closed: bool,
}

pub struct SweepSection {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

pub enum SweepFrameMode {
    ParallelTransport,
    FixedUp(Vec3),
}

pub struct SweepSpec<'a> {
    pub path: &'a SweepPath,
    pub section: &'a SweepSection,
    pub frame_mode: SweepFrameMode,
    pub caps: SweepCaps,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

pub fn try_sweep(spec: &SweepSpec<'_>) -> Result<GeometryMesh, SweepError>;
```

- [ ] Use parallel transport as the default frame: it minimizes twist along a spatial path and avoids the arbitrary roll artifacts that a naive Frenet frame produces on straight or inflected paths.
- [ ] For closed paths, distribute seam twist continuously across the loop so the final frame matches the first frame.
- [ ] Require a non-self-degenerate path and a convex section in the first release.
- [ ] Extract the T‑54 track belt to `sweep`; preserve its current silhouette, cross-section, outward normals and detail LOD behavior.
- [ ] Keep track pads and guide teeth as separate semantic detail geometry, not fused into the sweep itself.
- [ ] Test:
  - straight pipe;
  - planar closed rounded-rectangle path;
  - T‑54 track loop;
  - closed-manifold topology;
  - no frame flip along an S-shaped path;
  - deterministic seam;
  - explicit failure for zero-length segments, invalid up vector and concave section.
- [ ] Commit:

```powershell
git add Cargo.toml Cargo.lock crates/sweep crates/revolve
git commit -m "feat(sweep): add path sweep kernel and migrate tracks"
```

## 9. Panel and shell kernels

### Task 13: Add thin fabricated panels

**Files:**

- Create: `crates/panel/Cargo.toml`
- Create: `crates/panel/src/lib.rs`
- Create: `crates/panel/src/profile.rs`
- Create: `crates/panel/tests/panel_contract.rs`
- Modify: workspace `Cargo.toml`

- [ ] Define `PanelSpec` with a convex or simple polygon outline, normal direction, thickness, optional edge treatment and material/smoothing.
- [ ] First supported edge treatments:

```rust
pub enum PanelEdge {
    Sharp,
    Chamfer { width: f32 },
    Hem { width: f32, return_depth: f32 },
}
```

- [ ] Generate top, bottom, perimeter walls and optional edge geometry.
- [ ] Preserve exact planar faces and hard-edge smoothing by default.
- [ ] Use the panel kernel for a limited T‑54 target:
  - one engine-deck panel family;
  - one fender section family;
  - transmission-cover or grille frame parts.
- [ ] Do not replace exact `solid` hull plates with panels unless a part genuinely needs thin-sheet behavior.
- [ ] Test bounds, thickness, closed manifold, outward winding, chamfer dimensions and all edge-policy failures.

### Task 14: Add shelling for eligible open surfaces

**Files:**

- Create: `crates/shell/Cargo.toml`
- Create: `crates/shell/src/lib.rs`
- Create: `crates/shell/tests/shell_contract.rs`
- Modify: workspace `Cargo.toml`

- [ ] Define:

```rust
pub struct ShellSpec<'a> {
    pub surface: &'a GeometryMesh,
    pub thickness: f32,
    pub direction: ShellDirection,
    pub rim_policy: ShellRimPolicy,
}

pub enum ShellDirection {
    AlongVertexNormals,
    PositiveNormal,
    NegativeNormal,
}

pub enum ShellRimPolicy {
    Open,
    Bridge,
}
```

- [ ] Restrict v1 to an open, manifold input with consistent winding. Reject self-intersecting or non-manifold sources.
- [ ] Use it for basket-like sheets, splash guards and optional fender lips only after panel tests are stable.
- [ ] Never use `shell` on gameplay armour unless the outer visible surface remains the same exact armour surface used by the combat model.
- [ ] Commit panels and shell separately:

```powershell
git add Cargo.toml Cargo.lock crates/panel
git commit -m "feat(panel): add fabricated plate kernel"

git add Cargo.toml Cargo.lock crates/shell
git commit -m "feat(shell): add controlled surface thickening"
```

### R4 acceptance

- The track generator is a reusable path-sweep primitive rather than a vehicle-specific special case.
- Thin fabricated geometry is modeled as thin fabricated geometry, not as accidental stacks of boxes.
- New generators obey the same audit and deterministic-bake rules as existing kernels.

---

# R5 — Semantic detail, scatter and controlled deformation

## 10. Details attached to meaning, not random mesh vertices

### Task 15: Add semantic attachment frames

**Files:**

- Create: `crates/vehicle_build/src/attachment.rs`
- Modify: `crates/vehicle_build/src/part.rs`
- Modify: `crates/vehicle_build/src/description.rs`
- Modify: `crates/vehicle_forge/src/part_graph/types.rs`
- Create: `crates/vehicle_build/tests/attachments.rs`

- [ ] Add named local anchors on parts:

```rust
pub struct SurfaceAttachment {
    pub part: PartKey,
    pub local_frame: MountFrame,
    pub normal: Vec3,
    pub allowed_lods: PartLod,
}
```

- [ ] Examples for T‑54:
  - cupola hatch;
  - periscope bases;
  - DShK mount;
  - tow hooks;
  - engine-deck handles;
  - fender brackets;
  - mantlet seam.
- [ ] Attachments must be derived from the owning semantic part, never calculated from arbitrary final merged-mesh vertex indices.
- [ ] Test that attachments retain correct hull/turret/gun pose anchors and do not migrate across LOD boundaries unexpectedly.

### Task 16: Add deterministic detail and scatter kernels

**Files:**

- Create: `crates/detail/Cargo.toml`
- Create: `crates/detail/src/lib.rs`
- Create: `crates/detail/src/bolt.rs`
- Create: `crates/detail/src/weld.rs`
- Create: `crates/detail/src/scatter.rs`
- Create: `crates/detail/tests/determinism.rs`
- Modify: workspace `Cargo.toml`

- [ ] Add explicit primitives:
  - bolt head;
  - weld bead;
  - handle/rail segment via `sweep`;
  - casting seam;
  - louvre/slat array.
- [ ] Add scatter requests that require a stable seed, named attachment surface, spacing/radius constraints, material and LOD class.
- [ ] Use a stable hash of `(vehicle kind, part key, feature kind, ordinal)` rather than process-random hashing.
- [ ] Enforce exclusion zones around:
  - armour impact/hitbox-critical boundaries;
  - gun socket;
  - hatches;
  - other placed details.
- [ ] Make scatter output deterministic in order as well as shape.
- [ ] Add T‑54 details in three layers:
  - silhouette-critical: none through scatter;
  - close-range identifiable: weld seams, handle rails, cupola fittings;
  - micro-detail: bolts and small grille repetition.
- [ ] Set `PartLod::Detail` for micro-detail; retain only named silhouette or mount-critical details at lower LODs.

## 11. Bake-only deformation

### Task 17: Add controlled visual deformation after base geometry is valid

**Files:**

- Create: `crates/deform/Cargo.toml`
- Create: `crates/deform/src/lib.rs`
- Create: `crates/deform/tests/deformation_contract.rs`
- Modify: workspace `Cargo.toml`
- Modify: `crates/vehicle_build/src/part.rs`

- [ ] Define a constrained displacement API with explicit semantic scope:

```rust
pub enum DeformationKind {
    CastAsymmetry,
    ShallowDent,
    SurfaceWear,
}

pub struct DeformationSpec {
    pub kind: DeformationKind,
    pub center: Vec3,
    pub radius: f32,
    pub amplitude: f32,
    pub seed: u64,
}
```

- [ ] Deformation may apply only to a visual `GeometryMesh` after the base shape passes audit.
- [ ] It may not change:
  - `HitboxProfile`;
  - armour normals/facets used by penetration;
  - mount frames;
  - part anchors;
  - gameplay module locations.
- [ ] Clamp displacement to an explicitly configured visual tolerance inside the existing gameplay volume.
- [ ] Start with optional low-amplitude cast asymmetry on the T‑54 turret; do not use battle damage as the first case.
- [ ] Test determinism, bounds containment, finite normals after re-smoothing and no mount-frame drift.
- [ ] Commit:

```powershell
git add Cargo.toml Cargo.lock crates/detail crates/deform crates/vehicle_build crates/vehicle_forge
git commit -m "feat(forge): add semantic detail and visual deformation kernels"
```

### R5 acceptance

The T‑54 gains recognizable production detail without turning into random procedural noise, breaking LOD, or changing gameplay truth.

---

# R6 — Part-aware LOD and Forge source-of-truth convergence

## 12. Make part identity survive until LOD selection

### Task 18: Build LOD from semantic parts before flattening

**Files:**

- Modify: `crates/vehicle_build/src/part.rs`
- Modify: `crates/vehicle_build/src/description.rs`
- Modify: `crates/vehicle_build/src/t54.rs`
- Modify: `crates/vehicle_forge/src/production_bake.rs`
- Modify: `crates/vehicle_geometry/src/lod.rs`
- Create: `crates/vehicle_build/tests/part_lod.rs`

- [ ] Add a stable `PartKey` to `VehiclePart`.
- [ ] Keep the current three classes but unify their meaning with Forge:
  - `Silhouette`;
  - `MountCritical`;
  - `Detail`.
- [ ] Change production baking so it first calls `VehicleDescription::build_lod(profile.lod_level())`, then runs mesh reduction only on the retained result.
- [ ] Do not reduce a flattened LOD0 asset and call that “part-aware LOD.”
- [ ] For LOD policy:
  - LOD0: all parts;
  - LOD1: all silhouette and mount-critical parts; detail reduced or omitted per explicit part policy;
  - LOD2: hull/turret/gun silhouette plus essential running gear only; all micro-detail omitted.
- [ ] Preserve the three runtime groups: hull, turret and gun. Part granularity is authoring/LOD metadata, not a new runtime pose hierarchy.
- [ ] Test that omitted parts are exactly the intended keys; no mount-critical part disappears; all retained groups remain non-empty.

### Task 19: Replace global-only clustering with protected part reduction

**Files:**

- Modify: `crates/vehicle_geometry/src/lod.rs`
- Create: `crates/vehicle_geometry/src/lod/importance.rs`
- Create: `crates/vehicle_geometry/tests/part_aware_lod.rs`

- [ ] Extend LOD input with per-part importance metadata before merge.
- [ ] Use lower clustering cell size for silhouette and mount-critical meshes than for retained detail.
- [ ] Preserve hard edges and material boundaries in cluster keys.
- [ ] Add a post-reduction audit plus:
  - no bounds expansion outside LOD0;
  - no collapse of a required part to zero triangles;
  - no movement of mount frames;
  - turret roof and gun/mantlet silhouette retain minimum screen-independent geometric extents.
- [ ] Keep LOD deterministic and backend-neutral.
- [ ] Benchmark T‑54 LOD0/1/2 triangle counts and bake time; capture the measured limits in tests only after the new topology is approved.

## 13. Converge `vehicle_build` and Forge semantics

### Task 20: Make the Forge graph derive from executable parts

**Files:**

- Modify: `crates/vehicle_build/src/description.rs`
- Modify: `crates/vehicle_forge/src/part_graph.rs`
- Modify: `crates/vehicle_forge/src/part_data/t54.rs`
- Modify: `crates/vehicle_forge/tests/part_graph.rs`

- [ ] Stop deriving the T‑54 Forge part graph independently from the older flat blueprint fields.
- [ ] Expose a renderer-free `VehicleDescription::part_manifest()` that returns:
  - `PartKey`;
  - anchor;
  - material;
  - LOD class;
  - source note;
  - local bounds;
  - gameplay role.
- [ ] Build `ForgePartGraph` from this manifest.
- [ ] Keep `VehicleBlueprint` as a compatibility/prototype input where needed, but do not allow it to become a second semantic source for the same production part.
- [ ] Require every production part to have:
  - non-empty source note;
  - non-degenerate bounds;
  - known anchor/group;
  - explicit LOD policy;
  - material;
  - generator kind.
- [ ] Make Forge report the selected generator per part. Example: T‑54 turret shell = `cast_loft`, glacis = `solid`, barrel = `revolve`, belt = `sweep`.
- [ ] Commit:

```powershell
git add crates/vehicle_geometry crates/vehicle_build crates/vehicle_forge
git commit -m "feat(forge): preserve semantic parts through LOD baking"
```

### R6 acceptance

- LOD selection operates on named parts before meshes are merged.
- Forge reports what built each part and why.
- The T‑54 cannot silently drift between a blueprint, a semantic graph and executable geometry recipe.

---

# R7 — Hybrid UV/triplanar mapping and vehicle rendering

## 14. Extend the mesh contract with surface coordinates

### Task 21: Add explicit mapping mode and UV0 to geometry vertices

**Files:**

- Modify: `crates/vehicle_geometry/src/mesh.rs`
- Modify: `crates/vehicle_geometry/src/vehicle.rs`
- Modify: `crates/vehicle_forge/src/artifact/mesh_payload.rs`
- Modify: `crates/vehicle_forge/tests/artifact.rs`
- Modify: all production kernels

- [ ] Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceMapping {
    ParametricUv,
    Triplanar,
}

pub struct GeometryVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv0: Vec2,
    pub mapping: SurfaceMapping,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    pub surface_shade: f32,
}
```

- [ ] Set generator mapping policy:
  - `solid`: planar UV per face;
  - generic convex loft and `panel`: station/face chart UV;
  - `cast_loft`: azimuth as U, cumulative station distance as V;
  - `revolve`: azimuth as U, cumulative profile distance as V;
  - `sweep`: around-section U, cumulative path length V;
  - SDF/SDF mesh: triplanar;
  - scatter/deform: inherit from the host feature or use triplanar.
- [ ] Split vertices at UV seams. Do not weld vertices that require different UVs even if position, normal and smoothing group match.
- [ ] Update `weld_and_smooth` key to include UV and mapping mode where applicable.
- [ ] Bump Forge mesh payload to version 3:
  - encode UV and mapping mode;
  - decode version 2 by supplying `(0, 0)` and `Triplanar`;
  - write only version 3;
  - reject unknown future versions.
- [ ] Extend deterministic hashes to cover UV and mapping mode.
- [ ] Add audit checks for finite UVs; UV-range validation applies only to atlas-baked assets, not tiling parametric coordinates.

## 15. Make `VehicleVertex` carry mapping mode

### Task 22: Remove client-side box UV generation

**Files:**

- Modify: `crates/client/src/vehicle_pbr_mesh.rs`
- Modify: `crates/renderer_api/src/vehicle.rs`
- Modify: `crates/renderer_wgpu/src/vehicle_pipeline.rs`
- Modify: `crates/renderer_wgpu/src/shaders/vehicle.wgsl`
- Modify: `crates/renderer_wgpu/tests/wgsl_layout.rs`
- Modify: `crates/client/tests/vehicle_asset_catalog.rs`

- [ ] Extend `VehicleVertex` with a flat `mapping_mode: u32`.
- [ ] Update POD size, vertex attributes and WGSL locations together; add layout tests before the implementation change.
- [ ] Replace `box_uv` in `vehicle_submesh_vertices` with `vertex.uv0` and `vertex.mapping`.
- [ ] Generate tangents only for `ParametricUv` triangles.
- [ ] For `Triplanar` triangles, retain a valid fallback tangent but do not rely on it for normal reconstruction.
- [ ] Pass object-local position into the fragment shader so material coordinates remain stable while the hull rotates, turret traverses and gun elevates.

## 16. Implement triplanar shading without seams

### Task 23: Extend the WGSL vehicle shader

**Files:**

- Modify: `crates/renderer_wgpu/src/shaders/vehicle.wgsl`
- Modify: `crates/renderer_wgpu/tests/vehicle_render_frame.rs`
- Modify: `crates/client/examples/t54_views.rs`

- [ ] Add a mapping branch:
  - `ParametricUv`: use UV0 and tangent-space normal map as today;
  - `Triplanar`: sample X/Y/Z projections from object-local coordinates, weighted by normalized absolute object-space normal components.
- [ ] Construct a stable projected tangent frame for each triplanar axis before blending normal-map samples.
- [ ] Blend albedo, normal, AO/roughness and cavity consistently; do not blend only albedo while leaving a mismatched tangent-space normal.
- [ ] Use a small weight exponent to reduce projection seams without producing abrupt material bands.
- [ ] Preserve existing per-instance tint and material-ID behavior.
- [ ] Add headless visual tests for T‑54:
  - no NaN/shader validation errors;
  - rendering remains non-empty;
  - camera movement changes lighting but not material coordinate anchoring;
  - turret rotation does not cause visible texture swimming in the local turret frame.
- [ ] Commit:

```powershell
git add crates/vehicle_geometry crates/vehicle_forge crates/client crates/renderer_api crates/renderer_wgpu
git commit -m "feat(render): add hybrid vehicle surface mapping"
```

### R7 acceptance

- Every kernel has a declared material-coordinate strategy.
- Cast lofts, SDF blends and deformation can use seamless triplanar materials.
- Fabricated and axial parts use predictable parametric UVs.
- The renderer receives only baked, backend-neutral geometry attributes and remains separated from Forge.

---

# R8 — Material-family baking, review assets, and runtime variation

## 17. Replace one generic map set with role-aware material families

### Task 24: Bake deterministic material families

**Files:**

- Modify: `crates/vehicle_forge/src/artifact/texture_maps.rs`
- Modify: `crates/vehicle_forge/src/artifact/mod.rs`
- Modify: `crates/renderer_api/src/vehicle_asset.rs`
- Modify: `crates/client/src/vehicle_asset_catalog_loader.rs`
- Modify: `crates/renderer_wgpu/src/scene_renderer/vehicle_materials.rs`
- Modify: `crates/renderer_wgpu/src/shaders/vehicle.wgsl`

- [ ] Produce five role layers: rolled armour, cast armour, barrel steel, track metal and rubber.
- [ ] Keep map semantics consistent per layer:
  - albedo;
  - normal;
  - AO/roughness/metalness;
  - cavity.
- [ ] Use texture arrays so the shader selects the layer from `material_id`; do not split every vehicle into fifteen runtime mesh handles only to choose a material.
- [ ] Material synthesis requirements:
  - cast armour: low-frequency undulation, fine cast grain, muted cavity;
  - rolled armour: finer plate grain and weld-adjacent variation;
  - steel barrel: smoother, lower roughness;
  - track metal: dark, high cavity/roughness;
  - rubber: very dark and high roughness.
- [ ] Keep all procedural texture generation deterministic and artifact-contained.
- [ ] Update artifact manifest entries to identify material role and map semantic.
- [ ] Test clean fallback when one map/layer is absent or malformed.

## 18. Bake per-vehicle surface signals only after mapping is stable

### Task 25: Add optional vehicle-specific bake layers

**Files:**

- Create: `crates/vehicle_forge/src/artifact/surface_bake.rs`
- Modify: `crates/vehicle_forge/src/artifact/texture_maps.rs`
- Modify: `crates/vehicle_forge/tests/artifact.rs`

- [ ] Add optional deterministic bake passes for:
  - ambient-contact/cavity around turret ring, mantlet seat, wheel overlaps and track recesses;
  - local weld darkening;
  - panel-edge dirt accumulation;
  - low-amplitude cast variation.
- [ ] Only parametric-UV geometry receives rasterized per-vehicle layers in this release.
- [ ] Triplanar geometry keeps its tiled role material plus vertex `surface_shade` and optional procedural cavity.
- [ ] Store bake source hash and bake configuration in the manifest so artifacts are invalidated intentionally when the algorithm changes.
- [ ] Keep battle damage, decals, mud and camouflage as runtime overlays, not permanent changes to the base asset.

## 19. Add runtime variation after base materials are stable

### Task 26: Expand presentation-only variation

**Files:**

- Modify: `crates/client/src/vehicle_variation.rs`
- Modify: `crates/renderer_api/src/vehicle.rs`
- Modify: `crates/renderer_wgpu/src/shaders/vehicle.wgsl`
- Create: `crates/client/tests/vehicle_variation_material.rs`

- [ ] Add bounded overlays for dust, mud, snow, camouflage masks and module-state darkening.
- [ ] Keep them driven by presentation data and existing snapshots; they do not modify baked base geometry.
- [ ] Reserve decals/projected impacts for a later dedicated render pass rather than embedding them into every vehicle mesh.
- [ ] Test that variation cannot affect mesh handles, mount transforms, hitboxes, source hashes or simulation state.
- [ ] Commit:

```powershell
git add crates/vehicle_forge crates/client crates/renderer_api crates/renderer_wgpu
git commit -m "feat(forge): bake role-aware vehicle materials"
```

### R8 acceptance

T‑54 no longer relies on one neutral grain texture and box projection. Its cast turret, welded hull, barrel, tracks and rubber read as different physical surfaces under the same PBR-lite pipeline.

---

# R9 — Production vehicle migration program

## 20. Migration contract

Every migrated vehicle must ship with all of the following:

1. `ReferencePack` with ratio targets and source notes.
2. Executable `VehicleDescription` whose parts have stable semantic keys.
3. Forge graph derived from that executable description.
4. Appropriate kernel selection documented for each major part.
5. LOD0/1/2 part-aware bakes.
6. Mesh-quality audit passing for every output submesh.
7. Hitbox/armour/mount-frame honesty tests.
8. Material-family artifact and standard review renders.
9. Deterministic source hash, payload decode and client fallback test.
10. A photo-reference quality document and a concise explanation of intentional gameplay deviations.

## 21. Migration order

### Task 27: Jagdtiger — fixed casemate proof

- [ ] First migrated vehicle after T‑54.
- [ ] Prove `solid` and `panel` on large welded plates and casemate surfaces.
- [ ] Use `revolve` for wheels, gun and running gear; `sweep` for tracks.
- [ ] Explicitly test that turret yaw is ignored and gun elevation remains correctly attached to the fixed casemate.
- [ ] Add no cast-loft unless the historical geometry truly needs a cast component.

### Task 28: Tiger I — welded heavy turret proof

- [ ] Use exact fabricated panels/solids for hull and turret where historically appropriate.
- [ ] Use controlled panel seams, bolts and weld details.
- [ ] Ensure its character does not accidentally inherit T‑54’s cast-turret treatment.

### Task 29: Tiger II — sloped welded armour proof

- [ ] Stress exact plane normals and multi-panel hull transitions.
- [ ] Validate every visual armour facet against gameplay armour slope policy.
- [ ] Use part-aware LOD to protect the distinctive upper-hull and turret silhouette.

### Task 30: Panther II — interpretation-decision proof

- [ ] Begin only after an explicit reference/interpretation decision is documented.
- [ ] Treat uncertain historical details as named assumptions in its reference pack.
- [ ] Reuse kernels but do not copy T‑54 station or detail data.

### Task 31: Legacy handling

- [ ] Keep T‑55A and prototype kinds wire-compatible.
- [ ] Do not claim them as Forge-quality production assets until they satisfy the complete migration contract.
- [ ] Preserve legacy `vehicle_geometry::bake_vehicle` fallback until each kind is individually migrated.

### R9 acceptance

The program scales by family without spreading a low-quality generic template. Every migrated vehicle proves a different combination of the kernel library.

---

# R10 — Isolated CAD research lane

## 22. Mesh booleans

### Task 32: Prototype only after R4–R8 are stable

**Files:**

- Create: `crates/mesh_cad_experimental/`
- Create: `crates/mesh_cad_experimental/tests/boolean_contract.rs`
- Do not add to `vehicle_build` or `vehicle_forge` production dependencies initially.

- [ ] Implement union, subtraction and intersection only for closed, audited, manifold meshes.
- [ ] Define robust failure output rather than producing partial geometry.
- [ ] Test cube/cylinder and panel opening cases with topology audit.
- [ ] Benchmark determinism and error cases.
- [ ] Promote to production only if it solves a named vehicle feature that `solid`, `panel`, `shell`, SDF and detail kernels cannot express cleanly.

## 23. Subdivision / crease surface research

### Task 33: Prototype only as bake-time refinement

- [ ] Test Catmull-Clark-like subdivision or a crease-aware alternative on a small fabricated turret or fender sample.
- [ ] Require explicit crease tags and a post-subdivision quality audit.
- [ ] Never run it on gameplay-exact armour plates unless the exact collision/armour surface stays separate.
- [ ] Reject promotion if it merely smooths over a poor base silhouette that should instead be fixed in a loft, panel or solid definition.

### R10 acceptance

CAD power becomes available when it proves a concrete value, but cannot infect the reliable armored-vehicle production path by default.

---

# 24. Cross-cutting test and verification protocol

For every implementation task:

1. Write a narrow failing test before production code.
2. Run that exact test and confirm it fails for the missing behavior.
3. Implement the smallest behavior required.
4. Re-run the focused test.
5. Run the affected crate suite.
6. Run formatting in check mode.
7. Run clippy for changed crates with warnings denied.
8. Review `git diff --check`.
9. Commit one coherent change only.

Minimum focused commands:

```powershell
cargo fmt --all --check
cargo test -j 1 -p vehicle_geometry
cargo test -j 1 -p cast_loft
cargo test -j 1 -p solid
cargo test -j 1 -p revolve
cargo test -j 1 -p sdf -p sdf_mesh
cargo test -j 1 -p vehicle_build --tests
cargo test -j 1 -p vehicle_forge
cargo test -j 1 -p client --test vehicle_asset_catalog
cargo test -j 1 -p renderer_wgpu
cargo clippy -j 1 -p vehicle_geometry -p cast_loft -p solid -p revolve -p sdf -p sdf_mesh -p vehicle_build -p vehicle_forge -p client -p renderer_api -p renderer_wgpu --all-targets -- -D warnings
```

At each completed release:

```powershell
./scripts/verify.ps1
```

Required review evidence:

- quality report for every production T‑54 submesh and LOD;
- ratio report and source hash for every Forge artifact;
- CPU review render set for reference consistency;
- headless GPU render validation for shader/layout/material changes;
- no new source file over the repository’s 220-line production-file guard—split by responsibility rather than suppressing the rule.

---

# 25. Assumptions and defaults

- The existing committed cast-loft T‑54 is the canonical starting state.
- T‑54 remains the primary visual and technical benchmark until R8 completes.
- Geometry stays procedural-source-first and bake-first; glTF import remains optional and later.
- The canonical armored-battle constraints remain: desktop-native, large outdoor terrain, server-authoritative simulation, fixed-tick gameplay, strict render/simulation boundary.
- UV/triplanar mapping is hybrid: parameterized surfaces use deterministic UV0; SDF, blends and deformation use object-local triplanar mapping.
- Mesh booleans and subdivision are included only as a late, isolated, bake-only research lane.
- Runtime variation is presentation-only and cannot alter physics, hit detection, replay, mounts or armour truth.
- Full-workspace verification is mandatory at release boundaries; focused `-j 1` verification is acceptable during individual implementation tasks.
