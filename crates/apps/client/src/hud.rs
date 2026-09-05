use renderer_api::HudVertex;
use ui_kit::draw_list::{DrawList, Element, Payload};
use ui_kit::rect::Rect;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod ammo_panel;
pub(crate) mod damage_log;
pub(crate) mod demo;
pub(crate) mod demo_strip;
pub(crate) mod elements;
pub use ui_kit::font;
pub(crate) mod health;
pub(crate) mod health_bar;
pub(crate) mod hit_direction;
pub(crate) use ui_kit::icons;
pub(crate) mod kill_marker;
pub(crate) mod minimap;
pub(crate) mod module_panel;
pub(crate) mod number;
pub(crate) mod outcome;
pub(crate) mod pause_menu;
pub(crate) mod rack_callout;
pub use ui_kit::primitives;
pub(crate) mod readouts;
pub(crate) mod reticle;
pub(crate) mod reticle_marks;
pub(crate) mod reticle_overlay;
pub(crate) mod reticle_readouts;
pub(crate) mod reticle_sweep;
pub(crate) mod review;
pub(crate) mod scope_overlay;
pub(crate) mod spot_bracket;
pub(crate) mod states;
pub(crate) mod team_list;
pub(crate) mod top_bar;
pub use ui_kit::theme;
pub(crate) mod crew_panel;
pub(crate) mod track_callout;

pub(crate) use elements::HudElement;
pub(crate) use health::health_color;
pub(crate) use outcome::BattleHudOutcome;
#[cfg(test)]
pub(crate) use outcome::OUTCOME_VICTORY_COLOR;
pub(crate) use primitives::{push_hairline, push_panel, push_quad};
pub(crate) use reticle_overlay::HudReticle;
pub use review::{HudReviewView, hud_review_views};
pub use states::{HudSizeClass, HudState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudVitals {
    pub hit_points: u32,
    pub max_hit_points: u32,
    pub reload_remaining_s: f32,
    pub reload_seconds: f32,
}

/// Everything the battle HUD draws in one frame, gathered by `render_now` and consumed by
/// `build_battle_hud`. One struct instead of a growing positional parameter list — upcoming
/// elements (damage log, ammo panel, minimap) land here as fields.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleHudModel {
    pub vitals: HudVitals,
    pub reticle: Option<HudReticle>,
    /// Draws the top-right frame-rate readout when positive; `0.0` omits it (offline examples).
    pub fps: f32,
    /// 95th-percentile frame interval (ms) over the last ~1.5 s: the drops, as a number.
    pub frame_p95_ms: f32,
    /// Draws the bottom-left speed readout when at least 0.5 km/h.
    pub speed_kmh: f32,
    /// Sniper magnification; `None` in third person (no readout).
    pub zoom_factor: Option<f32>,
    /// Recent dealt/taken damage rows, newest first (`hud/damage_log.rs`).
    pub damage_log: Vec<damage_log::DamageLogEntry>,
    /// Track-damage callout + re-seat bars for the player's own hull (`hud/track_callout.rs`).
    pub track_feedback: track_callout::TrackFeedbackModel,
    /// Seconds left on the player's OWN lit ammunition rack (`hud/rack_callout.rs`); `None`
    /// when the rack is quiet. Protocol v43 — the ten seconds the crew can win, made visible.
    pub rack_fire_remaining_s: Option<f32>,
    /// Incoming hits resolved to screen bearings (`hud/hit_direction.rs`).
    pub incoming_hits: Vec<hit_direction::IncomingHit>,
    pub ammo: Option<ammo_panel::AmmoHudModel>,
    /// The player's own module-condition row under the health bar (`hud/module_panel.rs`); `None`
    /// before the first snapshot, then always present so a knocked-out gun is never a mystery.
    pub modules: Option<module_panel::ModulePanelModel>,
    /// The crew row under the module panel (`hud/crew_panel.rs`, v46): who is down, the first-aid
    /// countdown, who came back scarred. `None` before the first snapshot.
    pub crew: Option<crew_panel::CrewPanelModel>,
    pub minimap: Option<minimap::MinimapModel>,
    pub battle_outcome: Option<BattleHudOutcome>,
    /// Seconds left on the battle clock, drawn top-center as M:SS; `None` hides it (untimed).
    pub battle_clock_remaining_s: Option<f32>,
    /// The top bar (H1): frags and pools; `None` before the roster lands.
    pub top_bar: Option<top_bar::TopBarModel>,
    /// The team lists (H2); `None` before the roster lands.
    pub team_lists: Option<team_list::TeamListsModel>,
    /// Seconds since the player's most recent kill; `None` once the confirmation has played out.
    pub kill_confirm_age_s: Option<f32>,
    /// Seconds since the reload finished, driving the gun-ready flash at the reticle.
    pub reload_ready_age_s: Option<f32>,
    /// Seconds since a fire click was REFUSED (still reloading / empty slot / dead gun),
    /// driving the red denial pulse at the reticle — a swallowed shot is seen, never wondered
    /// about. `None` when no refusal is fresh.
    pub fire_denied_age_s: Option<f32>,
    /// How much of the scope surround shows (0 = none, 1 = settled sniper). Rides the presented
    /// camera's mode-blend clock, so the optics iris in/out WITH the view instead of hard-cutting
    /// (see `camera::present::scope_dressing`).
    pub scope_fade: f32,
    /// The ESC modal when it is up; `None` is a closed menu (`hud/pause_menu.rs`).
    pub pause_menu: Option<pause_menu::PauseMenuModel>,
}

