//! T1 (the one program, block "maps and terrain"): the grid is 2.5 m map-wide. The 5 m grid
//! turned a 12 m hill into a tent (G7) and put 2–4 velocity creases a second under the camera
//! (C4); the strokes are analytic, so a finer raster is the same map sampled twice as often —
//! no re-authoring, no new bake. Measured on the way (2026-09-08, MX330, the warm box the
//! owner's cold-sandwich condition still owes a cold number): the ground mesh 56 277 → 176 677
//! vertices (331 k → 1 051 k indices), Orliny full scene p50 28.0 → 32.4 ms against a 5 m
//! rerun of 31.6 ms — inside the box's thermal drift, the frame is fill-bound (×1.9–2.2 at
//! ×2.25 pixels); map compile + report 60–150 → 170–520 ms per map; the leash a start or a
//! target walks to passable ground became METRES (`START_LEASH_M`), because two CELLS of the
//! old grid reached 10 m and two cells of the new one stopped inside the windmill's own box.
//!
//! The lock is the reason for the row: along six straight drive lines per map, sampled every
//! 0.5 m, the largest single slope kink the hull and the camera feel is at most 0.8× what the
//! 5 m raster gave (measured 0.50–0.78: Prokhorovka 0.369 → 0.275, Bystra 0.355 → 0.277,
//! Orliny 0.266 → 0.134, Ostrogorsk 0.603 → 0.344, Mazurski 0.189 → 0.133), every shipped map
//! ships the 2.5 m grid, compiles clean, and compiles with its report inside a budget.

use map_forge::{blueprint_for, compile};
use terrain::MapId;

/// The largest single kink of slope between consecutive 0.5 m segments along six straight
/// drive lines across the map — a cell edge the hull and the camera feel.
fn worst_slope_kink(hm: &terrain::HeightMap) -> f32 {
    let step = 0.5f32;
    let lines: [([f32; 2], [f32; 2]); 6] = [
        ([100.0, 150.0], [900.0, 850.0]),
        ([900.0, 150.0], [100.0, 850.0]),
        ([500.0, 120.0], [500.0, 880.0]),
        ([120.0, 500.0], [880.0, 500.0]),
        ([200.0, 300.0], [800.0, 700.0]),
        ([300.0, 800.0], [700.0, 200.0]),
    ];
    let mut worst = 0.0f32;
    for (a, b) in lines {
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let n = (len / step) as usize;
        let (mut prev_h, mut prev_slope): (Option<f32>, Option<f32>) = (None, None);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, z) = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
            let Some(h) = hm.sample_height(x, z) else {
                (prev_h, prev_slope) = (None, None);
                continue;
            };
            if let Some(ph) = prev_h {
                let slope = (h - ph) / step;
                if let Some(ps) = prev_slope {
                    worst = worst.max((slope - ps).abs());
                }
                prev_slope = Some(slope);
            }
            prev_h = Some(h);
        }
    }
    worst
}

#[test]
fn every_shipped_map_is_sampled_at_two_and_a_half_metres_and_its_creases_shrank() {
    for id in MapId::SHIPPED {
        let shipped = blueprint_for(*id);
        assert_eq!(shipped.grid.cell_m, 2.5, "{id:?}: the grid is 2.5 m map-wide (T1)");
        assert_eq!(shipped.grid.samples_per_side(), 401, "{id:?}: 1000 m at 2.5 m");
        let started = std::time::Instant::now();
        let (fine, report) = compile(&shipped);
        let compile_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert!(
            !report.has_errors(),
            "{id:?}: the finer raster must ship clean: {:?}",
            report.errors().map(|e| e.message.clone()).collect::<Vec<_>>()
        );
        assert!(compile_ms < 1500.0, "{id:?}: compile + report {compile_ms:.0} ms (budget 1.5 s)");
        assert_eq!(fine.heightmap.cell_size_m(), 2.5);

        let mut coarse = shipped.clone();
        coarse.grid.cell_m = 5.0;
        let (coarse_map, _) = compile(&coarse);
        let fine_kink = worst_slope_kink(&fine.heightmap);
        let coarse_kink = worst_slope_kink(&coarse_map.heightmap);
        assert!(
            fine_kink <= coarse_kink * 0.8,
            "{id:?}: the worst slope kink under a drive line is {fine_kink:.3} at 2.5 m against \
             {coarse_kink:.3} at 5 m — the finer grid must take at least a fifth off it"
        );
    }
}
