//! Locks the frame-level HUD: bars, readouts, panels and the `BattleHudModel` contract. The
//! reticle overlay's own locks live in `hud/reticle_overlay_tests.rs` (shared helpers are here).

use super::HudElement;
use super::*;
use super::{HudSizeClass, HudState};
use crate::hud::number::{FPS_COLOR, RELOAD_TIME_COLOR};
use crate::hud::reticle::ReticleStatus;

/// A wire snapshot of one T-54 for the instrument locks: the fields a HUD row reads, nothing
/// authoritative behind it — the client never owns a sim, not even in a test.
pub(super) fn tank_snapshot(id: u64, team: u16, hit_points: u32) -> net::TankSnapshot {
    let vehicle = game_core::VehicleKind::T54_1951;
    let spec = vehicle.spec_ref();
    net::TankSnapshot {
        tank_id: game_core::TankId(id),
        team: game_core::TeamId(team),
        vehicle,
        position: [id as f32, 0.0, 0.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    }
}

pub(super) fn vitals() -> HudVitals {
    HudVitals {
        hit_points: 1000,
        max_hit_points: 1000,
        reload_remaining_s: 0.0,
        reload_seconds: 5.0,
    }
}

pub(super) fn reticle_at(
    status: ReticleStatus,
    hint: Option<super::reticle::PenetrationHint>,
) -> HudReticle {
    HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        gun_clip: None,
        aim_radius_clip: 0.08,
        target_distance_m: None,
        block_distance_m: None,
        arc_limit: None,
        status,
        penetration_hint: hint,
        reload_fraction: 1.0,
        hit_confirm: None,
        converged: false,
        mode: super::reticle::ReticleMode::ThirdPerson,
        // Through the shared matrix, like a live frame — so the locks below still fail if the
        // matrix ever starts speaking armor in third person.
        marker_color: super::reticle_overlay::marker_color(
            super::reticle::ReticleMode::ThirdPerson,
            hint,
            0.0,
        ),
    }
}

/// The same reticle seen through settled optics: mode AND the colour the matrix resolves there.
pub(super) fn sniper(reticle: HudReticle) -> HudReticle {
    let mode = super::reticle::ReticleMode::Sniper;
    HudReticle {
        converged: false,
        mode,
        marker_color: super::reticle_overlay::marker_color(mode, reticle.penetration_hint, 1.0),
        ..reticle
    }
}

pub(super) fn hint(penetrates: bool) -> super::reticle::PenetrationHint {
    super::reticle::PenetrationHint {
        penetrates,
        shell_pen_mm: 200.0,
        armor_mm: 150.0,
        facing: game_core::ArmorFacing::HullFront,
    }
}

#[test]
fn fps_readout_draws_digits_in_the_top_right_only_when_positive() {
    let without = build_hud(vitals(), 16.0 / 9.0);
    assert!(!without.iter().any(|vertex| vertex.color == FPS_COLOR), "0 fps draws nothing");

    // More digits means strictly more segment geometry (same digit isolates digit count
    // from which-segments-are-lit). "8" lights 7 segments; "888" lights 21.
    let one_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 8.0, 0.0, None);
    let three_digit = build_hud_with_reticle(vitals(), 16.0 / 9.0, None, 888.0, 0.0, None);
    let count = |hud: &[HudVertex]| hud.iter().filter(|v| v.color == FPS_COLOR).count();
    assert!(count(&one_digit) > 0, "fps digits should be drawn");
    assert!(count(&three_digit) > count(&one_digit), "888 needs more segments than 8");

    assert!(
        three_digit
            .iter()
            .filter(|v| v.color == FPS_COLOR)
            .all(|v| v.position[0] > 0.0 && v.position[1] > 0.0)
    );
}

