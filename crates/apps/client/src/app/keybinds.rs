//! The keys as a table (interface program P7). Every key the game reads is an ACTION with a
//! context — the window's, the battle's, the garage's, the HUD editor's — bound in one table
//! with the World of Tanks defaults (the owner's decision: the layout and the keys 1:1, plus
//! our own), persisted to `keybinds.json` on the garage's pattern, its conflicts named. The
//! two hard-coded matches the client used to carry are routers over this table now, and a
//! source rule in `quality` keeps every `KeyCode` in the app out of every file but this one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Where a key means something. A key may mean different things in different contexts; two
/// actions of ONE context sharing a key is a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Context {
    /// The window's keys: they work everywhere and reach nothing underneath.
    Global,
    Battle,
    Garage,
    HudEditor,
    /// The shell's pages (P6): the settings page, later the keybinds and the results.
    Shell,
}

impl Context {
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [Context; 5] =
        [Context::Global, Context::Battle, Context::Garage, Context::HudEditor, Context::Shell];
}

/// Every action a key can mean. Append-only: the slugs are the file's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // The window's.
    ToggleFullscreen,
    CyclePalette,
    // The battle's.
    Forward,
    Back,
    Left,
    Right,
    Brake,
    CruiseUp,
    CruiseDown,
    CommandWheel,
    MinimapSize,
    HitLogFold,
    MarkTarget,
    Sniper,
    FreeLook,
    Fire,
    ToGarage,
    Ammo1,
    Ammo2,
    Ammo3,
    CameraToggle,
    Escape,
    Continue,
    // The garage's.
    GaragePrev,
    GarageNext,
    GarageConfirm,
    GarageBack,
    FocusPrev,
    FocusNext,
    CycleFocusedPrev,
    CycleFocusedNext,
    GarageAmmo1,
    GarageAmmo2,
    GarageAmmo3,
    GarageMap,
    Daylight,
    Inspector,
    Repair,
    TechTree,
    // The HUD editor's.
    EditorPresetMinimal,
    EditorPresetStandard,
    EditorPresetFull,
    EditorReset,
    EditorDone,
    /// The Control key the editor's reset waits for.
    EditorModifier,
    // The shell's pages (P6).
    MenuUp,
    MenuDown,
    MenuLeft,
    MenuRight,
    MenuAccept,
    MenuBack,
}

impl Action {
    pub const ALL: [Action; 51] = [
        Action::ToggleFullscreen,
        Action::CyclePalette,
        Action::Forward,
        Action::Back,
        Action::Left,
        Action::Right,
        Action::Brake,
        Action::CruiseUp,
        Action::CruiseDown,
        Action::CommandWheel,
        Action::MinimapSize,
        Action::HitLogFold,
        Action::MarkTarget,
        Action::Sniper,
        Action::FreeLook,
        Action::Fire,
        Action::ToGarage,
        Action::Ammo1,
        Action::Ammo2,
        Action::Ammo3,
        Action::CameraToggle,
        Action::Escape,
        Action::Continue,
        Action::GaragePrev,
        Action::GarageNext,
        Action::GarageConfirm,
        Action::GarageBack,
        Action::FocusPrev,
        Action::FocusNext,
        Action::CycleFocusedPrev,
        Action::CycleFocusedNext,
        Action::GarageAmmo1,
        Action::GarageAmmo2,
        Action::GarageAmmo3,
        Action::GarageMap,
        Action::Daylight,
        Action::Inspector,
        Action::Repair,
        Action::TechTree,
        Action::EditorPresetMinimal,
        Action::EditorPresetStandard,
        Action::EditorPresetFull,
        Action::EditorReset,
        Action::EditorDone,
        Action::EditorModifier,
        Action::MenuUp,
        Action::MenuDown,
        Action::MenuLeft,
        Action::MenuRight,
        Action::MenuAccept,
        Action::MenuBack,
    ];

