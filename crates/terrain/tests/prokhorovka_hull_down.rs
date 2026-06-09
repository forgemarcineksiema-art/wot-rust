use terrain::{HeightMap, prokhorovka_hill_252_2};

/// A tank hull-down on the eastern hill must mask its hull behind the crest while its turret
/// fires over it, seen from an attacker pushing the centre. Scans the reverse shelf for such a
/// spot so the test locks the *existence* of a usable hull-down line, not one tuned point.
#[test]
fn eastern_hill_offers_a_hull_down_firing_line_toward_the_centre() {
    let map = prokhorovka_hill_252_2();
    let hm = &map.heightmap;

    let enemy = ground(hm, 640.0, 350.0);
    let enemy_eye = (enemy.0, enemy.1 + 2.3, enemy.2);

    let mut found = false;
    let mut best_hull = f32::NEG_INFINITY;
    let mut best_turret = f32::NEG_INFINITY;
    let mut bx = 735.0;
    while bx <= 800.0 {
        let g = ground(hm, bx, 350.0);
        let hull = (g.0, g.1 + 0.9, g.2);
        let turret = (g.0, g.1 + 2.4, g.2);
        let hull_masked = blockage(hm, enemy_eye, hull); // > 0 => crest rises above the hull line
        let turret_clear = clearance(hm, enemy_eye, turret); // > 0 => line stays above terrain
        if hull_masked > 0.4 && turret_clear > 0.4 {
            found = true;
            best_hull = best_hull.max(hull_masked);
            best_turret = best_turret.max(turret_clear);
        }
        bx += 3.0;
    }
    assert!(
        found,
        "the eastern hill must offer a hull-down spot (hull masked behind the crest, turret over it)"
    );
    assert!(
        best_hull > 0.5 && best_turret > 0.5,
        "the hull-down line must be pronounced (hull masked {best_hull:.2} m, turret clear {best_turret:.2} m)"
    );
}

#[test]
fn hill_has_a_crest_with_a_reverse_slope() {
    let map = prokhorovka_hill_252_2();
    let hm = &map.heightmap;

    // The crest must be a local ridge: ground rises to it from the centre, then drops onto the
    // reverse shelf behind it.
    let approach = ground(hm, 700.0, 350.0).1;
    let crest = ground(hm, 726.0, 350.0).1;
    let shelf = ground(hm, 752.0, 350.0).1;

    assert!(crest > approach + 0.4, "crest must rise above the western approach");
    assert!(crest > shelf + 0.8, "a reverse shelf must sit below the crest (hull cover)");
}

fn ground(hm: &HeightMap, x: f32, z: f32) -> (f32, f32, f32) {
    (x, hm.sample_height(x, z).expect("inside map"), z)
}

/// Largest amount the terrain rises above the straight sightline (> 0 means it is masked).
fn blockage(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
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
fn clearance(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
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
