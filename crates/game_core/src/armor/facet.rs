use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorFacet {
    pub nominal_thickness_mm: f32,
    pub slope_degrees: f32,
    pub weakspot_multiplier: f32,
}

impl ArmorFacet {
    pub const fn new(
        nominal_thickness_mm: f32,
        slope_degrees: f32,
        weakspot_multiplier: f32,
    ) -> Self {
        Self { nominal_thickness_mm, slope_degrees, weakspot_multiplier }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorFacetProfile {
    pub hull_front: ArmorFacet,
    pub hull_side: ArmorFacet,
    pub hull_rear: ArmorFacet,
    pub turret_front: ArmorFacet,
    pub turret_side: ArmorFacet,
    pub turret_rear: ArmorFacet,
}

impl Default for ArmorFacetProfile {
    fn default() -> Self {
        let empty = ArmorFacet::new(0.0, 0.0, 1.0);
        Self {
            hull_front: empty,
            hull_side: empty,
            hull_rear: empty,
            turret_front: empty,
            turret_side: empty,
            turret_rear: empty,
        }
    }
}