    pub fn context(self) -> Context {
        use Action as A;
        match self {
            A::ToggleFullscreen | A::CyclePalette => Context::Global,
            A::Forward
            | A::Back
            | A::Left
            | A::Right
            | A::Brake
            | A::CruiseUp
            | A::CruiseDown
            | A::CommandWheel
            | A::MinimapSize
            | A::HitLogFold
            | A::MarkTarget
            | A::Sniper
            | A::FreeLook
            | A::Fire
            | A::ToGarage
            | A::Ammo1
            | A::Ammo2
            | A::Ammo3
            | A::CameraToggle
            | A::Escape
            | A::Continue => Context::Battle,
            A::GaragePrev
            | A::GarageNext
            | A::GarageConfirm
            | A::GarageBack
            | A::FocusPrev
            | A::FocusNext
            | A::CycleFocusedPrev
            | A::CycleFocusedNext
            | A::GarageAmmo1
            | A::GarageAmmo2
            | A::GarageAmmo3
            | A::GarageMap
            | A::Daylight
            | A::Inspector
            | A::Repair
            | A::TechTree => Context::Garage,
            A::EditorPresetMinimal
            | A::EditorPresetStandard
            | A::EditorPresetFull
            | A::EditorReset
            | A::EditorDone
            | A::EditorModifier => Context::HudEditor,
            A::MenuUp | A::MenuDown | A::MenuLeft | A::MenuRight | A::MenuAccept | A::MenuBack => {
                Context::Shell
            }
        }
    }

    pub fn slug(self) -> &'static str {
        use Action as A;
        match self {
            A::ToggleFullscreen => "toggle_fullscreen",
            A::CyclePalette => "cycle_palette",
            A::Forward => "forward",
            A::Back => "back",
            A::Left => "left",
            A::Right => "right",
            A::Brake => "brake",
            A::CruiseUp => "cruise_up",
            A::CruiseDown => "cruise_down",
            A::CommandWheel => "command_wheel",
            A::MinimapSize => "minimap_size",
            A::HitLogFold => "hit_log_fold",
            A::MarkTarget => "mark_target",
            A::Sniper => "sniper",
            A::FreeLook => "free_look",
            A::Fire => "fire",
            A::ToGarage => "to_garage",
            A::Ammo1 => "ammo_1",
            A::Ammo2 => "ammo_2",
            A::Ammo3 => "ammo_3",
            A::CameraToggle => "camera_toggle",
            A::Escape => "escape",
            A::Continue => "continue",
            A::GaragePrev => "garage_prev",
            A::GarageNext => "garage_next",
            A::GarageConfirm => "garage_confirm",
            A::GarageBack => "garage_back",
            A::FocusPrev => "focus_prev",
            A::FocusNext => "focus_next",
            A::CycleFocusedPrev => "cycle_focused_prev",
            A::CycleFocusedNext => "cycle_focused_next",
            A::GarageAmmo1 => "garage_ammo_1",
            A::GarageAmmo2 => "garage_ammo_2",
            A::GarageAmmo3 => "garage_ammo_3",
            A::GarageMap => "garage_map",
            A::Daylight => "daylight",
            A::Inspector => "inspector",
            A::Repair => "repair",
            A::TechTree => "tech_tree",
            A::EditorPresetMinimal => "editor_preset_minimal",
            A::EditorPresetStandard => "editor_preset_standard",
            A::EditorPresetFull => "editor_preset_full",
            A::EditorReset => "editor_reset",
            A::EditorDone => "editor_done",
            A::EditorModifier => "editor_modifier",
            A::MenuUp => "menu_up",
            A::MenuDown => "menu_down",
            A::MenuLeft => "menu_left",
            A::MenuRight => "menu_right",
            A::MenuAccept => "menu_accept",
            A::MenuBack => "menu_back",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|action| action.slug() == slug)
    }

    /// The defaults: World of Tanks 1:1 where World of Tanks has the key, ours where it does
    /// not (`docs/interface-program.md`, the key map).
    pub fn default_keys(self) -> &'static [KeyCode] {
        use Action as A;
        use KeyCode as K;
        match self {
            A::ToggleFullscreen => &[K::F11],
            A::CyclePalette => &[K::F9],
            A::Forward => &[K::KeyW, K::ArrowUp],
            A::Back => &[K::KeyS, K::ArrowDown],
            A::Left => &[K::KeyA, K::ArrowLeft],
            A::Right => &[K::KeyD, K::ArrowRight],
            A::Brake => &[K::ControlLeft, K::ControlRight],
            A::CruiseUp => &[K::KeyR],
            A::CruiseDown => &[K::KeyF],
            A::CommandWheel => &[K::KeyZ],
            A::MinimapSize => &[K::KeyM],
            A::HitLogFold => &[K::KeyN],
            A::MarkTarget => &[K::KeyT],
            A::Sniper => &[K::ShiftLeft, K::ShiftRight],
            A::FreeLook => &[K::AltLeft, K::AltRight],
            A::Fire => &[K::Space],
            A::ToGarage => &[K::KeyG],
            A::Ammo1 => &[K::Digit1],
            A::Ammo2 => &[K::Digit2],
            A::Ammo3 => &[K::Digit3],
            A::CameraToggle => &[K::KeyV],
            A::Escape => &[K::Escape],
            A::Continue => &[K::Enter],
            A::GaragePrev => &[K::ArrowLeft],
            A::GarageNext => &[K::ArrowRight],
            A::GarageConfirm => &[K::Enter],
            A::GarageBack => &[K::Escape],
            A::FocusPrev => &[K::BracketLeft],
            A::FocusNext => &[K::BracketRight],
            A::CycleFocusedPrev => &[K::KeyQ],
            A::CycleFocusedNext => &[K::KeyE],
            A::GarageAmmo1 => &[K::KeyZ],
            A::GarageAmmo2 => &[K::KeyX],
            A::GarageAmmo3 => &[K::KeyC],
            A::GarageMap => &[K::KeyM],
            A::Daylight => &[K::KeyL],
            A::Inspector => &[K::KeyI],
            A::Repair => &[K::KeyR],
            A::TechTree => &[K::KeyT],
            A::EditorPresetMinimal => &[K::Digit1],
            A::EditorPresetStandard => &[K::Digit2],
            A::EditorPresetFull => &[K::Digit3],
            A::EditorReset => &[K::KeyR],
            A::EditorDone => &[K::Escape],
            A::EditorModifier => &[K::ControlLeft, K::ControlRight],
            A::MenuUp => &[K::ArrowUp, K::KeyW],
            A::MenuDown => &[K::ArrowDown, K::KeyS],
            A::MenuLeft => &[K::ArrowLeft, K::KeyA],
            A::MenuRight => &[K::ArrowRight, K::KeyD],
            A::MenuAccept => &[K::Enter],
            A::MenuBack => &[K::Escape],
        }
    }
}

