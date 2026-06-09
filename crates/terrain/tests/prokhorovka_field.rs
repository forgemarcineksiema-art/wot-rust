use terrain::{HeightMap, prokhorovka_hill_252_2};

#[test]
fn western_field_is_flat_open_ground_unlike_the_rolling_steppe() {
    let map = prokhorovka_hill_252_2();
    let hm = &map.heightmap;

    // Local relief (max-min over ~35 m windows) measures folds a tank could hide behind.
    // The open Psel field (deep west, clear of the embankment, ditch and overwatch knolls)
    // must be markedly flatter than the rolling central steppe.
    let field_relief = max_local_relief(hm, (8, 26), (46, 74), 3);
    let steppe_relief = max_local_relief(hm, (60, 92), (46, 74), 3);

    assert!(
        field_relief < 1.2,
        "the Psel field must be flat, coverless ground (local relief {field_relief:.2} m)"
    );
    assert!(
        steppe_relief > field_relief * 1.5,
        "the field must be markedly flatter than the rolling steppe \
         (field {field_relief:.2} m vs steppe {steppe_relief:.2} m)"
    );
}

#[test]
fn overwatch_knoll_holds_high_ground_and_sees_across_the_open_field() {
    let map = prokhorovka_hill_252_2();
    let hm = &map.heightmap;

    // South overwatch knoll vs a tank out in the open field at the same z.
    let perch = ground(hm, 235.0, 200.0);
    let prey = ground(hm, 90.0, 200.0);

    assert!(
        perch.1 > prey.1 + 3.0,
        "overwatch must hold the high ground over the field (perch {:.1} m vs field {:.1} m)",
        perch.1,
        prey.1
    );
    assert!(
        line_of_sight_clear(hm, perch, prey),
        "the open field must offer no terrain cover from the overwatch knoll"
    );
}

fn ground(hm: &HeightMap, x: f32, z: f32) -> (f32, f32, f32) {
    (x, hm.sample_height(x, z).expect("inside map"), z)
}

/// March a turret-height eye on `from` to a hull-height point on `to`; the sightline is clear
/// if the terrain never rises above the interpolated line between them.
fn line_of_sight_clear(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> bool {
    let eye_y = from.1 + 2.5;
    let target_y = to.1 + 1.5;
    let steps = 80;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.2 + (to.2 - from.2) * t;
        let line_y = eye_y + (target_y - eye_y) * t;
        if hm.sample_height(x, z).is_some_and(|terrain| terrain > line_y + 0.05) {
            return false;
        }
    }
    true
}

/// Worst-case local relief (max minus min height) over sliding `radius`-cell windows inside
/// the given inclusive-exclusive cell band.
fn max_local_relief(
    hm: &HeightMap,
    x_cells: (usize, usize),
    z_cells: (usize, usize),
    radius: usize,
) -> f32 {
    let mut worst = 0.0f32;
    for cz in z_cells.0..z_cells.1 {
        for cx in x_cells.0..x_cells.1 {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for nz in cz.saturating_sub(radius)..=(cz + radius).min(hm.height() - 1) {
                for nx in cx.saturating_sub(radius)..=(cx + radius).min(hm.width() - 1) {
                    let h = hm.sample_at_index(nx, nz);
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
            worst = worst.max(hi - lo);
        }
    }
    worst
}
