use terrain::{HeightMap, TerrainChunkId};

#[test]
fn heightmap_samples_the_cell_surface() {
    // Zero-twist cell (0+30 == 10+20): the triangle planes and the old bilinear patch
    // agree everywhere, so the classic midpoint value stands.
    let heightmap = HeightMap::new(2, 2, 1.0, vec![0.0, 10.0, 20.0, 30.0]).unwrap();

    assert_eq!(heightmap.sample_height(0.0, 0.0).unwrap(), 0.0);
    assert!((heightmap.sample_height(0.5, 0.5).unwrap() - 15.0).abs() < 0.001);
}

/// The honesty lock on the ground surface itself: the sampler stands on the DRAWN
/// triangles (the render mesh's anti-diagonal split), not on a bilinear patch of its own.
/// Corners 0/10/20/50 twist the cell by 20 m: bilinear would answer 20.0 at the centre —
/// half a metre of ground the eye never saw at map scale — while both drawn planes answer
/// 15.0. Off-centre probes pin each triangle; edge midpoints stay bilinear-identical, so
/// cross-cell continuity is explicit here too.
#[test]
fn a_twisted_cell_samples_the_drawn_triangles_not_a_bilinear_patch() {
    let heightmap = HeightMap::new(2, 2, 1.0, vec![0.0, 10.0, 20.0, 50.0]).unwrap();

    // The shared diagonal midpoint: both planes agree, bilinear would say 20.0.
    assert_eq!(heightmap.sample_height(0.5, 0.5).unwrap(), 15.0);
    // Lower triangle (tx + tz <= 1): plane through h00, h10, h01.
    assert_eq!(heightmap.sample_height(0.25, 0.25).unwrap(), 7.5);
    // Upper triangle: plane through h11, h01, h10.
    assert_eq!(heightmap.sample_height(0.75, 0.75).unwrap(), 32.5);
    // Cell edges are linear in both definitions — bit-for-bit unchanged.
    assert_eq!(heightmap.sample_height(0.5, 0.0).unwrap(), 5.0);
    assert_eq!(heightmap.sample_height(0.0, 0.5).unwrap(), 10.0);
    assert_eq!(heightmap.sample_height(1.0, 0.5).unwrap(), 30.0);
    assert_eq!(heightmap.sample_height(0.5, 1.0).unwrap(), 35.0);
}

#[test]
fn world_position_maps_to_signed_chunk_coordinates() {
    let chunk = TerrainChunkId::from_world_position(130.0, 0.0, -1.0);

    assert_eq!(chunk.x, 1);
    assert_eq!(chunk.z, -1);
}

#[test]
fn heightmap_samples_the_exact_far_edge() {
    // 3x3 grid, cell 1.0 => extent 2.0. The closed boundary must be sampleable.
    let heightmap =
        HeightMap::new(3, 3, 1.0, vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0, 20.0, 21.0, 22.0]).unwrap();

    assert_eq!(heightmap.extent_m(), [2.0, 2.0]);
    // Far corner equals the last sample (index 8 == 22.0), not None.
    assert!((heightmap.sample_height(2.0, 2.0).unwrap() - 22.0).abs() < 1e-4);
    // Just past the boundary is genuinely off-map.
    assert!(heightmap.sample_height(2.0001, 0.0).is_none());
}

/// The split rule is per cell and PICKS THE FLATTER DIAGONAL: with the main pair nearly
/// level (0/5) against a 10/20 anti pair, the cell must triangulate on the main diagonal,
/// and both planes must answer for their own halves.
#[test]
fn a_flat_main_diagonal_wins_the_split() {
    let heightmap = HeightMap::new(2, 2, 1.0, vec![0.0, 10.0, 20.0, 5.0]).unwrap();
    // tx >= tz: the plane through h00, h10, h11.
    assert_eq!(heightmap.sample_height(0.75, 0.25).unwrap(), 0.0 + 7.5 + 0.25 * (5.0 - 10.0));
    // tx < tz: the plane through h00, h01, h11.
    assert_eq!(heightmap.sample_height(0.25, 0.75).unwrap(), 0.0 + 15.0 + 0.25 * (5.0 - 20.0));
    // The shared main diagonal is linear between h00 and h11.
    assert_eq!(heightmap.sample_height(0.5, 0.5).unwrap(), 2.5);
    // Edges stay bilinear-identical.
    assert_eq!(heightmap.sample_height(0.5, 0.0).unwrap(), 5.0);
    assert_eq!(heightmap.sample_height(0.0, 0.5).unwrap(), 10.0);
}

