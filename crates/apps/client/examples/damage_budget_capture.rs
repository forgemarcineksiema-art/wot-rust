//! The Honest Steel phase-8 performance gate, run as a measurement:
//!
//! ```text
//! cargo run --release -p client --example damage_budget_capture
//! ```
//!
//! Drives the representative hit sequence through the real bounded damage-mesh worker and prints
//! the rolling p95 numbers against the program budgets (worker build < 8 ms, per-frame main-thread
//! damage work < 0.5 ms). Exits non-zero when a budget is exceeded, so the gate is one command —
//! but it is a release-mode review gate, deliberately NOT a CI timing assert.

use client::capture_damage_mesh_budget;

const WORKER_BUDGET_MS: f32 = 8.0;
const MAIN_THREAD_BUDGET_MS: f32 = 0.5;

fn main() {
    let capture = capture_damage_mesh_budget();
    let worker = capture.report.worker_p95_ms;
    let main_thread = capture.main_thread_p95_ms;
    println!("Honest Steel damage-mesh budget capture (production T-54)");
    println!(
        "  hits {} -> scheduled {} bakes, completed {}",
        capture.hits, capture.scheduled, capture.completed
    );
    println!("  worker build p95      {worker:7.3} ms (budget {WORKER_BUDGET_MS} ms)");
    println!(
        "  integration p95       {:7.3} ms (inside the main-thread budget)",
        capture.report.integration_p95_ms
    );
    println!("  main-thread p95/frame {main_thread:7.3} ms (budget {MAIN_THREAD_BUDGET_MS} ms)");
    let ok = worker < WORKER_BUDGET_MS && main_thread < MAIN_THREAD_BUDGET_MS;
    println!("  verdict: {}", if ok { "WITHIN BUDGET" } else { "OVER BUDGET" });
    if !ok {
        std::process::exit(1);
    }
}
