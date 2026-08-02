use terrain::{HeightMap, TerrainChunkId};

#[test]
fn heightmap_samples_bilinear_height() {
    let heightmap = HeightMap::new(2, 2, 1.0, vec![0.0, 10.0, 20.0, 30.0]).unwrap();

    assert_eq!(heightmap.sample_height(0.0, 0.0).unwrap(), 0.0);
    assert!((heightmap.sample_height(0.5, 0.5).unwrap() - 15.0).abs() < 0.001);
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
