//! The armour inspector at a point (interface program G11). With the inspector on, a click on
//! a plate of the parked hero is a QUESTION — which zone, how much steel, how much at the ray's
//! angle — and SHOOT ME answers it with the crew's own selected round at 100 m through
//! `resolve_traced_impact`, the one function a shell meets. The inspector never estimates: it
//! hands the resolver the trace's own zone, angle and thickness scale, on the hull as it is
//! parked, so the inspector and the shell are one verdict (`the_inspector_equals_the_shell_on_a_thousand_points`).

use game_core::math::HullPose;
use game_core::{ArmorZone, Penetrator, ShellSpec, ShellType, TracedImpact, resolve_traced_impact};
use glam::Vec3;
use ui_kit::draw_list::{Align, DigitMode, DrawList, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::GarageState;
use super::elements::GarageElement as E;
use super::hero_pick::HeroHit;
use super::screen::{plate, put, put_control, text};
use crate::hud::damage_log::zone_name;
use crate::ui_strings::battle as verdicts;
use crate::ui_strings::garage as words;

/// The range the inspector asks at: the one every garage number is quoted at.
pub(super) const INSPECTOR_RANGE_M: f32 = 100.0;

// The readout under the legend: two lines and the SHOOT ME switch.
const PANEL_SIZE_U: [f32; 2] = [640.0, 92.0];
const PANEL_TOP_U: f32 = 336.0;
const PAD_U: f32 = 14.0;
const LINE_H_U: f32 = 24.0;
const SWITCH_SIZE_U: [f32; 2] = [150.0, 34.0];

/// The point the inspector answers at: the trace's own record of the plate under the click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct InspectorPoint {
    pub zone: ArmorZone,
    pub hit_position: Vec3,
    pub plate_normal: Vec3,
    pub impact_angle_degrees: f32,
    pub thickness_scale: f32,
    pub direction: Vec3,
}

impl InspectorPoint {
    pub fn from_hit(hit: &HeroHit) -> Self {
        Self {
            zone: hit.zone,
            hit_position: hit.hit_position,
            plate_normal: hit.plate_normal,
            impact_angle_degrees: hit.impact_angle_degrees,
            thickness_scale: hit.thickness_scale,
            direction: hit.direction,
        }
    }
}

/// What the inspector says at a point: the plate, and — SHOOT ME — the round's verdict.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct InspectorReading {
    pub zone: ArmorZone,
    /// The plate's own steel at this spot: the zone's nominal times the trace's thickness scale.
    pub nominal_mm: f32,
    /// The steel the round meets at the ray's angle — the resolver's word.
    pub effective_mm: f32,
    pub angle_degrees: f32,
    pub round: String,
    pub penetration_mm: f32,
    pub penetrated: bool,
    pub ricocheted: bool,
    pub shattered: bool,
}

impl InspectorReading {
    /// The verdict's word, in the log's own order: shatter, ricochet, pen, no pen.
    pub fn verdict_word(&self) -> &'static str {
        if self.shattered {
            verdicts::HIT_SHATTER
        } else if self.ricocheted {
            verdicts::HIT_RICOCHET
        } else if self.penetrated {
            verdicts::HIT_PEN
        } else {
            verdicts::HIT_NO_PEN
        }
    }
}

/// A round's word: its designation, or its type when the catalogue has none.
pub(super) fn round_word(shell: &ShellSpec) -> String {
    shell.round.map_or_else(
        || {
            match shell.shell_type {
                ShellType::ArmorPiercing => "AP",
                ShellType::Apcr => "APCR",
                ShellType::Heat => "HEAT",
                ShellType::HighExplosive => "HE",
            }
            .to_string()
        },
        |round| round.designation().to_string(),
    )
}

impl GarageState {
    pub(super) fn inspector_point(&self) -> Option<InspectorPoint> {
        self.inspector_point
    }

    /// A click on a plate under the inspector: the question moves there; `None` clears it.
    pub(super) fn set_inspector_point(&mut self, point: Option<InspectorPoint>) {
        self.inspector_point = point;
    }

    pub(super) fn shoot_me(&self) -> bool {
        self.shoot_me
    }

    /// The SHOOT ME switch: the verdict line and the marker's colour on or off.
    pub(super) fn toggle_shoot_me(&mut self) {
        self.shoot_me = !self.shoot_me;
    }

    /// The crew's own round: the one the rack has selected.
    pub(super) fn own_round(&self) -> ShellSpec {
        let draft = self.draft();
        draft
            .ammo_options()
            .get(draft.ammo_index())
            .cloned()
            .unwrap_or_else(|| draft.assembled_spec().gun.shell)
    }

