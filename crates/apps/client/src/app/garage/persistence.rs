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
    /// The edited per-slot rack fill. `serde(default)` (= `None` = stock-heavy default fill)
    /// keeps every pre-editor save loading unchanged; a stored fill that no longer fits the
    /// vehicle's capacity degrades to the default in `draft::from_saved`.
    #[serde(default)]
    pub ammo_counts: Option<[u16; game_core::MAX_AMMO_SLOTS]>,
    pub crew_proficiency: f32,
}

/// Current on-disk schema version. Bump when `GarageSave`/`SavedLoadout` change shape; an older
/// or newer version loads as "no save" rather than mis-parsing. v2 switched the vehicle identity
/// from the `VehicleKind` enum to its slug string, so removing/renaming a vehicle degrades one
/// entry instead of failing the whole parse.
const SAVE_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct GarageSave {
    pub version: u32,
    /// Slug of the selected vehicle ([`VehicleKind::slug`]). A slug, not the enum, so a save from a
    /// build that has since renamed/removed a vehicle still parses (the field just fails to resolve
    /// and falls back to stock) instead of poisoning the whole file.
    pub selected_vehicle: String,
    /// Per-vehicle edited loadouts keyed by slug; a vehicle absent here has never been edited
    /// (stock), and a slug this build no longer knows is skipped on load.
    pub loadouts: HashMap<String, SavedLoadout>,
    /// The manual daylight override (H1): "morning" / "day" / "evening", absent = follow the
    /// player's clock. `serde(default)` keeps every pre-H1 save loading unchanged, and an
    /// unknown value degrades to the clock instead of failing the parse.
    #[serde(default)]
    pub daylight_override: Option<String>,
}

/// The override's on-disk names — a slug per variant, like the vehicles, so a save from a
/// build with more daylights degrades one field instead of poisoning the file.
fn daylight_slug(light: scene_build::hangar::HangarLight) -> &'static str {
    use scene_build::hangar::HangarLight;
    match light {
        HangarLight::Morning => "morning",
        HangarLight::Day => "day",
        HangarLight::Evening => "evening",
    }
}

fn daylight_from_slug(slug: &str) -> Option<scene_build::hangar::HangarLight> {
    use scene_build::hangar::HangarLight;
    HangarLight::ALL.into_iter().find(|light| daylight_slug(*light) == slug)
}

impl GarageSave {
    pub(super) fn new(
        selected_vehicle: VehicleKind,
        loadouts: &HashMap<VehicleKind, SavedLoadout>,
    ) -> Self {
        Self {
            version: SAVE_VERSION,
            selected_vehicle: selected_vehicle.slug().to_string(),
            loadouts: loadouts.iter().map(|(kind, l)| (kind.slug().to_string(), *l)).collect(),
            daylight_override: None,
        }
    }

    /// The selected vehicle, or `None` if its slug no longer resolves in this build.
    fn selected_kind(&self) -> Option<VehicleKind> {
        VehicleKind::from_slug(&self.selected_vehicle)
    }

    /// The stored loadouts whose slug still resolves, re-keyed by kind; unknown slugs are dropped.
    fn resolved_loadouts(&self) -> HashMap<VehicleKind, SavedLoadout> {
        self.loadouts
            .iter()
            .filter_map(|(slug, l)| VehicleKind::from_slug(slug).map(|kind| (kind, *l)))
            .collect()
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
/// warns and is otherwise ignored — losing a loadout edit must never interrupt play. The write is
/// atomic: the JSON goes to a sibling `.tmp` and is renamed over the target, so an interrupted or
/// failed write can never truncate the real save (a half-written `garage.json` used to wipe every
/// stored loadout back to stock). `std::fs::rename` replaces an existing file on both Windows and
/// Unix.
pub(super) fn store(path: &Path, save: &GarageSave) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create garage save directory");
        return;
    }
    let json = match serde_json::to_string_pretty(save) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(%error, "could not serialize garage save");
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&tmp, json) {
        tracing::warn!(%error, "could not write garage save temp");
        return;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        tracing::warn!(%error, "could not replace garage save");
        let _ = std::fs::remove_file(&tmp);
    }
}

