//! The player's settings (interface program H22; the seed of P6): `settings.json` beside the
//! garage's save, on the same pattern — versioned, serde defaults, a corrupt or missing file
//! is the defaults, an atomic write. Today it holds one thing: the semantic palette. P6 adds
//! the rest (gain, sensitivity per zoom step, UI scale, the HUD preset, the Shift mode…).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ui_kit::theme::Palette;

use super::ClientApp;

const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Settings {
    pub version: u32,
    /// The semantic palette by slug: `standard`, `deuteranopia`, `protanopia`, `tritanopia`.
    /// A slug this build does not know reads as `standard`.
    #[serde(default = "default_palette")]
    pub palette: String,
}

fn default_palette() -> String {
    palette_slug(Palette::Standard).to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self { version: SAVE_VERSION, palette: default_palette() }
    }
}

pub(crate) fn palette_slug(palette: Palette) -> &'static str {
    match palette {
        Palette::Standard => "standard",
        Palette::Deuteranopia => "deuteranopia",
        Palette::Protanopia => "protanopia",
        Palette::Tritanopia => "tritanopia",
    }
}

pub(crate) fn palette_from_slug(slug: &str) -> Option<Palette> {
    Palette::ALL.into_iter().find(|palette| palette_slug(*palette) == slug)
}

impl Settings {
    pub fn palette(&self) -> Palette {
        palette_from_slug(&self.palette).unwrap_or(Palette::Standard)
    }

    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette_slug(palette).to_string();
    }
}

/// Where the settings live: `settings.json` beside `garage.json`.
pub(crate) fn settings_path() -> PathBuf {
    super::garage::persistence::config_dir()
        .map_or_else(|| PathBuf::from("settings.json"), |dir| dir.join("settings.json"))
}

/// The settings at `path`; a missing, unreadable or foreign-version file is `None` (the
/// defaults), the broken cases logged so a genuinely broken file is visible, never fatal.
pub(crate) fn load_settings(path: &Path) -> Option<Settings> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Settings>(&raw) {
        Ok(settings) if settings.version == SAVE_VERSION => Some(settings),
        Ok(settings) => {
            tracing::warn!(
                found = settings.version,
                expected = SAVE_VERSION,
                "settings version mismatch; using defaults"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "settings are unreadable; using defaults");
            None
        }
    }
}

/// Persist `settings` to `path` atomically (a sibling `.tmp` renamed over the target); a
/// failed write warns and is otherwise ignored.
pub(crate) fn store_settings(path: &Path, settings: &Settings) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create the settings directory");
        return;
    }
    let json = match serde_json::to_string_pretty(settings) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(%error, "could not serialize the settings");
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&tmp, json) {
        tracing::warn!(%error, "could not write the settings temp");
        return;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        tracing::warn!(%error, "could not replace the settings");
        let _ = std::fs::remove_file(&tmp);
    }
}

impl ClientApp {
    /// Turn on settings persistence at `path`, loading what is there first. Called once from
    /// the real startup path; `ClientApp::new` stays pure so tests never touch the user's file.
    pub(super) fn enable_settings_persistence(&mut self, path: PathBuf) {
        if let Some(settings) = load_settings(&path) {
            self.settings = settings;
        }
        self.settings_path = Some(path);
    }

    fn persist_settings(&self) {
        if let Some(path) = &self.settings_path {
            store_settings(path, &self.settings);
        }
    }

    /// The semantic palette the interface wears.
    pub(crate) fn palette(&self) -> Palette {
        self.settings.palette()
    }

    /// F9 (until P6's settings screen): the next palette on the ring; the choice persists.
    pub(super) fn cycle_palette(&mut self) {
        let all = Palette::ALL;
        let at = all.iter().position(|palette| *palette == self.palette()).unwrap_or(0);
        self.settings.set_palette(all[(at + 1) % all.len()]);
        self.persist_settings();
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wot-settings-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    /// H22: the palette round-trips through the file; a missing or corrupt file is the
    /// defaults; every palette has a slug of its own.
    #[test]
    fn the_palette_setting_survives_a_restart() {
        for palette in Palette::ALL {
            assert_eq!(palette_from_slug(palette_slug(palette)), Some(palette));
        }
        assert_eq!(palette_from_slug("mauve"), None);
        let path = settings_temp_path("round-trip");
        assert!(load_settings(&path).is_none(), "no file: the defaults");
        let mut settings = Settings::default();
        settings.set_palette(Palette::Tritanopia);
        store_settings(&path, &settings);
        assert_eq!(load_settings(&path), Some(settings.clone()));
        assert_eq!(load_settings(&path).expect("stored").palette(), Palette::Tritanopia);
        std::fs::write(&path, "{ not json").expect("scribble");
        assert!(load_settings(&path).is_none(), "a corrupt file is the defaults, never a panic");
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    /// The app starts on the file's palette and writes the next one on F9.
    #[test]
    fn the_app_reads_its_palette_at_startup_and_writes_it_on_a_cycle() {
        let path = settings_temp_path("startup");
        let mut stored = Settings::default();
        stored.set_palette(Palette::Protanopia);
        store_settings(&path, &stored);
        let mut app = ClientApp::new();
        assert_eq!(app.palette(), Palette::Standard, "pure until persistence is enabled");
        app.enable_settings_persistence(path.clone());
        assert_eq!(app.palette(), Palette::Protanopia);
        app.cycle_palette();
        assert_eq!(app.palette(), Palette::Tritanopia);
        assert_eq!(load_settings(&path).expect("written").palette(), Palette::Tritanopia);
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }
}
