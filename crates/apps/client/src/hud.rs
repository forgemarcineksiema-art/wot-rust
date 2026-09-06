use renderer_api::HudVertex;
use ui_kit::draw_list::{DrawList, Element, Payload};
use ui_kit::rect::Rect;

use crate::hud::reticle::ReticleStatus;

pub(crate) mod ammo_panel;
pub(crate) mod budget;
pub(crate) mod command_wheel;
pub(crate) mod damage_log;
pub(crate) mod damage_panel;
pub(crate) mod demo;
pub(crate) mod demo_strip;
pub(crate) mod editor;
pub(crate) mod elements;
pub use ui_kit::font;
pub(crate) mod health;
pub(crate) mod hit_direction;
pub(crate) mod hull_box;
pub(crate) use ui_kit::icons;
pub(crate) mod kill_feed;
pub(crate) mod kill_marker;
pub(crate) mod layout;
pub(crate) mod marker;
pub(crate) mod minimap;
pub(crate) mod net_readout;
pub(crate) mod number;
pub(crate) mod outcome;
pub(crate) mod pause_menu;
pub(crate) mod ping_marker;
pub use ui_kit::primitives;
pub(crate) mod readouts;
pub(crate) mod reticle;
pub(crate) mod reticle_marks;
pub(crate) mod reticle_overlay;
pub(crate) mod reticle_readouts;
pub(crate) mod reticle_sweep;
pub(crate) mod review;
pub(crate) mod scope_overlay;
pub(crate) mod sixth_sense;
pub(crate) mod spectate;
pub(crate) mod speed;
pub(crate) mod states;
pub(crate) mod team_list;
pub(crate) mod top_bar;
pub use ui_kit::theme;
pub(crate) mod track_feedback;

pub(crate) use elements::HudElement;
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
    /// The cruise latch (H5), `-2..=3`; the speed instrument lights its notches by it.
    pub cruise_level: i8,
    /// Sniper magnification; `None` in third person (no readout).
    pub zoom_factor: Option<f32>,
    /// Recent dealt/taken hits, newest first (`hud/damage_log.rs`, H8).
    pub damage_log: Vec<damage_log::DamageLogEntry>,
    /// N (H8): the log shows its newest row only.
    pub hit_log_collapsed: bool,
    /// Incoming hits resolved to screen bearings (`hud/hit_direction.rs`).
    pub incoming_hits: Vec<hit_direction::IncomingHit>,
    pub ammo: Option<ammo_panel::AmmoHudModel>,
    /// The player's own hull as an instrument (`hud/damage_panel.rs`, H4/H17): the silhouette
    /// with its modules, the tracks, the hit points, the crew, the fires and every repair clock.
    /// `None` before the first snapshot, then always present so a knocked-out gun is never a
    /// mystery.
    pub damage: Option<damage_panel::DamagePanelModel>,
    pub minimap: Option<minimap::MinimapModel>,
    pub battle_outcome: Option<BattleHudOutcome>,
    /// Seconds left on the battle clock, drawn top-center as M:SS; `None` hides it (untimed).
    pub battle_clock_remaining_s: Option<f32>,
    /// The top bar (H1): frags and pools; `None` before the roster lands.
    pub top_bar: Option<top_bar::TopBarModel>,
    /// The team lists (H2); `None` before the roster lands.
    pub team_lists: Option<team_list::TeamListsModel>,
    /// The world-anchored markers (H10): every spotted enemy, the target in full.
    pub markers: Option<marker::MarkerModel>,
    /// The sixth sense (H13): lit exactly while the own mask says an enemy sees us.
    pub sixth_sense_lit: bool,
    /// The visibility budget line (H14); `None` before the roster lands.
    pub budget: Option<budget::BudgetModel>,
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
    /// The command wheel (H16) while Z is held or a refusal still knocks; `None` otherwise.
    pub command_wheel: Option<command_wheel::CommandWheelModel>,
    /// The team's pings in the world (H16); `None` before the battle.
    pub pings: Option<ping_marker::PingModel>,
    /// The team's newest word (H16); `None` when none is fresh.
    pub team_word: Option<ping_marker::TeamWord>,
    /// The kill feed (H3): every kill on the field, off the wire; `None` before the roster.
    pub kill_feed: Option<kill_feed::KillFeedModel>,
    /// The connection readout (H18); `None` in the offline examples.
    pub net: Option<net_readout::NetReadoutModel>,
    /// The dead crew's HUD (H19): `Some` once the own hull is a wreck — the intel sits back,
    /// the gun's instruments go, an ally's panel may come off the wire.
    pub dead: Option<spectate::DeadModel>,
    /// The semantic palette the HUD wears (H22): the player's setting.
    pub palette: ui_kit::theme::Palette,
    /// The layout (H21): the preset and every instrument's placement.
    pub layout: layout::HudLayout,
    /// The HUD editor's overlay while it is open (H21).
    pub editor: Option<editor::EditorModel>,
}

