//! Golden visual regression for the Forge Studio tiles — OPT-IN via `WOT_STUDIO_GOLDENS=1`
//! (skips silently otherwise, so `verify.ps1` stays fast until this gate is promoted).
//! Re-record with `WOT_UPDATE_GOLDENS=1`. The comparison is byte-exact: the CPU rasterizer is
//! deterministic on a machine (locked by `studio_bundle_is_deterministic`), and this repo's
//! merge gate is local verify — cross-platform tolerance can arrive with CI if CI ever does.

use std::path::PathBuf;

use game_core::VehicleKind;
use vehicle_forge::bake_studio_bundle;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("studio")
}

#[test]
fn studio_tiles_match_their_goldens() {
    if std::env::var("WOT_STUDIO_GOLDENS").as_deref() != Ok("1")
        && std::env::var("WOT_UPDATE_GOLDENS").as_deref() != Ok("1")
    {
        eprintln!("skipping studio goldens (set WOT_STUDIO_GOLDENS=1 to enable)");
        return;
    }
    let update = std::env::var("WOT_UPDATE_GOLDENS").as_deref() == Ok("1");

    for kind in VehicleKind::PLAYABLE {
        let bundle = bake_studio_bundle(kind).expect("studio bundle bakes");
        let dir = goldens_dir().join(kind.slug());
        std::fs::create_dir_all(&dir).expect("goldens dir");
        for view in bundle.views() {
            let golden_path = dir.join(view.name);
            if update {
                std::fs::write(&golden_path, &view.png).expect("write golden");
                continue;
            }
            let golden = std::fs::read(&golden_path).unwrap_or_else(|_| {
                panic!(
                    "missing golden {} — record with WOT_UPDATE_GOLDENS=1",
                    golden_path.display()
                )
            });
            assert_eq!(
                golden, view.png,
                "{:?} view {} drifted from its golden — if the geometry change is \
                 intentional, re-record with WOT_UPDATE_GOLDENS=1",
                kind, view.name
            );
        }
    }
}
