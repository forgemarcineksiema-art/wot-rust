use quality::is_test_module_file;
use quality::{rust_files, workspace_root};
use std::fs;

#[test]
fn simulation_render_separation_doc_is_required() {
    assert!(
        workspace_root().join("docs/simulation-render-separation.md").is_file(),
        "missing docs/simulation-render-separation.md"
    );
}

#[test]
fn sim_net_and_server_stay_free_of_render_frame_concepts() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for crate_name in ["sim", "net", "server"] {
        for source in rust_files(&quality::crate_src_dir(&root, crate_name)) {
            let text = fs::read_to_string(&source).expect("source should be readable");
            for forbidden in ["RenderFrame", "renderer_api", "renderer_wgpu", "wgpu::", "winit::"] {
                if text.contains(forbidden) {
                    offenders.push(format!("{} contains {forbidden}", source.display()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "authoritative simulation/server/network code must not depend on render frames:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn client_redraw_path_does_not_step_authoritative_simulation() {
    let dispatch =
        fs::read_to_string(workspace_root().join("crates/apps/client/src/app/loop_step.rs"))
            .expect("client loop dispatch should be readable");
    let redraw_index =
        dispatch.find("ClientLoopAction::RenderFrame").expect("render action exists");
    let fixed_tick_index =
        dispatch.find("ClientLoopAction::RunFixedTicks").expect("fixed tick action exists");

    assert!(
        fixed_tick_index < redraw_index,
        "fixed simulation ticks must be handled separately from render-frame actions"
    );
}

#[test]
fn client_does_not_own_authoritative_simulation_state() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for source in rust_files(&root.join("crates/apps/client/src")) {
        // Production client code must not own the authoritative sim. Test modules may use a
        // `SimulationState` as a parity oracle (e.g. predictor-vs-server lockstep checks).
        if is_test_module_file(&source) {
            continue;
        }
        let text = fs::read_to_string(&source).expect("client source should be readable");
        for forbidden in ["SimulationState", ".apply_commands(", ".spawn_tank("] {
            if text.contains(forbidden) {
                offenders.push(format!("{} contains {forbidden}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "client must send commands to a server instead of owning authoritative sim:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn client_vehicle_rendering_has_no_dynamic_fallback_mesh_path() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for source in rust_files(&root.join("crates/apps/client/src")) {
        let text = fs::read_to_string(&source).expect("client source should be readable");
        for forbidden in [
            "dynamic_fallback",
            "fallback_tanks",
            "append_tank_body",
            "append_tracks",
            "append_glacis",
        ] {
            if text.contains(forbidden) {
                offenders.push(format!("{} contains {forbidden}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "vehicle rendering must use baked objects for every vehicle, not the old dynamic fallback:\n{}",
        offenders.join("\n")
    );
}
