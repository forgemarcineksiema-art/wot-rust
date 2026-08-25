use map_forge::battlefield;
use terrain::{HeightMap, MapId};

#[test]
fn western_field_is_flat_open_ground_unlike_the_rolling_steppe() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;

    // Local relief (max-min over ~35 m windows) measures folds a tank could hide behind.
    // The open Psel field (deep west, clear of the embankment, ditch and overwatch knolls)
    // must be markedly flatter than the rolling central steppe. The window starts EAST of
    // the Psel's own bank (teren W5): the river the flank is named for is a feature, not a
    // violation - the FIELD between the reeds and the rails is what stays naked.
    let field_relief = max_local_relief(hm, (16, 26), (46, 74), 3);
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
    let map = battlefield(MapId::ProkhorovkaHill252_2);
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

/// Teren W5: the Psel is honest water on the map's west edge - and ONLY there. Every wet
/// node sits in the river lowland (x < 120), the deepest wade stays under the ford band's
/// roof (nothing on this map can drown), and the fighting ground - spawns, the farm bench,
/// the balka floors - is dry to real margins.
#[test]
fn the_psel_wets_only_the_western_lowland_and_never_drowns() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let water = map.water.expect("the Psel ships");
    let hm = &map.heightmap;
    let mut deepest = 0.0f32;
    let mut wet_nodes = 0;
    for zi in 0..hm.height() {
        for xi in 0..hm.width() {
            let depth = water.depth_over(hm.sample_at_index(xi, zi));
            if depth > 0.0 {
                wet_nodes += 1;
                let x = xi as f32 * hm.cell_size_m();
                assert!(x < 120.0, "water outside the Psel lowland at x {x}");
                deepest = deepest.max(depth);
            }
        }
    }
    assert!(wet_nodes > 200, "the reach is a real river, not a puddle ({wet_nodes} nodes)");
    assert!(deepest < 0.9, "the Psel wades, never drowns (deepest {deepest:.2} m)");
    for (x, z, what) in
        [(500.0, 150.0, "south spawn"), (500.0, 500.0, "the farm"), (520.0, 384.0, "balka floor")]
    {
        let ground = hm.sample_height(x, z).expect("probe");
        assert!(
            water.depth_over(ground) == 0.0 && ground > water.surface_level_m + 0.3,
            "{what} must stand dry with margin (ground {ground:.2})"
        );
    }
}