/// THE FAIRNESS LOCK the fixed anti-diagonal failed: mirroring a map across z must mirror
/// the sampled surface BETWEEN nodes too, or the two halves of a fair map fight on
/// different micro-ground (the old split diverged by the full twist term — decimetres).
/// The rule picks the same geometric diagonal on both halves, so mirrored probes agree to
/// float noise: the twin walks the same plane through different summand order, which costs
/// ULPs, never ground. (Determinism is untouched — both ends of the wire sample identical
/// points with identical code; this lock is about the DESIGN promise of fair ground.)
#[test]
fn mirrored_maps_sample_mirrored_ground_between_nodes() {
    let size = 7usize;
    let height_at = |x: f32, z: f32| {
        5.0 + (x * 0.7).sin() * 2.0 + (z * 0.45).cos() * 1.5 + (x * 0.31 + z * 0.17).sin()
    };
    let extent = (size - 1) as f32 * 5.0;
    let mut samples = Vec::new();
    let mut mirrored = Vec::new();
    for zi in 0..size {
        for xi in 0..size {
            samples.push(height_at(xi as f32 * 5.0, zi as f32 * 5.0));
        }
    }
    for zi in 0..size {
        for xi in 0..size {
            mirrored.push(height_at(xi as f32 * 5.0, extent - zi as f32 * 5.0));
        }
    }
    let map = HeightMap::new(size, size, 5.0, samples).unwrap();
    let twin = HeightMap::new(size, size, 5.0, mirrored).unwrap();
    let mut probes = 0;
    for xi in 0..(size - 1) * 4 {
        for zi in 0..(size - 1) * 4 {
            let (x, z) = (xi as f32 * 1.25 + 0.6, zi as f32 * 1.25 + 0.35);
            if x > extent || z > extent {
                continue;
            }
            probes += 1;
            let here = map.sample_height(x, z).unwrap();
            let there = twin.sample_height(x, extent - z).unwrap();
            assert!(
                (here - there).abs() < 1.0e-4,
                "mirror fairness broke between nodes at ({x}, {z}): {here} vs {there}"
            );
        }
    }
    assert!(probes > 300, "the sweep actually probed the interior ({probes})");
}

/// Teren W3b: a crag never floats. Seated on a slope, its box bottom sits at or below the
/// ground under EVERY footprint corner — the stone sinks into the hill instead of hovering
/// at the downhill edge the way a centre-seated axis-aligned box would.
#[test]
fn a_crag_seats_into_the_slope_it_stands_on() {
    let heightmap = terrain::heightmap_from_fn(21, 5.0, |x, _| x * 0.4); // a steady 0.4 slope in x
    let crag = terrain::grounded_cover(
        &heightmap,
        "crag",
        "crag",
        terrain::StaticCoverKind::Crag,
        [50.0, 50.0],
        [5.0, 2.5, 3.0],
    );
    let bottom = crag.center[1] - crag.half_extents_m[1];
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner_ground = heightmap.sample_height(50.0 + sx * 5.0, 50.0 + sz * 3.0).unwrap();
        assert!(
            bottom <= corner_ground + 1.0e-4,
            "the crag floats at a corner: bottom {bottom} over ground {corner_ground}"
        );
    }
    // And an ordinary building still seats at its centre ground.
    let barn = terrain::grounded_cover(
        &heightmap,
        "barn",
        "barn",
        terrain::StaticCoverKind::FarmBuilding,
        [50.0, 50.0],
        [5.0, 2.5, 3.0],
    );
    assert!((barn.center[1] - (20.0 + 2.5)).abs() < 1.0e-3);
}
