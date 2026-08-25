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
