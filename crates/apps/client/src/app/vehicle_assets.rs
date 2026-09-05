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

    /// Bake the render assets of ONE not-yet-cached playable vehicle; `false` once every kind is
    /// in hand. The garage calls this once per frame: the whole roster is baked within a few
    /// frames of the window opening, each bake costing one garage frame instead of all of them
    /// costing the window before it opened. A deploy before the ring is done still bakes the
    /// battle's roster itself (`preload_battle_vehicle_assets`), so no first sighting mid-battle
    /// ever pays for a bake (F6).
    pub(crate) fn prebake_next_playable_vehicle(&mut self) -> bool {
        let Some(kind) = game_core::VehicleKind::PLAYABLE
            .iter()
            .copied()
            .find(|kind| !self.vehicle_asset_catalog.vehicles.contains_key(kind))
        else {
            return false;
        };
        if self.vehicle_asset_catalog.vehicle_entry(kind).is_none() {
            warn!(?kind, "failed to prebake vehicle render assets");
        }
        true
    }

    /// Bake render assets for every playable vehicle at once (kinds a preloaded Forge artifact
    /// already covered are cached no-ops) — the whole ring of
    /// [`Self::prebake_next_playable_vehicle`] in one call, for tests that need the roster in
    /// hand. The running client steps instead.
    #[cfg(test)]
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

    /// The garage's per-frame bake: one vehicle per call, never two, until every playable kind
    /// is cached — then nothing, so a long garage stay costs no repeated work.
    #[test]
    fn the_roster_bakes_one_vehicle_per_garage_frame_until_it_is_whole() {
        let mut app = ClientApp::new_without_vehicle_artifacts();
        let total = game_core::VehicleKind::PLAYABLE.len();
        let mut steps = 0;
        while app.prebake_next_playable_vehicle() {
            steps += 1;
            assert_eq!(
                app.vehicle_asset_catalog.cached_vehicle_count(),
                steps.min(total),
                "each step bakes exactly one more vehicle"
            );
            assert!(steps <= total, "the ring ends when the roster is whole");
        }
        assert_eq!(steps, total);
        assert_eq!(app.vehicle_asset_catalog.cached_vehicle_count(), total);
        assert!(!app.prebake_next_playable_vehicle(), "a whole roster has nothing left to bake");
    }

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
        // Bound to the contract, not to a literal. This was `5`, and it was the third place the
        // layer count was written down by hand — after `VehicleMaterialFamilies::LAYERS` and the
        // array type in `default_materials.rs`. One fact held as three numbers is how the shader
        // ended up clamping twelve roles into five layers in the first place.
        assert_eq!(families.families().len(), renderer_api::VehicleMaterialFamilies::LAYERS);
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
