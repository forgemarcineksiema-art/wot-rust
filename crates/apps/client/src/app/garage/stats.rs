//! The VEHICLE column's content (interface program G1, G2): every row a labelled number with a
//! bar against the roster — where this hull stands among every playable hull, the class's
//! median ticked — and the derived rows the design owes (GDD §15.4): the effective front at
//! 0°, through the SAME resolver a shell meets; power per tonne; the dispersion on the move,
//! the sim's settled minimum plus the spec's movement bloom, capped where the sim caps it.
//! Separated from the drawing so the CONTENT of the column is locked directly.

use game_core::{ArmorZone, TankSpec, VehicleClass, VehicleKind};

use crate::hud::icons::HudIcon;
use crate::ui_strings::garage as words;

/// The rows, top to bottom. Append-only: the bar tests walk `ALL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StatKind {
    HitPoints,
    Armour,
    EffectiveFront,
    Power,
    PowerPerTonne,
    TopSpeed,
    Traverse,
    Penetration,
    Damage,
    Dispersion,
    DispersionMoving,
    AimTime,
    Reload,
}

impl StatKind {
    pub const ALL: [StatKind; 13] = [
        StatKind::HitPoints,
        StatKind::Armour,
        StatKind::EffectiveFront,
        StatKind::Power,
        StatKind::PowerPerTonne,
        StatKind::TopSpeed,
        StatKind::Traverse,
        StatKind::Penetration,
        StatKind::Damage,
        StatKind::Dispersion,
        StatKind::DispersionMoving,
        StatKind::AimTime,
        StatKind::Reload,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StatKind::HitPoints => words::STAT_HIT_POINTS,
            StatKind::Armour => words::STAT_ARMOUR,
            StatKind::EffectiveFront => words::STAT_EFFECTIVE_FRONT,
            StatKind::Power => words::STAT_POWER,
            StatKind::PowerPerTonne => words::STAT_POWER_PER_TONNE,
            StatKind::TopSpeed => words::STAT_TOP_SPEED,
            StatKind::Traverse => words::STAT_TRAVERSE,
            StatKind::Penetration => words::STAT_PENETRATION,
            StatKind::Damage => words::STAT_DAMAGE,
            StatKind::Dispersion => words::STAT_DISPERSION,
            StatKind::DispersionMoving => words::STAT_DISPERSION_MOVING,
            StatKind::AimTime => words::STAT_AIM_TIME,
            StatKind::Reload => words::STAT_RELOAD,
        }
    }

    pub fn icon(self) -> HudIcon {
        match self {
            StatKind::HitPoints => HudIcon::StatHp,
            StatKind::Armour | StatKind::EffectiveFront => HudIcon::StatArmor,
            StatKind::Power | StatKind::PowerPerTonne => HudIcon::StatPower,
            StatKind::TopSpeed => HudIcon::StatSpeed,
            StatKind::Traverse => HudIcon::StatTraverse,
            StatKind::Penetration | StatKind::Damage => HudIcon::StatPenetration,
            StatKind::Dispersion | StatKind::DispersionMoving => HudIcon::StatDispersion,
            StatKind::AimTime => HudIcon::StatAimTime,
            StatKind::Reload => HudIcon::StatReload,
        }
    }

    /// Whether more is better — the bar's sense (a lower dispersion, reload or aim time fills
    /// further).
    pub fn higher_is_better(self) -> bool {
        !matches!(
            self,
            StatKind::Dispersion
                | StatKind::DispersionMoving
                | StatKind::AimTime
                | StatKind::Reload
        )
    }

    /// The number the row is about, for the bar; the ARMOUR row's bar reads the hull front.
    pub fn measure(self, spec: &TankSpec) -> f32 {
        match self {
            StatKind::HitPoints => spec.hit_points as f32,
            StatKind::Armour => spec.hull.hull_front_mm,
            StatKind::EffectiveFront => effective_front_mm(spec),
            StatKind::Power => spec.engine_power_kw,
            StatKind::PowerPerTonne => power_per_tonne(spec),
            StatKind::TopSpeed => spec.max_forward_speed_mps * 3.6,
            StatKind::Traverse => spec.turret_rotation_rad_s.to_degrees(),
            StatKind::Penetration => spec.gun.shell.penetration_mm_at_100m,
            StatKind::Damage => spec.gun.shell.damage_hp as f32,
            StatKind::Dispersion => spec.gun.dispersion_mrad,
            StatKind::DispersionMoving => dispersion_moving_mrad(spec),
            StatKind::AimTime => spec.gun.aim_time_seconds,
            StatKind::Reload => spec.gun.reload_seconds,
        }
    }

    /// The printed value with its unit.
    pub fn value(self, spec: &TankSpec) -> String {
        match self {
            StatKind::HitPoints => format!("{}", spec.hit_points),
            StatKind::Armour => format!(
                "{} / {} {}",
                spec.hull.hull_front_mm.round() as i32,
                spec.hull.turret_front_mm.round() as i32,
                words::UNIT_MILLIMETERS
            ),
            StatKind::EffectiveFront => {
                format!("{} {}", effective_front_mm(spec).round() as i32, words::UNIT_MILLIMETERS)
            }
            StatKind::Power => {
                format!("{} {}", spec.engine_power_kw.round() as i32, words::UNIT_KILOWATTS)
            }
            StatKind::PowerPerTonne => {
                format!("{:.1} {}", power_per_tonne(spec), words::UNIT_KILOWATTS_PER_TONNE)
            }
            StatKind::TopSpeed => {
                format!("{} {}", (spec.max_forward_speed_mps * 3.6).round() as i32, words::UNIT_KMH)
            }
            StatKind::Traverse => format!(
                "{} {}",
                spec.turret_rotation_rad_s.to_degrees().round() as i32,
                words::UNIT_DEGREES_PER_S
            ),
            StatKind::Penetration => format!(
                "{} {}",
                spec.gun.shell.penetration_mm_at_100m.round() as i32,
                words::UNIT_MILLIMETERS
            ),
            StatKind::Damage => format!("{} {}", spec.gun.shell.damage_hp, words::UNIT_HIT_POINTS),
            StatKind::Dispersion => format!("{:.2} {}", spec.gun.dispersion_mrad, words::UNIT_MRAD),
            StatKind::DispersionMoving => {
                format!("{:.2} {}", dispersion_moving_mrad(spec), words::UNIT_MRAD)
            }
            StatKind::AimTime => {
                format!("{:.1} {}", spec.gun.aim_time_seconds, words::UNIT_SECONDS)
            }
            StatKind::Reload => format!("{:.1} {}", spec.gun.reload_seconds, words::UNIT_SECONDS),
        }
    }
}