/// H5: the speed is an instrument beside the damage panel — a plate, the number, the unit and
/// the cruise notches — and no speed digits float in the readouts any more.
#[test]
fn the_speed_instrument_sits_beside_the_damage_panel_and_reads_the_speed() {
    use crate::hud::elements::SpeedPart;
    let ui = ui_kit::ui::Ui::for_aspect(16.0 / 9.0);
    let moving = super::test_model(vitals(), None, 0.0, 42.0, None);
    let list = super::build_battle_hud_list(&moving, &ui);
    match &list.find(HudElement::Speed(SpeedPart::Number)).expect("number").payload {
        ui_kit::draw_list::Payload::Text { text, .. } => assert_eq!(text, "42"),
        other => panic!("{other:?}"),
    }
    let plate = list.find(HudElement::Speed(SpeedPart::Plate)).expect("plate").rect;
    assert!(plate.x < 960.0 && plate.y > 540.0, "bottom-left: {plate:?}");
    // The readouts no longer draw the speed: the vertices they emit are the same whether the
    // hull moves or not.
    let stopped = super::test_model(vitals(), None, 0.0, 0.0, None);
    let readouts_of = |model: &BattleHudModel| match &super::build_battle_hud_list(model, &ui)
        .find(HudElement::Readouts)
        .expect("readouts")
        .payload
    {
        ui_kit::draw_list::Payload::Legacy(v) => v.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        readouts_of(&moving),
        readouts_of(&stopped),
        "no speed digits float in the readouts"
    );
}

#[test]
fn the_hit_points_live_in_the_damage_panel() {
    use crate::hud::elements::DamagePart;
    let ui = ui_kit::ui::Ui::for_aspect(16.0 / 9.0);
    let mut model = super::test_model(vitals(), None, 0.0, 0.0, None);
    let tank = tank_snapshot(1, 1, 750);
    model.damage = Some(super::damage_panel::DamagePanelModel::from_snapshot(
        &tank,
        game_core::VehicleKind::T54_1951.spec_ref(),
        None,
        0.0,
        None,
    ));
    let list = super::build_battle_hud_list(&model, &ui);
    match &list.find(HudElement::DamagePanel(DamagePart::HpNumber)).expect("number").payload {
        ui_kit::draw_list::Payload::Text { text, .. } => assert_eq!(text, "750"),
        other => panic!("{other:?}"),
    }
    let bar = list.find(HudElement::DamagePanel(DamagePart::HpBar)).expect("bar");
    assert!(bar.rect.x < 400.0 && bar.rect.y > 700.0, "bottom-left: {:?}", bar.rect);
    // The readouts no longer draw the hit points: the vertices they emit are the same whether
    // the hull is whole or nearly dead.
    let readouts_of = |model: &BattleHudModel| match &super::build_battle_hud_list(model, &ui)
        .find(HudElement::Readouts)
        .expect("readouts")
        .payload
    {
        ui_kit::draw_list::Payload::Legacy(v) => v.clone(),
        other => panic!("{other:?}"),
    };
    let mut nearly_dead = model.clone();
    nearly_dead.vitals.hit_points = 40;
    assert_eq!(readouts_of(&model), readouts_of(&nearly_dead), "no HP digits float top-left");
}

#[test]
fn reload_seconds_count_down_at_the_reticle_and_nowhere_else() {
    // ONE loading display: the countdown lives beside the reticle's reload arc (where the eye
    // already is). The old bottom-center reload bar drew a second, competing indicator.
    let hud = build_hud(
        HudVitals {
            hit_points: 1000,
            max_hit_points: 1000,
            reload_remaining_s: 4.2,
            reload_seconds: 5.0,
        },
        16.0 / 9.0,
    );
    let reload_vertices: Vec<_> =
        hud.iter().filter(|vertex| vertex.color == RELOAD_TIME_COLOR).collect();

    assert!(!reload_vertices.is_empty(), "reload seconds should be drawn while reloading");
    assert!(
        reload_vertices.iter().all(|v| v.position[1] > -0.25 && v.position[1] < 0.05),
        "reload digits sit beside the reticle, not at a bottom bar"
    );
    // And when the gun is ready, the countdown vanishes entirely.
    let ready = build_hud(
        HudVitals {
            hit_points: 1000,
            max_hit_points: 1000,
            reload_remaining_s: 0.0,
            reload_seconds: 5.0,
        },
        16.0 / 9.0,
    );
    assert!(
        ready.iter().all(|vertex| vertex.color != RELOAD_TIME_COLOR),
        "no countdown when the gun is ready"
    );
}