impl GarageState {
    /// Turn on disk persistence at `path`, loading any existing save first. Kept separate from
    /// `default()` so tests and offscreen renders stay pure (no filesystem); the real client calls
    /// this once at startup. Loading happens before the path is stored, so it does not rewrite the
    /// file it just read.
    pub(super) fn enable_persistence(&mut self, path: PathBuf) {
        // FIRST RUN of the game on this machine (E3): no save file has ever been written, so
        // the hangar plays its full entrance — the gate opens, the hero drives in, the door
        // closes behind it. Deliberately `!exists()`, NOT `load() == None`: a corrupt or
        // newer-version save is a veteran's install with a broken file, and replaying the
        // first-run ceremony over it would misread damage as novelty.
        if !path.exists() {
            self.start_drive_in();
        }
        match load(&path) {
            Some(save) => {
                self.saved = save.resolved_loadouts();
                self.daylight_override =
                    save.daylight_override.as_deref().and_then(daylight_from_slug);
                self.foreign_loadouts = save
                    .loadouts
                    .iter()
                    .filter(|(slug, _)| VehicleKind::from_slug(slug).is_none())
                    .map(|(slug, loadout)| (slug.clone(), *loadout))
                    .collect();
                if let Some(kind) = save.selected_kind()
                    && let Some(index) = VehicleKind::PLAYABLE.iter().position(|k| *k == kind)
                {
                    self.selected_index = index;
                }
                // The draft mirrors whatever ends up selected — INCLUDING the fallback when the
                // stored selection no longer resolves. Restoring it only on the happy path left
                // a stock draft standing in for the default vehicle's saved edits, and the first
                // persist then wrote that stock draft over them.
                let kind = self.selected_vehicle();
                self.draft = match self.saved.get(&kind) {
                    Some(saved) => LoadoutDraft::from_saved(kind, saved),
                    None => LoadoutDraft::for_vehicle(kind),
                };
            }
            None => back_up_undecipherable(&path),
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
        let mut save = GarageSave::new(self.selected_vehicle(), &loadouts);
        save.daylight_override = self.daylight_override.map(|l| daylight_slug(l).to_string());
        // Entries this build could not resolve ride along verbatim — a resolvable slug can
        // never collide with them, so `or_insert` is only belt-and-braces.
        for (slug, loadout) in &self.foreign_loadouts {
            save.loadouts.entry(slug.clone()).or_insert(*loadout);
        }
        store(path, &save);
    }
}

/// A save that exists but cannot be understood — a newer build's version, a hand-edit gone
/// wrong — is copied to a `.bak` sibling before this session's first persist overwrites it:
/// degrading to the stock garage is fine, DESTROYING what we failed to read is not. Best-effort
/// like every write here, and an existing backup is never clobbered (the first failure is the
/// copy closest to intact).
fn back_up_undecipherable(path: &Path) {
    if !path.exists() {
        return;
    }
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        return;
    }
    if let Err(error) = std::fs::copy(path, &backup) {
        tracing::warn!(%error, "could not back up the unreadable garage save");
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
            SavedLoadout {
                option_index: [0, 1, 0, 0, 0, 0],
                ammo_index: 1,
                ammo_counts: Some([20, 8, 6]),
                crew_proficiency: 0.9,
            },
        );
        GarageSave::new(VehicleKind::TigerII, &loadouts)
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
            .replace("\"version\": 2", "\"version\": 999");
        std::fs::write(&versioned, bumped).unwrap();
        assert_eq!(load(&versioned), None, "unknown version = stock garage");

