use game_core::VehicleKind;

pub fn forge_vehicle_slug(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::PrototypeMedium => "prototype-medium",
        VehicleKind::T54_1951 => "t54-1951",
        VehicleKind::TigerI => "tiger-i-ausf-e",
        VehicleKind::TigerII => "tiger-ii-ausf-b",
        VehicleKind::Jagdtiger => "jagdtiger",
        VehicleKind::PantherII => "panther-ii",
        VehicleKind::IS3 => "is3",
        VehicleKind::Centurion => "centurion-mk3",
        VehicleKind::T34_85 => "t34-85",
        VehicleKind::KV1_1942 => "kv1-1942",
    }
}