/// Build the 2D HUD overlay (center crosshair, top-left health bar, bottom-center reload
/// bar) in clip space. `aspect` keeps the crosshair square on non-square viewports.
pub fn build_hud(vitals: HudVitals, aspect: f32) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals,
            reticle: None,
            fps: 0.0,
            frame_p95_ms: 0.0,
            speed_kmh: 0.0,
            zoom_factor: None,
            damage_log: Vec::new(),
            track_feedback: Default::default(),
            rack_fire_remaining_s: None,
            incoming_hits: Vec::new(),
            ammo: None,
            modules: None,
            crew: None,
            minimap: None,
            battle_outcome: None,
            battle_clock_remaining_s: None,
            top_bar: None,
            team_lists: None,
            kill_confirm_age_s: None,
            reload_ready_age_s: None,
            fire_denied_age_s: None,
            scope_fade: 0.0,
            pause_menu: None,
        },
        aspect,
    )
}

/// The model the HUD unit tests draw: everything off but the vitals and the reticle handed in.
#[cfg(test)]
pub(crate) fn test_model(
    vitals: HudVitals,
    reticle: Option<HudReticle>,
    fps: f32,
    speed_kmh: f32,
    zoom_factor: Option<f32>,
) -> BattleHudModel {
    BattleHudModel {
        vitals,
        reticle,
        fps,
        frame_p95_ms: 0.0,
        speed_kmh,
        zoom_factor,
        damage_log: Vec::new(),
        track_feedback: Default::default(),
        rack_fire_remaining_s: None,
        incoming_hits: Vec::new(),
        ammo: None,
        modules: None,
        crew: None,
        minimap: None,
        battle_outcome: None,
        battle_clock_remaining_s: None,
        top_bar: None,
        team_lists: None,
        kill_confirm_age_s: None,
        reload_ready_age_s: None,
        fire_denied_age_s: None,
        // The positional test path has no camera; sniper mode implies a settled scope.
        scope_fade: if reticle.is_some_and(|r| r.mode == reticle::ReticleMode::Sniper) {
            1.0
        } else {
            0.0
        },
        pause_menu: None,
    }
}

#[cfg(test)]
pub(crate) fn build_hud_with_reticle(
    vitals: HudVitals,
    aspect: f32,
    reticle: Option<HudReticle>,
    fps: f32,
    speed_kmh: f32,
    zoom_factor: Option<f32>,
) -> Vec<HudVertex> {
    build_battle_hud(&test_model(vitals, reticle, fps, speed_kmh, zoom_factor), aspect)
}

/// The reticle a frame draws when the model carries none: a neutral third-person marker at
/// the centre, no verdict, no target — what the old builder hard-coded inline.
pub(crate) fn default_reticle() -> HudReticle {
    HudReticle {
        aim_clip: [0.0, 0.0],
        impact_clip: None,
        gun_clip: None,
        aim_radius_clip: 0.0,
        target_distance_m: None,
        block_distance_m: None,
        arc_limit: None,
        status: ReticleStatus::Clear,
        penetration_hint: None,
        reload_fraction: 1.0,
        hit_confirm: None,
        converged: false,
        mode: crate::hud::reticle::ReticleMode::ThirdPerson,
        marker_color: reticle_overlay::RETICLE_NEUTRAL,
    }
}