#[test]
fn a_chamfered_panel_cuts_its_corners_and_stays_inside_its_rect() {
    let mut v = Vec::new();
    let (center, half, chamfer, aspect) =
        ([0.1_f32, -0.2_f32], [0.4_f32, 0.3_f32], 0.05_f32, 2.0_f32);
    push_panel(&mut v, center, half, chamfer, aspect, theme::color::PANEL);

    assert_eq!(v.len(), 18, "a chamfered panel is a 6-triangle fan");
    let inside = |p: [f32; 2]| {
        (p[0] - center[0]).abs() <= half[0] + 1e-6 && (p[1] - center[1]).abs() <= half[1] + 1e-6
    };
    assert!(v.iter().all(|vert| inside([vert.position[0], vert.position[1]])));

    // No vertex sits on an exact rect corner: the 45-degree cut removed all four.
    let corner = |p: [f32; 2]| {
        ((p[0] - center[0]).abs() - half[0]).abs() < 1e-6
            && ((p[1] - center[1]).abs() - half[1]).abs() < 1e-6
    };
    assert!(
        v.iter().all(|vert| !corner([vert.position[0], vert.position[1]])),
        "chamfer must cut every corner"
    );

    // The x cut is aspect-corrected: a vertex at the top edge starts chamfer/aspect in from the left.
    let top = center[1] + half[1];
    let leftmost_top = v
        .iter()
        .filter(|vert| (vert.position[1] - top).abs() < 1e-6)
        .map(|vert| vert.position[0])
        .fold(f32::INFINITY, f32::min);
    let expected = center[0] - half[0] + chamfer / aspect;
    assert!((leftmost_top - expected).abs() < 1e-5, "45-degree cut must be aspect-corrected");
}

#[test]
fn the_positional_wrapper_and_the_model_build_identical_huds() {
    let reticle = reticle_at(ReticleStatus::Clear, Some(hint(true)));
    let model = BattleHudModel {
        vitals: vitals(),
        reticle: Some(reticle),
        fps: 61.0,
        // The positional wrapper cannot carry p95 (it predates it): both sides draw none.
        frame_p95_ms: 0.0,
        speed_kmh: 33.0,
        cruise_level: 0,
        zoom_factor: Some(4.2),
        damage_log: Vec::new(),
        hit_log_collapsed: false,
        incoming_hits: Vec::new(),
        ammo: None,
        damage: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        top_bar: None,
        team_lists: None,
        markers: None,
        sixth_sense_lit: false,
        budget: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: 0.0,
        pause_menu: None,
        command_wheel: None,
        pings: None,
        team_word: None,
        kill_feed: None,
        net: None,
    };
    let from_model = build_battle_hud(&model, 16.0 / 9.0);
    let from_wrapper =
        build_hud_with_reticle(vitals(), 16.0 / 9.0, Some(reticle), 61.0, 33.0, Some(4.2));
    assert_eq!(from_model, from_wrapper, "wrapper must stay a pure forwarding shim");
    assert!(!from_model.is_empty());
}

/// The scope surround follows `scope_fade` (the camera's mode-blend clock), NOT the reticle
/// mode: mid-transition the reticle has already flipped while the housing is still irising, and
/// on exit the housing lingers a beat after the reticle returned to third person.
#[test]
fn the_scope_surround_is_fade_driven_not_mode_driven() {
    let base = BattleHudModel {
        vitals: vitals(),
        reticle: Some(reticle_at(ReticleStatus::Clear, None)),
        fps: 0.0,
        frame_p95_ms: 0.0,
        speed_kmh: 0.0,
        cruise_level: 0,
        zoom_factor: None,
        damage_log: Vec::new(),
        hit_log_collapsed: false,
        incoming_hits: Vec::new(),
        ammo: None,
        damage: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        top_bar: None,
        team_lists: None,
        markers: None,
        sixth_sense_lit: false,
        budget: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: 0.0,
        pause_menu: None,
        command_wheel: None,
        pings: None,
        team_word: None,
        kill_feed: None,
        net: None,
    };
    let housing = |hud: &[HudVertex]| {
        let [r, g, b, _] = super::scope_overlay::VIGNETTE_COLOR;
        hud.iter().any(|v| v.color[0] == r && v.color[1] == g && v.color[2] == b)
    };

    // A third-person reticle mid-entry: the housing already shows at partial fade.
    let entering = BattleHudModel { scope_fade: 0.5, ..base.clone() };
    assert!(housing(&build_battle_hud(&entering, 16.0 / 9.0)), "fade in draws the surround");

    // A sniper reticle with the fade at zero (first logical frame of an exit): nothing draws.
    let exited = BattleHudModel {
        reticle: Some(sniper(reticle_at(ReticleStatus::Clear, None))),
        scope_fade: 0.0,
        ..base.clone()
    };
    assert!(!housing(&build_battle_hud(&exited, 16.0 / 9.0)), "zero fade draws no surround");
}

