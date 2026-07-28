//! The chirality lock, one level up from the camera math: a rendered TILE must read like a
//! photograph. The unambiguous witness is the gun — on a T-54 it overhangs the bow by ~2.9 m,
//! so the thin end of the silhouette says which way the vehicle points.
//!
//! On a photograph of a tank's port side the bow runs to the left of frame; from its starboard
//! side, to the right. A mirrored rasterizer swaps them — and nothing else in the gate suite
//! notices, because bounds, ratios and budgets are all symmetric.

use game_core::VehicleKind;
use vehicle_forge::bake_studio_bundle;

/// Mean silhouette height (in pixels) over the first and last 40 columns of a tile. The gun end
/// is thin; the hull end is tall.
fn end_thickness(png: &[u8]) -> (f32, f32) {
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().expect("tile decodes");
    let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("tile frame");
    let (w, h, channels) = (info.width as usize, info.height as usize, info.color_type.samples());
    let background = [244u8, 244, 240];

    let column_ink = |x: usize| -> usize {
        (0..h)
            .filter(|&y| {
                // The camera label is burned into the top-left corner of every tile.
                if y < 34 && x < 200 {
                    return false;
                }
                let index = (y * w + x) * channels;
                let delta: i32 = buf[index..index + 3]
                    .iter()
                    .zip(background)
                    .map(|(a, b)| (i32::from(*a) - i32::from(b)).abs())
                    .sum();
                delta > 30
            })
            .count()
    };

    let occupied: Vec<usize> = (0..w).filter(|&x| column_ink(x) > 0).collect();
    assert!(occupied.len() > 80, "tile must contain a silhouette");
    let (first, last) = (occupied[0], occupied[occupied.len() - 1]);
    let mean = |range: std::ops::Range<usize>| -> f32 {
        let values: Vec<usize> = range.map(column_ink).collect();
        values.iter().sum::<usize>() as f32 / values.len() as f32
    };
    (mean(first..first + 40), mean(last - 40..last))
}

#[test]
fn the_profile_tiles_point_the_gun_the_way_a_photograph_would() {
    let bundle = bake_studio_bundle(VehicleKind::T54_1951).expect("studio bundle");
    let tile = |name: &str| {
        bundle
            .views()
            .iter()
            .find(|view| view.name == name)
            .unwrap_or_else(|| panic!("{name} tile"))
            .png
            .clone()
    };

    let (port_left, port_right) = end_thickness(&tile("left_profile.png"));
    assert!(
        port_left < port_right * 0.5,
        "left_profile.png: seen from the port side the bow (thin gun end) runs LEFT — got \
         {port_left:.1} px of ink at the left edge vs {port_right:.1} px at the right"
    );

    let (starboard_left, starboard_right) = end_thickness(&tile("right_profile.png"));
    assert!(
        starboard_right < starboard_left * 0.5,
        "right_profile.png: seen from the starboard side the bow runs RIGHT — got \
         {starboard_right:.1} px of ink at the right edge vs {starboard_left:.1} px at the left"
    );
}