/// The battle HUD as a draw list (interface program F5): one element per instrument, in the
/// order the old builder painted them, each carrying its legacy vertices verbatim. The H wave
/// replaces payloads element by element; the reticle stack stays legacy (H25).
pub(crate) fn build_battle_hud_list(
    model: &BattleHudModel,
    ui: &ui_kit::ui::Ui,
) -> DrawList<HudElement> {
    let aspect = ui.aspect();
    let theme = ui_kit::theme::Theme::standard();
    let mut list = DrawList::new();
    let mut order: i16 = 0;
    fn legacy(
        list: &mut DrawList<HudElement>,
        order: &mut i16,
        id: HudElement,
        vertices: Vec<HudVertex>,
    ) {
        list.push(Element::new(id, Rect::default(), Payload::Legacy(vertices)).z(*order));
        *order += 1;
    }
    let reticle = model.reticle.unwrap_or_else(default_reticle);

    // The scope surround paints first so every live marker (reticle, readouts) stays on top.
    // Fade-driven, not mode-driven: during the TPP <-> sniper camera blend the housing is
    // already irising in (or lifting away) while the logical mode has long since flipped.
    if model.scope_fade > 0.001 {
        let mut v = Vec::new();
        scope_overlay::push_scope_overlay(&mut v, aspect, model.scope_fade);
        legacy(&mut list, &mut order, HudElement::ScopeSurround, v);
    }
    {
        let mut v = Vec::new();
        reticle_overlay::push_reticle(&mut v, &reticle, aspect);
        legacy(&mut list, &mut order, HudElement::Reticle, v);
    }
    if let Some(age_s) = model.reload_ready_age_s {
        let mut v = Vec::new();
        reticle_marks::push_ready_ring(
            &mut v,
            reticle.aim_clip,
            age_s,
            reticle.aim_radius_clip,
            aspect,
        );
        legacy(&mut list, &mut order, HudElement::ReadyRing, v);
    }
    if let Some(age_s) = model.fire_denied_age_s {
        let mut v = Vec::new();
        reticle_marks::push_denied_flash(
            &mut v,
            reticle.aim_clip,
            reticle.aim_radius_clip,
            age_s,
            aspect,
        );
        legacy(&mut list, &mut order, HudElement::DeniedFlash, v);
    }
    // The reload countdown lives AT the reticle with its arc — one loading display, where the
    // eye already is (the old bottom-center bar was a second, competing one).
    if model.vitals.reload_remaining_s > 0.05 {
        let mut v = Vec::new();
        crate::hud::number::push_number(
            &mut v,
            model.vitals.reload_remaining_s.ceil().clamp(0.0, 99.0) as u32,
            reticle.aim_clip[0] + 0.075,
            reticle.aim_clip[1] - 0.115,
            0.042,
            aspect,
            crate::hud::number::RELOAD_TIME_COLOR,
        );
        legacy(&mut list, &mut order, HudElement::ReloadNumber, v);
    }
    {
        let mut v = Vec::new();
        readouts::push_battle_readouts(&mut v, model, aspect);
        legacy(&mut list, &mut order, HudElement::Readouts, v);
    }
    // H1, H2: the first instruments of the new toolkit — plates, text, bars and glass by name,
    // in the unit `u`, so they scale with the size class where the legacy quads cannot.
    if let Some(bar) = &model.top_bar {
        top_bar::push_top_bar(
            &mut list,
            ui,
            &theme,
            bar,
            model.battle_clock_remaining_s,
            &mut order,
        );
    }
    if let Some(lists) = &model.team_lists {
        team_list::push_team_lists(&mut list, ui, &theme, lists, &mut order);
    }
    {
        let mut v = Vec::new();
        damage_log::push_damage_log(&mut v, &model.damage_log, aspect);
        legacy(&mut list, &mut order, HudElement::DamageLog, v);
    }
    {
        let mut v = Vec::new();
        track_callout::push_track_callout(&mut v, &model.track_feedback, aspect);
        legacy(&mut list, &mut order, HudElement::TrackCallout, v);
    }
    {
        let mut v = Vec::new();
        rack_callout::push_rack_callout(&mut v, model.rack_fire_remaining_s, aspect);
        legacy(&mut list, &mut order, HudElement::RackCallout, v);
    }
    {
        let mut v = Vec::new();
        hit_direction::push_hit_direction(&mut v, &model.incoming_hits, aspect);
        legacy(&mut list, &mut order, HudElement::HitDirection, v);
    }
    if let Some(ammo) = &model.ammo {
        let mut v = Vec::new();
        ammo_panel::push_ammo_panel(&mut v, ammo, aspect);
        legacy(&mut list, &mut order, HudElement::AmmoPanel, v);
    }
    if let Some(modules) = &model.modules {
        let mut v = Vec::new();
        module_panel::push_module_panel(&mut v, modules, aspect);
        legacy(&mut list, &mut order, HudElement::ModulePanel, v);
    }
    if let Some(crew) = &model.crew {
        let mut v = Vec::new();
        crew_panel::push_crew_panel(&mut v, crew, aspect);
        legacy(&mut list, &mut order, HudElement::CrewPanel, v);
    }
    if let Some(map) = &model.minimap {
        // H0: an enamel plate, the relief baked into the sheet as ONE quad, the vector overlays
        // on top, and a pane of glass over the lot.
        let map_rect = minimap::map_rect_px(ui);
        let enamel = theme.plates.enamel_black;
        list.push(
            Element::new(
                HudElement::MinimapPlate,
                map_rect.inset(-ui.px(6.0)),
                Payload::Plate {
                    tile: enamel.tile,
                    radius_u: 3.0,
                    bevel_u: theme.bevel_u,
                    color: enamel.color,
                },
            )
            .z(order),
        );
        order += 1;
        list.push(
            Element::new(
                HudElement::MinimapRelief,
                map_rect,
                Payload::Image { uv: ui_kit::sheet::minimap_region_uv(), color: [1.0; 4] },
            )
            .z(order),
        );
        order += 1;
        let mut v = Vec::new();
        minimap::push_minimap(&mut v, map, aspect);
        legacy(&mut list, &mut order, HudElement::Minimap, v);
        list.push(
            Element::new(
                HudElement::MinimapGlass,
                map_rect,
                Payload::Glass { radius_u: 1.0, phase: 0.2, color: theme.plates.glass.color },
            )
            .z(order),
        );
        order += 1;
    }
    if let Some(outcome) = model.battle_outcome {
        let mut v = Vec::new();
        outcome::push_battle_outcome(&mut v, outcome, aspect);
        legacy(&mut list, &mut order, HudElement::Outcome, v);
    }
    if let Some(age_s) = model.kill_confirm_age_s {
        let mut v = Vec::new();
        kill_marker::push_kill_confirm(&mut v, age_s, aspect);
        legacy(&mut list, &mut order, HudElement::KillConfirm, v);
    }
    // Last, so the modal sits over every battle marker — including the outcome banner, which a
    // player can be reading when they reach for ESC.
    if let Some(menu) = &model.pause_menu {
        let mut v = Vec::new();
        pause_menu::push_pause_menu(&mut v, menu, aspect);
        legacy(&mut list, &mut order, HudElement::PauseMenu, v);
    }
    list
}

