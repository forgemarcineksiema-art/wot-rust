// The gate holds rules about code. The combat policy lives at `docs/combat-policy.md`, cited
// here.
use quality::workspace_root;
use std::fs;

#[test]
fn sim_state_uses_combat_pipeline_not_placeholder_shell_motion() {
    let root = workspace_root();
    let state =
        fs::read_to_string(root.join("crates/runtime/sim/src/state.rs")).expect("sim state");
    let combat =
        fs::read_to_string(root.join("crates/runtime/sim/src/combat.rs")).expect("sim combat");
    // The shared shell-collision kernel (server + client reticle) owns armor-zone resolution:
    // the volume path in tank.rs plus the legacy band path split into legacy_boxes.rs.
    let trace = fs::read_to_string(root.join("crates/runtime/sim/src/shell_trace/tank.rs"))
        .expect("sim shell-trace tank helpers")
        + &fs::read_to_string(root.join("crates/runtime/sim/src/shell_trace/legacy_boxes.rs"))
            .expect("sim shell-trace legacy band helpers");

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
    assert!(dispatch.contains("tick_with_player_input"));
    assert!(dispatch.contains("fire"));
}