/// The keys a binding may use, by the name the file writes.
const BINDABLE: &[KeyCode] = &[
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Space,
    KeyCode::Enter,
    KeyCode::Escape,
    KeyCode::Tab,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::BracketLeft,
    KeyCode::BracketRight,
    KeyCode::Backquote,
    KeyCode::Minus,
    KeyCode::Equal,
    KeyCode::Semicolon,
    KeyCode::Quote,
    KeyCode::Comma,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Backslash,
    KeyCode::CapsLock,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
    KeyCode::NumpadAdd,
    KeyCode::NumpadSubtract,
    KeyCode::NumpadMultiply,
    KeyCode::NumpadDivide,
    KeyCode::NumpadEnter,
    KeyCode::NumpadDecimal,
];

/// The name the file writes for a key: winit's own.
pub fn key_name(key: KeyCode) -> String {
    format!("{key:?}")
}

pub fn key_from_name(name: &str) -> Option<KeyCode> {
    BINDABLE.iter().copied().find(|key| key_name(*key) == name)
}

/// A key as the interface prints it (P6): `KeyW` → `W`, `Digit1` → `1`, `ArrowUp` → `UP`,
/// `Escape` → `ESC`, `ShiftLeft` → `SHIFT`. The file keeps `key_name`.
pub fn key_label(key: KeyCode) -> String {
    let name = key_name(key);
    let short = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .or_else(|| name.strip_prefix("Arrow"))
        .unwrap_or(&name);
    let short = match short {
        "Escape" => "ESC",
        "ShiftLeft" | "ShiftRight" => "SHIFT",
        "ControlLeft" | "ControlRight" => "CTRL",
        "AltLeft" | "AltRight" => "ALT",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        other => other,
    };
    short.to_uppercase()
}

/// A key two actions of one context share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Conflict {
    pub context: Context,
    pub key: KeyCode,
    pub actions: [Action; 2],
}

/// The table: every action's keys. The defaults until the player binds otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyBindings {
    keys: BTreeMap<Action, Vec<KeyCode>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            keys: Action::ALL
                .into_iter()
                .map(|action| (action, action.default_keys().to_vec()))
                .collect(),
        }
    }
}

