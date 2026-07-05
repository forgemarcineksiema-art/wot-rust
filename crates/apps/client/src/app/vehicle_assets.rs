use std::path::{Path, PathBuf};

use tracing::warn;

use super::ClientApp;

const FORGE_ASSET_DIR_ENV: &str = "WOT_FORGE_ASSET_DIR";

impl ClientApp {
    pub(super) fn new_with_default_vehicle_artifacts() -> Self {
        let root = default_vehicle_artifact_root();
        Self::new_with_vehicle_artifact_root(root.as_deref())
    }

    pub(super) fn new_with_vehicle_artifact_root(root: Option<&Path>) -> Self {
        let mut app = Self::new_without_vehicle_artifacts();
        if let Some(root) = root
            && let Err(error) = app.vehicle_asset_catalog.load_forge_artifact_tree(root)
        {
            warn!(%error, path = %root.display(), "failed to preload Forge vehicle artifacts");
        }
        app
    }

    /// Bake render assets for every playable vehicle up front (kinds a preloaded Forge artifact
    /// already covered are cached no-ops). Left to the lazy path instead, the first SIGHTING of
    /// an enemy kind mid-battle runs its whole procedural bake inside that render frame — a
    /// several-hundred-millisecond stall exactly when the shooting starts. Called once from the
    /// real startup path; tests construct `ClientApp` directly and skip the cost.
    pub(crate) fn prebake_playable_vehicle_assets(&mut self) {
        for kind in game_core::VehicleKind::PLAYABLE {
            if self.vehicle_asset_catalog.vehicle_entry(kind).is_none() {
                warn!(?kind, "failed to prebake vehicle render assets");
            }
        }
    }
}

fn default_vehicle_artifact_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(FORGE_ASSET_DIR_ENV).map(PathBuf::from) {
        return Some(path);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/forge");
    root.exists().then_some(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The startup prebake must cover the ENTIRE playable roster: any kind it misses gets baked
    /// lazily on first sighting, stalling a mid-battle frame for the whole procedural bake.
    #[test]
    fn prebake_covers_every_playable_vehicle() {
        let mut app = ClientApp::new_without_vehicle_artifacts();
        assert_eq!(app.vehicle_asset_catalog.cached_vehicle_count(), 0);

        app.prebake_playable_vehicle_assets();

        assert_eq!(
            app.vehicle_asset_catalog.cached_vehicle_count(),
            game_core::VehicleKind::PLAYABLE.len(),
            "every playable vehicle must have baked render assets before the battle starts"
        );
    }
}
