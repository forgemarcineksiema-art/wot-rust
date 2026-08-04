//! A collapsed building as GROUND rather than as an obstacle.
//!
//! `CoverPhase::Rubble` has always been a lowered box, which is the right shape for a shell trace
//! and a sight line — but a box is exactly the wrong shape for something a hull is meant to drive
//! onto. A box has vertical sides, so a rigid running gear meeting one either teleports up its
//! full height at the edge or is stopped dead by it. Neither is a mound.
//!
//! So for the purpose of SUPPORT a collapsed building is a truncated pyramid: a flat top with
//! flanks at the angle of repose of broken masonry ([`RUBBLE_REPOSE_GRADE`], 0.78 / ~38°). The
//! debris stays inside the authored footprint, so streets do not silt up, and the surface is
//! continuous with the ground at the footprint edge — there is no step to fall off.
//!
//! Worth stating plainly, because the shape invites the opposite guess: that grade is steeper than
//! anything a tank climbs (the momentum-climb ceiling is 0.68), and yet every mound the shipping
//! `rubble_height_frac` values produce is crossable anyway. The support envelope is why. A barn
//! mound is 2.4 m of debris over a 3.1 m talus and a T-54's running gear spans 4.4 m between end
//! stations, so the rigid beam BRIDGES the flank exactly as it bridges a trench narrower than its
//! wheel pitch. A pile smaller than the tank is something a tank drives over.
//!
//! The repose grade therefore governs the SURFACE, not passability. It only bites for a flank the
//! gear cannot span (~5 m of debris and up), which no map authors today; the knob for gating
//! manoeuvre on rubble is `rubble_height_frac`, never a special case in the drive.

use serde::{Deserialize, Serialize};

use crate::battlefield::StaticCoverObject;

/// Angle of repose of broken masonry, as rise/run. ~38°.
pub const RUBBLE_REPOSE_GRADE: f32 = 0.78;

/// One collapsed building, as the ground a hull stands on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RubbleMound {
    /// Centre of the authored footprint, in world XZ.
    pub center_xz_m: [f32; 2],
    /// Half extents of the authored FOOTPRINT — the debris never spreads past this.
    pub footprint_half_m: [f32; 2],
    /// Half extents of the flat top, inset from the footprint by the talus run.
    pub crown_half_m: [f32; 2],
    /// Ground the mound rests on.
    pub base_y_m: f32,
    /// World height of the flat top.
    pub crest_y_m: f32,
}

impl RubbleMound {
    /// The mound an authored cover box collapses into. `object` is the box AS AUTHORED (not the
    /// lowered sight-and-shells version); the collapsed height comes from the same
    /// [`crate::StaticCoverKind::rubble_height_frac`] the sight geometry uses, so the mound a hull
    /// climbs and the mound a shell meets are the same pile.
    pub fn from_cover(object: &StaticCoverObject) -> Self {
        let footprint_half =
            [object.half_extents_m[0].max(0.01), object.half_extents_m[2].max(0.01)];
        let base_y_m = object.center[1] - object.half_extents_m[1];
        let full_height = object.half_extents_m[1] * 2.0;
        let mound_height = full_height * object.kind.rubble_height_frac();

        // A narrow footprint cannot hold a tall pile: the talus from both sides meets in the
        // middle and the mound tops out at whatever the repose angle allows over half the span.
        let smallest_half = footprint_half[0].min(footprint_half[1]);
        let height = mound_height.min(smallest_half * RUBBLE_REPOSE_GRADE);
        let talus_run = height / RUBBLE_REPOSE_GRADE;

        Self {
            center_xz_m: [object.center[0], object.center[2]],
            footprint_half_m: footprint_half,
            crown_half_m: [
                (footprint_half[0] - talus_run).max(0.0),
                (footprint_half[1] - talus_run).max(0.0),
            ],
            base_y_m,
            crest_y_m: base_y_m + height,
        }
    }

    /// Height of the debris surface at a world XZ point, or `None` outside the footprint.
    ///
    /// The inset is Chebyshev (the larger of the two axis insets), which is what an inset
    /// RECTANGLE means — so the surface is continuous: at the footprint edge the flank has run
    /// exactly back down to [`Self::base_y_m`], where the surrounding terrain takes over.
    pub fn height_at(&self, x_m: f32, z_m: f32) -> Option<f32> {
        let dx = (x_m - self.center_xz_m[0]).abs();
        let dz = (z_m - self.center_xz_m[1]).abs();
        if dx > self.footprint_half_m[0] || dz > self.footprint_half_m[1] {
            return None;
        }
        let inset = (dx - self.crown_half_m[0]).max(dz - self.crown_half_m[1]).max(0.0);
        Some((self.crest_y_m - inset * RUBBLE_REPOSE_GRADE).max(self.base_y_m))
    }
}

