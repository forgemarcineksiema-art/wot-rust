//! Garage persistence: the selected vehicle and each vehicle's edited loadout survive a client
//! restart. Stored as versioned JSON in the user's config dir. Everything here is best-effort —
//! a missing, unreadable, or version-mismatched file degrades to the stock garage, never a panic,
//! so a corrupt save can never brick the hangar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

use super::{GarageState, LoadoutDraft};

/// The persistable choices behind a `LoadoutDraft`: the option index per fitting slot, the ammo
/// selection, and the crew proficiency. Stored per vehicle so the garage remembers each tank's
/// loadout across restarts. Restoring re-applies these through the compatibility-checked install
/// path (`draft::from_saved`), so stale indices degrade to stock instead of forcing an invalid fit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(super) struct SavedLoadout {
    pub option_index: [usize; 6],
    pub ammo_index: usize,
    pub crew_proficiency: f32,
}

/// Current on-disk schema version. Bump when `GarageSave`/`SavedLoadout` change shape; an older
/// or newer version loads as "no save" rather than mis-parsing.
const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct GarageSave {
    pub version: u32,
    pub selected_vehicle: VehicleKind,
    /// Per-vehicle edited loadouts; a vehicle absent here has never been edited (stock).
    pub loadouts: HashMap<VehicleKind, SavedLoadout>,
}

impl GarageSave {
    pub(super) fn new(
        selected_vehicle: VehicleKind,
        loadouts: HashMap<VehicleKind, SavedLoadout>,
    ) -> Self {
        Self { version: SAVE_VERSION, selected_vehicle, loadouts }
    }
}

/// Where the save lives: `%APPDATA%/wot-prototype/garage.json` on Windows (or `$XDG_CONFIG_HOME`
/// / `$HOME/.config` elsewhere), falling back to `garage.json` next to the working directory when
/// no config dir is discoverable — so the game still remembers the loadout in odd environments.
pub(super) fn save_path() -> PathBuf {
    config_dir().map_or_else(|| PathBuf::from("garage.json"), |dir| dir.join("garage.json"))
}

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("wot-prototype"))
}

/// Load the save at `path`. Any failure — missing file, malformed JSON, or a version this build
/// does not understand — yields `None` (the stock garage), logging a warning for the malformed
/// cases so a genuinely broken save is visible without being fatal.
pub(super) fn load(path: &Path) -> Option<GarageSave> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<GarageSave>(&raw) {
        Ok(save) if save.version == SAVE_VERSION => Some(save),
        Ok(save) => {
            tracing::warn!(
                found = save.version,
                expected = SAVE_VERSION,
                "garage save version mismatch; using stock"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "garage save is unreadable; using stock");
            None
        }
    }
}

/// Persist `save` to `path`, creating the parent directory if needed. Best-effort: a failed write
/// warns and is otherwise ignored — losing a loadout edit must never interrupt play.
pub(super) fn store(path: &Path, save: &GarageSave) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create garage save directory");
        return;
    }
    match serde_json::to_string_pretty(save) {
        Ok(json) => {
            if let Err(error) = std::fs::write(path, json) {
                tracing::warn!(%error, "could not write garage save");
            }
        }
        Err(error) => tracing::warn!(%error, "could not serialize garage save"),
    }
}

impl GarageState {
    /// Turn on disk persistence at `path`, loading any existing save first. Kept separate from
    /// `default()` so tests and offscreen renders stay pure (no filesystem); the real client calls
    /// this once at startup. Loading happens before the path is stored, so it does not rewrite the
    /// file it just read.
    pub(super) fn enable_persistence(&mut self, path: PathBuf) {
        if let Some(save) = load(&path) {
            self.saved = save.loadouts;
            if let Some(index) =
                VehicleKind::PLAYABLE.iter().position(|kind| *kind == save.selected_vehicle)
            {
                self.selected_index = index;
                self.draft = match self.saved.get(&save.selected_vehicle) {
                    Some(saved) => LoadoutDraft::from_saved(save.selected_vehicle, saved),
                    None => LoadoutDraft::for_vehicle(save.selected_vehicle),
                };
            }
        }
        self.save_path = Some(path);
    }

    /// Best-effort write of the current garage to disk. A no-op until `enable_persistence` sets a
    /// path, which keeps the pure in-memory garage (tests, examples) off the filesystem.
    pub(super) fn persist(&self) {
        let Some(path) = &self.save_path else {
            return;
        };
        let mut loadouts = self.saved.clone();
        loadouts.insert(self.selected_vehicle(), self.draft.to_saved());
        store(path, &GarageSave::new(self.selected_vehicle(), loadouts));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("wot-garage-test-{}-{name}", std::process::id()));
        dir.join("garage.json")
    }

    fn sample() -> GarageSave {
        let mut loadouts = HashMap::new();
        loadouts.insert(
            VehicleKind::T54_1951,
            SavedLoadout { option_index: [0, 1, 0, 0, 0, 0], ammo_index: 1, crew_proficiency: 0.9 },
        );
        GarageSave::new(VehicleKind::TigerII, loadouts)
    }

    #[test]
    fn a_stored_save_round_trips_through_load() {
        let path = temp_path("roundtrip");
        let save = sample();
        store(&path, &save);
        assert_eq!(load(&path), Some(save));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_missing_or_corrupt_or_wrong_version_save_loads_as_none() {
        let missing = temp_path("missing");
        assert_eq!(load(&missing), None, "no file = stock garage");

        let corrupt = temp_path("corrupt");
        store(&corrupt, &sample()); // create the dir
        std::fs::write(&corrupt, b"{ not json").unwrap();
        assert_eq!(load(&corrupt), None, "garbage = stock garage, no panic");

        // A future/older schema version is treated as absent, not mis-parsed.
        let versioned = temp_path("version");
        store(&versioned, &sample());
        let bumped = std::fs::read_to_string(&versioned)
            .unwrap()
            .replace("\"version\": 1", "\"version\": 999");
        std::fs::write(&versioned, bumped).unwrap();
        assert_eq!(load(&versioned), None, "unknown version = stock garage");

        let _ = std::fs::remove_dir_all(corrupt.parent().unwrap());
        let _ = std::fs::remove_dir_all(versioned.parent().unwrap());
    }
}
