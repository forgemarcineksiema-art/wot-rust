//! Short display names for vehicles — compact enough for carousel cells and damage-log rows.
//! Entity identity lives data-side by policy (`ui_strings` header); this is the one shared map
//! from `VehicleKind` to its abbreviated label.

use game_core::VehicleKind;

pub(crate) fn short_name(kind: VehicleKind) -> &'static str {
    match kind {
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
