// No physics engine on the authoritative path. Rapier left the workspace 2026-08-02 (audit D6: an
// API surface consumed only by its own tests) and parry3d followed on 2026-09-07 (the one
// program's X9: a footprint query with zero production callers). Every real path — SAT
// footprints, heightmap stepping, the support envelope, the Jacobi contact solver — is custom
// deterministic code (`docs/physics-policy.md`). Re-adding either is a design decision with its
// own row in `docs/game-design.md`, never a side effect of a dependency bump.
use quality::workspace_root;
use std::fs;

#[test]
fn no_physics_engine_returns_as_a_dependency() {
    let manifest =
        fs::read_to_string(workspace_root().join("Cargo.toml")).expect("workspace manifest exists");
    for engine in ["rapier", "parry"] {
        assert!(
            !manifest.contains(engine),
            "`{engine}` is back in the workspace manifest; the physics engine decision              (`docs/game-design.md` row 1, `docs/program.md` X9) is not reopened by a Cargo edit"
        );
    }
}
