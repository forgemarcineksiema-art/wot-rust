use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::TankSpec;

/// The historical origin nation of a vehicle. Used by the garage carousel and tech tree to group
/// the roster; renderer-neutral (no wgpu dependency lives here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nation {
    Ussr,
    Germany,
    Britain,
}

impl Nation {
    /// Every nation the roster can open. The tech tree is built by walking this.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [Nation; 3] = [Nation::Ussr, Nation::Germany, Nation::Britain];

    pub fn label(self) -> &'static str {
        match self {
            Nation::Ussr => "USSR",
            Nation::Germany => "Germany",
            Nation::Britain => "Britain",
        }
    }

    /// Display colour for nation labels and column headers in the garage UI.
    pub fn color(self) -> [f32; 3] {
        match self {
            Nation::Ussr => [0.58, 0.64, 0.40],
            Nation::Germany => [0.38, 0.42, 0.48],
            Nation::Britain => [0.52, 0.46, 0.34],
        }
    }
}

/// The game's three technology eras — the matchmaking bracket, replacing tier spread entirely:
/// a battle is one era, never a mix. Era is derived from [`VehicleKind`], never serialized,
/// so appending eras (and vehicles) later stays wire-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Era {
    /// Early-war designs (1939-42). No playable vehicles yet — reserved by the roadmap.
    EarlyWar,
    /// Late-war heavy metal (1943-45): the Tiger/Panther family.
    LateWar,
    /// Early Cold War (1946-55): T-54, IS-3 and their contemporaries.
    ColdWar,
}

impl Era {
    /// Every era, oldest first — the tech tree's primary axis.
    pub const ALL: [Era; 3] = [Era::EarlyWar, Era::LateWar, Era::ColdWar];

    pub fn label(self) -> &'static str {
        match self {
            Era::EarlyWar => "Era I",
            Era::LateWar => "Era II",
            Era::ColdWar => "Era III",
        }
    }

    /// The era's descriptive name + years, for surfaces that explain the bracket (the tech
    /// tree's era bands) rather than just tagging it.
    pub fn display_name(self) -> &'static str {
        match self {
            Era::EarlyWar => "EARLY WAR 1939-42",
            Era::LateWar => "LATE WAR 1943-45",
            Era::ColdWar => "COLD WAR 1946-55",
        }
    }

    /// The production roster restricted to this era — the pool a random battle draws from.
    pub fn playable(self) -> impl Iterator<Item = VehicleKind> {
        VehicleKind::PLAYABLE.into_iter().filter(move |kind| kind.era() == self)
    }
}

/// Stable, semantic vehicle identity shared across simulation, networking, and rendering.
///
/// **The variant order is the wire identity**: `net` serializes this enum's discriminant via
/// bincode, so new vehicles must be *appended* — never reordered or removed — or existing
/// snapshots would decode to the wrong vehicle.
///
/// Variants intentionally mirror the snake-case `TankSpec` constructors and asset slugs
/// (`t54_1951`, `tiger_i_ausf_e`, ...), so `non_camel_case_types` is allowed for this one type.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VehicleKind {
    #[default]
    PrototypeMedium,
    T54_1951,
    TigerI,
    TigerII,
    Jagdtiger,
    PantherII,
    IS3,
    Centurion,
    T34_85,
}

impl VehicleKind {
    /// The fleet's BENCHMARK vehicle — the one with the richest authored content, which budget
    /// baselines, authoring harnesses and contact-lock fixtures measure against.
    ///
    /// Declared HERE, in the data crate, so no app-layer file ever names a specific vehicle to
    /// get a reference hull (the W4 dispatch rule forbids exactly that). When another vehicle
    /// overtakes the T-54's fidelity, the whole fleet's instruments retarget on this one line.
    pub const BENCHMARK: VehicleKind = VehicleKind::T54_1951;

    /// Every known vehicle, in declaration (wire) order.
    pub const ALL: [VehicleKind; 9] = [
        VehicleKind::PrototypeMedium,
        VehicleKind::T54_1951,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
        VehicleKind::IS3,
        VehicleKind::Centurion,
        VehicleKind::T34_85,
    ];

