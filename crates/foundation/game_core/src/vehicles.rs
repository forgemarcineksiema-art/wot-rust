use crate::{TankSpec, VehicleKind};

// Each known vehicle is assembled from its stock module loadout (see `crate::modules`), so
// these constructors are thin delegators to `VehicleKind::spec`.
impl TankSpec {
    pub fn t54_1951() -> Self {
        VehicleKind::T54_1951.spec()
    }

    pub fn t55a() -> Self {
        VehicleKind::T55A.spec()
    }

    pub fn tiger_i_ausf_e() -> Self {
        VehicleKind::TigerI.spec()
    }

    pub fn tiger_ii_ausf_b() -> Self {
        VehicleKind::TigerII.spec()
    }

    pub fn jagdtiger() -> Self {
        VehicleKind::Jagdtiger.spec()
    }

    pub fn panther_ii() -> Self {
        VehicleKind::PantherII.spec()
    }
}

pub fn known_tank_specs() -> Vec<TankSpec> {
    VehicleKind::ALL.iter().map(|kind| kind.spec()).collect()
}
