//! Aimable weakspots enumerated FROM the baked armor volumes — the same geometry a shell
//! resolves against, read in the other direction. A gunner who cannot beat a front plate aims
//! at what he can see on it: the bow ports, the mantlet, the commander's drum. This module
//! answers "where are those, and how big" without duplicating a single bake number: every
//! point here is derived from [`VehicleArmorVolumes`], so an edit to a blueprint moves the
//! aim points exactly as far as it moves the steel.

use glam::Vec3;

use super::ArmorZone;
use super::vehicle_volumes::VehicleArmorVolumes;
use super::volumes::ArmorVolume;

/// Which rotating frame a weakspot's local coordinates live in. Hull points ride the hull pose
/// alone; turret points additionally traverse about the ring pivot `(0, 0, turret_ring_z)`,
/// exactly like the shell trace's turret segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakspotFrame {
    Hull,
    Turret,
}

/// One aimable weakspot: a disc a gunner can hold the reticle on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeakspotPoint {
    pub zone: ArmorZone,
    pub frame: WeakspotFrame,
    /// Centre in the shell-trace local frame (hitbox-centred, hull scale).
    pub center: Vec3,
    /// Radius of the disc worth holding on — the patch's own radius, or the drum's presented
    /// half-extent.
    pub radius_m: f32,
    /// Frame-local outward presentation, for culling far-side points. `None` presents
    /// all-around (the cupola drum offers a face from every bearing).
    pub outward: Option<Vec3>,
}

impl VehicleArmorVolumes {
    /// Every aimable weakspot on this vehicle: the patches riding its hull and turret plates,
    /// plus the cupola drum. Order is stable (hull patches, then turret, then cupola) so
    /// deterministic consumers stay deterministic.
    pub fn weakspot_aim_points(&self) -> Vec<WeakspotPoint> {
        let mut points: Vec<WeakspotPoint> = Vec::new();
        // A pike bow clones its ports onto both swept glacis planes; identical clones collapse
        // to one aim point.
        for volume in &self.hull {
            for plane in &volume.planes {
                for patch in &plane.patches {
                    let point = WeakspotPoint {
                        zone: patch.zone,
                        frame: WeakspotFrame::Hull,
                        center: patch.center,
                        radius_m: patch.radius_m,
                        outward: Some(patch.presents_normal.unwrap_or(plane.normal)),
                    };
                    if !points.contains(&point) {
                        points.push(point);
                    }
                }
            }
        }
        // A dome carries its mantlet patch on every front sector, each with the patch centre
        // projected onto that sector's plane. One aim point per zone suffices: the sector most
        // squarely facing forward carries the centre a frontal gunner actually sees.
        let mut turret: Vec<(f32, WeakspotPoint)> = Vec::new();
        for plane in &self.turret.planes {
            for patch in &plane.patches {
                let point = WeakspotPoint {
                    zone: patch.zone,
                    frame: WeakspotFrame::Turret,
                    center: patch.center,
                    radius_m: patch.radius_m,
                    outward: Some(patch.presents_normal.unwrap_or(plane.normal)),
                };
                match turret.iter_mut().find(|(_, existing)| existing.zone == patch.zone) {
                    Some((forwardness, existing)) if plane.normal.z > *forwardness => {
                        *forwardness = plane.normal.z;
                        *existing = point;
                    }
                    Some(_) => {}
                    None => turret.push((plane.normal.z, point)),
                }
            }
        }
        points.extend(turret.into_iter().map(|(_, point)| point));
        if let Some(drum) = drum_aim_point(&self.cupola) {
            points.push(drum);
        }
        points
    }
}