#[test]
fn battle_outcome_banner_draws_only_when_the_battle_has_ended() {
    let running = BattleHudModel {
        vitals: vitals(),
        reticle: None,
        fps: 0.0,
        frame_p95_ms: 0.0,
        speed_kmh: 0.0,
        cruise_level: 0,
        zoom_factor: None,
        damage_log: Vec::new(),
        hit_log_collapsed: false,
        incoming_hits: Vec::new(),
        ammo: None,
        damage: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        top_bar: None,
        team_lists: None,
        markers: None,
        sixth_sense_lit: false,
        budget: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        scope_fade: 0.0,
        pause_menu: None,
        command_wheel: None,
        pings: None,
        team_word: None,
        kill_feed: None,
        net: None,
    };
    let victory =
        BattleHudModel { battle_outcome: Some(BattleHudOutcome::Victory), ..running.clone() };

    let running_hud = build_battle_hud(&running, 16.0 / 9.0);
    let victory_hud = build_battle_hud(&victory, 16.0 / 9.0);

    assert!(!running_hud.iter().any(|vertex| vertex.color == OUTCOME_VICTORY_COLOR));
    assert!(victory_hud.iter().any(|vertex| vertex.color == OUTCOME_VICTORY_COLOR));

    // A draw (mutual wipe or clock expiry) banners in its own neutral color.
    let draw = BattleHudModel { battle_outcome: Some(BattleHudOutcome::Draw), ..running.clone() };
    let draw_hud = build_battle_hud(&draw, 16.0 / 9.0);
    assert!(draw_hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_DRAW_COLOR));
    assert!(!running_hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_DRAW_COLOR));

    // A broken authority is not called a defeat, but is just as terminal and actionable.
    let disconnected = BattleHudModel {
        battle_outcome: Some(BattleHudOutcome::ConnectionLost),
        ..running.clone()
    };
    let disconnected_hud = build_battle_hud(&disconnected, 16.0 / 9.0);
    assert!(
        disconnected_hud
            .iter()
            .any(|vertex| vertex.color == super::outcome::OUTCOME_CONNECTION_COLOR)
    );

    // Input is dead once the battle ends or disconnects; every banner carries the way-out hint.
    let hint = |hud: &[HudVertex]| {
        hud.iter().any(|vertex| vertex.color == super::outcome::OUTCOME_HINT_COLOR)
    };
    assert!(
        hint(&victory_hud) && hint(&draw_hud) && hint(&disconnected_hud),
        "every terminal battle points back to the garage"
    );
    assert!(!hint(&running_hud), "no garage hint while the battle runs");
}

/// The battle clock draws top-center when the server reports a timed battle and disappears for
/// untimed ones; the last minute switches to the alert color so the squeeze reads at a glance.
/// The battle clock is the top bar's (H1): timed battles show it, untimed ones do not, and it
/// is a named element the top bar's own lock reads — nothing floats in the readouts any more.
#[test]
fn battle_clock_is_the_top_bars_and_draws_only_when_timed() {
    let ui = ui_kit::ui::Ui::for_aspect(16.0 / 9.0);
    let mut untimed = super::test_model(vitals(), None, 0.0, 0.0, None);
    untimed.top_bar = Some(super::top_bar::TopBarModel {
        frags: [0, 0],
        team_hit_points: [7_000, 7_000],
        team_hit_points_max: [7_000, 7_000],
    });
    let timed = BattleHudModel { battle_clock_remaining_s: Some(474.0), ..untimed.clone() };
    assert!(super::build_battle_hud_list(&untimed, &ui).find(HudElement::TopBarClock).is_none());
    assert!(super::build_battle_hud_list(&timed, &ui).find(HudElement::TopBarClock).is_some());
    let bare = BattleHudModel { top_bar: None, ..timed.clone() };
    assert!(
        super::build_battle_hud_list(&bare, &ui).find(HudElement::TopBar).is_none(),
        "no roster, no bar — and no clock floating on its own"
    );
}

