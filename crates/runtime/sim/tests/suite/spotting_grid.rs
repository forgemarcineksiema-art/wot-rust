//! The ONE march (the one program's V0 / S21): the eye and the shell read the same exact
//! terrain kernel. The eye used to step a flat 2 m without interpolation and the shell 1 m
//! with it; on a ±30° crest the eye saw ~0.9 m through what killed the shell, and the Bystra
//! hull-down contract was signed with the eye. Now `terrain::ground_blocks_segment` and
//! `terrain::first_ground_impact` are two readings of one kernel, exact on the piecewise-planar
//! surface; the only difference left between them is the eye's grazing slack and the shell's
//! radius — and the bot fires on the shell's reading.

use game_core::SIGHT_GRAZE_SLACK_M;
use glam::Vec3;

fn splitmix(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
}

/// The parity lock, on every shipped map: whatever the shell's march clears, the eye's clears
/// too; wherever the eye sees and the shell dies, the ground rose by no more than the eye's
/// slack (the kernel is one, the slack is the whole difference); and with the slack taken away
/// the two verdicts are identical, segment for segment.
#[test]
fn the_eye_and_the_shell_march_one_surface_on_every_map() {
    for &map_id in terrain::MapId::SHIPPED {
        let map = map_forge::battlefield(map_id);
        let heightmap = &map.heightmap;
        let [ex, ez] = heightmap.extent_m();
        let mut seed = 0x5EED_0000 ^ map_id as u64;
        let (mut agreed, mut slack_only) = (0, 0);
        for _ in 0..400 {
            let ax = 20.0 + splitmix(&mut seed) * (ex - 40.0);
            let az = 20.0 + splitmix(&mut seed) * (ez - 40.0);
            let reach = 30.0 + splitmix(&mut seed) * 250.0;
            let angle = splitmix(&mut seed) * std::f32::consts::TAU;
            let bx = (ax + reach * angle.cos()).clamp(5.0, ex - 5.0);
            let bz = (az + reach * angle.sin()).clamp(5.0, ez - 5.0);
            let from = Vec3::new(ax, heightmap.sample_height(ax, az).unwrap() + 2.4, az);
            let to = Vec3::new(bx, heightmap.sample_height(bx, bz).unwrap() + 1.2, bz);
            let eye_sees = sim::line_of_sight(Some(heightmap), &[], from, to);
            let shell_clears = sim::shell_line_clear(Some(heightmap), &[], from, to, 0.0);
            if shell_clears {
                assert!(
                    eye_sees,
                    "{map_id:?}: the shell clears but the eye is blind: {from} -> {to}"
                );
            }
            let eye_no_slack =
                !terrain::ground_blocks_segment(heightmap, from.to_array(), to.to_array(), 0.0);
            assert_eq!(
                eye_no_slack, shell_clears,
                "{map_id:?}: without its slack the eye is the shell: {from} -> {to}"
            );
            if eye_sees && !shell_clears {
                // The eye sees over a crest the shell would eat, by no more than its slack:
                // the one disagreement the kernel leaves, and it is small and it is named.
                slack_only += 1;
                assert!(
                    !terrain::ground_blocks_segment(
                        heightmap,
                        from.to_array(),
                        to.to_array(),
                        SIGHT_GRAZE_SLACK_M
                    ),
                    "{map_id:?}: the eye's verdict is the slack's: {from} -> {to}"
                );
            } else {
                agreed += 1;
            }
        }
        println!(
            "ONE MARCH {map_id:?}: {agreed} lines agree, {slack_only} the eye sees by its slack"
        );
        assert!(
            slack_only * 100 <= (agreed + slack_only) * 12,
            "{map_id:?}: the slack explains {slack_only} of {} lines — more than an eighth",
            agreed + slack_only
        );
    }
}

/// The step is gone: no sight-line sampling step exists to tie to a grid any more (the kernel
/// walks cell lines and diagonals); a crater's bowl is walked at its own fine step, pinned here.
const _: () = assert!(terrain::CRATER_MARCH_STEP_M <= 0.25);
