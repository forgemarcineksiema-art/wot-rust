//! The player's settings (interface program H22; the seed of P6): `settings.json` beside the
//! garage's save, on the same pattern — versioned, serde defaults, a corrupt or missing file
//! is the defaults, an atomic write. Today it holds one thing: the semantic palette. P6 adds
//! the rest (gain, sensitivity per zoom step, UI scale, the HUD preset, the Shift mode…).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ui_kit::theme::Palette;

use super::ClientApp;

const SAVE_VERSION: u32 = 1;

/// The zoom steps a sensitivity is read per: the third-person view, then the scope's five.
pub(crate) const SENSITIVITY_STEPS: usize = 1 + crate::camera::zoom::SNIPER_FOV_STEPS_DEGREES.len();

/// The sensitivity multiplier's range and step (P6).
pub(crate) const SENSITIVITY_RANGE: [f32; 2] = [0.25, 3.0];
pub(crate) const SENSITIVITY_STEP: f32 = 0.05;
/// The interface scale's range and step (P6): one `u` is `ui_scale` pixels at 1080p.
pub(crate) const UI_SCALE_RANGE: [f32; 2] = [0.8, 1.5];
pub(crate) const UI_SCALE_STEP: f32 = 0.05;
pub(crate) const GAIN_STEP: f32 = 0.05;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Settings {
    pub version: u32,
    /// The semantic palette by slug: `standard`, `deuteranopia`, `protanopia`, `tritanopia`.
    /// A slug this build does not know reads as `standard`.
    #[serde(default = "default_palette")]
    pub palette: String,
    /// The mixer's master gain, 0 to 1.
    #[serde(default = "default_gain")]
    pub master_gain: f32,
    /// Mouse sensitivity per zoom step (P6): a multiplier over the sight's own scale, one for
    /// the third-person view and one for each of the scope's magnifications.
    #[serde(default = "default_sensitivity")]
    pub sensitivity: [f32; SENSITIVITY_STEPS],
    /// The interface's scale: one `u` is `ui_scale` pixels at 1080p.
    #[serde(default = "default_scale")]
    pub ui_scale: f32,
    /// Whether the sniper key TOGGLES the scope (World of Tanks, the default) or holds it.
    #[serde(default = "default_sniper_toggle")]
    pub sniper_toggle: bool,
    /// Borderless fullscreen at start (P9): what F11 last left.
    #[serde(default)]
    pub borderless: bool,
    /// The hall's daylight by slug — `auto`, `morning`, `day`, `evening`; `None` is a file from
    /// before the setting existed, which leaves the garage file's own word standing (the
    /// daylight lived there first).
    #[serde(default)]
    pub daylight: Option<String>,
}

fn default_palette() -> String {
    palette_slug(Palette::Standard).to_string()
}

fn default_gain() -> f32 {
    0.85
}

fn default_sensitivity() -> [f32; SENSITIVITY_STEPS] {
    [1.0; SENSITIVITY_STEPS]
}

fn default_scale() -> f32 {
    1.0
}