/// The damage panel draws only when the model carries one, and a knocked-out module paints its
/// signal red into the frame — the fix for a silent fire-refusal, kept through the redesign.
#[test]
fn the_damage_panel_draws_only_when_present_and_a_dead_module_reads_red() {
    use crate::hud::elements::DamagePart;
    let ui = ui_kit::ui::Ui::for_aspect(16.0 / 9.0);
    let theme = ui_kit::theme::Theme::standard();
    let base = super::test_model(vitals(), None, 0.0, 0.0, None);
    assert!(
        super::build_battle_hud_list(&base, &ui)
            .find(HudElement::DamagePanel(DamagePart::Plate))
            .is_none(),
        "no panel while the model carries none"
    );
    let mut tank = tank_snapshot(1, 1, 750);
    tank.module_hit_points[game_core::ModuleSlot::Gun.wire_index()] = 0;
    let with_dead_gun = BattleHudModel {
        damage: Some(super::damage_panel::DamagePanelModel::from_snapshot(
            &tank,
            game_core::VehicleKind::T54_1951.spec_ref(),
            None,
            0.0,
            None,
        )),
        ..base
    };
    let list = super::build_battle_hud_list(&with_dead_gun, &ui);
    let gun = HudElement::DamagePanel(DamagePart::Module(game_core::ModuleSlot::Gun));
    match &list.find(gun).expect("gun").payload {
        ui_kit::draw_list::Payload::Icon { color, .. } => {
            assert_eq!(*color, theme.semantic.module[2], "a dead gun must read red")
        }
        other => panic!("{other:?}"),
    }
}

/// F5: the legacy instruments ride the draw list element by element — the one emitter yields
/// their bytes verbatim, in order, and every instrument the model asks for has a name. (Since
/// H1 the list also carries elements the old builder never drew, so the comparison is over the
/// legacy elements alone.)
#[test]
fn the_draw_list_emits_the_legacy_hud_byte_for_byte() {
    use ui_kit::draw_list::{DrawList, Element, Payload};
    let ui = ui_kit::ui::Ui::for_aspect(16.0 / 9.0);
    let theme = ui_kit::theme::Theme::standard();
    let model = super::test_model(vitals(), None, 0.0, 0.0, None);
    let list = super::build_battle_hud_list(&model, &ui);
    let mut legacy_only = DrawList::new();
    let mut expected: Vec<HudVertex> = Vec::new();
    for element in list.iter() {
        if let Payload::Legacy(v) = &element.payload {
            expected.extend_from_slice(v);
            legacy_only.push(
                Element::new(element.id, element.rect, Payload::Legacy(v.clone())).z(element.z),
            );
        }
    }
    assert!(!expected.is_empty());
    assert_eq!(legacy_only.emit(&ui, &theme), expected, "legacy payloads ride verbatim, in order");
    for id in [HudElement::Reticle, HudElement::Readouts, HudElement::HitDirection] {
        assert!(list.find(id).is_some(), "{id:?} is a named element");
    }
    assert!(list.find(HudElement::PauseMenu).is_none(), "no menu, no element");
}

/// H25: the reticle stack is carried verbatim — the element's payload IS `push_reticle`'s output.
#[test]
fn the_reticle_stack_is_emitted_verbatim() {
    let aspect = 16.0 / 9.0;
    let model = super::test_model(vitals(), None, 0.0, 0.0, None);
    let list = super::build_battle_hud_list(&model, &ui_kit::ui::Ui::for_aspect(aspect));
    let mut expected = Vec::new();
    super::reticle_overlay::push_reticle(&mut expected, &super::default_reticle(), aspect);
    match &list.find(HudElement::Reticle).expect("the reticle element").payload {
        ui_kit::draw_list::Payload::Legacy(v) => assert_eq!(v, &expected),
        other => panic!("the reticle must stay a legacy payload, found {other:?}"),
    }
}

/// F9: the busiest state fits the renderer's 16 384-vertex buffer with headroom. The number
/// is what `hud_states` prints per element; since H0 baked the relief the bulk is text.
#[test]
fn the_full_hud_state_fits_the_buffer_with_headroom() {
    const HEADROOM_CEILING: usize = 14_000;
    let mut busiest = (HudState::ThirdPersonIdle, 0usize);
    for state in HudState::ALL {
        for size in HudSizeClass::ALL {
            let n = super::hud_state_vertices(state, size, 1920, 1080).len();
            if n > busiest.1 {
                busiest = (state, n);
            }
        }
    }
    println!("HUD CENSUS: busiest state {:?} at {} vertices", busiest.0, busiest.1);
    assert!(
        busiest.1 <= HEADROOM_CEILING,
        "{:?} emits {} vertices — over the {HEADROOM_CEILING} headroom ceiling of the 16 384 buffer",
        busiest.0,
        busiest.1
    );
}