/// The highest debris surface at a world XZ point across every mound, or `None` if the point is
/// on none of them. `mounds` is empty for the overwhelming majority of battles, which is why this
/// is a plain linear scan: the fast path is the emptiness check.
/// What broken masonry does under a track (teren F2). Before this constant existed, a hull
/// on a collapsed tenement drove with whatever the splat said was under the BUILDING —
/// lawn, usually. Debris bites decently (edges catch the links) but rolls hard (the climb
/// is a churn of brick), and stone rubble takes no rut. The mound's surface is the mound's
/// material; the ground rule keeps owning everything that is actually ground.
pub const RUBBLE_PROPERTIES: crate::ground::GroundProperties = crate::ground::GroundProperties {
    grip_scale: 0.98,
    rolling_resist_scale: 1.3,
    rut_depth_m: 0.0,
};

pub fn rubble_height_at(mounds: &[RubbleMound], x_m: f32, z_m: f32) -> Option<f32> {
    let mut highest: Option<f32> = None;
    for mound in mounds {
        if let Some(height) = mound.height_at(x_m, z_m) {
            highest = Some(highest.map_or(height, |current: f32| current.max(height)));
        }
    }
    highest
}

/// Ground height at a point with the debris folded in: the terrain, raised wherever a mound
/// stands on it. This is THE function every support and contact sample goes through, so the
/// hull, the attitude and the drive forces all read one surface.
pub fn ground_with_rubble(terrain_y_m: Option<f32>, rubble_y_m: Option<f32>) -> Option<f32> {
    match (terrain_y_m, rubble_y_m) {
        (Some(terrain), Some(rubble)) => Some(terrain.max(rubble)),
        (Some(terrain), None) => Some(terrain),
        // Debris off the edge of the heightmap is not ground the game owns.
        (None, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battlefield::StaticCoverKind;

    fn tenement() -> StaticCoverObject {
        StaticCoverObject {
            id: "tenement".into(),
            name: "tenement".into(),
            kind: StaticCoverKind::CityBuilding,
            center: [0.0, 5.5, 0.0],
            half_extents_m: [9.0, 5.5, 5.0],
        }
    }

    #[test]
    fn the_mound_is_continuous_with_the_ground_at_the_footprint_edge() {
        let mound = RubbleMound::from_cover(&tenement());
        // Dead centre: the flat top.
        assert!((mound.height_at(0.0, 0.0).unwrap() - mound.crest_y_m).abs() < 1.0e-5);
        // At the footprint edge the flank has run all the way back down to the base...
        let edge = mound.height_at(9.0, 0.0).expect("the edge is still on the footprint");
        assert!((edge - mound.base_y_m).abs() < 1.0e-4, "edge {edge} vs base {}", mound.base_y_m);
        // ...and one hair outside it there is no debris at all, so no step to fall off.
        assert_eq!(mound.height_at(9.01, 0.0), None);
    }

    /// The flanks stand at the repose angle, everywhere on the talus. Whether a hull can climb
    /// them is a question about the pile's size against the running gear, not about this number —
    /// see `physics/tests/rubble_support.rs`.
    #[test]
    fn the_flanks_stand_at_the_angle_of_repose() {
        let mound = RubbleMound::from_cover(&tenement());
        // Both samples must be ON the flank: the crown of this footprint reaches x = 6.46.
        assert!(mound.crown_half_m[0] < 7.0, "the sample points must clear the flat top");
        let a = mound.height_at(7.0, 0.0).unwrap();
        let b = mound.height_at(8.0, 0.0).unwrap();
        assert!((a - b - RUBBLE_REPOSE_GRADE).abs() < 1.0e-4, "flank grade {}", a - b);
    }

    /// A narrow footprint cannot hold a tall pile — the talus from both sides meets in the
    /// middle. Without this a garden wall's rubble would be a spike with 45-degree-plus sides.
    #[test]
    fn a_narrow_footprint_tops_out_at_what_the_repose_angle_allows() {
        let wall = StaticCoverObject {
            id: "yard_wall".into(),
            name: "yard_wall".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [0.0, 3.0, 0.0],
            half_extents_m: [8.0, 3.0, 0.5],
        };
        let mound = RubbleMound::from_cover(&wall);
        let height = mound.crest_y_m - mound.base_y_m;
        assert!(
            height <= 0.5 * RUBBLE_REPOSE_GRADE + 1.0e-5,
            "a 1 m-wide footprint cannot pile {height} m high"
        );
        assert_eq!(mound.crown_half_m[1], 0.0, "there is no flat top left across the narrow axis");
    }

    #[test]
    fn debris_raises_the_ground_but_never_invents_it_off_the_map() {
        assert_eq!(ground_with_rubble(Some(2.0), Some(4.5)), Some(4.5));
        assert_eq!(ground_with_rubble(Some(6.0), Some(4.5)), Some(6.0));
        assert_eq!(ground_with_rubble(Some(6.0), None), Some(6.0));
        assert_eq!(ground_with_rubble(None, Some(4.5)), None);
    }
}