// The rebinding screen (with P6) is the reader of the table's editing API; until it lands the
// readers are the locks' alone.
#[cfg_attr(not(test), allow(dead_code))]
impl KeyBindings {
    /// The action `key` means in `context`, if any: the first in the table's order.
    pub fn action(&self, context: Context, key: PhysicalKey) -> Option<Action> {
        let PhysicalKey::Code(code) = key else { return None };
        Action::ALL
            .into_iter()
            .filter(|action| action.context() == context)
            .find(|action| self.keys.get(action).is_some_and(|keys| keys.contains(&code)))
    }

    pub fn keys(&self, action: Action) -> &[KeyCode] {
        self.keys.get(&action).map_or(&[], Vec::as_slice)
    }

    /// Bind `action` to `key` alone: the rebinding screen's word.
    pub fn bind(&mut self, action: Action, key: KeyCode) {
        self.keys.insert(action, vec![key]);
    }

    pub fn reset(&mut self, action: Action) {
        self.keys.insert(action, action.default_keys().to_vec());
    }

    /// Every key two actions of one context share, named.
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut found = Vec::new();
        for context in Context::ALL {
            let actions: Vec<Action> =
                Action::ALL.into_iter().filter(|a| a.context() == context).collect();
            for (i, a) in actions.iter().enumerate() {
                for b in &actions[i + 1..] {
                    for key in self.keys(*a) {
                        if self.keys(*b).contains(key) {
                            found.push(Conflict { context, key: *key, actions: [*a, *b] });
                        }
                    }
                }
            }
        }
        found
    }
}

// ---------------------------------------------------------------- the file

const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct BindingsFile {
    version: u32,
    #[serde(default)]
    bindings: BTreeMap<String, Vec<String>>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl KeyBindings {
    fn to_file(&self) -> BindingsFile {
        BindingsFile {
            version: SAVE_VERSION,
            bindings: self
                .keys
                .iter()
                .filter(|(action, keys)| keys.as_slice() != action.default_keys())
                .map(|(action, keys)| {
                    (action.slug().to_string(), keys.iter().map(|k| key_name(*k)).collect())
                })
                .collect(),
        }
    }

    /// The defaults with the file's rebindings over them; an unknown action or key is
    /// dropped, never guessed.
    fn from_file(file: &BindingsFile) -> Self {
        let mut bindings = Self::default();
        for (slug, names) in &file.bindings {
            let Some(action) = Action::from_slug(slug) else { continue };
            let keys: Vec<KeyCode> = names.iter().filter_map(|name| key_from_name(name)).collect();
            if !keys.is_empty() {
                bindings.keys.insert(action, keys);
            }
        }
        bindings
    }
}

/// Where the bindings live: `keybinds.json` beside the garage's save.
pub(crate) fn keybinds_path() -> PathBuf {
    crate::app::garage::persistence::config_dir()
        .map_or_else(|| PathBuf::from("keybinds.json"), |dir| dir.join("keybinds.json"))
}

pub(crate) fn load_keybinds(path: &Path) -> Option<KeyBindings> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<BindingsFile>(&raw) {
        Ok(file) if file.version == SAVE_VERSION => Some(KeyBindings::from_file(&file)),
        Ok(file) => {
            tracing::warn!(
                found = file.version,
                expected = SAVE_VERSION,
                "keybinds version mismatch; using the defaults"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "keybinds are unreadable; using the defaults");
            None
        }
    }
}

