//! Named surface anchors: where details (hatches, periscopes, the DShK mount, tow hooks, handles,
//! brackets, the mantlet seam) attach to a vehicle. Anchors are derived from the owning semantic
//! part's authored dimensions — never from arbitrary final merged-mesh vertex indices — so they stay
//! put as the mesh is reduced, rebaked or re-detailed.

use game_core::{HybridVisual, MountFrame};
use glam::Vec3;
use vehicle_geometry::{LodLevel, SubmeshKind};

use crate::part::{PartKey, PartLod};

/// A local anchor frame on one of the vehicle's pose groups (hull / turret / gun), with the surface
/// normal it sits on and the LOD policy that decides when it is present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceAttachment {
    pub key: PartKey,
    /// Which runtime pose group this anchor rides (so it follows hull/turret/gun motion).
    pub group: SubmeshKind,
    pub local_frame: MountFrame,
    pub normal: Vec3,
    pub allowed_lods: PartLod,
}

impl SurfaceAttachment {
    /// Whether this attachment is present at `lod` (mirrors the part LOD policy).
    pub fn present_at(&self, lod: LodLevel) -> bool {
        self.allowed_lods.kept_at(lod)
    }
}

/// The T-54's named surface anchors, derived from the blueprint's fitting and detail dimensions.
pub fn t54_attachments(visual: &HybridVisual) -> Vec<SurfaceAttachment> {
    let f = &visual.fittings;
    let d = &visual.detail;
    let up = Vec3::Y;
    let turret = SubmeshKind::Turret;
    let hull = SubmeshKind::Hull;

    let anchor = |key, group, pos: Vec3, normal, lod| SurfaceAttachment {
        key: PartKey(key),
        group,
        local_frame: MountFrame::new(pos),
        normal,
        allowed_lods: lod,
    };

    vec![
        // Silhouette/mount-critical roof furniture: kept down the LOD ladder.
        anchor("cupola_hatch", turret, f.cupola_hatch_center, up, PartLod::MountCritical),
        anchor("dshk_mount", turret, d.dshk_mount_center, up, PartLod::MountCritical),
        // Close-range identifiable detail: dropped below LOD0.
        anchor("loader_hatch", turret, f.loader_hatch_center, up, PartLod::Detail),
        anchor("turret_periscope", turret, d.periscope_center, up, PartLod::Detail),
        anchor("driver_hatch", hull, f.driver_hatch_center, up, PartLod::Detail),
        anchor("headlight", hull, f.headlight_center, Vec3::Z, PartLod::Detail),
        anchor("tow_hook", hull, f.tow_hook_center, Vec3::Z, PartLod::Detail),
        anchor(
            "mantlet_seam",
            turret,
            Vec3::new(0.0, d.periscope_center.y, 1.04),
            Vec3::Z,
            PartLod::Detail,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use game_core::{VehicleBlueprint, VehicleKind};

    use super::*;

    fn attachments() -> Vec<SurfaceAttachment> {
        let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
        t54_attachments(bp.hybrid().unwrap())
    }

    fn find(list: &[SurfaceAttachment], key: &str) -> SurfaceAttachment {
        *list.iter().find(|a| a.key.0 == key).expect("attachment exists")
    }

    #[test]
    fn anchors_are_derived_from_the_blueprint_not_the_mesh() {
        let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
        let visual = bp.hybrid().unwrap();
        let list = t54_attachments(visual);
        // The cupola-hatch anchor sits exactly at the blueprint cupola-hatch centre.
        let cupola = find(&list, "cupola_hatch");
        assert_eq!(cupola.local_frame.translation, visual.fittings.cupola_hatch_center);
        assert_eq!(cupola.group, SubmeshKind::Turret);
        // The driver hatch rides the hull group.
        assert_eq!(find(&list, "driver_hatch").group, SubmeshKind::Hull);
    }

    #[test]
    fn mount_critical_anchors_survive_lower_lods_but_detail_does_not() {
        let list = attachments();
        let cupola = find(&list, "cupola_hatch");
        let periscope = find(&list, "turret_periscope");
        assert!(cupola.present_at(LodLevel::Lod0));
        assert!(cupola.present_at(LodLevel::Lod2), "the cupola hatch is mount-critical");
        assert!(periscope.present_at(LodLevel::Lod0));
        assert!(!periscope.present_at(LodLevel::Lod1), "micro-detail drops below LOD0");
    }

    #[test]
    fn every_anchor_carries_a_finite_unit_normal() {
        for a in attachments() {
            assert!(a.normal.is_finite() && (a.normal.length() - 1.0).abs() < 1.0e-5);
            assert!(a.local_frame.translation.is_finite());
        }
    }
}