/// The upper glacis as a shell meets it, head on at 100 m: the resolver's effective armour
/// against the hull's own stock round (the effective thickness is the plate's, whichever shell
/// asks; the round only decides whether it gets through).
pub(crate) fn effective_front_mm(spec: &TankSpec) -> f32 {
    game_core::resolve_penetration_at_distance_on_zone_scaled(
        &spec.gun.shell,
        &spec.hull,
        ArmorZone::UpperGlacis,
        0.0,
        100.0,
        1.0,
    )
    .effective_armor_mm
}

pub(crate) fn power_per_tonne(spec: &TankSpec) -> f32 {
    spec.engine_power_kw / (spec.mass_kg / 1000.0).max(0.1)
}

/// The dispersion on the move at full speed: the sim's settled minimum for a healthy gun plus
/// the spec's movement bloom, capped at the spec's maximum — the sim's own ceiling.
pub(crate) fn dispersion_moving_mrad(spec: &TankSpec) -> f32 {
    (sim::base_dispersion_mrad(spec, 0.0) + spec.gun.movement_bloom_mrad)
        .min(spec.gun.max_dispersion_mrad.max(spec.gun.dispersion_mrad))
}

/// One row as the column prints it: the label, the value, the bar against the roster and the
/// class's median as a tick — both as fractions of the bar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StatRow {
    pub kind: StatKind,
    pub label: &'static str,
    pub value: String,
    pub frac: f32,
    pub median_frac: f32,
}

/// The roster's span of one stat, over every playable hull's STOCK spec.
fn roster_span(kind: StatKind) -> (f32, f32) {
    let values = VehicleKind::PLAYABLE.iter().map(|k| kind.measure(k.spec_ref()));
    let min = values.clone().fold(f32::MAX, f32::min);
    let max = values.fold(f32::MIN, f32::max);
    (min, max)
}

/// Where `value` sits on the roster's span, in the bar's sense: a lower reload fills further.
fn frac_on_span(kind: StatKind, value: f32, (min, max): (f32, f32)) -> f32 {
    if (max - min).abs() < 1e-6 {
        return 0.5;
    }
    let raw = ((value - min) / (max - min)).clamp(0.0, 1.0);
    if kind.higher_is_better() { raw } else { 1.0 - raw }
}

/// The median of one stat over the hulls of a class.
fn class_median(kind: StatKind, class: VehicleClass) -> f32 {
    let mut values: Vec<f32> = VehicleKind::PLAYABLE
        .iter()
        .filter(|k| k.class() == class)
        .map(|k| kind.measure(k.spec_ref()))
        .collect();
    values.sort_by(f32::total_cmp);
    match values.len() {
        0 => 0.0,
        n if n % 2 == 1 => values[n / 2],
        n => (values[n / 2 - 1] + values[n / 2]) * 0.5,
    }
}

