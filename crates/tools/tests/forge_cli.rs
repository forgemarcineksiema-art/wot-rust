use std::process::Command;

#[test]
fn forge_vehicle_cli_writes_artifact_folder() {
    let out = temp_path("forge_vehicle_cli");
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale cli artifact");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_tools"))
        .args(["forge-vehicle", "--vehicle", "t54-1951", "--profile", "lod0", "--out"])
        .arg(&out)
        .output()
        .expect("run forge-vehicle");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out.join("manifest.json").is_file());
    assert!(out.join("meshes.bin").is_file());
    assert!(out.join("albedo.png").is_file());
    assert!(out.join("normal.png").is_file());
    assert!(out.join("ao_roughness.png").is_file());
    assert!(out.join("cavity.png").is_file());
    assert!(out.join("report.md").is_file());
    assert!(out.join("review").join("front.png").is_file());
    assert!(out.join("review").join("battle_oblique.png").is_file());

    let manifest = std::fs::read_to_string(out.join("manifest.json")).expect("manifest json");
    assert!(manifest.contains("\"vehicle_slug\": \"t54-1951\""));
    assert!(manifest.contains("\"profile\": \"lod0\""));

    std::fs::remove_dir_all(out).expect("remove cli artifact");
}

#[test]
fn forge_report_cli_writes_ratio_markdown_only() {
    let out = temp_path("forge_report_cli.md");
    if out.exists() {
        std::fs::remove_file(&out).expect("remove stale report");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_tools"))
        .args(["forge-report", "--vehicle", "t54-1951", "--out"])
        .arg(&out)
        .output()
        .expect("run forge-report");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let report = std::fs::read_to_string(&out).expect("report markdown");
    assert!(report.contains("T-54 Forge reference report"));
    assert!(report.contains("GunProtrusionToHullLength"));

    std::fs::remove_file(out).expect("remove report");
}

#[test]
fn forge_lineup_cli_writes_benchmark_artifacts_and_index() {
    let out = temp_path("forge_lineup_cli");
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale lineup");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_tools"))
        .args(["forge-lineup", "--out"])
        .arg(&out)
        .output()
        .expect("run forge-lineup");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out.join("index.md").is_file());
    assert!(out.join("t54-1951").join("manifest.json").is_file());
    assert!(out.join("t54-1951").join("albedo.png").is_file());
    assert!(out.join("t54-1951").join("review").join("front.png").is_file());
    assert!(
        !out.join("t55a").exists(),
        "T-55A is legacy-compatible only and must not be emitted by forge-lineup"
    );

    let index = std::fs::read_to_string(out.join("index.md")).expect("lineup index");
    assert!(index.contains("Armored Vehicle Forge lineup"));
    assert!(index.contains("t54-1951"));
    assert!(!index.contains("t55a"));

    std::fs::remove_dir_all(out).expect("remove lineup");
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wot_{name}_{}", std::process::id()))
}
