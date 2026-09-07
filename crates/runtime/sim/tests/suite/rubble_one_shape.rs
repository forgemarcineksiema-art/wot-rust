//! X11 (the one program, block 4): rubble has ONE shape. The 38° pyramid a hull climbs
//! (`terrain::RubbleMound::height_at`) is the surface the shell stops on and the eye is
//! stopped by. Before this the shell and the eye met a lowered box with vertical faces, the
//! picture a slab at 0.55 of the crest — three shapes for one pile, and a hull "hull-down"
//! on a talus that the shell treated as a wall.
//!
//! The lock: for a grid of points on the talus and the crown, a horizontal segment a hand
//! UNDER the surface is blocked (sight and shell alike) and one a hand OVER it passes —
//! `line_of_sight` and `segment_impact` resolve against `height_at`, everywhere on the pile.

use glam::Vec3;
use sim::{ShellTraceWorld, line_of_sight, segment_impact};
use terrain::{RubbleMound, StaticCoverKind, StaticCoverObject};

fn tenement(yaw_rad: f32) -> StaticCoverObject {
    StaticCoverObject {
        id: "tenement".into(),
        name: "tenement".into(),
        kind: StaticCoverKind::CityBuilding,
        center: [100.0, 5.5, 100.0],
        half_extents_m: [9.0, 5.5, 5.0],
        yaw_rad,
    }
}

/// The pile's surface decides for the eye and the shell: under it blocked, over it clear,
/// on a grid across the whole footprint — talus, crown and the foot of the flank.
fn assert_surface_decides(object: &StaticCoverObject) {
    let cover = [object.clone()];
    let mut states = sim::initial_cover_states(&cover);
    sim::damage_cover(&mut states, &cover, 0, u32::MAX, 0.0);
    let live = sim::live_cover_for_sight_and_shells(&cover, &states);
    assert!(live.is_empty(), "the sight slice carries no box for a mound");
    let mounds = sim::rubble_mounds(&cover, &states);
    assert_eq!(mounds.len(), 1);
    let mound: RubbleMound = mounds[0];
    assert!(mound.crest_y_m > mound.base_y_m + 1.0);

    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &[],
        blockers: &[],
        heightmap: None,
        cover: &live,
        rubble: &mounds,
        water: terrain::WaterView::DRY,
    };
    let hand = 0.1;
    let mut checked = 0;
    for ix in 0..=18 {
        for iz in 0..=10 {
            // A grid in the box's OWN frame, so a turned box is sampled across its own talus.
            let local = [(-9.0 + ix as f32) * 0.999, 0.0, (-5.0 + iz as f32) * 0.999];
            let world_xz = terrain::CoverBox::of(object).to_world(local);
            let Some(surface) = mound.height_at(world_xz[0], world_xz[2]) else {
                panic!("the grid is inside the footprint");
            };
            if surface <= mound.base_y_m + hand {
                continue; // the very foot of the flank: nothing to be under
            }
            checked += 1;
            for (dx, dz) in [(1.0, 0.0), (0.0, 1.0), (0.7, 0.7)] {
                let run = Vec3::new(dx, 0.0, dz) * 30.0;
                let at = |y: f32| Vec3::new(world_xz[0], y, world_xz[2]);
                let under_from = at(surface - hand) - run;
                let under_to = at(surface - hand) + run;
                let over_from = at(surface + hand) - run;
                let over_to = at(surface + hand) + run;
                assert!(
                    !line_of_sight(None, &live, &mounds, under_from, under_to),
                    "a hand under the surface at {world_xz:?} the eye is stopped"
                );
                let shell = segment_impact(under_from, under_to, run, &world)
                    .unwrap_or_else(|| panic!("a hand under the surface the shell stops"));
                let point = shell.point();
                let on_surface =
                    mound.height_at(point.x, point.z).expect("the impact is on the pile");
                assert!(
                    (point.y - on_surface).abs() < 0.2,
                    "the shell stops ON the pile's surface: {point:?} vs {on_surface}"
                );
                // A hand over the local surface can still meet a HIGHER part of the pile
                // further along the run — that is the pyramid, not a fault. Only where the
                // run's highest surface is under the segment must it pass.
                let peak = (0..=600)
                    .filter_map(|i| {
                        let p = over_from + (over_to - over_from) * (i as f32 / 600.0);
                        mound.height_at(p.x, p.z)
                    })
                    .fold(f32::MIN, f32::max);
                if peak < surface + hand * 0.5 {
                    assert!(
                        line_of_sight(None, &live, &mounds, over_from, over_to),
                        "a hand over the surface at {world_xz:?} the eye passes"
                    );
                    assert!(
                        segment_impact(over_from, over_to, run, &world).is_none(),
                        "a hand over the surface at {world_xz:?} the shell passes"
                    );
                }
            }
        }
    }
    assert!(checked >= 150, "the grid covers the pile: {checked} points");
}

#[test]
fn the_shell_and_the_eye_stop_exactly_where_the_hull_stands_on_a_mound() {
    assert_surface_decides(&tenement(0.0));
}

/// X1's turned boxes: the talus runs in the box's own frame for the shell too.
#[test]
fn a_turned_mound_is_the_same_pyramid_in_its_own_frame() {
    assert_surface_decides(&tenement(0.6));
}