/// The column for an assembled spec.
pub(crate) fn stat_rows(spec: &TankSpec) -> Vec<StatRow> {
    StatKind::ALL
        .iter()
        .map(|&kind| {
            let span = roster_span(kind);
            StatRow {
                kind,
                label: kind.label(),
                value: kind.value(spec),
                frac: frac_on_span(kind, kind.measure(spec), span),
                median_frac: frac_on_span(kind, class_median(kind, spec.kind.class()), span),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G1: every row prints its own label, a number with its unit, and a bar against the
    /// roster — the roster's best fills it, its worst empties it — with the class's median
    /// ticked; no two rows share a label.
    #[test]
    fn every_stat_row_prints_its_own_label_a_number_and_a_bar_against_the_roster() {
        let rows = stat_rows(&VehicleKind::BENCHMARK.spec());
        assert_eq!(rows.len(), StatKind::ALL.len());
        let mut labels = std::collections::HashSet::new();
        for row in &rows {
            assert!(!row.label.is_empty());
            assert!(labels.insert(row.label), "{} twice", row.label);
            assert!(row.value.chars().any(|c| c.is_ascii_digit()), "{}: {}", row.label, row.value);
            assert!((0.0..=1.0).contains(&row.frac), "{}: {}", row.label, row.frac);
            assert!((0.0..=1.0).contains(&row.median_frac));
        }
        for kind in StatKind::ALL {
            let specs: Vec<TankSpec> = VehicleKind::PLAYABLE.iter().map(|k| k.spec()).collect();
            let fracs: Vec<f32> = specs.iter().map(|s| stat_rows(s)[kind as usize].frac).collect();
            assert!(
                fracs.iter().any(|f| (*f - 1.0).abs() < 1e-5),
                "{kind:?}: someone fills the bar"
            );
            assert!(fracs.iter().any(|f| f.abs() < 1e-5), "{kind:?}: someone empties it");
        }
        // The sense: a shorter reload fills further than a longer one.
        let mut quick = VehicleKind::BENCHMARK.spec();
        quick.gun.reload_seconds = 1.0;
        let mut slow = VehicleKind::BENCHMARK.spec();
        slow.gun.reload_seconds = 60.0;
        assert!(
            stat_rows(&quick)[StatKind::Reload as usize].frac
                > stat_rows(&slow)[StatKind::Reload as usize].frac
        );
    }

    /// G2: the derived rows agree with the resolver and the sim to 1e-3 — the effective front
    /// is what the resolver hands a shell at 0° on the upper glacis, power per tonne is the
    /// engine over the mass, the moving dispersion is the sim's settled minimum plus the
    /// movement bloom under the spec's cap.
    #[test]
    fn the_derived_rows_agree_with_the_resolver_and_the_sim_to_1e_3() {
        for kind in VehicleKind::PLAYABLE {
            let spec = kind.spec();
            let resolved = game_core::resolve_penetration_at_distance_on_zone_scaled(
                &spec.gun.shell,
                &spec.hull,
                ArmorZone::UpperGlacis,
                0.0,
                100.0,
                1.0,
            )
            .effective_armor_mm;
            assert!((effective_front_mm(&spec) - resolved).abs() < 1e-3, "{kind:?}");
            assert!(
                effective_front_mm(&spec) >= spec.hull.hull_front_mm - 1e-3,
                "{kind:?}: a slope never thins a plate"
            );
            let ppt = spec.engine_power_kw / (spec.mass_kg / 1000.0);
            assert!((power_per_tonne(&spec) - ppt).abs() < 1e-3, "{kind:?}");
            let moving = (sim::base_dispersion_mrad(&spec, 0.0) + spec.gun.movement_bloom_mrad)
                .min(spec.gun.max_dispersion_mrad.max(spec.gun.dispersion_mrad));
            assert!((dispersion_moving_mrad(&spec) - moving).abs() < 1e-3, "{kind:?}");
            assert!(
                dispersion_moving_mrad(&spec) >= spec.gun.dispersion_mrad,
                "{kind:?}: moving never groups tighter than still"
            );
        }
    }

    /// The rows follow the assembled spec: a gun that groups differently changes the column.
    #[test]
    fn the_column_follows_the_fitted_gun() {
        let mut spec = VehicleKind::BENCHMARK.spec();
        let stock = stat_rows(&spec);
        spec.gun.dispersion_mrad += 0.10;
        let fitted = stat_rows(&spec);
        assert_ne!(
            stock[StatKind::Dispersion as usize].value,
            fitted[StatKind::Dispersion as usize].value
        );
    }
}