/// The battle HUD as vertices: the draw list through the one emitter. Byte-identical to the
/// old builder (`the_draw_list_emits_the_legacy_hud_byte_for_byte`).
pub(crate) fn build_battle_hud(model: &BattleHudModel, aspect: f32) -> Vec<HudVertex> {
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    build_battle_hud_list(model, &ui).emit(&ui, &ui_kit::theme::Theme::standard())
}

#[cfg(test)]
mod reticle_overlay_tests;
#[cfg(test)]
mod tests;

/// The vertices one HUD state draws at one size class on a `width` x `height` viewport: the
/// state's model through the draw list and the one emitter — what the golden instrument
/// uploads, what the probe writes, what the census counts (F8, F9).
pub fn hud_state_vertices(
    state: HudState,
    size: HudSizeClass,
    width: u32,
    height: u32,
) -> Vec<HudVertex> {
    let ui = ui_kit::ui::Ui::new(width, height, size.user_scale());
    build_battle_hud_list(&state.model(), &ui).emit(&ui, &ui_kit::theme::Theme::standard())
}

/// The census of one state: vertices per element, in paint order.
pub fn hud_state_census(state: HudState, aspect: f32) -> Vec<(HudElement, usize)> {
    build_battle_hud_list(&state.model(), &ui_kit::ui::Ui::for_aspect(aspect))
        .iter()
        .map(|e| {
            let n = match &e.payload {
                Payload::Legacy(v) => v.len(),
                Payload::Text { text, .. } => text.chars().count() * 12,
                Payload::Bar { .. } => 12,
                _ => 6,
            };
            (e.id, n)
        })
        .collect()
}