pub(crate) fn store_keybinds(path: &Path, bindings: &KeyBindings) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create the keybinds directory");
        return;
    }
    let json = match serde_json::to_string_pretty(&bindings.to_file()) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(%error, "could not serialize the keybinds");
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&tmp, json) {
        tracing::warn!(%error, "could not write the keybinds temp");
        return;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        tracing::warn!(%error, "could not replace the keybinds");
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl super::ClientApp {
    /// Turn on keybind persistence at `path`, loading what is there first (the real startup
    /// path; tests never touch the user's file).
    pub(super) fn enable_keybinds_persistence(&mut self, path: PathBuf) {
        if let Some(bindings) = load_keybinds(&path) {
            self.keybinds = bindings;
        }
        self.keybinds_path = Some(path);
    }

    pub(crate) fn keybinds(&self) -> &KeyBindings {
        &self.keybinds
    }

    /// Bind and write: the rebinding screen's one verb (the screen lands with P6).
    pub(crate) fn rebind(&mut self, action: Action, key: KeyCode) {
        self.keybinds.bind(action, key);
        if let Some(path) = &self.keybinds_path {
            store_keybinds(path, &self.keybinds);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P7: every action has a default key, its name round-trips through the file's spelling,
    /// and no two actions of one context share a key by default.
    #[test]
    fn every_action_has_a_default_key_and_no_two_share_one_in_a_context() {
        for action in Action::ALL {
            assert!(!action.default_keys().is_empty(), "{action:?} has no default key");
            assert_eq!(Action::from_slug(action.slug()), Some(action));
            for key in action.default_keys() {
                assert_eq!(
                    key_from_name(&key_name(*key)),
                    Some(*key),
                    "{key:?} is bindable by name"
                );
            }
        }
        let defaults = KeyBindings::default();
        assert!(defaults.conflicts().is_empty(), "{:?}", defaults.conflicts());
        // World of Tanks 1:1 where it has the key.
        assert_eq!(
            defaults.action(Context::Battle, PhysicalKey::Code(KeyCode::KeyT)),
            Some(Action::MarkTarget)
        );
        assert_eq!(
            defaults.action(Context::Battle, PhysicalKey::Code(KeyCode::KeyZ)),
            Some(Action::CommandWheel)
        );
        assert_eq!(
            defaults.action(Context::Battle, PhysicalKey::Code(KeyCode::ShiftLeft)),
            Some(Action::Sniper)
        );
        assert_eq!(
            defaults.action(Context::Garage, PhysicalKey::Code(KeyCode::KeyZ)),
            Some(Action::GarageAmmo1)
        );
        assert_eq!(
            defaults.action(Context::Battle, PhysicalKey::Code(KeyCode::F11)),
            None,
            "the window's keys are the window's"
        );
        assert_eq!(
            defaults.action(Context::Global, PhysicalKey::Code(KeyCode::F11)),
            Some(Action::ToggleFullscreen)
        );
        assert_eq!(
            defaults.action(
                Context::Battle,
                PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified)
            ),
            None
        );
    }

    /// A rebinding comes back from the file; a conflict is named — the key, the context, the
    /// two actions; a corrupt or missing file is the defaults.
    #[test]
    fn a_rebinding_survives_a_restart_and_a_conflict_is_named() {
        let mut bindings = KeyBindings::default();
        bindings.bind(Action::Fire, KeyCode::KeyW);
        let conflicts = bindings.conflicts();
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert_eq!(conflicts[0].context, Context::Battle);
        assert_eq!(conflicts[0].key, KeyCode::KeyW);
        assert_eq!(conflicts[0].actions, [Action::Forward, Action::Fire]);
        bindings.bind(Action::Fire, KeyCode::KeyH);
        assert!(bindings.conflicts().is_empty());
        let dir = std::env::temp_dir().join(format!("wot-keybinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("keybinds.json");
        assert!(load_keybinds(&path).is_none(), "no file: the defaults");
        store_keybinds(&path, &bindings);
        let back = load_keybinds(&path).expect("written");
        assert_eq!(back, bindings);
        assert_eq!(back.keys(Action::Fire), &[KeyCode::KeyH]);
        assert_eq!(
            back.keys(Action::Forward),
            Action::Forward.default_keys(),
            "the rest stays the design"
        );
        let raw = std::fs::read_to_string(&path).expect("file");
        assert!(
            raw.contains("\"fire\"") && raw.contains("\"KeyH\"") && !raw.contains("\"forward\""),
            "only the rebindings are written: {raw}"
        );
        std::fs::write(&path, "{ not json").expect("scribble");
        assert!(load_keybinds(&path).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        bindings.reset(Action::Fire);
        assert_eq!(bindings, KeyBindings::default());
    }

    /// P6: the labels the pages print for the keys — short, upper-case, never the file's name.
    #[test]
    fn a_key_label_is_the_short_word_the_pages_print() {
        assert_eq!(key_label(KeyCode::KeyW), "W");
        assert_eq!(key_label(KeyCode::Digit1), "1");
        assert_eq!(key_label(KeyCode::ArrowUp), "UP");
        assert_eq!(key_label(KeyCode::Escape), "ESC");
        assert_eq!(key_label(KeyCode::ShiftLeft), "SHIFT");
        assert_eq!(key_label(KeyCode::BracketLeft), "[");
        assert_eq!(key_label(KeyCode::F11), "F11");
        assert_eq!(key_label(KeyCode::Enter), "ENTER");
        assert_eq!(key_name(KeyCode::ArrowUp), "ArrowUp", "the file keeps the long name");
    }
}