    /// The reading at `point` for `shell` at the inspector's range: the trace's own zone,
    /// angle and thickness scale through the resolver a shell meets, on the hull as parked.
    pub(super) fn inspector_reading(
        &self,
        point: InspectorPoint,
        shell: &ShellSpec,
    ) -> InspectorReading {
        let spec = self.draft().assembled_spec();
        let pose = self.drive_in_pose();
        let result = resolve_traced_impact(&TracedImpact {
            shell,
            armor: &spec.hull,
            hull: HullPose::level(pose.yaw_rad),
            zone: point.zone,
            impact_angle_degrees: point.impact_angle_degrees,
            distance_m: INSPECTOR_RANGE_M,
            thickness_scale: point.thickness_scale,
            direction: point.direction,
            belts_present: [true, true],
        });
        InspectorReading {
            zone: point.zone,
            nominal_mm: spec.hull.plate(point.zone).nominal_thickness_mm * point.thickness_scale,
            effective_mm: result.effective_armor_mm,
            angle_degrees: point.impact_angle_degrees,
            round: round_word(shell),
            penetration_mm: shell.penetration_mm_at_distance(INSPECTOR_RANGE_M),
            penetrated: result.penetrated,
            ricocheted: result.ricocheted,
            shattered: result.ricocheted && shell.penetrator == Penetrator::TungstenCore,
        }
    }

    /// The marker on the plate (the FX pass draws it): where, which way it faces, and its
    /// colour — the lamp for a question, the verdict's green or red once SHOOT ME answers.
    pub(in crate::app) fn inspector_marker(&self, theme: &Theme) -> Option<(Vec3, Vec3, [f32; 3])> {
        let point = self.inspector_point?;
        let color = if self.shoot_me {
            let reading = self.inspector_reading(point, &self.own_round());
            let c = if reading.penetrated {
                theme.semantic.hp_ramp[0]
            } else {
                theme.semantic.hp_ramp[2]
            };
            [c[0], c[1], c[2]]
        } else {
            [theme.lamp[0], theme.lamp[1], theme.lamp[2]]
        };
        Some((point.hit_position, point.plate_normal, color))
    }
}

/// The readout under the legend (drawn while the inspector is on): the plate's line, the
/// verdict's line when SHOOT ME is on, and the switch.
pub(super) fn push_inspector_panel(
    list: &mut DrawList<E>,
    ui: &Ui,
    theme: &Theme,
    state: &GarageState,
) {
    let panel = ui.anchor(Anchor::Top, PANEL_SIZE_U, [0.0, PANEL_TOP_U]);
    put(list, E::InspectorPlate, panel, plate(theme, theme.plates.enamel_black, 3.0));
    let pad = ui.px(PAD_U);
    let switch = Rect::new(
        panel.right() - pad - ui.px(SWITCH_SIZE_U[0]),
        panel.y + (panel.h - ui.px(SWITCH_SIZE_U[1])) * 0.5,
        ui.px(SWITCH_SIZE_U[0]),
        ui.px(SWITCH_SIZE_U[1]),
    );
    let line = |i: f32| {
        Rect::new(
            panel.x + pad,
            panel.y + pad + ui.px(i * LINE_H_U),
            switch.x - pad - (panel.x + pad),
            ui.px(LINE_H_U),
        )
    };
    let reading =
        state.inspector_point().map(|point| state.inspector_reading(point, &state.own_round()));
    let first = reading.as_ref().map_or_else(
        || words::INSPECTOR_HINT.to_string(),
        |r| {
            format!(
                "{} \u{b7} {} {} @ {}\u{b0} \u{b7} {} {} {}",
                zone_name(r.zone),
                r.nominal_mm.round() as i32,
                words::LEGEND_UNIT,
                r.angle_degrees.round() as i32,
                r.effective_mm.round() as i32,
                words::LEGEND_UNIT,
                words::INSPECTOR_EFFECTIVE
            )
        },
    );
    put(
        list,
        E::InspectorLine(0),
        line(0.0),
        text(&first, Style::VALUE, 16.0, Align::Left, theme.text.value, DigitMode::Tabular),
    );
    if let Some(r) = reading.as_ref().filter(|_| state.shoot_me()) {
        let color =
            if r.penetrated { theme.semantic.hp_ramp[0] } else { theme.semantic.hp_ramp[2] };
        put(
            list,
            E::InspectorLine(1),
            line(1.0),
            text(
                &format!(
                    "{} \u{b7} {} {} @ {} {} \u{b7} {}",
                    r.round,
                    r.penetration_mm.round() as i32,
                    words::LEGEND_UNIT,
                    INSPECTOR_RANGE_M as i32,
                    words::INSPECTOR_METRES,
                    r.verdict_word()
                ),
                Style::VALUE,
                16.0,
                Align::Left,
                color,
                DigitMode::Tabular,
            ),
        );
    }
    put_control(
        list,
        E::InspectorShootMe,
        switch,
        plate(theme, theme.plates.steel_painted, 2.0),
        if state.shoot_me() { WidgetState::Focused } else { WidgetState::Idle },
    );
    put(
        list,
        E::InspectorShootMeLabel,
        switch,
        text(
            words::INSPECTOR_SHOOT_ME,
            Style::VALUE_STRONG,
            18.0,
            Align::Center,
            if state.shoot_me() { theme.lamp } else { theme.text.value },
            DigitMode::Proportional,
        ),
    );
}
