//! Blueprint-driven chassis: a shaped hull (glacis built from the armour slope, raked rear, side
//! fenders) and a wrapped track belt (a stadium that hugs the wheel line) with road wheels, a drive
//! sprocket, an idler, and return rollers — the realistic running gear, replacing the legacy box.

use game_core::{HullShape, SkirtShape, TrackShape};
use glam::{Vec2, Vec3};

use super::SG_HARD;
use crate::{Axis, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder};

/// The plane-honest prism hull: both body prisms lofted directly from the armor volumes' plane
/// equations — the fold ridge at the sponson step, the glacis leaning `glacis_slope_deg` above
/// it, the derived lower nose below it, the rear pair at `rear_slope_deg`, and (unlike the
/// extruded [`blueprint_hull`]) upper SIDE walls leaned inward by `side_slope_deg`, the same
/// plane the armor model resolves a side shot on. For the sloped German school: what you see
/// leaning is what you shoot.
pub(crate) fn blueprint_prism_hull(hull: &HullShape, side_slope_deg: f32) -> MeshBuilder {
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    // The lower-plate slope derives from the glacis exactly like the armor model's zone table.
    let lower = (hull.glacis_slope_deg * 0.45).to_radians().tan();
    let rear = hull.rear_slope_deg.to_radians().tan();
    let side = side_slope_deg.to_radians().tan();
    let step = hull.sponson_y;

    // One rectangular plan ring per height: front/rear z from the fold planes, width from the
    // side lean above the step (the tub stays vertical below it).
    let ring = |y: f32| -> Vec<Vec2> {
        let (width, run) = if y >= step {
            (hull.half_width - (y - step) * side, y - step)
        } else {
            (hull.lower_half_width, 0.0)
        };
        let front = if y >= step {
            hull.half_len - run * glacis
        } else {
            hull.half_len - (step - y) * lower
        };
        let back = if y >= step {
            -hull.half_len + run * rear
        } else {
            -hull.half_len + (step - y) * rear
        };
        vec![
            Vec2::new(width, front),
            Vec2::new(width, back),
            Vec2::new(-width, back),
            Vec2::new(-width, front),
        ]
    };
    let prism = |bottom: f32, top: f32| LoftSpec {
        sections: vec![LoftSection::new(bottom, ring(bottom)), LoftSection::new(top, ring(top))],
        axis: Axis::Y,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
        cap_ends: true,
    };
    MeshBuilder::new()
        .loft(Vec3::ZERO, prism(hull.belly_y, step))
        .loft(Vec3::ZERO, prism(step, hull.deck_y))
}

