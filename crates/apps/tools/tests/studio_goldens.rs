//! Golden visual regression for the Forge Studio tiles — part of the merge gate, no opt-in.
//!
//! History this file exists to not repeat: the gate used to skip unless `WOT_STUDIO_GOLDENS=1`,
//! so `verify.ps1` never ran it and the tiles went stale fleet-wide without anyone seeing;
//! `assert_eq!` on two PNG byte vectors dumped ~50k tokens of raw bytes into the panic message
//! (unusable as a diagnosis); it panicked on the first mismatch, so one drifted vehicle hid the
//! other seven; and in update mode it rewrote every tile unconditionally — a bless with nothing
//! to review is a rubber stamp.
//!
//! Now: hashes in the failure message, every mismatch collected, and both the recorded and the
//! fresh PNG written to `target/studio-diff/<vehicle>/` so the author can look at what moved
//! before deciding it was intended.
//!
//! Re-record with `WOT_UPDATE_GOLDENS=1 cargo test -p tools --test studio_goldens` — as its own
//! deliberate commit, after looking at the diff directory.

use std::path::{Path, PathBuf};

use game_core::VehicleKind;
use vehicle_forge::bake_studio_bundle;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("studio")
}

fn diff_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/studio-diff")
}

/// FNV-1a over the PNG bytes: short enough to read in a panic message, specific enough that two
/// different tiles never collide in practice.
fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn write_diff(dir: &Path, name: &str, recorded: &[u8], fresh: &[u8]) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    let stem = name.trim_end_matches(".png");
    let recorded_path = dir.join(format!("{stem}.recorded.png"));
    let fresh_path = dir.join(format!("{stem}.fresh.png"));
    let _ = std::fs::write(&recorded_path, recorded);
    let _ = std::fs::write(&fresh_path, fresh);
    fresh_path
}

#[test]
fn studio_tiles_match_their_goldens() {
    let update = std::env::var("WOT_UPDATE_GOLDENS").as_deref() == Ok("1");
    let mut drifted: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut recorded_count = 0usize;
    let mut compared = 0usize;

    for kind in VehicleKind::PLAYABLE {
        let bundle = bake_studio_bundle(kind).expect("studio bundle bakes");
        let dir = goldens_dir().join(kind.slug());
        if update {
            std::fs::create_dir_all(&dir).expect("goldens dir");
        }
        for view in bundle.views() {
            let golden_path = dir.join(view.name);
            if update {
                std::fs::write(&golden_path, &view.png).expect("write golden");
                recorded_count += 1;
                continue;
            }
            let Ok(golden) = std::fs::read(&golden_path) else {
                missing.push(format!("{} / {}", kind.slug(), view.name));
                continue;
            };
            compared += 1;
            if golden != view.png {
                let out = write_diff(&diff_dir().join(kind.slug()), view.name, &golden, &view.png);
                drifted.push(format!(
                    "{} / {}: recorded {} vs fresh {} ({} vs {} bytes) — see {}",
                    kind.slug(),
                    view.name,
                    digest(&golden),
                    digest(&view.png),
                    golden.len(),
                    view.png.len(),
                    out.display(),
                ));
            }
        }
    }

    if update {
        // A bless is a deliberate act: say exactly what was rewritten so the commit can say it too.
        println!("re-recorded {recorded_count} studio tiles across the fleet");
        assert!(recorded_count > 0, "update mode must record something");
        return;
    }

    assert!(
        missing.is_empty(),
        "{} studio golden(s) are missing — record them with WOT_UPDATE_GOLDENS=1:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
    assert!(
        drifted.is_empty(),
        "{} studio tile(s) drifted from their goldens. Look at target/studio-diff/ FIRST; \
         re-record only if the change is intended, in its own commit:\n  {}",
        drifted.len(),
        drifted.join("\n  ")
    );
    assert!(compared >= 40, "the fleet's tile gate must actually compare tiles (got {compared})");
}
