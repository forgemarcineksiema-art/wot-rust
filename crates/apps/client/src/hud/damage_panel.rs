//! The damage panel (interface program H4, H17): the player's own hull as an instrument, in
//! the bottom-left where World of Tanks keeps it. A side silhouette from the vehicle's hitbox
//! profile with the six modules on it, the two tracks under it, the hit points, the five crew
//! stations, the fire lamp and the ammunition rack's fuze — and every repair clock the crew is
//! running, from the server's own clocks (W-4), never a local guess.
//!
//! This one panel retired four instruments: the module row, the crew row, the thrown-track
//! callout with its re-seat bar, and the rack callout. What the thrown-track callout said with
//! words the track now says with a pulse.

use game_core::{
    CREW_FIRST_AID_S, CREW_ROLE_COUNT, CrewRole, CrewVitals, MODULE_SLOT_COUNT, ModuleCondition,
    ModuleSlot, TankSpec, TrackDamageMask, TrackSide, module_condition,
};
use net::{RepairClocks, TankSnapshot};
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::icons::HudIcon;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{DamagePart, HudElement};
use super::health::health_color;
use super::track_feedback::CalloutView;

/// One module on the silhouette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleView {
    pub slot: ModuleSlot,
    pub condition: ModuleCondition,
    /// Seconds until the crew's field patch lands, when a patchable module is destroyed.
    pub repair_remaining_s: Option<f32>,
}

