//! Locks the shell's settings page (interface program P6): the way in from the escape menu,
//! the keys, the mouse, the file, the way out.

use winit::keyboard::{KeyCode, PhysicalKey};

use super::ClientApp;
use super::keybinds::{Action, KeyBindings};
use super::shell::shell_footer;
use crate::hud::elements::{HudElement, ShellPart};
use crate::hud::pause_menu::{PauseMenuButton, SETTINGS_CENTER};
use crate::hud::shell::SettingsRow;

fn key(code: KeyCode) -> PhysicalKey {
    PhysicalKey::Code(code)
}

/// P6: SETTINGS on the escape menu opens the page; the shell's keys walk the rows and step
/// the values, applied at once and written to the file; the page never drives; a click on a
/// row's arrow steps that row; Esc goes back to the menu.
#[test]
fn the_settings_page_opens_from_the_escape_menu_steps_a_setting_and_escapes_back() {
    let dir = std::env::temp_dir().join(format!("wot-settings-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("settings.json");
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.enable_settings_persistence(path.clone());
    app.open_pause_menu();
    app.pause_menu.as_mut().expect("menu").cursor_clip = SETTINGS_CENTER;
    assert_eq!(app.pause_menu.expect("menu").hovered(), Some(PauseMenuButton::Settings));
    app.pause_menu_primary_press();
    assert!(app.shell_open() && app.pause_menu.is_none(), "the page is up, the menu is down");
    // The page has the keys: W is MENU UP here (it wraps to the last row), never the throttle.
    app.on_battle_keyboard(key(KeyCode::KeyW), true);
    assert!(!app.input.forward, "the page never drives");
    app.on_battle_keyboard(key(KeyCode::KeyW), false);
    // Down past the top to MASTER VOLUME, right steps it; the file follows.
    app.on_battle_keyboard(key(KeyCode::ArrowDown), true);
    app.on_battle_keyboard(key(KeyCode::ArrowDown), true);
    app.on_battle_keyboard(key(KeyCode::ArrowRight), true);
    assert!((app.settings().master_gain - 0.90).abs() < 1e-5, "{}", app.settings().master_gain);
    let written = super::settings::load_settings(&path).expect("written");
    assert!((written.master_gain - 0.90).abs() < 1e-5);
    // Up to PALETTE, left wraps the ring.
    app.on_battle_keyboard(key(KeyCode::ArrowUp), true);
    app.on_battle_keyboard(key(KeyCode::ArrowLeft), true);
    assert_eq!(app.palette(), ui_kit::theme::Palette::Tritanopia);
    // The end of a range is a refusal, not a wrap.
    for _ in 0..40 {
        app.adjust_setting(SettingsRow::MasterVolume, 1);
    }
    assert!((app.settings().master_gain - 1.0).abs() < 1e-5);
    assert!(!app.adjust_setting(SettingsRow::MasterVolume, 1), "nowhere to go");
    // The mouse: a click on a row's > arrow steps that row, whichever row was selected.
    let ui = ui_kit::ui::Ui::new(1920, 1080, 1.0);
    let mut model = crate::hud::demo::demo_model(false);
    model.shell = app.shell_model();
    let list = crate::hud::build_battle_hud_list(&model, &ui);
    app.remember_shell_hits(crate::hud::shell::shell_hit_rects(&list));
    let inc = list.find(HudElement::Shell(ShellPart::RowInc(8))).expect("UI SCALE's arrow").rect;
    app.shell_cursor(inc.center());
    app.shell_press();
    assert!((app.settings().ui_scale - 1.05).abs() < 1e-5, "{}", app.settings().ui_scale);
    // Esc: back to the menu.
    app.on_battle_keyboard(key(KeyCode::Escape), true);
    assert!(!app.shell_open() && app.pause_menu.is_some(), "back to the menu");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The footer names the table's keys, not literals: a rebinding renames it.
#[test]
fn the_footer_prints_the_bound_keys() {
    assert_eq!(
        shell_footer(&KeyBindings::default()),
        "UP/DOWN SELECT \u{b7} LEFT/RIGHT CHANGE \u{b7} ESC BACK"
    );
    let mut keys = KeyBindings::default();
    keys.bind(Action::MenuBack, KeyCode::KeyB);
    assert!(shell_footer(&keys).ends_with("B BACK"), "{}", shell_footer(&keys));
}
