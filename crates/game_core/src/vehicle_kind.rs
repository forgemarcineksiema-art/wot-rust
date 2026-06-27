use serde::{Deserialize, Serialize};

use crate::TankSpec;

/// The historical origin nation of a vehicle. Used by the garage carousel and tech tree to group
/// the roster; renderer-neutral (no wgpu dependency lives here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nation {
    Ussr,
    Germany,
}

impl Nation {
    pub fn label(self) -> &'static str {
        match self {
            Nation::Ussr => "USSR",
            Nation::Germany => "Germany",
        }
    }

    /// Display colour for nation labels and column headers in the garage UI.
    pub fn color(self) -> [f32; 3] {
        match self {
            Nation::Ussr => [0.58, 0.64, 0.40],
            Nation::Germany => [0.38, 0.42, 0.48],
        }
    }
}

/// Stable, semantic vehicle identity shared across simulation, networking, and rendering.
///
/// **The variant order is the wire identity**: `net` serializes this enum's discriminant via
/// bincode, so new vehicles must be *appended* — never reordered or removed — or existing
/// snapshots would decode to the wrong vehicle.
///
/// Variants intentionally mirror the snake-case `TankSpec` constructors and asset slugs
/// (`t54_1951`, `t55a`, ...), so `non_camel_case_types` is allowed for this one type.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VehicleKind {
    #[default]
    PrototypeMedium,
    T54_1951,
    T55A,
    TigerI,
    TigerII,
    Jagdtiger,
    PantherII,
}

impl VehicleKind {
    /// Every known vehicle, in declaration (wire) order.
    pub const ALL: [VehicleKind; 7] = [
        VehicleKind::PrototypeMedium,
        VehicleKind::T54_1951,
        VehicleKind::T55A,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
    ];

    /// Player-facing production roster. Legacy/test-only vehicles remain in [`Self::ALL`] for
    /// stable wire identity, but garage and review surfaces should use this list.
    pub const PLAYABLE: [VehicleKind; 5] = [
        VehicleKind::T54_1951,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
    ];