/// One track side under the silhouette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackView {
    pub side: TrackSide,
    pub condition: ModuleCondition,
    /// Seconds until the thrown track is re-seated.
    pub repair_remaining_s: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrewState {
    Whole,
    Weakened,
    /// Down, with the first aid's seconds left.
    Down {
        remaining_s: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrewView {
    pub role: CrewRole,
    pub state: CrewState,
}

/// The side view's proportions, from the hitbox profile: the hull's length and the turret's
/// length and where it sits, as fractions of the hull. The picture is a schematic, not a
/// drawing — it says WHERE a module is, not what the tank looks like.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Silhouette {
    /// The turret's length over the hull's.
    pub turret_length_frac: f32,
    /// The turret's centre along the hull, −1 (rear) to +1 (front).
    pub turret_center_frac: f32,
}

impl Silhouette {
    pub fn from_spec(spec: &TankSpec) -> Self {
        let hull = spec.hitbox.half_length_m.max(0.1);
        Self {
            turret_length_frac: (spec.hitbox.turret_half_length_m / hull).clamp(0.2, 1.0),
            turret_center_frac: (spec.hitbox.turret_center_z_m / hull).clamp(-0.8, 0.8),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamagePanelModel {
    pub hit_points: u32,
    pub max_hit_points: u32,
    pub silhouette: Silhouette,
    pub modules: [ModuleView; MODULE_SLOT_COUNT],
    pub tracks: [TrackView; 2],
    pub crew: [CrewView; CREW_ROLE_COUNT],
    pub engine_fire: bool,
    pub fuel_fire: bool,
    /// Seconds left on the lit ammunition rack's fuze.
    pub rack_fuze_s: Option<f32>,
    /// The thrown-track beat: which side, how hard, how long ago.
    pub track_callout: Option<CalloutView>,
}

/// The modules the crew field-patches, in `sim::repair`'s order; the turret ring and the radio
/// stay knocked out.
const PATCHED: [ModuleSlot; 4] =
    [ModuleSlot::Engine, ModuleSlot::Suspension, ModuleSlot::Gun, ModuleSlot::AmmoRack];

impl DamagePanelModel {
    /// The panel from the player's snapshot: the module pools and masks, the crew masks, the
    /// fires, and the crew's repair clocks (W-4) advanced by the snapshot's age so the countdown
    /// runs between deliveries.
    pub fn from_snapshot(
        tank: &TankSnapshot,
        spec: &TankSpec,
        clocks: Option<&RepairClocks>,
        snapshot_age_s: f32,
        track_callout: Option<CalloutView>,
    ) -> Self {
        let full = spec.module_health.hit_points_by_slot();
        let modules = std::array::from_fn(|index| {
            let slot = ModuleSlot::ALL[index];
            let condition = module_condition(tank.module_hit_points[index], full[index]);
            let repair_remaining_s = (condition == ModuleCondition::Destroyed
                && PATCHED.contains(&slot))
            .then(|| {
                let clock = clocks.map_or(0.0, |clocks| clocks.module_s[index]) + snapshot_age_s;
                (sim::MODULE_PATCH_S - clock).max(0.0)
            });
            ModuleView { slot, condition, repair_remaining_s }
        });
        let broken = TrackDamageMask::from_bits(tank.track_damage_mask);
        let tracks = std::array::from_fn(|index| {
            let side = if index == 0 { TrackSide::Left } else { TrackSide::Right };
            let condition = if broken.is_broken(side) {
                ModuleCondition::Destroyed
            } else {
                module_condition(
                    u32::from(tank.track_hp[index]),
                    u32::from(game_core::TRACK_HP_MAX),
                )
            };
            let repair_remaining_s = broken.is_broken(side).then(|| {
                let clock = clocks.map_or(0.0, |clocks| clocks.track_s[index]) + snapshot_age_s;
                (sim::TRACK_REPAIR_S - clock).max(0.0)
            });
            TrackView { side, condition, repair_remaining_s }
        });
        let vitals = CrewVitals::from_wire(
            tank.crew_unconscious_mask,
            tank.crew_weakened_mask,
            tank.crew_down_remaining_s,
        );
        let down = vitals.down_remaining_s();
        let crew = std::array::from_fn(|index| {
            let role = CrewRole::ALL[index];
            let state = if let Some(remaining_s) = down[index] {
                CrewState::Down { remaining_s }
            } else if tank.crew_weakened_mask & role.mask_bit() != 0 {
                CrewState::Weakened
            } else {
                CrewState::Whole
            };
            CrewView { role, state }
        });
        Self {
            hit_points: tank.hit_points,
            max_hit_points: spec.hit_points,
            silhouette: Silhouette::from_spec(spec),
            modules,
            tracks,
            crew,
            engine_fire: tank.engine_fire,
            fuel_fire: tank.fuel_fire,
            rack_fuze_s: tank.rack_fire_remaining_s,
            track_callout,
        }
    }

    pub fn burning(&self) -> bool {
        self.engine_fire || self.fuel_fire
    }
}

/// The damage log's colour for a crewman hit (the retired crew row's red, tagged as before).
pub(crate) const CREW_DOWN: [f32; 4] = super::theme::tagged([0.90, 0.26, 0.22, 1.0], 0.95);

pub(crate) fn role_letter(role: CrewRole) -> &'static str {
    match role {
        CrewRole::Commander => "C",
        CrewRole::Gunner => "G",
        CrewRole::Driver => "D",
        CrewRole::Loader => "L",
        CrewRole::RadioOperator => "R",
    }
}

const PANEL_SIZE_U: [f32; 2] = [340.0, 132.0];
/// Above the legacy speed readout, which H5 folds into its own instrument.
const PANEL_OFFSET_U: [f32; 2] = [12.0, 100.0];
const SILHOUETTE_W_U: f32 = 124.0;
const HULL_H_U: f32 = 30.0;
const TURRET_H_U: f32 = 24.0;
const TRACK_H_U: f32 = 6.0;
const ICON_U: f32 = 18.0;
const ROW_H_U: f32 = 24.0;
const TEXT_U: f32 = 16.0;
/// The thrown-track pulse: two beats of the callout's life.
const CALLOUT_PULSE_HZ: f32 = 3.0;

fn condition_color(theme: &Theme, condition: ModuleCondition) -> [f32; 4] {
    match condition {
        ModuleCondition::Healthy => theme.semantic.module[0],
        ModuleCondition::Damaged => theme.semantic.module[1],
        ModuleCondition::Destroyed => theme.semantic.module[2],
    }
}

/// Where a module sits on the schematic: `(x along the hull, −1 rear … +1 front; on the turret?)`.
fn module_seat(slot: ModuleSlot) -> (f32, bool) {
    match slot {
        ModuleSlot::Engine => (-0.62, false),
        ModuleSlot::Suspension => (0.0, false),
        ModuleSlot::AmmoRack => (0.55, false),
        ModuleSlot::Turret => (0.0, true),
        ModuleSlot::Gun => (0.62, true),
        ModuleSlot::Radio => (-0.62, true),
    }
}

pub(crate) fn push_damage_panel(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &DamagePanelModel,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let key = HudElement::DamagePanel;
    let steel = theme.plates.steel_painted;
    let enamel = theme.plates.enamel_black;
    let panel = ui.anchor(Anchor::BottomLeft, PANEL_SIZE_U, PANEL_OFFSET_U);
    push(Element::new(
        key(DamagePart::Plate),
        panel,
        Payload::Plate {
            tile: steel.tile,
            radius_u: 4.0,
            bevel_u: theme.bevel_u,
            color: steel.color,
        },
    ));

    // The silhouette: the turret over the hull over the two tracks, modules seated on them.
    let sil_x = panel.x + ui.px(10.0);
    let sil_w = ui.px(SILHOUETTE_W_U);
    let turret_top = panel.y + ui.px(12.0);
    let turret_w = sil_w * model.silhouette.turret_length_frac;
    let turret_x =
        sil_x + sil_w * 0.5 + model.silhouette.turret_center_frac * sil_w * 0.5 - turret_w * 0.5;
    let turret = Rect::new(turret_x, turret_top, turret_w, ui.px(TURRET_H_U));
    let hull = Rect::new(sil_x, turret.bottom(), sil_w, ui.px(HULL_H_U));
    let plate =
        |color: [f32; 4]| Payload::Plate { tile: enamel.tile, radius_u: 2.0, bevel_u: 1.0, color };
    let steel_dark = [steel.color[0] * 0.7, steel.color[1] * 0.7, steel.color[2] * 0.7, 0.95];
    push(Element::new(key(DamagePart::Turret), turret, plate(steel_dark)));
    push(Element::new(key(DamagePart::Hull), hull, plate(steel_dark)));
    for (index, track) in model.tracks.iter().enumerate() {
        let top = hull.bottom() + ui.px(3.0) + index as f32 * ui.px(TRACK_H_U + 3.0);
        let rect = Rect::new(sil_x, top, sil_w, ui.px(TRACK_H_U));
        let mut color = condition_color(theme, track.condition);
        if let Some(callout) = model.track_callout
            && callout.side == track.side
        {
            // The thrown-track beat: the side pulses white against its state for the callout's
            // life — what the retired callout said in words.
            let pulse =
                0.5 + 0.5 * (callout.age_s * CALLOUT_PULSE_HZ * std::f32::consts::TAU).cos();
            let white = if callout.broke { 0.85 } else { 0.55 };
            for channel in color.iter_mut().take(3) {
                *channel += (1.0 - *channel) * white * pulse;
            }
        }
        push(Element::new(key(DamagePart::Track(track.side)), rect, plate(color)));
        if let Some(remaining_s) = track.repair_remaining_s {
            let frac = 1.0 - (remaining_s / sim::TRACK_REPAIR_S).clamp(0.0, 1.0);
            push(Element::new(
                key(DamagePart::TrackClock(track.side)),
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                Payload::Bar { frac, fill: theme.lamp, back: [0.0, 0.0, 0.0, 0.0] },
            ));
        }
    }
    for module in &model.modules {
        let (along, on_turret) = module_seat(module.slot);
        let host = if on_turret { turret } else { hull };
        let icon = ui.px(ICON_U);
        let cx = host.x + host.w * (0.5 + along * 0.5);
        let rect = Rect::new(cx - icon * 0.5, host.center()[1] - icon * 0.5, icon, icon);
        push(Element::new(
            key(DamagePart::Module(module.slot)),
            rect,
            Payload::Icon {
                icon: HudIcon::for_module(module.slot),
                color: condition_color(theme, module.condition),
            },
        ));
        if let Some(remaining_s) = module.repair_remaining_s {
            let frac = 1.0 - (remaining_s / sim::MODULE_PATCH_S).clamp(0.0, 1.0);
            push(Element::new(
                key(DamagePart::ModuleClock(module.slot)),
                Rect::new(rect.x, rect.bottom() + ui.px(1.0), rect.w, ui.px(3.0)),
                Payload::Bar { frac, fill: theme.lamp, back: [0.0, 0.0, 0.0, 0.45] },
            ));
        }
    }

    // The status column: hit points, the crew, the fires, the radio.
    let col_x = sil_x + sil_w + ui.px(14.0);
    let col_w = panel.right() - ui.px(10.0) - col_x;
    let row = |index: f32| {
        Rect::new(col_x, panel.y + ui.px(8.0) + index * ui.px(ROW_H_U), col_w, ui.px(ROW_H_U))
    };
    let hp_row = row(0.0);
    let hp_frac = (model.hit_points as f32 / model.max_hit_points.max(1) as f32).clamp(0.0, 1.0);
    let number_w = ui.px(64.0);
    push(Element::new(
        key(DamagePart::HpBar),
        Rect::new(hp_row.x, hp_row.y + ui.px(8.0), hp_row.w - number_w - ui.px(8.0), ui.px(8.0)),
        Payload::Bar {
            frac: hp_frac,
            fill: health_color(hp_frac),
            back: [enamel.color[0], enamel.color[1], enamel.color[2], 0.85],
        },
    ));
    push(Element::new(
        key(DamagePart::HpNumber),
        Rect::new(hp_row.right() - number_w, hp_row.y, number_w, hp_row.h),
        Payload::Text {
            text: model.hit_points.min(9_999).to_string(),
            style: Style::VALUE_BOLD,
            size_u: 20.0,
            align: Align::Right,
            color: theme.text.value,
            digits: DigitMode::Tabular,
        },
    ));

    let crew_row = row(1.0);
    let cell_w = crew_row.w / CREW_ROLE_COUNT as f32;
    for (index, member) in model.crew.iter().enumerate() {
        let cell = Rect::new(crew_row.x + index as f32 * cell_w, crew_row.y, cell_w, crew_row.h);
        let color = match member.state {
            CrewState::Whole => theme.text.label,
            CrewState::Weakened => theme.semantic.module[1],
            CrewState::Down { .. } => theme.semantic.module[2],
        };
        push(Element::new(
            key(DamagePart::Crew(member.role)),
            Rect::new(cell.x, cell.y + ui.px(2.0), cell.w, ui.px(TEXT_U)),
            Payload::Text {
                text: role_letter(member.role).to_string(),
                style: Style::VALUE_STRONG,
                size_u: TEXT_U,
                align: Align::Center,
                color,
                digits: DigitMode::Proportional,
            },
        ));
        if let CrewState::Down { remaining_s } = member.state {
            let frac = 1.0 - (remaining_s / CREW_FIRST_AID_S).clamp(0.0, 1.0);
            push(Element::new(
                key(DamagePart::CrewClock(member.role)),
                Rect::new(
                    cell.x + ui.px(4.0),
                    cell.bottom() - ui.px(4.0),
                    cell.w - ui.px(8.0),
                    ui.px(3.0),
                ),
                Payload::Bar { frac, fill: theme.lamp, back: [0.0, 0.0, 0.0, 0.45] },
            ));
        }
    }

    let alarm_row = row(2.0);
    let red = theme.semantic.module[2];
    let mut alarm_x = alarm_row.x;
    if model.burning() {
        let lamp =
            Rect::new(alarm_x, alarm_row.y + ui.px(2.0), ui.px(64.0), alarm_row.h - ui.px(4.0));
        push(Element::new(
            key(DamagePart::FireLamp),
            lamp,
            Payload::Plate {
                tile: enamel.tile,
                radius_u: 3.0,
                bevel_u: 1.0,
                color: [red[0], red[1], red[2], 0.92],
            },
        ));
        push(Element::new(
            key(DamagePart::FireText),
            lamp,
            Payload::Text {
                text: crate::ui_strings::battle::FIRE_LAMP.to_string(),
                style: Style::LABEL,
                size_u: TEXT_U,
                align: Align::Center,
                color: theme.text.value,
                digits: DigitMode::Proportional,
            },
        ));
        alarm_x = lamp.right() + ui.px(8.0);
    }
    if let Some(remaining_s) = model.rack_fuze_s {
        // The fuze blinks in its last three seconds — the crew can still win them.
        let blink_on = remaining_s > 3.0 || (remaining_s * 4.0).floor() as i32 % 2 == 0;
        if blink_on {
            push(Element::new(
                key(DamagePart::RackFuze),
                Rect::new(
                    alarm_x,
                    alarm_row.y + ui.px(2.0),
                    alarm_row.right() - alarm_x,
                    alarm_row.h - ui.px(4.0),
                ),
                Payload::Text {
                    text: format!("{} {remaining_s:.1}", crate::ui_strings::battle::RACK_FUZE),
                    style: Style::LABEL,
                    size_u: TEXT_U,
                    align: Align::Left,
                    color: red,
                    digits: DigitMode::Tabular,
                },
            ));
        }
    }

    let radio = &model.modules[ModuleSlot::Radio.wire_index()];
    if radio.condition == ModuleCondition::Destroyed {
        push(Element::new(
            key(DamagePart::RadioOut),
            row(3.0),
            Payload::Text {
                text: crate::ui_strings::battle::RADIO_OUT.to_string(),
                style: Style::LABEL,
                size_u: TEXT_U,
                align: Align::Left,
                color: red,
                digits: DigitMode::Proportional,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;

    fn snapshot() -> TankSnapshot {
        crate::hud::tests::tank_snapshot(1, 1, 750)
    }

    fn build(model: &DamagePanelModel) -> DrawList<HudElement> {
        let mut list = DrawList::new();
        let mut z = 0;
        push_damage_panel(&mut list, &Ui::reference(), &Theme::standard(), model, &mut z);
        list
    }

    fn has(list: &DrawList<HudElement>, part: DamagePart) -> bool {
        list.find(HudElement::DamagePanel(part)).is_some()
    }

    /// H4: every module is on the silhouette, and a destroyed patchable one carries the crew's
    /// clock — the server's (W-4) advanced by the snapshot's age — while a whole one carries none.
    #[test]
    fn the_damage_panel_shows_all_six_modules_and_their_repair_clocks() {
        let spec = VehicleKind::T54_1951.spec_ref();
        let mut tank = snapshot();
        tank.module_hit_points[ModuleSlot::Engine.wire_index()] = 0;
        let mut thrown = TrackDamageMask::healthy();
        thrown.damage(TrackSide::Right);
        tank.track_damage_mask = thrown.bits();
        let mut clocks = RepairClocks {
            tank_id: tank.tank_id,
            module_s: [0.0; MODULE_SLOT_COUNT],
            track_s: [0.0; 2],
        };
        clocks.module_s[ModuleSlot::Engine.wire_index()] = 3.0;
        clocks.track_s[1] = 4.0;
        let model = DamagePanelModel::from_snapshot(&tank, spec, Some(&clocks), 0.5, None);
        let engine = model.modules[ModuleSlot::Engine.wire_index()];
        assert_eq!(engine.condition, ModuleCondition::Destroyed);
        assert!(
            (engine.repair_remaining_s.expect("clock") - (sim::MODULE_PATCH_S - 3.5)).abs() < 1e-4
        );
        assert!(
            model.modules[ModuleSlot::Gun.wire_index()].repair_remaining_s.is_none(),
            "a whole gun runs no clock"
        );
        assert!(
            (model.tracks[1].repair_remaining_s.expect("track clock")
                - (sim::TRACK_REPAIR_S - 4.5))
                .abs()
                < 1e-4
        );
        assert!(model.tracks[0].repair_remaining_s.is_none());

        let list = build(&model);
        for slot in ModuleSlot::ALL {
            assert!(has(&list, DamagePart::Module(slot)), "{slot:?} is on the silhouette");
        }
        assert!(has(&list, DamagePart::ModuleClock(ModuleSlot::Engine)));
        assert!(!has(&list, DamagePart::ModuleClock(ModuleSlot::Gun)));
        assert!(has(&list, DamagePart::TrackClock(TrackSide::Right)));
        assert!(!has(&list, DamagePart::TrackClock(TrackSide::Left)));
        assert!(has(&list, DamagePart::HpBar) && has(&list, DamagePart::HpNumber));
        for role in CrewRole::ALL {
            assert!(has(&list, DamagePart::Crew(role)));
        }
        let plate = list.find(HudElement::DamagePanel(DamagePart::Plate)).expect("plate").rect;
        let ui = Ui::reference();
        assert!(
            ui.viewport().encloses(&plate) && plate.x < 100.0 && plate.bottom() > 900.0,
            "bottom-left: {plate:?}"
        );
    }

    /// H4: a dead radio prints RADIO OUT; a whole one prints nothing.
    #[test]
    fn a_dead_radio_says_so() {
        let spec = VehicleKind::T54_1951.spec_ref();
        let whole = DamagePanelModel::from_snapshot(&snapshot(), spec, None, 0.0, None);
        assert!(!has(&build(&whole), DamagePart::RadioOut));
        let mut tank = snapshot();
        tank.module_hit_points[ModuleSlot::Radio.wire_index()] = 0;
        let dead = DamagePanelModel::from_snapshot(&tank, spec, None, 0.0, None);
        assert!(
            dead.modules[ModuleSlot::Radio.wire_index()].repair_remaining_s.is_none(),
            "a radio is never patched"
        );
        match &build(&dead)
            .find(HudElement::DamagePanel(DamagePart::RadioOut))
            .expect("RADIO OUT")
            .payload
        {
            Payload::Text { text, .. } => assert_eq!(text, crate::ui_strings::battle::RADIO_OUT),
            other => panic!("{other:?}"),
        }
    }

    /// H17: a burning hull wears the lamp, a cooking rack counts down; a quiet hull shows neither.
    #[test]
    fn a_burning_hull_wears_the_lamp_and_a_cooking_rack_counts_down() {
        let spec = VehicleKind::T54_1951.spec_ref();
        let quiet = DamagePanelModel::from_snapshot(&snapshot(), spec, None, 0.0, None);
        let list = build(&quiet);
        assert!(!has(&list, DamagePart::FireLamp) && !has(&list, DamagePart::RackFuze));
        let mut tank = snapshot();
        tank.engine_fire = true;
        tank.rack_fire_remaining_s = Some(7.0);
        let burning = DamagePanelModel::from_snapshot(&tank, spec, None, 0.0, None);
        assert!(burning.burning());
        let list = build(&burning);
        assert!(has(&list, DamagePart::FireLamp) && has(&list, DamagePart::FireText));
        match &list.find(HudElement::DamagePanel(DamagePart::RackFuze)).expect("fuze").payload {
            Payload::Text { text, .. } => assert_eq!(text, "RACK 7.0"),
            other => panic!("{other:?}"),
        }
    }

    /// The crew states read off the masks: whole, scarred, down with the bandage's clock.
    #[test]
    fn the_crew_row_reads_whole_scarred_and_down() {
        let spec = VehicleKind::T54_1951.spec_ref();
        let mut tank = snapshot();
        tank.crew_unconscious_mask = CrewRole::Loader.mask_bit();
        tank.crew_weakened_mask = CrewRole::Driver.mask_bit();
        tank.crew_down_remaining_s[CrewRole::Loader.wire_index()] = Some(9.0);
        let model = DamagePanelModel::from_snapshot(&tank, spec, None, 0.0, None);
        assert_eq!(
            model.crew[CrewRole::Loader.wire_index()].state,
            CrewState::Down { remaining_s: 9.0 }
        );
        assert_eq!(model.crew[CrewRole::Driver.wire_index()].state, CrewState::Weakened);
        assert_eq!(model.crew[CrewRole::Gunner.wire_index()].state, CrewState::Whole);
        let list = build(&model);
        assert!(has(&list, DamagePart::CrewClock(CrewRole::Loader)));
        assert!(!has(&list, DamagePart::CrewClock(CrewRole::Driver)));
    }
}
