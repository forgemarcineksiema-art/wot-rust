//! Shared sightline fixtures for map playability tests.
//!
//! Every map test asks the same three questions of a heightmap — is a line masked, is it
//! clear, how steep is the walk — and the hull-down scan composes them. One copy here; the
//! per-map judgement (which lines, which bounds) stays in the map's own test.
#![allow(dead_code)]

use terrain::HeightMap;

/// Largest amount the terrain rises above the straight sightline (> 0 means it is masked).
pub fn blockage(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    let mut worst = f32::NEG_INFINITY;
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.2 + (to.2 - from.2) * t;
        let line_y = from.1 + (to.1 - from.1) * t;
        if let Some(g) = hm.sample_height(x, z) {
            worst = worst.max(g - line_y);
        }
    }
    worst
}

/// Smallest gap the sightline keeps above the terrain (> 0 everywhere means it is visible).
pub fn clearance(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    let mut worst = f32::INFINITY;
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.2 + (to.2 - from.2) * t;
        let line_y = from.1 + (to.1 - from.1) * t;
        if let Some(g) = hm.sample_height(x, z) {
            worst = worst.min(line_y - g);
        }
    }
    worst
}

/// The steepest 5 m-step grade along a straight walk between two points.
pub fn max_grade(hm: &HeightMap, from: (f32, f32), to: (f32, f32)) -> f32 {
    let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
    let steps = (length / 5.0).ceil().max(1.0) as u32;
    let mut worst = 0.0_f32;
    let mut previous: Option<f32> = None;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.1 + (to.1 - from.1) * t;
        let Some(h) = hm.sample_height(x, z) else { continue };
        if let Some(previous) = previous {
            worst = worst.max((h - previous).abs() / (length / steps as f32));
        }
        previous = Some(h);
    }
    worst
}

/// Scans `shelf_from..shelf_to` at `z` for a spot where the hull is masked (> 0.4 m) and the
/// turret clears (> 0.4 m) against an attacker eye at `enemy` — the existence of a usable
/// hull-down line, not one tuned point.
pub fn assert_hull_down_line(
    hm: &HeightMap,
    enemy: (f32, f32),
    shelf_from: f32,
    shelf_to: f32,
    z: f32,
) {
    let enemy_ground = hm.sample_height(enemy.0, enemy.1).expect("inside map");
    let enemy_eye = (enemy.0, enemy_ground + 2.3, enemy.1);
    let mut found = false;
    let mut bx = shelf_from;
    while bx <= shelf_to {
        let g = hm.sample_height(bx, z).expect("inside map");
        let hull = (bx, g + 0.9, z);
        let turret = (bx, g + 2.4, z);
        if blockage(hm, enemy_eye, hull) > 0.4 && clearance(hm, enemy_eye, turret) > 0.4 {
            found = true;
            break;
        }
        bx += 3.0;
    }
    assert!(
        found,
        "no usable hull-down spot on the shelf x {shelf_from}..{shelf_to} at z {z} \
         against an eye at {enemy:?}"
    );
}
