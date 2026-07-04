use std::{fs, path::PathBuf};

#[test]
fn combat_policy_doc_is_required() {
    let doc = fs::read_to_string(workspace_root().join("docs/combat-policy.md"))
        .expect("combat policy doc must exist");

    for required in [
        "server authoritative",
        "TankCommand.fire",
        "reload",
        "projectile",
        "hit detection",
        "armor facing",
        "penetration",
        "DamageEvent",
        "Snapshot",
        "replay regression",
    ] {
        assert!(doc.contains(required), "combat doc missing phrase: {required}");
    }
}

#[test]
fn sim_state_uses_combat_pipeline_not_placeholder_shell_motion() {
    let root = workspace_root();
    let state =
        fs::read_to_string(root.join("crates/runtime/sim/src/state.rs")).expect("sim state");
    let combat =
        fs::read_to_string(root.join("crates/runtime/sim/src/combat.rs")).expect("sim combat");
    // The shared shell-collision kernel (server + client reticle) owns armor-zone resolution.
    let trace = fs::read_to_string(root.join("crates/runtime/sim/src/shell_trace/tank.rs"))
        .expect("sim shell-trace tank helpers");

    assert!(state.contains("CombatTickContext"));
    assert!(state.contains("try_fire_shell"));
    assert!(combat.contains("resolve_penetration"));
    assert!(trace.contains("ArmorZone"));
    assert!(trace.contains("zone.facing()"));
    assert!(combat.contains("DamageEvent"));
}

#[test]
fn shell_state_does_not_duplicate_shell_damage_value() {
    let state = fs::read_to_string(workspace_root().join("crates/runtime/sim/src/state.rs"))
        .expect("sim state");

    assert!(
        !state.contains("pub damage_hp:"),
        "ShellState must use ShellSpec.damage_hp as the single source of truth"
    );
}

#[test]
fn client_can_send_fire_intent_to_authoritative_server() {
    let input = fs::read_to_string(workspace_root().join("crates/apps/client/src/app/input.rs"))
        .expect("client input");
    let dispatch =
        fs::read_to_string(workspace_root().join("crates/apps/client/src/app/loop_step.rs"))
            .expect("client fixed-step dispatch");

    assert!(input.contains("KeyCode::Space"));
    assert!(input.contains("fire_pending"));
    assert!(dispatch.contains("tick_with_input"));
    assert!(dispatch.contains("fire"));
}

fn workspace_root() -> PathBuf {
    // Layout-agnostic: the nearest ancestor whose Cargo.toml declares [workspace].
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
    {
        assert!(dir.pop(), "a Cargo.toml with [workspace] should exist in an ancestor");
    }
    dir
}
