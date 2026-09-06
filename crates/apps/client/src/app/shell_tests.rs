//! Locks the shell's settings page (interface program P6): the way in from the escape menu,
//! the keys, the mouse, the file, the way out.

use winit::keyboard::{KeyCode, PhysicalKey};

use super::ClientApp;
use super::input_tests::in_battle;
use super::keybinds::{Action, Context, KeyBindings};
use super::shell::shell_footer;
use crate::hud::elements::{HudElement, ShellPart};
use crate::hud::shell::{MenuItem, MenuKind, SettingsRow, ShellModel};

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
    app.click_menu_item(MenuItem::Settings);
    assert!(app.shell_open() && !app.menu_open(), "the page is up, the menu is down");
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
    assert!(app.menu_open(), "back to the menu");
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

/// P8: KEY BINDINGS on the escape menu opens the page on the battle's keys; Enter listens
/// and the next key binds and is written; a shared key is named on both rows; R resets and
/// writes; Esc backs out of listening, the context row steps the contexts, and Esc leaves the
/// page for the menu.
#[test]
fn the_keybinds_page_rebinds_a_key_names_the_conflict_and_resets_it() {
    let dir = std::env::temp_dir().join(format!("wot-keybinds-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("keybinds.json");
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.enable_keybinds_persistence(path.clone());
    app.open_pause_menu();
    app.click_menu_item(MenuItem::Keybinds);
    let Some(ShellModel::Keybinds(page)) = app.shell_model() else { panic!("the page") };
    assert_eq!(page.context, Context::Battle);
    assert_eq!(page.selected, 0, "the context row first");
    // Down to FIRE, Enter listens, W binds — through the window's router, which the
    // listening row takes first — and the file follows.
    let fire = page.rows.iter().position(|row| row.action == Action::Fire).expect("FIRE") + 1;
    for _ in 0..fire {
        app.on_battle_keyboard(key(KeyCode::ArrowDown), true);
    }
    app.on_battle_keyboard(key(KeyCode::Enter), true);
    assert_eq!(app.shell_listening(), Some(Action::Fire));
    app.on_key(key(KeyCode::KeyW), true, false);
    assert_eq!(app.shell_listening(), None, "the key was taken");
    assert_eq!(app.keybinds().keys(Action::Fire), &[KeyCode::KeyW]);
    let written = super::keybinds::load_keybinds(&path).expect("written");
    assert_eq!(written.keys(Action::Fire), &[KeyCode::KeyW]);
    let Some(ShellModel::Keybinds(page)) = app.shell_model() else { panic!("the page") };
    let row = |action: Action| page.rows.iter().find(|row| row.action == action).expect("row");
    assert_eq!(row(Action::Fire).conflict.as_deref(), Some("FORWARD"));
    assert_eq!(row(Action::Forward).conflict.as_deref(), Some("FIRE"));
    assert!(row(Action::Fire).changed && !row(Action::Back).changed);
    // R resets FIRE and writes.
    app.on_battle_keyboard(key(KeyCode::KeyR), true);
    assert_eq!(app.keybinds().keys(Action::Fire), Action::Fire.default_keys());
    let written = super::keybinds::load_keybinds(&path).expect("written");
    assert_eq!(written.keys(Action::Fire), Action::Fire.default_keys());
    // Esc while listening cancels the listening, not the page; a key outside the alphabet
    // is refused.
    app.on_battle_keyboard(key(KeyCode::Enter), true);
    app.on_key(key(KeyCode::Power), true, false);
    assert_eq!(app.shell_listening(), Some(Action::Fire), "not in the alphabet");
    app.on_key(key(KeyCode::Escape), true, false);
    assert_eq!(app.shell_listening(), None);
    assert!(app.shell_open());
    // Up to the context row; Right steps to the garage's keys, from their top.
    for _ in 0..fire {
        app.on_battle_keyboard(key(KeyCode::ArrowUp), true);
    }
    app.on_battle_keyboard(key(KeyCode::ArrowRight), true);
    let Some(ShellModel::Keybinds(page)) = app.shell_model() else { panic!("the page") };
    assert_eq!((page.context, page.selected, page.first_visible), (Context::Garage, 0, 0));
    // Esc: back to the menu.
    app.on_battle_keyboard(key(KeyCode::Escape), true);
    assert!(app.menu_open(), "back to the menu");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A start of the escape lock's walk: the app in one screen.
type Start = Box<dyn Fn() -> ClientApp>;

/// P8: from every screen, ESC reaches a menu with a way out — QUIT or EXIT TO GARAGE — in
/// at most three presses: the cold garage, its option list, a live battle, the garage over
/// it, the HUD editor, the settings page, the key bindings page.
#[test]
fn escape_always_offers_a_way_out() {
    let starts: Vec<(&str, Start)> = vec![
        ("cold garage", Box::new(ClientApp::new)),
        (
            "option list",
            Box::new(|| {
                let mut app = ClientApp::new();
                app.garage.open_option_list(super::garage::FitSlot::Gun);
                app
            }),
        ),
        ("battle", Box::new(in_battle)),
        (
            "garage over the battle",
            Box::new(|| {
                let mut app = in_battle();
                app.open_garage();
                app
            }),
        ),
        (
            "hud editor",
            Box::new(|| {
                let mut app = in_battle();
                app.open_hud_editor();
                app
            }),
        ),
        (
            "settings page",
            Box::new(|| {
                let mut app = in_battle();
                app.open_settings_page();
                app
            }),
        ),
        (
            "key bindings page",
            Box::new(|| {
                let mut app = in_battle();
                app.open_keybinds_page();
                app
            }),
        ),
    ];
    for (name, start) in starts {
        let mut app = start();
        let mut presses = 0;
        while !app.way_out_offered() && presses < 3 {
            app.on_key(key(KeyCode::Escape), true, false);
            app.on_key(key(KeyCode::Escape), false, false);
            presses += 1;
        }
        assert!(app.way_out_offered(), "{name}: no way out after {presses} presses");
    }
}

/// P8: the cold garage's menu — SETTINGS and KEY BINDINGS open their pages over the garage
/// and ESC brings the garage's menu back, not the battle's; QUIT asks the loop to leave; a
/// battle menu's STAY returns the gun to the mouse.
#[test]
fn the_garage_menu_opens_its_pages_and_quit_asks_the_loop_to_leave() {
    let mut app = ClientApp::new();
    app.on_key(key(KeyCode::Escape), true, false);
    let Some(ShellModel::Menu(menu)) = app.shell_model() else { panic!("the garage's menu") };
    assert_eq!(menu.kind, MenuKind::Garage);
    assert_eq!(menu.selected, 0, "SETTINGS first, never the commit");
    app.on_battle_keyboard(key(KeyCode::Enter), true);
    assert!(matches!(app.shell_model(), Some(ShellModel::Settings(_))), "SETTINGS opens");
    app.on_key(key(KeyCode::Escape), true, false);
    let Some(ShellModel::Menu(menu)) = app.shell_model() else { panic!("back to the menu") };
    assert_eq!(menu.kind, MenuKind::Garage, "the garage's, not the battle's");
    assert!(!app.quit_requested());
    app.click_menu_item(MenuItem::Quit);
    assert!(app.quit_requested() && !app.shell_open(), "QUIT: the loop leaves");
    // The battle's menu: STAY hands the gun back.
    let mut app = in_battle();
    app.open_pause_menu();
    assert!(!app.cursor_captured);
    app.click_menu_item(MenuItem::Stay);
    assert!(!app.shell_open() && app.cursor_captured, "STAY: the mouse is the gun again");
}
