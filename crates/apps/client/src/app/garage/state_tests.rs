//! Locks the `GarageState` core: per-vehicle persistent loadouts (the behaviour change from
//! reset-on-switch), save-file round-trips, vehicle selection, framing, rejection clearing,
//! keyboard focus, and the tech-tree view. Split from `garage/mod.rs` for the file budget.

use super::layout::{CAR_HALF, CAR_Y, carousel_arrows};
use super::*;

fn temp_save_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("wot-garage-{}-{name}", std::process::id()))
        .join("garage.json")
}

#[test]
fn switching_vehicles_restores_each_vehicles_saved_loadout() {
    // Behaviour change from the old reset-on-switch: each vehicle keeps its own edited draft.
    let mut garage = GarageState::default(); // starts on T-54, which has a real gun choice
    assert!(garage.draft().has_choice(FitSlot::Gun), "T-54 needs a gun choice for this test");
    let stock = garage.draft().assembled_spec().gun.shell;
    garage.cycle_module(FitSlot::Gun, 1);
    let edited = garage.draft().assembled_spec().gun.shell;
    assert_ne!(edited, stock, "the edit must move the gun");

    // Switch away and back: the T-54's edit survives instead of resetting to stock.
    garage.select_index(1);
    assert_eq!(
        garage.draft().assembled_spec().gun.shell,
        VehicleKind::PLAYABLE[1].spec().gun.shell,
        "the second vehicle is stock (never edited)"
    );
    garage.select_index(0);
    assert_eq!(garage.draft().assembled_spec().gun.shell, edited, "the T-54 edit persists");
}

#[test]
fn an_edited_loadout_survives_a_restart_through_the_save_file() {
    let path = temp_save_path("restart");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());

    let mut garage = GarageState::default();
    garage.enable_persistence(path.clone());
    garage.cycle_module(FitSlot::Gun, 1);
    let edited = garage.draft().assembled_spec().gun.shell;

    // A "restart": a brand-new garage loading the same file must come up on the edited draft.
    let mut restarted = GarageState::default();
    restarted.enable_persistence(path.clone());
    assert_eq!(restarted.draft().assembled_spec().gun.shell, edited);

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn enabling_persistence_with_no_save_leaves_the_stock_garage() {
    let path = temp_save_path("absent");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    let mut garage = GarageState::default();
    garage.enable_persistence(path);
    assert_eq!(garage.selected_vehicle(), VehicleKind::PLAYABLE[0]);
    assert_eq!(
        garage.draft().assembled_spec().gun.shell,
        VehicleKind::PLAYABLE[0].spec().gun.shell
    );
}

#[test]
fn garage_roster_starts_on_t54_and_playable_is_the_full_set() {
    let garage = GarageState::default();
    assert_eq!(garage.selected_vehicle(), VehicleKind::T54_1951);
    assert_eq!(VehicleKind::PLAYABLE, VehicleKind::ALL);
}

#[test]
fn selecting_a_vehicle_restores_the_safe_inspection_framing() {
    let mut garage = GarageState {
        orbit_yaw: -1.0,
        orbit_pitch: 1.0,
        orbit_distance: 6.0,
        ..Default::default()
    };

    garage.select_index(1);

    assert_eq!(garage.orbit_yaw, HERO_ORBIT_YAW);
    assert_eq!(garage.orbit_pitch, HERO_ORBIT_PITCH);
    assert_eq!(garage.orbit_distance, HERO_ORBIT_DISTANCE);
}

#[test]
fn confirm_returns_the_edited_spec_and_closes_the_garage() {
    let mut garage = GarageState::default();
    garage.select_vehicle(VehicleKind::TigerII);
    // W1: the edit that survives to battle is a MODULE choice — the crew dial is gone.
    garage.cycle_module(crate::app::garage::draft::FitSlot::Gun, 1);
    let edited = garage.draft().assembled_spec().gun.reload_seconds;
    let spec = garage.confirm();
    assert!(!garage.is_open() && garage.has_started());
    assert_eq!(spec.kind, VehicleKind::TigerII);
    assert!((spec.gun.reload_seconds - edited).abs() < 1.0e-6, "confirmed spec carries the edit");
}

#[test]
fn rejected_slot_starts_none_and_clears_on_successful_cycle() {
    let garage = GarageState::default();
    assert_eq!(garage.rejected_slot(), None);

    let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
    assert_eq!(garage.rejected_slot(), Some(FitSlot::Gun));

    garage.cycle_module(FitSlot::Engine, 1);
    assert_eq!(garage.rejected_slot(), None, "successful cycle clears rejection");
}

#[test]
fn rejected_slot_clears_on_vehicle_select() {
    let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
    garage.select_index(1);
    assert_eq!(garage.rejected_slot(), None, "selecting a vehicle clears rejection");
}