/// The **static belt band** for one blueprint, mirrored to both sides: a thin top run and bottom
/// run with wrapped metal end loops around the idler/sprocket, so the belt reads as one continuous
/// band. The moving parts — road wheels, drive sprocket, idler, and the shoe links — are no longer
/// baked here; they are instanced and animated at render time from
/// [`crate::running_gear::running_gear_placements`] so the wheels spin and the links scroll.
pub(crate) fn blueprint_running_gear(track: &TrackShape) -> GeometryMesh {
    // The static band follows the SAME belt path the animated links ride (model-logic audit
    // defect #7): flat ground run pressed under the wheels, tangent ramps onto the true
    // idler/sprocket wraps, and a top run that drapes onto its carriers instead of the old
    // taut straight boxes floating at `top_y`/`bottom_y`.
    let kin = crate::running_gear::RunningGearKinematics::from_track(track);
    let path = crate::running_gear_belt::BeltPath::new(&kin);
    let half_w = kin.band_half_width;
    // Plate thickness of the band: a whisker above the link centre line, more below it, so
    // the instanced shoes read as the band's own surface rather than a second skin.
    // v3: the band tucks DEEPER under the shoes (was +0.020 — near-coplanar with angled
    // plates at kinks and wraps, shimmering in hangar light).
    let up = 0.010;
    let down = 0.044;
    let ring = |y: f32| -> Vec<Vec2> {
        vec![
            Vec2::new(kin.link_x - half_w, y - down),
            Vec2::new(kin.link_x + half_w, y - down),
            Vec2::new(kin.link_x + half_w, y + up),
            Vec2::new(kin.link_x - half_w, y + up),
        ]
    };
    let band_loft = |builder: MeshBuilder, stations: Vec<(f32, f32)>| -> MeshBuilder {
        let sections = stations.into_iter().map(|(z, y)| LoftSection::new(z, ring(y))).collect();
        builder.loft(
            Vec3::ZERO,
            LoftSpec {
                sections,
                axis: Axis::Z,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
                cap_ends: true,
            },
        )
    };

    let bez = path.bottom_end_z();
    let yb = path.y_bot();
    let (rz, ry) = path.rear_ramp_end();
    let mut builder = MeshBuilder::new();
    // One loft for ramp -> ground run -> ramp (monotonic in Z), one for the draped top run.
    let mut lower = vec![(-bez, yb), (bez, yb)];
    if (rz - -bez).abs() > 1.0e-3 || (ry - yb).abs() > 1.0e-3 {
        lower.insert(0, (rz, ry));
        lower.push((-rz, ry));
    }
    builder = band_loft(builder, lower);
    builder = band_loft(builder, path.top_polyline(3));
    // The wrap ARCS carry band too (user report: the floor showed straight through the
    // wedge gaps between angled shoe plates around the sprocket/idler — the loop had no
    // skin there). Each arc splits at its outermost point into two Z-monotonic halves so
    // the same band loft covers it; the links still read as the surface, the band only
    // blocks the see-through.
    let (wr, wcy, wcz, theta_start) = path.wrap_arc();
    let arc_stations = |theta_a: f32, theta_b: f32, cz: f32, flip: f32| -> Vec<(f32, f32)> {
        let steps = 8;
        (0..=steps)
            .map(|i| {
                let t = theta_a + (theta_b - theta_a) * (i as f32 / steps as f32);
                (flip * (cz + wr * t.cos()), wcy + wr * t.sin())
            })
            .collect()
    };
    use std::f32::consts::PI;
    for flip in [-1.0_f32, 1.0] {
        // Tangent point -> rear-most (z monotonic toward the end), then rear-most -> top.
        builder = band_loft(builder, arc_stations(theta_start, -PI, -wcz, flip));
        builder = band_loft(builder, arc_stations(-PI, -1.5 * PI, -wcz, flip));
    }
    builder.mirror(Axis::X).build()
}

/// The side skirts, mirrored to both sides: one thin plate run hung outside the track band at
/// the blueprint's standoff — the SAME plane the armor volumes bake the skirt screen on, so the
/// sheet the player sees is the sheet a HEAT jet detonates against. A skirt-less blueprint
/// builds an empty mesh (a no-op append), so every blueprint recipe can call this untouched.
pub(crate) fn blueprint_skirts(hull: &HullShape, track: &TrackShape) -> GeometryMesh {
    let Some(skirt): Option<SkirtShape> = hull.skirt else {
        return MeshBuilder::new().build();
    };
    let cx = track.outer_x + skirt.standoff_m + skirt.thickness_m * 0.5;
    let cy = (skirt.top_y + skirt.bottom_y) * 0.5;
    let cz = (skirt.front_z + skirt.rear_z) * 0.5;
    let half = Vec3::new(
        skirt.thickness_m * 0.5,
        (skirt.top_y - skirt.bottom_y).abs() * 0.5,
        (skirt.front_z - skirt.rear_z).abs() * 0.5,
    );
    MeshBuilder::new()
        .chamfered_prism(Vec3::new(cx, cy, cz), half, 0.02, MaterialRole::RolledArmor, SG_HARD)
        .mirror(Axis::X)
        .build()
}
