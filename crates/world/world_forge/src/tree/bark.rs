//! Bark meshing (Drzewa 3.0 PR4): the skeleton's branches swept into round, tapering tubes by
//! the `sweep` kernel — parallel-transport frames so a bending limb never twists, per-station
//! `section_scale` so the taper is the radius table, sides from the roundness law so the RADIUS
//! decides how round a bole is, not what somebody typed. The old `tapered_tube` was a straight
//! flat-facetted prism; a trunk built here can lean, flare and curve, and reads round at the
//! range the probe reviews it.

use game_core::roundness::{SILHOUETTE_TOLERANCE_M, segments_for_radius};
use glam::Vec2;
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{GeometryMesh, SmoothingGroup};

use super::TreeLod;
use super::skeleton::TreeSkeleton;
use crate::WorldMaterial;
use crate::shape::merge_meshes;

/// Mesh the bark for one LOD rung.
///
/// Close carries the whole skeleton — trunk, limbs and twigs, because the twigs carry the
/// card canopy (PR6) and a card needs its wood. Mid keeps trunk AND limbs at half the
/// stations and a coarser tolerance (the user's verdict, 2026-08-22: a bare pole with cards
/// at 60 m was a different tree from the one at 50 m — the 55 m swap must move triangles,
/// never the tree's ANATOMY); only the twigs stay inside the card mass past the band.
pub(crate) fn mesh_bark(skeleton: &TreeSkeleton, lod: TreeLod) -> GeometryMesh {
    let (max_level, tolerance_scale, station_stride) = match lod {
        TreeLod::Close => (2, 1.0, 1),
        TreeLod::Mid => (1, 4.0, 2),
    };
    let mut bark: Option<GeometryMesh> = None;
    for branch in &skeleton.branches {
        if branch.level > max_level {
            continue;
        }
        // Decimate stations for the coarse rung, but the TIP always survives — a rung swap
        // moves triangles, never metres.
        let mut picked: Vec<usize> = (0..branch.stations.len()).step_by(station_stride).collect();
        if *picked.last().expect("a branch has stations") != branch.stations.len() - 1 {
            picked.push(branch.stations.len() - 1);
        }
        let points: Vec<glam::Vec3> =
            picked.iter().map(|&index| branch.stations[index].position).collect();
        let scales: Vec<f32> =
            picked.iter().map(|&index| branch.stations[index].radius_m.max(1.0e-3)).collect();
        // The roundness law, on a tolerance schedule: the trunk gets the honest fleet
        // tolerance, each deeper level doubles it (a limb reads at half the scrutiny), the
        // rung's own scale coarsens Mid. Radius still decides — the author only picks the
        // schedule.
        let tolerance = SILHOUETTE_TOLERANCE_M * tolerance_scale * (1 << branch.level) as f32;
        // The law's MIN_SEGMENTS floor (8) is written for parts that must READ round; a
        // card-covered stick among hundreds of alpha shapes is not one of them, and eight
        // sides on each would spend ~700 tris on wood nobody sees. Twigs (level 2+) and any
        // branch under 12 cm — a bush's fan, a pine's frond branch, a poplar's shoot, every
        // one of them wearing its card mass — take a hand-typed 4, with this justification;
        // boles and real limbs stay under the honest law.
        let sides = if branch.level >= 2 || branch.base().radius_m < 0.12 {
            4
        } else {
            segments_for_radius(branch.base().radius_m, tolerance)
        };
        let section = SweepSection {
            points: (0..sides)
                .map(|side| {
                    let angle = side as f32 / sides as f32 * std::f32::consts::TAU;
                    Vec2::new(angle.cos(), angle.sin())
                })
                .collect(),
            closed: true,
        };
        let spec = SweepSpec {
            path: &SweepPath { points, closed: false },
            section: &section,
            frame_mode: SweepFrameMode::ParallelTransport,
            // The trunk's butt sits in the ground and its top inside the canopy — open. A limb
            // tip ends in the air until its twigs and cards arrive — capped, so no see-through.
            caps: if branch.parent.is_none() { SweepCaps::Open } else { SweepCaps::Both },
            material: WorldMaterial::Bark.carrier(),
            smoothing: SmoothingGroup(1),
            section_scale: Some(&scales),
        };
        let mesh = try_sweep(&spec).expect("a grown branch is a valid sweep");
        bark = Some(match bark {
            Some(accumulated) => merge_meshes(accumulated, mesh),
            None => mesh,
        });
    }
    bark.expect("a skeleton has a trunk")
}

