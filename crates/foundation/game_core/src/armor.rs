use serde::{Deserialize, Serialize};

mod facet;
mod impact;
mod resolve;
mod vehicle_volumes;
mod volumes;
mod weakspots;
mod zone;

pub use facet::{ArmorFacet, ArmorFacetProfile};
pub use impact::{TracedImpact, belt_index, resolve_traced_impact, struck_flank};
pub(crate) use resolve::resolve_penetration_at_distance_on_facet;
pub use resolve::{
    PenetrationResult, resolve_penetration, resolve_penetration_at_distance,
    resolve_penetration_through_open_channel, resolve_penetration_through_screens,
    resolve_penetration_through_track,
};
pub use vehicle_volumes::{VehicleArmorVolumes, vehicle_armor_volumes};
pub use volumes::{
    ArmorPatch, ArmorVolume, TaggedPlane, VolumeInterval, segment_volume_entry,
    segment_volume_entry_with_margin, segment_volume_interval_with_margin,
};
pub use weakspots::{WeakspotFrame, WeakspotPoint};
pub use zone::{
    ArmorZone, resolve_penetration_at_distance_on_zone,
    resolve_penetration_at_distance_on_zone_scaled,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ArmorFacing {
    #[default]
    HullFront,
    HullSide,
    HullRear,
    TurretFront,
    TurretSide,
    TurretRear,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorProfile {
    pub hull_front_mm: f32,
    pub hull_side_mm: f32,
    pub hull_rear_mm: f32,
    pub turret_front_mm: f32,
    pub turret_side_mm: f32,
    pub turret_rear_mm: f32,
    /// Turret roof (mm) when the vehicle's dossier states it. `None` falls back to the fleet
    /// formula off the front plate — which is how the T-54 ended up with 24 mm where its
    /// documents say 30.
    #[serde(default)]
    pub turret_roof_mm: Option<f32>,
    /// The lower front plate, when the vehicle's dossier states it. `None` falls back to the
    /// fleet derivation off the glacis (0.72 thickness, 0.45 slope) - which is how the T-54
    /// carried a 27-degree lower plate under a dossier that says 55.
    #[serde(default)]
    pub lower_front: Option<ArmorFacet>,
    #[serde(default)]
    pub facets: ArmorFacetProfile,
}

impl ArmorProfile {
    pub fn new(
        hull_front_mm: f32,
        hull_side_mm: f32,
        hull_rear_mm: f32,
        turret_front_mm: f32,
        turret_side_mm: f32,
        turret_rear_mm: f32,
    ) -> Self {
        Self::new_with_facets(
            ArmorFacet::new(hull_front_mm, 0.0, 1.0),
            ArmorFacet::new(hull_side_mm, 0.0, 1.0),
            ArmorFacet::new(hull_rear_mm, 0.0, 1.0),
            ArmorFacet::new(turret_front_mm, 0.0, 1.0),
            ArmorFacet::new(turret_side_mm, 0.0, 1.0),
            ArmorFacet::new(turret_rear_mm, 0.0, 1.0),
        )
    }

    /// Author the turret roof instead of deriving it (the dossier states it).
    pub fn with_turret_roof_mm(mut self, roof_mm: f32) -> Self {
        self.turret_roof_mm = Some(roof_mm);
        self
    }

    pub fn new_with_facets(
        hull_front: ArmorFacet,
        hull_side: ArmorFacet,
        hull_rear: ArmorFacet,
        turret_front: ArmorFacet,
        turret_side: ArmorFacet,
        turret_rear: ArmorFacet,
    ) -> Self {
        Self {
            hull_front_mm: hull_front.nominal_thickness_mm,
            hull_side_mm: hull_side.nominal_thickness_mm,
            hull_rear_mm: hull_rear.nominal_thickness_mm,
            turret_front_mm: turret_front.nominal_thickness_mm,
            turret_side_mm: turret_side.nominal_thickness_mm,
            lower_front: None,
            turret_rear_mm: turret_rear.nominal_thickness_mm,
            turret_roof_mm: None,
            facets: ArmorFacetProfile {
                hull_front,
                hull_side,
                hull_rear,
                turret_front,
                turret_side,
                turret_rear,
            },
        }
    }

    pub fn nominal_thickness_mm(&self, facing: ArmorFacing) -> f32 {
        match facing {
            ArmorFacing::HullFront => self.hull_front_mm,
            ArmorFacing::HullSide => self.hull_side_mm,
            ArmorFacing::HullRear => self.hull_rear_mm,
            ArmorFacing::TurretFront => self.turret_front_mm,
            ArmorFacing::TurretSide => self.turret_side_mm,
            ArmorFacing::TurretRear => self.turret_rear_mm,
        }
    }

    pub fn facet(&self, facing: ArmorFacing) -> ArmorFacet {
        let facet = match facing {
            ArmorFacing::HullFront => self.facets.hull_front,
            ArmorFacing::HullSide => self.facets.hull_side,
            ArmorFacing::HullRear => self.facets.hull_rear,
            ArmorFacing::TurretFront => self.facets.turret_front,
            ArmorFacing::TurretSide => self.facets.turret_side,
            ArmorFacing::TurretRear => self.facets.turret_rear,
        };
        if facet.nominal_thickness_mm > 0.0 {
            facet
        } else {
            ArmorFacet::new(self.nominal_thickness_mm(facing), 0.0, 1.0)
        }
    }

    pub fn effective_thickness_mm(&self, facing: ArmorFacing, impact_angle_degrees: f32) -> f32 {
        resolve::effective_facet_thickness_mm(self.facet(facing), impact_angle_degrees)
    }
}
