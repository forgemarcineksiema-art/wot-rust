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
}

fn default_vehicle_artifact_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(FORGE_ASSET_DIR_ENV).map(PathBuf::from) {
        return Some(path);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/forge");
    root.exists().then_some(root)
}