    /// Player-facing production roster. Legacy/test-only vehicles remain in [`Self::ALL`] for
    /// stable wire identity, but garage and review surfaces should use this list.
    pub const PLAYABLE: [VehicleKind; 8] = [
        VehicleKind::T54_1951,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
        VehicleKind::IS3,
        VehicleKind::Centurion,
        VehicleKind::T34_85,
    ];

    /// Asset slug stem; matches `assets/vehicles/<slug>.vehicle.json` for every playable
    /// vehicle (the test-only prototype medium ships no asset file).
    pub fn slug(self) -> &'static str {
        match self {
            VehicleKind::PrototypeMedium => "prototype_medium",
            VehicleKind::T54_1951 => "t54_1951",
            VehicleKind::TigerI => "tiger_i_ausf_e",
            VehicleKind::TigerII => "tiger_ii_ausf_b",
            VehicleKind::Jagdtiger => "jagdtiger",
            VehicleKind::PantherII => "panther_ii",
            VehicleKind::IS3 => "is3",
            VehicleKind::Centurion => "centurion_mk3",
            VehicleKind::T34_85 => "t34_85",
        }
    }

    /// The vehicle whose [`slug`](Self::slug) matches, or `None` for an unknown slug — e.g. a save
    /// written by a build that has since renamed or removed that vehicle. The inverse of `slug`, used
    /// by persistence so a stale entry degrades gracefully instead of failing a whole parse.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == slug)
    }

    /// Canonical display name (the historical designation).
    pub fn display_name(self) -> &'static str {
        match self {
            VehicleKind::PrototypeMedium => "Prototype Medium",
            VehicleKind::T54_1951 => "T-54-3 obr. 1951",
            VehicleKind::TigerI => "Panzerkampfwagen VI Tiger Ausf. E",
            VehicleKind::TigerII => "Panzerkampfwagen VI B Tiger II",
            VehicleKind::Jagdtiger => "Panzerjager Tiger Ausf. B Jagdtiger",
            VehicleKind::PantherII => "Panzerkampfwagen V Panther II",
            VehicleKind::IS3 => "IS-3",
            VehicleKind::Centurion => "Centurion Mk 3",
            VehicleKind::T34_85 => "T-34-85",
        }
    }

    /// Short display name — compact enough for carousel cells and damage-log rows. Entity
    /// identity is DATA: this table lived in the client (`vehicle/display.rs`) as the last
    /// hand-rolled `VehicleKind` match in the app layer, which is exactly the asymmetry the
    /// dispatch rule exists to burn (W4 F4). It lives here beside [`Self::display_name`], so a
    /// new vehicle states both names in one file or fails the exhaustive match.
    pub fn short_name(self) -> &'static str {
        match self {
            VehicleKind::PrototypeMedium => "Proto",
            VehicleKind::T54_1951 => "T-54",
            VehicleKind::TigerI => "Tiger I",
            VehicleKind::TigerII => "Tiger II",
            VehicleKind::Jagdtiger => "Jagdtg",
            VehicleKind::PantherII => "Panth II",
            VehicleKind::IS3 => "IS-3",
            VehicleKind::Centurion => "Cent 3",
            VehicleKind::T34_85 => "T-34-85",
        }
    }

    /// The canonical [`TankSpec`] for this vehicle, assembled from its stock module loadout.
    ///
    /// This is a fresh, owned spec every call. Reading ONE field off it is what
    /// [`Self::spec_ref`] is for.
    pub fn spec(self) -> TankSpec {
        self.spec_ref().clone()
    }

    /// The canonical stock [`TankSpec`], assembled once per process.
    ///
    /// Assembling a spec is not cheap: it clones the gun (a `String`), formats the display name
    /// into another, and builds the armour profile, module health, hitbox, damage layout, mount
    /// frames and contact footprint. Hot paths were paying all of it to read a single number —
    /// the HUD's enemy health bars once per tank per FRAME, and the server's distant-HP
    /// quantiser once per tank per VIEWER per snapshot, which on a full 7v7 at 20 Hz is roughly
    /// four thousand assemblies a second for one `u32`.
    ///
    /// Callers that only read take this; callers that own or mutate take [`Self::spec`].
    pub fn spec_ref(self) -> &'static TankSpec {
        static CACHE: OnceLock<[TankSpec; VehicleKind::ALL.len()]> = OnceLock::new();
        let cache = CACHE.get_or_init(|| VehicleKind::ALL.map(|kind| kind.assemble_spec()));
        let index = VehicleKind::ALL.iter().position(|&kind| kind == self).expect("kind is in ALL");
        &cache[index]
    }

    /// The uncached assembly — the one place the cache above is filled from.
    fn assemble_spec(self) -> TankSpec {
        self.default_loadout().assemble(self)
    }

    pub fn has_fixed_casemate(self) -> bool {
        self.spec_ref().has_fixed_casemate()
    }

    /// The technology era this vehicle fights in — the matchmaking bracket. The test-only
    /// prototype medium is a T-55-shaped stand-in, so it sits with the Cold War park.
    pub fn era(self) -> Era {
        match self {
            VehicleKind::T34_85
            | VehicleKind::TigerI
            | VehicleKind::TigerII
            | VehicleKind::Jagdtiger
            | VehicleKind::PantherII => Era::LateWar,
            VehicleKind::PrototypeMedium
            | VehicleKind::T54_1951
            | VehicleKind::IS3
            | VehicleKind::Centurion => Era::ColdWar,
        }
    }

    /// The historical origin nation of this vehicle — used by the garage carousel and tech tree.
    pub fn nation(self) -> Nation {
        match self {
            VehicleKind::PrototypeMedium
            | VehicleKind::T34_85
            | VehicleKind::T54_1951
            | VehicleKind::IS3 => Nation::Ussr,
            VehicleKind::TigerI
            | VehicleKind::TigerII
            | VehicleKind::Jagdtiger
            | VehicleKind::PantherII => Nation::Germany,
            VehicleKind::Centurion => Nation::Britain,
        }
    }

    pub fn effective_turret_yaw_rad(self, turret_yaw_rad: f32) -> f32 {
        self.spec_ref().effective_turret_yaw_rad(turret_yaw_rad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_complete_and_unique() {
        assert_eq!(VehicleKind::ALL.len(), 9);
        for (index, kind) in VehicleKind::ALL.iter().enumerate() {
            for other in &VehicleKind::ALL[index + 1..] {
                assert_ne!(kind, other, "VehicleKind::ALL must not contain duplicates");
            }
        }
    }

    #[test]
    fn playable_roster_exposes_every_production_vehicle() {
        // The roster rule: no clones. Every non-test vehicle is playable; the only variant
        // outside this list is the test-only prototype medium.
        assert_eq!(
            VehicleKind::PLAYABLE,
            [
                VehicleKind::T54_1951,
                VehicleKind::TigerI,
                VehicleKind::TigerII,
                VehicleKind::Jagdtiger,
                VehicleKind::PantherII,
                VehicleKind::IS3,
                VehicleKind::Centurion,
                VehicleKind::T34_85,
            ]
        );
        assert!(!VehicleKind::PLAYABLE.contains(&VehicleKind::PrototypeMedium));
        assert_eq!(VehicleKind::ALL.len(), VehicleKind::PLAYABLE.len() + 1);
    }

    /// The cache must be the SAME spec the uncached assembly produces — a stale or mis-indexed
    /// entry would hand a vehicle another's armour, hitbox and gun, silently, everywhere.
    #[test]
    fn the_cached_spec_is_the_assembled_spec_for_every_kind() {
        for kind in VehicleKind::ALL {
            assert_eq!(
                *kind.spec_ref(),
                kind.assemble_spec(),
                "{kind:?}: the cached spec drifted from its assembly"
            );
            assert_eq!(kind.spec(), kind.assemble_spec(), "{kind:?}: the owned spec drifted");
            assert_eq!(kind.spec_ref().kind, kind, "{kind:?}: the cache is mis-indexed");
        }
        // Every kind gets its own slot: repeated lookups are stable and never alias.
        for (index, kind) in VehicleKind::ALL.iter().enumerate() {
            assert!(std::ptr::eq(kind.spec_ref(), kind.spec_ref()), "lookups must be stable");
            for other in &VehicleKind::ALL[index + 1..] {
                assert!(
                    !std::ptr::eq(kind.spec_ref(), other.spec_ref()),
                    "{kind:?} and {other:?} must not share a cache slot"
                );
            }
        }
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
    fn from_slug_inverts_slug_and_rejects_unknown() {
        for kind in VehicleKind::ALL {
            assert_eq!(VehicleKind::from_slug(kind.slug()), Some(kind), "slug round-trips");
        }
        assert_eq!(VehicleKind::from_slug("ghost_tank_9000"), None, "unknown slug is None");
        assert_eq!(VehicleKind::from_slug(""), None, "empty slug is None");
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
    fn eras_split_the_park_along_the_historical_generation_gap() {
        // The whole Tiger/Panther family fights in Era II; the postwar Soviet park in Era III.
        for kind in [
            VehicleKind::TigerI,
            VehicleKind::TigerII,
            VehicleKind::Jagdtiger,
            VehicleKind::PantherII,
        ] {
            assert_eq!(kind.era(), Era::LateWar, "{kind:?}");
        }
        for kind in [VehicleKind::T54_1951, VehicleKind::IS3, VehicleKind::Centurion] {
            assert_eq!(kind.era(), Era::ColdWar, "{kind:?}");
        }
    }

    #[test]
    fn every_populated_era_fields_at_least_two_playable_vehicles() {
        // A one-vehicle era would put clone armies on both teams AND leave the enemy roster
        // with zero variety; the roadmap fills Era I before any vehicle claims it.
        for era in [Era::EarlyWar, Era::LateWar, Era::ColdWar] {
            let pool = era.playable().count();
            assert!(pool == 0 || pool >= 2, "{era:?} fields {pool} vehicle(s)");
        }
        assert!(Era::LateWar.playable().count() >= 2);
        assert!(Era::ColdWar.playable().count() >= 2);
    }

    #[test]
    fn era_labels_read_as_the_three_brackets() {
        assert_eq!(Era::EarlyWar.label(), "Era I");
        assert_eq!(Era::LateWar.label(), "Era II");
        assert_eq!(Era::ColdWar.label(), "Era III");
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
        assert_eq!(VehicleKind::TigerI.nation(), Nation::Germany);
        assert_eq!(VehicleKind::TigerII.nation(), Nation::Germany);
        assert_eq!(VehicleKind::Jagdtiger.nation(), Nation::Germany);
        assert_eq!(VehicleKind::PantherII.nation(), Nation::Germany);
        assert_eq!(VehicleKind::IS3.nation(), Nation::Ussr);
        assert_eq!(VehicleKind::Centurion.nation(), Nation::Britain);
    }

    #[test]
    fn nation_labels_and_colors_are_distinct() {
        let labels = [Nation::Ussr.label(), Nation::Germany.label(), Nation::Britain.label()];
        for (index, label) in labels.iter().enumerate() {
            assert!(!label.is_empty());
            for other in &labels[index + 1..] {
                assert_ne!(label, other);
            }
        }

        let colors = [Nation::Ussr.color(), Nation::Germany.color(), Nation::Britain.color()];
        for (index, color) in colors.iter().enumerate() {
            for other in &colors[index + 1..] {
                assert_ne!(color, other);
            }
        }
    }

    #[test]
    fn playable_roster_spans_all_nations() {
        for nation in [Nation::Ussr, Nation::Germany, Nation::Britain] {
            assert!(
                VehicleKind::PLAYABLE.iter().any(|kind| kind.nation() == nation),
                "playable roster must include {nation:?}"
            );
        }
    }

    /// W4 F4: the short name's contract is COMPACT — it fits a carousel cell and a damage-log
    /// row. Eight ASCII characters is the widest shipped name ("Tiger II", "Panth II"); a new
    /// vehicle that needs more needs an abbreviation, not a wider cell.
    #[test]
    fn short_names_are_short_ascii_and_distinct() {
        let mut seen = std::collections::HashSet::new();
        for kind in VehicleKind::ALL {
            let name = kind.short_name();
            assert!(!name.is_empty(), "{kind:?} has a short name");
            assert!(name.len() <= 8, "{kind:?}: '{name}' exceeds the 8-char carousel budget");
            assert!(name.is_ascii(), "{kind:?}: the font atlas bakes ASCII only");
            assert!(seen.insert(name), "{kind:?}: '{name}' collides with another vehicle");
        }
    }
}
