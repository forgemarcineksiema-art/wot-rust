//! The world-anchored markers (interface program H10): every spotted enemy hull wears a thin,
//! dimmed bar over its hitbox; THE target — the hull the crew marked with T (H11) — wears the
//! full marker: the class glyph, the short name and the seat, the hit points as a bar and a
//! number, and the range. One hull speaks; the rest are known.
//!
//! Honesty: a marker exists only for a hull the snapshot shows (spotted, alive); the numbers are
//! the snapshot's (the server quantises a distant enemy's hit points before they get here).

use engine::PresentationTank;
use game_core::{TankId, TeamId, VehicleKind};
use net::RosterEntry;
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::icons::HudIcon;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

use super::elements::{HudElement, MarkerPart};
use super::health::health_color;

#[derive(Debug, Clone, PartialEq)]
pub struct HullMarker {
    pub id: TankId,
    /// The hull's projected hitbox on the screen, in physical pixels — the marker hangs over it
    /// and the T key reads it.
    pub rect_px: Rect,
    pub vehicle: VehicleKind,
    pub seat: char,
    pub hit_points: u32,
    pub max_hit_points: u32,
    pub distance_m: u32,
    pub is_target: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarkerModel {
    pub hulls: Vec<HullMarker>,
}

fn clip_to_px(clip: [f32; 2], viewport_px: [f32; 2]) -> [f32; 2] {
    [(clip[0] + 1.0) * 0.5 * viewport_px[0], (1.0 - clip[1]) * 0.5 * viewport_px[1]]
}

impl MarkerModel {
    /// The markers of one frame: every live enemy the player's team has spotted, projected.
    pub fn from_presentation(
        tanks: &[PresentationTank],
        player_tank: TankId,
        player_team: TeamId,
        view_projection: [[f32; 4]; 4],
        viewport_px: [f32; 2],
        roster: &[RosterEntry],
        target: Option<TankId>,
    ) -> Self {
        let player_bit = player_team.spotting_bit();
        let player_position =
            tanks.iter().find(|tank| tank.id == player_tank).map(|tank| tank.translation);
        let mut hulls = Vec::new();
        for tank in tanks {
            if tank.id == player_tank || tank.team == player_team || tank.hit_points == 0 {
                continue;
            }
            if tank.spotted_by_teams_mask & player_bit == 0 {
                continue;
            }
            let Some((min, max)) = super::spot_bracket::projected_hitbox(
                tank.translation,
                tank.hull_yaw_rad,
                tank.vehicle,
                view_projection,
            ) else {
                continue;
            };
            let top_left = clip_to_px([min[0], max[1]], viewport_px);
            let bottom_right = clip_to_px([max[0], min[1]], viewport_px);
            let distance_m = player_position.map_or(0.0, |p| {
                let d = [
                    tank.translation[0] - p[0],
                    tank.translation[1] - p[1],
                    tank.translation[2] - p[2],
                ];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
            });
            hulls.push(HullMarker {
                id: tank.id,
                rect_px: Rect::from_min_max(top_left, bottom_right),
                vehicle: tank.vehicle,
                seat: roster
                    .iter()
                    .find(|entry| entry.tank_id == tank.id)
                    .map_or(' ', RosterEntry::seat_letter),
                hit_points: tank.hit_points,
                max_hit_points: tank.vehicle.spec_ref().hit_points,
                distance_m: distance_m.round() as u32,
                is_target: target == Some(tank.id),
            });
        }
        Self { hulls }
    }

    /// A staged model's boxes are authored in reference pixels (1920 × 1080); the golden
    /// instrument scales them to the viewport it renders. The live model is never scaled — its
    /// boxes are already physical.
    pub fn scaled_from_reference(mut self, viewport_px: [f32; 2]) -> Self {
        let sx = viewport_px[0] / 1920.0;
        let sy = viewport_px[1] / 1080.0;
        for hull in &mut self.hulls {
            let r = hull.rect_px;
            hull.rect_px = Rect::new(r.x * sx, r.y * sy, r.w * sx, r.h * sy);
        }
        self
    }

