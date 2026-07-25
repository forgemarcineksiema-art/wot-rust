use std::path::{Path, PathBuf};

use tracing::warn;

use super::ClientApp;

const FORGE_ASSET_DIR_ENV: &str = "WOT_FORGE_ASSET_DIR";

impl ClientApp {
    /// Bake and queue EVERY vehicle kind in the current battle roster (Płynność 2.0 / F6).
    /// Before this, a vehicle's procedural bake + mesh/material GPU registration ran lazily on
    /// FIRST SIGHT — an enemy cresting a ridge mid-battle cost a full bake on the render
    /// thread, the exact "FPS drops when enemies appear" hitch. The roster is known from the
    /// first snapshot; pay the whole bill during deployment, when a hitch is invisible.
    pub(super) fn preload_battle_vehicle_assets(&mut self) {
        let kinds: std::collections::HashSet<game_core::VehicleKind> = self
            .render_state
            .latest_snapshot()
            .map(|snapshot| snapshot.tanks.iter().map(|tank| tank.vehicle).collect())
            .unwrap_or_default();
        for kind in kinds {
            let _ = self.vehicle_asset_catalog.vehicle_entry(kind);
        }
    }

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
        let missing_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/missing-forge-root");
        assert!(!missing_root.exists(), "fixture must model a clean checkout");
        assert_eq!(
            app.vehicle_asset_catalog
                .load_forge_artifact_tree(&missing_root)
                .expect("a missing optional Forge root is not an error"),
            0
        );

        app.prebake_playable_vehicle_assets();

        assert_eq!(
            app.vehicle_asset_catalog.cached_vehicle_count(),
            game_core::VehicleKind::PLAYABLE.len(),
            "every playable vehicle must have baked render assets before the battle starts"
        );
        assert_eq!(
            app.vehicle_asset_catalog.material_count(),
            1,
            "the fallback fleet must share one GPU material handle"
        );
        let pending = app.vehicle_asset_catalog.take_pending_vehicle_materials();
        assert_eq!(pending.len(), 1, "the fallback fleet queues one material upload");
        let families = &pending[0].1;
        assert_eq!(families.families().len(), 5);
        assert_ne!(
            families.layer(0).albedo().rgba(),
            families.layer(1).albedo().rgba(),
            "rolled and cast armour must not use duplicate albedo"
        );
        assert_ne!(
            families.layer(0).normal().rgba(),
            families.layer(1).normal().rgba(),
            "rolled and cast armour must not use duplicate normals"
        );
        assert_ne!(
            families.layer(0).albedo().rgba(),
            families.layer(0).normal().rgba(),
            "different map semantics must not collapse to one bitmap"
        );
        assert!(app.vehicle_asset_catalog.take_pending_vehicle_materials().is_empty());
    }
}