#[test]
fn rejected_slot_clears_on_ammo_select() {
    let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
    garage.set_ammo(1);
    assert_eq!(garage.rejected_slot(), None, "selecting ammo clears rejection");
}

#[test]
fn default_focus_is_gun() {
    let garage = GarageState::default();
    assert_eq!(garage.focused_slot(), FitSlot::Gun);
}

#[test]
fn focus_adjacent_wraps_around_all_slots() {
    let mut garage = GarageState::default();
    // Gun (index 1) -> Hull -> ... -> Turret (index 0) -> Gun (wrap forward).
    for _ in 0..FitSlot::ALL.len() {
        garage.focus_adjacent(1);
    }
    assert_eq!(garage.focused_slot(), FitSlot::Gun, "full loop returns to gun");

    // Backward from Gun (index 1) -> Turret (index 0); backward from Turret wraps to Radio.
    garage.focus_adjacent(-1);
    assert_eq!(garage.focused_slot(), FitSlot::Turret);
    garage.focus_adjacent(-1);
    assert_eq!(garage.focused_slot(), FitSlot::Radio, "backward wraps to last slot");
}

#[test]
fn cycle_focused_cycles_the_focused_slot_option() {
    let mut garage = GarageState::default();
    garage.select_vehicle(VehicleKind::T54_1951);
    let before = garage.draft().gun_name();
    // Focus starts on Gun; cycle forward. (Identity, not barrel length — the T-54's two D-10
    // variants share one physical tube, so the silhouette is deliberately unchanged.)
    garage.cycle_focused(1);
    assert_ne!(garage.draft().gun_name(), before, "focused cycle changed the gun");
}

#[test]
fn selecting_a_vehicle_resets_focus_to_gun() {
    let mut garage = GarageState::default();
    garage.focus_adjacent(1); // move focus off Gun
    assert_ne!(garage.focused_slot(), FitSlot::Gun);
    garage.select_index(1);
    assert_eq!(garage.focused_slot(), FitSlot::Gun, "vehicle select resets focus to gun");
}

#[test]
fn default_view_is_hangar() {
    let garage = GarageState::default();
    assert_eq!(garage.view(), GarageView::Hangar);
}

#[test]
fn open_tech_tree_switches_view() {
    let mut garage = GarageState::default();
    garage.open_tech_tree();
    assert_eq!(garage.view(), GarageView::TechTree);
}

#[test]
fn close_tech_tree_restores_hangar() {
    let mut garage = GarageState::default();
    garage.open_tech_tree();
    garage.close_tech_tree();
    assert_eq!(garage.view(), GarageView::Hangar);
}

#[test]
fn open_tech_tree_cancels_drag() {
    let mut garage = GarageState::default();
    garage.begin_drag();
    garage.open_tech_tree();
    assert!(!garage.is_dragging(), "opening tech tree cancels any in-progress drag");
}

#[test]
fn the_current_roster_fits_so_the_carousel_never_scrolls_or_shows_arrows() {
    let mut garage = GarageState::default();
    // Six vehicles fit inside the window, so scrolling is a no-op and the arrows are inert.
    garage.scroll_carousel(1);
    assert_eq!(garage.carousel_scroll(), 0, "a roster that fits never scrolls");

    let (left, right) = carousel_arrows();
    garage.set_cursor(left);
    assert_ne!(garage.hit_test(false), GarageHit::CarouselScroll(-1), "no left arrow to hit");
    garage.set_cursor(right);
    assert_ne!(garage.hit_test(false), GarageHit::CarouselScroll(1), "no right arrow to hit");
}

#[test]
fn the_cursor_over_carousel_predicate_gates_wheel_routing() {
    let mut garage = GarageState::default();
    garage.set_cursor([0.0, CAR_Y]);
    assert!(garage.cursor_over_carousel(), "the wheel scrolls over the carousel row");
    garage.set_cursor([0.0, CAR_Y + CAR_HALF[1] + 0.2]);
    assert!(!garage.cursor_over_carousel(), "above the strip the wheel zooms the camera");
}

#[test]
fn map_cycle_walks_auto_then_every_shipped_map_and_wraps() {
    let mut garage = GarageState::default();
    assert_eq!(garage.selected_map(), None, "the garage starts on AUTO");
    for &map in terrain::MapId::SHIPPED {
        garage.cycle_map(1);
        assert_eq!(garage.selected_map(), Some(map), "the ring visits every shipped map");
    }
    garage.cycle_map(1);
    assert_eq!(garage.selected_map(), None, "past the last map the ring wraps back to AUTO");
    // Backward from AUTO lands on the last shipped map — and `Scratch` (absent from
    // `MapId::SHIPPED`) is never a stop on the ring.
    garage.cycle_map(-1);
    assert_eq!(garage.selected_map(), terrain::MapId::SHIPPED.last().copied());
}