    /// The hull under a screen point — the reticle's aim, for T. The nearest by distance wins
    /// when hitboxes overlap on the screen.
    pub fn hull_under(&self, point_px: [f32; 2]) -> Option<TankId> {
        self.hulls
            .iter()
            .filter(|hull| hull.rect_px.contains(point_px))
            .min_by_key(|hull| hull.distance_m)
            .map(|hull| hull.id)
    }
}

const FULL_W_U: f32 = 168.0;
const FULL_H_U: f32 = 44.0;
const DIM_W_U: f32 = 56.0;
const DIM_H_U: f32 = 4.0;
const GAP_U: f32 = 8.0;
const TEXT_U: f32 = 16.0;

/// Draw the markers, world-anchored: under the reticle stack in paint order (the marker may sit
/// where the crosshair is), over the picture.
pub(crate) fn push_markers(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &MarkerModel,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let steel = theme.plates.steel_painted;
    let enamel = theme.plates.enamel_black;
    for (index, hull) in model.hulls.iter().enumerate() {
        let i = index as u8;
        let cx = hull.rect_px.center()[0];
        let hp_frac = (hull.hit_points as f32 / hull.max_hit_points.max(1) as f32).clamp(0.0, 1.0);
        if !hull.is_target {
            // Known, not spoken to: a thin dimmed bar over the hull, nothing else.
            let w = ui.px(DIM_W_U);
            let rect =
                Rect::new(cx - w * 0.5, hull.rect_px.y - ui.px(GAP_U + DIM_H_U), w, ui.px(DIM_H_U));
            push(Element::new(
                HudElement::Marker(MarkerPart::Bar(i)),
                rect,
                Payload::Bar {
                    frac: hp_frac,
                    fill: [
                        theme.semantic.team_enemy[0],
                        theme.semantic.team_enemy[1],
                        theme.semantic.team_enemy[2],
                        0.7,
                    ],
                    back: [enamel.color[0], enamel.color[1], enamel.color[2], 0.5],
                },
            ));
            continue;
        }
        let w = ui.px(FULL_W_U);
        let plate =
            Rect::new(cx - w * 0.5, hull.rect_px.y - ui.px(GAP_U + FULL_H_U), w, ui.px(FULL_H_U));
        push(Element::new(
            HudElement::Marker(MarkerPart::Plate(i)),
            plate,
            Payload::Plate {
                tile: steel.tile,
                radius_u: 3.0,
                bevel_u: theme.bevel_u,
                color: steel.color,
            },
        ));
        let glyph = ui.px(18.0);
        push(Element::new(
            HudElement::Marker(MarkerPart::Class(i)),
            Rect::new(plate.x + ui.px(6.0), plate.y + ui.px(4.0), glyph, glyph),
            Payload::Icon {
                icon: HudIcon::for_class(hull.vehicle.class()),
                color: theme.semantic.team_enemy,
            },
        ));
        push(Element::new(
            HudElement::Marker(MarkerPart::Name(i)),
            Rect::new(
                plate.x + ui.px(30.0),
                plate.y + ui.px(4.0),
                plate.w - ui.px(36.0),
                ui.px(TEXT_U),
            ),
            Payload::Text {
                text: format!("{} \u{b7} {}", hull.vehicle.short_name(), hull.seat),
                style: Style::LABEL,
                size_u: TEXT_U,
                align: Align::Left,
                color: theme.text.label,
                digits: DigitMode::Proportional,
            },
        ));
        let number_w = ui.px(48.0);
        let distance_w = ui.px(56.0);
        let bar = Rect::new(
            plate.x + ui.px(8.0),
            plate.y + ui.px(30.0),
            plate.w - ui.px(16.0) - number_w - distance_w,
            ui.px(5.0),
        );
        push(Element::new(
            HudElement::Marker(MarkerPart::Bar(i)),
            bar,
            Payload::Bar {
                frac: hp_frac,
                fill: health_color(hp_frac),
                back: [enamel.color[0], enamel.color[1], enamel.color[2], 0.85],
            },
        ));
        push(Element::new(
            HudElement::Marker(MarkerPart::Number(i)),
            Rect::new(bar.right() + ui.px(4.0), plate.y + ui.px(24.0), number_w, ui.px(TEXT_U)),
            Payload::Text {
                text: hull.hit_points.min(9_999).to_string(),
                style: Style::VALUE_STRONG,
                size_u: TEXT_U,
                align: Align::Right,
                color: theme.text.value,
                digits: DigitMode::Tabular,
            },
        ));
        push(Element::new(
            HudElement::Marker(MarkerPart::Distance(i)),
            Rect::new(
                plate.right() - ui.px(4.0) - distance_w,
                plate.y + ui.px(24.0),
                distance_w,
                ui.px(TEXT_U),
            ),
            Payload::Text {
                text: format!("{} {}", hull.distance_m, crate::ui_strings::battle::DISTANCE_UNIT),
                style: Style::VALUE,
                size_u: TEXT_U,
                align: Align::Right,
                color: theme.text.unit,
                digits: DigitMode::Tabular,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hull(id: u64, x: f32, is_target: bool) -> HullMarker {
        HullMarker {
            id: TankId(id),
            rect_px: Rect::new(x, 400.0, 120.0, 60.0),
            vehicle: VehicleKind::BENCHMARK,
            seat: 'C',
            hit_points: 640,
            max_hit_points: 1_000,
            distance_m: 214,
            is_target,
        }
    }

    fn build(model: &MarkerModel) -> DrawList<HudElement> {
        let mut list = DrawList::new();
        let mut z = 0;
        push_markers(&mut list, &Ui::reference(), &Theme::standard(), model, &mut z);
        list
    }

    /// H10: the target wears the plate, the class, the name and seat, the bar, the number and
    /// the range; every other spotted hull wears a dimmed bar and nothing else.
    #[test]
    fn only_the_target_wears_a_full_marker() {
        let model = MarkerModel {
            hulls: vec![hull(1, 300.0, false), hull(2, 900.0, true), hull(3, 1_400.0, false)],
        };
        let list = build(&model);
        let has = |part: MarkerPart| list.find(HudElement::Marker(part)).is_some();
        assert!(has(MarkerPart::Plate(1)) && has(MarkerPart::Class(1)) && has(MarkerPart::Name(1)));
        assert!(
            has(MarkerPart::Bar(1)) && has(MarkerPart::Number(1)) && has(MarkerPart::Distance(1))
        );
        for other in [0, 2] {
            assert!(has(MarkerPart::Bar(other)), "a known hull keeps its bar");
            assert!(
                !has(MarkerPart::Plate(other))
                    && !has(MarkerPart::Name(other))
                    && !has(MarkerPart::Number(other))
                    && !has(MarkerPart::Distance(other)),
                "and nothing else"
            );
        }
        match &list.find(HudElement::Marker(MarkerPart::Name(1))).expect("name").payload {
            Payload::Text { text, .. } => assert_eq!(text, "T-54 \u{b7} C"),
            other => panic!("{other:?}"),
        }
        match &list.find(HudElement::Marker(MarkerPart::Distance(1))).expect("range").payload {
            Payload::Text { text, .. } => assert_eq!(text, "214 M"),
            other => panic!("{other:?}"),
        }
        // The plate hangs above the hull's projected box.
        let plate = list.find(HudElement::Marker(MarkerPart::Plate(1))).expect("plate").rect;
        assert!(plate.bottom() <= 400.0 && (plate.center()[0] - 960.0).abs() < 1.0);
    }

    /// The hull under a screen point, nearest first when boxes overlap.
    #[test]
    fn the_hull_under_a_point_is_the_nearest_whose_box_holds_it() {
        let mut near = hull(1, 880.0, false);
        near.distance_m = 120;
        let far = hull(2, 900.0, false);
        let model = MarkerModel { hulls: vec![far, near] };
        assert_eq!(model.hull_under([950.0, 430.0]), Some(TankId(1)));
        assert_eq!(model.hull_under([50.0, 50.0]), None);
    }
}