    /// Asset slug stem; matches `assets/vehicles/<slug>.vehicle.json` for the six vehicles
    /// that ship an asset file (the prototype medium is test-only and has none).
    pub fn slug(self) -> &'static str {
        match self {
            VehicleKind::PrototypeMedium => "prototype_medium",
            VehicleKind::T54_1951 => "t54_1951",
            VehicleKind::T55A => "t55a",
            VehicleKind::TigerI => "tiger_i_ausf_e",
            VehicleKind::TigerII => "tiger_ii_ausf_b",
            VehicleKind::Jagdtiger => "jagdtiger",
            VehicleKind::PantherII => "panther_ii",
        }
    }

    /// Canonical display name (the historical designation).
    pub fn display_name(self) -> &'static str {
        match self {
            VehicleKind::PrototypeMedium => "Prototype Medium",
            VehicleKind::T54_1951 => "T-54-3 obr. 1951",
            VehicleKind::T55A => "T-55A",
            VehicleKind::TigerI => "Panzerkampfwagen VI Tiger Ausf. E",
            VehicleKind::TigerII => "Panzerkampfwagen VI B Tiger II",
            VehicleKind::Jagdtiger => "Panzerjager Tiger Ausf. B Jagdtiger",
            VehicleKind::PantherII => "Panzerkampfwagen V Panther II",
        }
    }

    /// The canonical [`TankSpec`] for this vehicle, assembled from its stock module loadout.
    pub fn spec(self) -> TankSpec {
        self.default_loadout().assemble(self)
    }

    pub fn has_fixed_casemate(self) -> bool {
        self.spec().has_fixed_casemate()
    }

    /// The historical origin nation of this vehicle — used by the garage carousel and tech tree.
    pub fn nation(self) -> Nation {
        match self {
            VehicleKind::PrototypeMedium | VehicleKind::T54_1951 | VehicleKind::T55A => {
                Nation::Ussr
            }
            VehicleKind::TigerI
            | VehicleKind::TigerII
            | VehicleKind::Jagdtiger
            | VehicleKind::PantherII => Nation::Germany,
        }
    }

    pub fn effective_turret_yaw_rad(self, turret_yaw_rad: f32) -> f32 {
        self.spec().effective_turret_yaw_rad(turret_yaw_rad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_complete_and_unique() {
        assert_eq!(VehicleKind::ALL.len(), 7);
        for (index, kind) in VehicleKind::ALL.iter().enumerate() {
            for other in &VehicleKind::ALL[index + 1..] {
                assert_ne!(kind, other, "VehicleKind::ALL must not contain duplicates");
            }
        }
    }

    #[test]
    fn playable_roster_exposes_production_vehicles_without_t55a_clone() {
        assert_eq!(
            VehicleKind::PLAYABLE,
            [
                VehicleKind::T54_1951,
                VehicleKind::TigerI,
                VehicleKind::TigerII,
                VehicleKind::Jagdtiger,
                VehicleKind::PantherII,
            ]
        );
        assert!(!VehicleKind::PLAYABLE.contains(&VehicleKind::PrototypeMedium));
        assert!(!VehicleKind::PLAYABLE.contains(&VehicleKind::T55A));
    }

    #[test]
    fn each_spec_round_trips_to_its_kind() {
        for kind in VehicleKind::ALL {
            assert_eq!(kind.spec().kind, kind, "spec for {kind:?} carries the wrong kind");
        }
    }

    #[test]
    fn default_is_prototype_medium() {
        assert_eq!(VehicleKind::default(), VehicleKind::PrototypeMedium);
    }

    #[test]
    fn slugs_are_unique_and_non_empty() {
        for (index, kind) in VehicleKind::ALL.iter().enumerate() {
            assert!(!kind.slug().is_empty());
            for other in &VehicleKind::ALL[index + 1..] {
                assert_ne!(kind.slug(), other.slug(), "slugs must be unique");
            }
        }
    }

    #[test]
    fn fixed_casemate_rule_is_shared_by_vehicle_kind_and_tank_spec() {
        for kind in VehicleKind::ALL {
            let spec = kind.spec();
            assert_eq!(kind.has_fixed_casemate(), spec.has_fixed_casemate());
            assert_eq!(
                spec.effective_turret_yaw_rad(0.75),
                if kind == VehicleKind::Jagdtiger { 0.0 } else { 0.75 }
            );
        }
    }

    #[test]
    fn each_vehicle_kind_has_a_nation() {
        for kind in VehicleKind::ALL {
            let _ = kind.nation();
        }
    }

    #[test]
    fn nation_matches_historical_origin() {
        assert_eq!(VehicleKind::PrototypeMedium.nation(), Nation::Ussr);
        assert_eq!(VehicleKind::T54_1951.nation(), Nation::Ussr);
        assert_eq!(VehicleKind::T55A.nation(), Nation::Ussr);
        assert_eq!(VehicleKind::TigerI.nation(), Nation::Germany);
        assert_eq!(VehicleKind::TigerII.nation(), Nation::Germany);
        assert_eq!(VehicleKind::Jagdtiger.nation(), Nation::Germany);
        assert_eq!(VehicleKind::PantherII.nation(), Nation::Germany);
    }

    #[test]
    fn nation_labels_and_colors_are_distinct() {
        let labels = [Nation::Ussr.label(), Nation::Germany.label()];
        assert_ne!(labels[0], labels[1]);
        assert!(!labels.iter().any(|label| label.is_empty()));

        let colors = [Nation::Ussr.color(), Nation::Germany.color()];
        assert_ne!(colors[0], colors[1]);
    }

    #[test]
    fn playable_roster_spans_both_nations() {
        let has_ussr = VehicleKind::PLAYABLE.iter().any(|kind| kind.nation() == Nation::Ussr);
        let has_germany = VehicleKind::PLAYABLE.iter().any(|kind| kind.nation() == Nation::Germany);
        assert!(has_ussr && has_germany, "playable roster must include both nations");
    }
}