/// The drum reduced to an aim disc. The cupola is a convex volume, not a patch: its centre and
/// presented size are recovered from its own planes — the lid and floor bound the height, the
/// wall ring bounds the footprint — so the aim point IS the volume, not a second authored copy.
fn drum_aim_point(cupola: &ArmorVolume) -> Option<WeakspotPoint> {
    let mut top_y = f32::NEG_INFINITY;
    let mut bottom_y = f32::NEG_INFINITY;
    let mut walls: Vec<(Vec3, f32)> = Vec::new();
    for plane in &cupola.planes {
        if plane.normal.y > 0.99 {
            top_y = plane.offset;
        } else if plane.normal.y < -0.99 {
            bottom_y = plane.offset; // normal -Y: offset = -root_y
        } else if plane.normal.y.abs() < 1.0e-3 {
            walls.push((plane.normal, plane.offset));
        }
    }
    if !top_y.is_finite() || !bottom_y.is_finite() || walls.len() < 3 {
        return None;
    }
    let root_y = -bottom_y;
    // A uniform ring of unit normals n_i with offsets d_i = n_i·c + r satisfies
    // Σ d_i·n_i = (Σ n_i n_iᵀ)·c = (N/2)·c in the horizontal plane, because the ring's
    // normals average to zero and their outer products average to half the identity.
    let count = walls.len() as f32;
    let summed = walls.iter().fold(Vec3::ZERO, |sum, (normal, offset)| sum + *normal * *offset);
    let center_xz = summed * (2.0 / count);
    let radius =
        walls.iter().map(|(normal, offset)| offset - normal.dot(center_xz)).sum::<f32>() / count;
    let half_height = (top_y - root_y) * 0.5;
    if radius <= 0.0 || half_height <= 0.0 {
        return None;
    }
    Some(WeakspotPoint {
        zone: ArmorZone::Cupola,
        frame: WeakspotFrame::Turret,
        center: Vec3::new(center_xz.x, root_y + half_height, center_xz.z),
        // The presented face is bounded by the SMALLER of footprint and height: a wide shallow
        // drum offers its height, a tall narrow one its width.
        radius_m: radius.min(half_height),
        outward: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VehicleBlueprint, VehicleKind, vehicle_armor_volumes};

    #[test]
    fn the_t54_enumerates_its_port_its_mantlet_and_its_drum() {
        let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("baked");
        let points = volumes.weakspot_aim_points();
        let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let cy = blueprint.hull.hitbox_center_y;

        let port = points
            .iter()
            .find(|point| point.zone == ArmorZone::GlacisPort)
            .expect("the bow MG port is aimable");
        let authored = blueprint.armor.glacis_ports[0].expect("T-54 authors its MG port");
        assert_eq!(port.frame, WeakspotFrame::Hull);
        assert!((port.center.x - authored.x).abs() < 1.0e-6);
        assert!((port.center.y - (authored.y - cy)).abs() < 1.0e-6);
        assert!((port.radius_m - authored.radius_m).abs() < 1.0e-6);
        assert_eq!(port.outward, Some(Vec3::Z), "the ball presents flat out of the bow");

        let mantlet = points
            .iter()
            .find(|point| point.zone == ArmorZone::Mantlet)
            .expect("the mantlet is aimable");
        assert_eq!(mantlet.frame, WeakspotFrame::Turret);
        assert!(mantlet.center.z > 0.0, "the mantlet sits forward of the ring");
        assert!(
            mantlet.outward.expect("carrier presentation").z > 0.5,
            "the frontal sector carries the aim point"
        );

        let drum = points
            .iter()
            .find(|point| point.zone == ArmorZone::Cupola)
            .expect("the commander's drum is aimable");
        assert_eq!(drum.frame, WeakspotFrame::Turret);
        assert!((drum.center.x - blueprint.turret.cupola_x).abs() < 1.0e-4);
        assert!((drum.center.z - blueprint.turret.cupola_z).abs() < 1.0e-4);
        assert!(drum.radius_m > 0.0);
        assert!(
            drum.radius_m <= blueprint.turret.cupola_radius + 1.0e-4,
            "the presented disc never exceeds the drum's own footprint"
        );
        assert_eq!(drum.outward, None, "a drum offers a face from every bearing");
    }

    #[test]
    fn a_pike_bow_collapses_its_cloned_ports_to_single_aim_points() {
        // Tiger I authors two bow ports on a flat bow; the IS-3's pike carries NO ports today.
        // The clone-collapse contract is exercised by any vehicle whose bow sweeps two planes:
        // every authored port must appear exactly once per authored detail, never once per plane.
        for kind in VehicleKind::PLAYABLE {
            let Some(volumes) = vehicle_armor_volumes(kind) else { continue };
            let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else { continue };
            let authored = blueprint.armor.glacis_ports.iter().flatten().count();
            let enumerated = volumes
                .weakspot_aim_points()
                .iter()
                .filter(|point| point.zone == ArmorZone::GlacisPort)
                .count();
            assert_eq!(
                enumerated, authored,
                "{kind:?}: {authored} authored ports must enumerate as exactly {enumerated}"
            );
        }
    }

    #[test]
    fn every_baked_vehicle_offers_its_mantlet_and_its_drum() {
        for kind in VehicleKind::PLAYABLE {
            let Some(volumes) = vehicle_armor_volumes(kind) else { continue };
            let points = volumes.weakspot_aim_points();
            assert!(
                points.iter().any(|point| point.zone == ArmorZone::Mantlet),
                "{kind:?} must offer its mantlet as an aim point"
            );
            assert!(
                points.iter().any(|point| point.zone == ArmorZone::Cupola),
                "{kind:?} must offer its commander's drum as an aim point"
            );
        }
    }
}