fn default_sniper_toggle() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            palette: default_palette(),
            master_gain: default_gain(),
            sensitivity: default_sensitivity(),
            ui_scale: default_scale(),
            sniper_toggle: default_sniper_toggle(),
            borderless: false,
            daylight: None,
        }
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

    /// The sensitivity multiplier for a zoom step (0 = third person, 1.. = the scope's).
    pub fn sensitivity_for(&self, step: usize) -> f32 {
        self.sensitivity[step.min(SENSITIVITY_STEPS - 1)]
            .clamp(SENSITIVITY_RANGE[0], SENSITIVITY_RANGE[1])
    }

    /// The hall's daylight override; `None` is the player's own clock — or, when the setting
    /// was never written, no word at all (`daylight_set`).
    pub fn daylight(&self) -> Option<scene_build::hangar::HangarLight> {
        self.daylight.as_deref().and_then(crate::app::garage::persistence::daylight_from_slug)
    }

    pub fn daylight_set(&self) -> bool {
        self.daylight.is_some()
    }

    pub fn set_daylight(&mut self, light: Option<scene_build::hangar::HangarLight>) {
        self.daylight =
            Some(light.map_or("auto", crate::app::garage::persistence::daylight_slug).to_string());
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
        self.apply_settings();
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

    /// The settings as held; the locks read them today, the keybinds page (P8) next.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Change the settings and apply them at once; the file follows.
    pub(crate) fn edit_settings(&mut self, edit: impl FnOnce(&mut Settings)) {
        edit(&mut self.settings);
        self.apply_settings();
        self.persist_settings();
    }

    /// Every setting applied where it lives (P6): the mixer's gain, the hall's daylight, the
    /// window's fullscreen (P9). The palette, the sensitivity and the UI scale are read where
    /// they are used, every frame.
    pub(crate) fn apply_settings(&mut self) {
        let gain = self.settings.master_gain;
        if let Some(audio) = &self.audio {
            audio.with_engine(|engine| engine.set_master_gain(gain));
        }
        if self.settings.daylight_set() {
            self.garage.set_daylight_override(self.settings.daylight());
        }
        self.apply_fullscreen_setting();
    }

    /// The window follows the borderless setting (P9); F11 writes it. Borderless, never
    /// exclusive: the same picture with no mode switch, no black flash on an alt-tab, and the
    /// OS compositor's own vsync.
    pub(super) fn apply_fullscreen_setting(&mut self) {
        self.fullscreen = self.settings.borderless;
        if let Some(window) = &self.window {
            window.set_fullscreen(
                self.fullscreen.then_some(winit::window::Fullscreen::Borderless(None)),
            );
        }
    }

    /// The zoom step the sensitivity is read for: the third-person view, or the scope's
    /// magnification nearest the current field of view.
    pub(crate) fn zoom_step(&self) -> usize {
        if self.camera_controller.mode() != crate::BattleCameraMode::Sniper {
            return 0;
        }
        let fov = self.camera_controller.sniper_fov_degrees();
        let steps = crate::camera::zoom::SNIPER_FOV_STEPS_DEGREES;
        let nearest = steps
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - fov).abs().total_cmp(&(b.1 - fov).abs()))
            .map_or(0, |(index, _)| index);
        1 + nearest
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

    /// P6: every setting round-trips through the file; a missing file is the defaults; a file
    /// from before the list (the palette alone) loads with the defaults for the rest and no
    /// word on the daylight.
    #[test]
    fn every_setting_round_trips_through_the_file_and_a_missing_file_is_the_defaults() {
        let path = settings_temp_path("every-setting");
        assert!(load_settings(&path).is_none(), "no file: the defaults");
        let mut settings = Settings::default();
        settings.set_palette(Palette::Protanopia);
        settings.master_gain = 0.4;
        settings.sensitivity = [1.5, 0.9, 0.8, 0.7, 0.6, 0.5];
        settings.ui_scale = 1.25;
        settings.sniper_toggle = false;
        settings.borderless = true;
        settings.set_daylight(Some(scene_build::hangar::HangarLight::Evening));
        store_settings(&path, &settings);
        assert_eq!(load_settings(&path), Some(settings.clone()));
        assert_eq!(
            load_settings(&path).expect("stored").daylight(),
            Some(scene_build::hangar::HangarLight::Evening)
        );
        std::fs::write(&path, r#"{"version":1,"palette":"deuteranopia"}"#).expect("an old file");
        let old = load_settings(&path).expect("an old file still loads");
        assert_eq!(old.palette(), Palette::Deuteranopia);
        assert_eq!(
            (old.master_gain, old.sensitivity, old.ui_scale, old.sniper_toggle, old.borderless),
            (0.85, [1.0; SENSITIVITY_STEPS], 1.0, true, false),
            "the rest is the defaults"
        );
        assert!(!old.daylight_set() && old.daylight().is_none(), "no word on the daylight");
        let mut auto = Settings::default();
        auto.set_daylight(None);
        assert!(auto.daylight_set() && auto.daylight().is_none(), "AUTO is a word");
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