/// Build the 2D HUD overlay from the vitals alone (the reticle and the readouts; the hit
/// points live in the damage panel, which needs a snapshot). `aspect` keeps the crosshair
/// square on non-square viewports.
pub fn build_hud(vitals: HudVitals, aspect: f32) -> Vec<HudVertex> {
    build_battle_hud(
        &BattleHudModel {
            vitals,
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
            dead: None,
            palette: ui_kit::theme::Palette::Standard,
            layout: crate::hud::layout::HudLayout::default(),
            editor: None,
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
        cruise_level: 0,
        zoom_factor,
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
        // The positional test path has no camera; sniper mode implies a settled scope.
        scope_fade: if reticle.is_some_and(|r| r.mode == reticle::ReticleMode::Sniper) {
            1.0
        } else {
            0.0
        },
        pause_menu: None,
        command_wheel: None,
        pings: None,
        team_word: None,
        kill_feed: None,
        net: None,
        dead: None,
        palette: ui_kit::theme::Palette::Standard,
        layout: crate::hud::layout::HudLayout::default(),
        editor: None,
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
    let theme = ui_kit::theme::Theme::standard().with_palette(model.palette);
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
    // H21: every instrument lays out in its own context — nudged and scaled by its placement
    // — and the preset says which show. The reticle stack above has no placement.
    let layout = &model.layout;
    let ui_of = |instrument: layout::Instrument| layout.ui_for(ui, instrument);
    if let Some(bar) = &model.top_bar {
        top_bar::push_top_bar(
            &mut list,
            &ui_of(layout::Instrument::TopBar),
            &theme,
            bar,
            model.battle_clock_remaining_s,
            &mut order,
        );
    }
    if let Some(lists) = &model.team_lists {
        team_list::push_team_lists(
            &mut list,
            &ui_of(layout::Instrument::TeamLists),
            &theme,
            lists,
            &mut order,
        );
    }
    // H13/H14: the sixth-sense lamp under the top bar, the budget line under the lamp's slot.
    let ui_stack = ui_of(layout::Instrument::TopStack);
    sixth_sense::push_sixth_sense(&mut list, &ui_stack, &theme, model.sixth_sense_lit, &mut order);
    if let Some(budget) = &model.budget
        && layout.shows_budget()
    {
        budget::push_budget(&mut list, &ui_stack, &theme, budget, &mut order);
    }
    // H16: the team's newest word under the budget line.
    if let Some(word) = &model.team_word {
        ping_marker::push_team_word(&mut list, &ui_stack, &theme, word, &mut order);
    }
    // H3, H18: the kill feed under the enemy ear, the connection under the frame counter.
    if let Some(feed) = &model.kill_feed
        && layout.shows(layout::Instrument::KillFeed)
    {
        kill_feed::push_kill_feed(
            &mut list,
            &ui_of(layout::Instrument::KillFeed),
            &theme,
            feed,
            &mut order,
        );
    }
    if let Some(net) = &model.net
        && layout.shows(layout::Instrument::NetReadout)
    {
        net_readout::push_net_readout(
            &mut list,
            &ui_of(layout::Instrument::NetReadout),
            &theme,
            net,
            &mut order,
        );
    }
    // H10: the markers ride under everything drawn so far — they are world-anchored and may sit
    // where the reticle is; the reticle stays on top. The team's pings (H16) ride with them.
    {
        let mut below: i16 = -64;
        if let Some(markers) = &model.markers {
            marker::push_markers(&mut list, ui, &theme, markers, &mut below);
        }
        if let Some(pings) = &model.pings {
            ping_marker::push_pings(&mut list, ui, &theme, pings, &mut below);
        }
    }
    // H8: the hit log under the reticle, on the toolkit. While the wheel (H16) is open the
    // ring takes the centre and the log yields it; the log is back on the release.
    let wheel_open = model.command_wheel.as_ref().is_some_and(|wheel| wheel.open);
    if !wheel_open {
        damage_log::push_hit_log(
            &mut list,
            &ui_of(layout::Instrument::HitLog),
            &theme,
            &model.damage_log,
            layout.hit_log_folded(model.hit_log_collapsed),
            &mut order,
        );
    }
    {
        let mut v = Vec::new();
        hit_direction::push_hit_direction(&mut v, &model.incoming_hits, aspect, model.palette);
        legacy(&mut list, &mut order, HudElement::HitDirection, v);
    }
    // H5/H6: the speed instrument and the ammunition panel, on the new toolkit.
    speed::push_speed(
        &mut list,
        &ui_of(layout::Instrument::Speed),
        &theme,
        model.speed_kmh,
        model.cruise_level,
        &mut order,
    );
    if let Some(ammo) = &model.ammo {
        ammo_panel::push_ammo_panel(
            &mut list,
            &ui_of(layout::Instrument::Ammo),
            &theme,
            ammo,
            &mut order,
        );
    }
    // H4/H17: the damage panel — the four callout instruments folded into one plate.
    if let Some(damage) = &model.damage {
        damage_panel::push_damage_panel(
            &mut list,
            &ui_of(layout::Instrument::DamagePanel),
            &theme,
            damage,
            &mut order,
        );
    }
    if let Some(map) = &model.minimap {
        // H0: an enamel plate, the relief baked into the sheet as ONE quad, the vector overlays
        // on top, and a pane of glass over the lot. H21: the whole square in its own context.
        let ui_map = ui_of(layout::Instrument::Minimap);
        let ui = &ui_map;
        let map_rect = minimap::map_rect_px(ui, map.size);
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
        // The overlay is clip-space vertices: they take the square's nudge as a clip translation.
        let nudge = ui.nudge_px();
        let viewport = ui.viewport();
        let shift = [nudge[0] * 2.0 / viewport.w, -nudge[1] * 2.0 / viewport.h];
        for vertex in &mut v {
            vertex.position[0] += shift[0];
            vertex.position[1] += shift[1];
        }
        legacy(&mut list, &mut order, HudElement::Minimap, v);
        // H15: the blips are class glyphs in the team colours, with the seat where it fits; the
        // grid's letters and numbers along the edges on the sizes that fit them.
        let glyph = ui.px(if map.size.labelled() { 14.0 } else { 11.0 });
        for (index, (blip, enemy)) in map
            .allies
            .iter()
            .map(|b| (b, false))
            .chain(map.enemies.iter().map(|b| (b, true)))
            .enumerate()
        {
            let i = index as u8;
            let center = map.world_to_px(blip.xz, ui);
            let color = if enemy { theme.semantic.team_enemy } else { theme.semantic.team_ally };
            list.push(
                Element::new(
                    HudElement::MinimapBlip(i),
                    Rect::new(center[0] - glyph * 0.5, center[1] - glyph * 0.5, glyph, glyph),
                    Payload::Icon { icon: ui_kit::icons::HudIcon::for_class(blip.class), color },
                )
                .clipped(Some(map_rect))
                .z(order),
            );
            order += 1;
            if map.size.labelled() {
                // H23: the seat letter sits on enamel, not on the relief — ink over a plate.
                let enamel = theme.plates.enamel_black;
                list.push(
                    Element::new(
                        HudElement::MinimapSeatPlate(i),
                        Rect::new(
                            center[0] + glyph * 0.45,
                            center[1] - ui.px(10.0),
                            ui.px(20.0),
                            ui.px(20.0),
                        ),
                        Payload::Plate {
                            tile: enamel.tile,
                            radius_u: 2.0,
                            bevel_u: 0.0,
                            color: [enamel.color[0], enamel.color[1], enamel.color[2], 0.82],
                        },
                    )
                    .clipped(Some(map_rect))
                    .z(order),
                );
                order += 1;
                list.push(
                    Element::new(
                        HudElement::MinimapSeat(i),
                        Rect::new(
                            center[0] + glyph * 0.55,
                            center[1] - ui.px(8.0),
                            ui.px(18.0),
                            ui.px(16.0),
                        ),
                        Payload::Text {
                            text: blip.seat.to_string(),
                            style: ui_kit::font::Style::VALUE_STRONG,
                            size_u: 16.0,
                            align: ui_kit::draw_list::Align::Left,
                            // The glyph carries the team; the letter is ink on enamel (H23).
                            color: theme.text.label,
                            digits: ui_kit::draw_list::DigitMode::Proportional,
                        },
                    )
                    .clipped(Some(map_rect))
                    .z(order),
                );
                order += 1;
            }
        }
        if map.size.labelled() {
            let cell = map_rect.w / minimap::GRID_CELLS as f32;
            for i in 0..minimap::GRID_CELLS {
                let letter = char::from(b'A' + i as u8).to_string();
                list.push(
                    Element::new(
                        HudElement::MinimapGridLabel(i as u8),
                        Rect::new(
                            map_rect.x + i as f32 * cell,
                            map_rect.y + ui.px(1.0),
                            cell,
                            ui.px(16.0),
                        ),
                        Payload::Text {
                            text: letter,
                            style: ui_kit::font::Style::VALUE,
                            size_u: 16.0,
                            align: ui_kit::draw_list::Align::Center,
                            color: theme.text.label,
                            digits: ui_kit::draw_list::DigitMode::Proportional,
                        },
                    )
                    .z(order),
                );
                order += 1;
                let row = (minimap::GRID_CELLS - 1 - i) as f32;
                list.push(
                    Element::new(
                        HudElement::MinimapGridLabel((minimap::GRID_CELLS + i) as u8),
                        Rect::new(
                            map_rect.x + ui.px(3.0),
                            map_rect.y + row * cell + cell * 0.5,
                            ui.px(24.0),
                            ui.px(16.0),
                        ),
                        Payload::Text {
                            text: (i + 1).to_string(),
                            style: ui_kit::font::Style::VALUE,
                            size_u: 16.0,
                            align: ui_kit::draw_list::Align::Left,
                            color: theme.text.label,
                            digits: ui_kit::draw_list::DigitMode::Tabular,
                        },
                    )
                    .z(order),
                );
                order += 1;
            }
        }
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
    // H16: the command wheel over the battle, under the modal only.
    if let Some(wheel) = &model.command_wheel {
        command_wheel::push_command_wheel(&mut list, ui, &theme, wheel, &mut order);
    }
    // H21: the editor's overlay over the living instruments, under the modal.
    if let Some(editor) = &model.editor {
        let frames = editor::instrument_frames(&list);
        editor::push_editor(&mut list, ui, &theme, &frames, editor, &mut order);
    }
    // H19: a dead crew keeps the intel — sat back — and nothing of its own gun; the ally it
    // rides, if any, brings its strip and its panel full-lit. The banner keeps its light; the modal
    // below draws over all of it.
    if let Some(dead) = &model.dead {
        list.retain(spectate::survives_death);
        list.dim_where(spectate::INTEL_ALPHA, spectate::dimmed_when_dead);
        if let Some(strip) = &dead.spectating {
            let ui_stack = model.layout.ui_for(ui, layout::Instrument::TopStack);
            spectate::push_spectate(&mut list, &ui_stack, &theme, strip, &mut order);
        }
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
    build_battle_hud_list(model, &ui)
        .emit(&ui, &ui_kit::theme::Theme::standard().with_palette(model.palette))
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
    let mut model = state.model();
    // The staged markers are authored in reference pixels; the frame is whatever size it is.
    model.markers =
        model.markers.map(|markers| markers.scaled_from_reference([width as f32, height as f32]));
    model.pings =
        model.pings.map(|pings| pings.scaled_from_reference([width as f32, height as f32]));
    build_battle_hud_list(&model, &ui)
        .emit(&ui, &ui_kit::theme::Theme::standard().with_palette(model.palette))
}

/// The draw list one HUD state builds at one size class on a `width` x `height` viewport,
/// staged exactly as `hud_state_vertices` stages it: what the floors (H23, H24) walk.
pub fn hud_state_list(
    state: HudState,
    size: HudSizeClass,
    width: u32,
    height: u32,
) -> DrawList<HudElement> {
    let ui = ui_kit::ui::Ui::new(width, height, size.user_scale());
    let mut model = state.model();
    model.markers =
        model.markers.map(|markers| markers.scaled_from_reference([width as f32, height as f32]));
    model.pings =
        model.pings.map(|pings| pings.scaled_from_reference([width as f32, height as f32]));
    build_battle_hud_list(&model, &ui)
}

/// The battle strings' size floor (H23): no battle text under 16 px at 1080p.
pub const TEXT_SIZE_FLOOR_U: f32 = 16.0;
/// The numbers the crew acts on — the hit points, the round counts, the speed, the clock —
/// sit at 24 px or more.
pub const ACTED_ON_SIZE_FLOOR_U: f32 = 24.0;

/// Whether an element is one of the numbers the crew acts on (H23).
pub fn is_acted_on_number(id: HudElement) -> bool {
    matches!(
        id,
        HudElement::DamagePanel(elements::DamagePart::HpNumber)
            | HudElement::Ammo(elements::AmmoPart::Count(_))
            | HudElement::Speed(elements::SpeedPart::Number)
            | HudElement::TopBarClock
    )
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