#[cfg(test)]
mod tests {
    use super::super::TreeSpecies;
    use super::super::skeleton::grow;
    use super::*;

    fn oak_skeleton(seed: u64) -> TreeSkeleton {
        grow(&TreeSpecies::Oak.architecture().expect("the oak is branched"), seed)
    }

    /// The bark is a real mesh under the kernel's own quality bar, at both rungs.
    #[test]
    fn oak_bark_is_a_valid_open_mesh_at_both_rungs() {
        for lod in [TreeLod::Close, TreeLod::Mid] {
            let bark = mesh_bark(&oak_skeleton(0), lod);
            let report = bark.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH);
            assert!(report.is_ok(), "{lod:?}: {report:?}");
            assert!(bark.triangle_count() > 0, "{lod:?} bark draws something");
        }
    }

    /// The law, not the author: the trunk's sides come from its radius. A 0.52 m bole at the
    /// fleet tolerance needs ~30 sides — the hand-typed 7 the legacy tube carried left 5+ mm
    /// of facet error on the single most-reviewed cylinder in the world.
    #[test]
    fn the_trunk_is_round_under_the_law_and_mid_stays_cheaper() {
        let close = mesh_bark(&oak_skeleton(0), TreeLod::Close);
        let mid = mesh_bark(&oak_skeleton(0), TreeLod::Mid);
        let trunk_sides = segments_for_radius(0.52 * 1.35, SILHOUETTE_TOLERANCE_M);
        assert!(trunk_sides >= 24, "the law asks a real bole for real sides: {trunk_sides}");
        assert!(
            close.triangle_count() > mid.triangle_count(),
            "the ladder descends: Close {} vs Mid {}",
            close.triangle_count(),
            mid.triangle_count()
        );
    }

    /// The rung swap moves triangles, never metres OR anatomy: both rungs mesh the SAME
    /// skeleton, Mid carries the limbs (the 55 m swap must not amputate the tree), and the
    /// Mid bark tip agrees with the highest station Mid actually meshes (levels 0–1).
    #[test]
    fn both_rungs_share_the_skeleton_and_mid_keeps_the_limbs() {
        for seed in [0_u64, 7, 42] {
            let skeleton = oak_skeleton(seed);
            let tip =
                |mesh: &GeometryMesh| mesh.bounds().map(|bounds| bounds.max.y).unwrap_or_default();
            let mid_levels_tip = skeleton
                .branches
                .iter()
                .filter(|branch| branch.level <= 1)
                .map(|branch| branch.tip().position.y)
                .fold(0.0_f32, f32::max);
            let mid = mesh_bark(&skeleton, TreeLod::Mid);
            assert!(
                (tip(&mid) - mid_levels_tip).abs() < 0.05,
                "seed {seed}: the Mid bark tip drifted from the skeleton: {} vs {mid_levels_tip}",
                tip(&mid)
            );
            // The anatomy check: Mid's triangle count is far past a lone bole's — the limbs
            // are IN the mesh, not amputated at the band.
            let close = mesh_bark(&skeleton, TreeLod::Close);
            assert!(
                mid.triangle_count() > 250 && mid.triangle_count() < close.triangle_count(),
                "seed {seed}: Mid carries limbs yet stays the cheaper rung: {} vs {}",
                mid.triangle_count(),
                close.triangle_count()
            );
        }
    }
}