        let _ = std::fs::remove_dir_all(corrupt.parent().unwrap());
        let _ = std::fs::remove_dir_all(versioned.parent().unwrap());
    }

    #[test]
    fn a_vanished_selected_vehicle_does_not_cost_the_default_vehicle_its_edits() {
        // The stored selection names a vehicle this build does not ship. The garage falls back
        // to the default vehicle — and must stand ITS saved edits up as the live draft: the old
        // happy-path-only restore left a stock draft there, and the very first persist then
        // wrote that stock draft over the player's edits.
        let path = temp_path("vanished-selected");
        let default_kind = VehicleKind::PLAYABLE[0];
        let edited = SavedLoadout {
            option_index: [0, 1, 0, 0, 0, 0],
            ammo_index: 1,
            ammo_counts: None,
            crew_proficiency: 0.7,
        };
        let mut loadouts = HashMap::new();
        loadouts.insert(default_kind, edited);
        let mut save = GarageSave::new(default_kind, &loadouts);
        save.selected_vehicle = "vehicle-this-build-never-shipped".to_string();
        store(&path, &save);

        let mut garage = GarageState::default();
        garage.enable_persistence(path.clone());
        garage.persist();

        let reloaded = load(&path).expect("the save still parses after the persist");
        let kept = reloaded.loadouts.get(default_kind.slug()).expect("the entry survives");
        assert_eq!(kept.option_index, edited.option_index, "the persist keeps the edits");
        assert!((kept.crew_proficiency - 0.7).abs() < 1.0e-6);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn loadouts_this_build_cannot_resolve_survive_the_persist_round_trip() {
        // An entry for a vehicle this build does not know (removed, or written by a newer
        // build) is not ours to spend: it rides along by slug and is written back verbatim,
        // for the build that can use it. Loading used to drop it; the first persist then
        // erased it from disk for good.
        let path = temp_path("foreign-loadouts");
        let foreign = SavedLoadout {
            option_index: [5; 6],
            ammo_index: 2,
            ammo_counts: Some([1, 2, 3]),
            crew_proficiency: 1.0,
        };
        let mut save = sample();
        save.loadouts.insert("future-vehicle".to_string(), foreign);
        store(&path, &save);

        let mut garage = GarageState::default();
        garage.enable_persistence(path.clone());
        garage.persist();

        let reloaded = load(&path).expect("still a valid save");
        let kept = reloaded.loadouts.get("future-vehicle").expect("the unknown slug survives");
        assert_eq!(kept.option_index, [5; 6], "...verbatim");
        assert_eq!(kept.ammo_counts, Some([1, 2, 3]));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_undecipherable_save_is_backed_up_before_the_first_persist_clobbers_it() {
        // Degrading to the stock garage is fine; destroying the only copy of what we failed to
        // read is not. A version from a newer build (or a hand-edit gone wrong) is copied to
        // `.bak` at load, so the overwrite that follows costs nothing irreplaceable.
        let path = temp_path("bak-on-mismatch");
        store(&path, &sample());
        let newer =
            std::fs::read_to_string(&path).unwrap().replace("\"version\": 2", "\"version\": 999");
        std::fs::write(&path, &newer).unwrap();

        let mut garage = GarageState::default();
        garage.enable_persistence(path.clone());
        garage.persist(); // previously this overwrote the only copy

        let backup = path.with_extension("json.bak");
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            newer,
            "the unreadable original survives in the backup"
        );
        assert!(load(&path).is_some(), "the main file is this build's fresh save again");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        // A merely MISSING file earns no backup — there is nothing to protect.
        let missing = temp_path("bak-none");
        let mut fresh = GarageState::default();
        fresh.enable_persistence(missing.clone());
        assert!(!missing.with_extension("json.bak").exists());
        let _ = std::fs::remove_dir_all(missing.parent().unwrap());
    }

    #[test]
    fn an_unknown_slug_is_skipped_without_dropping_the_rest() {
        // A save referencing a vehicle this build no longer has (renamed/removed) must not fail the
        // whole parse — the unknown entry is skipped and the known loadouts still load. The old
        // `HashMap<VehicleKind, _>` rejected the entire file on one unknown key.
        let path = temp_path("unknown-slug");
        store(&path, &sample());
        let raw = std::fs::read_to_string(&path).unwrap();
        // Inject a bogus slug entry into the on-disk loadout map (JSON ignores the indentation).
        let hacked = raw.replace(
            "\"loadouts\": {",
            "\"loadouts\": {\n\"ghost_tank_9000\": { \"option_index\": [0,0,0,0,0,0], \"ammo_index\": 0, \"crew_proficiency\": 0.5 },",
        );
        assert_ne!(hacked, raw, "the injection must have matched the loadouts map");
        std::fs::write(&path, &hacked).unwrap();

        let loaded = load(&path).expect("a save with an unknown slug still parses");
        let resolved = loaded.resolved_loadouts();
        assert!(resolved.contains_key(&VehicleKind::T54_1951), "the known loadout survives");
        assert_eq!(resolved.len(), 1, "the unknown slug is dropped, not fatal");
        // The selected vehicle still resolves (it is a known slug).
        assert_eq!(loaded.selected_kind(), Some(VehicleKind::TigerII));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_first_run_earns_the_entrance_and_a_veteran_install_skips_it() {
        // A machine with no save has never seen the hangar: the drive-in plays (E3). Any
        // EXISTING file — even one this build cannot read — marks a veteran install, and the
        // ceremony stays skipped: damage must not be misread as novelty.
        let fresh = temp_path("first-run");
        let mut garage = GarageState::default();
        garage.enable_persistence(fresh.clone());
        assert!(garage.is_driving_in(), "a fresh install drives the hero in");
        let _ = std::fs::remove_dir_all(fresh.parent().unwrap());

        let veteran = temp_path("veteran-run");
        store(&veteran, &sample());
        let mut garage = GarageState::default();
        garage.enable_persistence(veteran.clone());
        assert!(!garage.is_driving_in(), "a save on disk = no entrance replay");

        let broken = temp_path("broken-run");
        store(&broken, &sample());
        std::fs::write(&broken, b"{ not json").unwrap();
        let mut garage = GarageState::default();
        garage.enable_persistence(broken.clone());
        assert!(!garage.is_driving_in(), "a corrupt save is a veteran install, not a first run");

        let _ = std::fs::remove_dir_all(veteran.parent().unwrap());
        let _ = std::fs::remove_dir_all(broken.parent().unwrap());
    }

    #[test]
    fn the_daylight_override_survives_a_restart_and_garbage_degrades_to_the_clock() {
        use scene_build::hangar::HangarLight;
        let path = temp_path("daylight");
        let mut garage = GarageState::default();
        garage.enable_persistence(path.clone());
        garage.daylight_override = Some(HangarLight::Evening);
        garage.persist();

        let mut fresh = GarageState::default();
        fresh.enable_persistence(path.clone());
        assert_eq!(fresh.hangar_light(), HangarLight::Evening, "the evening choice survives");

        // A slug from a future build (or a hand-edit) degrades to the clock, never a panic.
        let raw = std::fs::read_to_string(&path).unwrap().replace("evening", "midnight");
        std::fs::write(&path, raw).unwrap();
        let mut degraded = GarageState::default();
        degraded.enable_persistence(path.clone());
        assert_eq!(degraded.daylight_override, None, "an unknown daylight follows the clock");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_store_leaves_no_temp_file_behind() {
        // The atomic write renames its `.tmp` over the target; on success no sibling temp lingers.
        let path = temp_path("atomic");
        store(&path, &sample());
        let tmp = path.with_extension("json.tmp");
        assert!(path.exists(), "the real save is written");
        assert!(!tmp.exists(), "the temp file is renamed away, not left behind");
        assert_eq!(load(&path), Some(sample()), "and it round-trips");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
